//! Gemini session JSON parser: session_doc1, documented format session-doc-1.
//!
//! Documented format references (A10):
//! - Path: ~/.gemini/tmp/<project_hash>/chats/session-<date>T<time>-<hash>.json.
//!   One JSON document, not JSONL; no documented environment override.
//! - Root fields: sessionId, projectHash, startTime, lastUpdated, and messages[].
//!   The documented session format includes token usage statistics.
//! - Telemetry gemini_cli.token.usage types: input, output, thought, cache, tool.
//!   api_response has input/output/cached_content/thoughts/tool/total_token_count
//!   plus model and duration_ms; this parser does not ingest that telemetry.
//! - Message tokens fields input/output/cached/thoughts/tool/total are optional.
//!   Inclusion of thoughts, cached, and tool in input/output has not been checked.
//!   Read only reported totals; do not add thoughts/tool to other token fields.
//!
//! Read whole rewritten JSON within a 32 MiB limit. Store consumed bytes so the framework
//! skips unchanged files; content rewrites/truncation change generation and restart reads.
//! Stable event identities prevent duplicate inserts. Parse failure leaves the cursor unchanged.
//!
//! Reject an entire file for undocumented message types or user messages carrying tokens (V17).
//! Leave the cursor unchanged so subsequent scans retry the same file without guessing its format.
//!
//! No client-version field selects another implementation. Event parse_basis is KnownVersion
//! for this documented format, the only registered format; this does not verify native releases.
//! M2/V30 moved the implementation from root-level gemini.rs without changing its behavior.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::usage_map::{map_genai_usage, GenaiUsage};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;

pub const GEMINI_PARSER_VERSION: &str = "gemini-session-doc1";
/// Whole-file read limit, initially 32 MiB.
pub const GEMINI_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Documented message types; unverified info/error/warning shapes are rejected.
const MESSAGE_TYPES: &[&str] = &["user", "gemini"];

/// Six optional token fields; inclusion is unverified, so totals use only reported values.
const TOKEN_KEYS: &[&str] = &["input", "output", "cached", "thoughts", "tool", "total"];

fn diag(code: &str, field: Option<&str>, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// Parse six optional token fields; present values must be nonnegative and bounded.
/// Invalid fields reject the token object; extra keys are flagged while known fields remain.
fn parse_tokens(value: &serde_json::Value) -> Option<(GenaiUsage, bool)> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<Option<i64>> {
        match obj.get(key) {
            None => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                    return None;
                }
                Some(Some(n))
            }
        }
    };
    let usage = GenaiUsage {
        prompt_tokens: get("input")?,
        candidates_tokens: get("output")?,
        cached_tokens: get("cached")?,
        thoughts_tokens: get("thoughts")?,
        tool_tokens: get("tool")?,
        total_tokens: get("total")?,
    };
    let unknown = obj.keys().any(|k| !TOKEN_KEYS.contains(&k.as_str()));
    Some((usage, unknown))
}

/// Whole-document cursor stores consumed bytes and line_number=1 in the shared cursor shape.
/// The unchanged-file shortcut compares probe.len with cursor.offset.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    line_number: u64,
}

