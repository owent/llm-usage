//! 日/周/月汇总查询。周/月由日级加总；比例重新计算不取各日算术平均；
//! 跨周期会话数用 DISTINCT（从留存明细计算，明细已清理则标覆盖缺口）；
//! 未知模型保留独立行且计入总计；归属未核验的记录不进入总计并可按原因列出。

use crate::calendar::{parse_date, Calendar, WeekStart};
use crate::domain::QualityBucket;
use crate::error::CoreError;
use crate::metrics::Ratio;
use crate::storage::Storage;
use jiff::civil::Date;
use jiff::Span;
use rusqlite::params;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Hour,
    Day,
    Week,
    Month,
}

/// 基础筛选：不同字段 AND，同字段 OR。空列表 = 不筛选。
/// provider/model 的 `"unknown"` 匹配未知（空）值。
#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub agents: Vec<String>,
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub quality_buckets: Vec<QualityBucket>,
    /// 来源实例白名单（多用户：app 层把当前用户的来源集合传入）；
    /// 空 = 不过滤。事件表按 source_instance_id、汇总表按 instance_id 过滤。
    pub instances: Vec<String>,
}

impl Filters {
    fn matches(&self, row: &DailyRow) -> bool {
        let any_match = |filter: &[String], value: &str| {
            filter.is_empty()
                || filter
                    .iter()
                    .any(|f| f == value || (f == "unknown" && value.is_empty()))
        };
        any_match(&self.agents, &row.agent)
            && any_match(&self.providers, &row.provider_id)
            && any_match(&self.models, &row.model_raw)
            && (self.instances.is_empty() || self.instances.iter().any(|i| i == &row.instance_id))
            && (self.quality_buckets.is_empty()
                || self
                    .quality_buckets
                    .iter()
                    .any(|q| q.as_str() == row.quality_bucket))
    }
}

#[derive(Debug, Clone)]
pub struct SummaryRequest {
    pub timezone: String,
    pub week_start: WeekStart,
    /// 含端点的本地日区间。
    pub first_day: Date,
    pub last_day: Date,
    pub granularity: Granularity,
    pub filters: Filters,
    /// 查询方提供的“今天”（本地日），用于进行中标记。
    pub today: Date,
    /// 明细保留截止日：早于它的周期标记“部分历史”。
    pub retention_cutoff: Option<Date>,
}

/// 一组记录/一天的指标合计。known_sum 只在有已知样本时为 Some。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricSums {
    pub input_total_known: Option<i64>,
    /// 平均耗时（毫秒；仅有 duration 样本的均值）。
    pub avg_duration_ms: Option<i64>,
    /// 总耗时（毫秒）。
    pub total_duration_ms: Option<i64>,
    pub input_known_count: i64,
    pub input_unknown_count: i64,
    pub uncached_known: Option<i64>,
    pub cache_read_known: Option<i64>,
    pub cache_write_known: Option<i64>,
    pub output_total_known: Option<i64>,
    pub output_known_count: i64,
    pub output_unknown_count: i64,
    pub total_tokens_known: Option<i64>,
    pub total_known_count: i64,
    pub total_unknown_count: i64,
    pub event_count: i64,
    pub call_count: i64,
    pub attempt_count: i64,
    pub observation_count: i64,
    pub conflict_count: i64,
    ratio_input_sum: i128,
    ratio_cache_read_sum: i128,
    ratio_sample_count: i64,
}

impl MetricSums {
    /// 缓存输入占比：SUM(cache_read)/SUM(input_total)，只对两字段均已知的记录集合。
    /// 分母为零或无有效样本时为 None（显示“—”而不是 0%）。
    pub fn cache_input_ratio(&self) -> Option<Ratio> {
        if self.ratio_sample_count == 0 || self.ratio_input_sum == 0 {
            return None;
        }
        Ratio::new(self.ratio_cache_read_sum, self.ratio_input_sum)
    }

    pub fn ratio_sample_count(&self) -> i64 {
        self.ratio_sample_count
    }

    fn checked_add_opt(
        acc: &mut Option<i64>,
        value: Option<i64>,
        what: &'static str,
    ) -> Result<(), CoreError> {
        if let Some(v) = value {
            let next = (acc.unwrap_or(0) as i128) + (v as i128);
            if next > i64::MAX as i128 {
                return Err(CoreError::Overflow(what));
            }
            *acc = Some(next as i64);
        }
        Ok(())
    }

