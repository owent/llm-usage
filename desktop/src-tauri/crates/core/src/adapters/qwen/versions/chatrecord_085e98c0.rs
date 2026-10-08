//! Qwen Code ChatRecord JSONL parser: chatrecord_085e98c0, fixed source reference A18.
//!
//! Source: qwen-code commit 085e98c00cac2f8dd29eb39c760409bc6da889a9.
//! - Path: ~/.qwen/tmp/<project_id>/chats/<sessionId>.jsonl, append-only JSONL,
//!   without a documented environment override.
//! - ChatRecord: uuid/parentUuid/sessionId/ISO timestamp/type/
//!   optional subtype (33 values), provenance, cwd, and four types user/assistant/tool_result/system.
//!   Optional fields include CLI version, gitBranch, message, usageMetadata, model,
//!   contextWindowSize, agentId/agentName, isSidechain, and goalContext.
//! - recordAssistantTurn assigns data.tokens to usageMetadata and sets model.
//!   GenerateContentResponseUsageMetadata has six optional token categories.
//!   The optional tokens argument means assistant entries can legitimately lack usageMetadata.
//! - isSidechain=true or agentId identifies subagent records within the same session file.
//! - goal_state.goal.tokensUsed sums totalTokenCount across model calls;
//!   it is not additional request usage and must not be counted twice.
//!   Ignore goal_runtime/goal_turn_end/chat_compression control records as well.
//!   Assistant entries marked goal_context still represent individual model calls.
//! - session_model restores daemon state; it does not identify historical call models.
//!
//! Reject the entire file for unsupported type/subtype values or non-assistant
//! records carrying usageMetadata; keep the cursor for a later retry (V17).
//!
//! Version selection follows the fixed source format in architecture.md#adapter-layout.
//! Preserve each record.version as schema_version; it does not select another parser.
//! The super::versions registry keeps a consistent adapter structure.
//! M2/V30 moved root-level qwen.rs into this directory without changing behavior.

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{map_genai_usage, GenaiUsage};

pub const QWEN_PARSER_VERSION: &str = "qwen-chatrecord-085e98c0-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Four ChatRecord.type values from the fixed source.
pub const RECORD_TYPES: &[&str] = &["user", "assistant", "tool_result", "system"];

/// The 33 ChatRecord.subtype values from commit 085e98c0.
const RECORD_SUBTYPES: &[&str] = &[
    "chat_compression",
    "slash_command",
    "ui_telemetry",
    "at_command",
    "attribution_snapshot",
    "notification",
    "background_task_completed",
    "cron",
    "mid_turn_user_message",
    "custom_title",
    "parent_session",
    "session_source",
    "omni_recall",
    "session_model",
    "rewind",
    "agent_bootstrap",
    "agent_launch_prompt",
    "agent_retry",
    "agent_session_ready",
    "file_history_snapshot",
    "user_text_elements",
    "session_artifact_event",
    "session_artifact_snapshot",
    "session_sources_snapshot",
    "branch_checkpoint",
    "goal_state",
    "goal_runtime",
    "goal_turn_end",
    "realtime_message",
    "turn_result",
    "managed_session_header_v1",
    "managed_session_event_v1",
    "managed_session_commit_v1",
];

/// Ignore control subtypes for goal sums, runtime, turn boundaries, and compaction.
/// goal.tokensUsed spans turns; creating usage events from it would duplicate existing calls.
const IGNORED_SUBTYPES: &[&str] = &[
    "goal_state",
    "goal_runtime",
    "goal_turn_end",
    "chat_compression",
];

/// Six optional GenerateContentResponseUsageMetadata token fields.
const USAGE_KEYS: &[&str] = &[
    "promptTokenCount",
    "candidatesTokenCount",
    "cachedContentTokenCount",
    "thoughtsTokenCount",
    "toolUsePromptTokenCount",
    "totalTokenCount",
];

/// Persist per-file diagnostic flags and format selection across incremental scans.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct QwenParseContext {
    #[serde(default)]
    unmapped_usage_keys_reported: bool,
    /// KnownVersion identifies the registered fixed-source format, not every native client release.
    /// Older contexts default to None; the V30 move does not recreate sources or reset cursors.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// Restore cursor JSON; restart at the head for rescans or invalid cursors.
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> QwenParseContext {
    if rescan {
        return QwenParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<QwenParseContext>(v.clone()).ok())
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

/// Parse six optional usageMetadata fields; present values must be nonnegative and bounded.
/// Flag unknown extra keys once per file while retaining mapped fields.
fn parse_usage_metadata(value: &serde_json::Value) -> Option<(GenaiUsage, bool)> {
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
        prompt_tokens: get("promptTokenCount")?,
        candidates_tokens: get("candidatesTokenCount")?,
        cached_tokens: get("cachedContentTokenCount")?,
        thoughts_tokens: get("thoughtsTokenCount")?,
        tool_tokens: get("toolUsePromptTokenCount")?,
        total_tokens: get("totalTokenCount")?,
    };
    let unknown = obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str()));
    Some((usage, unknown))
}

