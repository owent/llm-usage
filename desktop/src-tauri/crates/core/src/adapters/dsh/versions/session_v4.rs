//! Actual npm rc.2 persistence/LLM/token-meter codecs; see m3-runtime-samples.md.
//! Only a complete bounded snapshot commits. Native settlement copies are folded once.
use crate::{
    adapters::{framework::*, usage_map::finish},
    domain::*,
    error::CoreError,
    ingest::DiagnosticInput,
};
use serde_json::Value;
use std::{collections::BTreeMap, io::Read, path::Path, time::Instant};

pub const FORMAT: &str = "dsh-session-v4-jsonl";
pub const PARSER_VERSION: &str = "dsh-session-v4-1";
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
const MAX_LINES: usize = 50_000;
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
// Exact vocabulary from the installed @deepseek-ai/dsh-session generated catalog.
const TYPES: &[&str] = &[
    "agent-preset/selected",
    "agent/inbox/spliced",
    "approval/asked",
    "approval/decided",
    "approval/policy",
    "assistant/attempt",
    "assistant/message",
    "command/done",
    "command/run",
    "compaction/end",
    "compaction/prune",
    "compaction/start",
    "compaction/summary",
    "deliverables/presented",
    "developer/message",
    "feedback/message-delete",
    "feedback/message-put",
    "feedback/record",
    "goal/change",
    "hook/invoked",
    "hook/result",
    "image/offload",
    "llm/retry",
    "llm/retry-started",
    "model/selection",
    "permission/preset",
    "plan/mode",
    "request/context",
    "request/header",
    "sandbox/mode",
    "schedule/change",
    "session-log-deepseek/delivery-accepted",
    "session/end-seed",
    "session/title",
    "session/title-llm-request",
    "step/end",
    "step/start",
    "subagent/catalog",
    "subagent/descriptor",
    "subagent/model-selection-policy",
    "system/message",
    "team/member",
    "team/message/delivered",
    "team/message/queued",
    "team/task",
    "todo/write",
    "tool-workflow/agent-end",
    "tool-workflow/agent-start",
    "tool-workflow/run-end",
    "tool-workflow/run-start",
    "tool/call",
    "tool/ptc-dispatch",
    "tool/ptc-dispatch-start",
    "tool/result",
    "turn/end",
    "turn/start",
    "user/message",
    "web/deepseek-search-llm-request",
    "workspace/changes",
];