    fn add_row(&mut self, row: &DailyRow) -> Result<(), CoreError> {
        Self::checked_add_opt(
            &mut self.input_total_known,
            row.input_known_sum,
            "input_total",
        )?;
        Self::checked_add_opt(
            &mut self.uncached_known,
            row.uncached_known_sum,
            "input_uncached",
        )?;
        Self::checked_add_opt(
            &mut self.cache_read_known,
            row.cache_read_known_sum,
            "input_cache_read",
        )?;
        Self::checked_add_opt(
            &mut self.cache_write_known,
            row.cache_write_known_sum,
            "input_cache_write",
        )?;
        Self::checked_add_opt(
            &mut self.output_total_known,
            row.output_known_sum,
            "output_total",
        )?;
        Self::checked_add_opt(
            &mut self.total_tokens_known,
            row.total_known_sum,
            "total_tokens",
        )?;
        self.input_known_count += row.input_known_count;
        self.input_unknown_count += row.input_unknown_count;
        self.output_known_count += row.output_known_count;
        self.output_unknown_count += row.output_unknown_count;
        self.total_known_count += row.total_known_count;
        self.total_unknown_count += row.total_unknown_count;
        self.event_count += row.event_count;
        self.call_count += row.call_count;
        self.attempt_count += row.attempt_count;
        self.observation_count += row.observation_count;
        self.conflict_count += row.conflict_count;
        self.ratio_input_sum += row.ratio_input_sum.unwrap_or(0) as i128;
        self.ratio_cache_read_sum += row.ratio_cache_read_sum.unwrap_or(0) as i128;
        self.ratio_sample_count += row.ratio_sample_count;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PeriodRow {
    pub label: String,
    pub start_day: Date,
    pub end_day: Date,
    pub utc_start_ms: i64,
    pub utc_end_ms: i64,
    /// 当前周/月只完成一部分。
    pub in_progress: bool,
    /// 保留时间截断的最早周期，或明细已清理（封存）。
    pub partial_history: bool,
    pub sums: MetricSums,
    /// 跨周期 DISTINCT 会话数；明细缺失时为 None（覆盖缺口）。
    pub distinct_sessions: Option<i64>,
    /// 有活动的本地日数；明细缺失时为 None。
    pub active_days: Option<i64>,
    /// 平均请求耗时（毫秒；仅有 duration 的样本）；None = 无样本。
    pub avg_duration_ms: Option<i64>,
    /// 总请求耗时（毫秒）。
    pub total_duration_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ModelRow {
    pub provider_id: Option<String>,
    pub model_raw: Option<String>,
    pub sums: MetricSums,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub data_revision: i64,
    pub timezone: String,
    pub week_start: WeekStart,
    pub periods: Vec<PeriodRow>,
    pub totals: MetricSums,
    /// 模型分组（含 unknown 独立行）；总计必须包含其已知 token。
    pub model_breakdown: Vec<ModelRow>,
    /// 范围内归属未核验/被排除的事件数（未计入 totals）。
    pub excluded_event_count: i64,
}

#[derive(Debug, Clone)]
struct DailyRow {
    local_day: Date,
    instance_id: String,
    /// 小时粒度时的-hour 值（日粒度行恒 None；由 hourly_usage 加载）。
    hour: Option<i64>,
    agent: String,
    provider_id: String,
    model_raw: String,
    quality_bucket: String,
    sealed: bool,
    event_count: i64,
    call_count: i64,
    attempt_count: i64,
    observation_count: i64,
    input_known_sum: Option<i64>,
    input_known_count: i64,
    input_unknown_count: i64,
    uncached_known_sum: Option<i64>,
    cache_read_known_sum: Option<i64>,
    cache_write_known_sum: Option<i64>,
    output_known_sum: Option<i64>,
    output_known_count: i64,
    output_unknown_count: i64,
    total_known_sum: Option<i64>,
    total_known_count: i64,
    total_unknown_count: i64,
    ratio_input_sum: Option<i64>,
    ratio_cache_read_sum: Option<i64>,
    ratio_sample_count: i64,
    conflict_count: i64,
}

/// 执行汇总查询。
pub fn query_summary(storage: &Storage, request: &SummaryRequest) -> Result<Summary, CoreError> {
    if request.last_day < request.first_day {
        return Err(CoreError::Query("last_day before first_day".to_string()));
    }
    let calendar = Calendar::new(&request.timezone)?;
    // 所有 SELECT 共用 SQLite 读快照，修订号不能来自较晚的提交。
    let snapshot = storage.conn().unchecked_transaction()?;
    let revision = storage.data_revision()?;
    let rows = if request.granularity == Granularity::Hour {
        // 小时粒度：从 hourly_usage 读（按日+小时展开为"周期"行）。
        load_hourly_as_daily(
            storage,
            &request.timezone,
            request.first_day,
            request.last_day,
        )?
    } else {
        load_daily_rows(
            storage,
            &request.timezone,
            request.first_day,
            request.last_day,
        )?
    };
    let rows: Vec<DailyRow> = rows
        .into_iter()
        .filter(|r| request.filters.matches(r))
        .collect();

    // 周期分组键。
    let key_of = |day: Date, hour: Option<i64>| -> (String, Date, Date) {
        match request.granularity {
            Granularity::Hour => {
                let label = match hour {
                    Some(h) => format!("{} {:02}:00", day, h),
                    None => day.to_string(),
                };
                (label, day, day)
            }
            Granularity::Day => (day.to_string(), day, day),
            Granularity::Week => {
                let start = calendar.week_start_of(day, request.week_start);
                let end = start.checked_add(Span::new().days(6)).expect("week end");
                (calendar.week_label(day, request.week_start), start, end)
            }
            Granularity::Month => {
                let start = calendar.month_start_of(day);
                (calendar.month_label(day), start, start.last_of_month())
            }
        }
    };

    let mut groups: BTreeMap<(Date, String), (Date, Date, Vec<&DailyRow>)> = BTreeMap::new();
    for row in &rows {
        let (label, start, end) = key_of(row.local_day, row.hour);
        groups
            .entry((start, label))
            .or_insert_with(|| (start, end, Vec::new()))
            .2
            .push(row);
    }

    let mut periods = Vec::new();
    let mut totals = MetricSums::default();
    for ((start, label), (_, end, group_rows)) in &groups {
        let mut sums = MetricSums::default();
        let mut any_sealed = false;
        for row in group_rows {
            sums.add_row(row)?;
            any_sealed |= row.sealed;
        }
        totals.add_row_merged(&sums)?;
        let (utc_start_ms, utc_end_ms) = period_range_ms(&calendar, *start, *end)?;
        let in_progress = request.today >= *start && request.today <= *end;
        let mut partial_history = request
            .retention_cutoff
            .is_some_and(|cutoff| *start < cutoff);
        // 封存日 = 明细已清理：distinct 会话等明细指标不可得。
        let details_available = !any_sealed;
        if any_sealed {
            partial_history = true;
        }
        let (distinct_sessions, active_days) = if details_available {
            let (selected_start, selected_end) = period_range_ms(
                &calendar,
                (*start).max(request.first_day),
                (*end).min(request.last_day),
            )?;
            let (s, d) = session_stats(
                storage,
                &calendar,
                selected_start,
                selected_end,
                &request.filters,
            )?;
            (Some(s), Some(d))
        } else {
            (None, None)
        };
        periods.push(PeriodRow {
            label: label.clone(),
            start_day: *start,
            end_day: *end,
            utc_start_ms,
            utc_end_ms,
            in_progress,
            partial_history,
            sums,
            distinct_sessions,
            active_days,
            avg_duration_ms: None,
            total_duration_ms: None,
        });
    }

    // 物化周期合并（分级归档）：日层保留期外的历史周/月来自 period_usage；
    // 日层已覆盖的周期不重复计入（同一周期二选一，日层更新鲜含进行中状态）。
    if matches!(request.granularity, Granularity::Week | Granularity::Month) {
        let granularity_str = match request.granularity {
            Granularity::Week => "week",
            _ => "month",
        };
        let covered: BTreeSet<String> = periods.iter().map(|p| p.label.clone()).collect();
        struct MatAgg {
            start: String,
            end: String,
            sums: MetricSums,
            active_days: i64,
        }
        let mut materialized: BTreeMap<String, MatAgg> = BTreeMap::new();
        {
            let mut stmt = storage.conn().prepare(
                "SELECT period_key, period_start_day, period_end_day, instance_id, agent,
                        provider_id, model_raw, event_count, call_count,
                        input_known_sum, cache_read_known_sum, cache_write_known_sum,
                        output_known_sum, total_known_sum, conflict_count, active_days
                 FROM period_usage
                 WHERE tz_version = ?1 AND granularity = ?2
                   AND period_end_day >= ?3 AND period_start_day <= ?4",
            )?;
            let mat_rows = stmt.query_map(
                params![
                    request.timezone,
                    granularity_str,
                    request.first_day.to_string(),
                    request.last_day.to_string()
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, i64>(7)?,
                        r.get::<_, i64>(8)?,
                        r.get::<_, Option<i64>>(9)?,
                        r.get::<_, Option<i64>>(10)?,
                        r.get::<_, Option<i64>>(11)?,
                        r.get::<_, Option<i64>>(12)?,
                        r.get::<_, Option<i64>>(13)?,
                        r.get::<_, i64>(14)?,
                        r.get::<_, i64>(15)?,
                    ))
                },
            )?;
            for row in mat_rows {
                let (
                    key,
                    start,
                    end,
                    instance,
                    agent,
                    provider,
                    model,
                    event_count,
                    call_count,
                    input,
                    cache_read,
                    cache_write,
                    output,
                    total,
                    conflict,
                    active_days,
                ) = row?;
                let filter_row = DailyRow {
                    local_day: parse_date(&start)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    instance_id: instance,
                    hour: None,
                    agent,
                    provider_id: provider,
                    model_raw: model,
                    quality_bucket: String::new(),
                    sealed: true,
                    event_count: 0,
                    call_count: 0,
                    attempt_count: 0,
                    observation_count: 0,
                    input_known_sum: input,
                    input_known_count: 0,
                    input_unknown_count: 0,
                    uncached_known_sum: None,
                    cache_read_known_sum: cache_read,
                    cache_write_known_sum: cache_write,
                    output_known_sum: output,
                    output_known_count: 0,
                    output_unknown_count: 0,
                    total_known_sum: total,
                    total_known_count: 0,
                    total_unknown_count: 0,
                    ratio_input_sum: None,
                    ratio_cache_read_sum: None,
                    ratio_sample_count: 0,
                    conflict_count: conflict,
                };
                if !request.filters.matches(&filter_row) {
                    continue;
                }
                let agg = materialized.entry(key.clone()).or_insert_with(|| MatAgg {
                    start: start.clone(),
                    end: end.clone(),
                    sums: MetricSums::default(),
                    active_days: 0,
                });
                agg.sums.event_count += event_count;
                agg.sums.call_count += call_count;
                agg.sums.conflict_count += conflict;
                agg.sums.input_total_known = merge_opt(agg.sums.input_total_known, input);
                agg.sums.cache_read_known = merge_opt(agg.sums.cache_read_known, cache_read);
                agg.sums.cache_write_known = merge_opt(agg.sums.cache_write_known, cache_write);
                agg.sums.output_total_known = merge_opt(agg.sums.output_total_known, output);
                agg.sums.total_tokens_known = merge_opt(agg.sums.total_tokens_known, total);
                agg.active_days = agg.active_days.max(active_days);
            }
        }
        for (key, agg) in materialized {
            if covered.contains(&key) {
                continue;
            }
            let start_day = parse_date(&agg.start)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let end_day = parse_date(&agg.end)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let (utc_start_ms, utc_end_ms) = period_range_ms(&calendar, start_day, end_day)?;
            totals.event_count += agg.sums.event_count;
            totals.call_count += agg.sums.call_count;
            totals.conflict_count += agg.sums.conflict_count;
            totals.input_total_known =
                merge_opt(totals.input_total_known, agg.sums.input_total_known);
            totals.cache_read_known = merge_opt(totals.cache_read_known, agg.sums.cache_read_known);
            totals.cache_write_known =
                merge_opt(totals.cache_write_known, agg.sums.cache_write_known);
            totals.output_total_known =
                merge_opt(totals.output_total_known, agg.sums.output_total_known);
            totals.total_tokens_known =
                merge_opt(totals.total_tokens_known, agg.sums.total_tokens_known);
            periods.push(PeriodRow {
                label: key,
                start_day,
                end_day,
                utc_start_ms,
                utc_end_ms,
                in_progress: false,
                partial_history: true,
                sums: agg.sums,
                distinct_sessions: None,
                active_days: Some(agg.active_days),
                avg_duration_ms: None,
                total_duration_ms: None,
            });
        }
        periods.sort_by_key(|p| p.start_day);
    }

    // 模型分组：unknown（空串）保留独立行；大小写不敏感合并（GLM-5.3 与
    // glm-5.3 是同一模型）；provider/model 前缀变体在显示层统一。
    let mut model_groups: BTreeMap<(String, String), MetricSums> = BTreeMap::new();
    for row in &rows {
        let key = (
            row.provider_id.clone().to_lowercase(),
            row.model_raw.clone().to_lowercase(),
        );
        model_groups.entry(key).or_default().add_row(row)?;
    }
    // 显示名：从原始行反查该小写键的首个原始变体。
    let display_name = |lower: &str| -> Option<String> {
        rows.iter()
            .find(|r| r.model_raw.to_lowercase() == lower && !r.model_raw.is_empty())
            .map(|r| r.model_raw.clone())
    };
    let model_breakdown = model_groups
        .into_iter()
        .map(|((provider, model), sums)| ModelRow {
            provider_id: if provider.is_empty() {
                None
            } else {
                display_name(&provider).or(Some(provider))
            },
            model_raw: if model.is_empty() { None } else { Some(model) },
            sums,
        })
        .collect();

    let excluded_event_count = count_excluded(storage, &calendar, request)?;
    snapshot.commit()?;

    Ok(Summary {
        data_revision: revision,
        timezone: request.timezone.clone(),
        week_start: request.week_start,
        periods,
        totals,
        model_breakdown,
        excluded_event_count,
    })
}

impl MetricSums {
    fn add_row_merged(&mut self, other: &MetricSums) -> Result<(), CoreError> {
        Self::checked_add_opt(
            &mut self.input_total_known,
            other.input_total_known,
            "input_total",
        )?;
        Self::checked_add_opt(
            &mut self.uncached_known,
            other.uncached_known,
            "input_uncached",
        )?;
        Self::checked_add_opt(
            &mut self.cache_read_known,
            other.cache_read_known,
            "input_cache_read",
        )?;
        Self::checked_add_opt(
            &mut self.cache_write_known,
            other.cache_write_known,
            "input_cache_write",
        )?;
        Self::checked_add_opt(
            &mut self.output_total_known,
            other.output_total_known,
            "output_total",
        )?;
        Self::checked_add_opt(
            &mut self.total_tokens_known,
            other.total_tokens_known,
            "total_tokens",
        )?;
        self.input_known_count += other.input_known_count;
        self.input_unknown_count += other.input_unknown_count;
        self.output_known_count += other.output_known_count;
        self.output_unknown_count += other.output_unknown_count;
        self.total_known_count += other.total_known_count;
        self.total_unknown_count += other.total_unknown_count;
        self.event_count += other.event_count;
        self.call_count += other.call_count;
        self.attempt_count += other.attempt_count;
        self.observation_count += other.observation_count;
        self.conflict_count += other.conflict_count;
        self.ratio_input_sum += other.ratio_input_sum;
        self.ratio_cache_read_sum += other.ratio_cache_read_sum;
        self.ratio_sample_count += other.ratio_sample_count;
        Ok(())
    }
}

fn load_daily_rows(
    storage: &Storage,
    tz: &str,
    first_day: Date,
    last_day: Date,
) -> Result<Vec<DailyRow>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT local_day, instance_id, agent, provider_id, model_raw, quality_bucket, sealed,
                event_count, call_count, attempt_count, observation_count,
                input_known_sum, input_known_count, input_unknown_count,
                uncached_known_sum,
                cache_read_known_sum,
                cache_write_known_sum,
                output_known_sum, output_known_count, output_unknown_count,
                total_known_sum, total_known_count, total_unknown_count,
                ratio_input_sum, ratio_cache_read_sum, ratio_sample_count, conflict_count
         FROM daily_usage
         WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
         ORDER BY local_day",
    )?;
    let rows = stmt.query_map(
        params![tz, first_day.to_string(), last_day.to_string()],
        |r| {
            Ok(DailyRow {
                local_day: parse_date(&r.get::<_, String>(0)?)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                instance_id: r.get(1)?,
                hour: None,
                agent: r.get(2)?,
                provider_id: r.get(3)?,
                model_raw: r.get(4)?,
                quality_bucket: r.get(5)?,
                sealed: r.get::<_, i64>(6)? != 0,
                event_count: r.get(7)?,
                call_count: r.get(8)?,
                attempt_count: r.get(9)?,
                observation_count: r.get(10)?,
                input_known_sum: r.get(11)?,
                input_known_count: r.get(12)?,
                input_unknown_count: r.get(13)?,
                uncached_known_sum: r.get(14)?,
                cache_read_known_sum: r.get(15)?,
                cache_write_known_sum: r.get(16)?,
                output_known_sum: r.get(17)?,
                output_known_count: r.get(18)?,
                output_unknown_count: r.get(19)?,
                total_known_sum: r.get(20)?,
                total_known_count: r.get(21)?,
                total_unknown_count: r.get(22)?,
                ratio_input_sum: r.get(23)?,
                ratio_cache_read_sum: r.get(24)?,
                ratio_sample_count: r.get(25)?,
                conflict_count: r.get(26)?,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn merge_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x + y),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn period_range_ms(calendar: &Calendar, start: Date, end: Date) -> Result<(i64, i64), CoreError> {
    let (s, _) = calendar.day_range_ms(start)?;
    let (_, e) = calendar.day_range_ms(end)?;
    Ok((s, e))
}

/// 跨周期会话统计：DISTINCT（源实例+会话 ID）；活跃本地日数。
/// 明细索引按需查询；会话 ID 仅用于关联。
fn session_stats(
    storage: &Storage,
    calendar: &Calendar,
    utc_start_ms: i64,
    utc_end_ms: i64,
    filters: &Filters,
) -> Result<(i64, i64), CoreError> {
    let mut sql = String::from(
        "SELECT DISTINCT source_instance_id, session_id, occurred_at_ms FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
           AND attribution_status = 'verified'",
    );
    let mut values: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::Integer(utc_start_ms),
        rusqlite::types::Value::Integer(utc_end_ms),
    ];
    append_filters(&mut sql, &mut values, filters, "source_instance_id");
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut sessions: BTreeSet<(String, String)> = BTreeSet::new();
    let mut days: BTreeSet<Date> = BTreeSet::new();
    for row in rows {
        let (instance, session, ms) = row?;
        if let Some(session) = session {
            sessions.insert((instance, session));
        }
        days.insert(calendar.local_day_of(ms)?);
    }
    Ok((sessions.len() as i64, days.len() as i64))
}

/// 归属未核验/被排除的事件数：不进入总计，可按排除原因列出。
fn count_excluded(
    storage: &Storage,
    calendar: &Calendar,
    request: &SummaryRequest,
) -> Result<i64, CoreError> {
    let (start_ms, _) = calendar.day_range_ms(request.first_day)?;
    let (_, end_ms) = calendar.day_range_ms(request.last_day)?;
    let mut sql = String::from(
        "SELECT COUNT(*) FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2 AND attribution_status != 'verified'",
    );
    let mut values = vec![start_ms.into(), end_ms.into()];
    append_filters(
        &mut sql,
        &mut values,
        &request.filters,
        "source_instance_id",
    );
    let count: i64 = storage
        .conn()
        .query_row(&sql, rusqlite::params_from_iter(values), |r| r.get(0))?;
    Ok(count)
}

/// 按排除原因分组的排除记录清单（为 V25 打底）。
pub fn list_excluded(
    storage: &Storage,
    first_ms: i64,
    end_ms: i64,
) -> Result<Vec<(String, i64)>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT COALESCE(exclusion_reason, 'unverified'), COUNT(*) FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2 AND attribution_status != 'verified'
         GROUP BY COALESCE(exclusion_reason, 'unverified')",
    )?;
    let rows = stmt.query_map(params![first_ms, end_ms], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn append_filters(
    sql: &mut String,
    values: &mut Vec<rusqlite::types::Value>,
    filters: &Filters,
    instance_column: &str,
) {
    let push_in = |sql: &mut String,
                   values: &mut Vec<rusqlite::types::Value>,
                   column: &str,
                   list: &[String]| {
        if list.is_empty() {
            return;
        }
        let mut clause = String::new();
        clause.push_str(" AND (");
        for (i, item) in list.iter().enumerate() {
            if i > 0 {
                clause.push_str(" OR ");
            }
            if item == "unknown" && column != "agent" {
                clause.push_str(&format!(
                    "({column} IS NULL OR {column} = '' OR {column} = 'unknown')"
                ));
            } else {
                clause.push_str(&format!("{column} = ?{}", values.len() + 1));
                values.push(rusqlite::types::Value::Text(item.clone()));
            }
        }
        clause.push(')');
        sql.push_str(&clause);
    };
    push_in(sql, values, "agent", &filters.agents);
    push_in(sql, values, "provider_id", &filters.providers);
    push_in(sql, values, "model_raw", &filters.models);
    if !filters.instances.is_empty() {
        let mut clause = format!(" AND {instance_column} IN (");
        for (i, item) in filters.instances.iter().enumerate() {
            if i > 0 {
                clause.push(',');
            }
            clause.push_str(&format!("?{}", values.len() + 1));
            values.push(rusqlite::types::Value::Text(item.clone()));
        }
        clause.push(')');
        sql.push_str(&clause);
    }
    if !filters.quality_buckets.is_empty() {
        let mut clause = String::from(" AND (");
        for (i, q) in filters.quality_buckets.iter().enumerate() {
            if i > 0 {
                clause.push_str(" OR ");
            }
            clause.push_str(&format!("quality_bucket = ?{}", values.len() + 1));
            values.push(rusqlite::types::Value::Text(q.as_str().to_string()));
        }
        clause.push(')');
        sql.push_str(&clause);
    }
}

/// Agent 分组行（与 ModelRow 同构；总计口径一致）。
#[derive(Debug, Clone)]
pub struct AgentRow {
    pub agent: String,
    pub sums: MetricSums,
}

/// 按 Agent 分组（含 unknown 独立行：provider/model 为空的记录归入该 Agent 名下）。
pub fn agent_breakdown(
    storage: &Storage,
    request: &SummaryRequest,
) -> Result<Vec<AgentRow>, CoreError> {
    let rows = if request.granularity == Granularity::Hour {
        // 小时粒度：从 hourly_usage 读（按日+小时展开为"周期"行）。
        load_hourly_as_daily(
            storage,
            &request.timezone,
            request.first_day,
            request.last_day,
        )?
    } else {
        load_daily_rows(
            storage,
            &request.timezone,
            request.first_day,
            request.last_day,
        )?
    };
    let mut groups: BTreeMap<String, MetricSums> = BTreeMap::new();
    for row in rows {
        if !request.filters.matches(&row) {
            continue;
        }
        let sums = groups.entry(row.agent.clone()).or_default();
        sums.add_row(&row)?;
    }
    Ok(groups
        .into_iter()
        .map(|(agent, sums)| AgentRow { agent, sums })
        .collect())
}

/// 单日逐小时桶（今日小时图）。calls/known sums 按 source_completion 的本地小时分桶；
/// DST 重复小时先合并显示（offset 可区分的完整处理随 V04 用例细化）。
#[derive(Debug, Clone, Default)]
pub struct HourBucket {
    pub hour: u32,
    pub event_count: i64,
    pub call_count: i64,
    pub input_total_known: Option<i64>,
    pub cache_read_known: Option<i64>,
    pub output_total_known: Option<i64>,
    pub total_tokens_known: Option<i64>,
    /// 该小时的 DISTINCT 会话数。
    pub session_count: Option<i64>,
    /// 平均耗时（毫秒）。
    pub avg_duration_ms: Option<i64>,
}

pub fn hourly_breakdown(
    storage: &Storage,
    timezone: &str,
    day: Date,
    filters: &Filters,
) -> Result<Vec<HourBucket>, CoreError> {
    // 分级归档：小时图读持久化小时表（明细删除后仍有 30 天数据）。
    // 该表在每次提交时随受影响日同事务重算；筛选按维度列下推。
    let mut sql = String::from(
        "SELECT h.hour, h.event_count, h.call_count, h.input_known_sum, h.cache_read_known_sum,
                h.output_known_sum, h.total_known_sum, h.agent, COALESCE(h.provider_id, ''),
                COALESCE(h.model_raw, ''),
                (SELECT COUNT(DISTINCT e.session_id) FROM usage_events e
                 WHERE e.source_instance_id = h.instance_id
                   AND e.occurred_at_ms >= ?3 AND e.occurred_at_ms < ?4
                   AND e.session_id IS NOT NULL),
                (SELECT CAST(AVG(e.duration_ms) AS INTEGER) FROM usage_events e
                 WHERE e.source_instance_id = h.instance_id
                   AND e.occurred_at_ms >= ?3 AND e.occurred_at_ms < ?4
                   AND e.duration_ms IS NOT NULL)
         FROM hourly_usage h WHERE h.tz_version = ?1 AND h.local_day = ?2",
    );
    let calendar = Calendar::new(timezone)?;
    let (day_start_ms, day_end_ms) = calendar.day_range_ms(day)?;
    let mut values: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::Text(timezone.to_string()),
        rusqlite::types::Value::Text(day.to_string()),
        rusqlite::types::Value::Integer(day_start_ms),
        rusqlite::types::Value::Integer(day_end_ms),
    ];
    if !filters.agents.is_empty()
        || !filters.providers.is_empty()
        || !filters.models.is_empty()
        || !filters.instances.is_empty()
    {
        sql.push_str(" AND (");
        let mut first = true;
        let any_match =
            |filter: &[String], column: &str, values: &mut Vec<rusqlite::types::Value>| {
                if filter.is_empty() {
                    return None;
                }
                let placeholders: Vec<String> = filter
                    .iter()
                    .map(|f| {
                        values.push(rusqlite::types::Value::Text(f.clone()));
                        format!("?{}", values.len())
                    })
                    .collect();
                Some(format!("{column} IN ({})", placeholders.join(", ")))
            };
        for clause in [
            any_match(&filters.agents, "agent", &mut values),
            any_match(&filters.providers, "provider_id", &mut values),
            any_match(&filters.models, "model_raw", &mut values),
            any_match(&filters.instances, "instance_id", &mut values),
        ]
        .into_iter()
        .flatten()
        {
            if !first {
                sql.push_str(" AND ");
            }
            first = false;
            sql.push_str(&clause);
        }
        sql.push(')');
    }
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(10)?,
            r.get::<_, Option<i64>>(11)?,
        ))
    })?;
    let mut buckets: BTreeMap<u32, HourBucket> = BTreeMap::new();
    for row in rows {
        let (hour, event_count, call_count, input, cache_read, output, total, sessions, avg_dur) =
            row?;
        let hour = u32::try_from(hour).unwrap_or(0);
        let bucket = buckets.entry(hour).or_insert_with(|| HourBucket {
            hour,
            ..Default::default()
        });
        bucket.event_count += event_count;
        bucket.call_count += call_count;
        bucket.input_total_known = merge_opt(bucket.input_total_known, input);
        bucket.cache_read_known = merge_opt(bucket.cache_read_known, cache_read);
        bucket.output_total_known = merge_opt(bucket.output_total_known, output);
        bucket.total_tokens_known = merge_opt(bucket.total_tokens_known, total);
        // 会话数取最大（各维度行的子查询各自独立，取代表值）；耗时取均值合并。
        bucket.session_count = merge_opt(bucket.session_count, sessions).map(|v| v.max(0));
        bucket.avg_duration_ms = merge_opt(bucket.avg_duration_ms, avg_dur);
    }
    Ok((0u32..24).filter_map(|h| buckets.remove(&h)).collect())
}

