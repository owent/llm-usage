//! Codex rollout JSONL implementation rollout_v1.
//!
//! References: M0 native reads and samples under build/desktop-usage-validation.
//! - token_usage_record stores model_call with six payload.usage fields:
//!   input/cached/cache_write/output/reasoning/total; cached is within input and reasoning within output.
//!   total=input+output held in 319/319 records; response_id identifies the call.
//! - event_msg/token_count info.total_token_usage is cumulative: use its final value or
//!   detailed calls, without summing snapshots. Compaction removes carried records from the snapshot;
//!   checked detailed sum = final snapshot + compacted carried sum.
//! - Ignore info.last_token_usage echoes in this format to prevent duplicate usage.
//! - compacted payload.latest_token_usage_record copies the excluded boundary call;
//!   normally it shares response_id with the detailed stream and is not in the snapshot.
//! - usage has no model; use preceding turn_context, leaving unverified attribution unknown.
//! - session_meta supplies payload.cli_version; parent_thread_id identifies a subagent session.
//!
//! Version selection follows architecture.md#unknown-version and
//! super::super::versions::select. Unregistered/missing versions try this latest reader,
//! with event parse_basis compatibility metadata rather than rejection by version alone.

use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};

use super::super::common::{map_codex_record, CodexRecordUsage};

// Registry rule changes replay consumed files; retain cumulative mismatches and per-call errors across batches.
pub const CODEX_PARSER_VERSION: &str = "codex-rollout-5";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Six usage-field sums for snapshot/carried reconciliation; i128 protects additions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct UsageSums {
    input: i128,
    cached: i128,
    write: i128,
    output: i128,
    reasoning: i128,
    total: i128,
    calls: u64,
}

impl UsageSums {
    fn add(&mut self, usage: &CodexRecordUsage) {
        self.input += i128::from(usage.input_tokens);
        self.cached += i128::from(usage.cached_input_tokens);
        self.write += i128::from(usage.cache_write_input_tokens);
        self.output += i128::from(usage.output_tokens);
        self.reasoning += i128::from(usage.reasoning_output_tokens);
        self.total += i128::from(usage.total_tokens);
        self.calls += 1;
    }
}

/// Final snapshot state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct SnapshotState {
    usage_input: i64,
    usage_cached: i64,
    usage_write: i64,
    usage_output: i64,
    usage_reasoning: i64,
    usage_total: i64,
    line: u64,
    ts_ms: i64,
}

