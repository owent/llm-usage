//! Codex usage mapping moved from root usage_map.rs under the V30 directory rules.
//! Shared MappedUsage/finish/sub_checked/contradiction checks remain in usage_map.rs.

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::metrics::Contradiction;

/// Cumulative snapshots are comparisons only; per-record read errors determine file health.
pub(crate) fn is_record_error(diagnostic: &crate::ingest::DiagnosticInput) -> bool {
    matches!(
        diagnostic.code.as_str(),
        "bad_json_line" | "usage_shape_deviation" | "timestamp_unparseable" | "line_too_long"
    ) && diagnostic.field.as_deref() != Some("info.total_token_usage")
}

/// TokenCountEvent.info is Option<TokenUsageInfo>; quota-only updates may lack usage.
/// Verified against official rust-v0.144.0 protocol.rs and native 0.144.0-alpha.4 samples.
pub(crate) fn token_count_has_no_usage(payload: &serde_json::Value) -> bool {
    matches!(payload.get("info"), None | Some(serde_json::Value::Null))
}

/// Codex rollout usage: input includes cache read, with no cache-creation field;
/// output includes reasoning, and total=input+output.
/// declares_no_cache_creation requires a verified version/format declaration:
/// only then derive uncached=input-cached and cache write=0.
#[derive(Debug, Clone, Copy)]
pub struct CodexUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub declares_no_cache_creation: bool,
}

/// Complete Codex 0.155.0-alpha.16.3 usage in token_usage_record.payload.usage
/// and token_count total/last_token_usage has the same six required fields.
/// cached is an input subset (319/319 native samples); reasoning is an output subset;
/// total=input+output. Cache-write inclusion is a schema-based mapping assumption:
/// native samples cover cache_write=0 only. Diagnose contradictions instead of hiding them with max(0, ...).
#[derive(Debug, Clone, Copy)]
pub struct CodexRecordUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_write_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

pub fn map_codex_record(raw: &CodexRecordUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let cache_parts = match raw
        .cached_input_tokens
        .checked_add(raw.cache_write_input_tokens)
    {
        Some(v) => v,
        None => {
            diagnostics.push(Contradiction {
                code: "derived_overflow",
                field: "input_uncached",
                detail: "cached + cache_write overflows i64".into(),
            });
            return finish(TokenUsage::default(), TokenQuality::default(), diagnostics);
        }
    };
    let uncached = sub_checked(
        "input_uncached",
        raw.input_tokens,
        cache_parts,
        &mut diagnostics,
    );
    if raw.total_tokens != raw.input_tokens.saturating_add(raw.output_tokens) {
        diagnostics.push(Contradiction {
            code: "source_total_mismatch",
            field: "total_tokens",
            detail: format!(
                "codex total {} != input {} + output {}",
                raw.total_tokens, raw.input_tokens, raw.output_tokens
            ),
        });
    }
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: Some(raw.cached_input_tokens),
        input_cache_write: Some(raw.cache_write_input_tokens),
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: Some(raw.reasoning_output_tokens),
        total_tokens: raw.input_tokens.checked_add(raw.output_tokens),
        source_total: Some(raw.total_tokens),
    };
    let quality = TokenQuality {
        input_uncached: Q::Derived,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Reported,
        total_tokens: Q::Derived,
        source_total: Q::Reported,
    };
    finish(usage, quality, diagnostics)
}

pub fn map_codex(raw: &CodexUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = if raw.declares_no_cache_creation {
        sub_checked(
            "input_uncached",
            raw.input_tokens,
            raw.cached_input_tokens,
            &mut diagnostics,
        )
    } else {
        None
    };
    if raw.total_tokens != raw.input_tokens.saturating_add(raw.output_tokens) {
        diagnostics.push(Contradiction {
            code: "source_total_mismatch",
            field: "total_tokens",
            detail: format!(
                "codex total {} != input {} + output {}",
                raw.total_tokens, raw.input_tokens, raw.output_tokens
            ),
        });
    }
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: Some(raw.cached_input_tokens),
        input_cache_write: if raw.declares_no_cache_creation {
            Some(0)
        } else {
            None
        },
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: Some(raw.reasoning_output_tokens),
        total_tokens: raw.input_tokens.checked_add(raw.output_tokens),
        source_total: Some(raw.total_tokens),
    };
    let quality = TokenQuality {
        input_uncached: Q::Derived,
        input_cache_read: Q::Reported,
        input_cache_write: if raw.declares_no_cache_creation {
            Q::Derived
        } else {
            Q::Unknown
        },
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Reported,
        total_tokens: Q::Derived,
        source_total: Q::Reported,
    };
    finish(usage, quality, diagnostics)
}
