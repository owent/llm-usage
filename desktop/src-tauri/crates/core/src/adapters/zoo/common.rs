//! Zoo Code 产品特有的 usage 字段映射（固定源码 f7806475331fcae5f4e8b5558d04415eeb5da88c，
//! A19，按文档或源码实现，待真实样本核验；本机 not_found）。
//!
//! 依据（packages/core/src/message-utils/consolidateTokenUsage.ts）：
//! - `api_req_started` 消息 `text` JSON 的 tokensIn/tokensOut/cacheWrites/
//!   cacheReads/cost 逐字段可选（typeof number 检查）；
//! - **tokensIn 存的是总输入 token（含缓存 token）**（固定源码注释原文：
//!   "Since tokensIn now stores TOTAL input tokens (including cache tokens),
//!   we no longer need to add cacheWrites and cacheReads separately.
//!   This applies to both Anthropic and OpenAI protocols."）⇒ 与 Cline 的
//!   四桶互斥关系相反：input_total = tokensIn 直报（reported），
//!   cacheReads/cacheWrites 是其子集（方向已证），input_uncached 不推导；
//! - per-request 总量计算：上游 contextTokens = tokensIn + tokensOut
//!   （对最后一条请求；同文件算术）⇒ total_tokens 按同式派生（derived）；
//! - cost 是扩展自算值（合并 finished 后写入 text）⇒ estimated micro-USD
//!   （与 cline 同规则）。

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::CostAmount;
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Zoo `api_req_started` text JSON 的 usage 四可选字段
///（tokensIn 含缓存 = 总输入；cacheWrites/cacheReads 为其子集）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ZooUsage {
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub cache_writes: Option<i64>,
    pub cache_reads: Option<i64>,
}

impl ZooUsage {
    /// 四个 usage 字段是否全部缺失（无 usage 数字的载体记录）。
    pub fn is_empty(&self) -> bool {
        self.tokens_in.is_none()
            && self.tokens_out.is_none()
            && self.cache_writes.is_none()
            && self.cache_reads.is_none()
    }
}

pub fn map_zoo_usage(raw: &ZooUsage) -> MappedUsage {
    // tokensIn 已含缓存（两协议规则相同，固定源码注释）：直报 input_total；
    // 未缓存输入不可拆（精确包含集合尚未验证），input_uncached 保持未知。
    let total = raw
        .tokens_in
        .and_then(|i| raw.tokens_out.and_then(|o| i.checked_add(o)));
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: raw.cache_reads,
        input_cache_write: raw.cache_writes,
        input_total: raw.tokens_in,
        output_total: raw.tokens_out,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Unknown,
        input_cache_read: raw.cache_reads.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_write: raw.cache_writes.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_total: raw.tokens_in.map(|_| Q::Reported).unwrap_or(Q::Unknown),
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

/// cost 浮点美元 → micro-USD（estimated；上游自行估算，与 cline 规则相同）。
/// 溢出/负值/非有限返回 None（调用方保持未知，不截断数值）。
pub fn map_zoo_cost(total: Option<f64>) -> Option<CostAmount> {
    let total = total?;
    if !total.is_finite() || total < 0.0 {
        return None;
    }
    let micros = total * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        kind: crate::domain::CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_tokens_in_as_total_input_with_cache_subset() {
        let m = map_zoo_usage(&ZooUsage {
            tokens_in: Some(100),
            tokens_out: Some(20),
            cache_writes: Some(10),
            cache_reads: Some(40),
        });
        // tokensIn 已含缓存（固定源码注释）：input_total 直报 100，不与缓存相加。
        assert_eq!(m.usage.input_total, Some(100));
        assert_eq!(m.usage.input_cache_read, Some(40));
        assert_eq!(m.usage.input_cache_write, Some(10));
        assert_eq!(m.usage.total_tokens, Some(120), "上游算术 in+out");
        assert_eq!(m.usage.input_uncached, None, "精确包含集合未证不拆");
        assert!(m.diagnostics.is_empty());
    }

    #[test]
    fn map_missing_fields_stay_unknown() {
        let m = map_zoo_usage(&ZooUsage {
            tokens_in: None,
            tokens_out: Some(5),
            cache_writes: None,
            cache_reads: None,
        });
        assert_eq!(m.usage.input_total, None);
        assert_eq!(m.usage.output_total, Some(5));
        assert_eq!(m.usage.total_tokens, None, "缺 tokensIn 不派生总量");
    }

    #[test]
    fn cost_maps_to_estimated_micro_usd() {
        let cost = map_zoo_cost(Some(0.005)).unwrap();
        assert_eq!(cost.amount_minor, 5_000);
        assert_eq!(cost.kind, crate::domain::CostKind::Estimated);
        assert!(map_zoo_cost(None).is_none());
        assert!(map_zoo_cost(Some(-1.0)).is_none());
    }
}