/// Scan ChatRecord JSONL, dispatched by QwenAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // The fixed-source format is registered as known_version;
    // persisted context follows the shared adapter structure (V30).
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String, &'static str)> = None;
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
        if !RECORD_TYPES.contains(&record_type) {
            fail_closed = Some((
                raw.number,
                format!("record type {record_type:?} not in ChatRecord set"),
                "undocumented_record_type",
            ));
            break;
        }
        if let Some(subtype) = line.get("subtype").and_then(|s| s.as_str()) {
            if !RECORD_SUBTYPES.contains(&subtype) {
                fail_closed = Some((
                    raw.number,
                    format!("record subtype {subtype:?} not in pinned 33-value enum"),
                    "undocumented_record_subtype",
                ));
                break;
            }
            // Ignore control records such as goal sums to avoid duplicate usage.
            if IGNORED_SUBTYPES.contains(&subtype) {
                continue;
            }
        }
        match record_type {
            "assistant" => {
                let usage_value = line.get("usageMetadata");
                let Some(usage_value) = usage_value else {
                    // Optional source tokens permit assistant entries without usageMetadata; create no usage event.
                    continue;
                };
                let Some((usage, unknown_keys)) = parse_usage_metadata(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usageMetadata"),
                        raw.number,
                        "usageMetadata value negative, non-integer or out of range; record skipped",
                    ));
                    continue;
                };
                if unknown_keys && !context.unmapped_usage_keys_reported {
                    context.unmapped_usage_keys_reported = true;
                    diagnostics.push(diag(
                        "unmapped_usage_keys",
                        Some("usageMetadata"),
                        raw.number,
                        "usageMetadata carries keys beyond the pinned six; mapped fields kept",
                    ));
                }
                let Some((occurred_ms, source_time)) =
                    json_str(&line, "timestamp").and_then(|raw_ts| {
                        raw_ts
                            .parse::<jiff::Timestamp>()
                            .ok()
                            .map(|t| (t.as_millisecond(), raw_ts.to_string()))
                    })
                else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "record timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let session_id = json_str(&line, "sessionId").unwrap_or("unknown-session");
                let (source_record_key, origin_call_id) = match json_str(&line, "uuid") {
                    Some(uuid) => (format!("qwen:{uuid}"), Some(uuid.to_string())),
                    None => {
                        diagnostics.push(diag(
                            "missing_uuid",
                            Some("uuid"),
                            raw.number,
                            "assistant record without uuid; fallback identity session + line",
                        ));
                        (format!("seq:{session_id}:{}", raw.number), None)
                    }
                };
                let is_subagent = line
                    .get("isSidechain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || line.get("agentId").is_some();
                let model = json_str(&line, "model").map(str::to_string);
                let mapped = map_genai_usage(&usage);
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
                    schema_version: json_str(&line, "version").unwrap_or("unknown").to_string(),
                    parser_version: QWEN_PARSER_VERSION.to_string(),
                    // KnownVersion records the fixed-source format reference, not universal release acceptance.
                    parse_basis: Some(VersionBasis::KnownVersion),
                    origin_call_id,
                    attempt_id: None,
                    session_id: Some(session_id.to_string()),
                    parent_session_id: None,
                    host_application: None,
                    agent: "qwen-code".to_string(),
                    call_category: if is_subagent {
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
                    provider_id: None,
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
            // usageMetadata on a non-assistant record rejects the whole file.
            _ => {
                if line.get("usageMetadata").is_some() {
                    fail_closed = Some((
                        raw.number,
                        format!("record type {record_type:?} carries usageMetadata"),
                        "usage_on_unexpected_record_type",
                    ));
                    break;
                }
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
    // Discard events and retain the checkpoint on rejection; retry the same file next scan.
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
    fn subtype_enum_matches_pinned_source() {
        assert_eq!(RECORD_SUBTYPES.len(), 33);
        for required in [
            "goal_state",
            "goal_runtime",
            "goal_turn_end",
            "chat_compression",
            "session_model",
        ] {
            assert!(RECORD_SUBTYPES.contains(&required), "missing {required}");
        }
        for ignored in IGNORED_SUBTYPES {
            assert!(RECORD_SUBTYPES.contains(ignored), "ignored not in enum");
        }
    }

    #[test]
    fn parse_usage_metadata_optional_fields() {
        let full = serde_json::json!({
            "promptTokenCount": 100,
            "candidatesTokenCount": 20,
            "cachedContentTokenCount": 30,
            "thoughtsTokenCount": 5,
            "toolUsePromptTokenCount": 7,
            "totalTokenCount": 120
        });
        let (usage, unknown) = parse_usage_metadata(&full).unwrap();
        assert_eq!(usage.prompt_tokens, Some(100));
        assert_eq!(usage.tool_tokens, Some(7));
        assert!(!unknown);

        let partial = serde_json::json!({"promptTokenCount": 10});
        let (usage, unknown) = parse_usage_metadata(&partial).unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.total_tokens, None);
        assert!(!unknown);

        let negative = serde_json::json!({"promptTokenCount": -1});
        assert!(parse_usage_metadata(&negative).is_none());

        let extra = serde_json::json!({"promptTokenCount": 1, "newField": 2});
        let (usage, unknown) = parse_usage_metadata(&extra).unwrap();
        assert_eq!(usage.prompt_tokens, Some(1));
        assert!(unknown);
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // Older contexts without version_basis deserialize successfully to None.
        // The V30 directory move does not recreate sources or reset cursors.
        let legacy = serde_json::json!({"unmapped_usage_keys_reported": true});
        let ctx: QwenParseContext = serde_json::from_value(legacy).expect("restore");
        assert!(ctx.unmapped_usage_keys_reported);
        assert_eq!(ctx.version_basis, None);
    }
}
