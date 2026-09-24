//! 来源原生区间汇总（source_aggregates）、累计快照求差与额度快照。
//!
//! 合同要点：
//! - interval_aggregate 保留原生范围、字段和单位，不伪装成逐次请求；
//! - cumulative_snapshot 按身份/版本/重置边界求差；首次值保留为源原生区间总量，
//!   区间跨日且无中间采样时不把全部 token 记入某一天，也不按时长摊分；
//! - 相同 series/start/end 的重复数据只处理一次；delta 也去重；
//! - 已证明覆盖互斥的汇总可求和；重叠覆盖的汇总标记 duplicate，不双计。

use crate::domain::{TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::identity::content_hash;
use crate::metrics::detect_contradictions;
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// 汇总范围类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateScope {
    Day,
    Session,
    ModelSeries,
    ProcessSeries,
    Custom,
}

impl AggregateScope {
    pub fn as_str(self) -> &'static str {
        match self {
            AggregateScope::Day => "day",
            AggregateScope::Session => "session",
            AggregateScope::ModelSeries => "model_series",
            AggregateScope::ProcessSeries => "process_series",
            AggregateScope::Custom => "custom",
        }
    }
}

/// 覆盖关系：已证明互斥才可求和；重叠的标记 duplicate 指向正主。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    /// 已证明与其他汇总互斥。
    Exclusive,
    /// 已被证明覆盖与另一条相同：仅作对照，不参与求和。
    Duplicate,
    /// 覆盖关系未知：分开展示，默认不自动叠加。
    OverlapUnknown,
}

impl Coverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Coverage::Exclusive => "exclusive",
            Coverage::Duplicate => "duplicate",
            Coverage::OverlapUnknown => "overlap_unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "exclusive" => Ok(Coverage::Exclusive),
            "duplicate" => Ok(Coverage::Duplicate),
            "overlap_unknown" => Ok(Coverage::OverlapUnknown),
            other => Err(CoreError::Validation(format!("unknown coverage: {other}"))),
        }
    }
}

/// 一条来源原生区间汇总（如 Hermes 两日累计行、OTel 累计区间）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAggregateInput {
    pub instance_id: String,
    pub scope: AggregateScope,
    /// 系列/行身份：资源+instrument+属性+进程实例+start_time，或来源行身份。
    pub scope_key: String,
    /// 区间起点；首次观察的累计值可能不知道 series start，为 None。
    pub interval_start_ms: Option<i64>,
    pub interval_end_ms: i64,
    /// 来源端点是闭区间语义（如 last_seen 瞬时）时为 true。
    pub interval_end_inclusive: bool,
    pub usage: TokenUsage,
    pub quality: TokenQuality,
    /// 来源报告调用汇总（如 api_call_count 区间累计）；不伪造逐次 model_call。
    pub reported_call_count: Option<i64>,
    pub coverage: Coverage,
    /// coverage = duplicate 时指向正主 aggregate 的 scope_key。
    pub duplicate_of: Option<String>,
    pub time_basis: TimeBasis,
    pub source_revision: Option<i64>,
}

impl SourceAggregateInput {
    pub fn validate(&self) -> Result<(), CoreError> {
        self.usage.validate()?;
        if let Some(start) = self.interval_start_ms {
            if self.interval_end_ms < start {
                return Err(CoreError::Validation(format!(
                    "aggregate interval end {} before start {start}",
                    self.interval_end_ms
                )));
            }
        }
        Ok(())
    }
}

