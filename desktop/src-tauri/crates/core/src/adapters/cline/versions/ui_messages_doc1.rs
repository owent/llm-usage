//! Legacy Cline ui_messages.json parser: ui_messages_doc1, documented ui-messages-doc-1.
//!
//! Fixed reference: dcf8c3c33596e3d561a941202297c564a1cbcd49 (A03).
//! This legacy UI format and its migration remain separate from native 4.1.22 SDK acceptance.
//! - Path: saoudrizwan.claude-dev host globalStorage/tasks/<taskId>/ui_messages.json.
//!   disk.ts ensureTaskDirectoryExists uses getGlobalStorageDir(tasks,taskId)
//!   with GlobalFileNames.uiMessages.
//!   The file is a full JSON array, rewritten after message deletion/merging.
//! - getApiMetrics.ts reads type=say records with kinds api_req_started,
//!   deleted_api_reqs, and subagent_usage; text is a JSON string.
//!   Optional fields: tokensIn/tokensOut/cacheWrites/cacheReads/cost.
//!   Started requests already contain merged finished data; do not count every say as a call.
//!   This reference has no intermediate streaming usage. Compaction tokensBefore/tokensAfter
//!   are SDK context estimates based on chars/4, excluded from consumption.
//! - Epoch-millisecond ts identifies messages in the referenced examples;
//!   array positions move after deletion and do not identify events.
//!
//! Read the full file within 32 MiB, recording consumed bytes for unchanged-file checks.
//! Rewrites/truncation change generation; stable keys deduplicate event updates.
//! Partial JSON leaves the cursor unchanged for the next retry.
//!
//! Legacy deletion moves removed api_req_started usage into deleted_api_reqs aggregates.
//! After a valid rewrite, compare previously tracked keys against the current keys.
//! Emit Corrected/Excluded deletion records for vanished keys to remove prior contributions
//! under ingest's lifecycle selection and prevent counting both detail and aggregate.
//! Restoring identical content to an excluded corrected key does not reactivate it; retain this limit.
//!
//! Reject non-say or undocumented say kinds for this legacy format (V17).
//! Keep the cursor for retry; ask/text kinds lack a checked mapping here.
//! Native SDK support does not establish these older UI record shapes.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;
use std::path::Path;

use super::super::common::{map_cline_usage, ClineUsage};
use super::CLINE_FORMAT_VERSION;

pub const CLINE_PARSER_VERSION: &str = "cline-ui-messages-doc1";
/// Whole-file read limit, initially 32 MiB.
pub const CLINE_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Three documented usage kinds plus compaction context estimates.
const DOCUMENTED_SAY_KINDS: &[&str] = &[
    "api_req_started",
    "deleted_api_reqs",
    "subagent_usage",
    "compaction",
];
const COMPACTION_SAY_KIND: &str = "compaction";
/// Selected usage text keys: tokensIn/tokensOut/cacheWrites/cacheReads/cost.
const USAGE_KEYS: &[&str] = &[
    "request",
    "tokensIn",
    "tokensOut",
    "cacheWrites",
    "cacheReads",
    "cost",
];

/// Persist previously imported keys for deletion comparison across rescans,
/// plus once-per-file diagnostics and format selection.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ClineParseContext {
    /// Keys previously imported from this file; rescans retain them for deletion comparisons.
    #[serde(default)]
    tracked_keys: Vec<String>,
    #[serde(default)]
    compaction_reported: bool,
    #[serde(default)]
    without_numbers_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    /// Format selection known_version/latest_fallback; older contexts default to None.
    /// This documented legacy format uses KnownVersion without identifying a native release.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// Whole-file cursor shares consumed-byte offset and line_number=1 fields.
/// Unchanged-file checks compare probe.len with cursor.offset.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    line_number: u64,
}

/// Rescans reset diagnostic flags but retain tracked_keys
/// as the previous record set, independently of parsing positions.
fn restore_context(stored: &StoredScanState, rescan: bool) -> ClineParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ClineParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan {
        ctx.compaction_reported = false;
        ctx.without_numbers_reported = false;
        ctx.unmapped_keys_reported = false;
    }
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

