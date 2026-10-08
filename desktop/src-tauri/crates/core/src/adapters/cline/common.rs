//! Legacy Cline UI usage mapping from fixed getApiMetrics.ts dcf8c3c, A03.
//! SDK records have separate parsing/mapping; native SDK acceptance does not verify this UI format.
//!
//! Source: apps/vscode/src/shared/getApiMetrics.ts.
//! - text JSON tokensIn/tokensOut/cacheWrites/cacheReads are individually optional;
//!   upstream checks typeof === "number", without replacing absent values with zero.
//! - Four buckets are exclusive: getLastApiReqTotalTokens calculates
//!   tokensIn + tokensOut + cacheWrites + cacheReads.
//!   Therefore input_total = tokensIn + cacheWrites + cacheReads (derived),
//!   and total_tokens sums all four buckets (derived).
//!
//! Shared MappedUsage/finish remain in cross-agent usage_map.rs.

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Four optional exclusive usage buckets in Cline say-message text JSON; missing remains unknown.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClineUsage {
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub cache_writes: Option<i64>,
    pub cache_reads: Option<i64>,
}

impl ClineUsage {
    /// Whether all four usage fields are absent.
    pub fn is_empty(&self) -> bool {
        self.tokens_in.is_none()
            && self.tokens_out.is_none()
            && self.cache_writes.is_none()
            && self.cache_reads.is_none()
    }
}

pub fn map_cline_usage(raw: &ClineUsage) -> MappedUsage {
    let input_total = raw
        .tokens_in
        .zip(raw.cache_writes)
        .and_then(|(a, b)| a.checked_add(b))
        .and_then(|v| raw.cache_reads.and_then(|r| v.checked_add(r)));
    let total = input_total.and_then(|i| raw.tokens_out.and_then(|o| i.checked_add(o)));
    let usage = TokenUsage {
        input_uncached: raw.tokens_in,
        input_cache_read: raw.cache_reads,
        input_cache_write: raw.cache_writes,
        input_total,
        output_total: raw.tokens_out,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: raw.tokens_in.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_read: raw.cache_reads.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_write: raw.cache_writes.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: raw.tokens_out.map(|_| Q::Reported).unwrap_or(Q::Unknown),
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