/// upsert 一条区间汇总：相同 (instance, scope, scope_key) 按修订/内容幂等。
/// 返回是否发生变更。
pub fn upsert_source_aggregate(
    storage: &Storage,
    input: &SourceAggregateInput,
    now_ms: i64,
) -> Result<bool, CoreError> {
    input.validate()?;
    for c in detect_contradictions(&input.usage) {
        // 矛盾进入诊断，不隐藏。
        storage.conn().execute(
            "INSERT INTO diagnostics (batch_id, run_id, instance_id, event_id, code, field, position, message, created_ms)
             VALUES (NULL, NULL, ?1, NULL, ?2, ?3, NULL, ?4, ?5)",
            params![input.instance_id, c.code, c.field, c.detail, now_ms],
        )?;
    }
    let hash = content_hash(input);
    let aggregate_id = format!("{}#{}#{}", input.instance_id, input.scope.as_str(), input.scope_key);
    let existing: Option<(String, Option<i64>)> = storage
        .conn()
        .query_row(
            "SELECT content_hash, source_revision FROM source_aggregates
             WHERE instance_id = ?1 AND scope = ?2 AND scope_key = ?3",
            params![input.instance_id, input.scope.as_str(), input.scope_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    if let Some((old_hash, old_rev)) = &existing {
        // 重复扫描幂等：内容相同则不变更。
        if *old_hash == hash {
            return Ok(false);
        }
        // 有修订号时旧修订不覆盖新修订。
        if let (Some(new_rev), Some(old_rev)) = (input.source_revision, *old_rev) {
            if new_rev < old_rev {
                return Ok(false);
            }
        }
    }
    storage.conn().execute(
        "INSERT INTO source_aggregates (
           aggregate_id, instance_id, scope, scope_key, interval_start_ms, interval_end_ms,
           interval_end_inclusive, input_uncached, input_cache_read, input_cache_write, input_total,
           output_total, output_reasoning, total_tokens, source_total, quality_json,
           reported_call_count, coverage, duplicate_of, time_basis, source_revision, content_hash,
           created_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?23)
         ON CONFLICT(instance_id, scope, scope_key) DO UPDATE SET
           interval_start_ms = excluded.interval_start_ms,
           interval_end_ms = excluded.interval_end_ms,
           interval_end_inclusive = excluded.interval_end_inclusive,
           input_uncached = excluded.input_uncached,
           input_cache_read = excluded.input_cache_read,
           input_cache_write = excluded.input_cache_write,
           input_total = excluded.input_total,
           output_total = excluded.output_total,
           output_reasoning = excluded.output_reasoning,
           total_tokens = excluded.total_tokens,
           source_total = excluded.source_total,
           quality_json = excluded.quality_json,
           reported_call_count = excluded.reported_call_count,
           coverage = excluded.coverage,
           duplicate_of = excluded.duplicate_of,
           time_basis = excluded.time_basis,
           source_revision = excluded.source_revision,
           content_hash = excluded.content_hash,
           updated_at_ms = excluded.updated_at_ms",
        params![
            aggregate_id,
            input.instance_id,
            input.scope.as_str(),
            input.scope_key,
            input.interval_start_ms,
            input.interval_end_ms,
            input.interval_end_inclusive as i64,
            input.usage.input_uncached,
            input.usage.input_cache_read,
            input.usage.input_cache_write,
            input.usage.input_total,
            input.usage.output_total,
            input.usage.output_reasoning,
            input.usage.total_tokens,
            input.usage.source_total,
            serde_json::to_string(&input.quality)?,
            input.reported_call_count,
            input.coverage.as_str(),
            input.duplicate_of,
            input.time_basis.as_str(),
            input.source_revision,
            hash,
            now_ms
        ],
    )?;
    Ok(true)
}

/// 已证明互斥覆盖的汇总 token 求和（Hermes 样本 11：100+20=120 而非 220）。
/// duplicate 行不参与；overlap_unknown 行不参与并单独计数。
pub fn sum_exclusive_aggregates(
    storage: &Storage,
    instance_id: &str,
) -> Result<AggregateTotals, CoreError> {
    let mut totals = AggregateTotals::default();
    let mut stmt = storage.conn().prepare(
        "SELECT coverage, input_uncached, input_cache_read, input_cache_write, input_total,
                output_total, output_reasoning, total_tokens, reported_call_count
         FROM source_aggregates WHERE instance_id = ?1",
    )?;
    let rows = stmt.query_map(params![instance_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<i64>>(8)?,
        ))
    })?;
    for row in rows {
        let (coverage, uncached, read, write, input, output, reasoning, total, calls) = row?;
        match Coverage::parse(&coverage)? {
            Coverage::Exclusive => {
                totals.exclusive_rows += 1;
                add_opt(&mut totals.input_uncached, uncached)?;
                add_opt(&mut totals.input_cache_read, read)?;
                add_opt(&mut totals.input_cache_write, write)?;
                add_opt(&mut totals.input_total, input)?;
                add_opt(&mut totals.output_total, output)?;
                add_opt(&mut totals.output_reasoning, reasoning)?;
                add_opt(&mut totals.total_tokens, total)?;
                if let Some(c) = calls {
                    totals.reported_call_count = totals.reported_call_count.checked_add(c).ok_or(CoreError::Overflow("reported_call_count"))?;
                }
            }
            Coverage::Duplicate => totals.duplicate_rows += 1,
            Coverage::OverlapUnknown => totals.overlap_unknown_rows += 1,
        }
    }
    Ok(totals)
}

