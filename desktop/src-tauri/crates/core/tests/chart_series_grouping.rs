//! chart_series 维度分组查询（2026-09-26 粒度/筛选修复回归）：
//! 小时粒度读 hourly_usage、周/月粒度按日历周期聚合、period_usage 物化周期并入、
//! Agent/模型/实例筛选生效；标签与 query_summary 总用量视图保持一致。
mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use jiff::civil::Date;
use llm_usage_core::calendar::{ymd, Calendar, WeekStart};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{
    chart_series, query_summary, ChartDimension, Filters, Granularity, SummaryRequest,
};
use llm_usage_core::storage::Storage;

fn request(granularity: Granularity, first: Date, last: Date) -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: first,
        last_day: last,
        granularity,
        filters: Filters::default(),
        today: ymd(2026, 9, 24),
        retention_cutoff: None,
    }
}

fn insert_event(storage: &Storage, key: &str, occurred: &str, model: &str, input: i64, output: i64) {
    let mut e = with_tokens(evt("inst", key, ts(occurred)), input, output);
    e.model_raw = Some(model.to_string());
    commit_batch(
        storage,
        &batch("inst", "UTC", ts("2026-09-24T12:00:00Z"), vec![e]),
        None,
    )
    .unwrap();
}

#[test]
fn hour_granularity_groups_by_hour_label_from_hourly_usage() {
    let (_dir, storage) = temp_storage("chart-hour");
    insert_event(&storage, "h1", "2026-09-24T01:30:00Z", "m1", 100, 10);
    insert_event(&storage, "h2", "2026-09-24T01:59:00Z", "m1", 50, 5);
    insert_event(&storage, "h3", "2026-09-24T14:10:00Z", "m2", 200, 20);

    let r = request(Granularity::Hour, ymd(2026, 9, 24), ymd(2026, 9, 24));
    let rows = chart_series(&storage, &r, &ChartDimension::ByModel).unwrap();

    let summary = query_summary(&storage, &r).unwrap();
    let summary_labels: Vec<&str> = summary.periods.iter().map(|p| p.label.as_str()).collect();
    let chart_labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    // 时间轴标签与总用量视图一致（小时格式 "YYYY-MM-DD HH:00"）。
    assert_eq!(chart_labels, summary_labels);
    assert_eq!(chart_labels, vec!["2026-09-24 01:00", "2026-09-24 14:00"]);

    let by: Vec<(String, String, i64, Option<i64>)> = rows
        .iter()
        .map(|r| {
            (
                r.series_name.clone(),
                r.label.clone(),
                r.call_count,
                r.input_total,
            )
        })
        .collect();
    assert_eq!(
        by,
        vec![
            ("m1".into(), "2026-09-24 01:00".into(), 2, Some(150)),
            ("m2".into(), "2026-09-24 14:00".into(), 1, Some(200)),
        ]
    );
}

#[test]
fn week_granularity_merges_days_into_calendar_weeks() {
    let (_dir, storage) = temp_storage("chart-week");
    // 同一周的周一与周三：周粒度应合并为一个周期。
    insert_event(&storage, "d1", "2026-09-21T08:00:00Z", "m1", 100, 10);
    insert_event(&storage, "d2", "2026-09-23T09:00:00Z", "m1", 40, 4);
    // 下一周。
    insert_event(&storage, "d3", "2026-09-29T10:00:00Z", "m2", 70, 7);

    let r = request(Granularity::Week, ymd(2026, 9, 1), ymd(2026, 9, 30));
    let rows = chart_series(&storage, &r, &ChartDimension::ByModel).unwrap();

    let summary = query_summary(&storage, &r).unwrap();
    let summary_labels: Vec<&str> = summary.periods.iter().map(|p| p.label.as_str()).collect();
    let chart_labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(chart_labels, summary_labels);
    assert_eq!(chart_labels.len(), 2, "two calendar weeks: {chart_labels:?}");

    let first = &rows[0];
    assert_eq!(first.series_name, "m1");
    assert_eq!(first.call_count, 2);
    assert_eq!(first.input_total, Some(140));
    assert_eq!(first.start_day, ymd(2026, 9, 21));
    assert_eq!(first.end_day, ymd(2026, 9, 27));
}

