//! pi session JSONL parser: session_v3, format version 3.
//!
//! Fixed reference: pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e, inspected read-only.
//! Native acceptance uses local samples; synthetic datasets are identified separately.
//! - packages/ai/src/types.ts defines input/output/cacheRead/cacheWrite, optional
//!   cacheWrite1h as a write subset, reasoning as an output subset, totalTokens, and cost.
//!   Referenced providers normalize input to uncached; totalTokens sums the four independent buckets.
//! - session-manager.ts entries have type/id/parentId/timestamp and a version=3
//!   session header with id/optional parentSession. Usage can appear in four entry types:
//!   assistant message model_call with optional responseId;
//!   standalone usage such as cache_warm with its own provider/model;
//!   compaction usage and branch_summary usage, whose model ownership is tracked
//!   from preceding model_change records. toolResult usage is separate tool consumption,
//!   mapped as auxiliary usage without adding it to the main context.
//!   fork/forkFrom copies non-header entries verbatim, keeping IDs, parents, and timestamps.
//!   The new header records parentSession; inherited entries do not represent new calls.
//!   A type/id/parentId/timestamp key deduplicates copies within the instance.
//! - agent-session.ts getSessionStats sums all file entries, including compacted
//!   or discarded branches; this adapter likewise retains their observed usage.
//! - config.ts resolves PI_CODING_AGENT_DIR or the direct PI_CODING_AGENT_SESSION_DIR;
//!   default sessions are ~/.pi/agent/sessions.
//!
//! V30 version rules: super::select reads the session header through the same registry
//! used by detection. Unregistered numeric versions attempt the latest compatible parser
//! with parse_basis markers; checked incompatible versions 1/2 or missing version
//! are skipped with diagnostics.
//!
//! V30 moved adapters/pi.rs without changing checked behavior.
//! Shared pi/omp usage parsing and event construction live here and are reexported by mod.rs.

use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{map_pi_family, PiFamilyUsage};

pub const PI_PARSER_VERSION: &str = "pi-session-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Persist session identity, model state, version selection, and unknown entry types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PiParseContext {
    session_id: Option<String>,
    parent_session: Option<String>,
    header_version: Option<i64>,
    model: Option<String>,
    model_provider: Option<String>,
    #[serde(default)]
    unknown_types: Vec<String>,
    /// Format basis known_version/latest_fallback; older contexts default to None.
    /// The V30 directory move does not recreate sources or reset cursors.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> PiParseContext {
    if rescan {
        return PiParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<PiParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// Parse shared usage; absent required values, invalid types, negatives, or excessive tokens return None.
/// Missing reasoning/reasoningTokens stays None rather than becoming zero.
pub(crate) fn parse_usage(value: &serde_json::Value) -> Option<PiFamilyUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    let reasoning = obj
        .get("reasoning")
        .or_else(|| obj.get("reasoningTokens"))
        .and_then(|v| v.as_i64())
        .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v));
    Some(PiFamilyUsage {
        input: get("input")?,
        output: get("output")?,
        cache_read: get("cacheRead")?,
        cache_write: get("cacheWrite")?,
        total_tokens: get("totalTokens")?,
        reasoning,
    })
}

/// Map usage.cost.total in USD as an Agent rate estimate rather than a supplier bill.
/// Zero cannot identify a known rate; nonfinite/negative/overflow values produce diagnostics and None.
pub(crate) fn map_cost(
    usage: &serde_json::Value,
    price_version: Option<String>,
    diagnostics: &mut Vec<DiagnosticInput>,
    line: u64,
) -> Option<CostAmount> {
    let total = usage.get("cost")?.get("total")?.as_f64()?;
    if !total.is_finite() || total < 0.0 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost.total"),
            line,
            "usage.cost.total not a finite non-negative number; cost left unknown",
        ));
        return None;
    }
    if total == 0.0 {
        return None;
    }
    let micros = (total * 1_000_000.0).round();
    if micros > i64::MAX as f64 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost.total"),
            line,
            "usage.cost.total overflows micro-unit i64; cost left unknown",
        ));
        return None;
    }
    Some(CostAmount {
        amount_minor: micros as i64,
        currency: "USD".to_string(),
        kind: CostKind::Estimated,
        price_version,
        billing_scope: None,
    })
}

