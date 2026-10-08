//! Zoo ui_messages.json parser: ui_messages_doc1,
//! format zoo-ui-messages-doc-1.
//!
//! Historical reference: f7806475331fcae5f4e8b5558d04415eeb5da88c (A19).
//! Native 3.86.0 checks use 6aa9d017; legacy finished/condense scenario limits remain separate.
//! - Path: host globalStorage/tasks/<taskId>/ui_messages.json.
//!   src/shared/globalFileNames.ts and packages/core/src/task-persistence/taskMessages.ts
//!   write a whole JSON array rather than JSONL, including after deletion/merging.
//!   VS Code extension identity is ZooCodeOrganization.zoo-code
//!   from src/package.json publisher/name.
//!   Source CLI default: ~/.vscode-mock/global-storage from apps/cli task-history
//!   and vscode-shim paths; native CLI acceptance is separate.
//! - Usage records follow consolidateTokenUsage.ts/consolidateApiRequests.ts:
//!   type=say with api_req_started or condense_context.
//!   api_req_started.text is JSON containing optional input/output/cache-write/
//!   cache-read/cost and apiProtocol anthropic/openai.
//!   The historical finished path merges later usage/cost back into started.
//!   LIFO pairing pops the latest unmatched start; finish fields override start
//!   through {...startData,...finishData}. Discard unmatched finished entries.
//!   tokensIn includes cache under both protocol rules.
//!   Upstream contextTokens=input+output is calculated request usage;
//!   it does not establish a separately reported native total.
//! - condense_context.contextCondense.cost contributes to upstream totalCost.
//!   newContextTokens measures context size, without request token usage.
//!   Record a separate auxiliary call with unknown tokens and any positive estimated cost.
//! - Millisecond ts is the documented message identity field;
//!   array positions change after deletion and are excluded from identity.
//!
//! Official 3.86.0 VSIX and source 6aa9d017 have checked native samples.
//! Read full ask/say enums from packages/types/src/message.ts; skip non-usage records.
//! Unknown types reject the file without advancing its cursor; default-zero usage/cost stays unknown.
//!
//! Read whole JSON within 32 MiB, storing consumed bytes for unchanged-file checks.
//! Rewrites/truncation change generation; stable keys prevent duplicate imports.
//! Partial JSON leaves the cursor unchanged for retry. Removed messages do not
//! create inferred deletion records; preserve old events until deletion semantics are checked.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;
use std::path::Path;

use super::super::common::{map_zoo_cost, map_zoo_usage, ZooUsage};
use super::ZOO_FORMAT_VERSION;

pub const ZOO_PARSER_VERSION: &str = "zoo-ui-messages-doc2";
/// Whole-file read limit, initially 32 MiB.
pub const ZOO_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Complete 3.86.0 enumeration; Roo's enum cannot substitute for Zoo's.
const DOCUMENTED_SAY_KINDS: &[&str] = &[
    "error",
    "api_req_started",
    "api_req_finished",
    "api_req_retried",
    "api_req_retry_delayed",
    "api_req_rate_limit_wait",
    "api_req_deleted",
    "text",
    "task",
    "image",
    "reasoning",
    "completion_result",
    "user_feedback",
    "user_feedback_diff",
    "command_output",
    "shell_integration_warning",
    "mcp_server_request_started",
    "mcp_server_response",
    "subtask_result",
    "checkpoint_saved",
    "rooignore_error",
    "diff_error",
    "condense_context",
    "condense_context_error",
    "sliding_window_truncation",
    "codebase_search_result",
    "user_edit_todos",
    "too_many_tools_warning",
    "tool",
];
const DOCUMENTED_ASK_KINDS: &[&str] = &[
    "followup",
    "command",
    "command_output",
    "completion_result",
    "tool",
    "api_req_failed",
    "resume_task",
    "resume_completed_task",
    "mistake_limit_reached",
    "use_mcp_server",
    "auto_approval_max_req_reached",
];
/// Documented text JSON keys from request examples and ParsedApiReqStartedTextType.
const DOCUMENTED_TEXT_KEYS: &[&str] = &[
    "request",
    "tokensIn",
    "tokensOut",
    "cacheWrites",
    "cacheReads",
    "cost",
    "apiProtocol",
    "cancelReason",
    "streamingFailedMessage",
];

