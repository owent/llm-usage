//! V01：五种来源口径的 token 字段映射函数。
//!
//! 实读核验结论（m0-agent-fixtures.md）：
//! - codex：total=input+output，cached⊆input，reasoning⊆output；usage 无 model 字段。
//! - kimi wire：inputOther/inputCacheRead/inputCacheCreation/output 四字段互斥，无 total。
//! - zcode：AI SDK `inputTokens` 含缓存读；anthropic `input_tokens` 不含缓存（双口径相反）。
//! - copilot：input = 未缓存 + read + write。
//! - kilo：total = input+output+reasoning+cache.read+cache.write 全互斥。
//!
//! 所有映射只填有证据的字段；缺失保持 None（unknown），不补零。
//! 矛盾（如缓存读 > 总输入）返回诊断，不用 max(0, …) 隐藏。

use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::metrics::{detect_contradictions, Contradiction};

/// 映射结果：规范化 usage、逐字段质量、一致性诊断。
#[derive(Debug, Clone)]
pub struct MappedUsage {
    pub usage: TokenUsage,
    pub quality: TokenQuality,
    pub diagnostics: Vec<Contradiction>,
}

fn finish(usage: TokenUsage, quality: TokenQuality, mut diagnostics: Vec<Contradiction>) -> MappedUsage {
    diagnostics.extend(detect_contradictions(&usage));
    MappedUsage { usage, quality, diagnostics }
}

fn sub_checked(name: &'static str, total: i64, part: i64, diagnostics: &mut Vec<Contradiction>) -> Option<i64> {
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

/// codex rollout token_usage_record：input 含缓存读，无缓存创建字段；
/// output 含 reasoning；total=input+output。
/// `declares_no_cache_creation` 为格式级证据：该版本明确无缓存创建时，
/// 未缓存输入才可证明为 input-cached，缓存写记 0（derived）。
#[derive(Debug, Clone, Copy)]
pub struct CodexUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub declares_no_cache_creation: bool,
}

pub fn map_codex(raw: &CodexUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = sub_checked("input_uncached", raw.input_tokens, raw.cached_input_tokens, &mut diagnostics);
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
        input_cache_write: if raw.declares_no_cache_creation { Some(0) } else { None },
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: Some(raw.reasoning_output_tokens),
        total_tokens: Some(raw.total_tokens),
        source_total: Some(raw.total_tokens),
    };
    let quality = TokenQuality {
        input_uncached: Q::Derived,
        input_cache_read: Q::Reported,
        input_cache_write: if raw.declares_no_cache_creation { Q::Derived } else { Q::Unknown },
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Reported,
        total_tokens: Q::Reported,
        source_total: Q::Reported,
    };
    finish(usage, quality, diagnostics)
}

/// kimi wire usage.record：四字段互斥，无 total。epoch 毫秒时间。
#[derive(Debug, Clone, Copy)]
pub struct KimiWireUsage {
    pub input_other: i64,
    pub input_cache_read: i64,
    pub input_cache_creation: i64,
    pub output: i64,
}

pub fn map_kimi_wire(raw: &KimiWireUsage) -> MappedUsage {
    let input_total = raw
        .input_other
        .checked_add(raw.input_cache_read)
        .and_then(|v| v.checked_add(raw.input_cache_creation));
    let total = input_total.and_then(|i| i.checked_add(raw.output));
    let usage = TokenUsage {
        input_uncached: Some(raw.input_other),
        input_cache_read: Some(raw.input_cache_read),
        input_cache_write: Some(raw.input_cache_creation),
        input_total,
        output_total: Some(raw.output),
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Derived,
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: Q::Derived,
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// zcode AI SDK camelCase 口径：`inputTokens` 含缓存读。
#[derive(Debug, Clone, Copy)]
pub struct ZcodeAiSdkUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub output_tokens: i64,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}

pub fn map_zcode_ai_sdk(raw: &ZcodeAiSdkUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let cache_parts: i64 = raw.cached_input_tokens.unwrap_or(0)
        + raw.cache_creation_input_tokens.unwrap_or(0);
    let uncached = sub_checked("input_uncached", raw.input_tokens, cache_parts, &mut diagnostics);
    // 缓存写是否包含于 inputTokens 未经实读证明：未知保持 None，不假设。
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: raw.cached_input_tokens,
        input_cache_write: raw.cache_creation_input_tokens,
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: raw.reasoning_tokens,
        total_tokens: raw.total_tokens,
        source_total: raw.total_tokens,
    };
    let quality = TokenQuality {
        input_uncached: Q::Derived,
        input_cache_read: if raw.cached_input_tokens.is_some() { Q::Reported } else { Q::Unknown },
        input_cache_write: if raw.cache_creation_input_tokens.is_some() { Q::Reported } else { Q::Unknown },
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: if raw.reasoning_tokens.is_some() { Q::Reported } else { Q::Unknown },
        total_tokens: if raw.total_tokens.is_some() { Q::Reported } else { Q::Unknown },
        source_total: if raw.total_tokens.is_some() { Q::Reported } else { Q::Unknown },
    };
    finish(usage, quality, diagnostics)
}

