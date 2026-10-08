//! OTel span JSONL parser: spans_doc1, Copilot native file parser v2.
//!
//! Reference: official documentation checked 2026-09-29 and 30 local CLIENT spans
//! from VS Code 1.140.0 on 2026-10-02; CLI/JetBrains require independent version checks.
//! - VS Code Copilot exporter reference: microsoft/vscode extensions/copilot/docs/
//!   monitoring/agent_monitoring.md at bdc5ebe.
//!   Use github.copilot.chat.otel.exporterType=file and outfile, or
//!   COPILOT_OTEL_FILE_EXPORTER_PATH. Exported JSONL records are native SDK records,
//!   rather than OTLP request payloads. They retain trace/span IDs, parent context,
//!   attributes/resource/scope/status and second/nanosecond start/end/duration pairs.
//!   CLIENT chat spans represent individual model requests with
//!   gen_ai.request.model/gen_ai.response.model and
//!   gen_ai.usage.input_tokens/output_tokens/cache_read.input_tokens/
//!   cache_creation.input_tokens/reasoning.output_tokens.
//!   TTFT fields include copilot_chat.time_to_first_token in milliseconds
//!   and gen_ai.response.time_to_first_chunk in seconds.
//!   Preserve these different units during normalization.
//! - Copilot CLI documentation describes COPILOT_OTEL_FILE_EXPORTER_PATH JSON lines;
//!   its native line schema still requires a local sample for acceptance.
//!   This implementation attempts structurally compatible spans without declaring release support.
//!   github.copilot.nano_aiu is credit; invoke_agent summarizes an entire turn.
//!   Read chat spans only, excluding parent sums to prevent duplicate usage.
//! - CodeBuddy agentlens documents model_stream request spans with
//!   usage.input_tokens/output_tokens/total_tokens and model_name/request.model.
//!   response.time_to_first_token has an unspecified unit in the reference.
//!   Exclude model_request to avoid duplicate usage.
//!   Its referenced protobuf transport is normalized by the local OTLP receiver to JSONL.
//! - Accept spanId/span_id spellings where references do not specify one exact field.
//!   Invalid records are skipped with diagnostics rather than invented values.
//! - Checked VS Code input includes cache and reasoning is an output subset;
//!   derive totals only from known input/output. Do not infer those rules for other clients.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::OTEL_FORMAT_VERSION;

pub const OTEL_PARSER_VERSION: &str = "otel-spans-file-2";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct OtelParseContext {
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
        .and_then(serde_json::Value::as_u64)
        != Some(3)
}

fn diag(code: &str, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(format!("line:{line}")),
        message: message.to_string(),
    }
}

fn restore_cursor(stored: &StoredScanState, generation: i64, rescan: bool) -> JsonlCursor {
    if rescan {
        return JsonlCursor {
            generation,
            offset: 0,
            line_number: 1,
        };
    }
    stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
        .filter(|c| c.generation == generation)
        .unwrap_or(JsonlCursor {
            generation,
            offset: 0,
            line_number: 1,
        })
}

/// Attribute maps from record attributes and resource.attributes.
fn attrs_of(record: &serde_json::Value) -> Vec<&serde_json::Map<String, serde_json::Value>> {
    let mut out = Vec::new();
    if let Some(map) = record.get("attributes").and_then(|v| v.as_object()) {
        out.push(map);
    }
    if let Some(map) = record
        .pointer("/resource/attributes")
        .and_then(|v| v.as_object())
    {
        out.push(map);
    }
    out
}

fn attr_u64(record: &serde_json::Value, keys: &[&str]) -> Option<Option<i64>> {
    for map in attrs_of(record) {
        for key in keys {
            match map.get(*key) {
                None => continue,
                Some(v) => {
                    // Numeric attributes may be direct numbers or OTLP intValue wrappers.
                    let n = v
                        .as_i64()
                        .or_else(|| v.get("intValue").and_then(|x| x.as_i64()))
                        .or_else(|| {
                            v.get("asInt")
                                .and_then(|x| x.as_i64())
                                .or_else(|| v.get("doubleValue").and_then(|x| x.as_i64()))
                        })?;
                    // Negative/out-of-range values return None for an invalid record;
                    // distinguish this from absent fields Some(None) instead of silently dropping known buckets.
                    if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                        return None;
                    }
                    return Some(Some(n));
                }
            }
        }
    }
    Some(None)
}

