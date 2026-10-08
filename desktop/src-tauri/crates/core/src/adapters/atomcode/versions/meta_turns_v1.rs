//! AtomCode session metadata implementation meta_turns_v1; format atomcode-meta-turns-1.
//!
//! References: AtomGit atomgit_atomcode/atomcode source commit
//! e4215f733eeba4cede553e28f9b559e6b3dc34ef, checked against the GitHub mirror;
//! and official npm 5.2.1 native .meta/API/CLI checks using a local model.
//! - $ATOMCODE_HOME (default ~/.atomcode)/sessions/<project_hash>/ stores
//!   <id>.meta SessionMeta JSON. The reader also accepts legacy <id>.json with messages
//!   and turn_stats (LegacyCatalogMeta); this does not establish native legacy acceptance.
//! - SessionMeta (manager.rs:376-430): schema v, id, working_dir,
//!   created_at/updated_at in Unix milliseconds, turn_stats[],
//!   detached_model_usage[] and detached_unattributed_tokens.
//! - TurnStat (manager.rs:634-668): turn_id, round_count (LLM round trips),
//!   duration_ms, total_tokens (last request prompt+completion, not cumulative),
//!   and model_usage[] entries {provider_id, model_id, tokens: TokenBreakdown}.
//!   TurnStat has no timestamp: store a session interval aggregate and summed
//!   reported_call_count, without inventing individual calls.
//! - TokenBreakdown (manager.rs:681-687 and usage_provider.rs:64-71):
//!   input = prompt - cached (uncached input); cached_input = min(cached,
//!   prompt) (cache read); output = completion. There is no cache-write field.
//!   The kernel has prompt/completion/cached. Complete valid sums may yield
//!   derived input_total = input + cached_input. Accepted uncached input and
//!   cache read retain native values; default zero or invalid fields remain unknown.
//! - Skip message <id>.jsonl and .snapshot/.todos/.rewind; usage comes from .meta.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

pub const ATOMCODE_PARSER_VERSION: &str = "atomcode-meta-turns-2";
pub const ATOMCODE_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Restore only the previous parser's default-zero buckets and their exact sums.
/// Full aggregate hashes keep identity, revision, interval, calls and coverage fixed.
pub(crate) fn prior_default_hashes(input: &SourceAggregateInput) -> Vec<String> {
    if input.scope != AggregateScope::Session || !input.scope_key.starts_with("atomcode:session:") {
        return Vec::new();
    }
    let mut hashes = std::collections::BTreeSet::new();
    for mask in 1..32 {
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
            if input.usage.input_uncached.is_some() || input.quality.input_uncached != Q::Unknown {
                continue;
            }
            prior.usage.input_uncached = input
                .usage
                .input_total
                .and_then(|n| n.checked_sub(prior.usage.input_cache_read.unwrap_or(0)))
                .or(Some(0))
                .filter(|n| *n >= 0);
            prior.quality.input_uncached = Q::Reported;
        }
        if mask & 4 != 0 {
            if input.usage.input_total.is_some() || input.quality.input_total != Q::Unknown {
                continue;
            }
            prior.usage.input_total = prior
                .usage
                .input_uncached
                .zip(prior.usage.input_cache_read)
                .and_then(|(i, c)| i.checked_add(c));
            if prior.usage.input_total.is_none() {
                continue;
            }
            prior.quality.input_total = Q::Derived;
        }
        if mask & 8 != 0 {
            if input.usage.output_total.is_some() || input.quality.output_total != Q::Unknown {
                continue;
            }
            prior.usage.output_total = Some(0);
            prior.quality.output_total = Q::Reported;
        }
        if mask & 16 != 0 {
            if input.usage.total_tokens.is_some() || input.quality.total_tokens != Q::Unknown {
                continue;
            }
            prior.usage.total_tokens = prior
                .usage
                .input_total
                .zip(prior.usage.output_total)
                .and_then(|(i, o)| i.checked_add(o));
            if prior.usage.total_tokens.is_none() {
                continue;
            }
            prior.quality.total_tokens = Q::Derived;
        }
        hashes.insert(crate::identity::content_hash(&prior));
    }
    hashes.into_iter().collect()
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

fn ms_field(value: Option<&serde_json::Value>) -> Option<i64> {
    let n = value?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&n)
        .then_some(n)
}

