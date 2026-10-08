//! ZCode model-io JSONL parser (modelio_v1).
//!
//! Format references: M0 test data, tests/fixtures/zcode/real-*, and local ZCode 3.14.3 reads.
//! - Path: ~/.zcode/cli/rollout/model-io-<sessionId>.jsonl, append-only JSONL
//!   with one record per model call; no documented environment override.
//! - Records: type="model_io", attempt, sessionId, requestId, turnId, traceId,
//!   querySource in {main_turn, subagent, session_title}; native db.model_usage separately
//!   confirms session_title. model{modelId,providerId}, startedAt/completedAt
//!   as ISO8601 UTC millisecond strings, durationMs, and request.headers["x-zcode-app-version"].
//! - Two usage views have different field meanings; select one without summing them:
//!   - Primary AI SDK camelCase response.usage: inputTokens includes cache reads;
//!     outputTokens, totalTokens, cacheReadTokens, and cacheWriteTokens.
//!   - Anthropic snake_case response.providerMetadata.anthropic.usage:
//!     input_tokens excludes cache; output_tokens, cache_read_input_tokens,
//!     and optional cache_creation_input_tokens. Cache creation may also appear as
//!     providerMetadata.anthropic.cacheCreationInputTokens; null means unreported.
//!
//! When both views exist, compare total input (in+cr+cw) and output for each record.
//! Report dual_caliber_mismatch and keep the primary AI SDK view on disagreement. When
//! only the Anthropic view exists, use it with a diagnostic; never add both views.
//! - An unfinished tail with response.finishReason=null and no usage/providerMetadata
//!   produces no event and no failure.
//! - Missing requestId uses seq:{sessionId}:{line number} with a diagnostic, avoiding
//!   a shared zcode:None key; see the ZCode row in adapters.md.
//!
//! V17 rejects an entire file containing an undocumented type other than model_io:
//! clear events, preserve the cursor, and reject again on the next scan. Native model-io
//! files contained only model_io; do not silently ignore a changed record format.
//!
//! Version policy (architecture.md#unknown-version): the version field
//! x-zcode-app-version is selected through super::super::versions::select. Unlisted or absent
//! versions try this latest parser and retain event parse_basis markers;
//! an unlisted version alone does not reject the file.

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::zcode::common::{
    map_zcode_ai_sdk, map_zcode_anthropic, ZcodeAiSdkUsage, ZcodeAnthropicUsage,
};

pub const ZCODE_PARSER_VERSION: &str = "zcode-modelio-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// Values below this epoch threshold use seconds; defensive rule tested by synthetic-epoch-timestamps.
const EPOCH_SECONDS_THRESHOLD: i64 = 100_000_000_000;

/// Observed db.model_usage querySource values mapped to call categories:
/// main_turn / subagent / session_title, as verified in local reads.
fn map_query_source(source: Option<&str>) -> (CallCategory, bool) {
    match source {
        Some("main_turn") => (CallCategory::Primary, false),
        Some("subagent") => (CallCategory::SubAgent, false),
        Some("session_title") => (CallCategory::Auxiliary, false),
        _ => (CallCategory::Unknown, true),
    }
}

/// Persisted parsing context for one-time diagnostics and version selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ZcodeParseContext {
    #[serde(default)]
    unmapped_query_source_reported: bool,
    /// Version basis (known_version / latest_fallback); old contexts default to None.
    /// V30 migration preserves sources and cursors.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> ZcodeParseContext {
    if rescan {
        return ZcodeParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ZcodeParseContext>(v.clone()).ok())
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

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// Present numeric fields must be bounded nonnegative integers; return None on invalid data for caller diagnostics.
fn token_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<Option<i64>> {
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
}

/// AI SDK camelCase usage requires inputTokens/outputTokens;
/// cacheRead/cacheWrite/total/reasoning are optional. An invalid present field returns None.
fn parse_ai_sdk_usage(value: &serde_json::Value) -> Option<ZcodeAiSdkUsage> {
    let obj = value.as_object()?;
    Some(ZcodeAiSdkUsage {
        input_tokens: token_field(obj, "inputTokens")??,
        cached_input_tokens: token_field(obj, "cacheReadTokens")?,
        cache_creation_input_tokens: token_field(obj, "cacheWriteTokens")?,
        output_tokens: token_field(obj, "outputTokens")??,
        reasoning_tokens: token_field(obj, "reasoningTokens")?,
        total_tokens: token_field(obj, "totalTokens")?,
    })
}

