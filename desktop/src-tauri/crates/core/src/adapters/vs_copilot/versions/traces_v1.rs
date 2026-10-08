//! VS Copilot OTLP telemetry implementation traces_v1; format vs-copilot-otlp-traces-v1.
//!
//! References: real local VS 18 Community data inspected read-only on 2026-10-01.
//! - Each line is an OTLP JSON batch {"resourceSpans":[{"resource":{"attributes":[...]},
//!   "scopeSpans":[{"spans":[...]}]}]}.
//! - Span fields include name/traceId/spanId/parentSpanId/kind,
//!   numeric/string nanosecond startTimeUnixNano/endTimeUnixNano and attributes.
//!   OTLP values use {key,value:{stringValue|intValue|...}}; int64 serializes as a string.
//! - chat <model> CLIENT spans contain gen_ai.usage.input_tokens,
//!   gen_ai.usage.output_tokens and gen_ai.usage.cache_read.input_tokens;
//!   compatibility handling also reads cache_creation/reasoning keys, gen_ai.request.model,
//!   gen_ai.response.model and gen_ai.conversation.id.
//! - invoke_agent root spans summarize whole turns without usage;
//!   skip them to avoid adding summaries to per-request usage, following the OTel warning.
//!
//! Appended batch lines use a JSONL byte-offset cursor.
//! Key vs-copilot:span:<traceId>:<spanId> deduplicates repeated reads.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, FieldQuality, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, TokenQuality, TokenUsage, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VS_COPILOT_PARSER_VERSION: &str = "vs-copilot-otlp-traces-3";

/// Maximum accepted single-span duration: 30 days in milliseconds.
const MAX_DURATION_MS: i64 = 30 * 24 * 3600 * 1000;
const PLAUSIBLE_MS: std::ops::RangeInclusive<i64> =
    crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct VsCopilotParseContext {
    #[serde(default)]
    policy_version: u32,
    #[serde(default)]
    skipped_names: Vec<String>,
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

pub fn should_scan_unchanged(stored: &StoredScanState) -> bool {
    stored
        .parse_context
        .as_ref()
        .and_then(|v| v.get("policy_version"))
        .and_then(Value::as_u64)
        != Some(1)
}

fn diag(code: &str, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

/// OTLP to i64: bare integer, intValue integer/string or numeric stringValue.
fn value_i64(v: &Value) -> Option<i64> {
    let raw = v
        .as_i64()
        .or_else(|| {
            v.get("intValue")
                .and_then(|x| x.as_i64())
                .or_else(|| x_get_str_int(v, "intValue"))
        })
        .or_else(|| {
            v.get("asInt")
                .and_then(|x| x.as_i64())
                .or_else(|| x_get_str_int(v, "asInt"))
        })
        .or_else(|| x_get_str_int(v, "stringValue"))?;
    Some(raw)
}

/// Parse string-encoded integer values such as {"intValue":"123"}.
fn x_get_str_int(v: &Value, key: &str) -> Option<i64> {
    v.get(key)?.as_str()?.trim().parse::<i64>().ok()
}

/// Missing key/value gives Ok(None); unparsable/out-of-range values give Err and a skipped-span diagnostic.
fn attr_i64(attrs: &[Value], key: &str) -> Result<Option<i64>, ()> {
    for attr in attrs {
        if attr.get("key").and_then(|k| k.as_str()) != Some(key) {
            continue;
        }
        let Some(value) = attr.get("value") else {
            continue;
        };
        let Some(n) = value_i64(value) else {
            return Err(());
        };
        if !(0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) {
            return Err(());
        }
        return Ok(Some(n));
    }
    Ok(None)
}

fn attr_str(attrs: &[Value], key: &str) -> Option<String> {
    for attr in attrs {
        if attr.get("key").and_then(|k| k.as_str()) != Some(key) {
            continue;
        }
        let value = attr.get("value")?;
        return value
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                value
                    .get("stringValue")
                    .and_then(|s| s.as_str())
                    .map(str::to_string)
            })
            .or_else(|| {
                // Convert numeric values to strings when no string representation exists.
                value_i64(value).map(|n| n.to_string())
            });
    }
    None
}