fn bucket(value: Option<&serde_json::Value>, key: &str) -> Option<i64> {
    let n = value?.get(key)?.as_i64()?;
    (0..=crate::domain::MAX_TOKEN_VALUE)
        .contains(&n)
        .then_some(n)
}

/// Accumulate one token bucket while tracking whether every value is known.
#[derive(Clone, Copy)]
struct BucketAccum {
    sum: i64,
    complete: bool,
}

impl Default for BucketAccum {
    fn default() -> Self {
        Self {
            sum: 0,
            complete: true,
        }
    }
}

impl BucketAccum {
    fn add(&mut self, value: Option<i64>) -> bool {
        let Some(value) = value else {
            self.complete = false;
            return false;
        };
        let Some(sum) = self
            .sum
            .checked_add(value)
            .filter(|n| *n <= crate::domain::MAX_TOKEN_VALUE)
        else {
            self.complete = false;
            return true;
        };
        self.sum = sum;
        false
    }

    fn positive(self) -> Option<i64> {
        (self.complete && self.sum > 0).then_some(self.sum)
    }
}

#[derive(Default, Clone, Copy)]
struct ModelAccum {
    input: BucketAccum,
    cached_input: BucketAccum,
    output: BucketAccum,
    rounds: i64,
    /// Track presence of a tokens value; presence or default zero does not establish reported usage.
    tokens_seen: bool,
}

impl ModelAccum {
    fn add_tokens(
        &mut self,
        tokens: Option<&serde_json::Value>,
        diagnostics: &mut Vec<DiagnosticInput>,
    ) {
        self.tokens_seen |= tokens.is_some();
        if tokens.is_some_and(|v| !v.is_object()) {
            diagnostics.push(diag(
                "token_shape_deviation",
                "native tokens carrier is not an object; kept unknown",
            ));
        }
        for (key, accum) in [
            ("input", &mut self.input),
            ("cached_input", &mut self.cached_input),
            ("output", &mut self.output),
        ] {
            let value = bucket(tokens, key);
            if tokens.and_then(|v| v.get(key)).is_some() && value.is_none() {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    "native token bucket has an invalid type or range; kept unknown",
                ));
            }
            if accum.add(value) {
                diagnostics.push(diag(
                    "token_sum_overflow",
                    "native token sum exceeds the supported range; kept unknown",
                ));
            }
        }
    }

    fn usage(self) -> (TokenUsage, TokenQuality) {
        let input_total = (self.input.complete && self.cached_input.complete)
            .then(|| self.input.sum.checked_add(self.cached_input.sum))
            .flatten()
            .filter(|n| (1..=crate::domain::MAX_TOKEN_VALUE).contains(n));
        let input_cache_read = self.cached_input.positive();
        let input_uncached =
            (input_total.is_some() && input_cache_read.is_some()).then_some(self.input.sum);
        let output_total = self.output.positive();
        let total_tokens = input_total
            .zip(output_total)
            .and_then(|(i, o)| i.checked_add(o))
            .filter(|n| *n <= crate::domain::MAX_TOKEN_VALUE);
        (
            TokenUsage {
                input_uncached,
                input_cache_read,
                input_total,
                output_total,
                total_tokens,
                ..Default::default()
            },
            TokenQuality {
                input_uncached: input_uncached.map(|_| Q::Reported).unwrap_or(Q::Unknown),
                input_cache_read: input_cache_read.map(|_| Q::Reported).unwrap_or(Q::Unknown),
                input_total: input_total.map(|_| Q::Derived).unwrap_or(Q::Unknown),
                output_total: output_total.map(|_| Q::Reported).unwrap_or(Q::Unknown),
                total_tokens: total_tokens.map(|_| Q::Derived).unwrap_or(Q::Unknown),
                ..Default::default()
            },
        )
    }
}

