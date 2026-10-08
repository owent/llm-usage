//! Shared Kimi wire JSONL helpers for Kimi Code A12 and Kimi Work A13 (M4).
//!
//! The products share a wire protocol family but retain separate identities, as specified
//! in adapters.md: separate data roots, instances, and log revisions. This root-level module,
//! like jsonl.rs, shares only helper behavior checked against native data from both products.
//!
//! - First-line metadata detection: protocol_version is a string in observed 1.5/1.4 files.
//! - usage.record has four exclusive camelCase fields: inputOther, output, inputCacheRead,
//!   inputCacheCreation; no native total, as documented in the M0 local-read results.
//! - All 82 locally read wire.jsonl files use epoch milliseconds (1.78e12–1.79e12).
//!   No seconds samples exist here; diagnose and skip implausible times without guessing a ×1000 conversion.
//! - step.end event.usage within context.append_loop_event repeats usage.record field values.
//!   Count usage.record alone and use repeats only for reconciliation, preventing double-counting.
//!   Observed repeats: 958/960 for 1.5 and 1329/1329 for 1.4; interrupted steps can lack repeats.
//!   The observed repeats are a subset; excess repeated totals signal a format difference.
//! - Main-wire subagent.completed.usage is a cumulative snapshot of child-wire calls
//!   through completed.time; local 2026-09-25 checks matched fields individually.
//!   Child wire records count independently; completed snapshots produce no events.
//! - Observed usageScope: turn identifies main-loop LLM calls; session identifies
//!   auxiliary calls. Local session records occurred within full_compaction.begin…complete,
//!   corresponding to llm.request kind=compaction, and count as auxiliary usage.
//! - usage.record has no uuid/messageId. Those IDs appear only in repeated
//!   step.end records, without a key linking them to usage.record. Event keys use
//!   {session directory}:{agent}:{time}:{same-millisecond sequence}. Kimi Work had
//!   two pairs of child/main usage.record completions at the same millisecond across files.
//!   Keep session/agent identity; same-file duplicate times get a deterministic sequence and diagnostic.
//!
//! Product differences checked separately in tests/fixtures/kimi-code and kimi-work:
//! - Kimi Code protocol 1.5: usage.record includes agentId; sessions/<wd>/session_<uuid>/;
//!   model is alias/model. The observed desktop product version was 1.0.3.
//! - Kimi Work protocol 1.4: usage.record lacks agentId; use agents/<id>/ directory identity;
//!   sessions/<wd>/<conv-*|ctitle-*>/; model is a bare ID; daimon host
//!   uses state.json createdBy=daimon-kernel-adapter.

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// Minimum plausible epoch milliseconds; skip lower values without assuming seconds or converting them.
const MIN_TIME_MS: i64 = crate::domain::MIN_PLAUSIBLE_MS;
/// Maximum epoch milliseconds in year 2286; observed local values reached only 1.79e12.
const MAX_TIME_MS: i64 = 10_000_000_000_000;

/// Four exclusive native usage.record fields, without a native total (M0 local reads).
/// Shared by Kimi Code/Work; moved from usage_map.rs into the M4 family module.
#[derive(Debug, Clone, Copy)]
pub struct KimiWireUsage {
    pub input_other: i64,
    pub input_cache_read: i64,
    pub input_cache_creation: i64,
    pub output: i64,
}