#[test]
fn month_granularity_uses_month_labels() {
    let (_dir, storage) = temp_storage("chart-month");
    insert_event(&storage, "d1", "2026-08-05T08:00:00Z", "m1", 100, 10);
    insert_event(&storage, "d2", "2026-09-05T09:00:00Z", "m1", 40, 4);

    let r = request(Granularity::Month, ymd(2026, 8, 1), ymd(2026, 9, 30));
    let rows = chart_series(&storage, &r, &ChartDimension::ByAgent).unwrap();
    let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, vec!["2026-08", "2026-09"]);
    assert!(rows.iter().all(|r| r.series_name == "agent-a"));
}

#[test]
fn filters_apply_to_grouped_series() {
    let (_dir, storage) = temp_storage("chart-filters");
    insert_event(&storage, "f1", "2026-09-24T01:00:00Z", "m1", 100, 10);
    insert_event(&storage, "f2", "2026-09-24T02:00:00Z", "m2", 200, 20);

    let mut r = request(Granularity::Day, ymd(2026, 9, 24), ymd(2026, 9, 24));
    r.filters.models = vec!["m1".into()];
    let rows = chart_series(&storage, &r, &ChartDimension::ByModel).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].series_name, "m1");
    assert_eq!(rows[0].call_count, 1);

    // 实例白名单：其他实例的数据不出现。
    r.filters = Filters {
        instances: vec!["other-inst".into()],
        ..Default::default()
    };
    let rows = chart_series(&storage, &r, &ChartDimension::ByModel).unwrap();
    assert!(rows.is_empty());
}

#[test]
fn week_merges_materialized_period_usage_and_skips_covered() {
    let (_dir, storage) = temp_storage("chart-materialized");
    // 日层数据只覆盖 2026-09-21 那一周；更早的一周只有 period_usage 物化行。
    insert_event(&storage, "d1", "2026-09-21T08:00:00Z", "m1", 100, 10);

    let cal = Calendar::new("UTC").unwrap();
    let old_week_start = ymd(2026, 8, 31);
    let old_key = cal.week_label(old_week_start, WeekStart::Monday);
    storage
        .conn()
        .execute(
            "INSERT INTO period_usage (tz_version, granularity, period_key, period_start_day,
                period_end_day, instance_id, agent, provider_id, model_raw, call_category,
                quality_bucket, event_count, call_count, input_known_sum, cache_read_known_sum,
                cache_write_known_sum, output_known_sum, total_known_sum, conflict_count,
                active_days, distinct_sessions, materialized_at_ms, data_revision)
             VALUES ('UTC', 'week', ?1, ?2, ?3, 'old-inst', 'agent-b', 'prov', 'm2',
                'primary', 'reported', 5, 5, 500, 400, NULL, 50, 550, 0, 3, 2, 0, 1)",
            rusqlite::params![
                old_key,
                old_week_start.to_string(),
                ymd(2026, 9, 6).to_string()
            ],
        )
        .unwrap();

    let r = request(Granularity::Week, ymd(2026, 8, 1), ymd(2026, 9, 30));
    let rows = chart_series(&storage, &r, &ChartDimension::ByModel).unwrap();
    let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, vec![old_key.as_str(), cal.week_label(ymd(2026, 9, 21), WeekStart::Monday).as_str()]);

    let old = &rows[0];
    assert_eq!((old.series_name.as_str(), old.call_count, old.input_total), ("m2", 5, Some(500)));
    assert_eq!(old.start_day, old_week_start);
    assert_eq!(old.end_day, ymd(2026, 9, 6));
    // 日层已覆盖的那周不含物化实例的数据（covered 去重）。
    let covered = &rows[1];
    assert_eq!(covered.call_count, 1);
    assert!(rows.iter().all(|r| r.series_name != "m2" || r.label == old_key));

    // 与总用量视图的周期集合一致。
    let summary = query_summary(&storage, &r).unwrap();
    let summary_labels: Vec<&str> =
        summary.periods.iter().map(|p| p.label.as_str()).collect();
    let chart_labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(chart_labels, summary_labels);

    // 实例白名单筛选同样作用于物化行。
    let mut r2 = r.clone();
    r2.filters.instances = vec!["inst".into()];
    let rows = chart_series(&storage, &r2, &ChartDimension::ByModel).unwrap();
    assert!(rows.iter().all(|r| r.label != old_key));
}
