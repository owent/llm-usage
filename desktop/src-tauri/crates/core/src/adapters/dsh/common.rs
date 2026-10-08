//! DSH usage mapping originates in fixed token-meter README 46a7f68, A08.
//! Legacy persistence references remain distinct from the separately checked native v4 reader.
//!
//! Reference: packages/llm/token-meter/README.md at 46a7f68.
//! - "tokenUsage carries the complete durable log's `uncachedInputTokens`,
//!   `outputTokens`, `cacheReadTokens`, and `cacheWriteTokens`"; all four fields
//!   are optional: "each corresponding aggregate appears only when every
//!   participating attempt reports its optional cache, reasoning, or route
//!   value". Missing cache is unknown, without zero substitution.
//! - uncachedInputTokens is uncached input;
//!   input_total derives uncached + cacheRead + cacheWrite;
//!   total_tokens derives input_total + output.
//!
//! Shared MappedUsage/finish logic remains in cross-product usage_map.rs.

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Four optional DSH durable-log usage fields carried by assistant/message records.
/// Serde persists unfinished attempt samples across scan rounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DshUsage {
    pub uncached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

impl DshUsage {
    /// True when all four usage fields are absent.
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
