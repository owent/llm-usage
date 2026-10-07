//! Claude 产品特有的 usage 字段映射（从根级 usage_map.rs 下沉，V30 目录约定）。
//! 共享的 MappedUsage/finish/sub_checked/矛盾检测仍留在跨 Agent 的 usage_map.rs。

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Claude Code transcript assistant 条目的 usage 四字段（文档或源码依据 A01：
/// monitoring-usage 的 input/output/cache_read/cache_creation 分类 + Anthropic
/// usage 块互斥关系，与 map_zcode_anthropic 同形）。四字段缺一不可（缺失是未知，
/// 不能当 0）；input_total/total_tokens 由互斥拆分派生。
/// 无版本文档级载体保留旧映射；原生 2.1.197 使用独立的默认零规则。
#[derive(Debug, Clone, Copy)]
pub struct ClaudeTranscriptUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
}

pub fn map_claude_transcript(raw: &ClaudeTranscriptUsage) -> MappedUsage {
    let input_total = raw
        .input_tokens
        .checked_add(raw.cache_read_input_tokens)
        .and_then(|v| v.checked_add(raw.cache_creation_input_tokens));
    let total = input_total.and_then(|i| i.checked_add(raw.output_tokens));
    let usage = TokenUsage {
        input_uncached: Some(raw.input_tokens),
        input_cache_read: Some(raw.cache_read_input_tokens),
        input_cache_write: Some(raw.cache_creation_input_tokens),
        input_total,
        output_total: Some(raw.output_tokens),
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
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

/// 2.1.197 的流式写者会补入供应商未报告的零桶，且不保存有效性标记。
/// 正桶仍为报告值；零桶未知，不用另一条 HTTP 响应替本地历史认证零。
pub fn map_claude_native(raw: &ClaudeTranscriptUsage) -> MappedUsage {
    let positive = |value| (value > 0).then_some(value);
    let input_uncached = positive(raw.input_tokens);
    let input_cache_read = positive(raw.cache_read_input_tokens);
    let input_cache_write = positive(raw.cache_creation_input_tokens);
    let output_total = positive(raw.output_tokens);
    let input_total = input_uncached
        .zip(input_cache_read)
        .zip(input_cache_write)
        .and_then(|((i, r), w)| i.checked_add(r)?.checked_add(w));
    let total_tokens = input_total
        .zip(output_total)
        .and_then(|(i, o)| i.checked_add(o));
    let reported = |value: Option<i64>| {
        if value.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        }
    };
    let derived = |value: Option<i64>| {
        if value.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        }
    };
    finish(
        TokenUsage {
            input_uncached,
            input_cache_read,
            input_cache_write,
            input_total,
            output_total,
            output_reasoning: None,
            total_tokens,
            source_total: None,
        },
        TokenQuality {
            input_uncached: reported(input_uncached),
            input_cache_read: reported(input_cache_read),
            input_cache_write: reported(input_cache_write),
            input_total: derived(input_total),
            output_total: reported(output_total),
            output_reasoning: Q::Unknown,
            total_tokens: derived(total_tokens),
            source_total: Q::Unknown,
        },
        Vec::new(),
    )
}