pub fn generation(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    let name = name.strip_suffix(".zstd").unwrap_or(name);
    if name == "session.jsonl" {
        return Some(0);
    }
    let number = name.strip_prefix("session.v")?.strip_suffix(".jsonl")?;
    if number.starts_with('0') || number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    number
        .parse::<u64>()
        .ok()
        .filter(|v| *v <= MAX_SAFE_INTEGER as u64)
}
pub fn is_native_path(path: &Path) -> bool {
    generation(path).is_some() || path.extension().is_some_and(|x| x == "zstd")
}
fn number(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .filter(|n| (0..=MAX_SAFE_INTEGER).contains(n))
}
fn text(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty() && s.len() <= 1024)
}
fn absolute_text(s: &str) -> bool {
    s.starts_with('/')
        || s.starts_with("\\\\")
        || (s.as_bytes().get(1) == Some(&b':')
            && s.as_bytes()
                .get(2)
                .is_some_and(|b| *b == b'/' || *b == b'\\')
            && s.as_bytes()[0].is_ascii_alphabetic())
}
pub(crate) fn header(doc: &Value) -> bool {
    let Some(obj) = doc.as_object() else {
        return false;
    };
    obj.keys().all(|k| {
        [
            "type",
            "version",
            "id",
            "createdAt",
            "isSeeded",
            "delegationDepth",
            "cwd",
            "parentSession",
            "origin",
            "agentPreset",
        ]
        .contains(&k.as_str())
    }) && doc["type"] == "session"
        && doc["version"] == 4
        && text(&doc["id"]).is_some()
        && number(&doc["createdAt"]).is_some()
        && number(&doc["delegationDepth"]).is_some()
        && doc["isSeeded"].is_boolean()
        && obj
            .get("cwd")
            .map_or(true, |v| v.as_str().is_some_and(absolute_text))
        && ["parentSession", "agentPreset"]
            .iter()
            .all(|k| obj.get(*k).map_or(true, |v| v.is_string()))
        && obj.get("origin").map_or(true, |v| v == "subagent")
}
fn diag(code: &str, line: usize) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: None,
        position: Some(format!("line {line}")),
        message: "DSH v4 validated framing/usage contract; no content retained".into(),
    }
}
fn read_bytes(
    path: &Path,
    started: Instant,
    limits: &ScanLimits,
) -> Result<Result<Vec<u8>, &'static str>, CoreError> {
    let file = crate::adapters::run_policy::checked_file(path)?;
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Ok(Err("file_exceeds_size_cap"));
    }
    let mut reader: Box<dyn Read> = if path.extension().is_some_and(|x| x == "zstd") {
        let mut decoder = match zstd::stream::read::Decoder::new(file.take(MAX_FILE_BYTES + 1)) {
            Ok(d) => d,
            Err(_) => return Ok(Err("dsh_zstd_invalid")),
        };
        if decoder.window_log_max(26).is_err() {
            return Ok(Err("dsh_zstd_window_invalid"));
        }
        Box::new(decoder)
    } else {
        Box::new(file.take(MAX_FILE_BYTES + 1))
    };
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        crate::adapters::run_policy::check()?;
        if limits
            .jsonl
            .time_budget
            .is_some_and(|b| started.elapsed() >= b)
        {
            return Ok(Err("dsh_snapshot_budget"));
        }
        let n = match reader.read(&mut chunk) {
            Ok(n) => n,
            Err(_) => {
                crate::adapters::run_policy::check()?;
                return Ok(Err("dsh_snapshot_unreadable"));
            }
        };
        if n == 0 {
            break;
        }
        if bytes.len() + n > MAX_FILE_BYTES as usize {
            return Ok(Err("decoded_exceeds_size_cap"));
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    Ok(Ok(bytes))
}
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    if let Some(found) = generation(path).filter(|v| *v != 4) {
        return Ok(DetectOutcome::UnsupportedVersion{format:FORMAT.into(),found:Some(found.to_string()),reason:"only the actual rc.2 v4 generation is validated; older migrations and newer formats require evidence".into()});
    }
    let bytes = match read_bytes(path, Instant::now(), &ScanLimits::default())? {
        Ok(bytes) => bytes,
        Err(code) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: code.into(),
            })
        }
    };
    let Some(end) = bytes.iter().position(|b| *b == b'\n') else {
        return Ok(DetectOutcome::Pending);
    };
    if end > MAX_LINE_BYTES {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "DSH header exceeds line cap".into(),
        });
    }
    let Ok(doc) = crate::adapters::run_policy::json_from_slice::<Value>(&bytes[..end]) else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "DSH header is not JSON".into(),
        });
    };
    if doc["type"] == "session" && doc["version"].as_u64().is_some_and(|v| v != 4) {
        return Ok(DetectOutcome::UnsupportedVersion {
            format: FORMAT.into(),
            found: doc["version"].as_u64().map(|v| v.to_string()),
            reason: "unvalidated DSH generation".into(),
        });
    }
    if !header(&doc) {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "DSH v4 header failed its exact contract".into(),
        });
    }
    if bytes.len() == end + 1 {
        return Ok(DetectOutcome::Pending);
    }
    Ok(DetectOutcome::Supported {
        format: FORMAT.into(),
        format_version: Some("4".into()),
        basis: VersionBasis::KnownVersion,
    })
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Context {
    #[serde(default)]
    revision_floor: i64,
    #[serde(default)]
    tracked: BTreeMap<String, Tracked>,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    created_at: Option<i64>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Tracked {
    digest: String,
    revision: i64,
    observed_at: i64,
}
fn failed(mut out: ScanOutcome, code: &str, line: usize) -> ScanOutcome {
    out.events.clear();
    out.cursor = None;
    out.parse_context = None;
    out.status = if code.contains("cap") {
        ScanStatus::LineTooLong
    } else if code == "dsh_snapshot_budget" {
        ScanStatus::BudgetExhausted
    } else {
        ScanStatus::Pending
    };
    out.health = "degraded".into();
    out.diagnostics.push(diag(code, line));
    out
}
fn envelope(row: &Value, seq: usize) -> bool {
    let Some(obj) = row.as_object() else {
        return false;
    };
    obj.keys().all(|k| {
        [
            "type",
            "seq",
            "time",
            "data",
            "ignorable",
            "sourceEventSeqs",
            "surfaceOp",
        ]
        .contains(&k.as_str())
    }) && row["seq"].as_u64() == Some(seq as u64)
        && row["time"]
            .as_i64()
            .is_some_and(|v| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&v))
        && row["type"].is_string()
        && obj.contains_key("data")
        && obj.get("ignorable").map_or(true, |v| v == true)
        && obj
            .get("sourceEventSeqs")
            .map_or(true, |v| source_ranges(v, seq))
        && obj.get("surfaceOp").map_or(true, |v| surface_op(v, seq))
}
fn source_ranges(value: &Value, seq: usize) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    let mut count = 0u64;
    let mut previous = None;
    let mut increasing = true;
    let mut has_range = false;
    for item in items {
        let (start, end) = if let Some(v) = number(item) {
            (v, v)
        } else {
            let Some(pair) = item.as_array().filter(|a| a.len() == 2) else {
                return false;
            };
            let Some(values) = number(&pair[0]).zip(number(&pair[1])) else {
                return false;
            };
            has_range = true;
            values
        };
        if end < start {
            return false;
        }
        count += (end - start + 1) as u64;
        if count > seq as u64 {
            return false;
        }
        increasing &= previous.map_or(true, |v| start > v);
        previous = Some(end);
    }
    !has_range || increasing
}
fn surface_op(value: &Value, seq: usize) -> bool {
    if value == "append" {
        return true;
    }
    let Some(obj) = value.as_object() else {
        return false;
    };
    obj.len() == 3
        && value["op"] == "replace"
        && number(&value["startSeq"])
            .zip(number(&value["endSeq"]))
            .is_some_and(|(a, b)| a <= b && b < seq as i64)
}
fn sample(row: &Value) -> Option<&Value> {
    if row["type"] == "assistant/message" {
        if let Some(usage) = row["data"].get("usage") {
            return Some(usage);
        }
    }
    row["data"]["stream"]
        .as_array()?
        .iter()
        .rev()
        .find(|s| s["type"] == "chunk" && s["chunk"]["type"] == "usage")?
        .get("chunk")?
        .get("usage")
}
fn map_usage(
    value: Option<&Value>,
    verified_api: bool,
) -> Option<crate::adapters::usage_map::MappedUsage> {
    let value = value?;
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<Option<i64>> {
        match obj.get(key) {
            None => Some(None),
            Some(v) => v
                .as_i64()
                .filter(|n| (0..=MAX_TOKEN_VALUE).contains(n))
                .map(Some),
        }
    };
    let input = get("inputTokens")?;
    let output = get("outputTokens")?;
    let read = get("cacheReadTokens")?;
    let write = get("cacheWriteTokens")?;
    let native_total = get("totalTokens")?;
    let positive = |v: Option<i64>| v.filter(|v| *v > 0);
    let input = positive(input);
    let output = positive(output);
    let total_input = if verified_api {
        input
            .and_then(|i| i.checked_add(read.unwrap_or(0)))
            .and_then(|i| i.checked_add(write.unwrap_or(0)))
    } else {
        None
    };
    let total = total_input.and_then(|i| output.and_then(|o| i.checked_add(o)));
    let q = |v: Option<i64>| {
        if v.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        }
    };
    let d = |v: Option<i64>| {
        if v.is_some() {
            FieldQuality::Derived
        } else {
            FieldQuality::Unknown
        }
    };
    let mut issues = vec![];
    if let (Some(a), Some(b)) = (total, native_total) {
        if a != b {
            issues.push(crate::metrics::Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: "DSH codec self-calculated total differs from reconstructed buckets".into(),
            });
        }
    }
    Some(finish(
        TokenUsage {
            input_uncached: input,
            input_cache_read: positive(read),
            input_cache_write: positive(write),
            input_total: total_input,
            output_total: output,
            output_reasoning: None,
            total_tokens: total,
            source_total: None,
        },
        TokenQuality {
            input_uncached: q(input),
            input_cache_read: q(positive(read)),
            input_cache_write: q(positive(write)),
            input_total: d(total_input),
            output_total: q(output),
            output_reasoning: FieldQuality::Unknown,
            total_tokens: d(total),
            source_total: FieldQuality::Unknown,
        },
        issues,
    ))
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let started = Instant::now();
    let mut out = ScanOutcome {
        status: ScanStatus::Pending,
        cursor: None,
        parse_context: None,
        events: vec![],
        aggregates: vec![],
        diagnostics: vec![],
        lines_read: 0,
        records_seen: 0,
        reconciliations: vec![],
        health: "active".into(),
    };
    let bytes = match read_bytes(&target.path, started, limits)? {
        Ok(b) => b,
        Err(c) => return Ok(failed(out, c, 1)),
    };
    if crate::adapters::jsonl::probe_file(&target.path)? != target.probe {
        return Ok(failed(out, "dsh_source_changed_during_read", 1));
    }
    if bytes.is_empty() {
        return Ok(out);
    }
    if bytes.last() != Some(&b'\n') {
        return Ok(failed(out, "dsh_snapshot_incomplete", 1));
    }
    let mut rows = vec![];
    for (index, line) in bytes[..bytes.len() - 1].split(|b| *b == b'\n').enumerate() {
        crate::adapters::run_policy::check()?;
        if limits
            .jsonl
            .time_budget
            .is_some_and(|b| started.elapsed() >= b)
            || index
                >= limits
                    .jsonl
                    .max_lines
                    .unwrap_or(MAX_LINES as u64)
                    .min(MAX_LINES as u64) as usize
        {
            return Ok(failed(out, "dsh_snapshot_budget", index + 1));
        }
        if line.len() > limits.jsonl.max_line_bytes.min(MAX_LINE_BYTES) {
            return Ok(failed(out, "line_exceeds_size_cap", index + 1));
        }
        let Ok(row) = crate::adapters::run_policy::json_from_slice::<Value>(line) else {
            return Ok(failed(out, "dsh_row_invalid", index + 1));
        };
        rows.push(row);
    }
    out.lines_read = rows.len() as u64;
    let head = &rows[0];
    if !header(head) || generation(&target.path).is_some_and(|v| v != 4) {
        return Ok(failed(out, "dsh_header_invalid", 1));
    }
    let mut inherited = None;
    for (seq, row) in rows.iter().skip(1).enumerate() {
        if !envelope(row, seq) {
            return Ok(failed(out, "dsh_envelope_invalid", seq + 2));
        }
        let kind = row["type"].as_str().unwrap();
        if !TYPES.contains(&kind) {
            if row["ignorable"] == true {
                continue;
            }
            return Ok(failed(out, "dsh_required_event_unknown", seq + 2));
        }
        if !row["data"].is_object() {
            return Ok(failed(out, "dsh_event_data_invalid", seq + 2));
        }
        if kind == "session/end-seed" && row["data"]["inherited"] == true {
            inherited = Some(seq);
        }
    }
    if head["isSeeded"] == true && inherited.is_none()
        || head["isSeeded"] == false && inherited.is_some()
    {
        return Ok(failed(out, "dsh_inheritance_invalid", 1));
    }
    let session = head["id"].as_str().unwrap();
    let created_at = head["createdAt"].as_i64().unwrap();
    let mut context: Context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if context.session.as_deref().is_some_and(|id| id != session)
        || context.created_at.is_some_and(|t| t != created_at)
    {
        return Ok(failed(out, "dsh_session_identity_changed", 1));
    }
    let mut folded: BTreeMap<String, &Value> = BTreeMap::new();
    let mut turn = None;
    let mut step = None;
    let mut next_turn = 1i64;
    let mut next_step = 1i64;
    let mut attempt = 0i64;
    for (seq, row) in rows.iter().skip(1).enumerate() {
        crate::adapters::run_policy::check()?;
        if limits
            .jsonl
            .time_budget
            .is_some_and(|b| started.elapsed() >= b)
        {
            return Ok(failed(out, "dsh_snapshot_budget", seq + 2));
        }
        let kind = row["type"].as_str().unwrap();
        let data = &row["data"];
        // Inherited rows establish lifecycle coordinates, but never local usage.
        let coords = || number(&data["turn"]).zip(number(&data["step"]));
        let valid = match kind {
            "turn/start" => {
                let v = number(&data["turn"]);
                if turn.is_none() && v == Some(next_turn) {
                    turn = v;
                    next_step = 1;
                    true
                } else {
                    false
                }
            }
            "step/start" => {
                if coords() == turn.zip(Some(next_step)) && turn.is_some() && step.is_none() {
                    step = Some(next_step);
                    attempt = seq as i64;
                    true
                } else {
                    false
                }
            }
            "step/end" => {
                if coords() == turn.zip(step) && step.is_some() {
                    step = None;
                    next_step += 1;
                    true
                } else {
                    false
                }
            }
            "turn/end" => {
                if number(&data["turn"]) == turn && turn.is_some() && step.is_none() {
                    turn = None;
                    next_turn += 1;
                    true
                } else {
                    false
                }
            }
            "llm/retry-started" => {
                if coords() == turn.zip(step) && step.is_some() {
                    attempt = seq as i64;
                    true
                } else {
                    false
                }
            }
            "assistant/message" | "assistant/attempt" => {
                coords() == turn.zip(step) && step.is_some()
            }
            _ => true,
        };
        if !valid {
            return Ok(failed(out, "dsh_attempt_coordinates_invalid", seq + 2));
        }
        if inherited.is_some_and(|cut| seq <= cut) {
            continue;
        }
        if kind == "session/title-llm-request"
            || kind == "web/deepseek-search-llm-request"
            || kind == "compaction/summary"
        {
            out.diagnostics
                .push(diag("auxiliary_usage_coverage_unverified", seq + 2));
        }
        if kind != "assistant/message" && kind != "assistant/attempt" {
            continue;
        }
        if kind == "assistant/message"
            && (data["message"]["role"] != "assistant"
                || data["message"]["source"]["kind"] != "model"
                || text(&data["message"]["id"]).is_none()
                || row.get("sourceEventSeqs").is_some())
        {
            return Ok(failed(out, "dsh_message_ownership_invalid", seq + 2));
        }
        // A settlement without reported usage does not overwrite a prior sample.
        // It still establishes an observed attempt when no sample exists.
        let key = format!(
            "v4:{}",
            serde_json::to_string(&(session, turn.unwrap(), step.unwrap(), attempt))?
        );
        if sample(row).is_some() || !folded.contains_key(&key) {
            folded.insert(key, row);
        }
    }
    if context.tracked.keys().any(|key| !folded.contains_key(key)) {
        return Ok(failed(out, "dsh_snapshot_regressed", 1));
    }
    let mut tracked = BTreeMap::new();
    for (key, row) in folded {
        crate::adapters::run_policy::check()?;
        if limits
            .jsonl
            .time_budget
            .is_some_and(|b| started.elapsed() >= b)
        {
            return Ok(failed(out, "dsh_snapshot_budget", 1));
        }
        let data = &row["data"];
        let source = &data["message"]["source"];
        let response = &source["replayState"]["response"];
        let own_model = row["type"] == "assistant/message"
            && data["message"]["role"] == "assistant"
            && source["kind"] == "model";
        let verified_api = own_model
            && response["kind"] == "pi-ai"
            && response["version"] == 2
            && response["api"] == "openai-completions"
            && text(&source["model"]).is_some()
            && text(&source["provider"]).is_some()
            && response["model"] == source["model"]
            && response["provider"] == source["provider"];
        let mapped = map_usage(sample(row), verified_api);
        if sample(row).is_some() && mapped.is_none() {
            out.diagnostics.push(diag(
                "usage_shape_deviation",
                row["seq"].as_u64().unwrap() as usize + 2,
            ));
            out.health = "degraded".into();
        }
        let (usage, quality) = if let Some(mapped) = mapped {
            for issue in mapped.diagnostics {
                out.diagnostics
                    .push(diag(issue.code, row["seq"].as_u64().unwrap() as usize + 2));
                out.health = "degraded".into();
            }
            (mapped.usage, mapped.quality)
        } else {
            (TokenUsage::default(), TokenQuality::default())
        };
        let Some(ts) = row["time"]
            .as_i64()
            .filter(|v| (MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(v))
        else {
            return Ok(failed(
                out,
                "dsh_timestamp_invalid",
                row["seq"].as_u64().unwrap() as usize + 2,
            ));
        };
        let model = own_model
            .then(|| text(&source["model"]).map(str::to_string))
            .flatten();
        let provider = own_model
            .then(|| text(&source["provider"]).map(str::to_string))
            .flatten();
        let mut event = EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: key.clone(),
            record_kind: RecordKind::ModelCall,
            schema_version: "dsh-session-v4".into(),
            parser_version: PARSER_VERSION.into(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: Some(key.clone()),
            session_id: Some(session.into()),
            parent_session_id: head["parentSession"].as_str().map(str::to_string),
            host_application: None,
            agent: "deepseek-harness".into(),
            call_category: if head["origin"] == "subagent" {
                CallCategory::SubAgent
            } else {
                CallCategory::Primary
            },
            occurred_at_ms: ts,
            observed_at_ms: None,
            source_time: Some(ts.to_string()),
            time_basis: TimeBasis::ObservedAt,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: provider,
            model_raw: model.clone(),
            model_canonical: None,
            model_attribution: if model.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage,
            quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: if row["type"] == "assistant/attempt" {
                Some("attempt_settlement".into())
            } else if data["interrupted"] == true {
                Some("interrupted".into())
            } else {
                None
            },
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        };
        let digest = crate::identity::event_content_hash(&event);
        let previous = context.tracked.get(&key).filter(|v| v.digest == digest);
        let revision = if let Some(p) = previous {
            p.revision
        } else {
            context.revision_floor = context
                .revision_floor
                .checked_add(1)
                .ok_or(CoreError::Overflow("DSH revision"))?;
            context.revision_floor
        };
        let observed_at = previous.map(|p| p.observed_at).unwrap_or(now_ms);
        event.source_revision = Some(revision);
        event.observed_at_ms = Some(observed_at);
        tracked.insert(
            key,
            Tracked {
                digest,
                revision,
                observed_at,
            },
        );
        out.events.push(event);
    }
    if crate::adapters::jsonl::probe_file(&target.path)? != target.probe {
        return Ok(failed(out, "dsh_source_changed_during_read", 1));
    }
    context.tracked = tracked;
    context.session = Some(session.into());
    context.created_at = Some(created_at);
    out.records_seen = out.events.len() as u64;
    out.status = ScanStatus::Complete;
    out.cursor = Some(
        serde_json::json!({"generation":target.generation,"offset":target.probe.len,"line_number":rows.len()+1}),
    );
    out.parse_context = Some(serde_json::to_value(context)?);
    Ok(out)
}