fn attr_f64(attrs: &[Value], key: &str) -> Option<f64> {
    let value = attrs
        .iter()
        .find(|a| a.get("key").and_then(Value::as_str) == Some(key))?
        .get("value")?;
    value
        .as_f64()
        .or_else(|| value.get("doubleValue").and_then(Value::as_f64))
        .or_else(|| value_i64(value).map(|n| n as f64))
        .or_else(|| {
            value
                .get("stringValue")
                .and_then(Value::as_str)?
                .parse()
                .ok()
        })
}

/// Unix nanoseconds, numeric or string, to milliseconds.
fn nano_ms(v: &Value) -> Option<i64> {
    let nano = v
        .as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))?;
    let ms = nano.checked_div(1_000_000)?;
    PLAUSIBLE_MS.contains(&ms).then_some(ms)
}

/// Read startTimeUnixNano/endTimeUnixNano used by this product.
/// Also accept startTime/endTime [seconds,nanoseconds] pairs or integer milliseconds.
fn span_time_ms(span: &Value, field: &str) -> Option<i64> {
    if let Some(v) = span.get(format!("{field}UnixNano")) {
        if let Some(ms) = nano_ms(v) {
            return Some(ms);
        }
    }
    match span.get(field) {
        Some(Value::Array(pair)) if pair.len() == 2 => {
            let secs = pair[0].as_i64()?;
            let nanos = pair[1].as_i64().unwrap_or(0);
            let ms = secs.checked_mul(1000)?.checked_add(nanos / 1_000_000)?;
            PLAUSIBLE_MS.contains(&ms).then_some(ms)
        }
        Some(v) => {
            let ms = v.as_i64()?;
            PLAUSIBLE_MS.contains(&ms).then_some(ms)
        }
        None => None,
    }
}

/// Per-request name predicate: chat or chat <model>; the caller checks CLIENT kind.
fn per_request(name: &str) -> bool {
    name == "chat" || name.starts_with("chat ")
}

