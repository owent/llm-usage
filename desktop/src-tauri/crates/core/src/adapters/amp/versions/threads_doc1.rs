//! Amp threads/T-*.json implementation threads_doc1; document format amp-threads-doc-1.
//!
//! Reference: third-party tokscale commit
//! 1d9a9395418efc6952944b794097935d7d6fa1e8, sessions/amp.rs, describing the closed-source
//! product under Sourcegraph. Original inspection had no local installation/native samples.
//! - ~/.local/share/amp/threads/T-*.json uses PathRoot::XdgData (clients.rs:463-472).
//!   Windows layout remains unverified; %LOCALAPPDATA%/amp/threads is a candidate
//!   checked by format detection.
//! - One JSON object per thread has id, created in Unix milliseconds, messages[]
//!   and usageLedger (amp.rs:64-71).
//! - usageLedger.events[] is the primary source (amp.rs:13-24), with RFC3339 timestamp,
//!   model, credits (billing units, not USD/cost) and tokens{input,
//!   output, cacheReadInputTokens, cacheCreationInputTokens},
//!   plus operationType and fromMessageId/toMessageId.
//! - Assistant messages[].usage (amp.rs:37-57) has messageId and
//!   usage{model,inputTokens,outputTokens,cacheReadInputTokens,
//!   cacheCreationInputTokens,credits}, without a timestamp.
//!
//! M8 reconciliation joins ledger toMessageId to messageId, avoiding duplicate usage.
//! Timestamped ledger events are imported; unmatched assistant usage has no timestamp.
//! The reference fabricates thread.created + messageId*1000 timing, which the matrix rejects:
//! skip those messages with diagnostics and a reconciliation comparison.
//! Credits are Amp billing units and do not map to CostAmount.
//! Inclusion among the four input/output/cache fields is unverified; keep separate without deriving totals.

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

use super::AMP_FORMAT_VERSION;

pub const AMP_PARSER_VERSION: &str = "amp-threads-doc1";
/// Bounded read limit per file.
pub const AMP_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
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

fn thread_id_of(path: &std::path::Path) -> Option<String> {
    path.file_name()?
        .to_str()?
        .strip_prefix("T-")?
        .strip_suffix(".json")
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn rfc3339_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let s: &str = value?.as_str()?;
    let ts: jiff::Timestamp = s.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

/// Four token fields: missing is None/unknown; invalid values reject the bucket set with caller diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FiveBuckets {
    input: Option<i64>,
    output: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
}

impl FiveBuckets {
    fn total(&self) -> i64 {
        [self.input, self.output, self.cache_read, self.cache_write]
            .iter()
            .filter_map(|v| *v)
            .sum()
    }
    fn is_empty(&self) -> bool {
        self.input.is_none()
            && self.output.is_none()
            && self.cache_read.is_none()
            && self.cache_write.is_none()
    }
}

fn opt_token(value: Option<&serde_json::Value>) -> Option<Option<i64>> {
    match value {
        None => Some(None),
        Some(v) => {
            let n = v.as_i64()?;
            Some((0..=MAX_REASONABLE_TOKEN).contains(&n).then_some(n))
        }
    }
}

/// Read ledger events[].tokens input/output/cacheReadInputTokens/cacheCreationInputTokens.
fn ledger_tokens(obj: &serde_json::Map<String, serde_json::Value>) -> Option<FiveBuckets> {
    let tokens = obj.get("tokens").and_then(|v| v.as_object())?;
    Some(FiveBuckets {
        input: opt_token(tokens.get("input"))?,
        output: opt_token(tokens.get("output"))?,
        cache_read: opt_token(tokens.get("cacheReadInputTokens"))?,
        cache_write: opt_token(tokens.get("cacheCreationInputTokens"))?,
    })
}

