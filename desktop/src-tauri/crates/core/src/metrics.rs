//! 统计数学：互斥输入分类求和、缓存占比、聚合与矛盾诊断。
//! 合同：input_total = 三类互斥之和；total_tokens = input_total + output_total；
//! cache_input_ratio = SUM(cache_read) / SUM(input_total)（两字段均已知的记录集合）。

use crate::domain::{FieldQuality, TokenQuality, TokenUsage};
use crate::error::CoreError;

/// 数学矛盾的受限诊断码（不持久化正文，只存代码与字段名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contradiction {
    pub code: &'static str,
    pub field: &'static str,
    pub detail: String,
}

/// 已知字段求和，任一未知则整体未知。防溢出。
pub fn checked_sum(values: &[Option<i64>], what: &'static str) -> Result<Option<i64>, CoreError> {
    if values.iter().any(Option::is_none) {
        return Ok(None);
    }
    let mut acc: i128 = 0;
    for v in values.iter().flatten() {
        acc += *v as i128;
    }
    if acc > i64::MAX as i128 {
        return Err(CoreError::Overflow(what));
    }
    Ok(Some(acc as i64))
}

/// input_total：三类互斥拆分全已知时相加；否则采用来源直报值；都没有则为 None。
/// 返回值的第二个分量是推导出的字段质量。
pub fn input_total(usage: &TokenUsage, quality: &TokenQuality) -> Option<(i64, FieldQuality)> {
    if let (Some(u), Some(r), Some(w)) =
        (usage.input_uncached, usage.input_cache_read, usage.input_cache_write)
    {
        let sum = (u as i128) + (r as i128) + (w as i128);
        if sum <= i64::MAX as i128 {
            let worst = [quality.input_uncached, quality.input_cache_read, quality.input_cache_write]
                .contains(&FieldQuality::Estimated);
            return Some((sum as i64, if worst { FieldQuality::Estimated } else { FieldQuality::Derived }));
        }
    }
    usage.input_total.map(|v| (v, quality.input_total))
}

/// total_tokens：input_total 与 output_total 均已知时相加；否则保留源 total（含 basis）。
pub fn total_tokens(usage: &TokenUsage, quality: &TokenQuality) -> Option<(i64, FieldQuality)> {
    if let (Some((input, _)), Some(output)) = (input_total(usage, quality), usage.output_total) {
        let sum = (input as i128) + (output as i128);
        if sum <= i64::MAX as i128 {
            return Some((sum as i64, FieldQuality::Derived));
        }
    }
    usage.total_tokens.map(|v| (v, quality.total_tokens))
}

/// 一致性诊断：负值在 domain 校验拒绝；这里发现"缓存大于已知总输入"等矛盾，
/// 进入诊断，不用 max(0, …) 隐藏。
pub fn detect_contradictions(usage: &TokenUsage) -> Vec<Contradiction> {
    let mut out = Vec::new();
    if let (Some(read), Some(total)) = (usage.input_cache_read, usage.input_total) {
        if read > total {
            out.push(Contradiction {
                code: "cache_read_exceeds_input_total",
                field: "input_cache_read",
                detail: format!("cache_read {read} > input_total {total}"),
            });
        }
    }
    if let (Some(write), Some(total)) = (usage.input_cache_write, usage.input_total) {
        if write > total {
            out.push(Contradiction {
                code: "cache_write_exceeds_input_total",
                field: "input_cache_write",
                detail: format!("cache_write {write} > input_total {total}"),
            });
        }
    }
    if let (Some(read), Some(write), Some(total)) =
        (usage.input_cache_read, usage.input_cache_write, usage.input_total)
    {
        if (read as i128) + (write as i128) > total as i128 {
            out.push(Contradiction {
                code: "cache_sum_exceeds_input_total",
                field: "input_total",
                detail: format!("cache_read {read} + cache_write {write} > input_total {total}"),
            });
        }
    }
    if let (Some(reasoning), Some(output)) = (usage.output_reasoning, usage.output_total) {
        if reasoning > output {
            out.push(Contradiction {
                code: "reasoning_exceeds_output_total",
                field: "output_reasoning",
                detail: format!("reasoning {reasoning} > output_total {output}"),
            });
        }
    }
    if let (Some(uncached), Some(total)) = (usage.input_uncached, usage.input_total) {
        if uncached > total {
            out.push(Contradiction {
                code: "uncached_exceeds_input_total",
                field: "input_uncached",
                detail: format!("uncached {uncached} > input_total {total}"),
            });
        }
    }
    if let (Some(source), Some((normalized, _))) = (usage.source_total, total_tokens(usage, &TokenQuality::default()))
    {
        if source != normalized {
            out.push(Contradiction {
                code: "source_total_mismatch",
                field: "source_total",
                detail: format!("source_total {source} != normalized total {normalized}"),
            });
        }
    }
    out
}

/// 比例：分子/分母的精确整数对。分母为零或无有效样本时返回 None（显示"—"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio {
    pub numerator: i64,
    pub denominator: i64,
}

impl Ratio {
    pub fn new(numerator: i64, denominator: i64) -> Option<Self> {
        if denominator <= 0 {
            return None;
        }
        Some(Ratio { numerator, denominator })
    }

    pub fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

/// 一组记录的缓存输入占比：只对 input_total 与 input_cache_read 均已知的记录集合。
/// 返回 (ratio, 有效记录数)。无有效样本或分母为零时 ratio 为 None。
pub fn cache_input_ratio(samples: &[(Option<i64>, Option<i64>)]) -> (Option<Ratio>, i64) {
    let mut input_sum: i128 = 0;
    let mut read_sum: i128 = 0;
    let mut n: i64 = 0;
    for (input_total, cache_read) in samples {
        if let (Some(i), Some(r)) = (input_total, cache_read) {
            input_sum += *i as i128;
            read_sum += *r as i128;
            n += 1;
        }
    }
    if n == 0 || input_sum == 0 {
        return (None, n);
    }
    (
        Some(Ratio {
            numerator: read_sum as i64,
            denominator: input_sum as i64,
        }),
        n,
    )
}
