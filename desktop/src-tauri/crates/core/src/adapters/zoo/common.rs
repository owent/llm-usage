//! Zoo Code usage mapping: fixed f7806475331fcae5f4e8b5558d04415eeb5da88c,
//! A19, plus real checks of official 3.86.0 VSIX/API/native files.
//!
//! Reference: packages/core/src/message-utils/consolidateTokenUsage.ts.
//! - tokensIn/tokensOut/cacheWrites/cacheReads/cost in api_req_started text JSON
//!   are independently optional, checked with typeof number.
//! - tokensIn stores total input including cache; fixed upstream comment:
//!   "Since tokensIn now stores TOTAL input tokens (including cache tokens),
//!   we no longer need to add cacheWrites and cacheReads separately.
//!   This applies to both Anthropic and OpenAI protocols." Unlike legacy Cline UI's
//!   exclusive buckets, input_total = tokensIn is reported directly;
//!   cacheReads/cacheWrites are input subsets; uncached input remains unknown.
//! - Upstream contextTokens = tokensIn + tokensOut for the last request;
//!   derive total_tokens using that same arithmetic.
//! - Extension-calculated cost is written to text after merging finished results;
//!   treat it as estimated micro-USD, following Cline's cost rule.

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::CostAmount;
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Four optional usage fields from Zoo api_req_started text JSON.
/// tokensIn includes cache; cacheWrites/cacheReads are subsets.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZooUsage {
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub cache_writes: Option<i64>,
    pub cache_reads: Option<i64>,
}

impl ZooUsage {
    /// True when all four usage fields are absent from the record.
    pub fn is_empty(&self) -> bool {
        self.tokens_in.is_none()
            && self.tokens_out.is_none()
            && self.cache_writes.is_none()
            && self.cache_reads.is_none()
    }
}

pub fn map_zoo_usage(raw: &ZooUsage) -> MappedUsage {
    // Task initializes zeros; OpenAI-compatible handling does not read nested cached_tokens.
    // Native zero cannot distinguish unused fields from unavailable usage; reported zero is unverified.
    let input = raw.tokens_in.filter(|v| *v > 0);
    let output = raw.tokens_out.filter(|v| *v > 0);
    let read = raw.cache_reads.filter(|v| *v > 0);
    let write = raw.cache_writes.filter(|v| *v > 0);
    // tokensIn already includes cache under both protocols; report input_total directly.
    // Exact cache containment is unverified; do not derive input_uncached.
    let total = input.and_then(|i| output.and_then(|o| i.checked_add(o)));
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: read,
        input_cache_write: write,
        input_total: input,
        output_total: output,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Unknown,
        input_cache_read: read.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_write: write.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_total: input.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: output.map(|_| Q::Reported).unwrap_or(Q::Unknown),
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

/// Convert positive finite USD cost to estimated micro-USD, following Cline's cost rule.
/// Zero/negative/nonfinite/overflow returns None; callers retain unknown without truncation.
pub fn map_zoo_cost(total: Option<f64>) -> Option<CostAmount> {
    let total = total?;
    if !total.is_finite() || total <= 0.0 {
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
        // tokensIn includes cache: report input_total 100 without adding cache again.
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
