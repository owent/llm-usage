//! V01 shared token mappings based on native source field meanings.
//!
//! Directory layout follows architecture.md#adapter-layout. Product-specific mappings live in
//! adapters/codex/common.rs, adapters/claude/,
//! adapters/zcode/common.rs (M4, map_zcode_ai_sdk and map_zcode_anthropic),
//! adapters/kilo/common.rs (M3, map_kilo and KiloUsage),
//! and adapters/kimi_wire.rs (M4, map_kimi_wire and KimiWireUsage),
//! shared by Kimi Code and Kimi Work. This module retains cross-Agent types and helpers:
//!
//! - pi/omp share map_pi_family, with rules checked in fixed source versions;
//! - gemini/qwen share map_genai_usage for the matching usageMetadata field shape.
//!
//! Source checks recorded in m0-agent-fixtures.md establish:
//! - Kimi wire has independent inputOther/inputCacheRead/inputCacheCreation/output, without total
//!   (mapping in adapters/kimi_wire.rs).
//! - ZCode AI SDK inputTokens includes cache reads; Anthropic input_tokens excludes cache
//!   (separate mappings in adapters/zcode/common.rs).
//! - Copilot input includes uncached input, cache read and cache write (adapters/copilot/common.rs).
//! - Kilo total sums independent input/output/reasoning/cache.read/cache.write
//!   (mapping in adapters/kilo/common.rs).
//!
//! Map only verified fields; missing values stay None (unknown), without replacement zeros.
//! Diagnose contradictions such as cache read above total input; do not hide them with max(0, ...).

use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::metrics::{detect_contradictions, Contradiction};

/// Normalized usage, per-field quality and consistency diagnostics.
#[derive(Debug, Clone)]
pub struct MappedUsage {
    pub usage: TokenUsage,
    pub quality: TokenQuality,
    pub diagnostics: Vec<Contradiction>,
}

pub(crate) fn finish(
    usage: TokenUsage,
    quality: TokenQuality,
    diagnostics: Vec<Contradiction>,
) -> MappedUsage {
    finish_impl(usage, quality, diagnostics, true)
}

/// Map independently reported fields when inclusion relationships are unverified, as in Zed.
/// Subset checks such as cache <= input_total require known inclusion relationships and can falsely
/// reject independent reports. Domain validation still checks nonnegative values and upper limits.
pub(crate) fn finish_parallel(
    usage: TokenUsage,
    quality: TokenQuality,
    diagnostics: Vec<Contradiction>,
) -> MappedUsage {
    finish_impl(usage, quality, diagnostics, false)
}

fn finish_impl(
    usage: TokenUsage,
    mut quality: TokenQuality,
    mut diagnostics: Vec<Contradiction>,
    check_subset: bool,
) -> MappedUsage {
    for (value, field) in [
        (usage.input_uncached, &mut quality.input_uncached),
        (usage.input_cache_read, &mut quality.input_cache_read),
        (usage.input_cache_write, &mut quality.input_cache_write),
        (usage.input_total, &mut quality.input_total),
        (usage.output_total, &mut quality.output_total),
        (usage.output_reasoning, &mut quality.output_reasoning),
        (usage.total_tokens, &mut quality.total_tokens),
        (usage.source_total, &mut quality.source_total),
    ] {
        if value.is_none() {
            *field = Q::Unknown;
        }
    }
    if check_subset {
        diagnostics.extend(detect_contradictions(&usage));
    }
    MappedUsage {
        usage,
        quality,
        diagnostics,
    }
}

pub(crate) fn sub_checked(
    name: &'static str,
    total: i64,
    part: i64,
    diagnostics: &mut Vec<Contradiction>,
) -> Option<i64> {
    match total.checked_sub(part) {
        Some(v) if v >= 0 => Some(v),
        _ => {
            diagnostics.push(Contradiction {
                code: "negative_derived_field",
                field: name,
                detail: format!("derived {name} = {total} - {part} would be negative"),
            });
            None
        }
    }
}
// M4 moved the four independent Kimi wire fields to adapters/kimi_wire.rs, shared by
// Kimi Code/Kimi Work, and the two ZCode mappings to adapters/zcode/common.rs.
// Each source file documents its own calculation references.