fn add_opt(acc: &mut Option<i64>, value: Option<i64>) -> Result<(), CoreError> {
    if let Some(v) = value {
        let base = acc.unwrap_or(0);
        *acc = Some(base.checked_add(v).ok_or(CoreError::Overflow("aggregate sum"))?);
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AggregateTotals {
    pub input_uncached: Option<i64>,
    pub input_cache_read: Option<i64>,
    pub input_cache_write: Option<i64>,
    pub input_total: Option<i64>,
    pub output_total: Option<i64>,
    pub output_reasoning: Option<i64>,
    pub total_tokens: Option<i64>,
    pub reported_call_count: i64,
    pub exclusive_rows: i64,
    pub duplicate_rows: i64,
    pub overlap_unknown_rows: i64,
}

/// 累计序列状态（持久化在 ingestion_checkpoints.parse_context）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CumulativeState {
    pub series_key: String,
    pub last_value: i64,
    pub last_observed_ms: i64,
    /// 进程/series 起点（已知时）。
    pub start_ms: Option<i64>,
}

/// 累计观察结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CumulativeOutcome {
    /// 首次看到累计值：保存为源原生区间总量，不硬塞进今天。
    FirstObservation { native_total: i64 },
    /// 区间增量（含 0）。
    Delta { amount: i64 },
    /// 明确重置证据（新进程）：新基线即新区间量。
    Reset { new_baseline: i64 },
    /// 累计值下降但缺少重置证据：不按零重新累加，记冲突诊断。
    Regression { previous: i64, observed: i64 },
}

/// 观察一次累计值。相同值重复观察返回 Delta 0（调用方幂等去重）。
pub fn observe_cumulative(
    series_key: &str,
    previous: Option<&CumulativeState>,
    value: i64,
    observed_ms: i64,
    reset_evidence: bool,
) -> (CumulativeState, CumulativeOutcome) {
    match previous {
        None => (
            CumulativeState { series_key: series_key.to_string(), last_value: value, last_observed_ms: observed_ms, start_ms: None },
            CumulativeOutcome::FirstObservation { native_total: value },
        ),
        Some(prev) => {
            if value > prev.last_value {
                (
                    CumulativeState { last_value: value, last_observed_ms: observed_ms, ..prev.clone() },
                    CumulativeOutcome::Delta { amount: value - prev.last_value },
                )
            } else if value == prev.last_value {
                (
                    CumulativeState { last_observed_ms: observed_ms, ..prev.clone() },
                    CumulativeOutcome::Delta { amount: 0 },
                )
            } else if reset_evidence {
                (
                    CumulativeState { series_key: series_key.to_string(), last_value: value, last_observed_ms: observed_ms, start_ms: Some(observed_ms) },
                    CumulativeOutcome::Reset { new_baseline: value },
                )
            } else {
                (
                    prev.clone(),
                    CumulativeOutcome::Regression { previous: prev.last_value, observed: value },
                )
            }
        }
    }
}

/// 额度快照输入。额度独立显示，不转成 token 或 request。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaSnapshotInput {
    pub quota_id: String,
    pub instance_id: String,
    pub observed_at_ms: i64,
    /// credits / subscription_window / balance / rate_limit。
    pub kind: String,
    /// 整数最小单位；未知为 None，不补零。
    pub quantity_minor: Option<i64>,
    pub unit: String,
    pub window_start_ms: Option<i64>,
    pub window_end_ms: Option<i64>,
    /// 仅在本地记录可证明属于本机使用时为 true；账号总额属排除范围。
    pub locality_verified: bool,
    pub detail: Option<serde_json::Value>,
}

pub fn insert_quota_snapshot(storage: &Storage, input: &QuotaSnapshotInput) -> Result<(), CoreError> {
    storage.conn().execute(
        "INSERT INTO quota_snapshots (
           quota_id, instance_id, observed_at_ms, kind, quantity_minor, unit,
           window_start_ms, window_end_ms, locality_verified, detail_json, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(quota_id) DO UPDATE SET
           observed_at_ms = excluded.observed_at_ms,
           kind = excluded.kind,
           quantity_minor = excluded.quantity_minor,
           unit = excluded.unit,
           window_start_ms = excluded.window_start_ms,
           window_end_ms = excluded.window_end_ms,
           locality_verified = excluded.locality_verified,
           detail_json = excluded.detail_json",
        params![
            input.quota_id,
            input.instance_id,
            input.observed_at_ms,
            input.kind,
            input.quantity_minor,
            input.unit,
            input.window_start_ms,
            input.window_end_ms,
            input.locality_verified as i64,
            input.detail.as_ref().map(serde_json::to_string).transpose()?,
            input.observed_at_ms
        ],
    )?;
    Ok(())
}
