//! Codex legacy rollout JSONL implementation rollout_legacy for registered 0.139-0.151 versions.
//!
//! Native reference: 2026-09-26 reads of all 238 local ~/.codex/sessions files in that series,
//! covering 21 versions and 13,481 token_count records. Analysis output lives under ignored
//! build/codex-legacy-forensics/.
//! - All 238 files lacked token_usage_record, unlike rollout_v1, so this reader uses
//!   event_msg/token_count for individual usage.
//! - info.total_token_usage is a six-field cumulative snapshot; last_token_usage
//!   repeats the latest call with input/cached/cache_write/output/reasoning/total.
//!   All 13,481 envelope timestamps are ISO8601 UTC with milliseconds.
//! - Compare cumulative totals record by record to classify last:
//!   * delta=current total - previous total. Positive delta covers one or more calls, with last the
//!     latest. All 13,032 positive-delta records changed last; 13,024 had delta=last.total,
//!     and eight had larger deltas covering multiple calls with only the latest in last.
//!     Example: 0.139.0 rollout-...-019ec029...jsonl L1043-L1051 has four function_call records
//!     in two model-call batches, delta=400,696 and last=199,034.
//!   * Zero delta with unchanged last occurred 121 times as repeated reports;
//!     skip duplicates when neither total nor last changes.
//!   * Zero delta with changed last occurred 85 times, all immediately after compacted:
//!     (0,0,0,0,0,N>0) compaction-summary calls are excluded from native cumulative total.
//!     Example: 0.146.0-alpha.3 rollout-...-019f9496...jsonl
//!     L335-L339 retains total=9,823,579 across compaction, with last=16,894.
//!     Emit carried events excluded from snapshot comparison, following rollout_v1:
//!     detailed sum = final snapshot + carried sum.
//!   * Four negative deltas reflect native regression/reset: 0.142.3 L338 3,015,122 to 407,209;
//!     L456 to 258400=context_window with all-zero last; 0.146.0-alpha.3 L434/L441 decreases of
//!     319/607. Without a reset marker, diagnose snapshot_regression and reset the comparison baseline.
//!     Changed nonzero last still emits an observed call; residuals remain reconciliation mismatches.
//! - First token_count had last=total in 227/238 files; the other 11 resumed sessions
//!   retained earlier context in total. Emit only last for the current observed call,
//!   leaving the residual visible in reconciliation.
//! - All 91 compacted records had latest_token_usage_record=null, unlike the 0.155 copy.
//!   Use compacted only as the structural marker for changed last with zero delta.
//! - All 2,482 turn_context records had model; use context available by the call line.
//! - Without response_id, identity is seq:<session>:<line-number>.
//! - Known structural types include session_meta/turn_context/event_msg/response_item/world_state/compacted/
//!   inter_agent_communication_metadata (129 records), ignored without usage.
//! - In 222/238 files the detailed sum matched the final snapshot; the other 16 fell into
//!   resumed-baseline, multiple-call or native-regression cases, retained as diagnostics without invented data.
//!
//! Version policy in architecture.md#unknown-version: this reader serves registered
//! 0.139-0.151 versions; unregistered versions use LatestFallback with rollout_v1.
//! Registry parser/supported-version changes reset stored checkpoints and revisit consumed files.
//! Previously incompatible legacy files had no advanced cursor; registering them permits a full read.

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use super::super::common::{map_codex_record, CodexRecordUsage};

pub const CODEX_LEGACY_PARSER_VERSION: &str = "codex-rollout-legacy-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Six usage-field sums, using i128 against overflow.
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
    usage: StoredUsage,
    line: u64,
    ts_ms: i64,
}

/// Persisted six-field snapshot suitable for serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct StoredUsage {
    input: i64,
    cached: i64,
    write: i64,
    output: i64,
    reasoning: i64,
    total: i64,
}

impl StoredUsage {
    fn to_record(self) -> CodexRecordUsage {
        CodexRecordUsage {
            input_tokens: self.input,
            cached_input_tokens: self.cached,
            cache_write_input_tokens: self.write,
            output_tokens: self.output,
            reasoning_output_tokens: self.reasoning,
            total_tokens: self.total,
        }
    }
}