/// Persist once-per-file diagnostics and format selection; reset diagnostic flags on rescan.
/// No verified Zoo deleted_api_reqs removal flow establishes a deletion comparison baseline.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ZooParseContext {
    #[serde(default)]
    parser_policy_version: Option<String>,
    #[serde(default)]
    without_numbers_reported: bool,
    #[serde(default)]
    unpaired_finished_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    /// Format selection is fixed to the documented KnownVersion entry;
    /// it does not identify every record's native client release.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// Whole-file cursor stores consumed bytes with line_number=1.
/// The unchanged-file shortcut compares probe.len with cursor.offset.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    line_number: u64,
}

/// Reset once-per-file diagnostic flags when rescanning so new reads can report them.
fn restore_context(stored: &StoredScanState, rescan: bool) -> ZooParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ZooParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan || ctx.parser_policy_version.as_deref() != Some(ZOO_PARSER_VERSION) {
        ctx.without_numbers_reported = false;
        ctx.unpaired_finished_reported = false;
        ctx.unmapped_keys_reported = false;
    }
    ctx.parser_policy_version = Some(ZOO_PARSER_VERSION.into());
    ctx
}

fn diag(code: &str, field: Option<&str>, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

/// Task directory name tasks/<taskId> supplies session identity.
fn task_id_of(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-task")
        .to_string()
}

/// Parse four optional nonnegative, bounded i64 token fields and separate cost.
/// Nonobject text/invalid token values return None and produce a shape diagnostic.
/// Flag extra undocumented keys separately.
fn parse_usage_text(
    text: &str,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<(ZooUsage, Option<f64>, bool)> {
    let value: serde_json::Value = crate::adapters::run_policy::json_from_str(text).ok()?;
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
    let usage = ZooUsage {
        tokens_in: get("tokensIn")?,
        tokens_out: get("tokensOut")?,
        cache_writes: get("cacheWrites")?,
        cache_reads: get("cacheReads")?,
    };
    let cost = match obj.get("cost") {
        None => None,
        Some(v) => match v.as_f64() {
            Some(c) if c.is_finite() && c >= 0.0 => Some(c),
            _ => {
                diagnostics.push(diag(
                    "cost_shape_deviation",
                    Some("cost"),
                    position,
                    "cost not a finite non-negative number; cost left unknown",
                ));
                None
            }
        },
    };
    let unknown = obj
        .keys()
        .any(|k| !DOCUMENTED_TEXT_KEYS.contains(&k.as_str()));
    Some((usage, cost, unknown))
}

/// Read epoch-millisecond ts; missing, noninteger, or out-of-range values are diagnosed and skipped.
fn message_ts(message: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    let ts = message.get("ts")?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ts)
        .then_some(ts)
}

/// Started request optionally merged with paired finished text.
struct ConsolidatedRequest {
    text: Option<String>,
    ts: Option<i64>,
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    target: &ScanTarget,
    task_id: &str,
    say: &str,
    ts: i64,
    category: CallCategory,
    mapped: crate::adapters::usage_map::MappedUsage,
    cost: Option<crate::domain::CostAmount>,
    now_ms: i64,
) -> EventInput {
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("{task_id}:{say}:{ts}"),
        record_kind: RecordKind::ModelCall,
        schema_version: ZOO_FORMAT_VERSION.to_string(),
        parser_version: ZOO_PARSER_VERSION.to_string(),
        parse_basis: Some(VersionBasis::KnownVersion),
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(task_id.to_string()),
        parent_session_id: None,
        host_application: None,
        agent: "zoo-code".to_string(),
        call_category: category,
        occurred_at_ms: ts,
        observed_at_ms: Some(now_ms),
        source_time: Some(ts.to_string()),
        // Started ts identifies request start; finished may merge later cost into its record.
        // Condense ts identifies message writing, without identifying call start/completion.
        time_basis: if say == "condense_context" {
            TimeBasis::Uncertain
        } else {
            TimeBasis::SourceStart
        },
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: None,
        model_canonical: None,
        model_attribution: ModelAttribution::Unknown,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost,
    }
}