/// Derive input_total/total_tokens from the four exclusive wire fields.
/// No native total or reasoning field; reasoning remains unknown.
pub fn map_kimi_wire(raw: &KimiWireUsage) -> MappedUsage {
    let input_total = raw
        .input_other
        .checked_add(raw.input_cache_read)
        .and_then(|v| v.checked_add(raw.input_cache_creation));
    let total = input_total.and_then(|i| i.checked_add(raw.output));
    let usage = TokenUsage {
        input_uncached: Some(raw.input_other),
        input_cache_read: Some(raw.input_cache_read),
        input_cache_write: Some(raw.input_cache_creation),
        input_total,
        output_total: Some(raw.output),
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Derived,
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: Q::Derived,
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// Product identity supplied by each product's scan implementation; this module holds no product state.
pub(crate) struct WireProduct {
    /// Event-key namespace: adapter_id kimi-code or kimi-work.
    pub ns: &'static str,
    /// Agent name used in statistics: kimi-code or kimi-work.
    pub agent: &'static str,
    /// Parser version for this product implementation.
    pub parser_version: &'static str,
}

/// Observed non-usage types are ignored; diagnose unlisted types once.
/// Covers the union of observed types across 82 native protocol-1.5/1.4 wire.jsonl files.
pub(crate) const KNOWN_IGNORED_TYPES: &[&str] = &[
    // Lifecycle/control records.
    "agent.message.appended",
    "agent.switched",
    "agent.turn.started",
    "agent.turn.ended",
    "config.update",
    "task.started",
    "task.terminated",
    "task.waitDelivered",
    "turn.prompt",
    "turn.ended",
    "turn.cancel",
    "turn.steer",
    "turn.step.interrupted",
    "turn.step.retrying",
    "prompt.aborted",
    "prompt.completed",
    "prompt.steered",
    "plugin.session_start",
    "runtime.set_binding",
    "profile.bind",
    "permission.set_mode",
    "permission.record_approval_result",
    "swarm_mode.enter",
    "swarm_mode.exit",
    // Child-agent lifecycle; completed snapshots have separate non-event handling.
    "subagent.spawned",
    "subagent.started",
    "subagent.failed",
    "subagent.cancelled",
    // Context/compaction control records.
    "context.append_message",
    "context.apply_compaction",
    "context.undo",
    "context.undone",
    "full_compaction.begin",
    "full_compaction.complete",
    "micro_compaction.apply",
    // Request/tool snapshots; llm.request has no stable key linking it to usage.record, so do not combine them.
    "llm.request",
    "llm.tools_snapshot",
    "mcp.tools_discovered",
    "tools.update_store",
    "tools.set_active_tools",
    "tools.register_user_tool",
    // token_counting describes estimated/cumulative context; exclude it from per-call usage.
    "token_counting.measured",
    "token_counting.rebased",
    "token_counting.truncated",
    "token_counting.turn_recorded",
    // File history and miscellaneous records.
    "file_history.tracked",
    "file_history.checkpoint",
];

/// Shared first-line metadata detection result for detect and scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataHead {
    pub protocol_version: Option<String>,
}

/// First-line result: Pending without a complete line; Metadata with an identified header;
/// NotMetadata with a rejection reason. I/O failures return Err.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadProbe {
    Pending,
    Metadata(MetadataHead),
    NotMetadata(String),
}

/// Bounded first-line read identifying the family's metadata fingerprint.
pub(crate) fn read_metadata_head(path: &std::path::Path) -> Result<HeadProbe, CoreError> {
    let limits = super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: super::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(HeadProbe::Pending);
    };
    let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(HeadProbe::NotMetadata("first line is not JSON".to_string()));
    };
    if line.get("type").and_then(|t| t.as_str()) != Some("metadata") {
        return Ok(HeadProbe::NotMetadata(
            "first record type is not metadata header".to_string(),
        ));
    }
    Ok(HeadProbe::Metadata(MetadataHead {
        // Observed protocol versions 1.4/1.5 are strings; other types become None for registry fallback.
        protocol_version: line
            .get("protocol_version")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }))
}

/// Persist counters, reconciliation totals, and one-time diagnostics across incremental rounds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WireParseContext {
    pub protocol_version: Option<String>,
    pub version_basis: Option<VersionBasis>,
    /// Increment the event-key sequence for same-millisecond repeats within one file; none were locally observed.
    usage_records_seen: u64,
    last_usage_time: Option<i64>,
    dup_in_last_time: u64,
    /// Saturating i64 reconciliation totals; repeated values do not contribute to usage.
    /// Sum only turn-scope records: session-scope compaction has no corresponding repeated usage.
    /// Native 1.4 had 1329 repeats for 1329 turn records and no repeats for 8 session records.
    record_turn_total_sum: i64,
    echo_total_sum: i64,
    subagent_completed: u64,
    #[serde(default)]
    unknown_types: Vec<String>,
    #[serde(default)]
    missing_session_id_reported: bool,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> WireParseContext {
    if rescan {
        return WireParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<WireParseContext>(v.clone()).ok())
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

/// Check plausible epoch milliseconds; diagnose out-of-range times without converting seconds.
fn plausible_time_ms(value: i64) -> bool {
    (MIN_TIME_MS..=MAX_TIME_MS).contains(&value)
}

/// Parse four exclusive usage fields; missing/invalid/negative/excessive values return None for caller diagnostics.
pub(crate) fn parse_wire_usage(value: &serde_json::Value) -> Option<KimiWireUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(KimiWireUsage {
        input_other: get("inputOther")?,
        input_cache_read: get("inputCacheRead")?,
        input_cache_creation: get("inputCacheCreation")?,
        output: get("output")?,
    })
}

