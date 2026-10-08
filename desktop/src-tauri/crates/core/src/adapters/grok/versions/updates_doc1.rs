//! Grok updates.jsonl implementation updates_doc1; document format grok-updates-doc-1.
//!
//! Reference: third-party tokscale commit
//! 1d9a9395418efc6952944b794097935d7d6fa1e8, sessions/grok.rs, for the closed-source xAI product.
//! Original inspection had no local installation/native samples; this reference is not native acceptance.
//! - $GROK_HOME, default ~/.grok, contains
//!   sessions/<workspace>/<session>/updates.jsonl JSON-RPC lines.
//! - Read only explicit params.update.usage objects, as required by the adapter matrix.
//!   Aliases from grok.rs:180-253: input=inputTokens|input_tokens|promptTokens;
//!   output=outputTokens|output_tokens|completionTokens;
//!   cache_read=cachedReadTokens|cacheReadTokens|cache_read_input_tokens;
//!   cache_write=cachedWriteTokens|cacheWriteTokens|cacheCreationTokens|
//!   cache_creation_input_tokens; reasoning=reasoningTokens|thoughtTokens|
//!   thinkingTokens. The reference also lists totalTokens|total_tokens, which this reader does not map.
//!   Model lookup: params.update._meta.modelId, params._meta.modelId, params.modelId,
//!   params.model_id, modelId, model, then the first modelUsage map key.
//!   Time: params._meta.agentTimestampMs, params.update._meta.agentTimestampMs,
//!   params.timestamp, timestamp or ts; numeric milliseconds or RFC3339 strings.
//! - Exclude inferred cumulative totalTokens deltas such as _meta.totalTokens,
//!   signals.json compaction differences, unified.jsonl subagent PID attribution
//!   and events.jsonl/summary.json totals.
//! - The reference assumes cachedRead within inputTokens and reasoning within outputTokens.
//!   Do not use its subtraction mapping; keep separate fields without deriving totals.
//! - params._meta.eventId can repeat; include the file line number in the key,
//!   following tokscale grok:&lt;session&gt;:usage:&lt;index&gt;:&lt;eventId&gt;.

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

use super::GROK_FORMAT_VERSION;

pub const GROK_PARSER_VERSION: &str = "grok-updates-doc1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct GrokParseContext {
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

fn diag(code: &str, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(format!("line:{line}")),
        message: message.to_string(),
    }
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

fn session_id_of(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-session")
        .to_string()
}

fn alias_token(
    obj: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<Option<i64>> {
    for key in keys {
        match obj.get(*key) {
            None => continue,
            Some(v) => {
                let n = v.as_i64()?;
                // Invalid values return None so the caller diagnoses and skips the line;
                // absent keys return Some(None) and remain distinguishable from invalid fields.
                if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                    return None;
                }
                return Some(Some(n));
            }
        }
    }
    Some(None)
}

