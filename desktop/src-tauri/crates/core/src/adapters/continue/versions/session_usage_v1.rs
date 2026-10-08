//! Continue session-file implementation (`session_usage_v1`, continue-session-usage-1).
//!
//! References: continuedev/continue commit 5522c6f44ca0ac3528b37244818fbfa39b5af470,
//! inspected upstream source and real local-model samples from official CLI 1.5.47.
//! - $CONTINUE_GLOBAL_DIR, default ~/.continue, contains sessions/&lt;uuidv4&gt;.json:
//!   rewritten JSON {sessionId, title, workspaceDirectory, history[], usage?}.
//! - Only the CLI writes top-level usage (extensions/cli session.ts:149-178).
//!   trackUsage accumulates and persists promptTokens/completionTokens,
//!   promptTokensDetails?{cachedTokens?, cacheWriteTokens?} and totalCost after each request.
//!   VS Code/JetBrains GUI streams stay in memory; missing usage supplies no data.
//!   dev_data/devdata.sqlite tokens_generated is a local tokenizer estimate
//!   and is excluded, as are all tokenizer-estimate paths.
//! - usage is a session cumulative value, not per-turn details; API request values stay in memory.
//!   Store a session IntervalAggregate without inventing per-call events.
//! - Cache inclusion differs by provider: OpenAI cached is within prompt; Anthropic separates it.
//!   The source does not distinguish these semantics; retain separate fields without deriving totals.
//!   CLI cache buckets start at zero and add only nonzero values; zero remains unknown.
//! - Sessions have no embedded timestamp. sessions.json dateCreated differs by client:
//!   core uses millisecond strings and CLI uses ISO. Use file mtime as the uncertain interval endpoint;
//!   do not read the inconsistent index.
//! - The JetBrains plugin hardcodes ~/.continue and ignores CONTINUE_GLOBAL_DIR.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

pub const CONTINUE_PARSER_VERSION: &str = "continue-session-usage-2";
pub const CONTINUE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Only the old parser's initialized cache zeroes may be replaced by unknown.
/// The hash covers identity, every token/quality, interval, coverage and revision.
pub(crate) fn prior_cache_hashes(input: &SourceAggregateInput) -> Vec<String> {
    if input.scope != AggregateScope::Session || !input.scope_key.starts_with("continue:session:") {
        return Vec::new();
    }
    let mut hashes = Vec::new();
    for mask in 1..=3 {
        let mut prior = input.clone();
        if mask & 1 != 0 {
            if input.usage.input_cache_read.is_some()
                || input.quality.input_cache_read != Q::Unknown
            {
                continue;
            }
            prior.usage.input_cache_read = Some(0);
            prior.quality.input_cache_read = Q::Reported;
        }
        if mask & 2 != 0 {
            if input.usage.input_cache_write.is_some()
                || input.quality.input_cache_write != Q::Unknown
            {
                continue;
            }
            prior.usage.input_cache_write = Some(0);
            prior.quality.input_cache_write = Q::Reported;
        }
        hashes.push(crate::identity::content_hash(&prior));
    }
    hashes
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
}

fn diag(code: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some("document".to_string()),
        message: message.to_string(),
    }
}

fn session_id_of(path: &std::path::Path) -> Option<String> {
    path.file_stem()?.to_str().map(str::to_string)
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    if target.probe.len > CONTINUE_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "session JSON exceeds the 64 MiB cap; cursor held",
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
        CONTINUE_MAX_FILE_BYTES + 1,
    )
    .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "session_unparseable",
                    "session JSON does not parse (mid-write?); retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let Some(session_id) = document
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| session_id_of(&target.path))
    else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "session_schema_deviation",
                "sessionId missing; not a Continue session file",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    // A GUI session without usage completes with no data; never substitute zero.
    let Some(usage) = document.get("usage").and_then(|v| v.as_object()) else {
        return Ok(ScanOutcome {
            status: ScanStatus::Complete,
            cursor: Some(serde_json::to_value(WholeFileCursor {
                generation: target.generation,
                offset: bytes.len() as u64,
                line_number: 1,
            })?),
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: Vec::new(),
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    };
    let get = |key: &str| -> Option<Option<i64>> {
        match usage.get(key) {
            None => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                Some(
                    (0..=crate::domain::MAX_TOKEN_VALUE)
                        .contains(&n)
                        .then_some(n),
                )
            }
        }
    };
    let (Some(prompt), Some(completion)) = (get("promptTokens"), get("completionTokens")) else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "token_shape_deviation",
                "usage token field carries an out-of-range value; fail closed",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let details = usage.get("promptTokensDetails").and_then(|v| v.as_object());
    let mut detail_deviation = false;
    let detail = |key: &str, deviation: &mut bool| {
        match details.and_then(|d| d.get(key)) {
            None => None,
            Some(v) => match v.as_i64() {
                // CLI initializes both cache fields to zero and only adds nonzero
                // provider values: zero does not establish a provider-reported value.
                Some(0) => None,
                Some(n) if (1..=crate::domain::MAX_TOKEN_VALUE).contains(&n) => Some(n),
                // Diagnose out-of-range components and leave them unknown while retaining other valid fields.
                _ => {
                    *deviation = true;
                    None
                }
            },
        }
    };
    let cached = detail("cachedTokens", &mut detail_deviation);
    let cache_write = detail("cacheWriteTokens", &mut detail_deviation);
    if prompt.is_none() && completion.is_none() {
        return Ok(ScanOutcome {
            status: ScanStatus::Complete,
            cursor: Some(serde_json::to_value(WholeFileCursor {
                generation: target.generation,
                offset: bytes.len() as u64,
                line_number: 1,
            })?),
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: Vec::new(),
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    }
    let usage_tokens = TokenUsage {
        input_uncached: None,
        input_cache_read: cached,
        input_cache_write: cache_write,
        input_total: prompt,
        output_total: completion,
        output_reasoning: None,
        total_tokens: None,
        source_total: None,
    };
    let quality = TokenQuality {
        input_cache_read: cached.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_write: cache_write.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_total: prompt.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: completion.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        ..Default::default()
    };
    // Validate mtime before using it as a revision; keep raw interval_end_ms for aggregate
    // validation, which rejects and diagnoses implausible endpoints.
    let mtime_ms = (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&target.probe.mtime_ms)
        .then_some(target.probe.mtime_ms);
    let mut diagnostics = Vec::new();
    if detail_deviation {
        diagnostics.push(diag(
            "token_shape_deviation",
            "promptTokensDetails sub-field out of range; kept unknown",
        ));
    }
    let aggregate = SourceAggregateInput {
        instance_id: target.instance_id.clone(),
        scope: AggregateScope::Session,
        scope_key: format!("continue:session:{session_id}"),
        interval_start_ms: None,
        interval_end_ms: target.probe.mtime_ms,
        interval_end_inclusive: true,
        usage: usage_tokens,
        quality,
        reported_call_count: None,
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::Uncertain,
        source_revision: mtime_ms,
    };
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: None,
        events: Vec::new(),
        aggregates: vec![aggregate],
        diagnostics,
        lines_read: 1,
        records_seen: 1,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_from_path() {
        assert_eq!(
            session_id_of(std::path::Path::new(
                "/home/u/.continue/sessions/0b6c3a2e-1111-2222-3333-444455556666.json"
            ))
            .as_deref(),
            Some("0b6c3a2e-1111-2222-3333-444455556666")
        );
    }
}