/// Anthropic snake_case usage, with camelCase cacheCreationInputTokens fallback.
/// anthropic_parent is the providerMetadata.anthropic object.
fn parse_anthropic_usage(
    usage: &serde_json::Value,
    anthropic_parent: &serde_json::Value,
) -> Option<ZcodeAnthropicUsage> {
    let obj = usage.as_object()?;
    let creation = token_field(obj, "cache_creation_input_tokens")?.or_else(|| {
        anthropic_parent
            .get("cacheCreationInputTokens")
            .and_then(|v| v.as_i64())
            .filter(|&n| (0..=MAX_REASONABLE_TOKEN).contains(&n))
    });
    Some(ZcodeAnthropicUsage {
        input_tokens: token_field(obj, "input_tokens")??,
        cache_read_input_tokens: token_field(obj, "cache_read_input_tokens")?,
        cache_creation_input_tokens: creation,
        output_tokens: token_field(obj, "output_tokens")??,
    })
}

/// Native timestamps are ISO8601 millisecond strings; defensive numeric inputs use seconds below 1e11.
/// Return (milliseconds, original timestamp text).
fn parse_ts(value: &serde_json::Value) -> Option<(i64, String)> {
    match value {
        serde_json::Value::String(s) => s
            .parse::<jiff::Timestamp>()
            .ok()
            .map(|t| (t.as_millisecond(), s.clone())),
        serde_json::Value::Number(n) => {
            let v = n.as_i64()?;
            let ms = if v < EPOCH_SECONDS_THRESHOLD {
                v.checked_mul(1000)?
            } else {
                v
            };
            Some((ms, n.to_string()))
        }
        _ => None,
    }
}

/// Per-record version field shared by detection and event schema_version.
pub fn version_anchor(line: &serde_json::Value) -> Option<&str> {
    line.get("request")?
        .get("headers")?
        .get("x-zcode-app-version")?
        .as_str()
}