/// Persisted model, previous snapshot/echo, reconciliation sums and version basis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LegacyParseContext {
    model: Option<String>,
    cli_version: Option<String>,
    thread_id: Option<String>,
    parent_thread: Option<String>,
    originator: Option<String>,
    model_provider: Option<String>,
    category: Option<String>,
    session_started_ms: Option<i64>,
    /// Previous token_count total for delta comparison; reset after regression.
    prev_total: Option<i64>,
    /// Previous last six-field echo for duplicate detection.
    prev_last: Option<[i64; 6]>,
    /// Whether compacted occurred since the previous token_count, used for changed last with zero delta.
    saw_compacted: bool,
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
    /// Version selection basis: known_version/latest_fallback.
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> LegacyParseContext {
    if rescan {
        return LegacyParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<LegacyParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// Require six bounded nonnegative integers; missing/invalid fields return None.
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

fn usage_six(usage: &CodexRecordUsage) -> [i64; 6] {
    [
        usage.input_tokens,
        usage.cached_input_tokens,
        usage.cache_write_input_tokens,
        usage.output_tokens,
        usage.reasoning_output_tokens,
        usage.total_tokens,
    ]
}

fn parse_envelope_ts(line: &serde_json::Value) -> Option<(i64, String)> {
    let raw = line.get("timestamp")?.as_str()?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

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

/// last_token_usage classification using the native checks described above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastVerdict {
    /// Observed new call: positive delta, valid first record or changed echo after regression.
    NewCall,
    /// Changed last with zero delta after compacted: emit a carried call excluded from snapshot comparison.
    CompactionEcho,
    /// Zero delta with unchanged last is a duplicate report; skip it.
    DuplicateReport,
    /// Missing/all-zero last has no individual usage; skip it and expose residuals in reconciliation.
    NoPerCallEvidence,
}

/// Classify one token_count last echo using the checked rules.
fn classify_last(
    prev_total: Option<i64>,
    cur_total: i64,
    prev_last: Option<[i64; 6]>,
    last: Option<[i64; 6]>,
    saw_compacted: bool,
) -> LastVerdict {
    let last_valid = last.is_some_and(|l| l[5] > 0);
    match prev_total {
        None => {
            // A valid first last represents the first observed call, including resumed sessions; diagnose residuals.
            if last_valid {
                LastVerdict::NewCall
            } else {
                LastVerdict::NoPerCallEvidence
            }
        }
        Some(prev) => {
            let delta = cur_total - prev;
            if delta > 0 {
                // Positive delta covers one or more calls; last is latest (13,032/13,032 native echoes changed).
                if last_valid {
                    LastVerdict::NewCall
                } else {
                    LastVerdict::NoPerCallEvidence
                }
            } else if delta == 0 {
                if last != prev_last && last_valid {
                    if saw_compacted {
                        LastVerdict::CompactionEcho
                    } else {
                        // This case was absent from 13,481 native records; emit the changed nonzero echo and diagnose residuals.
                        LastVerdict::NewCall
                    }
                } else {
                    LastVerdict::DuplicateReport
                }
            } else {
                // On native regression, emit a changed nonzero echo and reset the comparison baseline.
                if last_valid && last != prev_last {
                    LastVerdict::NewCall
                } else {
                    LastVerdict::NoPerCallEvidence
                }
            }
        }
    }
}

/// Build model_call from last_token_usage with seq:<session>:<line-number> identity.
#[allow(clippy::too_many_arguments)]
fn build_last_event(
    target: &ScanTarget,
    context: &LegacyParseContext,
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
    let session_key = context.thread_id.as_deref().unwrap_or("unknown-session");
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("seq:{session_key}:{line}"),
        record_kind: RecordKind::ModelCall,
        schema_version: context
            .cli_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: CODEX_LEGACY_PARSER_VERSION.to_string(),
        parse_basis: context.version_basis,
        origin_call_id: None,
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

/// Incrementally scan legacy rollout files through CodexAdapter registry dispatch.
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
            "compacted" => {
                // All 91 checked legacy compacted records lacked a carried-record copy.
                // Use the marker only for changed last with zero delta.
                context.saw_compacted = true;
            }
            "event_msg" => {
                let sub = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match sub {
                    "token_count" => {
                        if super::super::common::token_count_has_no_usage(&payload) {
                            continue;
                        }
                        let info = payload
                            .get("info")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let Some(total) = info.get("total_token_usage").and_then(parse_usage)
                        else {
                            diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("info.total_token_usage"),
                                raw.number,
                                "token_count snapshot missing required numeric fields; skipped",
                            ));
                            continue;
                        };
                        let last = info.get("last_token_usage").and_then(parse_usage);
                        let Some((observed_ms, source_time)) = parse_envelope_ts(&line) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "envelope timestamp missing or unparseable; snapshot skipped",
                            ));
                            continue;
                        };
                        let cur_total = total.total_tokens;
                        let verdict = classify_last(
                            context.prev_total,
                            cur_total,
                            context.prev_last,
                            last.as_ref().map(usage_six),
                            context.saw_compacted,
                        );
                        if let Some(prev) = context.prev_total.filter(|p| cur_total < *p) {
                            diagnostics.push(diag(
                                "snapshot_regression",
                                Some("info.total_token_usage"),
                                raw.number,
                                &format!(
                                    "cumulative snapshot decreased {prev} -> {cur_total} without structural reset evidence; delta baseline rebased"
                                ),
                            ));
                        }
                        match verdict {
                            LastVerdict::NewCall | LastVerdict::CompactionEcho => {
                                let usage = last.expect("verdict guarantees last");
                                if verdict == LastVerdict::NewCall {
                                    context.sum_per_call.add(&usage);
                                } else {
                                    context.sum_carried.add(&usage);
                                }
                                events.push(build_last_event(
                                    target,
                                    &context,
                                    &usage,
                                    observed_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    &mut diagnostics,
                                ));
                            }
                            LastVerdict::DuplicateReport | LastVerdict::NoPerCallEvidence => {}
                        }
                        context.prev_total = Some(cur_total);
                        context.prev_last = last.as_ref().map(usage_six);
                        context.saw_compacted = false;
                        context.final_snapshot = Some(SnapshotState {
                            usage: StoredUsage {
                                input: total.input_tokens,
                                cached: total.cached_input_tokens,
                                write: total.cache_write_input_tokens,
                                output: total.output_tokens,
                                reasoning: total.reasoning_output_tokens,
                                total: total.total_tokens,
                            },
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
            // Ignore known structural records without usage in this series.
            "response_item" | "world_state" | "inter_agent_communication_metadata" => {}
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
    // Reconcile only at the current EOF while allowing future appends:
    // regular+carried sum = final snapshot+carried; native snapshots exclude compaction-summary echoes.
    if status == ScanStatus::Complete {
        let detail = context.sum_per_call.total + context.sum_carried.total;
        let snapshot = context.final_snapshot.map(|s| i128::from(s.usage.total));
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
                    "per-call sum {} != final snapshot {} + compaction carried {} (diff {}); known causes: resumed-session baseline, multi-call interval, source counter regression",
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
        // Keep the final native interval snapshot for comparison without adding it to totals.
        if let Some(snap) = context.final_snapshot {
            let mapped = map_codex_record(&snap.usage.to_record());
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
        || diagnostics.iter().any(|d| {
            // Legacy call classification requires total and last together; malformed snapshots can hide observed calls.
            matches!(
                d.code.as_str(),
                "bad_json_line"
                    | "usage_shape_deviation"
                    | "timestamp_unparseable"
                    | "line_too_long"
            )
        });
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

    fn six_usage(
        input: i64,
        cached: i64,
        write: i64,
        output: i64,
        reasoning: i64,
        total: i64,
    ) -> [i64; 6] {
        [input, cached, write, output, reasoning, total]
    }

    #[test]
    fn first_token_count_emits_when_last_positive() {
        assert_eq!(
            classify_last(
                None,
                100,
                None,
                Some(six_usage(80, 20, 0, 20, 5, 100)),
                false
            ),
            LastVerdict::NewCall
        );
        assert_eq!(
            classify_last(None, 0, None, Some(six_usage(0, 0, 0, 0, 0, 0)), false),
            LastVerdict::NoPerCallEvidence
        );
        assert_eq!(
            classify_last(None, 100, None, None, false),
            LastVerdict::NoPerCallEvidence
        );
    }

    #[test]
    fn positive_delta_is_new_call_even_when_equal_to_previous() {
        // Positive delta is a new observed call even if last equals its predecessor (13,032/13,032 native checks).
        let last = six_usage(80, 20, 0, 20, 5, 100);
        assert_eq!(
            classify_last(Some(0), 100, Some(last), Some(last), false),
            LastVerdict::NewCall
        );
        // Missing last cannot produce an individual usage event.
        assert_eq!(
            classify_last(Some(0), 100, Some(last), None, false),
            LastVerdict::NoPerCallEvidence
        );
    }

    #[test]
    fn zero_delta_dedups_repeated_report() {
        let last = six_usage(80, 20, 0, 20, 5, 100);
        assert_eq!(
            classify_last(Some(100), 100, Some(last), Some(last), false),
            LastVerdict::DuplicateReport
        );
    }

    #[test]
    fn zero_delta_changed_last_after_compaction_is_carried() {
        // Compaction-summary echo leaves total unchanged and changes last to total-only positive usage.
        let prev = six_usage(214636, 211840, 0, 982, 391, 215618);
        let echo = six_usage(0, 0, 0, 0, 0, 13444);
        assert_eq!(
            classify_last(Some(3_000_000), 3_000_000, Some(prev), Some(echo), true),
            LastVerdict::CompactionEcho
        );
        // A zero-delta change without compacted was not observed natively; retain the changed nonzero echo.
        assert_eq!(
            classify_last(Some(3_000_000), 3_000_000, Some(prev), Some(echo), false),
            LastVerdict::NewCall
        );
    }

    #[test]
    fn regression_rebases_and_emits_only_real_change() {
        let prev_last = six_usage(146293, 140160, 0, 1014, 0, 147307);
        let new_last = six_usage(147942, 140160, 0, 867, 516, 148809);
        // Regression plus changed usage emits a call, matching native 0.142.3 L338.
        assert_eq!(
            classify_last(
                Some(3_015_122),
                407_209,
                Some(prev_last),
                Some(new_last),
                false
            ),
            LastVerdict::NewCall
        );
        // Regression with all-zero last has no individual usage, matching native 0.142.3 L456.
        assert_eq!(
            classify_last(
                Some(568_759),
                258_400,
                Some(new_last),
                Some(six_usage(0, 0, 0, 0, 0, 0)),
                false
            ),
            LastVerdict::NoPerCallEvidence
        );
        // Regression with unchanged last has no new observed call.
        assert_eq!(
            classify_last(
                Some(568_759),
                258_400,
                Some(new_last),
                Some(new_last),
                false
            ),
            LastVerdict::NoPerCallEvidence
        );
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        let legacy = serde_json::json!({
            "model": Some("gpt-5-codex"),
            "cli_version": Some("0.142.5"),
            "thread_id": Some("t"),
            "parent_thread": null,
            "originator": null,
            "model_provider": Some("openai"),
            "category": Some("primary"),
            "session_started_ms": Some(1),
            "prev_total": Some(100),
            "prev_last": Some([80, 20, 0, 20, 5, 100]),
            "saw_compacted": false,
            "sum_per_call": UsageSums::default(),
            "sum_carried": UsageSums::default(),
            "final_snapshot": null,
            "turns_started": 1,
            "turns_completed": 1,
            "turns_aborted": 0,
            "unknown_types": [],
        });
        let ctx: LegacyParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
        assert_eq!(ctx.cli_version.as_deref(), Some("0.142.5"));
        assert_eq!(ctx.prev_last, Some([80, 20, 0, 20, 5, 100]));
    }
}
