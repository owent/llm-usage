//! Command Code v3 tree-session JSONL parser: tree_v3, commandcode-tree-v3.
//!
//! Source reference: official command-code@1.69.0 npm dist/cli.mjs inspected line by line.
//! The public repository contains documentation only; 1.74.3 preparation reached authentication limits.
//! - Path: ~/.commandcode/projects/<slug>/*.jsonl, using @sindresorhus/slugify(cwd).
//!   Upstream prefers HOME over USERPROFILE; different roots require discovery checks.
//!   Exclude session filenames containing .checkpoints., .prompts., or .v2.bak.
//! - Header: type=session, version=3, id, ISO timestamp, cwd, optional parentSession.
//!   Entries carry type, 8-hex id, parentId, and ISO timestamp.
//!   Entry types: message/model_change/effort_change/compaction/branch_summary/
//!   custom/custom_message/label/session_info.
//! - Assistant usage: inputTokens/outputTokens/cacheReadTokens/cacheWriteTokens,
//!   optional cacheWriteTokens1h/costUsd. The source normalizes completed request buckets
//!   with ?? 0; this parser reports present zeros. Missing usage leaves a request unknown.
//!   inputTokens includes cache reads/writes, as shown by max(0,input-cacheR-cacheW)
//!   in the source cost formula. costUsd is a local rate estimate, absent without rates.
//! - buildSessionPath/getTree follows parentId from the last entry to the root.
//!   Rewinding leaves discarded branches in the append-only file; exclude orphan branches.
//!   Compaction retains real calls on the selected path, including pre-compaction entries.
//!   Count all assistant usage entries on that path.
//! - Forks copy root-to-leaf entries without changing IDs or timestamps.
//!   Cross-file key: cmd:<entry id>:<timestamp>; the timestamp reduces collisions
//!   from 32-bit entry IDs but does not establish collision-free identity.
//! - subagentProgressTranslator excludes subagent usage from persisted records;
//!   these files contain main-conversation usage only.
//! - v3 timestamps are ISO strings; v2 millisecond numbers and v1 shapes are not read here.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

use super::COMMANDCODE_FORMAT_VERSION;

pub const COMMANDCODE_PARSER_VERSION: &str = "commandcode-tree-v3";
/// Whole-file read limit for rebuilding the session tree.
pub const CMD_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