/// Scan one task's ui_messages.json, dispatched by ZooAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = restore_context(stored, target.rescan);
    // No record version is available; KnownVersion refers to the documented format.
    context.version_basis = Some(VersionBasis::KnownVersion);
    let task_id = task_id_of(&target.path);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let cursor_at = |offset: u64| -> Result<serde_json::Value, CoreError> {
        Ok(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset,
            line_number: 1,
        })?)
    };
    // Oversized files remain limited with their cursor unchanged for retry.
    if target.probe.len > ZOO_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            None,
            "document",
            "ui_messages.json exceeds the 32 MiB cap; cursor held at start for controlled retry",
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
        .take(ZOO_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > ZOO_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            None,
            "document",
            "ui_messages.json grew past the 32 MiB cap during read; cursor held at start",
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
    // Partial JSON leaves the cursor unchanged; retry on the next scan.
    let document: serde_json::Value =
        match crate::adapters::run_policy::json_from_slice(super::super::strip_bom(&bytes)) {
            Ok(v) => v,
            Err(_) => {
                diagnostics.push(diag(
                    "ui_messages_unparseable",
                    None,
                    "document",
                    "ui_messages.json does not parse (mid-write or corrupt); cursor held for retry",
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
    let Some(messages) = document.as_array() else {
        diagnostics.push(diag(
            "session_schema_deviation",
            None,
            "document",
            "top-level value is not a JSON array of messages",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 1,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };

    // First pass: upstream LIFO request pairing.
    // Push started into results and the open stack; finished pops the latest start
    // and overrides matching text fields. Discard unmatched finished records.
    let mut requests: Vec<ConsolidatedRequest> = Vec::new();
    let mut condenses: Vec<(Option<i64>, Option<f64>)> = Vec::new();
    let mut open_started: Vec<usize> = Vec::new();
    let mut records_seen: u64 = 0;
    for (index, message) in messages.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let position = format!("messages[{index}]");
        let fail_closed = |diagnostics: &mut Vec<DiagnosticInput>,
                           code: &'static str,
                           detail: String|
         -> ScanOutcome {
            diagnostics.push(diag(code, None, &position, &detail));
            ScanOutcome {
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
            }
        };
        let Some(message_obj) = message.as_object() else {
            let outcome = fail_closed(
                &mut diagnostics,
                "session_schema_deviation",
                format!("{position} is not an object"),
            );
            return Ok(outcome);
        };
        let record_type = message_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if record_type == "ask" {
            let kind = message_obj
                .get("ask")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !DOCUMENTED_ASK_KINDS.contains(&kind) {
                return Ok(fail_closed(
                    &mut diagnostics,
                    "undocumented_ask_kind",
                    format!("ask kind {kind:?} not in documented set"),
                ));
            }
            continue;
        }
        if record_type != "say" {
            let outcome = fail_closed(
                &mut diagnostics,
                "undocumented_record_type",
                format!("record type {record_type:?} not in documented set (ask, say)"),
            );
            return Ok(outcome);
        }
        let say_kind = message_obj
            .get("say")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if !DOCUMENTED_SAY_KINDS.contains(&say_kind) {
            let outcome = fail_closed(
                &mut diagnostics,
                "undocumented_say_kind",
                format!("say kind {say_kind:?} not in documented set"),
            );
            return Ok(outcome);
        }
        match say_kind {
            "api_req_started" => {
                requests.push(ConsolidatedRequest {
                    text: message_obj
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(str::to_string),
                    ts: message_ts(message_obj),
                });
                open_started.push(requests.len() - 1);
            }
            "api_req_finished" => {
                match open_started.pop() {
                    Some(start_index) => {
                        let started = &mut requests[start_index];
                        let parse_obj =
                            |text: Option<&str>| -> serde_json::Map<String, serde_json::Value> {
                                text.and_then(|t| {
                                    crate::adapters::run_policy::json_from_str::<serde_json::Value>(
                                        t,
                                    )
                                    .ok()
                                })
                                .and_then(|v| v.as_object().cloned())
                                .unwrap_or_default()
                            };
                        let mut merged = parse_obj(started.text.as_deref());
                        for (key, value) in
                            parse_obj(message_obj.get("text").and_then(|t| t.as_str()))
                        {
                            merged.insert(key, value);
                        }
                        started.text = Some(serde_json::to_string(&merged)?);
                    }
                    None => {
                        // Unmatched finished records produce no usage event, matching the fixed source.
                        if !context.unpaired_finished_reported {
                            context.unpaired_finished_reported = true;
                            diagnostics.push(diag(
                                "unpaired_finished_dropped",
                                Some("say"),
                                &position,
                                "api_req_finished without open api_req_started; dropped per consolidateApiRequests",
                            ));
                        }
                    }
                }
            }
            "condense_context" => {
                // contextCondense.cost contributes to cost, without token fields.
                let cost = message_obj
                    .get("contextCondense")
                    .and_then(|c| c.get("cost"))
                    .and_then(|c| c.as_f64())
                    .filter(|c| c.is_finite() && *c >= 0.0);
                condenses.push((message_ts(message_obj), cost));
            }
            _ => {} // Known ordinary messages, removed-request memos, and subtask summaries add no usage.
        }
    }

    // Second pass: upstream consolidated token-usage rules.
    for request in &requests {
        crate::adapters::run_policy::check()?;
        let Some(text) = request.text.as_deref() else {
            // Usage records without text report no usable usage and produce no event.
            continue;
        };
        let position = format!("{}:api_req_started", task_id);
        let Some((usage, cost, unknown_keys)) = parse_usage_text(text, &position, &mut diagnostics)
        else {
            diagnostics.push(diag(
                "usage_shape_deviation",
                Some("text"),
                &position,
                "usage text not a JSON object or a value is negative/non-integer/out of range; record skipped",
            ));
            continue;
        };
        if unknown_keys && !context.unmapped_keys_reported {
            context.unmapped_keys_reported = true;
            diagnostics.push(diag(
                "unmapped_usage_keys",
                Some("text"),
                &position,
                "usage text carries keys beyond the documented set; mapped fields kept",
            ));
        }
        let Some(ts) = request.ts else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("ts"),
                &position,
                "message ts missing/implausible; record skipped",
            ));
            continue;
        };
        if usage.is_empty() {
            // A started record without token values, including an unmatched incomplete request,
            // produces one diagnostic and no event; cost alone does not add request usage.
            if !context.without_numbers_reported {
                context.without_numbers_reported = true;
                diagnostics.push(diag(
                    "usage_carrier_without_numbers",
                    Some("text"),
                    &position,
                    "usage carrier without token numbers (unfinished request?); no event",
                ));
            }
            continue;
        }
        let mapped = map_zoo_usage(&usage);
        events.push(build_event(
            target,
            &task_id,
            "api_req_started",
            ts,
            CallCategory::Primary,
            mapped,
            map_zoo_cost(cost),
            now_ms,
        ));
    }
    for (ts, cost) in &condenses {
        crate::adapters::run_policy::check()?;
        let position = format!("{}:condense_context", task_id);
        let Some(ts) = ts else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("ts"),
                &position,
                "condense_context ts missing/implausible; record skipped",
            ));
            continue;
        };
        // Condense identifies an auxiliary call with unknown tokens;
        // map positive cost as an estimate when available.
        events.push(build_event(
            target,
            &task_id,
            "condense_context",
            *ts,
            CallCategory::Auxiliary,
            crate::adapters::usage_map::finish(
                crate::domain::TokenUsage::default(),
                crate::domain::TokenQuality::default(),
                Vec::new(),
            ),
            map_zoo_cost(*cost),
            now_ms,
        ));
    }

    let health_degraded = diagnostics
        .iter()
        .any(|d| d.code == "usage_shape_deviation");
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(cursor_at(consumed)?),
        parse_context: Some(serde_json::to_value(&context)?),
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
    fn parse_usage_text_optional_fields_and_protocol_key() {
        let mut diags = Vec::new();
        let (usage, cost, unknown) = parse_usage_text(
            r#"{"request":"syn","tokensIn":100,"tokensOut":20,"cacheWrites":10,"cacheReads":40,"cost":0.005,"apiProtocol":"anthropic"}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.tokens_in, Some(100));
        assert_eq!(usage.cache_reads, Some(40));
        assert_eq!(cost, Some(0.005));
        assert!(!unknown, "apiProtocol 在固定源码类型内");

        let (usage, cost, _) = parse_usage_text(
            r#"{"tokensIn":10,"tokensOut":2}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.cache_writes, None, "缺失保持未知");
        assert_eq!(cost, None);

        assert!(parse_usage_text("not json", "messages[0]", &mut diags).is_none());
        assert!(
            parse_usage_text(r#"{"tokensIn":-1}"#, "messages[0]", &mut diags).is_none(),
            "负值拒绝"
        );
        let extra =
            parse_usage_text(r#"{"tokensIn":1,"surprise":2}"#, "messages[0]", &mut diags).unwrap();
        assert!(extra.2, "未文档化键报告，已映射字段保留");
    }

    #[test]
    fn task_id_from_path() {
        let p = Path::new("/home/u/.vscode-mock/global-storage/tasks/syn-zoo-1/ui_messages.json");
        assert_eq!(task_id_of(p), "syn-zoo-1".to_string());
    }
}