/// Parse four optional nonnegative, bounded i64 token fields plus cost.
/// Invalid text/tokens return None for a visible diagnostic and skip that record;
/// upstream silently skips parse errors. Extra keys are flagged separately.
fn parse_usage_text(
    text: &str,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<(ClineUsage, Option<f64>, bool)> {
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
    let usage = ClineUsage {
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
    let unknown = obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str()));
    Some((usage, cost, unknown))
}

/// Convert dollar cost to estimated micro-USD; actual billing origin remains unverified.
/// Like pi cost mapping, leave overflow unknown with a diagnostic.
fn map_cost(
    cost: Option<f64>,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<CostAmount> {
    let total = cost?;
    if !total.is_finite() || total < 0.0 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost"),
            position,
            "cost not a finite non-negative number; cost left unknown",
        ));
        return None;
    }
    let micros = total * 1_000_000.0;
    if micros > i64::MAX as f64 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost"),
            position,
            "cost overflows micro-unit i64; cost left unknown",
        ));
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

/// Scan one task's legacy UI messages, dispatched by ClineAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = restore_context(stored, target.rescan);
    // KnownVersion refers to the documented legacy format; no record client version is available.
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
    // Oversized files stay limited with their cursor unchanged for retry.
    if target.probe.len > CLINE_MAX_FILE_BYTES {
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
        .take(CLINE_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CLINE_MAX_FILE_BYTES {
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
    // Partial JSON leaves the cursor unchanged for the next retry.
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
    let mut records_seen: u64 = 0;
    let mut current_keys: Vec<String> = Vec::new();
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
        if record_type != "say" {
            // This checked legacy mapping accepts say only; reject other record types.
            let outcome = fail_closed(
                &mut diagnostics,
                "undocumented_record_type",
                format!("record type {record_type:?} not in documented set (say)"),
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
        if say_kind == COMPACTION_SAY_KIND {
            // Compaction tokensBefore/tokensAfter estimate context size from chars/4
            // for display, without contributing token consumption; diagnose once per file.
            if !context.compaction_reported {
                context.compaction_reported = true;
                diagnostics.push(diag(
                    "compaction_estimate_excluded",
                    Some("say"),
                    &position,
                    "compaction tokensBefore/tokensAfter are SDK estimates; excluded from usage",
                ));
            }
            continue;
        }
        let Some(text) = message_obj.get("text").and_then(|t| t.as_str()) else {
            // A usage record without text produces no event.
            continue;
        };
        let Some((usage, cost, unknown_keys)) = parse_usage_text(text, &position, &mut diagnostics)
        else {
            // Diagnose and skip malformed usage text while continuing other valid records.
            diagnostics.push(diag(
                "usage_shape_deviation",
                Some("text"),
                &position,
                "usage text not a JSON object or a value is negative/non-integer/out of range; record skipped",
            ));
            continue;
        };
        if usage.is_empty() {
            // Started requests can lack usage values, including unfinished requests.
            // Skip those records and continue reading other usage;
            // diagnose once per file. Cost alone does not identify request token usage.
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
        if unknown_keys && !context.unmapped_keys_reported {
            context.unmapped_keys_reported = true;
            diagnostics.push(diag(
                "unmapped_usage_keys",
                Some("text"),
                &position,
                "usage text carries keys beyond the documented five; mapped fields kept",
            ));
        }
        let Some(ts) = message_obj.get("ts").and_then(|t| t.as_i64()) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("ts"),
                &position,
                "message ts missing or not an integer; record skipped",
            ));
            continue;
        };
        if !(crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(&ts) {
            // Reject timestamps outside the configured millisecond range.
            diagnostics.push(diag(
                "timestamp_implausible",
                Some("ts"),
                &position,
                "message ts outside plausible millisecond range; record skipped",
            ));
            continue;
        }
        let is_aggregate = say_kind != "api_req_started";
        let source_record_key = format!("{task_id}:{say_kind}:{ts}");
        current_keys.push(source_record_key.clone());
        let mapped = map_cline_usage(&usage);
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
            schema_version: CLINE_FORMAT_VERSION.to_string(),
            parser_version: CLINE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(task_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "cline".to_string(),
            call_category: if say_kind == "subagent_usage" {
                CallCategory::SubAgent
            } else {
                CallCategory::Primary
            },
            occurred_at_ms: ts,
            observed_at_ms: Some(now_ms),
            source_time: Some(ts.to_string()),
            // Started ts identifies request start; usage may be merged back from finished.
            // Aggregate ts identifies message writing rather than an individual call boundary.
            time_basis: if is_aggregate {
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
            cost: map_cost(cost, &position, &mut diagnostics),
        });
    }
    // Keys missing after a valid rewrite may be represented in deleted_api_reqs;
    // emit Corrected/Excluded records to remove previous contributions and prevent duplicate counts.
    let tracked: std::collections::BTreeSet<String> =
        context.tracked_keys.iter().cloned().collect();
    let current: std::collections::BTreeSet<String> = current_keys.iter().cloned().collect();
    for vanished in tracked.difference(&current) {
        crate::adapters::run_policy::check()?;
        diagnostics.push(diag(
            "source_message_removed",
            None,
            vanished.as_str(),
            "previously ingested key absent after rewrite; tombstoned Corrected/Excluded (deleted-api-req flow)",
        ));
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: vanished.clone(),
            record_kind: RecordKind::ModelCall,
            schema_version: CLINE_FORMAT_VERSION.to_string(),
            parser_version: CLINE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(task_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "cline".to_string(),
            call_category: CallCategory::Unknown,
            occurred_at_ms: now_ms,
            observed_at_ms: Some(now_ms),
            source_time: None,
            time_basis: TimeBasis::ObservedAt,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: None,
            model_canonical: None,
            model_attribution: ModelAttribution::Unknown,
            usage: crate::domain::TokenUsage::default(),
            quality: crate::domain::TokenQuality::default(),
            lifecycle: Lifecycle::Corrected,
            source_revision: None,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Excluded,
            exclusion_reason: Some(
                "source message removed from ui_messages.json (deleted-api-req flow); usage restated via aggregate"
                    .to_string(),
            ),
            cost: None,
        });
    }
    context.tracked_keys = current_keys;
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
    fn parse_usage_text_optional_fields() {
        let mut diags = Vec::new();
        let (usage, cost, unknown) = parse_usage_text(
            r#"{"request":"syn","tokensIn":100,"tokensOut":20,"cacheWrites":10,"cacheReads":40,"cost":0.005}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.tokens_in, Some(100));
        assert_eq!(usage.cache_reads, Some(40));
        assert_eq!(cost, Some(0.005));
        assert!(!unknown);

        let (usage, cost, _) = parse_usage_text(
            r#"{"tokensIn":10,"tokensOut":2}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.cache_writes, None, "missing stays unknown");
        assert_eq!(cost, None);

        assert!(parse_usage_text("not json", "messages[0]", &mut diags).is_none());
        assert!(
            parse_usage_text(r#"{"tokensIn":-1}"#, "messages[0]", &mut diags).is_none(),
            "negative value rejected"
        );
        let extra =
            parse_usage_text(r#"{"tokensIn":1,"surprise":2}"#, "messages[0]", &mut diags).unwrap();
        assert!(extra.2, "unknown keys flagged, mapped fields kept");
    }

    #[test]
    fn map_cost_micro_usd_estimated() {
        let mut diags = Vec::new();
        let cost = map_cost(Some(0.005), "messages[0]", &mut diags).unwrap();
        assert_eq!(cost.amount_minor, 5_000);
        assert_eq!(cost.currency, "USD");
        assert_eq!(cost.kind, CostKind::Estimated);
        assert!(map_cost(None, "messages[0]", &mut diags).is_none());
        assert!(map_cost(Some(-1.0), "messages[0]", &mut diags).is_none());
    }

    #[test]
    fn task_id_from_path() {
        let p = Path::new(
            "/home/u/globalStorage/saoudrizwan.claude-dev/tasks/syn-task-1/ui_messages.json",
        );
        assert_eq!(task_id_of(p), "syn-task-1".to_string());
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        let legacy = serde_json::json!({"tracked_keys": ["k1"]});
        let ctx: ClineParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
        assert_eq!(ctx.tracked_keys, vec!["k1".to_string()]);
    }
}