/// Scan one session JSON document, dispatched by GeminiAdapter::scan.
/// This documented format has one implementation and no client-version dispatch.
pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let cursor_at = |offset: u64| -> Result<serde_json::Value, CoreError> {
        Ok(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset,
            line_number: 1,
        })?)
    };
    // Oversized files retain their starting cursor and return a limited result for retry.
    if target.probe.len > GEMINI_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            None,
            "document",
            "session JSON exceeds the 32 MiB cap; cursor held at start for controlled retry",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: Some(cursor_at(0)?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    crate::adapters::run_policy::checked_file(&target.path)?
        .take(GEMINI_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > GEMINI_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            None,
            "document",
            "session JSON grew past the 32 MiB cap during read; cursor held at start",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: Some(cursor_at(0)?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let consumed = bytes.len() as u64;
    // Partially written JSON fails parsing without advancing the cursor; retry next scan.
    let document: serde_json::Value =
        match crate::adapters::run_policy::json_from_slice(super::super::strip_bom(&bytes)) {
            Ok(v) => v,
            Err(_) => {
                diagnostics.push(diag(
                    "session_json_unparseable",
                    None,
                    "document",
                    "session JSON does not parse (mid-write or corrupt); cursor held for retry",
                ));
                return Ok(ScanOutcome {
                    status: ScanStatus::Pending,
                    cursor: None,
                    parse_context: None,
                    events,
                    aggregates: Vec::new(),
                    diagnostics,
                    lines_read: 1,
                    records_seen: 0,
                    reconciliations: Vec::new(),
                    health: "active".to_string(),
                });
            }
        };
    // Reject the file: clear events, retain the cursor, and report degraded health for retry.
    let fail_closed = |events: &mut Vec<EventInput>,
                       diagnostics: &mut Vec<DiagnosticInput>,
                       records_seen: u64,
                       code: &'static str,
                       detail: String|
     -> Result<ScanOutcome, CoreError> {
        events.clear();
        diagnostics.push(diag(code, None, "document", &detail));
        Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: std::mem::take(diagnostics),
            lines_read: 1,
            records_seen,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        })
    };
    let Some(session_id) = json_str(&document, "sessionId") else {
        return fail_closed(
            &mut events,
            &mut diagnostics,
            0,
            "session_schema_deviation",
            "top-level sessionId missing or not a string".to_string(),
        );
    };
    let Some(messages) = document.get("messages").and_then(|m| m.as_array()) else {
        return fail_closed(
            &mut events,
            &mut diagnostics,
            0,
            "session_schema_deviation",
            "top-level messages missing or not an array".to_string(),
        );
    };
    let mut records_seen: u64 = 0;
    let mut unmapped_keys_reported = false;
    for (index, message) in messages.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let position = format!("messages[{index}]");
        let Some(message_obj) = message.as_object() else {
            return fail_closed(
                &mut events,
                &mut diagnostics,
                records_seen,
                "session_schema_deviation",
                format!("{position} is not an object"),
            );
        };
        let message_type = message_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if !MESSAGE_TYPES.contains(&message_type) {
            return fail_closed(
                &mut events,
                &mut diagnostics,
                records_seen,
                "undocumented_message_type",
                format!("{position} type {message_type:?} not in documented set"),
            );
        }
        let tokens_value = message_obj.get("tokens");
        if message_type == "user" {
            if tokens_value.is_some() {
                return fail_closed(
                    &mut events,
                    &mut diagnostics,
                    records_seen,
                    "usage_on_unexpected_message_type",
                    format!("{position} user message carries a tokens object"),
                );
            }
            continue;
        }
        let Some(tokens_value) = tokens_value else {
            // Gemini messages without tokens report no usage and produce no event.
            continue;
        };
        let Some((usage, unknown_keys)) = parse_tokens(tokens_value) else {
            diagnostics.push(diag(
                "usage_shape_deviation",
                Some("tokens"),
                &position,
                "tokens value negative, non-integer or out of range; message skipped",
            ));
            continue;
        };
        if unknown_keys && !unmapped_keys_reported {
            unmapped_keys_reported = true;
            diagnostics.push(diag(
                "unmapped_usage_keys",
                Some("tokens"),
                &position,
                "tokens object carries keys beyond the documented six; mapped fields kept",
            ));
        }
        let Some((occurred_ms, source_time)) = json_str(message, "timestamp").and_then(|raw_ts| {
            raw_ts
                .parse::<jiff::Timestamp>()
                .ok()
                .map(|t| (t.as_millisecond(), raw_ts.to_string()))
        }) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("timestamp"),
                &position,
                "message timestamp missing or unparseable; message skipped",
            ));
            continue;
        };
        let (source_record_key, origin_call_id) = match json_str(message, "id") {
            Some(id) => (format!("gemini:{session_id}:{id}"), Some(id.to_string())),
            None => {
                diagnostics.push(diag(
                    "missing_message_id",
                    Some("id"),
                    &position,
                    "gemini message without id; fallback identity array index (append-only 假设)",
                ));
                (format!("gemini:{session_id}:idx-{index}"), None)
            }
        };
        let model = json_str(message, "model").map(str::to_string);
        let mapped = map_genai_usage(&usage);
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                &position,
                &contradiction.detail,
            ));
        }
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: super::GEMINI_FORMAT_VERSION.to_string(),
            parser_version: GEMINI_PARSER_VERSION.to_string(),
            // KnownVersion identifies the registered documented format, not a native client release.
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id,
            attempt_id: None,
            session_id: Some(session_id.to_string()),
            parent_session_id: None,
            host_application: None,
            agent: "gemini-cli".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(source_time),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: Some("google".to_string()),
            model_raw: model.clone(),
            model_canonical: None,
            model_attribution: if model.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
    }
    let health_degraded = diagnostics
        .iter()
        .any(|d| d.code == "usage_shape_deviation");
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(cursor_at(consumed)?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations: Vec::new(),
        health: if health_degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tokens_optional_fields() {
        let full = serde_json::json!({
            "input": 100,
            "output": 20,
            "cached": 30,
            "thoughts": 5,
            "tool": 7,
            "total": 120
        });
        let (usage, unknown) = parse_tokens(&full).unwrap();
        assert_eq!(usage.prompt_tokens, Some(100));
        assert_eq!(usage.thoughts_tokens, Some(5));
        assert!(!unknown);

        let partial = serde_json::json!({"input": 10});
        let (usage, unknown) = parse_tokens(&partial).unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.total_tokens, None);
        assert!(!unknown);

        let negative = serde_json::json!({"input": -1});
        assert!(parse_tokens(&negative).is_none());

        let extra = serde_json::json!({"input": 1, "surprise": 2});
        let (usage, unknown) = parse_tokens(&extra).unwrap();
        assert_eq!(usage.prompt_tokens, Some(1));
        assert!(unknown);
    }
}