/// Persisted model state, cumulative baseline, reconciliation sums and version basis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CodexParseContext {
    model: Option<String>,
    cli_version: Option<String>,
    thread_id: Option<String>,
    parent_thread: Option<String>,
    originator: Option<String>,
    model_provider: Option<String>,
    category: Option<String>,
    session_started_ms: Option<i64>,
    series_last_value: Option<i64>,
    series_last_observed_ms: Option<i64>,
    compacted_since_snapshot: bool,
    sum_per_call: UsageSums,
    sum_carried: UsageSums,
    final_snapshot: Option<SnapshotState>,
    turns_started: u64,
    turns_completed: u64,
    turns_aborted: u64,
    #[serde(default)]
    unknown_types: Vec<String>,
    #[serde(default)]
    has_record_errors: bool,
    /// known_version/latest_fallback; historical context without this field defaults to None.
    /// Adding the field alone does not rebuild sources or reset cursors (V30).
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// Restore a JSON cursor; rescan or invalid state starts at the file beginning.
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> CodexParseContext {
    if rescan {
        return CodexParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<CodexParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// Require six bounded nonnegative integer fields; return None for missing/invalid values for caller diagnostics.
fn parse_usage(value: &serde_json::Value) -> Option<CodexRecordUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(CodexRecordUsage {
        input_tokens: get("input_tokens")?,
        cached_input_tokens: get("cached_input_tokens")?,
        cache_write_input_tokens: get("cache_write_input_tokens")?,
        output_tokens: get("output_tokens")?,
        reasoning_output_tokens: get("reasoning_output_tokens")?,
        total_tokens: get("total_tokens")?,
    })
}

fn parse_envelope_ts(line: &serde_json::Value) -> Option<(i64, String)> {
    let raw = line.get("timestamp")?.as_str()?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// Map verified originator values to hosts; leave unknown hosts unset.
fn map_originator(originator: Option<&str>) -> Option<String> {
    match originator {
        Some("codex_vscode") => Some("vscode".to_string()),
        _ => None,
    }
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

struct UsageRecordIds<'a> {
    response_id: Option<&'a str>,
    thread_id: Option<&'a str>,
    session_id: Option<&'a str>,
}

/// Build model_call from token_usage_record or a compacted carried record.
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    context: &CodexParseContext,
    ids: &UsageRecordIds<'_>,
    usage: &CodexRecordUsage,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    let mapped = map_codex_record(usage);
    for contradiction in &mapped.diagnostics {
        diagnostics.push(diag(
            contradiction.code,
            Some(contradiction.field),
            line,
            &contradiction.detail,
        ));
    }
    let session_key = context
        .thread_id
        .as_deref()
        .or(ids.thread_id)
        .or(ids.session_id)
        .unwrap_or("unknown-session");
    let source_record_key = match ids.response_id {
        Some(rid) => format!("resp:{rid}"),
        None => {
            diagnostics.push(diag(
                "missing_response_id",
                Some("response_id"),
                line,
                "usage record without response_id; fallback identity session UUID + line number",
            ));
            format!("seq:{session_key}:{line}")
        }
    };
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key,
        record_kind: RecordKind::ModelCall,
        schema_version: context
            .cli_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: CODEX_PARSER_VERSION.to_string(),
        parse_basis: context.version_basis,
        origin_call_id: ids.response_id.map(str::to_string),
        attempt_id: None,
        session_id: Some(session_key.to_string()),
        parent_session_id: context.parent_thread.clone(),
        host_application: map_originator(context.originator.as_deref()),
        agent: "codex".to_string(),
        call_category: match context.category.as_deref() {
            Some("sub_agent") => CallCategory::SubAgent,
            _ => CallCategory::Primary,
        },
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(source_time.to_string()),
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: context.model_provider.clone(),
        model_raw: context.model.clone(),
        model_canonical: None,
        model_attribution: if context.model.is_some() {
            ModelAttribution::ProviderMapping
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

/// Incrementally read one rollout file, dispatched by CodexAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    let mut events: Vec<EventInput> = Vec::new();
    let mut aggregates: Vec<SourceAggregateInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut reconciliations: Vec<Reconciliation> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut emitted_ids: HashSet<String> = HashSet::new();
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
        let payload = line
            .get("payload")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        match record_type {
            "session_meta" => {
                if context.cli_version.is_some() {
                    diagnostics.push(diag(
                        "unexpected_session_meta",
                        Some("type"),
                        raw.number,
                        "second session_meta in one rollout file; ignored",
                    ));
                    continue;
                }
                let version = json_str(&payload, "cli_version");
                // Detection/scanning share registry selection: known versions map to implementations,
                // unknown/missing versions try this latest reader with compatibility metadata (V17/V30).
                let selection = super::super::versions::select(version);
                context.version_basis = Some(selection.basis);
                context.cli_version = version.map(str::to_string);
                context.thread_id = json_str(&payload, "id")
                    .or_else(|| json_str(&payload, "session_id"))
                    .map(str::to_string);
                context.parent_thread = json_str(&payload, "parent_thread_id").map(str::to_string);
                context.originator = json_str(&payload, "originator").map(str::to_string);
                context.model_provider = json_str(&payload, "model_provider").map(str::to_string);
                let subagent_source = payload
                    .get("source")
                    .map(|s| s.is_object() && s.get("subagent").is_some())
                    .unwrap_or(false);
                context.category = Some(
                    if context.parent_thread.is_some() || subagent_source {
                        "sub_agent"
                    } else {
                        "primary"
                    }
                    .to_string(),
                );
                context.session_started_ms = json_str(&payload, "timestamp")
                    .and_then(|s| s.parse::<jiff::Timestamp>().ok())
                    .map(|t| t.as_millisecond());
            }
            "turn_context" => {
                if let Some(model) = json_str(&payload, "model") {
                    context.model = Some(model.to_string());
                }
            }
            "token_usage_record" => {
                let Some(usage) = payload.get("usage").and_then(parse_usage) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "token_usage_record usage missing required numeric fields; record skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_envelope_ts(&line) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "envelope timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let ids = UsageRecordIds {
                    response_id: json_str(&payload, "response_id"),
                    thread_id: json_str(&payload, "thread_id"),
                    session_id: json_str(&payload, "session_id"),
                };
                // Emit events for ingest identity/conflict resolution; reconciliation sums count
                // each response_id once per run so repeated finals do not duplicate those sums.
                // Appended cross-run repeats/conflicts remain visible through conflicts and reconcile_mismatch diagnostics.
                let first_sighting = match ids.response_id {
                    Some(rid) => emitted_ids.insert(rid.to_string()),
                    None => true,
                };
                if first_sighting {
                    context.sum_per_call.add(&usage);
                }
                events.push(build_usage_event(
                    target,
                    &context,
                    &ids,
                    &usage,
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    &mut diagnostics,
                ));
            }
            "compacted" => {
                context.compacted_since_snapshot = true;
                let carried = payload
                    .get("latest_token_usage_record")
                    .filter(|value| !value.is_null())
                    .cloned();
                if let Some(carried) = carried {
                    match carried.get("usage").and_then(parse_usage) {
                        Some(usage) => {
                            context.sum_carried.add(&usage);
                            let rid = json_str(&carried, "response_id");
                            // A carried record normally also exists in the detailed stream. If absent,
                            // emit a recovery event with diagnostics so reconciliation exposes the difference.
                            if !rid.map(|r| emitted_ids.contains(r)).unwrap_or(false) {
                                diagnostics.push(diag(
                                    "compacted_carried_not_in_stream",
                                    Some("latest_token_usage_record"),
                                    raw.number,
                                    "compaction carried record not seen in per-call stream; recovered as model_call",
                                ));
                                if let Some((occurred_ms, source_time)) = parse_envelope_ts(&line) {
                                    let ids = UsageRecordIds {
                                        response_id: rid,
                                        thread_id: json_str(&carried, "thread_id"),
                                        session_id: json_str(&carried, "session_id"),
                                    };
                                    events.push(build_usage_event(
                                        target,
                                        &context,
                                        &ids,
                                        &usage,
                                        occurred_ms,
                                        &source_time,
                                        raw.number,
                                        now_ms,
                                        &mut diagnostics,
                                    ));
                                }
                            }
                        }
                        None => diagnostics.push(diag(
                            "usage_shape_deviation",
                            Some("latest_token_usage_record.usage"),
                            raw.number,
                            "compacted carried usage missing required numeric fields",
                        )),
                    }
                }
            }
            "event_msg" => {
                let sub = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match sub {
                    "token_count" => {
                        if super::super::common::token_count_has_no_usage(&payload) {
                            continue;
                        }
                        let Some(total) = payload
                            .get("info")
                            .and_then(|i| i.get("total_token_usage"))
                            .and_then(parse_usage)
                        else {
                            diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("info.total_token_usage"),
                                raw.number,
                                "token_count snapshot missing required numeric fields; skipped",
                            ));
                            continue;
                        };
                        let Some((observed_ms, _)) = parse_envelope_ts(&line) else {
                            diagnostics.push(diag(
                                "snapshot_timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "envelope timestamp missing or unparseable; snapshot skipped",
                            ));
                            continue;
                        };
                        // Native checks show compaction-carried records excluded from cumulative snapshots.
                        let previous = context.series_last_value.map(|last| {
                            crate::aggregates::CumulativeState {
                                series_key: "thread".to_string(),
                                last_value: last,
                                last_observed_ms: context.series_last_observed_ms.unwrap_or(0),
                                start_ms: context.session_started_ms,
                            }
                        });
                        let (state, cum_outcome) = crate::aggregates::observe_cumulative(
                            "thread",
                            previous.as_ref(),
                            total.total_tokens,
                            observed_ms,
                            context.compacted_since_snapshot,
                        );
                        match cum_outcome {
                            crate::aggregates::CumulativeOutcome::Regression {
                                previous,
                                observed,
                            } => {
                                diagnostics.push(diag(
                                    "snapshot_regression",
                                    Some("info.total_token_usage"),
                                    raw.number,
                                    &format!("cumulative snapshot decreased {previous} -> {observed} without compaction evidence"),
                                ));
                            }
                            crate::aggregates::CumulativeOutcome::OutOfOrder => {
                                diagnostics.push(diag(
                                    "snapshot_out_of_order",
                                    Some("info.total_token_usage"),
                                    raw.number,
                                    "cumulative snapshot out of order; baseline kept",
                                ));
                            }
                            crate::aggregates::CumulativeOutcome::Reset { .. }
                            | crate::aggregates::CumulativeOutcome::FirstObservation { .. }
                            | crate::aggregates::CumulativeOutcome::Delta { .. } => {}
                        }
                        context.series_last_value = Some(state.last_value);
                        context.series_last_observed_ms = Some(state.last_observed_ms);
                        context.compacted_since_snapshot = false;
                        context.final_snapshot = Some(SnapshotState {
                            usage_input: total.input_tokens,
                            usage_cached: total.cached_input_tokens,
                            usage_write: total.cache_write_input_tokens,
                            usage_output: total.output_tokens,
                            usage_reasoning: total.reasoning_output_tokens,
                            usage_total: total.total_tokens,
                            line: raw.number,
                            ts_ms: observed_ms,
                        });
                    }
                    "task_started" => context.turns_started += 1,
                    "task_complete" => context.turns_completed += 1,
                    "turn_aborted" => context.turns_aborted += 1,
                    _ => {}
                }
            }
            "response_item" | "world_state" => {}
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
    // Reconcile only at the current EOF because the file may still be growing.
    if status == ScanStatus::Complete {
        let detail = context.sum_per_call.total;
        let snapshot = context.final_snapshot.map(|s| i128::from(s.usage_total));
        let carried = context.sum_carried.total;
        let (difference, verdict) = match snapshot {
            Some(snap) => {
                let diff = detail - (snap + carried);
                let verdict = if diff == 0 { "matched" } else { "mismatch" };
                (Some(diff), verdict)
            }
            None => (None, "no_snapshot"),
        };
        if verdict == "mismatch" {
            diagnostics.push(DiagnosticInput {
                event_id: None,
                code: "reconcile_mismatch".to_string(),
                field: Some("total_tokens".to_string()),
                position: None,
                message: format!(
                    "per-call sum {} != final snapshot {} + compaction carried {} (diff {})",
                    detail,
                    snapshot.unwrap_or(0),
                    carried,
                    difference.unwrap_or(0)
                ),
            });
        }
        reconciliations.push(Reconciliation {
            series: "session_cumulative_snapshot".to_string(),
            detail_sum: detail.min(i128::from(i64::MAX)) as i64,
            snapshot_final: snapshot.map(|v| v.min(i128::from(i64::MAX)) as i64),
            carried_sum: carried.min(i128::from(i64::MAX)) as i64,
            difference: difference.map(|v| v.min(i128::from(i64::MAX)) as i64),
            verdict: verdict.to_string(),
        });
        // Store final native interval snapshots for comparison, excluded from totals.
        if let Some(snap) = context.final_snapshot {
            let mapped = map_codex_record(&CodexRecordUsage {
                input_tokens: snap.usage_input,
                cached_input_tokens: snap.usage_cached,
                cache_write_input_tokens: snap.usage_write,
                output_tokens: snap.usage_output,
                reasoning_output_tokens: snap.usage_reasoning,
                total_tokens: snap.usage_total,
            });
            aggregates.push(SourceAggregateInput {
                instance_id: target.instance_id.clone(),
                scope: AggregateScope::Session,
                scope_key: format!(
                    "codex-snapshot:{}",
                    context.thread_id.as_deref().unwrap_or("unknown-session")
                ),
                interval_start_ms: context.session_started_ms,
                interval_end_ms: snap.ts_ms,
                interval_end_inclusive: false,
                usage: mapped.usage,
                quality: mapped.quality,
                reported_call_count: None,
                coverage: Coverage::Duplicate,
                duplicate_of: None,
                time_basis: TimeBasis::SourceCompletion,
                source_revision: Some((target.generation << 48) | snap.line as i64),
            });
        }
    }
    let new_cursor = JsonlCursor {
        generation: target.generation,
        offset: outcome.next_offset,
        line_number: outcome.next_line_number,
    };
    context.has_record_errors |= !outcome.bad_lines.is_empty()
        || diagnostics
            .iter()
            .any(super::super::common::is_record_error);
    let degraded = context.has_record_errors;
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates,
        diagnostics,
        lines_read: outcome.lines.len() as u64,
        records_seen,
        reconciliations,
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
    fn parse_usage_requires_six_fields() {
        let v = serde_json::json!({
            "input_tokens": 10,
            "cached_input_tokens": 4,
            "cache_write_input_tokens": 0,
            "output_tokens": 2,
            "reasoning_output_tokens": 1,
            "total_tokens": 12
        });
        assert_eq!(parse_usage(&v).unwrap().total_tokens, 12);
        let missing = serde_json::json!({"input_tokens": 10});
        assert!(parse_usage(&missing).is_none());
        let negative = serde_json::json!({
            "input_tokens": -1,
            "cached_input_tokens": 0,
            "cache_write_input_tokens": 0,
            "output_tokens": 0,
            "reasoning_output_tokens": 0,
            "total_tokens": 0
        });
        assert!(parse_usage(&negative).is_none());
    }

    #[test]
    fn originator_mapping_is_versioned() {
        assert_eq!(
            map_originator(Some("codex_vscode")),
            Some("vscode".to_string())
        );
        assert_eq!(map_originator(Some("codex_cli")), None);
        assert_eq!(map_originator(None), None);
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // Deserialize historical M2-A context without version_basis;
        // basis stays None without rebuilding its source or resetting its cursor for that field alone (V30).
        let legacy = serde_json::json!({
            "model": Some("gpt-x"),
            "cli_version": Some("0.155.0-alpha.16.3"),
            "thread_id": Some("t"),
            "parent_thread": null,
            "originator": null,
            "model_provider": Some("openai"),
            "category": Some("primary"),
            "session_started_ms": Some(1),
            "series_last_value": Some(100),
            "series_last_observed_ms": Some(2),
            "compacted_since_snapshot": false,
            "sum_per_call": UsageSums::default(),
            "sum_carried": UsageSums::default(),
            "final_snapshot": null,
            "turns_started": 1,
            "turns_completed": 1,
            "turns_aborted": 0,
            "unknown_types": [],
        });
        let ctx: CodexParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
        assert_eq!(ctx.cli_version.as_deref(), Some("0.155.0-alpha.16.3"));
    }
}
