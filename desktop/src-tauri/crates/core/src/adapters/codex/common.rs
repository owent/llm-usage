//! Codex 产品特有的 usage 字段映射（从根级 usage_map.rs 下沉，V30 目录合同）。
//! 共享的 MappedUsage/finish/sub_checked/矛盾检测仍留在跨 Agent 的 usage_map.rs。

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::metrics::Contradiction;

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

/// codex 0.155.0-alpha.16.3 完整 usage 记录（token_usage_record.payload.usage
/// 与 token_count 的 total/last_token_usage 同形）：六字段全部存在。
/// 口径：`cached ⊆ input`（真实样本 319/319 成立）、`reasoning ⊆ output`、
/// `total = input + output`；`cache_write ⊆ input` 是该版本 schema 的映射假设
/// （真实样本仅覆盖 cache_write=0），矛盾进诊断，不用 max(0, …) 隐藏。
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
