//! Historical DSH documentation-based JSONL reader (session_log_doc1,
//! format session-log-doc-1), separate from the verified native v4 reader.
//!
//! Reference: token-meter README pinned at 46a7f68b0922371ce7144b668b90e377d8e799f4.
//! A08 documentation/source implementation; this legacy JSONL representation lacks native validation.
//! - Documented event types: step/start, assistant/message,
//!   llm/retry-started, request/context, request/header, image/offload.
//!   assistant/message carries usage samples with optional
//!   uncachedInputTokens/outputTokens/cacheReadTokens/cacheWriteTokens.
//! - A final assistant sample replaces streaming usage within the same attempt.
//!   llm/retry-started ends that replacement scope; a retry within the same
//!   step has separate usage. The README describes it as another billed attempt,
//!   without making the collected tokens a billing record. Samples replace earlier values;
//!   their totals need not increase monotonically.
//! - Exclude contextPressure (pressureTokens/projectedTokens/contextWindow)
//!   and contextBreakdown (systemTokens/toolsTokens/messageTokens): these describe
//!   estimated context sizes, not provider-reported usage.
//! - The README does not document disk paths or serialized rows. This reader's JSONL shape
//!   is a synthetic assumption, marked in test data; native v4 has separate rules.
//!
//! Sample replacement uses existing ingest revision and conflict handling:
//! - Attempt key: {file_identity}:s{step}:a{attempt}; count step/start for step,
//!   and llm/retry-started within each step for attempt; replay deterministically.
//! - Import each attempt sample immediately as Partial, with a sample revision.
//!   Sample ordinal plus monotonic revision_floor gives later samples higher revisions;
//!   Replace removes the old contribution (V03 sample 7), avoiding equal-revision replay conflicts.
//! - At llm/retry-started or step/start, emit the last sample again as
//!   Final with a higher revision. An open attempt at the log tail remains Partial:
//!   retain its current values without claiming a confirmed completion state.
//! - Missing attempt keys after a rescan emit Corrected/Excluded records to remove old contributions.
//!   Preserve revision_floor and tracked attempt keys across rescans; reset sample counters.
//!
//! V17 rejects an entire file with an undocumented event type, retaining its cursor for
//! repeated rejection. Skip and diagnose invalid usage fields (negative/noninteger/out-of-range) per record.
//! The README lacks per-event timestamps; occurred_at uses observed_at, limiting daily attribution.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::super::common::{map_dsh_usage, DshUsage};
use super::DSH_FORMAT_VERSION;

pub const DSH_PARSER_VERSION: &str = "dsh-session-log-doc1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Documented event types shared by detection and scanning.
pub const DOCUMENTED_EVENT_TYPES: &[&str] = &[
    "step/start",
    "assistant/message",
    "llm/retry-started",
    "request/context",
    "request/header",
    "image/offload",
];

/// Last imported sample of an open attempt, persisted until a boundary finalizes it.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct PendingSample {
    revision: i64,
    usage: DshUsage,
}

/// Persisted sample-replacement state. Rescans reset step/attempt/ordinal/pending counters
/// and replay from the beginning; preserve revision_floor and tracked_keys.
/// Higher revisions prevent replay conflicts; tracked keys identify removed attempts.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct DshParseContext {
    /// Highest emitted source_revision, monotonic across rounds.
    #[serde(default)]
    revision_floor: i64,
    /// Previously imported attempt keys for identifying removals; preserve across rescans.
    #[serde(default)]
    tracked_keys: Vec<String>,
    /// Number of observed step/start events; zero means no step yet.
    #[serde(default)]
    step_index: u64,
    /// Number of llm/retry-started events within the current step: the attempt index.
    #[serde(default)]
    attempt_index: u64,
    /// Samples emitted in the current attempt +1: the next sample ordinal.
    #[serde(default)]
    sample_ordinal: i64,
    /// Last sample of an open attempt, finalized at a boundary event.
    #[serde(default)]
    pending: Option<PendingSample>,
    #[serde(default)]
    without_usage_reported: bool,
    #[serde(default)]
    estimate_excluded_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    /// Version basis: KnownVersion identifies the documentation format, not a native DSH client version.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

impl DshParseContext {
    /// Rescan resets replacement counters and retains prior keys and the highest revision.
    fn reset_fold(&mut self) {
        self.step_index = 0;
        self.attempt_index = 0;
        self.sample_ordinal = 0;
        self.pending = None;
        self.without_usage_reported = false;
        self.estimate_excluded_reported = false;
        self.unmapped_keys_reported = false;
    }

    fn attempt_key(&self, file_identity: &str) -> String {
        format!(
            "{file_identity}:s{}:a{}",
            self.step_index, self.attempt_index
        )
    }
}

