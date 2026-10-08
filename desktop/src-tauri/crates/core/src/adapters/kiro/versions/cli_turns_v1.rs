//! Kiro CLI session-header implementation (`cli_turns_v1`, kiro-cli-turns-1).
//!
//! Reference: tokscale 1d9a939 sessions/kiro.rs:48-113; closed source, originally inspected without local installation.
//! - ~/.kiro/sessions/cli/*.json has session_id, cwd,
//!   session_state.rts_model_state.model_info.{model_id, context_window_tokens}
//!   and session_state.conversation_metadata.user_turn_metadatas[].
//! - Turn fields include input_token_count/output_token_count,
//!   cache_read_input_token_count/cache_write_input_token_count,
//!   end_timestamp/total_request_count/metering_usage[]{value,unit:"credit"}.
//! - The reference labels Auto-agent counts "ESTIMATED, not measured" and commonly zero.
//!   Exclude bytes/4 and context_window-difference estimates; require at least one
//!   positive count field. Skip and diagnose turns whose counts are all absent or zero.
//! - The third-party parser does not specify end_timestamp units: >=1e11 is treated as ms,
//!   otherwise multiply seconds by 1000, as in its Crush parser; reject out-of-range values.
//! - Metering credits are pricing units; the third-party 0.04 USD/credit conversion is unverified.
//!   No cost is mapped.
//! - Same-stem .jsonl stores Prompt/AssistantMessage/ToolResults transcripts
//!   without usage and is not read by this implementation.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

use super::KIRO_FORMAT_VERSION;

pub const KIRO_CLI_PARSER_VERSION: &str = "kiro-cli-turns-1";
pub const KIRO_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

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

/// Magnitude heuristic: >=1e11 is milliseconds; otherwise seconds. Reject out-of-range results.
fn ts_to_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let n = value?.as_f64()?;
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    let ms = if n >= 1e11 { n } else { n * 1000.0 };
    if !((crate::domain::MIN_PLAUSIBLE_MS as f64)..=4_102_444_800_000.0).contains(&ms) {
        return None;
    }
    Some(ms.round() as i64)
}

/// Count result: Ok(Some(v)) is valid; Ok(None) is absent and unknown.
/// Err identifies invalid shape, distinguishing noninteger and out-of-range values for diagnostics.
fn count(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<i64>, &'static str> {
    match obj.get(key) {
        None => Ok(None),
        Some(v) => match v.as_i64() {
            Some(n) if (0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) => Ok(Some(n)),
            Some(_) => Err("out-of-range"),
            None => Err("non-integer"),
        },
    }
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut diagnostics = Vec::new();
    let mut events = Vec::new();
    if target.probe.len > KIRO_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "session header exceeds the 32 MiB cap; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    std::io::Read::take(
        &mut crate::adapters::run_policy::checked_file(&target.path)?,
        KIRO_MAX_FILE_BYTES + 1,
    )
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
                    "session_unparseable",
                    "document",
                    "session header does not parse; retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let doc_session_id = document.get("session_id").and_then(|v| v.as_str());
    // Missing session_id uses file identity for deduplication, keeping distinct files separate
    // instead of collapsing them into kiro:unknown:turn:N. Event session_id remains None.
    let session_key = doc_session_id.unwrap_or(target.file_identity.as_str());
    let model = document
        .pointer("/session_state/rts_model_state/model_info/model_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let Some(turns) = document
        .pointer("/session_state/conversation_metadata/user_turn_metadatas")
        .and_then(|v| v.as_array())
    else {
        return Ok(ScanOutcome {
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
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    };
    let mut records_seen: u64 = 0;
    let mut zero_turns: u64 = 0;
    for (index, turn) in turns.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let Some(obj) = turn.as_object() else {
            diagnostics.push(diag(
                "record_shape_deviation",
                &format!("turn:{index}"),
                "turn entry is not an object; turn skipped",
            ));
            continue;
        };
        let fields = [
            "input_token_count",
            "output_token_count",
            "cache_read_input_token_count",
            "cache_write_input_token_count",
        ];
        let mut values = [None, None, None, None];
        let mut deviation: Option<String> = None;
        for (slot, key) in fields.iter().enumerate() {
            match count(obj, key) {
                Ok(v) => values[slot] = v,
                Err(kind) => {
                    deviation = Some(format!("field {key} carries a {kind} value"));
                    break;
                }
            }
        }
        if let Some(reason) = deviation {
            diagnostics.push(diag(
                "token_shape_deviation",
                &format!("turn:{index}"),
                &format!("{reason}; turn skipped"),
            ));
            continue;
        }
        let [input, output, cache_read, cache_write] = values;
        // All counts absent/zero do not establish measured usage; skip Auto-agent defaults from the reference.
        if input.unwrap_or(0) == 0
            && output.unwrap_or(0) == 0
            && cache_read.unwrap_or(0) == 0
            && cache_write.unwrap_or(0) == 0
        {
            zero_turns += 1;
            continue;
        }
        let Some(occurred_ms) = ts_to_ms(obj.get("end_timestamp")) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("turn:{index}"),
                "end_timestamp missing/implausible; turn skipped",
            ));
            continue;
        };
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: None,
                input_cache_read: cache_read,
                input_cache_write: cache_write,
                input_total: input,
                output_total: output,
                output_reasoning: None,
                total_tokens: None,
                source_total: None,
            },
            // Present counts need Reported quality; default Unknown would contradict their values
            // and domain.rs ingestion validation would reject those events.
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
            source_record_key: format!("kiro:{session_key}:turn:{index}"),
            // One event represents turn-level counts; underlying requests are not reconstructed.
            record_kind: RecordKind::ModelCall,
            schema_version: KIRO_FORMAT_VERSION.to_string(),
            parser_version: KIRO_CLI_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: doc_session_id.map(str::to_string),
            parent_session_id: None,
            host_application: None,
            agent: "kiro".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: model.clone(),
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
            cost: None,
        });
        // total_request_count has no mapped event field and is excluded; see capability limits.
    }
    if zero_turns > 0 {
        diagnostics.push(diag(
            "zero_count_turns_skipped",
            session_key,
            &format!("{zero_turns} turns carry only zero/absent counts (Auto agent default); not adopted as evidence"),
        ));
    }
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
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnitude_disambiguation() {
        assert_eq!(
            ts_to_ms(Some(&serde_json::json!(1_790_000_000))),
            Some(1_790_000_000_000)
        );
        assert_eq!(
            ts_to_ms(Some(&serde_json::json!(1_790_000_000_000i64))),
            Some(1_790_000_000_000)
        );
        assert_eq!(ts_to_ms(Some(&serde_json::json!(0))), None);
    }
}
