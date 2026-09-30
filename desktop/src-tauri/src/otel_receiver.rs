//! 本地 OTLP/HTTP 接收器（M5，按需启用）：仅绑定 127.0.0.1，接收
//! OTLP/JSON 与 OTLP/protobuf 的 /v1/traces（CodeBuddy 仅支持 protobuf；
//! Copilot CLI/VS Code 默认 JSON），gzip 有界解压，把逐请求 span 归一化为
//! otel 适配器可读的 JSONL（%APPDATA%/llm-usage-desktop/otel/spans.jsonl）。
//!
//! 合同（execution.md M5 / V22 / V25）：
//! - 默认关闭（settings.otel_receiver_enabled）；启用 = 用户显式授权本机实例；
//! - 仅 127.0.0.1（loopback 转发按 V25 判定：经本接收器到达的数据 = 本机实例，
//!   与直接读取外部 exporter 文件分列来源）；
//! - 请求头 ≤ 64 KiB、body ≤ 64 MiB、gzip 解压上限 64 MiB（压缩炸弹防护）；
//! - 字段白名单：只保留 span 名/ID/kind/时间与 gen_ai.*、usage.*、model*、
//!   copilot_chat.*、response.*、service.name 属性；正文/凭据不落盘；
//! - 无认证头校验（本地显式启用场景；OTLP_HEADERS 由上游 Agent 使用）。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

/// body 上限。
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
/// 头区上限。
const MAX_HEAD_BYTES: usize = 64 * 1024;
/// gzip 解压上限。
const MAX_GZIP_BYTES: usize = 64 * 1024 * 1024;
/// 属性白名单前缀/全名。
const ALLOWED_ATTR_PREFIXES: &[&str] = &[
    "gen_ai.",
    "usage.",
    "model_name",
    "request.model",
    "response.",
    "copilot_chat.",
    "github.copilot.turn_id",
    "server.address",
];
const ALLOWED_RESOURCE_KEYS: &[&str] = &["service.name"];
/// 正文/内容属性：即使命中前缀也拒绝落盘（V22 白名单合同）。
const DENIED_ATTR_KEYS: &[&str] = &[
    "gen_ai.input.messages",
    "gen_ai.output.messages",
    "input.value",
    "output.value",
    "tool.parameters",
];
/// 标量字符串上限（超出按非白名单丢弃：防超长正文变体）。
const MAX_ATTR_STRING_BYTES: usize = 256;

/// 启动接收器线程；绑定失败返回 Err（调用方展示）。线程随进程退出结束。
pub fn start(port: u16, out_dir: PathBuf) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    {
        std::thread::Builder::new()
            .name("otel-receiver".to_string())
            .spawn(move || {
                let listener = listener;
                listener.set_nonblocking(true).ok();
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let out = out_dir.clone();
                            std::thread::spawn(move || {
                                let _ = handle(stream, &out);
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
    }
    Ok(())
}

const CRLF: &str = "\u{0d}\u{0a}";

/// 处理一条 HTTP 连接（单请求即关；OTLP exporter 均为短连接）。
fn handle(mut stream: std::net::TcpStream, out_dir: &std::path::Path) -> std::io::Result<()> {
    // 接受自非阻塞 listener 的连接可能继承非阻塞属性：显式恢复阻塞读写。
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    // 读到头部结束或超限。
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
    let mut lines = head.split(CRLF);
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let mut content_length = 0usize;
    let mut content_type = String::new();
    let mut gzip = false;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        if k == "content-length" {
            content_length = v.parse().unwrap_or(0);
        } else if k == "content-type" {
            content_type = v.to_ascii_lowercase();
        } else if k == "content-encoding" && v.eq_ignore_ascii_case("gzip") {
            gzip = true;
        }
    }
    if method != "POST" || path != "/v1/traces" {
        respond(&mut stream, 404, "only POST /v1/traces")?;
        return Ok(());
    }
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
    if gzip {
        let decoder = flate2::read::GzDecoder::new(&body[..]);
        let mut out = Vec::new();
        decoder
            .take(MAX_GZIP_BYTES as u64 + 1)
            .read_to_end(&mut out)?;
        if out.len() > MAX_GZIP_BYTES {
            respond(&mut stream, 413, "decompressed body too large")?;
            return Ok(());
        }
        body = out;
    }
    let records = if content_type.contains("json") {
        parse_otlp_json(&body)
    } else {
        // 默认按 protobuf（CodeBuddy 唯一形态）。
        parse_otlp_protobuf(&body)
    };
    let count = records.len();
    if count > 0 {
        std::fs::create_dir_all(out_dir)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_dir.join("spans.jsonl"))?;
        for record in records {
            file.write_all(record.to_string().as_bytes())?;
            file.write_all(b"\n")?;
        }
    }
    let r = respond(&mut stream, 200, "");
    r?;
    let _ = count;
    Ok(())
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == [0x0d, 0x0a, 0x0d, 0x0a])
}

