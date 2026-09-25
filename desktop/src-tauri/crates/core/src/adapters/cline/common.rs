//! Cline 产品特有的 usage 字段映射（getApiMetrics.ts 固定源码 dcf8c3c，A03，
//! 文档级证据待真实样本；本机 not_found 无真实样本）。
//!
//! 证据（apps/vscode/src/shared/getApiMetrics.ts）：
//! - usage 载体 `text` JSON 的 tokensIn/tokensOut/cacheWrites/cacheReads 逐字段
//!   可选（上游 `typeof === "number"` 检查，缺失不补零）；
//! - 四桶互斥：getLastApiReqTotalTokens 的 total =
//!   `tokensIn + tokensOut + cacheWrites + cacheReads`（四分量之和）；
//!   因此 input_total = tokensIn + cacheWrites + cacheReads（派生）、
//!   total_tokens = 四桶之和（派生）。
//!
//! 共享的 MappedUsage/finish 逻辑在跨 Agent 的 usage_map.rs。

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Cline say 消息 text JSON 的 usage 四可选字段（互斥桶，缺失 = 未知）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ClineUsage {
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub cache_writes: Option<i64>,
    pub cache_reads: Option<i64>,
}

impl ClineUsage {
    /// 五个 usage 字段是否全部缺失（无 usage 数字的载体记录）。
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
