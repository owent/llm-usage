//! Claude Code transcript JSONL parser: transcript_doc1, moved from root claude.rs
//! in M2/V30 with the same whole-file rejection behavior.
//!
//! Older documented format A01 and native 2.1.197 use separate field rules:
//! - Path: $CLAUDE_CONFIG_DIR/projects/<project>/<session>.jsonl, defaulting to
//!   ~/.claude/projects/...; subagents use projects/<project>/<session>/subagents/.
//!   Older replaced transcripts may remain as <session>.orphaned-<ts>-<suffix>.jsonl
//!   or <session>.jsonl.superseded-<ts>, overlapping the active transcript.
//!   Stable event identities prevent those files from counting the same response twice.
//! - The monitoring documentation lists input/output/cache_read/cache_creation.
//!   Content blocks can repeat response usage. Prefer requestId, then message.id;
//!   native 2.1.197 samples have repeated message.id without requestId.
//!   OTel query_source main/subagent/auxiliary is not ingested by this parser.
//! - Upstream describes transcript entries as an internal, unstable format.
//!   The older documented mapping uses API usage fields input_tokens/output_tokens/
//!   cache_read_input_tokens/cache_creation_input_tokens, without verifying native default zeros.
//!
//! Reject files containing unsupported record types or usage on non-usage records (V17).
//! Retain the cursor so subsequent scans retry rather than infer another format.
//!
//! Native 2.1.197 per-record versions and positive/default-zero rules are recorded in validation.
//! Retain the older versionless format separately; other releases are compatibility reads only.

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};

use super::super::common::{map_claude_native, map_claude_transcript, ClaudeTranscriptUsage};
use super::CLAUDE_FORMAT_VERSION;

pub const CLAUDE_PARSER_VERSION: &str = "claude-transcript-doc1";
pub const NATIVE_PARSER_VERSION: &str = "claude-transcript-doc2";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Persist per-file diagnostic flags and format selection across incremental scans.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClaudeParseContext {
    #[serde(default)]
    unmapped_usage_keys_reported: bool,
    #[serde(default)]
    assistant_without_usage_reported: bool,
    /// Format selection: known_version/latest_fallback; older contexts default to None.
    /// Moving files does not recreate sources; assistant records select their own version rules.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
    #[serde(default)]
    native_rules_version: Option<u8>,
    #[serde(default)]
    native_rules_in_progress: bool,
    #[serde(default)]
    native_rules_invalid: bool,
}

/// Restore stored cursor JSON; restart at the file head for rescans or invalid cursors.
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> ClaudeParseContext {
    if rescan {
        return ClaudeParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ClaudeParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

fn diag(code: &str, field: Option<&str>, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(format!("line {line}")),
        message: message.to_string(),
    }
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn parse_ts_ms(value: &serde_json::Value) -> Option<(i64, String)> {
    let raw = json_str(value, "timestamp")?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

/// Parse four required nonnegative, bounded message.usage fields from assistant entries.
/// Missing/invalid fields return None; extra keys produce diagnostics without dropping valid usage.
fn parse_usage(value: &serde_json::Value) -> Option<ClaudeTranscriptUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(ClaudeTranscriptUsage {
        input_tokens: get("input_tokens")?,
        output_tokens: get("output_tokens")?,
        cache_read_input_tokens: get("cache_read_input_tokens")?,
        cache_creation_input_tokens: get("cache_creation_input_tokens")?,
    })
}

const USAGE_KEYS: &[&str] = &[
    "input_tokens",
    "output_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
];

fn has_unknown_usage_keys(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .map(|obj| obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str())))
        .unwrap_or(false)
}

/// Return the parent session name for files under .../<session>/subagents/.
fn subagent_parent_session(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    if parent.file_name()?.to_str()? != "subagents" {
        return None;
    }
    Some(parent.parent()?.file_name()?.to_str()?.to_string())
}