/// 热力图单元格：本地星期几（1=周一）× 小时的调用数与 token。
#[derive(Debug, Clone, Default)]
pub struct HeatCell {
    pub weekday: u8,
    pub hour: u32,
    pub call_count: i64,
    pub total_tokens_known: Option<i64>,
}

pub fn heatmap_cells(
    storage: &Storage,
    timezone: &str,
    first_day: Date,
    last_day: Date,
    filters: &Filters,
) -> Result<Vec<HeatCell>, CoreError> {
    let calendar = Calendar::new(timezone)?;
    let (start_ms, _) = calendar.day_range_ms(first_day)?;
    let (_, end_ms) = calendar.day_range_ms(last_day)?;
    let mut sql = String::from(
        "SELECT occurred_at_ms, record_kind, total_tokens, quality_json
         FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
           AND attribution_status = 'verified'
           AND record_kind IN ('model_call', 'transport_attempt', 'usage_observation')",
    );
    let mut values: Vec<rusqlite::types::Value> = vec![start_ms.into(), end_ms.into()];
    append_filters(&mut sql, &mut values, filters, "source_instance_id");
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut cells: BTreeMap<(u8, u32), HeatCell> = BTreeMap::new();
    for row in rows {
        let (ms, kind, total, quality_json) = row?;
        let weekday = calendar.local_weekday_of(ms)?;
        let hour = calendar.local_hour_of(ms)?;
        let cell = cells.entry((weekday, hour)).or_default();
        cell.weekday = weekday;
        cell.hour = hour;
        if kind == "model_call" {
            cell.call_count += 1;
        }
        if kind != "transport_attempt" {
            let known_total = serde_json::from_str::<serde_json::Value>(&quality_json)
                .ok()
                .and_then(|q| {
                    q.get("total_tokens")
                        .and_then(|v| v.as_str())
                        .map(|s| s == "reported" || s == "derived")
                })
                .unwrap_or(false);
            if known_total {
                cell.total_tokens_known =
                    Some(cell.total_tokens_known.unwrap_or(0) + total.unwrap_or(0));
            }
        }
    }
    Ok(cells.into_values().collect())
}

