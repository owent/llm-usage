//! V01：来源口径的 token 字段映射（跨 Agent 共享部分）。
//!
//! 目录合同（architecture.md#adapter-layout）：产品特有映射已下沉到各 Agent 目录
//! （codex → adapters/codex/common.rs，claude → adapters/claude/）；本模块只保留
//! 跨 Agent 共享的类型与辅助逻辑，以及尚无适配器目录的未来产品映射
//! （kimi/zcode/copilot/kilo，M3–M5 实现时再下沉）：
//! - pi/omp 共享 `map_pi_family`（固定源码证实两家族同口径）；
//! - gemini/qwen 共享 `map_genai_usage`（usageMetadata 同形）。
//!
//! 实读核验结论（m0-agent-fixtures.md）：
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

pub(crate) fn finish(
    usage: TokenUsage,
    mut quality: TokenQuality,
    mut diagnostics: Vec<Contradiction>,
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
    diagnostics.extend(detect_contradictions(&usage));
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
    // 缺少任一互斥部分时，不能用零补出未缓存输入。
    let uncached = raw
        .cached_input_tokens
        .zip(raw.cache_creation_input_tokens)
        .and_then(|(read, write)| match read.checked_add(write) {
            Some(parts) => sub_checked("input_uncached", raw.input_tokens, parts, &mut diagnostics),
            None => {
                diagnostics.push(Contradiction {
                    code: "derived_overflow",
                    field: "input_uncached",
                    detail: "cache parts overflow i64".into(),
                });
                None
            }
        });
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: raw.cached_input_tokens,
        input_cache_write: raw.cache_creation_input_tokens,
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: raw.reasoning_tokens,
        total_tokens: raw.input_tokens.checked_add(raw.output_tokens),
        source_total: raw.total_tokens,
    };
    let quality = TokenQuality {
        input_uncached: Q::Derived,
        input_cache_read: if raw.cached_input_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        input_cache_write: if raw.cache_creation_input_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: if raw.reasoning_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        total_tokens: Q::Derived,
        source_total: if raw.total_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
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
        .and_then(|(r, w)| {
            raw.input_tokens
                .checked_add(r)
                .and_then(|v| v.checked_add(w))
        });
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
        input_cache_read: if raw.cache_read_input_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        input_cache_write: if raw.cache_creation_input_tokens.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: if total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
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
    let cache_sum = raw
        .cached_input_tokens
        .saturating_add(raw.cache_creation_input_tokens);
    let uncached = sub_checked(
        "input_uncached",
        raw.input_tokens,
        cache_sum,
        &mut diagnostics,
    );
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
    let output_total = raw
        .reasoning
        .and_then(|reasoning| raw.output.checked_add(reasoning));
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
        total_tokens: derived_total.or(Some(raw.total)),
        source_total: Some(raw.total),
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Derived,
        output_total: Q::Derived,
        output_reasoning: if raw.reasoning.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Reported
        },
        source_total: Q::Reported,
    };
    finish(usage, quality, diagnostics)
}

/// pi / oh-my-pi 共享 Usage 口径（adapters.md：两家族可共享部分 Usage 类型知识）。
/// 证据（固定源码）：
/// - pi-mono b4559750 packages/ai/src/types.ts：`Usage{input,output,cacheRead,cacheWrite,
///   cacheWrite1h?(⊆cacheWrite),reasoning?(⊆output),totalTokens,cost}`；
///   anthropic-messages.ts 与 openai-completions.ts 均把 input 规范化为未缓存桶
///   （`input = max(0, prompt_tokens - cached - cache_write)`；
///   `totalTokens = input+output+cacheRead+cacheWrite`）。
/// - oh-my-pi 62bc57b packages/catalog/src/types.ts：`input` 注释为 "Non-cached
///   conversation input tokens"；`totalTokens` 为四桶之和外加 provider orchestration
///   （若有）；`reasoningTokens`⊆`output`；`cttl` 细分⊆`cacheWrite`。
///
/// 因此 input/cacheRead/cacheWrite 互斥；reasoning 不再加；totalTokens 与派生总量
/// 不一致（如 omp orchestration 附加）记诊断，不钳制。
#[derive(Debug, Clone, Copy)]
pub struct PiFamilyUsage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub total_tokens: i64,
    /// pi `reasoning` / omp `reasoningTokens`：缺字段表示供应商未报告（Unknown），
    /// 0 是报告值。
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

/// genai usageMetadata 六分类（Gemini CLI 会话 JSON 的 tokens 对象、Qwen Code
/// 固定源码 ChatRecord.usageMetadata 同形）：
/// prompt(input)/candidates(output)/cached/thoughts/tool/total，逐字段可选。
/// 证据只确认字段存在与分类；cached/thoughts/tool 与 input/output 的包含关系
/// 逐 provider 未核验（未知不补零、不猜），因此：
/// - input_uncached / output_reasoning 保持 None（不能断言互斥/子集关系）；
/// - total_tokens 只取来源直报的 total；缺失时不由拆分相加伪造
///   （thoughts/tool 可能在 input+output 之外）；
/// - tool 无统一桶，不并入任何字段（并入 input_total 与保持 total 一致二者不可兼得）。
///
/// 矛盾（如 cached > prompt）仍由 finish() 的矛盾检测进诊断。
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