/// Summary-name predicate excludes turn-wide spans from per-request usage.
fn is_summary(name: &str) -> bool {
    name == "invoke_agent"
        || name.starts_with("invoke_agent")
        || name == "codebuddy_code.interaction"
        || name == "model_request"
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<VsCopilotParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = VsCopilotParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let cursor = if target.rescan || context.policy_version != 1 {
        JsonlCursor {
            generation: target.generation,
            offset: 0,
            line_number: 1,
        }
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .unwrap_or(JsonlCursor {
                generation: target.generation,
                offset: 0,
                line_number: 1,
            })
    };
    let read = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    for line in &read.lines {
        crate::adapters::run_policy::check()?;
        let Ok(doc) = crate::adapters::run_policy::json_from_str::<Value>(&line.text) else {
            diagnostics.push(diag(
                "invalid_json_line",
                &format!("line:{}", line.number),
                "line is not JSON",
            ));
            continue;
        };
        let Some(batches) = doc.get("resourceSpans").and_then(|v| v.as_array()) else {
            diagnostics.push(diag(
                "envelope_missing",
                &format!("line:{}", line.number),
                "line lacks resourceSpans envelope",
            ));
            continue;
        };
        for batch in batches {
            let resource_attrs = batch
                .pointer("/resource/attributes")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if !crate::adapters::vs_copilot::detect::service_name_of(resource_attrs)
                .is_some_and(|s| s == "vs-copilot" || s == "visualstudio-copilot")
            {
                continue;
            }
            let scopes = batch
                .get("scopeSpans")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for scope in scopes {
                let spans = scope
                    .get("spans")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                for span in spans {
                    records_seen += 1;
                    let name = span.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    if is_summary(name) {
                        if !context.skipped_names.contains(&name.to_string()) {
                            context.skipped_names.push(name.to_string());
                        }
                        continue;
                    }
                    if !per_request(name) {
                        continue;
                    }
                    if span.get("kind").is_some_and(|v| {
                        v.as_i64() != Some(3) && v.as_str() != Some("SPAN_KIND_CLIENT")
                    }) {
                        continue;
                    }
                    let attrs = span
                        .get("attributes")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    // Read OTLP token buckets; invalid or out-of-range values diagnose and skip the span.
                    let keys = [
                        "gen_ai.usage.input_tokens",
                        "gen_ai.usage.output_tokens",
                        "gen_ai.usage.cache_read.input_tokens",
                        "gen_ai.usage.cache_creation.input_tokens",
                        "gen_ai.usage.reasoning.output_tokens",
                    ];
                    let mut buckets = [None; 5];
                    let mut bad = false;
                    for (slot, key) in keys.iter().enumerate() {
                        match attr_i64(&attrs, key) {
                            Ok(v) => buckets[slot] = v,
                            Err(()) => bad = true,
                        }
                    }
                    if bad {
                        diagnostics.push(diag(
                            "token_shape_deviation",
                            &format!("line:{}", line.number),
                            "a usage attribute carries an out-of-range value; span skipped",
                        ));
                        continue;
                    }
                    let [input, output, cache_read, cache_write, reasoning] = buckets;
                    let start_ms = span_time_ms(&span, "startTime");
                    let end_ms = span_time_ms(&span, "endTime");
                    let occurred_ms = match (end_ms, start_ms) {
                        (Some(end), _) => end,
                        (None, Some(start)) => start,
                        (None, None) => {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                &format!("line:{}", line.number),
                                "span times missing/implausible; skipped",
                            ));
                            continue;
                        }
                    };
                    let duration_ms = match (end_ms, start_ms) {
                        (Some(end), Some(start)) => {
                            let d = end - start;
                            (0..=MAX_DURATION_MS).contains(&d).then_some(d)
                        }
                        _ => None,
                    };
                    let trace_id = span
                        .get("traceId")
                        .or_else(|| span.get("trace_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let span_id = span
                        .get("spanId")
                        .or_else(|| span.get("span_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if span_id.is_empty() || trace_id.is_empty() {
                        diagnostics.push(diag(
                            "span_id_missing",
                            &format!("line:{}", line.number),
                            "span record without trace/span identity; skipped",
                        ));
                        continue;
                    }
                    let model = attr_str(&attrs, "gen_ai.response.model")
                        .or_else(|| attr_str(&attrs, "gen_ai.request.model"))
                        .or_else(|| {
                            name.strip_prefix("chat ")
                                .filter(|m| !m.is_empty())
                                .map(str::to_string)
                        });
                    let session_id = attr_str(&attrs, "gen_ai.conversation.id")
                        .or_else(|| attr_str(&attrs, "copilot_chat.session_id"));
                    // Compatibility TTFT fields, absent in the local sample: copilot_chat time_to_first_token is ms;
                    // gen_ai.response.time_to_first_chunk is seconds.
                    let ttft_ms = match attr_i64(&attrs, "copilot_chat.time_to_first_token") {
                        Ok(Some(ms)) => Some(ms),
                        _ => match attr_f64(&attrs, "gen_ai.response.time_to_first_chunk") {
                            Some(s)
                                if s.is_finite()
                                    && (0.0..=MAX_DURATION_MS as f64 / 1000.0).contains(&s) =>
                            {
                                Some((s * 1000.0).round() as i64)
                            }
                            _ => None,
                        },
                    }
                    .filter(|ms| (0..=MAX_DURATION_MS).contains(ms));
                    let error_status = match span.pointer("/status/code") {
                        Some(Value::String(s))
                            if s == "STATUS_CODE_ERROR" || s.eq_ignore_ascii_case("error") =>
                        {
                            Some("error".to_string())
                        }
                        Some(Value::Number(n)) if n.as_i64() == Some(2) => {
                            Some("error".to_string())
                        }
                        _ => None,
                    };
                    let usage = TokenUsage {
                        input_uncached: None,
                        input_cache_read: cache_read,
                        input_cache_write: cache_write,
                        input_total: input,
                        output_total: output,
                        output_reasoning: reasoning,
                        total_tokens: input
                            .zip(output)
                            .and_then(|(i, o)| i.checked_add(o))
                            .filter(|v| *v <= crate::domain::MAX_TOKEN_VALUE),
                        source_total: None,
                    };
                    let quality = TokenQuality {
                        total_tokens: FieldQuality::Derived,
                        input_cache_read: cache_read
                            .map(|_| FieldQuality::Reported)
                            .unwrap_or(FieldQuality::Unknown),
                        input_cache_write: cache_write
                            .map(|_| FieldQuality::Reported)
                            .unwrap_or(FieldQuality::Unknown),
                        input_total: input
                            .map(|_| FieldQuality::Reported)
                            .unwrap_or(FieldQuality::Unknown),
                        output_total: output
                            .map(|_| FieldQuality::Reported)
                            .unwrap_or(FieldQuality::Unknown),
                        output_reasoning: reasoning
                            .map(|_| FieldQuality::Reported)
                            .unwrap_or(FieldQuality::Unknown),
                        ..Default::default()
                    };
                    let mapped = crate::adapters::usage_map::finish(usage, quality, Vec::new());
                    events.push(EventInput {
                        source_instance_id: target.instance_id.clone(),
                        source_record_key: format!("vs-copilot:span:{trace_id}:{span_id}"),
                        record_kind: RecordKind::ModelCall,
                        schema_version: super::VS_COPILOT_FORMAT_VERSION.to_string(),
                        parser_version: VS_COPILOT_PARSER_VERSION.to_string(),
                        parse_basis: Some(VersionBasis::KnownVersion),
                        origin_call_id: Some(format!("otel-span:{trace_id}:{span_id}")),
                        attempt_id: None,
                        session_id,
                        parent_session_id: None,
                        host_application: Some("Visual Studio".to_string()),
                        agent: "vs-copilot".to_string(),
                        call_category: CallCategory::Primary,
                        occurred_at_ms: occurred_ms,
                        observed_at_ms: Some(now_ms),
                        source_time: Some(occurred_ms.to_string()),
                        time_basis: if end_ms.is_some() {
                            TimeBasis::SourceCompletion
                        } else {
                            TimeBasis::SourceStart
                        },
                        interval_start_ms: None,
                        interval_end_ms: None,
                        provider_id: Some("github-copilot".to_string()),
                        model_raw: model,
                        model_canonical: None,
                        model_attribution: ModelAttribution::RequestField,
                        usage: mapped.usage,
                        quality: mapped.quality,
                        lifecycle: Lifecycle::Final,
                        source_revision: None,
                        error_status,
                        duration_ms,
                        ttft_ms,
                        attribution_status: AttributionStatus::Verified,
                        exclusion_reason: None,
                        cost: None,
                    });
                }
            }
        }
    }
    let status = match read.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, offset } => {
            diagnostics.push(diag(
                "line_exceeds_cap",
                &format!("line:{number}"),
                &format!("line at byte {offset} exceeds the cap; cursor held for retry"),
            ));
            ScanStatus::LineTooLong
        }
    };
    if matches!(status, ScanStatus::Complete)
        && read.pending_bytes == 0
        && read.bad_lines.is_empty()
        && diagnostics.is_empty()
    {
        context.policy_version = 1;
    }
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(JsonlCursor {
            generation: target.generation,
            offset: read.next_offset,
            line_number: read.next_line_number,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: read.lines.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log(name: &str, lines: &[String]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-vs-copilot-{}-{}-{}.jsonl",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(&path, text).unwrap();
        path
    }

    fn target_for(path: &std::path::Path) -> ScanTarget {
        let probe = crate::adapters::jsonl::probe_file(path).unwrap();
        ScanTarget {
            instance_id: "vs-copilot@test".to_string(),
            path: path.to_path_buf(),
            file_id: "test".to_string(),
            file_identity: "test".to_string(),
            probe,
            generation: 0,
            rescan: false,
        }
    }

    fn run(path: &std::path::Path) -> ScanOutcome {
        scan(
            &target_for(path),
            &StoredScanState::default(),
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap()
    }

    /// Synthetic batch reproducing the observed local structure: string intValue and numeric nanosecond times.
    fn batch(spans: &str) -> String {
        format!(
            r#"{{"resourceSpans":[{{"resource":{{"attributes":[{{"key":"service.name","value":{{"stringValue":"vs-copilot"}}}},{{"key":"service.version","value":{{"stringValue":"18.10.1197+4b9e241b86"}}}}]}},"scopeSpans":[{{"spans":[{spans}]}}]}}]}}"#
        )
    }

    fn chat_span(span_id: &str, trace_id: &str) -> String {
        format!(
            r#"{{"traceId":"{trace_id}","spanId":"{span_id}","parentSpanId":"p","name":"chat gpt-5.3-codex","kind":3,"startTimeUnixNano":1790824588305057500,"endTimeUnixNano":1790824593533010900,"attributes":[{{"key":"gen_ai.usage.input_tokens","value":{{"intValue":"8697"}}}},{{"key":"gen_ai.usage.output_tokens","value":{{"intValue":"81"}}}},{{"key":"gen_ai.usage.cache_read.input_tokens","value":{{"intValue":"4608"}}}},{{"key":"gen_ai.request.model","value":{{"stringValue":"gpt-5.3-codex"}}}},{{"key":"gen_ai.response.model","value":{{"stringValue":"gpt-5.3-codex"}}}},{{"key":"gen_ai.conversation.id","value":{{"stringValue":"12d09993-03fd-412c-b0a3-5d1fc0ebb89c"}}}},{{"key":"gen_ai.input.messages","value":{{"stringValue":"<prompt-body>"}}}}]}}"#
        )
    }

    #[test]
    fn maps_chat_span_with_string_int_and_nano_times() {
        let path = temp_log(
            "map",
            &[batch(&format!(
                "{},{}",
                chat_span("7be9355f942b675e", "e2cc7ef3fbc1a8ebcde0f8ff2a48993e"),
                r#"{"traceId":"t2","spanId":"s2","name":"invoke_agent GitHub Copilot","startTimeUnixNano":1790824588305057500,"endTimeUnixNano":1790824593533010900,"attributes":[{"key":"gen_ai.agent.name","value":{"stringValue":"GitHub Copilot"}}]}"#
            ))],
        );
        let outcome = run(&path);
        assert_eq!(outcome.status, ScanStatus::Complete);
        // Skip invoke_agent summary, leaving one chat event.
        assert_eq!(outcome.events.len(), 1);
        let e = &outcome.events[0];
        assert_eq!(
            e.source_record_key,
            "vs-copilot:span:e2cc7ef3fbc1a8ebcde0f8ff2a48993e:7be9355f942b675e"
        );
        assert_eq!(e.usage.input_total, Some(8697));
        assert_eq!(e.usage.output_total, Some(81));
        assert_eq!(e.usage.total_tokens, Some(8778));
        assert_eq!(e.quality.total_tokens, FieldQuality::Derived);
        assert_eq!(e.usage.input_cache_read, Some(4608));
        assert_eq!(e.model_raw.as_deref(), Some("gpt-5.3-codex"));
        assert_eq!(
            e.session_id.as_deref(),
            Some("12d09993-03fd-412c-b0a3-5d1fc0ebb89c")
        );
        // Nanoseconds to milliseconds: completion time and derived duration truncate endpoints separately.
        assert_eq!(e.occurred_at_ms, 1_790_824_593_533);
        assert_eq!(e.duration_ms, Some(5_228));
        assert_eq!(e.time_basis, TimeBasis::SourceCompletion);
        assert_eq!(e.agent, "vs-copilot");
        assert_eq!(e.host_application.as_deref(), Some("Visual Studio"));
        // Record skipped summary names in parse context.
        assert!(
            outcome
                .parse_context
                .and_then(|c| c
                    .get("skipped_names")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len()))
                .unwrap_or(0)
                > 0
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ttft_double_values_client_kind_and_missing_trace_are_checked() {
        let mut span: Value =
            crate::adapters::run_policy::json_from_str(&chat_span("span-a", "trace-a")).unwrap();
        span["attributes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
            "key":"gen_ai.response.time_to_first_chunk","value":{"doubleValue":0.25}}));
        let path = temp_log("ttft-double", &[batch(&span.to_string())]);
        assert_eq!(run(&path).events[0].ttft_ms, Some(250));
        span["kind"] = serde_json::json!(2);
        std::fs::write(&path, format!("{}\n", batch(&span.to_string()))).unwrap();
        assert!(run(&path).events.is_empty());
        span["kind"] = serde_json::json!(3);
        span.as_object_mut().unwrap().remove("traceId");
        std::fs::write(&path, format!("{}\n", batch(&span.to_string()))).unwrap();
        let out = run(&path);
        assert!(out.events.is_empty());
        assert!(out.diagnostics.iter().any(|d| d.code == "span_id_missing"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn old_eof_checkpoint_replays_complete_snapshot_and_preserves_missing_usage() {
        let path = temp_log("old-eof", &[batch(&chat_span("span", "trace"))]);
        let first = run(&path);
        let old = StoredScanState {
            cursor: first.cursor.clone(),
            parse_context: Some(serde_json::json!({"policy_version":0})),
        };
        assert!(should_scan_unchanged(&old));
        let replay = scan(
            &target_for(&path),
            &old,
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(replay.events.len(), 1);
        assert_eq!(replay.events[0].usage.total_tokens, Some(8778));
        let updated = StoredScanState {
            cursor: replay.cursor,
            parse_context: replay.parse_context,
        };
        assert!(!should_scan_unchanged(&updated));
        let mut span: Value =
            crate::adapters::run_policy::json_from_str(&chat_span("span2", "trace")).unwrap();
        span["attributes"]
            .as_array_mut()
            .unwrap()
            .retain(|a| a["key"] != "gen_ai.usage.output_tokens");
        std::fs::write(&path, format!("{}\n", batch(&span.to_string()))).unwrap();
        let missing = run(&path);
        assert_eq!(missing.events[0].usage.total_tokens, None);
        assert_eq!(
            missing.events[0].quality.total_tokens,
            FieldQuality::Unknown
        );
        span["attributes"][0]["value"]["intValue"] =
            serde_json::json!(crate::domain::MAX_TOKEN_VALUE.to_string());
        span["attributes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"key":"gen_ai.usage.output_tokens","value":{"intValue":"1"}}));
        std::fs::write(&path, format!("{}\n", batch(&span.to_string()))).unwrap();
        let capped = run(&path);
        assert_eq!(capped.events[0].usage.total_tokens, None);
        assert!(
            capped.events[0].validate().is_ok(),
            "oversize derived total must not discard known input/output or the call"
        );
        // Invalid and incomplete replay must keep the old policy eligible for retry.
        std::fs::write(&path, "{broken}\n").unwrap();
        let invalid = scan(
            &target_for(&path),
            &old,
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_ne!(invalid.parse_context.unwrap()["policy_version"], 1);
        std::fs::write(&path, batch(&span.to_string())).unwrap();
        let partial = scan(
            &target_for(&path),
            &old,
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_ne!(partial.parse_context.unwrap()["policy_version"], 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn string_form_nano_timestamps_supported() {
        let path = temp_log(
            "strnano",
            &[batch(
                r#"{"traceId":"t3","spanId":"s3","name":"chat gpt-5.5","startTimeUnixNano":"1790824588305057500","endTimeUnixNano":"1790824593533010900","attributes":[{"key":"gen_ai.usage.input_tokens","value":{"intValue":100}},{"key":"gen_ai.usage.output_tokens","value":{"stringValue":"50"}}]}"#,
            )],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 1);
        let e = &outcome.events[0];
        assert_eq!(e.usage.input_total, Some(100));
        assert_eq!(e.usage.output_total, Some(50));
        assert_eq!(e.occurred_at_ms, 1_790_824_593_533);
        // Without model attributes, read the model segment of the span name.
        assert_eq!(e.model_raw.as_deref(), Some("gpt-5.5"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn no_usage_chat_span_skipped() {
        let path = temp_log(
            "nousage",
            &[batch(
                r#"{"traceId":"t4","spanId":"s4","name":"chat gpt-5.5","attributes":[]}"#,
            )],
        );
        let outcome = run(&path);
        assert!(outcome.events.is_empty());
        assert_eq!(outcome.records_seen, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn out_of_range_usage_diagnosed_and_skipped() {
        let path = temp_log(
            "range",
            &[batch(
                r#"{"traceId":"t5","spanId":"s5","name":"chat m","attributes":[{"key":"gen_ai.usage.input_tokens","value":{"intValue":"-3"}}]}"#,
            )],
        );
        let outcome = run(&path);
        assert!(outcome.events.is_empty());
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "token_shape_deviation"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_span_id_skips() {
        let path = temp_log(
            "noid",
            &[batch(
                r#"{"traceId":"t6","name":"chat m","startTimeUnixNano":1790824588305057500,"endTimeUnixNano":1790824593533010900,"attributes":[{"key":"gen_ai.usage.input_tokens","value":{"intValue":"10"}}]}"#,
            )],
        );
        let outcome = run(&path);
        assert!(outcome.events.is_empty());
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "span_id_missing"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn non_span_lines_and_incremental_resume() {
        let path = temp_log(
            "incr",
            &["not json".to_string(), batch(&chat_span("s7", "t7"))],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 1);
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "invalid_json_line"));
        let cursor = outcome.cursor.unwrap();
        // A malformed legacy snapshot stays eligible for full policy replay.
        let stored = StoredScanState {
            cursor: Some(cursor),
            parse_context: outcome.parse_context.clone(),
        };
        let probe = crate::adapters::jsonl::probe_file(&path).unwrap();
        let outcome2 = scan(
            &ScanTarget {
                instance_id: "vs-copilot@test".to_string(),
                path: path.clone(),
                file_id: "test".to_string(),
                file_identity: "test".to_string(),
                probe,
                generation: 0,
                rescan: false,
            },
            &stored,
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(outcome2.events.len(), 1);
        assert!(should_scan_unchanged(&StoredScanState {
            cursor: outcome2.cursor,
            parse_context: outcome2.parse_context
        }));
        // Once a complete valid snapshot is consumed, normal EOF resume emits nothing.
        std::fs::write(&path, format!("{}\n", batch(&chat_span("s7", "t7")))).unwrap();
        let valid = run(&path);
        let stable = StoredScanState {
            cursor: valid.cursor,
            parse_context: valid.parse_context,
        };
        assert!(!should_scan_unchanged(&stable));
        assert!(scan(
            &target_for(&path),
            &stable,
            &ScanLimits::default(),
            1_800_000_000_000
        )
        .unwrap()
        .events
        .is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