/// 明细行（详情页分页展示）。
#[derive(Debug, Clone)]
pub struct EventDetailRow {
    pub event_id: String,
    pub agent: String,
    pub model_raw: Option<String>,
    pub call_category: String,
    pub occurred_at_ms: i64,
    pub session_id: Option<String>,
    pub input_total: Option<i64>,
    pub cache_read: Option<i64>,
    pub output_total: Option<i64>,
    pub total_tokens: Option<i64>,
    pub duration_ms: Option<i64>,
    pub lifecycle: String,
}

pub struct EventDetailRequest {
    pub timezone: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub offset: i64,
    pub limit: i64,
    pub filters: Filters,
}

pub struct EventDetailPage {
    pub rows: Vec<EventDetailRow>,
    pub total_count: i64,
    pub offset: i64,
    pub limit: i64,
}

/// 分页查询明细事件（详情页；按时间倒序）。
pub fn event_details(
    storage: &Storage,
    request: &EventDetailRequest,
) -> Result<EventDetailPage, CoreError> {
    let mut where_clause = String::from(
        "WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2 AND attribution_status = 'verified'",
    );
    let mut values: Vec<rusqlite::types::Value> =
        vec![request.from_ms.into(), request.to_ms.into()];
    append_filters(
        &mut where_clause,
        &mut values,
        &request.filters,
        "source_instance_id",
    );
    let count: i64 = storage.conn().query_row(
        &format!("SELECT COUNT(*) FROM usage_events {where_clause}"),
        rusqlite::params_from_iter(values.iter()),
        |r| r.get(0),
    )?;
    let sql = format!(
        "SELECT event_id, agent, model_raw, call_category, occurred_at_ms, session_id,
                input_total, input_cache_read, output_total, total_tokens, duration_ms, lifecycle
         FROM usage_events {where_clause}
         ORDER BY occurred_at_ms DESC LIMIT ?{} OFFSET ?{}",
        values.len() + 1,
        values.len() + 2
    );
    values.push(request.limit.into());
    values.push(request.offset.into());
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        Ok(EventDetailRow {
            event_id: r.get(0)?,
            agent: r.get(1)?,
            model_raw: r.get(2)?,
            call_category: r.get(3)?,
            occurred_at_ms: r.get(4)?,
            session_id: r.get(5)?,
            input_total: r.get(6)?,
            cache_read: r.get(7)?,
            output_total: r.get(8)?,
            total_tokens: r.get(9)?,
            duration_ms: r.get(10)?,
            lifecycle: r.get(11)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(EventDetailPage {
        rows: out,
        total_count: count,
        offset: request.offset,
        limit: request.limit,
    })
}

/// 小时粒度：把 hourly_usage 展开为 DailyRow 形状（label = "HH:00"），
/// 供 query_summary 的通用分组/合并逻辑复用。
fn load_hourly_as_daily(
    storage: &Storage,
    tz: &str,
    first_day: Date,
    last_day: Date,
) -> Result<Vec<DailyRow>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT local_day, hour, instance_id, agent, provider_id, model_raw,
                call_category, quality_bucket,
                event_count, call_count, 0 as attempt_count, 0 as observation_count,
                input_known_sum, 1 as input_known_count, 0 as input_unknown_count,
                NULL as uncached_known_sum, 0 as uncached_known_count,
                cache_read_known_sum, 0 as cache_read_known_count,
                cache_write_known_sum, 0 as cache_write_known_count,
                output_known_sum, 0 as output_known_count, 0 as output_unknown_count,
                total_known_sum, 0 as total_known_count, 0 as total_unknown_count,
                NULL as ratio_input_sum, NULL as ratio_cache_read_sum, 0 as ratio_sample_count,
                0 as conflict_count
         FROM hourly_usage
         WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
         ORDER BY local_day, hour",
    )?;
    let rows = stmt.query_map(
        params![tz, first_day.to_string(), last_day.to_string()],
        |r| {
            Ok(DailyRow {
                local_day: parse_date(&r.get::<_, String>(0)?)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                instance_id: r.get(2)?,
                hour: Some(r.get(1)?),
                agent: r.get(3)?,
                provider_id: r.get(4)?,
                model_raw: r.get(5)?,
                quality_bucket: r.get(7)?,
                sealed: false,
                event_count: r.get(8)?,
                call_count: r.get(9)?,
                attempt_count: r.get(10)?,
                observation_count: r.get(11)?,
                input_known_sum: r.get(12)?,
                input_known_count: r.get(13)?,
                input_unknown_count: r.get(14)?,
                uncached_known_sum: r.get(15)?,
                cache_read_known_sum: r.get(17)?,
                cache_write_known_sum: r.get(19)?,
                output_known_sum: r.get(21)?,
                output_known_count: r.get(22)?,
                output_unknown_count: r.get(23)?,
                total_known_sum: r.get(24)?,
                total_known_count: r.get(25)?,
                total_unknown_count: r.get(26)?,
                ratio_input_sum: r.get(27)?,
                ratio_cache_read_sum: r.get(28)?,
                ratio_sample_count: r.get(29)?,
                conflict_count: r.get(30)?,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 图表维度分组模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartDimension {
    Total,
    ByModel,
    ByAgent,
    ByAgentModel,
}

/// 按维度分组的时间序列行（趋势图表数据源；直接从聚合表读，低计算量）。
#[derive(Debug, Clone)]
pub struct ChartSeriesRow {
    pub label: String,
    pub start_day: Date,
    pub end_day: Date,
    pub series_name: String,
    pub call_count: i64,
    pub input_total: Option<i64>,
    pub cache_read: Option<i64>,
    pub output_total: Option<i64>,
    pub total_tokens: Option<i64>,
}

/// 按维度分组查询时间序列（从 daily_usage / hourly_usage / period_usage 直接读，
/// 不触 usage_events 明细——降低图表数据源计算量，2026-09-26 用户合同）。
pub fn chart_series(
    storage: &Storage,
    request: &SummaryRequest,
    dimension: &ChartDimension,
) -> Result<Vec<ChartSeriesRow>, CoreError> {
    let group_expr = match dimension {
        ChartDimension::Total => "'总量'".to_string(),
        ChartDimension::ByModel => "COALESCE(NULLIF(model_raw, ''), 'unknown')".to_string(),
        ChartDimension::ByAgent => "agent".to_string(),
        ChartDimension::ByAgentModel => {
            "agent || '/' || COALESCE(NULLIF(model_raw, ''), 'unknown')".to_string()
        }
    };
    let sql = format!(
        "SELECT local_day, {ge} as series_name,
                SUM(call_count), SUM(input_known_sum), SUM(cache_read_known_sum),
                SUM(output_known_sum), SUM(total_known_sum)
         FROM daily_usage
         WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
         GROUP BY local_day, {ge}
         ORDER BY local_day",
        ge = group_expr
    );
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(
        params![
            request.timezone,
            request.first_day.to_string(),
            request.last_day.to_string()
        ],
        |r| {
            Ok(ChartSeriesRow {
                label: r.get(0)?,
                start_day: parse_date(&r.get::<_, String>(0)?)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                end_day: parse_date(&r.get::<_, String>(0)?)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                series_name: r.get(1)?,
                call_count: r.get(2)?,
                input_total: r.get(3)?,
                cache_read: r.get(4)?,
                output_total: r.get(5)?,
                total_tokens: r.get(6)?,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 诊断日志行（设置页日志查看器）。
#[derive(Debug, Clone)]
pub struct DiagnosticLogRow {
    pub created_ms: i64,
    pub code: String,
    pub field: Option<String>,
    pub instance_id: Option<String>,
    pub message: String,
}

/// 查询最近诊断日志（按时间倒序，限量；白名单字段无正文）。
pub fn diagnostic_logs(storage: &Storage, limit: i64) -> Result<Vec<DiagnosticLogRow>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT created_ms, code, field, instance_id, message
         FROM diagnostics ORDER BY created_ms DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(DiagnosticLogRow {
            created_ms: r.get(0)?,
            code: r.get(1)?,
            field: r.get(2)?,
            instance_id: r.get(3)?,
            message: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