fn wire_total(raw: &KimiWireUsage) -> i64 {
    raw.input_other
        .saturating_add(raw.input_cache_read)
        .saturating_add(raw.input_cache_creation)
        .saturating_add(raw.output)
}

/// Derive identity from …/<session>/agents/<agent>/wire.jsonl:
/// agent is the parent directory; session is the parent of the agents directory.
/// Kimi Work 1.4 usage.record has no agentId; obtain identity from directories.
pub(crate) fn identity_from_path(path: &std::path::Path) -> (Option<String>, Option<String>) {
    let agent = path
        .ancestors()
        .nth(1)
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string);
    let session = path
        .ancestors()
        .nth(3)
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string);
    (session, agent)
}

/// Build a shared usage.record event; identity and calculation references are in the module documentation.
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    product: &WireProduct,
    context: &WireParseContext,
    basis: Option<VersionBasis>,
    mapped: MappedUsage,
    model: Option<&str>,
    category: CallCategory,
    session_id: Option<&str>,
    agent_id: Option<&str>,
    occurred_ms: i64,
    now_ms: i64,
) -> EventInput {
    EventInput {
        source_instance_id: target.instance_id.clone(),
        // Stable identity: namespace + session directory + agent + record time + same-millisecond sequence.
        // usage.record lacks uuid/messageId. Keep session/agent because Kimi Work had two pairs
        // of child/main usage.record completions sharing a millisecond across files;
        // time+sequence alone would collide across those files and lose events during conflict handling.
        source_record_key: format!(
            "{}:usage:{}:{}:{}:{}",
            product.ns,
            session_id.unwrap_or("nosession"),
            agent_id.unwrap_or("noagent"),
            occurred_ms,
            context.dup_in_last_time
        ),
        record_kind: RecordKind::ModelCall,
        schema_version: context
            .protocol_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: product.parser_version.to_string(),
        parse_basis: basis,
        origin_call_id: None,
        attempt_id: None,
        session_id: session_id.map(str::to_string),
        parent_session_id: None,
        host_application: None,
        agent: product.agent.to_string(),
        call_category: category,
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(occurred_ms.to_string()),
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: model.map(str::to_string),
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
    }
}