/// Scan a transcript JSONL file, dispatched by ClaudeAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let replay = stored.cursor.is_some()
        && stored
            .parse_context
            .as_ref()
            .and_then(|c| c.get("native_rules_version"))
            .and_then(serde_json::Value::as_u64)
            != Some(1)
        && !stored
            .parse_context
            .as_ref()
            .and_then(|c| c.get("native_rules_in_progress"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    let cursor = restore_cursor(stored, target.generation, target.rescan || replay);
    let mut context = restore_context(stored, target.rescan || replay);
    // File-format checks remain independent of the client version carried by each record.
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String, &'static str)> = None;
    let parent_from_path = subagent_parent_session(&target.path);
    let outcome = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    for bad in &outcome.bad_lines {
        crate::adapters::run_policy::check()?;
        diagnostics.push(diag(
            bad.code,
            None,
            bad.number,
            "line is not valid UTF-8; isolated, content not stored",
        ));
    }
    for raw in &outcome.lines {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&raw.text)
        else {
            diagnostics.push(diag(
                "bad_json_line",
                None,
                raw.number,
                "line is not valid JSON; isolated, content not stored",
            ));
            continue;
        };
        let record_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match record_type {
            "assistant" => {
                let usage_value = line.get("message").and_then(|m| m.get("usage"));
                let Some(usage_value) = usage_value else {
                    // Assistant entries without usage produce no event; diagnose once per file.
                    if !context.assistant_without_usage_reported {
                        context.assistant_without_usage_reported = true;
                        diagnostics.push(diag(
                            "assistant_without_usage",
                            Some("message.usage"),
                            raw.number,
                            "assistant entry without message.usage; no usage evidence, no event",
                        ));
                    }
                    continue;
                };
                let Some(usage) = parse_usage(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("message.usage"),
                        raw.number,
                        "assistant usage missing required numeric fields or out of range; record skipped",
                    ));
                    continue;
                };
                if !context.unmapped_usage_keys_reported && has_unknown_usage_keys(usage_value) {
                    context.unmapped_usage_keys_reported = true;
                    diagnostics.push(diag(
                        "unmapped_usage_keys",
                        Some("message.usage"),
                        raw.number,
                        "usage object carries keys beyond the documented four; mapped fields kept",
                    ));
                }
                let Some((occurred_ms, source_time)) = parse_ts_ms(&line) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let session_id = json_str(&line, "sessionId").unwrap_or("unknown-session");
                let request_id = json_str(&line, "requestId");
                let message_id = line.get("message").and_then(|m| json_str(m, "id"));
                let entry_uuid = json_str(&line, "uuid");
                let (source_record_key, origin_call_id) = if let Some(rid) = request_id {
                    (format!("req:{rid}"), Some(rid.to_string()))
                } else if let Some(mid) = message_id {
                    diagnostics.push(diag(
                        "missing_request_id",
                        Some("requestId"),
                        raw.number,
                        "assistant entry without requestId; fallback identity message.id",
                    ));
                    (format!("msg:{mid}"), Some(mid.to_string()))
                } else if let Some(uuid) = entry_uuid {
                    diagnostics.push(diag(
                        "missing_request_id",
                        Some("requestId"),
                        raw.number,
                        "assistant entry without requestId/message.id; fallback identity entry uuid",
                    ));
                    (format!("uuid:{uuid}"), None)
                } else {
                    diagnostics.push(diag(
                        "missing_request_id",
                        Some("requestId"),
                        raw.number,
                        "assistant entry without requestId/message.id/uuid; fallback identity session + line",
                    ));
                    (format!("seq:{session_id}:{}", raw.number), None)
                };
                let is_sidechain = line
                    .get("isSidechain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let model = line
                    .get("message")
                    .and_then(|m| json_str(m, "model"))
                    .map(str::to_string);
                // 2.1.197 placeholder samples use model=<synthetic> with default-zero usage.
                // They do not establish a model call; skip the placeholder response.
                if model.as_deref() == Some("<synthetic>") {
                    diagnostics.push(diag(
                        "synthetic_assistant_skipped",
                        Some("message.model"),
                        raw.number,
                        "assistant entry with <synthetic> model placeholder; no model invocation, no event",
                    ));
                    continue;
                }
                let source_version = line.get("version").map(|v| {
                    v.as_str()
                        .filter(|s| s.len() <= 128)
                        .unwrap_or("unverified-version")
                });
                let schema_version = source_version.unwrap_or(CLAUDE_FORMAT_VERSION);
                let selection = super::select(Some(schema_version));
                let mapped = if source_version.is_some() {
                    map_claude_native(&usage)
                } else {
                    map_claude_transcript(&usage)
                };
                if selection.basis == VersionBasis::LatestFallback {
                    diagnostics.push(diag(
                        "latest_fallback",
                        Some("version"),
                        raw.number,
                        "record version unverified; conservative native mapping retained",
                    ));
                }
                for contradiction in &mapped.diagnostics {
                    diagnostics.push(diag(
                        contradiction.code,
                        Some(contradiction.field),
                        raw.number,
                        &contradiction.detail,
                    ));
                }
                events.push(EventInput {
                    source_instance_id: target.instance_id.clone(),
                    source_record_key,
                    record_kind: RecordKind::ModelCall,
                    schema_version: schema_version.to_string(),
                    parser_version: if source_version.is_some() {
                        NATIVE_PARSER_VERSION
                    } else {
                        CLAUDE_PARSER_VERSION
                    }
                    .to_string(),
                    parse_basis: Some(selection.basis),
                    origin_call_id,
                    attempt_id: None,
                    session_id: Some(session_id.to_string()),
                    parent_session_id: parent_from_path.clone(),
                    host_application: None,
                    agent: "claude-code".to_string(),
                    call_category: if is_sidechain || parent_from_path.is_some() {
                        CallCategory::SubAgent
                    } else {
                        CallCategory::Primary
                    },
                    occurred_at_ms: occurred_ms,
                    observed_at_ms: Some(now_ms),
                    source_time: Some(source_time),
                    time_basis: TimeBasis::SourceCompletion,
                    interval_start_ms: None,
                    interval_end_ms: None,
                    provider_id: source_version.is_none().then(|| "anthropic".to_string()),
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
            // 2.1.197 inspection identifies queue-operation as queue metadata, attachment
            // as context data, and last-prompt as a session pointer; none report usage.
            // Skip them unless they carry usage, which rejects the file as a format error.
            "user" | "system" | "queue-operation" | "attachment" | "last-prompt" => {
                // Usage on a non-usage record rejects the entire file.
                let carries_usage = line.get("usage").is_some()
                    || line.get("message").and_then(|m| m.get("usage")).is_some();
                if carries_usage {
                    fail_closed = Some((
                        raw.number,
                        format!("record type {record_type:?} carries a usage field"),
                        "usage_on_unexpected_record_type",
                    ));
                    break;
                }
            }
            other => {
                fail_closed = Some((
                    raw.number,
                    format!("record type {other:?} not in documented set"),
                    "undocumented_record_type",
                ));
                break;
            }
        }
    }
    let mut status = match &outcome.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, .. } => {
            diagnostics.push(diag(
                "line_too_long",
                None,
                *number,
                "line exceeds the 8 MiB limit; cursor held at line start for controlled retry",
            ));
            ScanStatus::LineTooLong
        }
    };
    let new_cursor = JsonlCursor {
        generation: target.generation,
        offset: outcome.next_offset,
        line_number: outcome.next_line_number,
    };
    let mut health_degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line" | "usage_shape_deviation" | "line_too_long"
            )
        });
    // On rejection, discard this scan's events and retain the checkpoint for the next retry.
    context.native_rules_invalid |= health_degraded;
    context.native_rules_in_progress = status == ScanStatus::BudgetExhausted;
    if status == ScanStatus::Complete && !context.native_rules_invalid && fail_closed.is_none() {
        context.native_rules_version = Some(1);
    }
    let (cursor_out, context_out) = if let Some((line_no, detail, code)) = fail_closed {
        diagnostics.push(diag(code, Some("type"), line_no, &detail));
        events.clear();
        status = ScanStatus::Pending;
        health_degraded = true;
        (None, None)
    } else {
        (
            Some(serde_json::to_value(new_cursor)?),
            Some(serde_json::to_value(&context)?),
        )
    };
    Ok(ScanOutcome {
        status,
        cursor: cursor_out,
        parse_context: context_out,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: outcome.lines.len() as u64,
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
    fn parse_usage_requires_four_fields() {
        let v = serde_json::json!({
            "input_tokens": 10,
            "output_tokens": 2,
            "cache_read_input_tokens": 4,
            "cache_creation_input_tokens": 3
        });
        let usage = parse_usage(&v).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.cache_creation_input_tokens, 3);
        assert!(parse_usage(&serde_json::json!({"input_tokens": 10})).is_none());
        assert!(parse_usage(&serde_json::json!({
            "input_tokens": -1,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        }))
        .is_none());
    }

    #[test]
    fn unknown_usage_keys_detected() {
        let v = serde_json::json!({
            "input_tokens": 1,
            "output_tokens": 1,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "service_tier": "standard"
        });
        assert!(has_unknown_usage_keys(&v));
        assert!(parse_usage(&v).is_some(), "mapped fields kept");
        let clean = serde_json::json!({
            "input_tokens": 1,
            "output_tokens": 1,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        });
        assert!(!has_unknown_usage_keys(&clean));
    }

    #[test]
    fn subagent_parent_from_path() {
        let p = Path::new("/home/u/.claude/projects/proj/sess-1/subagents/agent-a.jsonl");
        assert_eq!(subagent_parent_session(p), Some("sess-1".to_string()));
        let main = Path::new("/home/u/.claude/projects/proj/sess-1.jsonl");
        assert_eq!(subagent_parent_session(main), None);
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // Older contexts without version_basis deserialize to None.
        // The V30 directory move does not recreate sources or reset cursors.
        let legacy = serde_json::json!({
            "unmapped_usage_keys_reported": false,
            "assistant_without_usage_reported": false
        });
        let ctx: ClaudeParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
    }
}
