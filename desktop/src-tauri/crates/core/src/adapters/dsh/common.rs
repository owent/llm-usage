//! DSH 产品特有的 usage 字段映射（固定 token-meter README 46a7f68，A08，
//! 文档级证据待真实样本；本机 not_found 无真实样本）。
//!
//! 证据（packages/llm/token-meter/README.md @ 46a7f68）：
//! - "tokenUsage carries the complete durable log's `uncachedInputTokens`,
//!   `outputTokens`, `cacheReadTokens`, and `cacheWriteTokens`"——四字段
//!   各自可选（"each corresponding aggregate appears only when every
//!   participating attempt reports its optional cache, reasoning, or route
//!   value"：cache 是 attempt 可选值，缺失 = 未知不补零）；
//! - 字段名自证口径：uncachedInputTokens 是未缓存输入桶；
//!   input_total = uncached + cacheRead + cacheWrite（派生）、
//!   total_tokens = input_total + output（派生）。
//!
//! 共享的 MappedUsage/finish 逻辑在跨 Agent 的 usage_map.rs。

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// DSH 持久日志 usage 样本的四可选字段（assistant/message 携带）。
/// 待收口样本跨轮持久化，需要 serde 支持。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DshUsage {
    pub uncached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

impl DshUsage {
    /// 四字段是否全部缺失（无 usage 证据的消息）。
    pub fn is_empty(&self) -> bool {
        self.uncached_input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.cache_write_tokens.is_none()
    }
}

pub fn map_dsh_usage(raw: &DshUsage) -> MappedUsage {
    let input_total = raw
        .uncached_input_tokens
        .zip(raw.cache_read_tokens)
        .and_then(|(a, b)| a.checked_add(b))
        .and_then(|v| raw.cache_write_tokens.and_then(|w| v.checked_add(w)));
    let total = input_total.and_then(|i| raw.output_tokens.and_then(|o| i.checked_add(o)));
    let usage = TokenUsage {
        input_uncached: raw.uncached_input_tokens,
        input_cache_read: raw.cache_read_tokens,
        input_cache_write: raw.cache_write_tokens,
        input_total,
        output_total: raw.output_tokens,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: raw
            .uncached_input_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_cache_read: raw
            .cache_read_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_cache_write: raw
            .cache_write_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: raw.output_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
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