/// Shared incremental scan called by kimi-code and kimi-work version implementations.
pub(crate) fn scan_wire(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
    product: &WireProduct,
    select_version: &dyn Fn(Option<&str>) -> VersionBasis,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let (path_session, path_agent) = identity_from_path(&target.path);

    let outcome = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    for bad in &outcome.bad_lines {
        diagnostics.push(diag(
            bad.code,
            None,
            bad.number,
            "line is not valid UTF-8; isolated, content not stored",
        ));
    }
    for raw in &outcome.lines {
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
            "metadata" => {
                if context.protocol_version.is_some() {
                    diagnostics.push(diag(
                        "unexpected_metadata_header",
                        Some("type"),
                        raw.number,
                        "second metadata header in one file; first kept",
                    ));
                    continue;
                }
                context.protocol_version = line
                    .get("protocol_version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                // Product-supplied version registry shared by detection and scanning.
                context.version_basis = Some(select_version(context.protocol_version.as_deref()));
            }
            "usage.record" => {
                let scope = line
                    .get("usageScope")
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                if !matches!(scope, "turn" | "session") {
                    diagnostics.push(diag(
                        "usage_scope_unknown",
                        Some("usageScope"),
                        raw.number,
                        "usage.record scope outside {turn, session}; skipped (no guess)",
                    ));
                    continue;
                }
                let Some(usage_value) = line.get("usage") else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage.record without usage object; skipped",
                    ));
                    continue;
                };
                let Some(usage) = parse_wire_usage(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage fields missing, negative or out of range; record skipped",
                    ));
                    continue;
                };
                let time = line.get("time").and_then(|t| t.as_i64());
                let Some(time) = time.filter(|t| plausible_time_ms(*t)) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("time"),
                        raw.number,
                        "time outside plausible epoch-ms range; skipped without unit guessing",
                    ));
                    continue;
                };
                // Deterministic same-millisecond sequence remains stable on rescans.
                if context.last_usage_time == Some(time) {
                    context.dup_in_last_time += 1;
                    if context.dup_in_last_time == 1 {
                        diagnostics.push(diag(
                            "usage_time_collision",
                            Some("time"),
                            raw.number,
                            "two usage.records share one millisecond; sequence suffix appended",
                        ));
                    }
                } else {
                    context.last_usage_time = Some(time);
                    context.dup_in_last_time = 0;
                }
                context.usage_records_seen += 1;
                if scope == "turn" {
                    context.record_turn_total_sum = context
                        .record_turn_total_sum
                        .saturating_add(wire_total(&usage));
                }

                let mapped = map_kimi_wire(&usage);
                for contradiction in &mapped.diagnostics {
                    diagnostics.push(diag(
                        contradiction.code,
                        Some(contradiction.field),
                        raw.number,
                        &contradiction.detail,
                    ));
                }
                // session scope identifies auxiliary compaction calls in native samples;
                // turn scope uses agent identity: an agent other than main is sub_agent.
                let record_agent = line.get("agentId").and_then(|a| a.as_str());
                let effective_agent = record_agent.or(path_agent.as_deref());
                let category = if scope == "session" {
                    CallCategory::Auxiliary
                } else if effective_agent.is_some_and(|a| a != "main") {
                    CallCategory::SubAgent
                } else {
                    CallCategory::Primary
                };
                let model = line.get("model").and_then(|m| m.as_str());
                let session_ref = path_session.as_deref();
                if session_ref.is_none() && !context.missing_session_id_reported {
                    context.missing_session_id_reported = true;
                    diagnostics.push(diag(
                        "missing_session_identity",
                        Some("path"),
                        raw.number,
                        "wire path outside sessions/<wd>/<session>/agents/<agent>; no session identity",
                    ));
                }
                events.push(build_usage_event(
                    target,
                    product,
                    &context,
                    context.version_basis,
                    mapped,
                    model,
                    category,
                    session_ref,
                    effective_agent,
                    time,
                    now_ms,
                ));
            }
            "context.append_loop_event" => {
                // step.end event.usage repeats usage.record; count usage.record only:
                // retain repeated totals for reconciliation without creating events.
                let event = line.get("event");
                if event.and_then(|e| e.get("type")).and_then(|t| t.as_str()) == Some("step.end") {
                    if let Some(echo) = event
                        .and_then(|e| e.get("usage"))
                        .and_then(parse_wire_usage)
                    {
                        context.echo_total_sum =
                            context.echo_total_sum.saturating_add(wire_total(&echo));
                    }
                }
            }
            "subagent.completed" => {
                // The main-wire snapshot matched child-wire sums through completed.time in M0/local checks.
                // Child-wire calls count independently; retain only a completed-snapshot count for reconciliation notes.
                context.subagent_completed += 1;
            }
            other if KNOWN_IGNORED_TYPES.contains(&other) => {}
            "(no-type)" => {
                diagnostics.push(diag(
                    "missing_record_type",
                    Some("type"),
                    raw.number,
                    "record without type field; ignored",
                ));
            }
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
    // Reconcile repeated totals only at end-of-file, avoiding comparisons against a partial read.
    let mut reconciliations = Vec::new();
    if status == ScanStatus::Complete && context.usage_records_seen > 0 {
        // Sum turn records; repeated usage is a subset because interrupted steps can lack it (observed 958/960).
        // A positive difference describes that subset; repeated totals exceeding records signal a possible format change.
        let echo_exceeds = context.echo_total_sum > context.record_turn_total_sum;
        let difference = context.record_turn_total_sum - context.echo_total_sum;
        let verdict = if echo_exceeds {
            "mismatch"
        } else if difference == 0 {
            "matched"
        } else {
            "echo_subset"
        };
        if echo_exceeds {
            diagnostics.push(diag(
                "echo_exceeds_records",
                Some("event.usage"),
                new_cursor.line_number.saturating_sub(1),
                "step.end usage echo sum exceeds usage.record sum; format change suspected",
            ));
        }
        reconciliations.push(Reconciliation {
            series: "kimi_wire_step_end_echo".to_string(),
            detail_sum: context.record_turn_total_sum,
            snapshot_final: Some(context.echo_total_sum),
            carried_sum: 0,
            difference: Some(difference),
            verdict: verdict.to_string(),
        });
    }
    // Main-wire subagent.completed details live in another child wire file/instance;
    // retain the count and no_detail_in_file without inventing a comparison.
    if status == ScanStatus::Complete && context.subagent_completed > 0 {
        reconciliations.push(Reconciliation {
            series: format!(
                "kimi_subagent_completed_snapshot:count={}",
                context.subagent_completed
            ),
            detail_sum: 0,
            snapshot_final: None,
            carried_sum: 0,
            difference: None,
            verdict: "no_detail_in_file".to_string(),
        });
    }
    // Files without usage, such as error-only steps, remain valid; do not degrade health or insert zero usage.
    let health_degraded = !outcome.bad_lines.is_empty()
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
        reconciliations,
        health: if health_degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}
