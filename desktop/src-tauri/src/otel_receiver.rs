//! Opt-in local OTLP/HTTP receiver (M5), bound to 127.0.0.1.
//! /v1/traces accepts OTLP JSON/protobuf; referenced CodeBuddy exports use protobuf.
//! Normalize per-request spans into adapter-readable JSONL after bounded gzip decoding.
//! The managed output directory contains otel/spans.jsonl; supplemental logs stay separate.
//!
//! Receiver rules (execution.md M5/V22/V25):
//! - Disabled by default; enabling authorizes this local instance.
//! - Bind loopback only; verify each source Bearer token against native current-user credential storage.
//! - Headers <=64 KiB, body <=64 MiB, and decompressed gzip <=64 MiB.
//! - Retain only named span identity/timing fields and explicitly selected usage/model/session attributes.
//!   Do not accept arbitrary attribute prefixes or persist bodies/credentials.
//! - Authenticate before body processing; reject browser Origin and forwarded requests.
//! - Limit all sources together to 120 requests/minute and four active connections; return HTTP 429.

use crate::receiver_auth::Family;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
type Authority = std::sync::Arc<dyn Fn(&str, &str) -> Option<Family> + Send + Sync>;

/// Keep one owned listener; an occupied port belonging to another process is an error.
pub fn ensure_started(port: u16, out_dir: PathBuf) -> Result<(), String> {
    ensure_started_owned(port, out_dir).map(|_| ())
}

struct Running {
    port: u16,
    out_dir: PathBuf,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}
static LISTENER: std::sync::Mutex<Option<Running>> = std::sync::Mutex::new(None);

struct AdmissionState {
    active: usize,
    used: usize,
    window: std::time::Instant,
}
struct Admission(std::sync::Mutex<AdmissionState>);
impl Admission {
    fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self(std::sync::Mutex::new(AdmissionState {
            active: 0,
            used: 0,
            window: std::time::Instant::now(),
        })))
    }
    fn accept(self: &std::sync::Arc<Self>, now: std::time::Instant) -> Option<Permit> {
        let mut state = self.0.lock().ok()?;
        if now.saturating_duration_since(state.window) >= std::time::Duration::from_secs(60) {
            state.used = 0;
            state.window = now;
        }
        if state.active >= 4 || state.used >= 120 {
            return None;
        }
        state.active += 1;
        state.used += 1;
        Some(Permit(std::sync::Arc::clone(self)))
    }
}
struct Permit(std::sync::Arc<Admission>);
impl Drop for Permit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0 .0.lock() {
            state.active -= 1;
        }
    }
}

/// True only when this call starts a listener, allowing configuration rollback to stop its own listener.
pub fn ensure_started_owned(port: u16, out_dir: PathBuf) -> Result<bool, String> {
    let mut current = LISTENER.lock().map_err(|_| "receiver_busy")?;
    if let Some(active) = current.as_ref() {
        return if active.port == port && active.out_dir == out_dir {
            Ok(false)
        } else {
            Err("receiver_restart_required".into())
        };
    }
    *current = Some(start_owned(port, out_dir).map_err(|e| {
        if e == "credential_store_unavailable" {
            e
        } else {
            "receiver_bind_failed".into()
        }
    })?);
    Ok(true)
}

pub fn stop_owned(port: u16, out_dir: &std::path::Path) {
    if let Ok(mut current) = LISTENER.lock() {
        if current
            .as_ref()
            .is_some_and(|r| r.port == port && r.out_dir == out_dir)
        {
            if let Some(running) = current.take() {
                running
                    .stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = running.thread.join();
            }
        }
    }
}

/// Maximum body bytes.
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
/// Maximum header bytes.
const MAX_HEAD_BYTES: usize = 64 * 1024;
/// Maximum decompressed gzip bytes.
const MAX_GZIP_BYTES: usize = 64 * 1024 * 1024;
/// Exact keys consumed by the checked span adapter; review new attributes before adding them.
const ALLOWED_ATTR_KEYS: &[&str] = &[
    "gen_ai.usage.input_tokens",
    "gen_ai.usage.output_tokens",
    "gen_ai.usage.cache_read.input_tokens",
    "gen_ai.usage.cache_creation.input_tokens",
    "gen_ai.usage.reasoning.output_tokens",
    "gen_ai.usage.reasoning_tokens",
    "gen_ai.request.model",
    "gen_ai.response.model",
    "gen_ai.response.time_to_first_chunk",
    "gen_ai.conversation.id",
    "gen_ai.session.id",
    "usage.input_tokens",
    "usage.output_tokens",
    "usage.total_tokens",
    "usage.cache_read_input_tokens",
    "usage.cache_creation_input_tokens",
    "usage.reasoning_tokens",
    "model_name",
    "request.model",
    "response.time_to_first_token",
    "copilot_chat.time_to_first_token",
    "copilot_chat.chat_session_id",
    "copilot_chat.session_id",
    "copilot_chat.parent_chat_session_id",
    "github.copilot.turn_id",
];
const ALLOWED_RESOURCE_KEYS: &[&str] = &["service.name"];
/// Maximum scalar string bytes; discard oversized values instead of persisting body-like text.
const MAX_ATTR_STRING_BYTES: usize = 256;

/// Start the test receiver thread; return bind errors, and exit the thread with the process.
#[cfg(test)]
pub fn start(port: u16, out_dir: PathBuf) -> Result<(), String> {
    start_with_admission(port, out_dir, Admission::new(), test_authority()).map(|_| ())
}

#[cfg(test)]
fn test_authority() -> Authority {
    std::sync::Arc::new(|header, _| {
        (header == "Bearer receiver-fixture").then_some(Family::Synthetic)
    })
}