/// Restore the saved JSON cursor; invalid state or a rescan starts at the file beginning.
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> DshParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<DshParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan {
        ctx.reset_fold();
    }
    ctx
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

/// Four optional bounded nonnegative i64 usage fields; nonobjects return None.
/// Extra undocumented keys set true; retain mapped fields and diagnose once.
const USAGE_KEYS: &[&str] = &[
    "uncachedInputTokens",
    "outputTokens",
    "cacheReadTokens",
    "cacheWriteTokens",
];

fn parse_usage(value: &serde_json::Value) -> Option<(DshUsage, bool)> {
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
    let usage = DshUsage {
        uncached_input_tokens: get("uncachedInputTokens")?,
        output_tokens: get("outputTokens")?,
        cache_read_tokens: get("cacheReadTokens")?,
        cache_write_tokens: get("cacheWriteTokens")?,
    };
    let unknown = obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str()));
    Some((usage, unknown))
}

/// Incrementally scan a historical documentation-format log, called by DshAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // No native client version field; KnownVersion refers only to session-log-doc-1.
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String)> = None;
    // Incremental reads begin with previously tracked attempt keys, so append-only files
    // retain old attempts even without new rows. Rescans rebuild the complete set from empty;
    // removed keys are the previous set minus the complete rebuilt set.
    let mut current_keys: Vec<String> = if target.rescan {
        Vec::new()
    } else {
        context.tracked_keys.clone()
    };

    // Build a sample event: streaming samples are Partial; boundary-finalized samples are Final.
    let sample_event = |target: &ScanTarget,
                        context: &DshParseContext,
                        key: &str,
                        usage: &DshUsage,
                        revision: i64,
                        lifecycle: Lifecycle|
     -> EventInput {
        let mapped = map_dsh_usage(usage);
        EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: key.to_string(),
            record_kind: RecordKind::ModelCall,
            schema_version: DSH_FORMAT_VERSION.to_string(),
            parser_version: DSH_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: Some(format!(
                "s{}:a{}",
                context.step_index, context.attempt_index
            )),
            session_id: None,
            parent_session_id: None,
            host_application: None,
            agent: "deepseek-harness".to_string(),
            call_category: CallCategory::Primary,
            // No per-event time in the pinned README; observation time limits daily attribution.
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
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle,
            source_revision: Some(revision),
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        }
    };

    // Finalize an open attempt by re-emitting its last sample as Final with a higher revision;
    // with no pending sample, only advance the boundary.
    macro_rules! finalize_attempt {
        () => {
            if let Some(pending) = context.pending.take() {
                let key = context.attempt_key(&target.file_identity);
                let revision = context.revision_floor + 1;
                context.revision_floor = revision;
                events.push(sample_event(
                    target,
                    &context,
                    &key,
                    &pending.usage,
                    revision,
                    Lifecycle::Final,
                ));
            }
        };
    }

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
        let event_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !DOCUMENTED_EVENT_TYPES.contains(&event_type) {
            fail_closed = Some((
                raw.number,
                format!("event type {event_type:?} not in documented set"),
            ));
            break;
        }
        match event_type {
            "step/start" => {
                // Step boundary: finalize the current attempt and start a new step with attempt index zero.
                finalize_attempt!();
                context.step_index += 1;
                context.attempt_index = 0;
                context.sample_ordinal = 0;
            }
            "llm/retry-started" => {
                // Retry boundary: finalize the current attempt and start another within the same step,
                // with a separate key and independently counted usage.
                finalize_attempt!();
                context.attempt_index += 1;
                context.sample_ordinal = 0;
            }
            "assistant/message" => {
                let usage_value = line.get("usage");
                let Some(usage_value) = usage_value else {
                    // An assistant/message without usage produces no event;
                    // record a one-time diagnostic and retain other readable data.
                    if !context.without_usage_reported {
                        context.without_usage_reported = true;
                        diagnostics.push(diag(
                            "assistant_message_without_usage",
                            Some("usage"),
                            raw.number,
                            "assistant/message without usage object; no usage evidence, no event",
                        ));
                    }
                    continue;
                };
                let Some((usage, unknown_keys)) = parse_usage(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage value not an object or a field is negative/non-integer/out of range; record skipped",
                    ));
                    continue;
                };
                if usage.is_empty() {
                    if !context.without_usage_reported {
                        context.without_usage_reported = true;
                        diagnostics.push(diag(
                            "assistant_message_without_usage",
                            Some("usage"),
                            raw.number,
                            "usage object without any of the four documented fields; no event",
                        ));
                    }
                    continue;
                }
                if unknown_keys && !context.unmapped_keys_reported {
                    context.unmapped_keys_reported = true;
                    diagnostics.push(diag(
                        "unmapped_usage_keys",
                        Some("usage"),
                        raw.number,
                        "usage object carries keys beyond the documented four; mapped fields kept",
                    ));
                }
                // Import streaming/final samples in the same attempt immediately as Partial. Sample ordinal plus
                // revision_floor gives higher revisions, so Replace removes earlier contributions.
                context.sample_ordinal += 1;
                let revision = context.revision_floor + context.sample_ordinal;
                let key = context.attempt_key(&target.file_identity);
                if !current_keys.contains(&key) {
                    current_keys.push(key.clone());
                }
                events.push(sample_event(
                    target,
                    &context,
                    &key,
                    &usage,
                    revision,
                    Lifecycle::Partial,
                ));
                context.revision_floor = revision;
                context.pending = Some(PendingSample { revision, usage });
            }
            "request/context" => {
                // contextPressure (pressureTokens/projectedTokens/contextWindow)
                // is estimated context size; exclude it from usage and diagnose once.
                if !context.estimate_excluded_reported {
                    context.estimate_excluded_reported = true;
                    diagnostics.push(diag(
                        "context_estimate_not_counted",
                        Some("request/context"),
                        raw.number,
                        "contextPressure fields are estimates/projections; excluded from usage",
                    ));
                }
            }
            "request/header" | "image/offload" => {
                // Skip documented events without usage meaning.
            }
            other => {
                fail_closed = Some((
                    raw.number,
                    format!("event type {other:?} not in documented set"),
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
    // After a complete rescan, previously imported attempts missing from the rebuilt set
    // receive Corrected/Excluded records to remove old contributions and avoid counting rewritten files twice.
    // Update tracked keys only at end-of-file; interrupted bounded reads do not identify removals.
    let mut tracked_update = None;
    if status == ScanStatus::Complete {
        let tracked: std::collections::BTreeSet<String> =
            context.tracked_keys.iter().cloned().collect();
        let current: std::collections::BTreeSet<String> = current_keys.iter().cloned().collect();
        for vanished in tracked.difference(&current) {
            diagnostics.push(DiagnosticInput {
                event_id: None,
                code: "attempt_vanished_after_rewrite".to_string(),
                field: None,
                position: Some(vanished.clone()),
                message: "attempt key absent from rewritten log; tombstoned Corrected/Excluded"
                    .to_string(),
            });
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: vanished.clone(),
                record_kind: RecordKind::ModelCall,
                schema_version: DSH_FORMAT_VERSION.to_string(),
                parser_version: DSH_PARSER_VERSION.to_string(),
                parse_basis: Some(VersionBasis::KnownVersion),
                origin_call_id: None,
                attempt_id: None,
                session_id: None,
                parent_session_id: None,
                host_application: None,
                agent: "deepseek-harness".to_string(),
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
                    "attempt absent from rewritten log; tombstoned Corrected/Excluded".to_string(),
                ),
                cost: None,
            });
        }
        tracked_update = Some(current_keys);
    }
    if let Some(keys) = tracked_update {
        context.tracked_keys = keys;
    }
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
    // Reject undocumented types: clear this round's events, retain the checkpoint, and reject again next round.
    let (cursor_out, context_out) = if let Some((line_no, detail)) = fail_closed {
        diagnostics.push(diag(
            "undocumented_event_type",
            Some("type"),
            line_no,
            &detail,
        ));
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
    fn parse_usage_optional_fields() {
        let full = serde_json::json!({
            "uncachedInputTokens": 75,
            "outputTokens": 12,
            "cacheReadTokens": 20,
            "cacheWriteTokens": 5
        });
        let (usage, unknown) = parse_usage(&full).unwrap();
        assert_eq!(usage.uncached_input_tokens, Some(75));
        assert_eq!(usage.cache_read_tokens, Some(20));
        assert!(!unknown);

        let partial = serde_json::json!({"uncachedInputTokens": 10, "outputTokens": 2});
        let (usage, _) = parse_usage(&partial).unwrap();
        assert_eq!(usage.cache_write_tokens, None, "missing stays unknown");
        assert!(usage.output_tokens.is_some());

        assert!(parse_usage(&serde_json::json!({"uncachedInputTokens": -1})).is_none());
        assert!(parse_usage(&serde_json::json!("not an object")).is_none());
        let extra = serde_json::json!({"uncachedInputTokens": 1, "reasoningTokens": 3});
        let (_, unknown) = parse_usage(&extra).unwrap();
        assert!(
            unknown,
            "unevidenced extra keys flagged, mapped fields kept"
        );
    }

    #[test]
    fn old_parse_context_defaults_restore() {
        // Old or empty contexts restore with revision_floor=0 and an empty tracked-key set.
        let ctx: DshParseContext = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(ctx.revision_floor, 0);
        assert!(ctx.tracked_keys.is_empty());
        assert!(ctx.pending.is_none());
    }
}