pub(crate) fn parse_entry_ts(value: &serde_json::Value) -> Option<(i64, String)> {
    let raw = value.get("timestamp")?.as_str()?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

pub(crate) fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

pub(crate) fn diag(code: &str, field: Option<&str>, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(format!("line {line}")),
        message: message.to_string(),
    }
}

/// Fork copies preserve type/id/parentId/timestamp identity.
/// Stable instance keys use - for absent parentId and ns for the pi/omp adapter namespace.
pub(crate) fn family_entry_key(ns: &str, prefix: &str, entry: &serde_json::Value) -> String {
    let id = json_str(entry, "id").unwrap_or("noid");
    let parent = json_str(entry, "parentId").unwrap_or("-");
    let ts = json_str(entry, "timestamp").unwrap_or("notime");
    format!("{ns}:{prefix}:{id}:{parent}:{ts}")
}

/// Entry key in the pi namespace.
fn entry_key(prefix: &str, entry: &serde_json::Value) -> String {
    family_entry_key("pi", prefix, entry)
}

pub(crate) struct UsageEventBase<'a> {
    pub key: String,
    pub category: CallCategory,
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub attribution: ModelAttribution,
    pub origin_call_id: Option<&'a str>,
    pub error_status: Option<String>,
}

/// Shared pi/omp event constructor; absent usage leaves token fields unknown.
/// Assistant records still count an observed call without filling unknown tokens with zero.
/// Only omp assistant records define optional duration/ttft fields in the referenced types.
/// Other entries and pi leave those fields None; parse_basis carries the selected version reference.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_pi_family_event(
    target: &ScanTarget,
    agent: &str,
    parser_version: &str,
    session_id: Option<&str>,
    parent_session_id: Option<&str>,
    header_version: Option<i64>,
    parse_basis: Option<VersionBasis>,
    base: &UsageEventBase<'_>,
    duration_ms: Option<i64>,
    ttft_ms: Option<i64>,
    usage: Option<(&PiFamilyUsage, &serde_json::Value)>,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    let (mapped, cost) = match usage {
        Some((usage, usage_json)) => {
            let mapped = map_pi_family(usage);
            for contradiction in &mapped.diagnostics {
                diagnostics.push(diag(
                    contradiction.code,
                    Some(contradiction.field),
                    line,
                    &contradiction.detail,
                ));
            }
            let cost = map_cost(usage_json, None, diagnostics, line);
            (mapped, cost)
        }
        None => (
            crate::adapters::usage_map::MappedUsage {
                usage: crate::domain::TokenUsage::default(),
                quality: crate::domain::TokenQuality::default(),
                diagnostics: Vec::new(),
            },
            None,
        ),
    };
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: base.key.clone(),
        record_kind: RecordKind::ModelCall,
        schema_version: header_version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: parser_version.to_string(),
        parse_basis,
        origin_call_id: base.origin_call_id.map(str::to_string),
        attempt_id: None,
        session_id: session_id.map(str::to_string),
        parent_session_id: parent_session_id.map(str::to_string),
        host_application: None,
        agent: agent.to_string(),
        call_category: base.category,
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(source_time.to_string()),
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: base.provider.map(str::to_string),
        model_raw: base.model.map(str::to_string),
        model_canonical: None,
        model_attribution: base.attribution,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: base.error_status.clone(),
        duration_ms,
        ttft_ms,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost,
    }
}

/// pi event construction has no duration/ttft fields in the referenced types.
/// See build_pi_family_event; parse_basis comes from persisted format selection.
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    context: &PiParseContext,
    base: &UsageEventBase<'_>,
    usage: Option<(&PiFamilyUsage, &serde_json::Value)>,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    build_pi_family_event(
        target,
        "pi",
        PI_PARSER_VERSION,
        context.session_id.as_deref(),
        context.parent_session.as_deref(),
        context.header_version,
        context.version_basis,
        base,
        None,
        None,
        usage,
        occurred_ms,
        source_time,
        line,
        now_ms,
        diagnostics,
    )
}