fn start_owned(port: u16, out_dir: PathBuf) -> Result<Running, String> {
    if !crate::receiver_auth::available() {
        return Err("credential_store_unavailable".into());
    }
    let app = out_dir
        .parent()
        .ok_or("receiver_bind_failed")?
        .to_path_buf();
    let authority: Authority = std::sync::Arc::new(move |header, path| {
        crate::receiver_auth::authorize(&crate::receiver_auth::SystemStore, &app, header, path)
    });
    start_with_admission(port, out_dir, Admission::new(), authority)
}
fn start_with_admission(
    port: u16,
    out_dir: PathBuf,
    admission: std::sync::Arc<Admission>,
    authority: Authority,
) -> Result<Running, String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|_| "listener address")?
        .port();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_stop = std::sync::Arc::clone(&stop);
    let target = out_dir.clone();
    let thread = std::thread::Builder::new()
        .name("otel-receiver".to_string())
        .spawn(move || {
            let listener = listener;
            listener.set_nonblocking(true).ok();
            while !thread_stop.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let Some(permit) = admission.accept(std::time::Instant::now()) else {
                            let _ = stream.set_nonblocking(false);
                            let _ =
                                stream.set_write_timeout(Some(std::time::Duration::from_secs(1)));
                            let _ = respond(&mut stream, 429, "receiver request limit");
                            // Explicit FIN and bounded input draining preserve the response
                            // during Winsock connection shutdown.
                            // Rejected input is neither parsed nor persisted.
                            let _ = stream.shutdown(std::net::Shutdown::Write);
                            let deadline =
                                std::time::Instant::now() + std::time::Duration::from_millis(100);
                            let mut drained = 0;
                            let mut buffer = [0; 4096];
                            while drained < MAX_HEAD_BYTES {
                                let remaining =
                                    deadline.saturating_duration_since(std::time::Instant::now());
                                if remaining.is_zero()
                                    || stream.set_read_timeout(Some(remaining)).is_err()
                                {
                                    break;
                                }
                                match stream.read(&mut buffer) {
                                    Ok(0) | Err(_) => break,
                                    Ok(n) => drained += n,
                                }
                            }
                            continue;
                        };
                        let out = target.clone();
                        let authority = std::sync::Arc::clone(&authority);
                        let _ = std::thread::Builder::new()
                            .name("otel-request".into())
                            .spawn(move || {
                                let _permit = permit;
                                let _ = handle(stream, &out, &authority);
                            });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                    Err(_) => return,
                }
            }
        })
        .map_err(|e| format!("spawn receiver: {e}"))?;
    Ok(Running {
        port,
        out_dir,
        stop,
        thread,
    })
}

const CRLF: &str = "\u{0d}\u{0a}";

/// Handle one HTTP request per connection, then close the connection.
fn handle(
    mut stream: std::net::TcpStream,
    out_dir: &std::path::Path,
    authority: &Authority,
) -> std::io::Result<()> {
    // Restore blocking reads/writes explicitly after accepting from a nonblocking listener.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    // Read until headers end or the configured size limit is reached.
    let head_end = loop {
        if let Some(pos) = find_head_end(&buf) {
            break pos;
        }
        if buf.len() > MAX_HEAD_BYTES + 8 {
            respond(&mut stream, 431, "headers too large")?;
            return Ok(());
        }
        let n = match stream.read(&mut chunk) {
            Ok(n) => n,
            Err(e) => {
                return Err(e);
            }
        };
        if n == 0 {
            respond(&mut stream, 400, "truncated request")?;
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    if head_end > MAX_HEAD_BYTES {
        respond(&mut stream, 431, "headers too large")?;
        return Ok(());
    }
    let mut lines = head.split(CRLF);
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let mut content_length = 0usize;
    let mut content_type = String::new();
    let mut gzip = false;
    let mut authorization = None;
    let mut duplicate_authorization = false;
    let mut forwarded = false;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        if k == "authorization" {
            duplicate_authorization |= authorization.is_some();
            authorization = Some(v);
        }
        if matches!(
            k.as_str(),
            "origin"
                | "forwarded"
                | "via"
                | "x-forwarded-for"
                | "x-forwarded-host"
                | "x-forwarded-proto"
        ) {
            forwarded = true;
        }
        if k == "content-length" {
            content_length = v.parse().unwrap_or(0);
        } else if k == "content-type" {
            content_type = v.to_ascii_lowercase();
        } else if k == "content-encoding" && v.eq_ignore_ascii_case("gzip") {
            gzip = true;
        }
    }
    if method != "POST" || !matches!(path, "/v1/traces" | "/v1/logs" | "/v1/traces/supplemental") {
        respond(&mut stream, 404, "only POST /v1/traces or /v1/logs")?;
        return Ok(());
    }
    if forwarded {
        respond(&mut stream, 403, "only confirmed local exporters")?;
        return Ok(());
    }
    let family = if duplicate_authorization {
        None
    } else {
        authorization.and_then(|header| authority(header, path))
    };
    let Some(family) = family else {
        respond(&mut stream, 401, "exporter authentication required")?;
        return Ok(());
    };
    if content_length > MAX_BODY_BYTES {
        respond(&mut stream, 413, "body too large")?;
        return Ok(());
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_BODY_BYTES {
            respond(&mut stream, 413, "body too large")?;
            return Ok(());
        }
    }
    if body.len() < content_length {
        respond(&mut stream, 400, "truncated body")?;
        return Ok(());
    }
    body.truncate(content_length);
    if gzip {
        let decoder = flate2::read::GzDecoder::new(&body[..]);
        let mut out = Vec::new();
        if decoder
            .take(MAX_GZIP_BYTES as u64 + 1)
            .read_to_end(&mut out)
            .is_err()
        {
            respond(&mut stream, 400, "invalid gzip")?;
            return Ok(());
        }
        if out.len() > MAX_GZIP_BYTES {
            respond(&mut stream, 413, "decompressed body too large")?;
            return Ok(());
        }
        body = out;
    }
    if content_type.contains("json") && serde_json::from_slice::<serde_json::Value>(&body).is_err()
    {
        respond(&mut stream, 400, "invalid JSON")?;
        return Ok(());
    }
    let logs = path == "/v1/logs";
    let records = if logs {
        if content_type.contains("json") {
            parse_logs_json(&body)
        } else {
            parse_logs_protobuf(&body)
        }
    } else if content_type.contains("json") {
        parse_otlp_json(&body)
    } else {
        // Non-JSON content uses protobuf, the referenced CodeBuddy export encoding.
        parse_otlp_protobuf(&body)
    };
    let records = records
        .into_iter()
        .filter(|record| family.accepts_record(record))
        .collect::<Vec<_>>();
    let count = records.len();
    if count > 0 {
        // Supplemental logs are not auto-scanned as spans or added to native session usage.
        let output = if logs {
            out_dir
                .parent()
                .unwrap_or(out_dir)
                .join("telemetry/otlp-logs.jsonl")
        } else if path == "/v1/traces/supplemental" {
            out_dir
                .parent()
                .unwrap_or(out_dir)
                .join("telemetry/otlp-traces.jsonl")
        } else {
            out_dir.join("spans.jsonl")
        };
        std::fs::create_dir_all(output.parent().unwrap_or(out_dir))?;
        static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = WRITE_LOCK
            .lock()
            .map_err(|_| std::io::Error::other("receiver busy"))?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(output)?;
        for record in records {
            file.write_all(record.to_string().as_bytes())?;
            file.write_all(b"\n")?;
        }
    }
    let r = if content_type.contains("json") {
        respond(&mut stream, 200, "")
    } else {
        write!(stream,"HTTP/1.1 200 OK{CRLF}Content-Type: application/x-protobuf{CRLF}Content-Length: 0{CRLF}Connection: close{CRLF}{CRLF}")
    };
    r?;
    let _ = count;
    Ok(())
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == [0x0d, 0x0a, 0x0d, 0x0a])
}