fn respond(stream: &mut std::net::TcpStream, status: u16, msg: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let body = if status == 200 { "{}" } else { msg };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}
Content-Type: application/json
Content-Length: {}
Connection: close

{body}",
        body.len()
    )
}

/// 白名单过滤后的扁平属性值。
fn allowed_attr(key: &str, value: &serde_json::Value) -> Option<serde_json::Value> {
    if DENIED_ATTR_KEYS.contains(&key) {
        return None;
    }
    let keep = ALLOWED_ATTR_PREFIXES.iter().any(|p| key.starts_with(p))
        || ALLOWED_RESOURCE_KEYS.contains(&key);
    if !keep {
        return None;
    }
    // OTLP AnyValue 形 {stringValue/intValue/doubleValue/boolValue} → 标量。
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
    Some(value.clone())
}

/// OTLP/JSON ExportTraceServiceRequest → 归一化 span 记录。
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

/// 单 span（OTLP JSON 形）→ 归一化记录（startTime 毫秒整数 + 扁平属性）。
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
    // span status 透传（span 级元数据，非属性，不在白名单范围）：
    // ERROR 状态供 otel 适配器标 error_status，失败调用不能当成功入账。
    if let Some(code) = span.pointer("/status/code") {
        record.insert("status".into(), serde_json::json!({"code": code.clone()}));
    }
    record.insert("kind".into(), span.get("kind").cloned().unwrap_or_default());
    record.insert("attributes".into(), serde_json::Value::Object(attrs));
    record.insert(
        "resource".into(),
        serde_json::json!({"attributes": resource_attrs}),
    );
    serde_json::Value::Object(record)
}

// ---- protobuf wire 解析（ExportTraceServiceRequest）----

fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    for (i, byte) in data.iter().enumerate().take(10) {
        // 第 10 字节只允许 0/1（protobuf u64 编码规则）；更大值是损坏数据。
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

/// protobuf KeyValue → (key, 标量值)。
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
                        (2, Wire::Varint(v)) => {
                            value = Some(serde_json::Value::from(v));
                        }
                        (3, Wire::Fixed64(bits)) => {
                            value = Some(serde_json::Value::from(f64::from_bits(bits)));
                        }
                        (4, Wire::Varint(v)) => {
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

/// OTLP/protobuf body → 归一化 span 记录。
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
    let mut attrs = serde_json::Map::new();
    iter_fields(span, |field, wire| {
        match (field, wire) {
            (2, Wire::Bytes(id)) => {
                span_id = Some(id.iter().map(|b| format!("{b:02x}")).collect());
            }
            (5, Wire::Bytes(n)) => name = String::from_utf8_lossy(n).to_string(),
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
            // Status 子消息：#2 code（枚举；2=ERROR）。
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

    #[test]
    fn json_end_to_end() {
        let body = br#"{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"codebuddy"}}]},"scopeSpans":[{"spans":[{"traceId":"aa","spanId":"bb","name":"model_stream","kind":1,"startTimeUnixNano":"1780000000500000000","attributes":[{"key":"usage.input_tokens","value":{"intValue":"100"}},{"key":"gen_ai.input.messages","value":{"stringValue":"SECRET"}}]}]}]}]}"#;
        let records = parse_otlp_json(body);
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record["name"], "model_stream");
        assert_eq!(record["spanId"], "bb");
        assert_eq!(record["startTime"], serde_json::json!(1_780_000_000_500i64));
        // 白名单：usage.* 保留、正文与超长字符串丢弃。
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
        // span{spanId=0xab, name="model_stream", start=fixed64, attrs=[usage.input_tokens=100]}
        let kv = [field_bytes(1, b"usage.input_tokens"), {
            let mut any = read_varint_test(2 << 3); // field2 varint
            any.extend(read_varint_test(100));
            field_bytes(2, &any)
        }]
        .concat();
        let mut span = field_bytes(2, &[0xab]);
        span.extend(field_bytes(5, b"model_stream"));
        let mut time_tag = read_varint_test(7 << 3 | 1);
        time_tag.extend_from_slice(&1_780_000_000_500_000_000u64.to_le_bytes());
        span.extend(time_tag);
        span.extend(field_bytes(9, &kv));
        let scope_spans = field_bytes(2, &span);
        // Resource = field1(repeated KeyValue)；KeyValue = field1(key)+field2(AnyValue)。
        let name_kv = [
            field_bytes(1, b"service.name"),
            // AnyValue{field1 stringValue}：wire2 长度分隔。
            field_bytes(2, &field_bytes(1, b"codebuddy")),
        ]
        .concat();
        let resource = field_bytes(1, &name_kv);
        let rs = [resource, field_bytes(2, &scope_spans)].concat();
        let body = field_bytes(1, &rs);
        let records = parse_otlp_protobuf(&body);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], "model_stream");
        assert_eq!(records[0]["spanId"], "ab");
        assert_eq!(records[0]["attributes"]["usage.input_tokens"], 100);
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
        // 随机高位端口起接收器。
        let port = 46000u16 + (std::process::id() % 1000) as u16;
        start(port, dir.clone()).expect("receiver start");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let payload = br#"{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"codebuddy"}}]},"scopeSpans":[{"spans":[{"spanId":"e2e01","name":"model_stream","startTimeUnixNano":"1780000000500000000","attributes":[{"key":"usage.input_tokens","value":{"intValue":"100"}},{"key":"usage.output_tokens","value":{"intValue":"20"}}]}]}]}]}"#;
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let crlf = "\u{0d}\u{0a}";
        let request = format!(
            "POST /v1/traces HTTP/1.1{crlf}Host: 127.0.0.1{crlf}Content-Type: application/json{crlf}Content-Length: {len}{crlf}Connection: close{crlf}{crlf}",
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
        // 接收器线程异步写盘：轮询等待。
        let spans_path = dir.join("spans.jsonl");
        for _ in 0..50 {
            if spans_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let text = std::fs::read_to_string(&spans_path).expect("spans.jsonl written");
        // otel 适配器扫描该文件。
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
    fn json_status_passthrough() {
        // span status.code 透传（ERROR 供适配器标 error_status）。
        let body = br#"{"resourceSpans":[{"scopeSpans":[{"spans":[{"spanId":"s1","name":"chat","status":{"code":"STATUS_CODE_ERROR"}}]}]}]}"#;
        let records = parse_otlp_json(body);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["status"]["code"], "STATUS_CODE_ERROR");
        // 无 status 的 span 不带该键。
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
        // Status{#2 code varint 2（ERROR）} → Span #15。
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
        // 第 10 字节 > 1：拒绝（不静默截断高位）。
        let bad = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02];
        assert_eq!(read_varint(&bad), None);
    }
}