/// Scan pi session JSONL, dispatched by PiAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
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
        let Ok(entry) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&raw.text)
        else {
            diagnostics.push(diag(
                "bad_json_line",
                None,
                raw.number,
                "line is not valid JSON; isolated, content not stored",
            ));
            continue;
        };
        let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match entry_type {
            "session" => {
                if context.session_id.is_some() {
                    diagnostics.push(diag(
                        "unexpected_session_header",
                        Some("type"),
                        raw.number,
                        "second session header in one file; first kept",
                    ));
                    continue;
                }
                // Detection/scanning share V30 version selection. KnownVersion and LatestFallback
                // both continue with marked events when structure is valid.
                // Checked incompatible versions 1/2 or absent version are skipped with diagnostics.
                let header_version = entry.get("version").and_then(|v| v.as_i64());
                match super::select(header_version) {
                    Ok(selection) => {
                        context.version_basis = Some(selection.basis);
                        context.header_version = header_version;
                    }
                    Err(reason) => {
                        diagnostics.push(diag(
                            "unsupported_version",
                            Some("version"),
                            raw.number,
                            &format!("session header version incompatible; fail closed: {reason}"),
                        ));
                        continue;
                    }
                }
                context.session_id = json_str(&entry, "id").map(str::to_string);
                context.parent_session = json_str(&entry, "parentSession").map(str::to_string);
                if context.session_id.is_none() {
                    diagnostics.push(diag(
                        "missing_session_id",
                        Some("id"),
                        raw.number,
                        "session header without id; events fall back to no session identity",
                    ));
                }
            }
            "model_change" => {
                if let Some(model) = json_str(&entry, "modelId") {
                    context.model = Some(model.to_string());
                }
                if let Some(provider) = json_str(&entry, "provider") {
                    context.model_provider = Some(provider.to_string());
                }
            }
            "message" => {
                let message = entry
                    .get("message")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let role = message.get("role").and_then(|r| r.as_str()).unwrap_or("");
                match role {
                    "assistant" => {
                        let usage_json = message.get("usage").cloned();
                        let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "entry timestamp missing or unparseable; record skipped",
                            ));
                            continue;
                        };
                        let stop_reason = message
                            .get("stopReason")
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        let error_status = match stop_reason {
                            "error" => Some("error".to_string()),
                            "aborted" => Some("aborted".to_string()),
                            _ => None,
                        };
                        let base = UsageEventBase {
                            key: entry_key("message", &entry),
                            category: CallCategory::Primary,
                            provider: json_str(&message, "provider"),
                            model: json_str(&message, "model"),
                            attribution: ModelAttribution::RequestField,
                            origin_call_id: json_str(&message, "responseId"),
                            error_status,
                        };
                        match usage_json.as_ref().and_then(parse_usage) {
                            Some(usage) => {
                                events.push(build_usage_event(
                                    target,
                                    &context,
                                    &base,
                                    Some((&usage, usage_json.as_ref().unwrap())),
                                    occurred_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    &mut diagnostics,
                                ));
                            }
                            None => {
                                // Missing or invalid assistant usage still leaves an observed call:
                                // count the call with unknown tokens instead of zeros.
                                diagnostics.push(diag(
                                    "usage_shape_deviation",
                                    Some("message.usage"),
                                    raw.number,
                                    "assistant message without complete usage; call counted, tokens unknown",
                                ));
                                events.push(build_usage_event(
                                    target,
                                    &context,
                                    &base,
                                    None,
                                    occurred_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    &mut diagnostics,
                                ));
                            }
                        }
                    }
                    "toolResult" => {
                        // Tool-result usage is separate consumption mapped to auxiliary calls.
                        let Some(usage_json) = message.get("usage").cloned() else {
                            continue;
                        };
                        let Some(usage) = parse_usage(&usage_json) else {
                            diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("message.usage"),
                                raw.number,
                                "toolResult usage missing required numeric fields; skipped",
                            ));
                            continue;
                        };
                        let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "entry timestamp missing or unparseable; record skipped",
                            ));
                            continue;
                        };
                        let base = UsageEventBase {
                            key: entry_key("toolresult", &entry),
                            category: CallCategory::Auxiliary,
                            provider: None,
                            model: None,
                            attribution: ModelAttribution::Unknown,
                            origin_call_id: None,
                            error_status: None,
                        };
                        events.push(build_usage_event(
                            target,
                            &context,
                            &base,
                            Some((&usage, &usage_json)),
                            occurred_ms,
                            &source_time,
                            raw.number,
                            now_ms,
                            &mut diagnostics,
                        ));
                    }
                    _ => {}
                }
            }
            "usage" => {
                // Standalone usage such as cache_warm is auxiliary and carries its own provider/model.
                let usage_json = entry
                    .get("usage")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let Some(usage) = parse_usage(&usage_json) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage entry missing required numeric fields; skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let base = UsageEventBase {
                    key: entry_key("usage", &entry),
                    category: CallCategory::Auxiliary,
                    provider: json_str(&entry, "provider"),
                    model: json_str(&entry, "model"),
                    attribution: ModelAttribution::RequestField,
                    origin_call_id: None,
                    error_status: None,
                };
                events.push(build_usage_event(
                    target,
                    &context,
                    &base,
                    Some((&usage, &usage_json)),
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    &mut diagnostics,
                ));
            }
            "compaction" | "branch_summary" => {
                // Optional compaction/branch_summary usage tracks preceding model_change records;
                // leave the model unknown when ownership cannot be established.
                let Some(usage_json) = entry.get("usage").cloned() else {
                    continue;
                };
                let Some(usage) = parse_usage(&usage_json) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "compaction/branch_summary usage missing required numeric fields; skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let (attribution, model, provider) = match context.model.clone() {
                    Some(model) => (
                        ModelAttribution::StructuredChange,
                        Some(model),
                        context.model_provider.clone(),
                    ),
                    None => (ModelAttribution::Unknown, None, None),
                };
                let base = UsageEventBase {
                    key: entry_key(entry_type, &entry),
                    category: CallCategory::Auxiliary,
                    provider: provider.as_deref(),
                    model: model.as_deref(),
                    attribution,
                    origin_call_id: None,
                    error_status: None,
                };
                events.push(build_usage_event(
                    target,
                    &context,
                    &base,
                    Some((&usage, &usage_json)),
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    &mut diagnostics,
                ));
            }
            "thinking_level_change"
            | "custom"
            | "label"
            | "session_info"
            | "custom_message"
            | "context_edit" => {}
            other => {
                if !context.unknown_types.iter().any(|t| t == other) {
                    context.unknown_types.push(other.to_string());
                    diagnostics.push(diag(
                        "unknown_record_type",
                        Some("type"),
                        raw.number,
                        "record type not mapped by this parser version; ignored",
                    ));
                }
            }
        }
    }
    let status = match &outcome.stop {
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
    let degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line" | "usage_shape_deviation" | "line_too_long"
            )
        });
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: outcome.lines.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
        health: if degraded {
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
    fn parse_usage_requires_five_fields_and_keeps_reasoning_absent() {
        let v = serde_json::json!({
            "input": 100,
            "output": 50,
            "cacheRead": 800,
            "cacheWrite": 100,
            "totalTokens": 1050
        });
        let usage = parse_usage(&v).unwrap();
        assert_eq!(usage.total_tokens, 1050);
        assert_eq!(
            usage.reasoning, None,
            "absent reasoning stays unknown, not zero"
        );
        let with_reasoning = serde_json::json!({
            "input": 100,
            "output": 50,
            "cacheRead": 800,
            "cacheWrite": 100,
            "totalTokens": 1050,
            "reasoning": 0
        });
        assert_eq!(parse_usage(&with_reasoning).unwrap().reasoning, Some(0));
        let missing = serde_json::json!({"input": 100, "output": 50});
        assert!(parse_usage(&missing).is_none());
        let negative = serde_json::json!({
            "input": -1,
            "output": 0,
            "cacheRead": 0,
            "cacheWrite": 0,
            "totalTokens": 0
        });
        assert!(parse_usage(&negative).is_none());
    }

    #[test]
    fn entry_key_uses_stable_entry_quadruple() {
        let entry = serde_json::json!({
            "type": "message",
            "id": "a1b2c3d4",
            "parentId": null,
            "timestamp": "2026-01-05T15:00:02.000Z"
        });
        assert_eq!(
            entry_key("message", &entry),
            "pi:message:a1b2c3d4:-:2026-01-05T15:00:02.000Z"
        );
    }

    #[test]
    fn cost_maps_only_positive_finite_values() {
        let mut diags = Vec::new();
        let usage = serde_json::json!({"cost": {"total": 0.005}});
        let cost = map_cost(&usage, None, &mut diags, 1).unwrap();
        assert_eq!(cost.amount_minor, 5000);
        assert_eq!(cost.currency, "USD");
        assert_eq!(cost.kind, CostKind::Estimated);
        let zero = serde_json::json!({"cost": {"total": 0.0}});
        assert!(map_cost(&zero, None, &mut diags, 1).is_none());
        assert!(diags.is_empty());
        let negative = serde_json::json!({"cost": {"total": -1.0}});
        assert!(map_cost(&negative, None, &mut diags, 1).is_none());
        assert_eq!(diags.len(), 1);
    }
}