const LOG_KEYS: &[&str] = &[
    "event.name",
    "event_name",
    "model",
    "model_name",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_creation_tokens",
    "cached_input_tokens",
    "total_tokens",
    "reasoning_output_tokens",
    "duration_ms",
    "success",
    "status_code",
    "gen_ai.usage.input_tokens",
    "gen_ai.usage.output_tokens",
    "gen_ai.request.model",
];
const LOG_EVENTS: &[&str] = &[
    "claude_code.api_request",
    "claude_code.api_error",
    "api_request",
    "api_error",
    "codex.api_request",
    "codex.sse_event",
    "codex.websocket_event",
    "codex.conversation_starts",
];

fn log_attributes(
    list: Option<&Vec<serde_json::Value>>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    for attr in list.into_iter().flatten() {
        let key = attr["key"].as_str().unwrap_or_default();
        if LOG_KEYS.contains(&key) {
            if let Some(value) = filtered_scalar(&attr["value"]) {
                if !value.is_object() && !value.is_array() {
                    out.insert(key.into(), value);
                }
            }
        }
    }
    out
}

fn normalize_log(log: &serde_json::Value, service: Option<serde_json::Value>) -> serde_json::Value {
    let attrs = log_attributes(log["attributes"].as_array());
    let name = log["eventName"]
        .as_str()
        .or_else(|| attrs.get("event.name").and_then(|v| v.as_str()))
        .or_else(|| attrs.get("event_name").and_then(|v| v.as_str()))
        .or_else(|| log.pointer("/body/stringValue").and_then(|v| v.as_str()))
        .filter(|name| LOG_EVENTS.contains(name))
        .unwrap_or("unknown");
    let time = log["timeUnixNano"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| log["timeUnixNano"].as_u64());
    serde_json::json!({"name":name,"time_unix_nano":time.map(|v|v.to_string()),"service":service,"attributes":attrs})
}

fn parse_logs_json(body: &[u8]) -> Vec<serde_json::Value> {
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(body) else {
        return vec![];
    };
    let mut out = Vec::new();
    for resource in doc["resourceLogs"].as_array().into_iter().flatten() {
        let service = resource
            .pointer("/resource/attributes")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .find(|a| a["key"] == "service.name")
            .and_then(|a| filtered_scalar(&a["value"]));
        for scope in resource["scopeLogs"].as_array().into_iter().flatten() {
            for log in scope["logRecords"].as_array().into_iter().flatten() {
                out.push(normalize_log(log, service.clone()));
            }
        }
    }
    out
}

fn parse_logs_protobuf(body: &[u8]) -> Vec<serde_json::Value> {
    let mut resources = Vec::new();
    if iter_fields(body, |field, wire| {
        if let (1, Wire::Bytes(bytes)) = (field, wire) {
            resources.push(bytes);
        }
        Some(())
    })
    .is_none()
    {
        return vec![];
    }
    let mut out = Vec::new();
    for resource in resources {
        let mut service = None;
        let mut scopes = Vec::new();
        if iter_fields(resource, |field, wire| {
            match (field, wire) {
                (1, Wire::Bytes(bytes)) => {
                    iter_fields(bytes, |f, w| {
                        if let (1, Wire::Bytes(kv)) = (f, w) {
                            if let Some((k, v)) = protobuf_keyvalue(kv) {
                                if k == "service.name" {
                                    service = filtered_scalar(&v);
                                }
                            }
                        }
                        Some(())
                    })?;
                }
                (2, Wire::Bytes(bytes)) => scopes.push(bytes),
                _ => {}
            }
            Some(())
        })
        .is_none()
        {
            continue;
        }
        for scope in scopes {
            let mut records = Vec::new();
            if iter_fields(scope, |f, w| {
                if let (2, Wire::Bytes(b)) = (f, w) {
                    records.push(b);
                }
                Some(())
            })
            .is_none()
            {
                continue;
            }
            for record in records {
                let mut log = serde_json::json!({"attributes":[]});
                let mut attrs = Vec::new();
                let parsed = iter_fields(record, |f, w| {
                    match (f, w) {
                        (1, Wire::Fixed64(v)) => {
                            log["timeUnixNano"] = serde_json::json!(v.to_string())
                        }
                        (12, Wire::Bytes(v)) => {
                            log["eventName"] = serde_json::json!(String::from_utf8_lossy(v))
                        }
                        (6, Wire::Bytes(kv)) => {
                            if let Some((k, v)) = protobuf_keyvalue(kv) {
                                attrs.push(serde_json::json!({"key":k,"value":v}));
                            }
                        }
                        (5, Wire::Bytes(any)) => {
                            iter_fields(any, |af, aw| {
                                if let (1, Wire::Bytes(v)) = (af, aw) {
                                    let name = String::from_utf8_lossy(v);
                                    if LOG_EVENTS.contains(&name.as_ref()) {
                                        log["eventName"] = serde_json::json!(name);
                                    }
                                }
                                Some(())
                            })?;
                        }
                        _ => {}
                    }
                    Some(())
                });
                if parsed.is_some() {
                    log["attributes"] = serde_json::json!(attrs);
                    out.push(normalize_log(&log, service.clone()));
                }
            }
        }
    }
    out
}

fn respond(stream: &mut std::net::TcpStream, status: u16, msg: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let body = if status == 200 {
        "{}".to_string()
    } else {
        serde_json::json!({"code":status,"message":msg}).to_string()
    };
    let retry = if status == 429 {
        "Retry-After: 60\r\n"
    } else {
        ""
    };
    let auth = if status == 401 {
        "WWW-Authenticate: Bearer\r\n"
    } else {
        ""
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}{CRLF}Content-Type: application/json{CRLF}Content-Length: {}{CRLF}{retry}{auth}Connection: close{CRLF}{CRLF}{body}",
        body.len()
    )
}

/// Flatten only explicitly permitted attribute values.
fn allowed_attr(key: &str, value: &serde_json::Value) -> Option<serde_json::Value> {
    let keep = ALLOWED_ATTR_KEYS.contains(&key) || ALLOWED_RESOURCE_KEYS.contains(&key);
    if !keep {
        return None;
    }
    filtered_scalar(value)
}

