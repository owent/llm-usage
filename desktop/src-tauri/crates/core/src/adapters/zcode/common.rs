//! ZCode 产品特有的双口径 usage 映射（从根级 usage_map.rs 下沉，V30 目录合同）。
//! 共享的 MappedUsage/finish/sub_checked/矛盾检测仍留在跨 Agent 的 usage_map.rs。
//!
//! 双口径证据（m0-agent-fixtures.md，本机 ZCode 3.14.3 model-io JSONL 实读）：
//! 同一请求同一记录内两个 usage 视图，语义相反：
//!
//! - AI SDK camelCase `response.usage`：`inputTokens` **含**缓存读、`outputTokens`、
//!   `totalTokens`、`cacheReadTokens`、`cacheWriteTokens`（五键全，主口径）；
//! - anthropic snake_case `response.providerMetadata.anthropic.usage`：
//!   `input_tokens` **不含**缓存、`output_tokens`、`cache_read_input_tokens`、
//!   `cache_creation_input_tokens?`（对照口径）。
//!
//! 两视图互斥，绝不相加（计入即双计）；矛盾进诊断，主口径保留。

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::metrics::Contradiction;

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