/// Read assistant messages[].usage inputTokens/outputTokens/cacheReadInputTokens/cacheCreationInputTokens.
fn message_usage(obj: &serde_json::Map<String, serde_json::Value>) -> Option<FiveBuckets> {
    let usage = obj.get("usage").and_then(|v| v.as_object())?;
    Some(FiveBuckets {
        input: opt_token(usage.get("inputTokens"))?,
        output: opt_token(usage.get("outputTokens"))?,
        cache_read: opt_token(usage.get("cacheReadInputTokens"))?,
        cache_write: opt_token(usage.get("cacheCreationInputTokens"))?,
    })
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let thread_id = thread_id_of(&target.path).unwrap_or_else(|| "unknown".to_string());
    let mut diagnostics = Vec::new();
    let mut events = Vec::new();
    if target.probe.len > AMP_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "thread JSON exceeds the 32 MiB cap; cursor held for controlled retry",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    crate::adapters::run_policy::checked_file(&target.path)?
        .take(AMP_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events,
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "thread_unparseable",
                    "document",
                    "thread JSON does not parse (mid-write or corrupt); retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    // Primary source: timestamped usageLedger.events.
    let mut ledger_matched_message_ids: std::collections::BTreeSet<i64> = Default::default();
    let mut ledger_sum: i64 = 0;
    let mut records_seen: u64 = 0;
    if let Some(ledger_events) = document
        .pointer("/usageLedger/events")
        .and_then(|v| v.as_array())
    {
        for (index, entry) in ledger_events.iter().enumerate() {
            records_seen += 1;
            let Some(obj) = entry.as_object() else {
                diagnostics.push(diag(
                    "ledger_shape_deviation",
                    &format!("events[{index}]"),
                    "ledger event is not an object; skipped",
                ));
                continue;
            };
            let Some(buckets) = ledger_tokens(obj) else {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    &format!("events[{index}]"),
                    "ledger tokens carry a negative/out-of-range value; skipped",
                ));
                continue;
            };
            if buckets.is_empty() {
                continue;
            }
            let Some(occurred_ms) = rfc3339_ms(obj.get("timestamp")) else {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    &format!("events[{index}]"),
                    "ledger timestamp missing/implausible; event skipped",
                ));
                continue;
            };
            if let Some(to_message) = obj.get("toMessageId").and_then(|v| v.as_i64()) {
                ledger_matched_message_ids.insert(to_message);
            }
            ledger_sum = ledger_sum.saturating_add(buckets.total());
            let model = obj.get("model").and_then(|v| v.as_str()).unwrap_or("");
            let mapped = crate::adapters::usage_map::finish(
                crate::domain::TokenUsage {
                    input_uncached: None,
                    input_cache_read: buckets.cache_read,
                    input_cache_write: buckets.cache_write,
                    input_total: buckets.input,
                    output_total: buckets.output,
                    output_reasoning: None,
                    total_tokens: None,
                    source_total: None,
                },
                // Present fields need Reported quality; Unknown would contradict their values
                // and domain.rs ingestion validation would reject the event.
                crate::domain::TokenQuality {
                    input_cache_read: crate::domain::FieldQuality::Reported,
                    input_cache_write: crate::domain::FieldQuality::Reported,
                    input_total: crate::domain::FieldQuality::Reported,
                    output_total: crate::domain::FieldQuality::Reported,
                    ..Default::default()
                },
                Vec::new(),
            );
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: format!("amp:{thread_id}:ledger:{index}"),
                record_kind: RecordKind::ModelCall,
                schema_version: AMP_FORMAT_VERSION.to_string(),
                parser_version: AMP_PARSER_VERSION.to_string(),
                parse_basis: Some(VersionBasis::KnownVersion),
                origin_call_id: None,
                attempt_id: None,
                session_id: Some(thread_id.clone()),
                parent_session_id: None,
                host_application: None,
                agent: "amp".to_string(),
                call_category: CallCategory::Primary,
                occurred_at_ms: occurred_ms,
                observed_at_ms: Some(now_ms),
                source_time: obj
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                time_basis: TimeBasis::SourceCompletion,
                interval_start_ms: None,
                interval_end_ms: None,
                provider_id: None,
                model_raw: (!model.is_empty()).then(|| model.to_string()),
                model_canonical: None,
                model_attribution: ModelAttribution::RequestField,
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
    }
    // Comparison source: untimestamped assistant messages[].usage, used only for reconciliation.
    let mut unmatched_message_usage: u64 = 0;
    let mut message_sum: i64 = 0;
    if let Some(messages) = document.get("messages").and_then(|v| v.as_array()) {
        for message in messages.iter() {
            let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                continue;
            }
            records_seen += 1;
            let Some(obj) = message.as_object() else {
                continue;
            };
            let Some(buckets) = message_usage(obj) else {
                continue;
            };
            if buckets.is_empty() {
                continue;
            }
            let message_id = obj.get("messageId").and_then(|v| v.as_i64());
            if message_id.is_some_and(|id| ledger_matched_message_ids.contains(&id)) {
                // A matching ledger event already supplied usage; skip this message to avoid duplicates.
                continue;
            }
            unmatched_message_usage += 1;
            message_sum = message_sum.saturating_add(buckets.total());
        }
    }
    let mut reconciliations = Vec::new();
    if unmatched_message_usage > 0 {
        diagnostics.push(diag(
            "message_usage_without_ledger",
            &thread_id,
            &format!(
                "{unmatched_message_usage} assistant usage blocks have no matching ledger event and no timestamp; not counted (fabricated timing rejected)"
            ),
        ));
        reconciliations.push(crate::adapters::framework::Reconciliation {
            series: "message_usage_vs_usage_ledger".to_string(),
            detail_sum: message_sum,
            snapshot_final: Some(ledger_sum),
            carried_sum: 0,
            difference: Some(message_sum - ledger_sum),
            verdict: "mismatch".to_string(),
        });
    }
    let health = if diagnostics.is_empty() {
        "active".to_string()
    } else {
        "degraded".to_string()
    };
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations,
        health,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_id_from_filename() {
        assert_eq!(
            thread_id_of(std::path::Path::new("/x/threads/T-abc123.json")).as_deref(),
            Some("abc123")
        );
        assert_eq!(thread_id_of(std::path::Path::new("/x/other.json")), None);
        // Empty thread-id suffix returns None; callers supply unknown instead of an empty key.
        assert_eq!(
            thread_id_of(std::path::Path::new("/x/threads/T-.json")),
            None
        );
    }

    #[test]
    fn buckets_optional_fields() {
        let obj: serde_json::Map<String, serde_json::Value> =
            crate::adapters::run_policy::json_from_str(
                r#"{"tokens": {"input": 7, "output": 3, "cacheReadInputTokens": 5}}"#,
            )
            .unwrap();
        let buckets = ledger_tokens(&obj).unwrap();
        assert_eq!(buckets.input, Some(7));
        assert_eq!(buckets.cache_read, Some(5));
        assert_eq!(buckets.cache_write, None);
        assert_eq!(buckets.total(), 15);
    }
}