fn filtered_scalar(value: &serde_json::Value) -> Option<serde_json::Value> {
    // Convert OTLP AnyValue string/int/double/bool values to scalars.
    if let Some(obj) = value.as_object() {
        if let Some(v) = obj.get("stringValue").and_then(|v| v.as_str()) {
            if v.len() > MAX_ATTR_STRING_BYTES {
                return None;
            }
            return Some(serde_json::Value::String(v.to_string()));
        }
        for k in ["intValue", "doubleValue"] {
            if let Some(v) = obj.get(k) {
                if let Some(n) = v.as_i64() {
                    return Some(serde_json::Value::from(n));
                }
                if let Some(n) = v.as_str().and_then(|s| s.parse::<i64>().ok()) {
                    return Some(serde_json::Value::from(n));
                }
                if let Some(f) = v.as_f64() {
                    return Some(serde_json::Value::from(f));
                }
            }
        }
        if let Some(v) = obj.get("boolValue").and_then(|v| v.as_bool()) {
            return Some(serde_json::Value::from(v));
        }
        return None;
    }
    if let Some(v) = value.as_str() {
        if v.len() > MAX_ATTR_STRING_BYTES {
            return None;
        }
    }
    if value.is_object() || value.is_array() {
        return None;
    }
    Some(value.clone())
}

/// Normalize OTLP JSON ExportTraceServiceRequest to span records.
fn parse_otlp_json(body: &[u8]) -> Vec<serde_json::Value> {
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let Some(resource_spans) = doc.get("resourceSpans").and_then(|v| v.as_array()) else {
        return out;
    };
    for rs in resource_spans {
        let mut resource_attrs = serde_json::Map::new();
        if let Some(attrs) = rs
            .pointer("/resource/attributes")
            .and_then(|v| v.as_array())
        {
            for attr in attrs {
                let key = attr.get("key").and_then(|v| v.as_str()).unwrap_or("");
                if let (Some(value), Some(filtered)) = (
                    attr.get("value"),
                    allowed_attr(key, attr.get("value").unwrap_or(&serde_json::Value::Null)),
                ) {
                    let _ = value;
                    resource_attrs.insert(key.to_string(), filtered);
                }
            }
        }
        let Some(scope_spans) = rs.get("scopeSpans").and_then(|v| v.as_array()) else {
            continue;
        };
        for ss in scope_spans {
            let Some(spans) = ss.get("spans").and_then(|v| v.as_array()) else {
                continue;
            };
            for span in spans {
                out.push(normalize_span(span, &resource_attrs));
            }
        }
    }
    out
}

/// Map OTLP numeric span kind to its name.
fn otlp_kind(kind: u64) -> &'static str {
    match kind {
        1 => "INTERNAL",
        2 => "SERVER",
        3 => "CLIENT",
        4 => "PRODUCER",
        5 => "CONSUMER",
        _ => "UNSPECIFIED",
    }
}

fn normalize_span(
    span: &serde_json::Value,
    resource_attrs: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut attrs = serde_json::Map::new();
    if let Some(list) = span.get("attributes").and_then(|v| v.as_array()) {
        for attr in list {
            let key = attr.get("key").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(filtered) =
                allowed_attr(key, attr.get("value").unwrap_or(&serde_json::Value::Null))
            {
                attrs.insert(key.to_string(), filtered);
            }
        }
    }
    let start_ms = span
        .get("startTimeUnixNano")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .or_else(|| v.as_i64())
        })
        .map(|nanos| nanos / 1_000_000);
    let mut record = serde_json::Map::new();
    record.insert("name".into(), span.get("name").cloned().unwrap_or_default());
    for key in ["spanId", "span_id"] {
        if let Some(id) = span.get(key).and_then(|v| v.as_str()) {
            record.insert("spanId".into(), serde_json::Value::String(id.to_string()));
            break;
        }
    }
    if let Some(ms) = start_ms {
        record.insert("startTime".into(), serde_json::Value::from(ms));
    }
    // Preserve span-level status metadata separately from filtered attributes.
    // ERROR lets the adapter mark error_status without reporting failed calls as successful.
    if let Some(code) = span.pointer("/status/code") {
        record.insert("status".into(), serde_json::json!({"code": code.clone()}));
    }
    if let Some(kind) = span.get("kind").filter(|v| !v.is_null()) {
        record.insert(
            "kind".into(),
            kind.as_u64()
                .map(|k| serde_json::json!(otlp_kind(k)))
                .unwrap_or_else(|| kind.clone()),
        );
    }
    record.insert("attributes".into(), serde_json::Value::Object(attrs));
    record.insert(
        "resource".into(),
        serde_json::json!({"attributes": resource_attrs}),
    );
    serde_json::Value::Object(record)
}

// Parse ExportTraceServiceRequest protobuf wire fields.

fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    for (i, byte) in data.iter().enumerate().take(10) {
        // A u64 protobuf varint permits only 0/1 in byte ten; larger values are malformed.
        if i == 9 && byte > &1 {
            return None;
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

enum Wire<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
}

fn iter_fields<'a>(
    data: &'a [u8],
    mut visit: impl FnMut(u64, Wire<'a>) -> Option<()>,
) -> Option<()> {
    let mut pos = 0usize;
    while pos < data.len() {
        let (tag, used) = read_varint(&data[pos..])?;
        pos += used;
        let field = tag >> 3;
        match (tag & 0x7) as u8 {
            0 => {
                let (v, used) = read_varint(&data[pos..])?;
                pos += used;
                visit(field, Wire::Varint(v))?;
            }
            1 => {
                if pos + 8 > data.len() {
                    return None;
                }
                let mut b = [0u8; 8];
                b.copy_from_slice(&data[pos..pos + 8]);
                pos += 8;
                visit(field, Wire::Fixed64(u64::from_le_bytes(b)))?;
            }
            2 => {
                let (len, used) = read_varint(&data[pos..])?;
                pos += used;
                let len = usize::try_from(len).ok()?;
                let end = pos.checked_add(len)?;
                if end > data.len() {
                    return None;
                }
                visit(field, Wire::Bytes(&data[pos..end]))?;
                pos = end;
            }
            5 => {
                if pos + 4 > data.len() {
                    return None;
                }
                pos += 4;
            }
            _ => return None,
        }
    }
    Some(())
}

/// Decode protobuf KeyValue into a key and scalar value.
fn protobuf_keyvalue(data: &[u8]) -> Option<(String, serde_json::Value)> {
    let mut key = String::new();
    let mut value: Option<serde_json::Value> = None;
    iter_fields(data, |field, wire| {
        match (field, wire) {
            (1, Wire::Bytes(k)) => key = String::from_utf8_lossy(k).to_string(),
            (2, Wire::Bytes(any)) => {
                iter_fields(any, |f, w| {
                    match (f, w) {
                        (1, Wire::Bytes(s)) => {
                            value = Some(serde_json::Value::String(
                                String::from_utf8_lossy(s).to_string(),
                            ));
                        }
                        (3, Wire::Varint(v)) => {
                            value = Some(serde_json::Value::from(v as i64));
                        }
                        (4, Wire::Fixed64(bits)) => {
                            value = Some(serde_json::Value::from(f64::from_bits(bits)));
                        }
                        (2, Wire::Varint(v)) => {
                            value = Some(serde_json::Value::from(v != 0));
                        }
                        _ => {}
                    }
                    Some(())
                })?;
            }
            _ => {}
        }
        Some(())
    })?;
    Some((key, value?))
}

/// Normalize OTLP protobuf body to span records.
fn parse_otlp_protobuf(body: &[u8]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    iter_fields(body, |field, wire| {
        if let (1, Wire::Bytes(rs)) = (field, wire) {
            let mut resource_attrs = serde_json::Map::new();
            let mut scope_spans: Vec<&[u8]> = Vec::new();
            iter_fields(rs, |f, w| {
                match (f, w) {
                    (1, Wire::Bytes(resource)) => {
                        iter_fields(resource, |rf, rw| {
                            if let (1, Wire::Bytes(kv)) = (rf, rw) {
                                if let Some((k, v)) = protobuf_keyvalue(kv) {
                                    if let Some(filtered) = allowed_attr(&k, &v) {
                                        resource_attrs.insert(k, filtered);
                                    }
                                }
                            }
                            Some(())
                        })?;
                    }
                    (2, Wire::Bytes(ss)) => scope_spans.push(ss),
                    _ => {}
                }
                Some(())
            })?;
            for ss in scope_spans {
                iter_fields(ss, |f, w| {
                    if let (2, Wire::Bytes(span)) = (f, w) {
                        if let Some(record) = protobuf_span(span, &resource_attrs) {
                            out.push(record);
                        }
                    }
                    Some(())
                })?;
            }
        }
        Some(())
    });
    out
}

fn protobuf_span(
    span: &[u8],
    resource_attrs: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut name = String::new();
    let mut span_id: Option<String> = None;
    let mut start_ms: Option<i64> = None;
    let mut status_code: Option<u64> = None;
    let mut kind: Option<u64> = None;
    let mut attrs = serde_json::Map::new();
    iter_fields(span, |field, wire| {
        match (field, wire) {
            (2, Wire::Bytes(id)) => {
                span_id = Some(id.iter().map(|b| format!("{b:02x}")).collect());
            }
            (5, Wire::Bytes(n)) => name = String::from_utf8_lossy(n).to_string(),
            (6, Wire::Varint(k)) => kind = Some(k),
            (7, Wire::Fixed64(nanos)) => {
                start_ms = Some((nanos / 1_000_000) as i64);
            }
            (9, Wire::Bytes(kv)) => {
                if let Some((k, v)) = protobuf_keyvalue(kv) {
                    if let Some(filtered) = allowed_attr(&k, &v) {
                        attrs.insert(k, filtered);
                    }
                }
            }
            // Status submessage field 2 is code; enum 2 means ERROR.
            (15, Wire::Bytes(status)) => {
                iter_fields(status, |f, w| {
                    if let (2, Wire::Varint(c)) = (f, w) {
                        status_code = Some(c);
                    }
                    Some(())
                })?;
            }
            _ => {}
        }
        Some(())
    })?;
    let mut record = serde_json::Map::new();
    record.insert("name".into(), serde_json::Value::String(name));
    record.insert("spanId".into(), serde_json::Value::String(span_id?));
    if let Some(kind) = kind {
        record.insert("kind".into(), serde_json::json!(otlp_kind(kind)));
    }
    if let Some(ms) = start_ms {
        record.insert("startTime".into(), serde_json::Value::from(ms));
    }
    if let Some(code) = status_code {
        record.insert("status".into(), serde_json::json!({"code": code}));
    }
    record.insert("attributes".into(), serde_json::Value::Object(attrs));
    record.insert(
        "resource".into(),
        serde_json::json!({"attributes": resource_attrs}),
    );
    Some(serde_json::Value::Object(record))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_field(field: u64, payload: &[u8]) -> Vec<u8> {
        fn varint(mut n: u64) -> Vec<u8> {
            let mut out = vec![];
            loop {
                let b = (n & 127) as u8;
                n >>= 7;
                out.push(b | if n > 0 { 128 } else { 0 });
                if n == 0 {
                    return out;
                }
            }
        }
        [
            varint(field << 3 | 2),
            varint(payload.len() as u64),
            payload.to_vec(),
        ]
        .concat()
    }

    #[test]
    fn protobuf_any_value_uses_standard_field_numbers() {
        let kv = |any: &[u8]| [bytes_field(1, b"key"), bytes_field(2, any)].concat();
        assert_eq!(
            protobuf_keyvalue(&kv(&[3 << 3, 100])).unwrap().1,
            serde_json::json!(100)
        );
        assert_eq!(
            protobuf_keyvalue(&kv(&[2 << 3, 1])).unwrap().1,
            serde_json::json!(true)
        );
        let double = [vec![4 << 3 | 1], 1.5f64.to_le_bytes().to_vec()].concat();
        assert_eq!(
            protobuf_keyvalue(&kv(&double)).unwrap().1,
            serde_json::json!(1.5)
        );
    }

    #[test]
    #[cfg(windows)]
    fn owned_receiver_reuses_its_listener_but_never_claims_an_external_port() {
        let external = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = external.local_addr().unwrap().port();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/telemetry-receiver-tests/lifecycle/otel");
        assert_eq!(
            ensure_started_owned(port, out.clone()).unwrap_err(),
            "receiver_bind_failed"
        );
        drop(external);
        assert!(ensure_started_owned(port, out.clone()).unwrap());
        assert!(!ensure_started_owned(port, out.clone()).unwrap());
        stop_owned(port, &out);
        let _released = TcpListener::bind(("127.0.0.1", port)).unwrap();
        assert!(!out.exists());
    }

    #[test]
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    fn receiver_without_verified_native_vault_never_opens_an_unprotected_port() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-continuation/unavailable-vault/otel");
        let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        assert_eq!(
            ensure_started_owned(port, out.clone()).unwrap_err(),
            "credential_store_unavailable"
        );
        let _unbound = TcpListener::bind(("127.0.0.1", port)).unwrap();
        assert!(!out.exists());
    }

    #[test]
    fn supplemental_logs_keep_only_usage_fields_and_never_prompts_or_credentials() {
        let body=br#"{"resourceLogs":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"claude-code"}},{"key":"api_key","value":{"stringValue":"SECRET"}}]},"scopeLogs":[{"logRecords":[{"timeUnixNano":"1790000000000000000","body":{"stringValue":"SECRET prompt"},"attributes":[{"key":"event.name","value":{"stringValue":"claude_code.api_request"}},{"key":"input_tokens","value":{"intValue":"123"}},{"key":"output_tokens","value":{"intValue":"0"}},{"key":"prompt","value":{"stringValue":"SECRET"}},{"key":"gen_ai.system_instructions","value":{"stringValue":"SECRET"}}]}]}]}]}"#;
        let records = parse_logs_json(body);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], "claude_code.api_request");
        assert_eq!(records[0]["attributes"]["input_tokens"], 123);
        assert_eq!(records[0]["attributes"]["output_tokens"], 0);
        assert!(!records[0].to_string().contains("SECRET"));
        let arbitrary=br#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{"body":{"stringValue":"private content"}}]}]}]}"#;
        assert_eq!(parse_logs_json(arbitrary)[0]["name"], "unknown");
        assert!(!parse_logs_json(arbitrary)[0]
            .to_string()
            .contains("private content"));
    }

    fn logs_payload() -> Vec<u8> {
        let attr = [
            bytes_field(1, b"input_tokens"),
            bytes_field(2, &[3 << 3, 100]),
        ]
        .concat();
        let record = [
            bytes_field(12, b"codex.sse_event"),
            bytes_field(6, &attr),
            bytes_field(5, &bytes_field(1, b"SECRET body")),
        ]
        .concat();
        bytes_field(1, &bytes_field(2, &bytes_field(2, &record)))
    }

    #[test]
    fn standard_protobuf_logs_are_normalized_without_body() {
        let records = parse_logs_protobuf(&logs_payload());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], "codex.sse_event");
        assert_eq!(records[0]["attributes"]["input_tokens"], 100);
        assert!(!records[0].to_string().contains("SECRET"));
    }

    #[test]
    fn http_logs_return_empty_protobuf_response_and_do_not_enter_span_statistics() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/telemetry-receiver-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let out_dir = root.join("otel");
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let target = out_dir.clone();
        let worker = std::thread::spawn(move || {
            handle(listener.accept().unwrap().0, &target, &test_authority()).unwrap()
        });
        let payload = logs_payload();
        let mut client = std::net::TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(client,"POST /v1/logs HTTP/1.1{CRLF}Authorization: Bearer receiver-fixture{CRLF}Content-Type: application/x-protobuf{CRLF}Content-Length: {}{CRLF}{CRLF}",payload.len()).unwrap();
        client.write_all(&payload).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        worker.join().unwrap();
        assert!(response.contains("Content-Type: application/x-protobuf"));
        assert!(response.contains("Content-Length: 0"));
        assert!(response.ends_with(&format!("{CRLF}{CRLF}")));
        assert!(!out_dir.join("spans.jsonl").exists());
        assert!(
            !std::fs::read_to_string(root.join("telemetry/otlp-logs.jsonl"))
                .unwrap()
                .contains("SECRET")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn json_end_to_end() {
        let body = br#"{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"codebuddy"}}]},"scopeSpans":[{"spans":[{"traceId":"aa","spanId":"bb","name":"model_stream","kind":1,"startTimeUnixNano":"1780000000500000000","attributes":[{"key":"usage.input_tokens","value":{"intValue":"100"}},{"key":"gen_ai.input.messages","value":{"stringValue":"SECRET"}}]}]}]}]}"#;
        let records = parse_otlp_json(body);
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record["name"], "model_stream");
        assert_eq!(record["kind"], "INTERNAL");
        assert_eq!(record["spanId"], "bb");
        assert_eq!(record["startTime"], serde_json::json!(1_780_000_000_500i64));
        // Retain selected usage.* fields; discard bodies and oversized strings.
        assert_eq!(record["attributes"]["usage.input_tokens"], 100);
        assert!(record["attributes"].get("gen_ai.input.messages").is_none());
        assert_eq!(
            record["resource"]["attributes"]["service.name"],
            "codebuddy"
        );
    }

    #[test]
    fn protobuf_end_to_end() {
        fn field_bytes(field: u64, payload: &[u8]) -> Vec<u8> {
            let mut out = read_varint_test((field << 3) | 2);
            out.extend(read_varint_test(payload.len() as u64));
            out.extend_from_slice(payload);
            out
        }
        fn read_varint_test(mut v: u64) -> Vec<u8> {
            let mut out = Vec::new();
            loop {
                let mut b = (v & 0x7f) as u8;
                v >>= 7;
                if v != 0 {
                    b |= 0x80;
                }
                out.push(b);
                if v == 0 {
                    break;
                }
            }
            out
        }
        // Test span: ID 0xab, model_stream name, fixed64 start, usage.input_tokens=100.
        let kv = [field_bytes(1, b"usage.input_tokens"), {
            let mut any = read_varint_test(3 << 3); // AnyValue int_value is protobuf field 3.
            any.extend(read_varint_test(100));
            field_bytes(2, &any)
        }]
        .concat();
        let mut span = field_bytes(2, &[0xab]);
        span.extend(field_bytes(5, b"model_stream"));
        span.extend(read_varint_test(6 << 3));
        span.extend(read_varint_test(3));
        let mut time_tag = read_varint_test(7 << 3 | 1);
        time_tag.extend_from_slice(&1_780_000_000_500_000_000u64.to_le_bytes());
        span.extend(time_tag);
        span.extend(field_bytes(9, &kv));
        let scope_spans = field_bytes(2, &span);
        // Resource field 1 repeats KeyValue; its fields 1/2 hold key/AnyValue.
        let name_kv = [
            field_bytes(1, b"service.name"),
            // AnyValue stringValue uses field 1 with length-delimited wire type 2.
            field_bytes(2, &field_bytes(1, b"codebuddy")),
        ]
        .concat();
        let resource = field_bytes(1, &name_kv);
        let rs = [field_bytes(1, &resource), field_bytes(2, &scope_spans)].concat();
        let body = field_bytes(1, &rs);
        let records = parse_otlp_protobuf(&body);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], "model_stream");
        assert_eq!(records[0]["spanId"], "ab");
        assert_eq!(records[0]["kind"], "CLIENT");
        assert_eq!(records[0]["attributes"]["usage.input_tokens"], 100);
        assert_eq!(
            records[0]["resource"]["attributes"]["service.name"],
            "codebuddy"
        );
        assert_eq!(
            records[0]["startTime"],
            serde_json::json!(1_780_000_000_500i64)
        );
    }

    #[test]
    fn http_end_to_end_receiver_writes_scannable_spans() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-otel-e2e-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Start on a high port derived from the process ID.
        let port = 46000u16 + (std::process::id() % 1000) as u16;
        start(port, dir.clone()).expect("receiver start");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let payload = br#"{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"codebuddy"}}]},"scopeSpans":[{"spans":[{"spanId":"e2e01","name":"model_stream","startTimeUnixNano":"1780000000500000000","attributes":[{"key":"usage.input_tokens","value":{"intValue":"100"}},{"key":"usage.output_tokens","value":{"intValue":"20"}}]}]}]}]}"#;
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let crlf = "\u{0d}\u{0a}";
        let request = format!(
            "POST /v1/traces HTTP/1.1{crlf}Host: 127.0.0.1{crlf}Authorization: Bearer receiver-fixture{crlf}Content-Type: application/json{crlf}Content-Length: {len}{crlf}Connection: close{crlf}{crlf}",
            crlf = crlf,
            len = payload.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        stream.write_all(payload).unwrap();
        let mut response = String::new();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let _ = stream.read_to_string(&mut response);
        assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
        // Wait with bounded polling for the receiver thread to write the file.
        let spans_path = dir.join("spans.jsonl");
        for _ in 0..50 {
            if spans_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let text = std::fs::read_to_string(&spans_path).expect("spans.jsonl written");
        // Scan the written file with the otel adapter.
        use llm_usage_core::adapters::framework::{
            ScanLimits, ScanTarget, SourceAdapter, StoredScanState,
        };
        use llm_usage_core::adapters::jsonl::probe_file;
        let adapter = llm_usage_core::adapters::otel::OtelAdapter::new();
        let target = ScanTarget {
            instance_id: "otel@test".to_string(),
            path: spans_path.clone(),
            file_id: spans_path.to_string_lossy().to_string(),
            file_identity: "identity-e2e".to_string(),
            probe: probe_file(&spans_path).unwrap(),
            generation: 0,
            rescan: false,
        };
        let outcome = adapter
            .scan(
                &target,
                &StoredScanState::default(),
                &ScanLimits::default(),
                1_800_000_000_000,
            )
            .unwrap();
        assert_eq!(outcome.events.len(), 1, "text={text}");
        let event = &outcome.events[0];
        assert_eq!(event.agent, "codebuddy");
        assert_eq!(event.usage.input_total, Some(100));
        assert_eq!(event.usage.output_total, Some(20));
        assert_eq!(event.occurred_at_ms, 1_780_000_000_500);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gzip_and_bounds() {
        let raw = br#"{"resourceSpans":[]}"#;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(raw).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, raw);
    }

    #[test]
    fn exact_attribute_whitelist_rejects_sensitive_prefix_variants() {
        for key in [
            "gen_ai.system_instructions",
            "gen_ai.api_key",
            "gen_ai.request.prompt",
            "usage.secret",
            "response.body",
            "copilot_chat.password",
            "model_name_secret",
        ] {
            assert_eq!(
                allowed_attr(key, &serde_json::json!("SECRET")),
                None,
                "{key}"
            );
        }
        for key in ALLOWED_ATTR_KEYS {
            assert_eq!(
                allowed_attr(key, &serde_json::json!({"intValue":"10"})),
                Some(serde_json::json!(10)),
                "{key}"
            );
        }
    }

    #[test]
    fn receiver_admission_uses_a_monotonic_window_and_releases_slots() {
        let admission = Admission::new();
        let now = std::time::Instant::now();
        let permits = (0..4)
            .map(|_| admission.accept(now).unwrap())
            .collect::<Vec<_>>();
        assert!(admission.accept(now).is_none());
        drop(permits);
        for _ in 4..120 {
            drop(admission.accept(now).unwrap());
        }
        assert!(admission.accept(now).is_none());
        assert!(admission
            .accept(now + std::time::Duration::from_secs(59))
            .is_none());
        drop(
            admission
                .accept(now + std::time::Duration::from_secs(60))
                .unwrap(),
        );
        assert_eq!(admission.0.lock().unwrap().active, 0);
        assert_eq!(admission.0.lock().unwrap().used, 1);
    }

    #[test]
    fn actual_http_limits_rate_and_concurrent_connections_before_body_reading() {
        let admission = Admission::new();
        admission.0.lock().unwrap().used = 119;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-execution/receiver-admission")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let running = start_with_admission(
            0,
            root.clone(),
            std::sync::Arc::clone(&admission),
            test_authority(),
        )
        .unwrap();
        let request = || {
            let mut client = std::net::TcpStream::connect(("127.0.0.1", running.port)).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let payload = br#"{"resourceSpans":[]}"#;
            write!(client,"POST /v1/traces HTTP/1.1{CRLF}Authorization: Bearer receiver-fixture{CRLF}Content-Type: application/json{CRLF}Content-Length: {}{CRLF}{CRLF}",payload.len()).unwrap();
            client.write_all(payload).unwrap();
            // HTTP clients use Content-Length instead of waiting for EOF
            // when unread rejected input can cause a socket reset.
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                client.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
                assert!(head.len() < MAX_HEAD_BYTES);
            }
            let mut response = String::from_utf8(head).unwrap();
            let length = response
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .unwrap()
                .trim()
                .parse::<usize>()
                .unwrap();
            let mut body = vec![0; length];
            client.read_exact(&mut body).unwrap();
            response.push_str(&String::from_utf8(body).unwrap());
            response
        };
        assert!(request().starts_with("HTTP/1.1 200"));
        let rejected = request();
        assert!(rejected.starts_with("HTTP/1.1 429 Too Many Requests"));
        assert!(rejected.contains("Retry-After: 60"));
        for _ in 0..100 {
            if admission.0.lock().unwrap().active == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        admission.0.lock().unwrap().used = 0;
        let clients = (0..4)
            .map(|_| std::net::TcpStream::connect(("127.0.0.1", running.port)).unwrap())
            .collect::<Vec<_>>();
        for _ in 0..100 {
            if admission.0.lock().unwrap().active == 4 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(admission.0.lock().unwrap().active, 4);
        assert!(request().starts_with("HTTP/1.1 429"));
        drop(clients);
        running
            .stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        running.thread.join().unwrap();
        assert!(
            !root.exists(),
            "rejected/empty requests must not persist telemetry"
        );
    }

    #[test]
    fn actual_http_rejects_compression_bomb_bad_gzip_and_oversized_body() {
        fn request(body: &[u8], gzip: bool, declared: usize) -> String {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let address = listener.local_addr().unwrap();
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../build/plan-execution/receiver-bounds")
                .join(format!("{}", std::process::id()));
            let worker = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                handle(stream, &root, &test_authority()).unwrap();
                assert!(!root.join("spans.jsonl").exists());
            });
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let encoding = if gzip {
                "Content-Encoding: gzip\r\n"
            } else {
                ""
            };
            write!(stream,"POST /v1/traces HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer receiver-fixture{CRLF}Content-Type: application/json\r\n{encoding}Content-Length: {declared}\r\n\r\n").unwrap();
            stream.write_all(body).unwrap();
            let mut result = String::new();
            stream.read_to_string(&mut result).unwrap();
            worker.join().unwrap();
            result
        }
        assert!(request(b"broken", true, 6).starts_with("HTTP/1.1 400"));
        assert!(request(&[], false, MAX_BODY_BYTES + 1).starts_with("HTTP/1.1 413"));
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let block = [0u8; 8192];
        for _ in 0..MAX_GZIP_BYTES / block.len() {
            encoder.write_all(&block).unwrap();
        }
        encoder.write_all(&[0]).unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(request(&compressed, true, compressed.len()).starts_with("HTTP/1.1 413"));
    }

    #[test]
    fn actual_http_authenticates_before_body_and_rechecks_revocation() {
        use crate::receiver_auth::{
            self,
            tests::{root, MemoryStore},
            Family,
        };
        let app = root("http-auth");
        let out = app.join("otel");
        let store = std::sync::Arc::new(MemoryStore::default());
        let binding = receiver_auth::issue(
            store.as_ref(),
            &app,
            &app.join("claude/settings.json"),
            Family::Claude,
        )
        .unwrap();
        let credential_app = app.clone();
        let credential_store = std::sync::Arc::clone(&store);
        let authority: Authority = std::sync::Arc::new(move |header, path| {
            receiver_auth::authorize(credential_store.as_ref(), &credential_app, header, path)
        });
        let running = start_with_admission(0, out.clone(), Admission::new(), authority).unwrap();
        let request = |path: &str, headers: &str, body: &[u8], declared: usize| {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", running.port)).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            write!(stream,"POST {path} HTTP/1.1{CRLF}{headers}Content-Type: application/json{CRLF}Content-Length: {declared}{CRLF}{CRLF}").unwrap();
            stream.write_all(body).unwrap();
            let mut response = Vec::new();
            while !response.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                response.push(byte[0]);
            }
            String::from_utf8(response).unwrap()
        };
        // A huge declared body sends no bytes; authentication must still respond immediately.
        assert!(request("/v1/logs", "", b"", MAX_BODY_BYTES + 1).starts_with("HTTP/1.1 401"));
        let header = format!("Authorization: {}{CRLF}", binding.header());
        assert!(request("/v1/traces", &header, b"", MAX_BODY_BYTES + 1).starts_with("HTTP/1.1 401"));
        assert!(
            request("/v1/logs", &format!("{header}{header}"), b"", 0).starts_with("HTTP/1.1 401")
        );
        for extra in [
            "Origin: http://localhost",
            "Forwarded: for=127.0.0.1",
            "X-Forwarded-For: 127.0.0.1",
        ] {
            assert!(
                request("/v1/logs", &format!("{header}{extra}{CRLF}"), b"", 0)
                    .starts_with("HTTP/1.1 403")
            );
        }
        assert!(!app.join("telemetry/otlp-logs.jsonl").exists());
        let payload=br#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{"eventName":"claude_code.api_request","timeUnixNano":"1780000000000000000","attributes":[{"key":"input_tokens","value":{"intValue":"42"}},{"key":"authorization","value":{"stringValue":"SENSITIVE"}}]},{"eventName":"codex.api_request"}]}]}]}"#;
        assert!(request("/v1/logs", &header, payload, payload.len()).starts_with("HTTP/1.1 200"));
        let saved = std::fs::read_to_string(app.join("telemetry/otlp-logs.jsonl")).unwrap();
        assert_eq!(saved.lines().count(), 1);
        assert!(
            saved.contains("42")
                && !saved.contains("SENSITIVE")
                && !saved.contains(&binding.header())
                && !saved.contains("codex.api_request")
        );
        receiver_auth::revoke(store.as_ref(), &binding).unwrap();
        assert!(request("/v1/logs", &header, b"", 0).starts_with("HTTP/1.1 401"));
        assert_eq!(
            std::fs::read_to_string(app.join("telemetry/otlp-logs.jsonl")).unwrap(),
            saved
        );
        running
            .stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        running.thread.join().unwrap();
        assert!(store.values.lock().unwrap().is_empty());
        std::fs::remove_dir_all(app).unwrap();
    }

    #[test]
    fn json_status_passthrough() {
        // Preserve status.code=ERROR for adapter error_status.
        let body = br#"{"resourceSpans":[{"scopeSpans":[{"spans":[{"spanId":"s1","name":"chat","status":{"code":"STATUS_CODE_ERROR"}}]}]}]}"#;
        let records = parse_otlp_json(body);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["status"]["code"], "STATUS_CODE_ERROR");
        // Spans without status omit that key.
        let body =
            br#"{"resourceSpans":[{"scopeSpans":[{"spans":[{"spanId":"s2","name":"chat"}]}]}]}"#;
        let records = parse_otlp_json(body);
        assert!(records[0].get("status").is_none());
    }

    #[test]
    fn protobuf_status_passthrough() {
        fn field_bytes(field: u64, payload: &[u8]) -> Vec<u8> {
            let mut out = varint_test((field << 3) | 2);
            out.extend(varint_test(payload.len() as u64));
            out.extend_from_slice(payload);
            out
        }
        fn varint_test(mut v: u64) -> Vec<u8> {
            let mut out = Vec::new();
            loop {
                let mut b = (v & 0x7f) as u8;
                v >>= 7;
                if v != 0 {
                    b |= 0x80;
                }
                out.push(b);
                if v == 0 {
                    break;
                }
            }
            out
        }
        // Status field 2 varint ERROR=2 is nested in Span field 15.
        let status = {
            let mut s = varint_test(2 << 3);
            s.extend(varint_test(2));
            field_bytes(15, &s)
        };
        let mut span = field_bytes(2, &[0xcd]);
        span.extend(field_bytes(5, b"model_stream"));
        span.extend(status);
        let scope_spans = field_bytes(2, &span);
        let rs = field_bytes(2, &scope_spans);
        let body = field_bytes(1, &rs);
        let records = parse_otlp_protobuf(&body);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["status"]["code"], 2);
    }

    #[test]
    fn varint_tenth_byte_guard() {
        // Reject varint byte ten above 1 instead of truncating high bits.
        let bad = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02];
        assert_eq!(read_varint(&bad), None);
    }
}