/// Incrementally scan one model-io JSONL file, called by ZcodeAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // Detection and scanning use the same version registry. Persist the first encountered record's
    // parse basis across incremental rounds; missing version selects latest_fallback (V17/V30).
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String, &'static str)> = None;
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
        if record_type != "model_io" {
            // Undocumented record types reject the file: clear events, preserve the cursor, and reject again next round.
            fail_closed = Some((
                raw.number,
                format!("record type {record_type:?} not model_io"),
                "undocumented_record_type",
            ));
            break;
        }
        // Persist the registry's parse basis on the first encountered version field.
        let anchor = version_anchor(&line);
        if context.version_basis.is_none() {
            let selection = super::super::versions::select(anchor);
            context.version_basis = Some(selection.basis);
        }
        // Read both usage views.
        let response = line
            .get("response")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let ai_sdk_value = response.get("usage").filter(|u| u.is_object());
        let anthropic_parent = response
            .get("providerMetadata")
            .and_then(|pm| pm.get("anthropic"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let anthropic_value = anthropic_parent.get("usage").filter(|u| u.is_object());
        // A tail without either usage view produces no event and no failure.
        if ai_sdk_value.is_none() && anthropic_value.is_none() {
            continue;
        }
        // Prefer completedAt, then startedAt if missing or unparseable; skip when neither is usable.
        let (occurred_ms, source_time) = match line
            .get("completedAt")
            .and_then(parse_ts)
            .or_else(|| line.get("startedAt").and_then(parse_ts))
        {
            Some(pair) => pair,
            None => {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    Some("completedAt"),
                    raw.number,
                    "record timestamps missing or unparseable; record skipped",
                ));
                continue;
            }
        };
        // Parse usage and select exactly one view.
        let mapped = match ai_sdk_value {
            Some(value) => {
                let Some(sdk) = parse_ai_sdk_usage(value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("response.usage"),
                        raw.number,
                        "AI SDK usage missing required numeric fields or out of range; record skipped",
                    ));
                    continue;
                };
                // Compare both usage views when the comparison view exists; diagnose differences and keep the primary view.
                if let Some(anth_usage) =
                    anthropic_value.and_then(|v| parse_anthropic_usage(v, &anthropic_parent))
                {
                    let anth_input = anth_usage
                        .input_tokens
                        .saturating_add(anth_usage.cache_read_input_tokens.unwrap_or(0))
                        .saturating_add(anth_usage.cache_creation_input_tokens.unwrap_or(0));
                    if anth_input != sdk.input_tokens
                        || anth_usage.output_tokens != sdk.output_tokens
                    {
                        diagnostics.push(diag(
                            "dual_caliber_mismatch",
                            Some("response.usage"),
                            raw.number,
                            &format!(
                                "anthropic view in+cr+cw={} out={} != AI SDK inputTokens={} outputTokens={}; AI SDK caliber kept",
                                anth_input, anth_usage.output_tokens, sdk.input_tokens, sdk.output_tokens
                            ),
                        ));
                    }
                }
                map_zcode_ai_sdk(&sdk)
            }
            None => {
                // When AI SDK usage is absent, use Anthropic usage and record a diagnostic.
                let anth_usage =
                    anthropic_value.and_then(|v| parse_anthropic_usage(v, &anthropic_parent));
                let Some(anth) = anth_usage else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("providerMetadata.anthropic.usage"),
                        raw.number,
                        "anthropic usage missing required numeric fields or out of range; record skipped",
                    ));
                    continue;
                };
                diagnostics.push(diag(
                    "ai_sdk_usage_missing",
                    Some("response.usage"),
                    raw.number,
                    "response.usage absent; exclusive fallback to anthropic view (never summed)",
                ));
                map_zcode_anthropic(&anth)
            }
        };
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                raw.number,
                &contradiction.detail,
            ));
        }
        // Map observed querySource values; diagnose unlisted values once and leave their category unknown.
        let query_source = json_str(&line, "querySource");
        let (category, unmapped) = map_query_source(query_source);
        if unmapped && !context.unmapped_query_source_reported {
            context.unmapped_query_source_reported = true;
            diagnostics.push(diag(
                "unmapped_query_source",
                Some("querySource"),
                raw.number,
                "querySource value outside evidenced set {main_turn, subagent, session_title}; classified unknown",
            ));
        }
        // Identity: zcode:{requestId}:{attempt}; missing requestId uses seq:{sessionId}:{line number}.
        let session_id = json_str(&line, "sessionId");
        let attempt = line.get("attempt").and_then(|v| v.as_i64()).unwrap_or(1);
        let (source_record_key, origin_call_id) = match json_str(&line, "requestId") {
            Some(rid) => (format!("zcode:{rid}:{attempt}"), Some(rid.to_string())),
            None => {
                diagnostics.push(diag(
                    "missing_request_id",
                    Some("requestId"),
                    raw.number,
                    "model_io without requestId; fallback identity session + line number",
                ));
                (
                    format!(
                        "seq:{}:{}",
                        session_id.unwrap_or("unknown-session"),
                        raw.number
                    ),
                    None,
                )
            }
        };
        let model = line
            .get("model")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let model_raw = json_str(&model, "modelId").map(str::to_string);
        let provider_id = json_str(&model, "providerId").map(str::to_string);
        let duration_ms = line
            .get("durationMs")
            .and_then(|v| v.as_i64())
            .filter(|&d| d >= 0);
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: anchor.unwrap_or("unknown").to_string(),
            parser_version: ZCODE_PARSER_VERSION.to_string(),
            parse_basis: context.version_basis,
            origin_call_id,
            attempt_id: Some(attempt.to_string()),
            session_id: session_id.map(str::to_string),
            parent_session_id: None,
            host_application: None,
            agent: "zcode".to_string(),
            call_category: category,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(source_time),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id,
            model_raw: model_raw.clone(),
            model_canonical: None,
            model_attribution: if model_raw.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
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
    let new_cursor = JsonlCursor {
        generation: target.generation,
        offset: outcome.next_offset,
        line_number: outcome.next_line_number,
    };
    let mut health_degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line"
                    | "usage_shape_deviation"
                    | "timestamp_unparseable"
                    | "line_too_long"
            )
        });
    // Reject the file: clear this round's events, retain the old checkpoint, and reject again next round.
    let (cursor_out, context_out) = if let Some((line_no, detail, code)) = fail_closed {
        diagnostics.push(diag(code, Some("type"), line_no, &detail));
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
    fn epoch_numbers_normalize_to_milliseconds() {
        let (ms1, raw1) = parse_ts(&serde_json::json!(1_800_000_000_000_i64)).unwrap();
        assert_eq!(ms1, 1_800_000_000_000);
        assert_eq!(raw1, "1800000000000");
        // Convert numeric seconds below 1e11 to milliseconds.
        let (ms2, _) = parse_ts(&serde_json::json!(1_800_000_000_i64)).unwrap();
        assert_eq!(ms2, 1_800_000_000_000);
        // ISO strings match the observed native format.
        let (ms3, raw3) = parse_ts(&serde_json::json!("2026-09-25T09:10:45.285Z")).unwrap();
        assert_eq!(raw3, "2026-09-25T09:10:45.285Z");
        assert_eq!(
            ms3,
            "2026-09-25T09:10:45.285Z"
                .parse::<jiff::Timestamp>()
                .unwrap()
                .as_millisecond()
        );
        assert!(parse_ts(&serde_json::json!(null)).is_none());
    }

    #[test]
    fn ai_sdk_usage_requires_input_output() {
        let full = serde_json::json!({
            "inputTokens": 2000, "outputTokens": 100, "totalTokens": 2100,
            "cacheReadTokens": 800, "cacheWriteTokens": 200
        });
        let sdk = parse_ai_sdk_usage(&full).unwrap();
        assert_eq!(sdk.cache_creation_input_tokens, Some(200));
        let missing = serde_json::json!({"inputTokens": 10});
        assert!(parse_ai_sdk_usage(&missing).is_none());
        // Negative values (synthetic-negative-usage) return None.
        let negative = serde_json::json!({
            "inputTokens": -5, "outputTokens": 10, "totalTokens": 5,
            "cacheReadTokens": 0, "cacheWriteTokens": 0
        });
        assert!(parse_ai_sdk_usage(&negative).is_none());
    }

    #[test]
    fn anthropic_creation_falls_back_to_camel_field() {
        let parent = serde_json::json!({
            "usage": {"input_tokens": 1000, "output_tokens": 100, "cache_read_input_tokens": 800},
            "cacheCreationInputTokens": 200
        });
        let anth = parse_anthropic_usage(&parent["usage"], &parent).unwrap();
        assert_eq!(anth.cache_creation_input_tokens, Some(200));
        // Prefer snake_case when present; null or absence in both native representations leaves None.
        let real = serde_json::json!({
            "usage": {
                "input_tokens": 139, "output_tokens": 120, "cache_read_input_tokens": 390976
            },
            "cacheCreationInputTokens": null
        });
        let anth = parse_anthropic_usage(&real["usage"], &real).unwrap();
        assert_eq!(anth.cache_creation_input_tokens, None);
    }

    #[test]
    fn query_source_maps_evidenced_values() {
        assert_eq!(
            map_query_source(Some("main_turn")),
            (CallCategory::Primary, false)
        );
        assert_eq!(
            map_query_source(Some("subagent")),
            (CallCategory::SubAgent, false)
        );
        assert_eq!(
            map_query_source(Some("session_title")),
            (CallCategory::Auxiliary, false)
        );
        assert_eq!(
            map_query_source(Some("mystery")),
            (CallCategory::Unknown, true)
        );
        assert_eq!(map_query_source(None), (CallCategory::Unknown, true));
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // Old contexts without version_basis deserialize with basis=None;
        // V30 directory migration preserves sources and cursors.
        let legacy = serde_json::json!({"unmapped_query_source_reported": true});
        let ctx: ZcodeParseContext = serde_json::from_value(legacy).expect("restore");
        assert!(ctx.unmapped_query_source_reported);
        assert_eq!(ctx.version_basis, None);
    }
}