const DOCUMENTED_ENTRY_TYPES: &[&str] = &[
    "message",
    "model_change",
    "effort_change",
    "compaction",
    "branch_summary",
    "custom",
    "custom_message",
    "label",
    "session_info",
];

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CmdParseContext {
    #[serde(default)]
    version_basis: Option<VersionBasis>,
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

fn rfc3339_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let s: &str = value?.as_str()?;
    let ts: jiff::Timestamp = s.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn usd_cost(value: Option<&serde_json::Value>) -> Option<CostAmount> {
    let amount = value?.as_f64()?;
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        // Local rate estimate from estimateSessionCostUsd; absent when rates are unavailable.
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

/// Derive uncached input from the source-defined total input and cache subsets.
fn map_cmd_usage(
    input: Option<i64>,
    output: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
) -> MappedUsage {
    let mut contradictions = Vec::new();
    let uncached = match (input, cache_read, cache_write) {
        (Some(total), Some(r), Some(w)) => sub_checked(
            "input_uncached",
            total,
            r.saturating_add(w),
            &mut contradictions,
        ),
        _ => None,
    };
    let total = match (input, output) {
        (Some(i), Some(o)) => i.checked_add(o),
        _ => None,
    };
    let usage = crate::domain::TokenUsage {
        input_uncached: uncached,
        input_cache_read: cache_read,
        input_cache_write: cache_write,
        input_total: input,
        output_total: output,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = crate::domain::TokenQuality {
        input_uncached: if uncached.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        input_cache_read: cache_read
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_cache_write: cache_write
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_total: input
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        output_total: output
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        total_tokens: if total.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        ..Default::default()
    };
    finish(usage, quality, contradictions)
}

struct EntryLite {
    id: String,
    parent_id: Option<String>,
    entry_type: String,
    /// Four assistant usage buckets; Some means the field is present.
    usage_input: Option<i64>,
    usage_output: Option<i64>,
    usage_cache_read: Option<i64>,
    usage_cache_write: Option<i64>,
    cost: Option<f64>,
    model: Option<String>,
    timestamp: Option<i64>,
    is_assistant: bool,
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<CmdParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = CmdParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut diagnostics = Vec::new();
    if target.probe.len > CMD_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "session transcript exceeds the 32 MiB cap; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    // Rebuild the parentId graph from the whole file; unchanged append-only files can be skipped.
    let mut bytes = Vec::new();
    std::io::Read::take(
        &mut crate::adapters::run_policy::checked_file(&target.path)?,
        CMD_MAX_FILE_BYTES + 1,
    )
    .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CMD_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "session transcript grew past the cap during read; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut entries: Vec<EntryLite> = Vec::new();
    let mut session_id = String::new();
    let mut records_seen: u64 = 0;
    for (index, line) in text.lines().enumerate() {
        crate::adapters::run_policy::check()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        records_seen += 1;
        let Ok(value) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(line)
        else {
            // Match upstream safeParseRecord: count malformed lines as corrupted and continue.
            diagnostics.push(diag(
                "corrupted_line",
                &format!("line:{}", index + 1),
                "line does not parse (crash truncation?); counted as corrupted, skipped",
            ));
            continue;
        };
        let entry_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if index == 0 || entry_type == "session" {
            session_id = value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            continue;
        }
        if !DOCUMENTED_ENTRY_TYPES.contains(&entry_type) {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "undocumented_entry_type",
                    &format!("line:{}", index + 1),
                    &format!("entry type {entry_type:?} not in the documented v3 set"),
                )],
                lines_read: records_seen,
                records_seen,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            });
        }
        let message = value.get("message").and_then(|v| v.as_object());
        let is_assistant =
            message.and_then(|m| m.get("role")).and_then(|r| r.as_str()) == Some("assistant");
        let usage = value.get("usage").and_then(|v| v.as_object());
        let bucket = |key: &str| -> Option<Option<i64>> {
            match usage.and_then(|u| u.get(key)) {
                None => Some(None),
                Some(v) => {
                    let n = v.as_i64()?;
                    Some((0..=MAX_REASONABLE_TOKEN).contains(&n).then_some(n))
                }
            }
        };
        let (u_in, u_out, u_cr, u_cw) = if is_assistant && usage.is_some() {
            match (
                bucket("inputTokens"),
                bucket("outputTokens"),
                bucket("cacheReadTokens"),
                bucket("cacheWriteTokens"),
            ) {
                (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
                _ => {
                    diagnostics.push(diag(
                        "token_shape_deviation",
                        &format!("line:{}", index + 1),
                        "usage bucket carries an out-of-range value; entry skipped",
                    ));
                    (None, None, None, None)
                }
            }
        } else {
            (None, None, None, None)
        };
        entries.push(EntryLite {
            id: value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            parent_id: value
                .get("parentId")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            entry_type: entry_type.to_string(),
            usage_input: u_in,
            usage_output: u_out,
            usage_cache_read: u_cr,
            usage_cache_write: u_cw,
            cost: usage
                .and_then(|u| u.get("costUsd"))
                .and_then(|v| v.as_f64())
                .filter(|c| c.is_finite() && *c >= 0.0),
            model: value
                .get("model")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            timestamp: rfc3339_ms(value.get("timestamp")),
            is_assistant,
        });
    }
    // Follow parentId from the last entry, matching upstream buildSessionPath.
    let by_id: std::collections::BTreeMap<&str, usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.id.is_empty())
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut on_path = vec![false; entries.len()];
    let mut cursor: Option<usize> = entries.len().checked_sub(1);
    let mut current_model: Option<String> = None;
    // Mark the last-entry-to-root path, stopping at cycles; model changes are applied
    // when the next loop walks the selected entries in file order.
    while let Some(index) = cursor {
        crate::adapters::run_policy::check()?;
        if on_path[index] {
            break; // Stop at a parent cycle.
        }
        on_path[index] = true;
        let entry = &entries[index];
        cursor = entry
            .parent_id
            .as_deref()
            .and_then(|p| by_id.get(p).copied());
    }
    // Emit selected-path assistant usage in file order while tracking model changes.
    let mut events = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        if entry.entry_type == "model_change" && on_path[index] {
            if let Some(model) = &entry.model {
                current_model = Some(model.clone());
            }
        }
        if !entry.is_assistant || !on_path[index] {
            continue;
        }
        // No usage buckets means unknown usage for an incomplete/interrupted request; skip the event.
        if entry.usage_input.is_none()
            && entry.usage_output.is_none()
            && entry.usage_cache_read.is_none()
            && entry.usage_cache_write.is_none()
        {
            continue;
        }
        let Some(occurred_ms) = entry.timestamp else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("entry:{}", entry.id),
                "entry timestamp missing/implausible; skipped",
            ));
            continue;
        };
        if entry.id.is_empty() {
            continue;
        }
        let mapped = map_cmd_usage(
            entry.usage_input,
            entry.usage_output,
            entry.usage_cache_read,
            entry.usage_cache_write,
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            // Forks retain ID and timestamp; use both in the cross-file duplicate key.
            source_record_key: format!("cmd:{}:{}", entry.id, occurred_ms),
            record_kind: RecordKind::ModelCall,
            schema_version: COMMANDCODE_FORMAT_VERSION.to_string(),
            parser_version: COMMANDCODE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(session_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "command-code".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: entry.model.clone().or_else(|| current_model.clone()),
            model_canonical: None,
            model_attribution: ModelAttribution::StructuredChange,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: usd_cost(entry.cost.map(serde_json::Value::from).as_ref()),
        });
    }
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_input_includes_cache() {
        let mapped = map_cmd_usage(Some(100), Some(20), Some(30), Some(10));
        assert_eq!(mapped.usage.input_total, Some(100));
        assert_eq!(mapped.usage.input_uncached, Some(60));
        assert_eq!(mapped.usage.total_tokens, Some(120));
    }

    #[test]
    fn cost_optional() {
        assert!(usd_cost(Some(&serde_json::json!(0.25))).is_some());
        assert!(usd_cost(None).is_none());
    }
}
