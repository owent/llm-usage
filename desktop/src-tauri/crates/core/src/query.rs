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
    let rows = load_daily_rows(
        storage,
        &request.timezone,
        request.first_day,
        request.last_day,
    )?;
    let rows: Vec<DailyRow> = rows
        .into_iter()
        .filter(|r| request.filters.matches(r))
        .collect();

    // 周期分组键。
    let key_of = |day: Date| -> (String, Date, Date) {
        match request.granularity {
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
        let (label, start, end) = key_of(row.local_day);
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
        });
    }

    // 模型分组：unknown（空串）保留独立行。
    let mut model_groups: BTreeMap<(String, String), MetricSums> = BTreeMap::new();
    for row in &rows {
        model_groups
            .entry((row.provider_id.clone(), row.model_raw.clone()))
            .or_default()
            .add_row(row)?;
    }
    let model_breakdown = model_groups
        .into_iter()
        .map(|((provider, model), sums)| ModelRow {
            provider_id: if provider.is_empty() {
                None
            } else {
                Some(provider)
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
        "SELECT local_day, agent, provider_id, model_raw, quality_bucket, sealed,
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
                agent: r.get(1)?,
                provider_id: r.get(2)?,
                model_raw: r.get(3)?,
                quality_bucket: r.get(4)?,
                sealed: r.get::<_, i64>(5)? != 0,
                event_count: r.get(6)?,
                call_count: r.get(7)?,
                attempt_count: r.get(8)?,
                observation_count: r.get(9)?,
                input_known_sum: r.get(10)?,
                input_known_count: r.get(11)?,
                input_unknown_count: r.get(12)?,
                uncached_known_sum: r.get(13)?,
                cache_read_known_sum: r.get(14)?,
                cache_write_known_sum: r.get(15)?,
                output_known_sum: r.get(16)?,
                output_known_count: r.get(17)?,
                output_unknown_count: r.get(18)?,
                total_known_sum: r.get(19)?,
                total_known_count: r.get(20)?,
                total_unknown_count: r.get(21)?,
                ratio_input_sum: r.get(22)?,
                ratio_cache_read_sum: r.get(23)?,
                ratio_sample_count: r.get(24)?,
                conflict_count: r.get(25)?,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
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
    append_filters(&mut sql, &mut values, filters);
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
    append_filters(&mut sql, &mut values, &request.filters);
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

fn append_filters(sql: &mut String, values: &mut Vec<rusqlite::types::Value>, filters: &Filters) {
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