/// Timestamp: numeric milliseconds within range, or an RFC3339 string.
fn any_ts(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    match value {
        serde_json::Value::Number(n) => {
            let ms = n.as_i64()?;
            (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
                .contains(&ms)
                .then_some(ms)
        }
        serde_json::Value::String(s) => {
            let ts: jiff::Timestamp = s.trim().parse().ok()?;
            let ms = ts.as_millisecond();
            (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
                .contains(&ms)
                .then_some(ms)
        }
        _ => None,
    }
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<GrokParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = GrokParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let session_id = session_id_of(&target.path);
    let read = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    for line in &read.lines {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let Ok(value) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&line.text)
        else {
            diagnostics.push(diag("invalid_json_line", line.number, "line is not JSON"));
            continue;
        };
        // Read explicit params.update.usage.
        let Some(usage) = value
            .pointer("/params/update/usage")
            .and_then(|v| v.as_object())
        else {
            continue;
        };
        let input = alias_token(usage, &["inputTokens", "input_tokens", "promptTokens"]);
        let output = alias_token(
            usage,
            &["outputTokens", "output_tokens", "completionTokens"],
        );
        let cache_read = alias_token(
            usage,
            &[
                "cachedReadTokens",
                "cacheReadTokens",
                "cache_read_input_tokens",
            ],
        );
        let cache_write = alias_token(
            usage,
            &[
                "cachedWriteTokens",
                "cacheWriteTokens",
                "cacheCreationTokens",
                "cache_creation_input_tokens",
            ],
        );
        let reasoning = alias_token(
            usage,
            &["reasoningTokens", "thoughtTokens", "thinkingTokens"],
        );
        if [
            input.is_none(),
            output.is_none(),
            cache_read.is_none(),
            cache_write.is_none(),
            reasoning.is_none(),
        ]
        .iter()
        .any(|v| *v)
        {
            diagnostics.push(diag(
                "token_shape_deviation",
                line.number,
                "a usage alias carries a negative/out-of-range value; line skipped",
            ));
            continue;
        }
        let (input, output, cache_read, cache_write, reasoning) = (
            input.unwrap(),
            output.unwrap(),
            cache_read.unwrap(),
            cache_write.unwrap(),
            reasoning.unwrap(),
        );
        if input.is_none()
            && output.is_none()
            && cache_read.is_none()
            && cache_write.is_none()
            && reasoning.is_none()
        {
            continue;
        }
        // Select among timestamp paths.
        let ts_value = [
            value.pointer("/params/_meta/agentTimestampMs"),
            value.pointer("/params/update/_meta/agentTimestampMs"),
            value.pointer("/params/timestamp"),
            value.get("timestamp"),
            value.get("ts"),
        ]
        .into_iter()
        .find_map(any_ts);
        let Some(occurred_ms) = ts_value else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "no plausible timestamp alias; line skipped",
            ));
            continue;
        };
        // Select model paths, then the first modelUsage map key.
        let model = [
            value.pointer("/params/update/_meta/modelId"),
            value.pointer("/params/_meta/modelId"),
            value.pointer("/params/modelId"),
            value.pointer("/params/model_id"),
            value.get("modelId"),
            value.get("model"),
        ]
        .into_iter()
        .find_map(|v| v.and_then(|v| v.as_str()).map(str::to_string))
        .or_else(|| {
            usage
                .get("modelUsage")
                .and_then(|v| v.as_object())
                .and_then(|m| m.keys().next().cloned())
        });
        let event_id = value
            .pointer("/params/_meta/eventId")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: None,
                input_cache_read: cache_read,
                input_cache_write: cache_write,
                input_total: input,
                output_total: output,
                output_reasoning: reasoning,
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
                output_reasoning: crate::domain::FieldQuality::Reported,
                ..Default::default()
            },
            Vec::new(),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            // eventId is reused; include the line number, matching the referenced tokscale key.
            source_record_key: format!("grok:{session_id}:usage:{}:{event_id}", line.number),
            record_kind: RecordKind::ModelCall,
            schema_version: GROK_FORMAT_VERSION.to_string(),
            parser_version: GROK_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: Some(format!("grok-event:{event_id}")),
            attempt_id: None,
            session_id: Some(session_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "grok".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: model,
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
    let status = match read.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, offset } => {
            diagnostics.push(diag(
                "line_exceeds_cap",
                number,
                &format!("line at byte {offset} exceeds the cap; cursor held for retry"),
            ));
            ScanStatus::LineTooLong
        }
    };
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(JsonlCursor {
            generation: target.generation,
            offset: read.next_offset,
            line_number: read.next_line_number,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: read.lines.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_number_and_rfc3339() {
        assert_eq!(
            any_ts(Some(&serde_json::json!(1_780_000_000_000i64))),
            Some(1_780_000_000_000)
        );
        assert_eq!(any_ts(Some(&serde_json::json!(0))), None);
        assert_eq!(
            any_ts(Some(&serde_json::json!("2026-06-01T00:00:00Z"))),
            Some(1_780_272_000_000)
        );
        assert_eq!(any_ts(Some(&serde_json::json!(true))), None);
    }

    #[test]
    fn aliases() {
        let obj: serde_json::Map<String, serde_json::Value> =
            crate::adapters::run_policy::json_from_str(
                r#"{"inputTokens": 9, "cache_creation_input_tokens": 2}"#,
            )
            .unwrap();
        assert_eq!(alias_token(&obj, &["inputTokens", "input"]), Some(Some(9)));
        assert_eq!(
            alias_token(&obj, &["cachedWriteTokens", "cache_creation_input_tokens"]),
            Some(Some(2))
        );
        assert_eq!(alias_token(&obj, &["reasoningTokens"]), Some(None));
    }
}