fn attr_str<'a>(record: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    for map in attrs_of(record) {
        for key in keys {
            if let Some(v) = map.get(*key) {
                if let Some(s) = v.as_str() {
                    return Some(s);
                }
                if let Some(s) = v.get("stringValue").and_then(|x| x.as_str()) {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Read float attributes, including fractional TTFT seconds and doubleValue/intValue wrappers.
fn attr_f64(record: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    for map in attrs_of(record) {
        for key in keys {
            if let Some(v) = map.get(*key) {
                let f = v
                    .as_f64()
                    .or_else(|| v.get("doubleValue").and_then(|x| x.as_f64()))
                    .or_else(|| v.get("intValue").and_then(|x| x.as_i64()).map(|n| n as f64));
                if let Some(f) = f {
                    if f.is_finite() && f >= 0.0 && f <= MAX_REASONABLE_TOKEN as f64 {
                        return Some(f);
                    }
                }
            }
        }
    }
    None
}

/// Map service.name to the statistics Agent; missing service remains an unknown OTel source.
/// VS telemetry shares the vs-copilot name; source selection must prevent duplicate ingestion.
fn agent_of(record: &serde_json::Value) -> &'static str {
    match attr_str(record, &["service.name", "service_name"]).unwrap_or("") {
        "github-copilot" | "copilot-cli" => "copilot-cli",
        "copilot-chat" | "vscode-copilot-chat" => "vscode-copilot-chat",
        "vs-copilot" | "visualstudio-copilot" => "vs-copilot",
        "codebuddy" | "codebuddy-code" | "codebuddy_code" => "codebuddy",
        _ => "otel-unknown",
    }
}

/// startTime accepts second/nanosecond pairs or epoch-millisecond integers.
fn start_ms(record: &serde_json::Value) -> Option<i64> {
    match record.get("startTime") {
        Some(serde_json::Value::Array(pair)) if pair.len() == 2 => {
            let secs = pair[0].as_i64()?;
            let nanos = pair[1].as_i64().unwrap_or(0);
            let ms = secs.checked_mul(1000)?.checked_add(nanos / 1_000_000)?;
            (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
                .contains(&ms)
                .then_some(ms)
        }
        Some(v) => {
            let ms = v.as_i64()?;
            (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
                .contains(&ms)
                .then_some(ms)
        }
        None => None,
    }
}

/// Request spans are chat for Copilot or model_stream for CodeBuddy.
/// Exclude invoke_agent/codebuddy_code.interaction aggregates and model_request
/// so parent/request representations are not counted twice.
fn per_request(name: &str) -> bool {
    matches!(name, "chat" | "model_stream") || name.starts_with("chat ")
}

fn span_identity(value: &serde_json::Value, span: &str) -> String {
    match value
        .get("traceId")
        .or_else(|| value.get("trace_id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(trace) => serde_json::to_string(&(trace, span)).expect("string tuple serialization"),
        None => span.to_string(),
    }
}

fn is_summary(name: &str) -> bool {
    matches!(
        name,
        "invoke_agent" | "codebuddy_code.interaction" | "model_request"
    )
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    scan_with_byte_budget(target, stored, limits, now_ms, None)
}

/// Configuration checks sample bounded bytes without advancing persisted processing state.
pub fn scan_with_byte_budget(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
    max_bytes: Option<u64>,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<OtelParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = OtelParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let cursor = restore_cursor(
        stored,
        target.generation,
        target.rescan || context.policy_version != 3,
    );
    let read = crate::adapters::jsonl::read_jsonl_with_byte_budget(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
        max_bytes,
    )?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    for line in &read.lines {
        records_seen += 1;
        let Ok(value) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&line.text)
        else {
            diagnostics.push(diag("invalid_json_line", line.number, "line is not JSON"));
            continue;
        };
        let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if is_summary(name) {
            if !context.skipped_names.contains(&name.to_string()) {
                context.skipped_names.push(name.to_string());
            }
            continue; // Exclude parent sums that would overlap individual request spans.
        }
        if !per_request(name) {
            continue; // Skip tool, log, metric, and other non-usage records.
        }
        // Native SDK files mix spans/metrics/logs and encode CLIENT as 2;
        // OTLP uses 3. Reject explicitly non-CLIENT chat spans.
        if name.starts_with("chat")
            && value.get("kind").is_some_and(|k| {
                k.as_i64() != Some(2)
                    && k.as_str() != Some("CLIENT")
                    && k.as_str() != Some("SPAN_KIND_CLIENT")
            })
        {
            continue;
        }
        // Copilot uses gen_ai.* token fields; CodeBuddy uses usage.* fields.
        let input = attr_u64(&value, &["gen_ai.usage.input_tokens", "usage.input_tokens"]);
        let output = attr_u64(
            &value,
            &["gen_ai.usage.output_tokens", "usage.output_tokens"],
        );
        let cache_read = attr_u64(
            &value,
            &[
                "gen_ai.usage.cache_read.input_tokens",
                "usage.cache_read_input_tokens",
            ],
        );
        let cache_write = attr_u64(
            &value,
            &[
                "gen_ai.usage.cache_creation.input_tokens",
                "usage.cache_creation_input_tokens",
            ],
        );
        let reasoning = attr_u64(
            &value,
            &[
                "gen_ai.usage.reasoning.output_tokens",
                "gen_ai.usage.reasoning_tokens",
                "usage.reasoning_tokens",
            ],
        );
        if [
            input.is_none(),
            output.is_none(),
            cache_read.is_none(),
            cache_write.is_none(),
            reasoning.is_none(),
        ]
        .iter()
        .any(|v| *v)
        {
            diagnostics.push(diag(
                "token_shape_deviation",
                line.number,
                "a usage attribute carries an out-of-range value; record skipped",
            ));
            continue;
        }
        let (input, output, cache_read, cache_write, reasoning) = (
            input.unwrap(),
            output.unwrap(),
            cache_read.unwrap(),
            cache_write.unwrap(),
            reasoning.unwrap(),
        );
        let Some(occurred_ms) = start_ms(&value) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "startTime missing/implausible; record skipped",
            ));
            continue;
        };
        // Accept both spanId and span_id where field spelling is not specified.
        let span_id = value
            .get("spanId")
            .or_else(|| value.get("span_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if span_id.is_empty() {
            diagnostics.push(diag(
                "span_id_missing",
                line.number,
                "span record without span ID; skipped",
            ));
            continue;
        }
        let model = attr_str(
            &value,
            &[
                "gen_ai.request.model",
                "gen_ai.response.model",
                "model_name",
                "request.model",
            ],
        )
        .map(str::to_string);
        // TTFT field units differ: copilot_chat is milliseconds,
        // gen_ai.response.time_to_first_chunk is fractional seconds,
        // and agentlens response.time_to_first_token has no documented unit.
        // Convert specified units directly; the >1e4-millisecond heuristic applies only to unspecified units.
        let ttft_ms = {
            let documented_ms =
                attr_u64(&value, &["copilot_chat.time_to_first_token"]).unwrap_or(None);
            let documented_s = attr_f64(&value, &["gen_ai.response.time_to_first_chunk"]);
            let unmarked = attr_u64(&value, &["response.time_to_first_token"]).unwrap_or(None);
            if let Some(ms) = documented_ms {
                Some(ms)
            } else if let Some(s) = documented_s {
                Some((s * 1000.0).round() as i64)
            } else {
                unmarked.map(|v| {
                    if v > 10_000 {
                        v
                    } else {
                        v.saturating_mul(1000)
                    }
                })
            }
        };
        // Preserve failed-call status from OTel Status.code;
        // accept STATUS_CODE_ERROR strings or numeric enum 2.
        let error_status = match value.pointer("/status/code") {
            Some(serde_json::Value::String(s))
                if s == "STATUS_CODE_ERROR" || s.eq_ignore_ascii_case("error") =>
            {
                Some("error".to_string())
            }
            Some(serde_json::Value::Number(n)) if n.as_i64() == Some(2) => {
                Some("error".to_string())
            }
            _ => None,
        };
        let copilot = agent_of(&value) == "vscode-copilot-chat";
        let total = if copilot {
            input.zip(output).and_then(|(i, o)| i.checked_add(o))
        } else {
            None
        };
        let uncached = if copilot {
            input
                .zip(cache_read)
                .zip(cache_write)
                .and_then(|((i, r), w)| i.checked_sub(r)?.checked_sub(w))
                .filter(|v| *v >= 0)
        } else {
            None
        };
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: uncached,
                input_cache_read: cache_read,
                input_cache_write: cache_write,
                input_total: input,
                output_total: output,
                output_reasoning: reasoning,
                total_tokens: total,
                source_total: None,
            },
            // Known source buckets need Reported quality, while calculated fields use Derived.
            // Marking all fields Unknown would fail domain value/quality validation and reject the event.
            crate::domain::TokenQuality {
                input_uncached: crate::domain::FieldQuality::Derived,
                total_tokens: crate::domain::FieldQuality::Derived,
                input_cache_read: crate::domain::FieldQuality::Reported,
                input_cache_write: crate::domain::FieldQuality::Reported,
                input_total: crate::domain::FieldQuality::Reported,
                output_total: crate::domain::FieldQuality::Reported,
                output_reasoning: crate::domain::FieldQuality::Reported,
                ..Default::default()
            },
            Vec::new(),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("otel:{}", span_identity(&value, span_id)),
            record_kind: RecordKind::ModelCall,
            schema_version: OTEL_FORMAT_VERSION.to_string(),
            parser_version: OTEL_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: Some(format!("otel-span:{}", span_identity(&value, span_id))),
            attempt_id: None,
            session_id: attr_str(
                &value,
                &[
                    "copilot_chat.chat_session_id",
                    "gen_ai.conversation.id",
                    "copilot_chat.session_id",
                    "gen_ai.session.id",
                ],
            )
            .map(str::to_string),
            parent_session_id: attr_str(&value, &["copilot_chat.parent_chat_session_id"])
                .map(str::to_string),
            host_application: (agent_of(&value) == "vscode-copilot-chat")
                .then(|| "vscode".to_string()),
            agent: agent_of(&value).to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceStart,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: model,
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Corrected,
            source_revision: None,
            error_status,
            duration_ms: None,
            ttft_ms,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
    }
    let status = match read.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, offset } => {
            diagnostics.push(diag(
                "line_exceeds_cap",
                number,
                &format!("line at byte {offset} exceeds the cap; cursor held for retry"),
            ));
            ScanStatus::LineTooLong
        }
    };
    if matches!(status, ScanStatus::Complete) && diagnostics.is_empty() {
        context.policy_version = 3;
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

    #[test]
    fn start_time_pair_and_epoch() {
        let record: serde_json::Value =
            crate::adapters::run_policy::json_from_str(r#"{"startTime": [1780000000, 500000000]}"#)
                .unwrap();
        assert_eq!(start_ms(&record), Some(1_780_000_000_500));
        let record: serde_json::Value =
            crate::adapters::run_policy::json_from_str(r#"{"startTime": 1780000000500}"#).unwrap();
        assert_eq!(start_ms(&record), Some(1_780_000_000_500));
    }

    #[test]
    fn attribute_shapes_bare_and_otlp() {
        let record: serde_json::Value = crate::adapters::run_policy::json_from_str(
            r#"{"name":"chat","spanId":"ab","attributes":{"gen_ai.usage.input_tokens":100,
               "gen_ai.usage.cache_read.input_tokens":{"intValue":40},
               "copilot_chat.time_to_first_token":7298}}"#,
        )
        .unwrap();
        assert_eq!(
            attr_u64(&record, &["gen_ai.usage.input_tokens"]),
            Some(Some(100))
        );
        assert_eq!(
            attr_u64(&record, &["gen_ai.usage.cache_read.input_tokens"]),
            Some(Some(40))
        );
        assert_eq!(attr_u64(&record, &["usage.input_tokens"]), Some(None));
        assert_eq!(
            attr_str(&record, &["gen_ai.request.model", "model_name"]),
            None
        );
    }

    #[test]
    fn span_classification() {
        assert!(per_request("chat"));
        assert!(per_request("model_stream"));
        assert!(is_summary("invoke_agent"));
        assert!(is_summary("codebuddy_code.interaction"));
        assert!(is_summary("model_request"));
        assert!(!per_request("execute_tool"));
    }
}