/// Shared pi/oh-my-pi Usage meanings, within the limits documented in adapters.md.
/// Fixed source references:
/// - pi-mono b4559750 packages/ai/src/types.ts: Usage has input/output/cacheRead/cacheWrite,
///   optional cacheWrite1h (subset of cacheWrite), reasoning (subset of output), totalTokens and cost.
///   anthropic-messages.ts/openai-completions.ts normalize input as the uncached bucket:
///   input = max(0, prompt_tokens - cached - cache_write);
///   totalTokens = input + output + cacheRead + cacheWrite.
/// - oh-my-pi 62bc57b packages/catalog/src/types.ts defines input as uncached
///   conversation input. totalTokens sums the four buckets plus provider orchestration,
///   when present; reasoningTokens is within output, and cttl subdivides cacheWrite.
///
/// input/cacheRead/cacheWrite are independent; do not add reasoning again. Diagnose a mismatch
/// between totalTokens and derived totals, such as OMP orchestration, without truncating values.
#[derive(Debug, Clone, Copy)]
pub struct PiFamilyUsage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub total_tokens: i64,
    /// Optional pi reasoning/OMP reasoningTokens: absence is Unknown;
    /// preserve zero when supplied as a verified reported value.
    pub reasoning: Option<i64>,
}

pub fn map_pi_family(raw: &PiFamilyUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let input_total = raw
        .input
        .checked_add(raw.cache_read)
        .and_then(|v| v.checked_add(raw.cache_write));
    let derived_total = input_total.and_then(|i| i.checked_add(raw.output));
    if let Some(dt) = derived_total {
        if dt != raw.total_tokens {
            diagnostics.push(Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: format!(
                    "pi-family totalTokens {} != input {} + output {} + cacheRead {} + cacheWrite {} (derived {dt})",
                    raw.total_tokens, raw.input, raw.output, raw.cache_read, raw.cache_write
                ),
            });
        }
    }
    if let Some(reasoning) = raw.reasoning {
        if reasoning > raw.output {
            diagnostics.push(Contradiction {
                code: "reasoning_exceeds_output",
                field: "output_reasoning",
                detail: format!("reasoning {reasoning} > output {}", raw.output),
            });
        }
    }
    let usage = TokenUsage {
        input_uncached: Some(raw.input),
        input_cache_read: Some(raw.cache_read),
        input_cache_write: Some(raw.cache_write),
        input_total,
        output_total: Some(raw.output),
        output_reasoning: raw.reasoning,
        total_tokens: derived_total,
        source_total: Some(raw.total_tokens),
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: Q::Reported,
        output_reasoning: if raw.reasoning.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        source_total: Q::Reported,
    };
    finish(usage, quality, diagnostics)
}

/// genai usageMetadata has six categories in Gemini CLI session tokens and Qwen Code
/// ChatRecord.usageMetadata at the checked source versions:
/// optional prompt(input), candidates(output), cached, thoughts, tool and total.
/// The references verify fields/categories, but inclusion of cached/thoughts/tool in input/output
/// has not been checked for each provider. Preserve unknowns without inferred relationships:
/// - keep input_uncached/output_reasoning None without verified independence/subset rules;
/// - take total_tokens only from reported total, without summing incomplete components;
///   thoughts/tool may sit outside input+output;
/// - tool has no normalized field; do not force it into input_total to match total.
///
/// finish() still diagnoses contradictions such as cached above prompt.
#[derive(Debug, Clone, Copy, Default)]
pub struct GenaiUsage {
    pub prompt_tokens: Option<i64>,
    pub candidates_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub thoughts_tokens: Option<i64>,
    pub tool_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

pub fn map_genai_usage(raw: &GenaiUsage) -> MappedUsage {
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: raw.cached_tokens,
        input_cache_write: None,
        input_total: raw.prompt_tokens,
        output_total: raw.candidates_tokens,
        output_reasoning: None,
        total_tokens: raw.total_tokens,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Unknown,
        input_cache_read: if raw.cached_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        input_cache_write: Q::Unknown,
        input_total: if raw.prompt_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        output_total: if raw.candidates_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        output_reasoning: Q::Unknown,
        total_tokens: if raw.total_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}