fn model_key(model: &serde_json::Value) -> (String, String) {
    (
        model
            .get("provider_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model
            .get("model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    )
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    if target.probe.len > ATOMCODE_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "session meta exceeds the 16 MiB cap; cursor held",
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
        ATOMCODE_MAX_FILE_BYTES + 1,
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
                    "meta_unparseable",
                    "session meta does not parse; retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let session_id = document
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let start_ms = ms_field(document.get("created_at"));
    let end_ms = ms_field(document.get("updated_at")).or(start_ms);
    let mut diagnostics = Vec::new();
    let Some(end_ms) = end_ms else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "timestamp_unparseable",
                "created_at/updated_at missing/implausible; fail closed",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let mut records_seen: u64 = 0;
    // Sum turn_stats[] and detached_model_usage[] by (provider, model).
    let mut acc: std::collections::BTreeMap<(String, String), ModelAccum> = Default::default();
    // Round counts from multiple-model turns or turns without model_usage have no single model
    // attribution. Keep them separately rather than assuming every model made all round trips.
    let mut unattributed_rounds: i64 = 0;
    if let Some(turns) = document.get("turn_stats").and_then(|v| v.as_array()) {
        for turn in turns {
            records_seen += 1;
            let rounds = turn
                .get("round_count")
                .and_then(|v| v.as_i64())
                .filter(|r| *r >= 0)
                .unwrap_or(0);
            if let Some(models) = turn.get("model_usage").and_then(|v| v.as_array()) {
                let single_model = models.len() == 1;
                for model in models {
                    let key = model_key(model);
                    let tokens = model.get("tokens");
                    let entry = acc.entry(key).or_default();
                    entry.add_tokens(tokens, &mut diagnostics);
                    // Assign round_count to the model only for a single-model turn. Adding the same count
                    // to every model in a multiple-model turn would inflate the combined call count.
                    if single_model {
                        entry.rounds = entry.rounds.saturating_add(rounds);
                    }
                }
                if !single_model {
                    unattributed_rounds = unattributed_rounds.saturating_add(rounds);
                }
            } else {
                unattributed_rounds = unattributed_rounds.saturating_add(rounds);
            }
        }
    }
    if let Some(detached) = document
        .get("detached_model_usage")
        .and_then(|v| v.as_array())
    {
        for model in detached {
            records_seen += 1;
            let key = model_key(model);
            let tokens = model.get("tokens");
            let entry = acc.entry(key).or_default();
            entry.add_tokens(tokens, &mut diagnostics);
        }
    }
    if acc.is_empty() && unattributed_rounds == 0 {
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
            diagnostics,
            lines_read: 1,
            records_seen,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    }
    let mut aggregates = Vec::new();
    for ((provider, model), entry) in acc {
        crate::adapters::run_policy::check()?;
        // Skip rows with neither a tokens value nor rounds; retain rows with rounds even
        // when all token fields are unknown. Count calls separately without filling unknowns with zero.
        if !entry.tokens_seen && entry.rounds == 0 {
            continue;
        }
        if entry.input.complete
            && entry.cached_input.complete
            && entry.output.complete
            && entry
                .input
                .sum
                .checked_add(entry.cached_input.sum)
                .and_then(|i| i.checked_add(entry.output.sum))
                .map_or(true, |n| n > crate::domain::MAX_TOKEN_VALUE)
        {
            diagnostics.push(diag(
                "token_sum_overflow",
                "native derived token sum exceeds the supported range; kept unknown",
            ));
        }
        let (usage, quality) = entry.usage();
        let scope_model = if model.is_empty() { "unknown" } else { &model };
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: format!("atomcode:session:{session_id}:{provider}/{scope_model}"),
            interval_start_ms: start_ms,
            interval_end_ms: end_ms,
            interval_end_inclusive: false,
            usage,
            quality,
            reported_call_count: (entry.rounds > 0).then_some(entry.rounds),
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        });
    }
    if unattributed_rounds > 0 {
        // Keep calls from multiple-model/no-model_usage turns separately, without assigning a model.
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: format!("atomcode:session:{session_id}:unattributed"),
            interval_start_ms: start_ms,
            interval_end_ms: end_ms,
            interval_end_inclusive: false,
            usage: TokenUsage::default(),
            quality: TokenQuality::default(),
            reported_call_count: Some(unattributed_rounds),
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        });
    }
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: None,
        events: Vec::new(),
        aggregates,
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
    fn ms_bounds() {
        assert_eq!(
            ms_field(Some(&serde_json::json!(1_790_000_000_000i64))),
            Some(1_790_000_000_000)
        );
        assert_eq!(ms_field(Some(&serde_json::json!(5))), None);
        assert_eq!(ms_field(None), None);
    }
}