/// zcode anthropic snake_case 口径：`input_tokens` 不含缓存（与 AI SDK 相反）。
#[derive(Debug, Clone, Copy)]
pub struct ZcodeAnthropicUsage {
    pub input_tokens: i64,
    pub cache_read_input_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub output_tokens: i64,
}

pub fn map_zcode_anthropic(raw: &ZcodeAnthropicUsage) -> MappedUsage {
    let input_total = raw
        .cache_read_input_tokens
        .zip(raw.cache_creation_input_tokens)
        .and_then(|(r, w)| raw.input_tokens.checked_add(r).and_then(|v| v.checked_add(w)));
    let total = input_total.and_then(|i| i.checked_add(raw.output_tokens));
    let usage = TokenUsage {
        input_uncached: Some(raw.input_tokens),
        input_cache_read: raw.cache_read_input_tokens,
        input_cache_write: raw.cache_creation_input_tokens,
        input_total,
        output_total: Some(raw.output_tokens),
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: if raw.cache_read_input_tokens.is_some() { Q::Reported } else { Q::Unknown },
        input_cache_write: if raw.cache_creation_input_tokens.is_some() { Q::Reported } else { Q::Unknown },
        input_total: if input_total.is_some() { Q::Derived } else { Q::Unknown },
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: if total.is_some() { Q::Derived } else { Q::Unknown },
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// copilot session-store.db assistant_usage_events：input = 未缓存 + read + write。
/// request_multiplier 是付费倍率，不进入 token 统计。
#[derive(Debug, Clone, Copy)]
pub struct CopilotUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub output_tokens: i64,
}

pub fn map_copilot(raw: &CopilotUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let cache_sum = raw.cached_input_tokens.saturating_add(raw.cache_creation_input_tokens);
    let uncached = sub_checked("input_uncached", raw.input_tokens, cache_sum, &mut diagnostics);
    let total = raw.input_tokens.checked_add(raw.output_tokens);
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: Some(raw.cached_input_tokens),
        input_cache_write: Some(raw.cache_creation_input_tokens),
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Derived,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: Q::Derived,
        source_total: Q::Unknown,
    };
    finish(usage, quality, diagnostics)
}

/// kilo（opencode 派生）message.data.tokens：
/// total = input+output+reasoning+cache.read+cache.write 全互斥（与其他源相反）。
/// 因此 canonical output_total 须把 reasoning 并入（derived），才能满足
/// 统一合同 total_tokens = input_total + output_total。
#[derive(Debug, Clone, Copy)]
pub struct KiloUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: Option<i64>,
    pub cache_read: i64,
    pub cache_write: i64,
    pub total: i64,
}

pub fn map_kilo(raw: &KiloUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let input_total = raw
        .input
        .checked_add(raw.cache_read)
        .and_then(|v| v.checked_add(raw.cache_write));
    let reasoning = raw.reasoning.unwrap_or(0);
    let output_total = raw.output.checked_add(reasoning);
    let derived_total = input_total.and_then(|i| output_total.and_then(|o| i.checked_add(o)));
    if let Some(dt) = derived_total {
        if dt != raw.total {
            diagnostics.push(Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: format!("kilo total {} != derived sum {dt}", raw.total),
            });
        }
    }
    let usage = TokenUsage {
        input_uncached: Some(raw.input),
        input_cache_read: Some(raw.cache_read),
        input_cache_write: Some(raw.cache_write),
        input_total,
        output_total,
        output_reasoning: raw.reasoning,
        total_tokens: Some(raw.total),
        source_total: Some(raw.total),
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Derived,
        output_total: Q::Derived,
        output_reasoning: if raw.reasoning.is_some() { Q::Reported } else { Q::Unknown },
        total_tokens: Q::Reported,
        source_total: Q::Reported,
    };
    finish(usage, quality, diagnostics)
}
