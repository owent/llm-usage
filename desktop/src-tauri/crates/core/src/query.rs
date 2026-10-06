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
use rusqlite::{params, OptionalExtension};
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
    /// None = no restriction; Some([]) = no owned sources, therefore no rows.
    pub instances: Option<Vec<String>>,
}

impl Filters {
    fn matches(&self, row: &DailyRow) -> bool {
        let any_match = |filter: &[String], value: &str| {
            filter.is_empty()
                || filter.iter().any(|f| {
                    f.to_lowercase() == value.to_lowercase()
                        || (f.eq_ignore_ascii_case("unknown") && value.is_empty())
                })
        };
        any_match(&self.agents, &row.agent)
            && any_match(&self.providers, &row.provider_id)
            && any_match(
                &self
                    .models
                    .iter()
                    .map(|m| crate::model_names::model_key(m))
                    .collect::<Vec<_>>(),
                &row.model_raw,
            )
            && self
                .instances
                .as_ref()
                .map_or(true, |ids| ids.contains(&row.instance_id))
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
    pub duration_sample_count: i64,
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
    pub agent_breakdown: Vec<AgentRow>,
    pub distinct_sessions: Option<i64>,
    pub active_days: Option<i64>,
    /// 范围内归属未核验/被排除的事件数（未计入 totals）。
    pub excluded_event_count: i64,
}

#[derive(Debug, Clone)]
struct DailyRow {
    model_original: String,
    local_day: Date,
    instance_id: String,
    /// 小时粒度时的-hour 值（日粒度行恒 None；由 hourly_usage 加载）。
    hour: Option<i64>,
    agent: String,
    provider_id: String,
    model_raw: String,
    call_category: String,
    quality_bucket: String,
    sealed: bool,
    from_period: bool,
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

/// 周期分组键（标签 + 起止日）：query_summary 与 chart_series 共用，
/// 保证总用量视图与维度分组视图的时间轴标签一致。
/// 小时粒度标签 "YYYY-MM-DD HH:00"；周标签按 week_start（ISO 周或起始日）；
/// 月标签 "YYYY-MM"。
fn period_key_of(
    calendar: &Calendar,
    granularity: Granularity,
    week_start: WeekStart,
    day: Date,
    hour: Option<i64>,
) -> (String, Date, Date) {
    match granularity {
        Granularity::Hour => {
            let label = match hour {
                Some(h) => format!("{} {:02}:00", day, h),
                None => day.to_string(),
            };
            (label, day, day)
        }
        Granularity::Day => (day.to_string(), day, day),
        Granularity::Week => {
            let start = calendar.week_start_of(day, week_start);
            let end = start.checked_add(Span::new().days(6)).expect("week end");
            (calendar.week_label(day, week_start), start, end)
        }
        Granularity::Month => {
            let start = calendar.month_start_of(day);
            (calendar.month_label(day), start, start.last_of_month())
        }
    }
}

/// 执行汇总查询。
pub fn query_summary(storage: &Storage, request: &SummaryRequest) -> Result<Summary, CoreError> {
    query_summary_selected(storage, request, None)
}

/// Select visible period labels, including local hours (both DST folds share a label).
/// Date/user filters remain mandatory; no summing per-period session counts.
pub fn query_summary_selected(
    storage: &Storage,
    request: &SummaryRequest,
    selection: Option<(&str, &str)>,
) -> Result<Summary, CoreError> {
    if selection.is_some_and(|(first, last)| first.is_empty() || first > last || last.len() > 40) {
        return Err(CoreError::Query("invalid period selection".into()));
    }
    if request.last_day < request.first_day {
        return Err(CoreError::Query("last_day before first_day".into()));
    }
    let calendar = Calendar::new(&request.timezone)?;
    let snapshot = storage.conn().unchecked_transaction()?;
    let revision = storage.data_revision()?;
    let stamp = crate::summary_cache::Stamp {
        revision,
        data_version: storage
            .conn()
            .pragma_query_value(None, "data_version", |row| row.get(0))?,
        total_changes: storage.conn().total_changes(),
    };
    let cache_key = format!("{request:?}; selection={selection:?}");
    let cached = storage.summary_cache.borrow_mut().get(stamp, &cache_key);
    if let Some(cached) = cached {
        snapshot.commit()?;
        return Ok(cached);
    }
    let mut groups: BTreeMap<(Date, String), PeriodSums> = BTreeMap::new();
    let mut models: BTreeMap<(String, String), MetricSums> = BTreeMap::new();
    let mut agents: BTreeMap<String, MetricSums> = BTreeMap::new();
    let mut totals = MetricSums::default();
    let mut sealed = false;
    let mut from_period = false;
    let mut active = BTreeSet::new();
    let accelerated = crate::query_acceleration::usable(storage.conn(), request)?;
    let mut consume = |axis: &str, row: DailyRow| -> Result<(), CoreError> {
        let (label, start, end) = period_key_of(
            &calendar,
            request.granularity,
            request.week_start,
            row.local_day,
            row.hour,
        );
        if selection.is_some_and(|(first, last)| label.as_str() < first || label.as_str() > last) {
            return Ok(());
        }
        if axis == "model" {
            models
                .entry((
                    row.provider_id.trim().to_lowercase(),
                    crate::model_names::model_key(&row.model_raw),
                ))
                .or_default()
                .add_row(&row)?;
            return Ok(());
        }
        if axis == "agent" {
            agents
                .entry(row.agent.to_lowercase())
                .or_default()
                .add_row(&row)?;
            return Ok(());
        }
        let group = groups
            .entry((start, label))
            .or_insert_with(|| PeriodSums::new(end));
        group.sums.add_row(&row)?;
        group.sealed |= row.sealed;
        group.from_period |= row.from_period;
        sealed |= row.sealed;
        from_period |= row.from_period;
        if row.event_count > 0 {
            group.active.insert(row.local_day);
            active.insert(row.local_day);
        }
        totals.add_row(&row)?;
        if axis == "all" {
            models
                .entry((
                    row.provider_id.trim().to_lowercase(),
                    crate::model_names::model_key(&row.model_raw),
                ))
                .or_default()
                .add_row(&row)?;
            agents
                .entry(row.agent.to_lowercase())
                .or_default()
                .add_row(&row)?;
        }
        Ok(())
    };
    if accelerated {
        for (axis, table) in [
            (
                "day",
                if request.filters.providers.is_empty() {
                    "query_rollup_day"
                } else {
                    "query_rollup_provider_day"
                },
            ),
            ("model", "query_rollup_model"),
            (
                "agent",
                if request.filters.providers.is_empty() {
                    "query_rollup_agent"
                } else {
                    "query_rollup_provider_agent"
                },
            ),
        ] {
            visit_daily_rows_from(
                storage,
                table,
                &request.timezone,
                request.first_day,
                request.last_day,
                &request.filters,
                |row| consume(axis, row),
            )?;
        }
    } else {
        visit_summary_rows(storage, request, |row| consume("all", row))?;
    }
    let (details, by_period) = detail_stats(storage, &calendar, request, selection)?;
    let mut periods = Vec::new();
    for ((start, label), group) in groups {
        let end = group.end;
        let mut sums = group.sums;
        let stats = by_period.get(&label);
        if !group.sealed {
            set_duration(&mut sums, stats);
        }
        let (utc_start_ms, utc_end_ms) = period_range_ms(&calendar, start, end)?;
        periods.push(PeriodRow {
            label,
            start_day: start,
            end_day: end,
            utc_start_ms,
            utc_end_ms,
            in_progress: request.today >= start && request.today <= end,
            partial_history: group.sealed
                || carrier_coverage_gap(storage, utc_start_ms, utc_end_ms, &request.filters)?
                || request
                    .retention_cutoff
                    .is_some_and(|cutoff| start < cutoff),
            distinct_sessions: (!group.sealed && !stats.is_some_and(|s| s.unknown_session))
                .then(|| stats.map_or(0, DetailStats::session_count)),
            active_days: (!group.from_period).then_some(group.active.len() as i64),
            avg_duration_ms: sums.avg_duration_ms,
            total_duration_ms: sums.total_duration_ms,
            sums,
        });
    }
    if !sealed {
        set_duration(&mut totals, Some(&details));
    }
    let model_breakdown = models
        .into_iter()
        .map(|((provider, model), sums)| ModelRow {
            provider_id: (!provider.is_empty()).then_some(provider),
            model_raw: (!model.is_empty()).then_some(model),
            sums,
        })
        .collect();
    let agent_breakdown = agents
        .into_iter()
        .map(|(agent, sums)| AgentRow { agent, sums })
        .collect();
    let excluded_event_count = count_excluded(storage, &calendar, request, selection)?;
    snapshot.commit()?;
    let summary = Summary {
        data_revision: revision,
        timezone: request.timezone.clone(),
        week_start: request.week_start,
        periods,
        totals,
        model_breakdown,
        agent_breakdown,
        excluded_event_count,
        distinct_sessions: (!sealed && !details.unknown_session).then_some(details.session_count()),
        active_days: (!from_period).then_some(active.len() as i64),
    };
    storage.summary_cache.borrow_mut().put(cache_key, &summary);
    Ok(summary)
}

#[derive(Default)]
struct DetailStats {
    sessions: BTreeSet<(String, String)>,
    counted_sessions: Option<i64>,
    unknown_session: bool,
    duration_sum: i64,
    duration_count: i64,
}

impl DetailStats {
    fn session_count(&self) -> i64 {
        self.counted_sessions.unwrap_or(self.sessions.len() as i64)
    }
}

struct PeriodSums {
    end: Date,
    sums: MetricSums,
    sealed: bool,
    from_period: bool,
    active: BTreeSet<Date>,
}
impl PeriodSums {
    fn new(end: Date) -> Self {
        Self {
            end,
            sums: MetricSums::default(),
            sealed: false,
            from_period: false,
            active: BTreeSet::new(),
        }
    }
}

fn set_duration(sums: &mut MetricSums, details: Option<&DetailStats>) {
    if let Some(d) = details.filter(|d| d.duration_count > 0) {
        sums.total_duration_ms = Some(d.duration_sum);
        sums.avg_duration_ms = Some(d.duration_sum / d.duration_count);
        sums.duration_sample_count = d.duration_count;
    }
}

/// Read retained details once, rather than once for every period/dimension.
fn detail_stats(
    storage: &Storage,
    calendar: &Calendar,
    request: &SummaryRequest,
    selection: Option<(&str, &str)>,
) -> Result<(DetailStats, BTreeMap<String, DetailStats>), CoreError> {
    if request.granularity != Granularity::Hour {
        return detail_stats_days(storage, calendar, request, selection);
    }
    let (start, end) = period_range_ms(calendar, request.first_day, request.last_day)?;
    let mut sql = String::from(
        "SELECT source_instance_id, session_id, occurred_at_ms, duration_ms, record_kind
         FROM usage_events WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
         AND attribution_status = 'verified'
         AND record_kind IN ('model_call', 'transport_attempt', 'usage_observation')",
    );
    let mut values = vec![start.into(), end.into()];
    append_filters(
        &mut sql,
        &mut values,
        &request.filters,
        "source_instance_id",
    );
    let mut stmt = storage.conn().prepare(&sql)?;
    let mut records = stmt.query(rusqlite::params_from_iter(values))?;
    let mut total = DetailStats::default();
    let mut groups: BTreeMap<String, DetailStats> = BTreeMap::new();
    while let Some(r) = records.next()? {
        let instance: String = r.get(0)?;
        let session: Option<String> = r.get(1)?;
        let ms: i64 = r.get(2)?;
        let duration: Option<i64> = r.get(3)?;
        let kind: String = r.get(4)?;
        let day = calendar.local_day_of(ms)?;
        let (label, _, _) = period_key_of(
            calendar,
            request.granularity,
            request.week_start,
            day,
            Some(calendar.local_hour_of(ms)? as i64),
        );
        if selection.is_some_and(|(first, last)| label.as_str() < first || label.as_str() > last) {
            continue;
        }
        let group = groups.entry(label).or_default();
        for stats in [&mut total, group] {
            if let Some(id) = session.as_ref().filter(|id| !id.is_empty()) {
                stats.sessions.insert((instance.clone(), id.clone()));
            } else {
                stats.unknown_session = true;
            }
            if let Some(duration) = duration.filter(|d| *d >= 0 && kind == "model_call") {
                stats.duration_sum = stats
                    .duration_sum
                    .checked_add(duration)
                    .ok_or(CoreError::Overflow("duration_ms"))?;
                stats.duration_count += 1;
            }
        }
    }
    Ok((total, groups))
}

/// Aggregate retained details in SQLite before transferring them to Rust. Exact
/// calendar boundaries preserve DST and historical offsets; session sets still
/// span days and are never replaced with sums of per-day DISTINCT counts.
fn detail_stats_days(
    storage: &Storage,
    calendar: &Calendar,
    request: &SummaryRequest,
    selection: Option<(&str, &str)>,
) -> Result<(DetailStats, BTreeMap<String, DetailStats>), CoreError> {
    if crate::query_acceleration::usable(storage.conn(), request)? {
        return detail_stats_projection(storage, calendar, request, selection);
    }
    let mut sql = String::from(
        "SELECT source_instance_id, session_id,
        SUM(CASE WHEN record_kind='model_call' AND duration_ms>=0 THEN duration_ms ELSE 0 END),
        COUNT(CASE WHEN record_kind='model_call' AND duration_ms>=0 THEN 1 END)
        FROM usage_events WHERE occurred_at_ms>=?1 AND occurred_at_ms<?2
        AND attribution_status='verified'
        AND record_kind IN ('model_call','transport_attempt','usage_observation')",
    );
    let mut values = vec![0.into(), 0.into()];
    append_filters(
        &mut sql,
        &mut values,
        &request.filters,
        "source_instance_id",
    );
    sql.push_str(" GROUP BY source_instance_id, session_id");
    let mut stmt = storage.conn().prepare(&sql)?;
    let mut total = DetailStats::default();
    let mut groups: BTreeMap<String, DetailStats> = BTreeMap::new();
    let mut day = request.first_day;
    loop {
        let (label, _, _) =
            period_key_of(calendar, request.granularity, request.week_start, day, None);
        if selection.map_or(true, |(first, last)| {
            label.as_str() >= first && label.as_str() <= last
        }) {
            let (start, end) = calendar.day_range_ms(day)?;
            values[0] = start.into();
            values[1] = end.into();
            let mut records = stmt.query(rusqlite::params_from_iter(values.iter()))?;
            while let Some(row) = records.next()? {
                let instance: String = row.get(0)?;
                let session: Option<String> = row.get(1)?;
                let sum: i64 = row.get(2)?;
                let count: i64 = row.get(3)?;
                let group = groups.entry(label.clone()).or_default();
                for stats in [&mut total, group] {
                    if let Some(id) = session.as_ref().filter(|id| !id.is_empty()) {
                        stats.sessions.insert((instance.clone(), id.clone()));
                    } else {
                        stats.unknown_session = true;
                    }
                    stats.duration_sum = stats
                        .duration_sum
                        .checked_add(sum)
                        .ok_or(CoreError::Overflow("duration_ms"))?;
                    stats.duration_count = stats
                        .duration_count
                        .checked_add(count)
                        .ok_or(CoreError::Overflow("duration_count"))?;
                }
            }
        }
        if day >= request.last_day {
            break;
        }
        day = day.checked_add(jiff::Span::new().days(1))?;
    }
    Ok((total, groups))
}

fn detail_stats_projection(
    storage: &Storage,
    calendar: &Calendar,
    request: &SummaryRequest,
    selection: Option<(&str, &str)>,
) -> Result<(DetailStats, BTreeMap<String, DetailStats>), CoreError> {
    let table = if request.filters.providers.is_empty() {
        "query_session_days"
    } else {
        "query_provider_session_days"
    };
    let mut from = format!(" FROM {table} WHERE tz_version=?1 AND local_day>=?2 AND local_day<=?3");
    let mut values = vec![
        request.timezone.clone().into(),
        request.first_day.to_string().into(),
        request.last_day.to_string().into(),
    ];
    if let Some((first, last)) = selection {
        from.push_str(" AND local_day>=?4 AND local_day<=?5");
        values.push(first.to_string().into());
        values.push(last.to_string().into());
    }
    append_filters(&mut from, &mut values, &request.filters, "instance_id");
    let one_provider = request.filters.providers.len() == 1
        && !request.filters.providers[0].eq_ignore_ascii_case("unknown");
    let sql = if request.filters.providers.is_empty() || one_provider {
        format!("SELECT local_day,COUNT(CASE WHEN session_id<>'' THEN 1 END),MAX(session_id=''),SUM(duration_sum),SUM(duration_count){from} GROUP BY local_day")
    } else {
        format!("SELECT local_day,COUNT(CASE WHEN session_id<>'' THEN 1 END),MAX(session_id=''),SUM(duration_sum),SUM(duration_count) FROM
            (SELECT local_day,instance_id,session_id,SUM(duration_sum) duration_sum,SUM(duration_count) duration_count{from} GROUP BY local_day,instance_id,session_id) GROUP BY local_day")
    };
    let mut stmt = storage.conn().prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(values.iter()))?;
    let session_count: i64 = storage.conn().query_row(&format!("SELECT COUNT(*) FROM (SELECT instance_id,session_id{from} AND session_id<>'' GROUP BY instance_id,session_id)"),rusqlite::params_from_iter(values.iter()),|r| r.get(0))?;
    let mut total = DetailStats {
        counted_sessions: Some(session_count),
        ..Default::default()
    };
    let mut groups: BTreeMap<String, DetailStats> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let day = parse_date(&row.get::<_, String>(0)?)?;
        let (label, _, _) =
            period_key_of(calendar, request.granularity, request.week_start, day, None);
        if selection.is_some_and(|(first, last)| label.as_str() < first || label.as_str() > last) {
            continue;
        }
        let count_sessions: i64 = row.get(1)?;
        let unknown: bool = row.get(2)?;
        let sum: i64 = row.get(3)?;
        let count: i64 = row.get(4)?;
        let group = groups.entry(label).or_default();
        group.counted_sessions = Some(count_sessions);
        for stats in [&mut total, group] {
            stats.unknown_session |= unknown;
            stats.duration_sum = stats
                .duration_sum
                .checked_add(sum)
                .ok_or(CoreError::Overflow("duration_ms"))?;
            stats.duration_count = stats
                .duration_count
                .checked_add(count)
                .ok_or(CoreError::Overflow("duration_count"))?;
        }
    }
    Ok((total, groups))
}

/// One coverage selection for totals, models, agents and chart series.
/// A complete materialized partition replaces its matching daily dimensions;
/// it never replaces another source, nor gets assigned to a clipped interval.
fn load_summary_rows(
    storage: &Storage,
    request: &SummaryRequest,
) -> Result<Vec<DailyRow>, CoreError> {
    if request.granularity == Granularity::Hour {
        return Ok(load_hourly_as_daily(
            storage,
            &request.timezone,
            request.first_day,
            request.last_day,
            &request.filters,
        )?
        .into_iter()
        .filter(|r| request.filters.matches(r))
        .collect());
    }
    let mut rows = load_daily_rows(
        storage,
        &request.timezone,
        request.first_day,
        request.last_day,
        &request.filters,
    )?;
    let granularity = match request.granularity {
        Granularity::Week if request.week_start == WeekStart::Monday => Some("week"),
        Granularity::Month => Some("month"),
        _ => None,
    };
    if let Some(granularity) = granularity {
        let mut stmt = storage.conn().prepare(
            "SELECT p.period_start_day, p.period_end_day, p.instance_id, p.agent, p.provider_id,
                    p.model_raw, p.call_category, p.quality_bucket, p.event_count, p.call_count,
                    p.input_known_sum, p.cache_read_known_sum, p.cache_write_known_sum,
                    p.output_known_sum, p.total_known_sum, p.conflict_count
             FROM period_usage p WHERE p.tz_version = ?1 AND p.granularity = ?2
               AND p.period_start_day >= ?3 AND p.period_end_day <= ?4
               AND (p.period_start_day < COALESCE((SELECT value FROM settings
                   WHERE key = 'daily_retention_floor:' || p.tz_version), '')
                 OR NOT EXISTS (
                   SELECT 1 FROM daily_usage d WHERE d.tz_version = p.tz_version
                     AND d.instance_id = p.instance_id AND d.agent = p.agent
                     AND d.provider_id = p.provider_id AND d.model_raw = p.model_raw
                     AND d.call_category = p.call_category AND d.quality_bucket = p.quality_bucket
                     AND d.local_day BETWEEN p.period_start_day AND p.period_end_day))
               AND NOT EXISTS (
                 SELECT 1 FROM daily_usage d WHERE d.tz_version = p.tz_version
                   AND d.instance_id = p.instance_id AND d.agent = p.agent
                   AND d.provider_id = p.provider_id AND d.model_raw = p.model_raw
                   AND d.call_category = p.call_category AND d.quality_bucket = p.quality_bucket
                   AND d.local_day BETWEEN p.period_start_day AND p.period_end_day
                   AND d.data_revision > p.data_revision)",
        )?;
        let mut records = stmt.query(params![
            request.timezone,
            granularity,
            request.first_day.to_string(),
            request.last_day.to_string()
        ])?;
        let mut archived = Vec::new();
        while let Some(r) = records.next()? {
            let start = parse_date(&r.get::<_, String>(0)?)?;
            let row = DailyRow {
                model_original: r.get(5)?,
                local_day: start,
                hour: None,
                instance_id: r.get(2)?,
                agent: r.get(3)?,
                provider_id: r.get(4)?,
                model_raw: crate::model_names::model_key(&r.get::<_, String>(5)?),
                call_category: r.get(6)?,
                quality_bucket: r.get(7)?,
                sealed: true,
                from_period: true,
                event_count: r.get(8)?,
                call_count: r.get(9)?,
                attempt_count: 0,
                observation_count: 0,
                input_known_sum: r.get(10)?,
                input_known_count: 0,
                input_unknown_count: 0,
                uncached_known_sum: None,
                cache_read_known_sum: r.get(11)?,
                cache_write_known_sum: r.get(12)?,
                output_known_sum: r.get(13)?,
                output_known_count: 0,
                output_unknown_count: 0,
                total_known_sum: r.get(14)?,
                total_known_count: 0,
                total_unknown_count: 0,
                ratio_input_sum: None,
                ratio_cache_read_sum: None,
                ratio_sample_count: 0,
                conflict_count: r.get(15)?,
            };
            archived.push(row);
        }
        let calendar = Calendar::new(&request.timezone)?;
        let dimension_key = |r: &DailyRow, day| {
            (
                day,
                r.instance_id.clone(),
                r.agent.clone(),
                r.provider_id.clone(),
                r.model_original.clone(),
                r.call_category.clone(),
                r.quality_bucket.clone(),
            )
        };
        let replaced: BTreeSet<_> = archived
            .iter()
            .map(|r| dimension_key(r, r.local_day))
            .collect();
        rows.retain(|r| {
            let start = if granularity == "week" {
                calendar.week_start_of(r.local_day, WeekStart::Monday)
            } else {
                calendar.month_start_of(r.local_day)
            };
            !replaced.contains(&dimension_key(r, start))
        });
        rows.extend(archived);
    }
    rows.retain(|r| request.filters.matches(r));
    Ok(rows)
}

/// Current-price simulation reuses exactly the usage query's archive selection.
pub(crate) struct ArchivedPricingRow {
    pub event: crate::pricing::PricingEvent,
    pub day: String,
    pub hour: Option<i64>,
    pub count: i64,
    pub partial: bool,
    pub from_period: bool,
    pub instance: String,
    pub agent: String,
    pub category: String,
    pub quality: String,
    pub end_ms: i64,
    pub known_tokens_floor: i64,
    pub alias_stable: bool,
}

pub(crate) fn archived_pricing_rows(
    storage: &Storage,
    request: &SummaryRequest,
) -> Result<Vec<ArchivedPricingRow>, CoreError> {
    let calendar = Calendar::new(&request.timezone)?;
    let mut out = Vec::new();
    visit_summary_rows(storage, request, |row| {
        if !row.sealed || row.event_count <= row.attempt_count {
            return Ok(());
        }
        let start = calendar.day_range_ms(row.local_day)?.0;
        let end_day = if row.from_period {
            period_key_of(
                &calendar,
                request.granularity,
                request.week_start,
                row.local_day,
                row.hour,
            )
            .2
        } else {
            row.local_day
        };
        // Aggregate fields can cover different samples. Preserve the saved split;
        // subtracting independently known sums would invent uncached usage.
        let known_tokens_floor = row
            .input_known_sum
            .unwrap_or(0)
            .checked_add(row.output_known_sum.unwrap_or(0))
            .ok_or(CoreError::Overflow("archive known tokens"))?
            .max(row.total_known_sum.unwrap_or(0));
        let end_ms = calendar.day_range_ms(end_day)?.1;
        let alias = |at| {
            crate::model_names::reference_model_key_for(
                &row.model_original,
                Some(&row.provider_id),
                at,
            )
        };
        let alias_stable = alias(start) == alias(end_ms - 1);
        out.push(ArchivedPricingRow {
            event: crate::pricing::PricingEvent {
                provider_id: Some(row.provider_id),
                model_raw: Some(row.model_original),
                occurred_at_ms: start,
                input_uncached: row.uncached_known_sum,
                // Aggregate input is not a request size, nor necessarily the same sample set.
                input_total: None,
                input_cache_read: row.cache_read_known_sum,
                input_cache_write: row.cache_write_known_sum,
                output_total: row.output_known_sum,
                ..Default::default()
            },
            day: row.local_day.to_string(),
            hour: row.hour,
            count: row.event_count - row.attempt_count,
            partial: row.quality_bucket != "complete",
            from_period: row.from_period,
            instance: row.instance_id,
            agent: row.agent,
            category: row.call_category,
            quality: row.quality_bucket,
            end_ms,
            known_tokens_floor,
            alias_stable,
        });
        Ok(())
    })?;
    Ok(out)
}

fn visit_summary_rows(
    storage: &Storage,
    request: &SummaryRequest,
    mut consume: impl FnMut(DailyRow) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    if request.granularity == Granularity::Day {
        visit_daily_rows(
            storage,
            &request.timezone,
            request.first_day,
            request.last_day,
            &request.filters,
            consume,
        )
    } else {
        for row in load_summary_rows(storage, request)? {
            consume(row)?;
        }
        Ok(())
    }
}

fn load_daily_rows(
    storage: &Storage,
    tz: &str,
    first_day: Date,
    last_day: Date,
    filters: &Filters,
) -> Result<Vec<DailyRow>, CoreError> {
    let mut rows = Vec::new();
    visit_daily_rows(storage, tz, first_day, last_day, filters, |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok(rows)
}

fn visit_daily_rows(
    storage: &Storage,
    tz: &str,
    first_day: Date,
    last_day: Date,
    filters: &Filters,
    consume: impl FnMut(DailyRow) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    visit_daily_rows_from(
        storage,
        "daily_usage",
        tz,
        first_day,
        last_day,
        filters,
        consume,
    )
}

fn visit_daily_rows_from(
    storage: &Storage,
    table: &str,
    tz: &str,
    first_day: Date,
    last_day: Date,
    filters: &Filters,
    mut consume: impl FnMut(DailyRow) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    let mut sql = format!(
        "SELECT local_day, instance_id, agent, provider_id, model_raw, quality_bucket, sealed,
                event_count, call_count, attempt_count, observation_count,
                input_known_sum, input_known_count, input_unknown_count,
                uncached_known_sum,
                cache_read_known_sum,
                cache_write_known_sum,
                output_known_sum, output_known_count, output_unknown_count,
                total_known_sum, total_known_count, total_unknown_count,
                ratio_input_sum, ratio_cache_read_sum, ratio_sample_count, conflict_count, call_category
         FROM {table}
         WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3",
    );
    let mut values = vec![
        tz.to_string().into(),
        first_day.to_string().into(),
        last_day.to_string().into(),
    ];
    append_filters(&mut sql, &mut values, filters, "instance_id");
    sql.push_str(" ORDER BY local_day");
    let mut stmt = storage.conn().prepare(&sql)?;
    let mut days = BTreeMap::new();
    let mut names = BTreeMap::new();
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        let day: String = r.get(0)?;
        let local_day = if let Some(value) = days.get(&day) {
            *value
        } else {
            let value = parse_date(&day)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            days.insert(day, value);
            value
        };
        let raw: String = r.get(4)?;
        let model_raw = names
            .entry(raw.clone())
            .or_insert_with(|| crate::model_names::model_key(&raw))
            .clone();
        Ok(DailyRow {
            model_original: raw,
            local_day,
            instance_id: r.get(1)?,
            hour: None,
            agent: r.get(2)?,
            provider_id: r.get(3)?,
            model_raw,
            call_category: r.get(27)?,
            quality_bucket: r.get(5)?,
            sealed: r.get::<_, i64>(6)? != 0,
            from_period: false,
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
    })?;
    for row in rows {
        consume(row?)?;
    }
    Ok(())
}

fn period_range_ms(calendar: &Calendar, start: Date, end: Date) -> Result<(i64, i64), CoreError> {
    let (s, _) = calendar.day_range_ms(start)?;
    let (_, e) = calendar.day_range_ms(end)?;
    Ok((s, e))
}

/// 归属未核验/被排除的事件数：不进入总计，可按排除原因列出。
fn count_excluded(
    storage: &Storage,
    calendar: &Calendar,
    request: &SummaryRequest,
    selection: Option<(&str, &str)>,
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
    if let Some((first, last)) = selection {
        let sql = sql.replace("SELECT COUNT(*)", "SELECT occurred_at_ms");
        let mut stmt = storage.conn().prepare(&sql)?;
        let mut records = stmt.query(rusqlite::params_from_iter(values))?;
        let mut count = 0;
        while let Some(record) = records.next()? {
            let ms = record.get(0)?;
            let (label, _, _) = period_key_of(
                calendar,
                request.granularity,
                request.week_start,
                calendar.local_day_of(ms)?,
                Some(calendar.local_hour_of(ms)? as i64),
            );
            if label.as_str() >= first && label.as_str() <= last {
                count += 1;
            }
        }
        return Ok(count);
    }
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

fn carrier_coverage_gap(
    storage: &Storage,
    start: i64,
    end: i64,
    filters: &Filters,
) -> Result<bool, CoreError> {
    let mut sql = String::from("SELECT EXISTS(SELECT 1 FROM usage_events WHERE
        exclusion_reason IN ('copilot_otel_session_authority','qwen_sdk_session_authority','qwen_native_partition_sealed','qwen_sdk_partition_sealed') AND occurred_at_ms>=?1 AND occurred_at_ms<?2");
    let mut values = vec![start.into(), end.into()];
    append_filters(&mut sql, &mut values, filters, "source_instance_id");
    sql.push(')');
    Ok(storage
        .conn()
        .query_row(&sql, rusqlite::params_from_iter(values), |r| r.get(0))?)
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
            if item.eq_ignore_ascii_case("unknown") && column != "agent" {
                clause.push_str(&format!(
                    "({column} IS NULL OR {column} = '' OR fold_name({column}) = 'unknown')"
                ));
            } else {
                clause.push_str(&format!("fold_name({column}) = ?{}", values.len() + 1));
                values.push(rusqlite::types::Value::Text(item.to_lowercase()));
            }
        }
        clause.push(')');
        sql.push_str(&clause);
    };
    push_in(sql, values, "agent", &filters.agents);
    push_in(sql, values, "provider_id", &filters.providers);
    push_in(
        sql,
        values,
        "model_key(model_raw)",
        &filters
            .models
            .iter()
            .map(|m| crate::model_names::model_key(m))
            .collect::<Vec<_>>(),
    );
    if let Some(instances) = &filters.instances {
        if instances.is_empty() {
            sql.push_str(" AND 0");
        }
        let mut clause = format!(" AND {instance_column} IN (");
        for (i, item) in instances.iter().enumerate() {
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

/// Agent 分组行（与 ModelRow 同构；总计规则一致）。
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
    let mut groups: BTreeMap<String, MetricSums> = BTreeMap::new();
    visit_summary_rows(storage, request, |row| {
        groups
            .entry(row.agent.to_lowercase())
            .or_default()
            .add_row(&row)?;
        Ok(())
    })?;
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
    let request = SummaryRequest {
        timezone: timezone.into(),
        first_day: day,
        last_day: day,
        today: day,
        week_start: WeekStart::Monday,
        granularity: Granularity::Hour,
        filters: filters.clone(),
        retention_cutoff: None,
    };
    let summary = query_summary(storage, &request)?;
    Ok(summary
        .periods
        .into_iter()
        .map(|p| HourBucket {
            hour: p
                .label
                .rsplit_once(' ')
                .and_then(|(_, time)| time.split(':').next())
                .and_then(|h| h.parse().ok())
                .unwrap_or(0),
            event_count: p.sums.event_count,
            call_count: p.sums.call_count,
            input_total_known: p.sums.input_total_known,
            cache_read_known: p.sums.cache_read_known,
            output_total_known: p.sums.output_total_known,
            total_tokens_known: p.sums.total_tokens_known,
            session_count: p.distinct_sessions,
            avg_duration_ms: p.sums.avg_duration_ms,
        })
        .collect())
}

/// 热力图单元格：一个本地日。日层已清理且无法再分配到日期的周期汇总标为不可用。
#[derive(Debug, Clone, Default)]
pub struct HeatCell {
    pub day: String,
    pub weekday: u8,
    pub call_count: i64,
    pub total_tokens_known: Option<i64>,
    pub available: bool,
    pub partial: bool,
}

pub fn heatmap_cells(
    storage: &Storage,
    timezone: &str,
    first_day: Date,
    last_day: Date,
    filters: &Filters,
) -> Result<Vec<HeatCell>, CoreError> {
    if last_day < first_day {
        return Err(CoreError::Query("last_day before first_day".into()));
    }
    let calendar = Calendar::new(timezone)?;
    let snapshot = storage.conn().unchecked_transaction()?;
    let floor: Option<String> = storage
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [format!("daily_retention_floor:{timezone}")],
            |r| r.get(0),
        )
        .optional()?;
    // Only the heatmap's columns cross SQLite; push owner/model filters into
    // the bounded daily query instead of materializing every dimensional row.
    let mut sums: BTreeMap<Date, (i64, Option<i64>)> = BTreeMap::new();
    let mut day_sql = String::from(
        "SELECT local_day,SUM(call_count),
        CASE WHEN SUM(total_known_count)>0 THEN SUM(total_known_sum) END FROM daily_usage
        WHERE tz_version=?1 AND local_day>=?2 AND local_day<=?3",
    );
    let mut values: Vec<rusqlite::types::Value> = vec![
        timezone.to_string().into(),
        first_day.to_string().into(),
        last_day.to_string().into(),
    ];
    append_filters(&mut day_sql, &mut values, filters, "instance_id");
    day_sql.push_str(" GROUP BY local_day");
    let mut stmt = storage.conn().prepare(&day_sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<i64>>(2)?,
        ))
    })?;
    for row in rows {
        let (day, calls, total) = row?;
        sums.insert(parse_date(&day)?, (calls, total));
    }
    drop(stmt);
    // A period-only import may not carry the local daily_retention_floor setting.
    // Detect materialized partitions on their own; never assign their totals to a date.
    let mut period_sql = String::from(
        "SELECT p.period_start_day, p.period_end_day FROM period_usage p
         WHERE p.tz_version = ?1 AND p.period_start_day <= ?2 AND p.period_end_day >= ?3
           AND p.granularity IN ('week', 'month', 'year')
           AND (p.period_start_day < COALESCE((SELECT value FROM settings
                    WHERE key = 'daily_retention_floor:' || p.tz_version), '')
             OR NOT EXISTS (SELECT 1 FROM daily_usage d WHERE d.tz_version = p.tz_version
                    AND d.instance_id = p.instance_id AND d.agent = p.agent
                    AND d.provider_id = p.provider_id AND d.model_raw = p.model_raw
                    AND d.call_category = p.call_category AND d.quality_bucket = p.quality_bucket
                    AND d.local_day BETWEEN p.period_start_day AND p.period_end_day))
           AND NOT EXISTS (SELECT 1 FROM daily_usage d WHERE d.tz_version = p.tz_version
                    AND d.instance_id = p.instance_id AND d.agent = p.agent
                    AND d.provider_id = p.provider_id AND d.model_raw = p.model_raw
                    AND d.call_category = p.call_category AND d.quality_bucket = p.quality_bucket
                    AND d.local_day BETWEEN p.period_start_day AND p.period_end_day
                    AND d.data_revision > p.data_revision)",
    );
    let mut period_values: Vec<rusqlite::types::Value> = vec![
        timezone.to_string().into(),
        last_day.to_string().into(),
        first_day.to_string().into(),
    ];
    append_filters(&mut period_sql, &mut period_values, filters, "instance_id");
    let mut archived_days = BTreeSet::new();
    let mut stmt = storage.conn().prepare(&period_sql)?;
    let periods = stmt.query_map(rusqlite::params_from_iter(period_values), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for period in periods {
        let (start, end) = period?;
        let mut covered = parse_date(&start)?.max(first_day);
        let end = parse_date(&end)?.min(last_day);
        while covered <= end {
            archived_days.insert(covered);
            covered = covered.checked_add(Span::new().days(1))?;
        }
    }
    let mut cells = Vec::new();
    let mut day = first_day;
    loop {
        let (start_ms, _) = calendar.day_range_ms(day)?;
        let totals = sums.remove(&day);
        let day_text = day.to_string();
        let missing_daily =
            floor.as_deref().is_some_and(|f| day_text.as_str() < f) || archived_days.contains(&day);
        let partial = missing_daily && totals.is_some();
        let available = !missing_daily || partial;
        cells.push(HeatCell {
            day: day_text,
            weekday: calendar.local_weekday_of(start_ms)?,
            call_count: totals.as_ref().map_or(0, |s| s.0),
            total_tokens_known: totals.and_then(|s| s.1),
            available,
            partial,
        });
        if day == last_day {
            break;
        }
        day = day.checked_add(Span::new().days(1))?;
    }
    snapshot.commit()?;
    Ok(cells)
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
    filters: &Filters,
) -> Result<Vec<DailyRow>, CoreError> {
    let mut sql = String::from(
        "SELECT local_day, hour, instance_id, agent, provider_id, model_raw,
                call_category, quality_bucket,
                event_count, call_count, 0 as attempt_count, 0 as observation_count,
                input_known_sum, 0 as input_known_count, 0 as input_unknown_count,
                NULL as uncached_known_sum, 0 as uncached_known_count,
                cache_read_known_sum, 0 as cache_read_known_count,
                cache_write_known_sum, 0 as cache_write_known_count,
                output_known_sum, 0 as output_known_count, 0 as output_unknown_count,
                total_known_sum, 0 as total_known_count, 0 as total_unknown_count,
                NULL as ratio_input_sum, NULL as ratio_cache_read_sum, 0 as ratio_sample_count,
                conflict_count,
                NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.tz_version = hourly_usage.tz_version
                  AND d.local_day = hourly_usage.local_day AND d.instance_id = hourly_usage.instance_id
                   AND d.agent = hourly_usage.agent AND d.provider_id = hourly_usage.provider_id
                   AND d.model_raw = hourly_usage.model_raw AND d.call_category = hourly_usage.call_category
                   AND d.quality_bucket = hourly_usage.quality_bucket
                  AND d.sealed = 0) as sealed
         FROM hourly_usage
         WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3",
    );
    let mut values = vec![
        tz.to_string().into(),
        first_day.to_string().into(),
        last_day.to_string().into(),
    ];
    append_filters(&mut sql, &mut values, filters, "instance_id");
    sql.push_str(" ORDER BY local_day, hour");
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
        Ok(DailyRow {
            model_original: r.get(5)?,
            local_day: parse_date(&r.get::<_, String>(0)?)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            instance_id: r.get(2)?,
            hour: Some(r.get(1)?),
            agent: r.get(3)?,
            provider_id: r.get(4)?,
            model_raw: crate::model_names::model_key(&r.get::<_, String>(5)?),
            call_category: r.get(6)?,
            quality_bucket: r.get(7)?,
            sealed: r.get::<_, i64>(31)? != 0,
            from_period: false,
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
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    enrich_hourly_metadata(storage, tz, first_day, last_day, filters, &mut out)?;
    Ok(out)
}

/// Hourly storage predates completeness counters. Recover them from retained
/// records, without deriving unknown token fields by subtracting unlike samples.
fn enrich_hourly_metadata(
    storage: &Storage,
    tz: &str,
    first: Date,
    last: Date,
    filters: &Filters,
    rows: &mut [DailyRow],
) -> Result<(), CoreError> {
    let calendar = Calendar::new(tz)?;
    let (start, end) = period_range_ms(&calendar, first, last)?;
    let index: BTreeMap<_, _> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.sealed)
        .map(|(i, r)| {
            (
                (
                    r.local_day,
                    r.hour,
                    r.instance_id.clone(),
                    r.agent.clone(),
                    r.provider_id.clone(),
                    r.model_raw.clone(),
                    r.call_category.clone(),
                    r.quality_bucket.clone(),
                ),
                i,
            )
        })
        .collect();
    if index.is_empty() {
        return Ok(());
    }
    let mut sql = String::from(
        "SELECT occurred_at_ms,source_instance_id,agent,COALESCE(provider_id,''),COALESCE(model_raw,''),
          COALESCE(call_category,''),quality_bucket,record_kind,
          CASE WHEN json_extract(quality_json,'$.input_total') IN ('reported','derived') THEN input_total END,
          CASE WHEN json_extract(quality_json,'$.output_total') IN ('reported','derived') THEN output_total END,
          CASE WHEN json_extract(quality_json,'$.total_tokens') IN ('reported','derived') THEN total_tokens END,
          CASE WHEN json_extract(quality_json,'$.input_uncached') IN ('reported','derived') THEN input_uncached END,
          CASE WHEN json_extract(quality_json,'$.input_cache_read') IN ('reported','derived') THEN input_cache_read END
         FROM usage_events WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2 AND attribution_status='verified'
          AND record_kind IN ('model_call','transport_attempt','usage_observation')");
    let mut values = vec![start.into(), end.into()];
    append_filters(&mut sql, &mut values, filters, "source_instance_id");
    let mut stmt = storage.conn().prepare(&sql)?;
    let mut events = stmt.query(rusqlite::params_from_iter(values))?;
    while let Some(e) = events.next()? {
        let ms = e.get(0)?;
        let key = (
            calendar.local_day_of(ms)?,
            Some(calendar.local_hour_of(ms)? as i64),
            e.get(1)?,
            e.get(2)?,
            e.get(3)?,
            crate::model_names::model_key(&e.get::<_, String>(4)?),
            e.get(5)?,
            e.get(6)?,
        );
        let Some(&i) = index.get(&key) else {
            continue;
        };
        let row = &mut rows[i];
        let kind: String = e.get(7)?;
        row.attempt_count += i64::from(kind == "transport_attempt");
        row.observation_count += i64::from(kind == "usage_observation");
        if kind == "transport_attempt" {
            continue;
        }
        let input: Option<i64> = e.get(8)?;
        let output: Option<i64> = e.get(9)?;
        let total: Option<i64> = e.get(10)?;
        // 与 recompute_day 一致：quality_bucket='unknown'（无任何已知 token 字段）的
        // 记录是无用量的调用/观测，计 call/event 但不计作观测缺字段的未知字段。
        let field_gap = row.quality_bucket != "unknown";
        row.input_known_count += i64::from(input.is_some());
        row.input_unknown_count += i64::from(field_gap && input.is_none());
        row.output_known_count += i64::from(output.is_some());
        row.output_unknown_count += i64::from(field_gap && output.is_none());
        row.total_known_count += i64::from(total.is_some());
        row.total_unknown_count += i64::from(field_gap && total.is_none());
        MetricSums::checked_add_opt(&mut row.uncached_known_sum, e.get(11)?, "hourly_uncached")?;
        if let (Some(input), Some(read)) = (input, e.get::<_, Option<i64>>(12)?) {
            MetricSums::checked_add_opt(
                &mut row.ratio_input_sum,
                Some(input),
                "hourly_ratio_input",
            )?;
            MetricSums::checked_add_opt(
                &mut row.ratio_cache_read_sum,
                Some(read),
                "hourly_ratio_read",
            )?;
            row.ratio_sample_count += 1;
        }
    }
    Ok(())
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
    pub cache_write: Option<i64>,
    pub uncached: Option<i64>,
    pub cache_ratio: Option<f64>,
    pub output_total: Option<i64>,
    pub total_tokens: Option<i64>,
}

/// 按维度分组查询时间序列（趋势图表数据源；不触 usage_events 明细——降低图表
/// 数据源计算量，当前规则）。数据源与 query_summary 一致：小时粒度读
/// hourly_usage，日/周/月读 daily_usage；周/月再并入 period_usage 物化周期
/// （日层已覆盖的周期不重复计入）；筛选（Agent/provider/model/实例）同样生效。
/// 标签经共享 period_key_of 生成，与总用量视图时间轴一致。
pub fn chart_series(
    storage: &Storage,
    request: &SummaryRequest,
    dimension: &ChartDimension,
) -> Result<Vec<ChartSeriesRow>, CoreError> {
    let calendar = Calendar::new(&request.timezone)?;
    let mut groups: BTreeMap<(Date, String, String), (Date, MetricSums)> = BTreeMap::new();
    visit_summary_rows(storage, request, |row| {
        let (label, start, end) = period_key_of(
            &calendar,
            request.granularity,
            request.week_start,
            row.local_day,
            row.hour,
        );
        let model = if row.model_raw.is_empty() {
            "unknown".into()
        } else {
            row.model_raw.to_lowercase()
        };
        let agent = row.agent.to_lowercase();
        let series = match dimension {
            ChartDimension::Total => "total".into(),
            ChartDimension::ByModel => model,
            ChartDimension::ByAgent => agent,
            ChartDimension::ByAgentModel => format!("{agent}/{model}"),
        };
        groups
            .entry((start, label, series))
            .or_insert_with(|| (end, MetricSums::default()))
            .1
            .add_row(&row)?;
        Ok(())
    })?;
    Ok(groups
        .into_iter()
        .map(|((start, label, series), (end, sums))| ChartSeriesRow {
            label,
            start_day: start,
            end_day: end,
            series_name: series,
            call_count: sums.call_count,
            input_total: sums.input_total_known,
            cache_read: sums.cache_read_known,
            output_total: sums.output_total_known,
            cache_write: sums.cache_write_known,
            uncached: sums.uncached_known,
            cache_ratio: sums.cache_input_ratio().map(|r| r.as_f64()),
            total_tokens: sums.total_tokens_known,
        })
        .collect())
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
pub fn diagnostic_logs(
    storage: &Storage,
    limit: i64,
    code_filter: Option<&str>,
) -> Result<Vec<DiagnosticLogRow>, CoreError> {
    let (sql, values): (String, Vec<rusqlite::types::Value>) = match code_filter {
        Some(code) => (
            "SELECT created_ms, code, field, instance_id, message
             FROM diagnostics WHERE code = ?1 ORDER BY created_ms DESC LIMIT ?2"
                .to_string(),
            vec![
                rusqlite::types::Value::Text(code.to_string()),
                rusqlite::types::Value::Integer(limit),
            ],
        ),
        None => (
            "SELECT created_ms, code, field, instance_id, message
             FROM diagnostics ORDER BY created_ms DESC LIMIT ?1"
                .to_string(),
            vec![rusqlite::types::Value::Integer(limit)],
        ),
    };
    let mut stmt = storage.conn().prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), |r| {
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
