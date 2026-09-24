//! V06：日→周/月加总、加权比例 18% 用例、distinct 会话、部分周期标记。
//! 不平均百分比、不叠加每日 distinct。

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::FieldQuality;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

fn request(
    tz: &str,
    first: jiff::civil::Date,
    last: jiff::civil::Date,
    granularity: Granularity,
    today: jiff::civil::Date,
    cutoff: Option<jiff::civil::Date>,
) -> SummaryRequest {
    SummaryRequest {
        timezone: tz.into(),
        week_start: WeekStart::Monday,
        first_day: first,
        last_day: last,
        granularity,
        filters: Filters::default(),
        today,
        retention_cutoff: cutoff,
    }
}

/// 日→周/月加总一致；跨天加权比例为 18%（不取各日平均 50%）。
#[test]
fn v06_rollup_sums_and_weighted_ratio() {
    let (_dir, storage) = temp_storage("v06rollup");
    // 甲（day1）：输入 100/缓存读 90；乙（day2）：输入 900/缓存读 90。
    let mut a = with_tokens(evt("inst", "a", ts("2026-09-22T12:00:00Z")), 100, 10);
    a.usage.input_cache_read = Some(90);
    a.quality.input_cache_read = FieldQuality::Reported;
    let mut b = with_tokens(evt("inst", "b", ts("2026-09-23T12:00:00Z")), 900, 10);
    b.usage.input_cache_read = Some(90);
    b.quality.input_cache_read = FieldQuality::Reported;
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-09-24T00:00:00Z"), vec![a, b]), None)
        .unwrap();

    let daily = query_summary(
        &storage,
        &request("UTC", ymd(2026, 9, 22), ymd(2026, 9, 23), Granularity::Day, ymd(2026, 9, 24), None),
    )
    .unwrap();
    assert_eq!(daily.periods.len(), 2);
    // 各日比例 90% / 10%。
    assert!((daily.periods[0].sums.cache_input_ratio().unwrap().as_f64() - 0.90).abs() < 1e-12);
    assert!((daily.periods[1].sums.cache_input_ratio().unwrap().as_f64() - 0.10).abs() < 1e-12);

    for granularity in [Granularity::Week, Granularity::Month] {
        let s = query_summary(
            &storage,
            &request("UTC", ymd(2026, 9, 21), ymd(2026, 9, 30), granularity, ymd(2026, 9, 24), None),
        )
        .unwrap();
        assert_eq!(s.periods.len(), 1);
        let p = &s.periods[0];
        // 加总与比例重新计算：180/1000 = 18%。
        assert_eq!(p.sums.input_total_known, Some(1000));
        assert_eq!(p.sums.cache_read_known, Some(180));
        let ratio = p.sums.cache_input_ratio().unwrap();
        assert_eq!((ratio.numerator, ratio.denominator), (180, 1000));
        assert!((ratio.as_f64() - 0.18).abs() < 1e-12);
        assert_eq!(p.sums.ratio_sample_count(), 2);
    }
}

/// 分母为零：无有效占比（显示"—"），不是 0%。
#[test]
fn v06_zero_denominator_ratio_is_none() {
    let (_dir, storage) = temp_storage("v06zero");
    let mut e = with_tokens(evt("inst", "z", ts("2026-09-24T12:00:00Z")), 0, 0);
    e.usage.input_cache_read = Some(0);
    e.quality.input_cache_read = FieldQuality::Reported;
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-09-24T13:00:00Z"), vec![e]), None).unwrap();
    let s = query_summary(
        &storage,
        &request("UTC", ymd(2026, 9, 24), ymd(2026, 9, 24), Granularity::Day, ymd(2026, 9, 24), None),
    )
    .unwrap();
    let sums = &s.periods[0].sums;
    assert_eq!(sums.cache_input_ratio(), None);
    assert_eq!(sums.ratio_sample_count(), 1); // 字段已知但分母为零
}

/// distinct 会话不叠加每日：同会话两天 → 周 distinct=1；另一天另一会话 → 2。
#[test]
fn v06_distinct_sessions_not_summed_across_days() {
    let (_dir, storage) = temp_storage("v06sess");
    let mk = |key: &str, session: &str, ms: i64| {
        let mut e = with_tokens(evt("inst", key, ms), 10, 0);
        e.session_id = Some(session.into());
        e
    };
    let events = vec![
        mk("k1", "sess-1", ts("2026-09-22T10:00:00Z")),
        mk("k2", "sess-1", ts("2026-09-23T10:00:00Z")),
        mk("k3", "sess-2", ts("2026-09-23T11:00:00Z")),
    ];
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-09-24T00:00:00Z"), events), None).unwrap();

    let daily = query_summary(
        &storage,
        &request("UTC", ymd(2026, 9, 22), ymd(2026, 9, 23), Granularity::Day, ymd(2026, 9, 24), None),
    )
    .unwrap();
    assert_eq!(daily.periods[0].distinct_sessions, Some(1));
    assert_eq!(daily.periods[1].distinct_sessions, Some(2));

    let weekly = query_summary(
        &storage,
        &request("UTC", ymd(2026, 9, 21), ymd(2026, 9, 27), Granularity::Week, ymd(2026, 9, 24), None),
    )
    .unwrap();
    // 周 DISTINCT = 2，不是每日之和 1+2=3。
    assert_eq!(weekly.periods[0].distinct_sessions, Some(2));
    assert_eq!(weekly.periods[0].active_days, Some(2));
}

/// 部分周期：当前周标记“进行中”；被保留截断的最早周标记“部分历史”。
#[test]
fn v06_partial_period_flags() {
    let (_dir, storage) = temp_storage("v06partial");
    let events = vec![
        with_tokens(evt("inst", "old", ts("2026-08-03T10:00:00Z")), 1, 0), // 2026-W32
        with_tokens(evt("inst", "now", ts("2026-09-24T10:00:00Z")), 2, 0), // 2026-W39
    ];
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-09-24T12:00:00Z"), events), None).unwrap();

    let today = ymd(2026, 9, 24); // 周四
    let s = query_summary(
        &storage,
        &request(
            "UTC",
            ymd(2026, 8, 3),
            ymd(2026, 9, 24),
            Granularity::Week,
            today,
            Some(ymd(2026, 8, 20)), // 保留截止：8-20 之前的周期被截断
        ),
    )
    .unwrap();
    assert_eq!(s.periods.len(), 2);
    let old_week = &s.periods[0];
    assert_eq!(old_week.label, "2026-W32");
    assert!(old_week.partial_history);
    assert!(!old_week.in_progress);
    let current_week = &s.periods[1];
    assert_eq!(current_week.label, "2026-W39");
    assert!(current_week.in_progress); // 今天仍在该周
    assert!(!current_week.partial_history);

    // 已结束的周期不是"进行中"。
    assert!(!old_week.in_progress);
}
