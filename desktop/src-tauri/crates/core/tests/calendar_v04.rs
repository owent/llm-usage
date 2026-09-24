//! V04：跨午夜、闰日、ISO 跨年周、周日起始、DST 23/25 小时分桶。
//! 半开区间；周所属年和本地日期正确；重复小时可分辨。

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::{iso_week, ymd, Calendar, WeekStart};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

fn summary_for(
    storage: &llm_usage_core::storage::Storage,
    tz: &str,
    week_start: WeekStart,
    first: jiff::civil::Date,
    last: jiff::civil::Date,
    granularity: Granularity,
    today: jiff::civil::Date,
) -> llm_usage_core::query::Summary {
    query_summary(
        storage,
        &SummaryRequest {
            timezone: tz.into(),
            week_start,
            first_day: first,
            last_day: last,
            granularity,
            filters: Filters::default(),
            today,
            retention_cutoff: None,
        },
    )
    .unwrap()
}

/// 跨午夜请求默认归到完成时间所在本地日。
#[test]
fn v04_cross_midnight_belongs_to_completion_day() {
    let (_dir, storage) = temp_storage("v04midnight");
    // UTC：开始 23:55（day1），完成 00:10（day2）。
    let mut e = with_tokens(evt("inst", "k1", ts("2026-09-25T00:10:00Z")), 100, 0);
    e.time_basis = llm_usage_core::domain::TimeBasis::SourceCompletion;
    e.interval_start_ms = Some(ts("2026-09-24T23:55:00Z"));
    e.interval_end_ms = Some(ts("2026-09-25T00:10:00Z"));
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-09-25T01:00:00Z"), vec![e]), None).unwrap();

    let s = summary_for(
        &storage, "UTC", WeekStart::Monday, ymd(2026, 9, 24), ymd(2026, 9, 25),
        Granularity::Day, ymd(2026, 9, 25),
    );
    assert_eq!(s.periods.len(), 1);
    assert_eq!(s.periods[0].label, "2026-09-25");
}

/// 闰日 2024-02-29 是合法本地日；半开区间端点归属正确。
#[test]
fn v04_leap_day_and_half_open_boundary() {
    let (_dir, storage) = temp_storage("v04leap");
    let events = vec![
        with_tokens(evt("inst", "end-of-28", ts("2024-02-29T00:00:00Z")), 1, 0), // 恰好在 29 日起点
        with_tokens(evt("inst", "leap-noon", ts("2024-02-29T12:00:00Z")), 2, 0),
        with_tokens(evt("inst", "end-of-29", ts("2024-03-01T00:00:00Z")), 4, 0), // 恰好进入 3-01
    ];
    commit_batch(&storage, &batch("inst", "UTC", ts("2024-03-01T01:00:00Z"), events), None).unwrap();
    let s = summary_for(
        &storage, "UTC", WeekStart::Monday, ymd(2024, 2, 28), ymd(2024, 3, 1),
        Granularity::Day, ymd(2024, 3, 1),
    );
    let day29 = s.periods.iter().find(|p| p.label == "2024-02-29").unwrap();
    assert_eq!(day29.sums.input_total_known, Some(3));
    let mar1 = s.periods.iter().find(|p| p.label == "2024-03-01").unwrap();
    assert_eq!(mar1.sums.input_total_known, Some(4));
    assert!(s.periods.iter().all(|p| p.label != "2024-02-28"));
}

/// ISO 跨年周：2025-12-29..2026-01-04 属 2026-W01；周标签含周所属年份。
#[test]
fn v04_iso_cross_year_week_label() {
    assert_eq!(iso_week(ymd(2025, 12, 31)), (2025 + 1, 1));
    let (_dir, storage) = temp_storage("v04isoweek");
    let events = vec![
        with_tokens(evt("inst", "a", ts("2025-12-29T10:00:00Z")), 1, 0),
        with_tokens(evt("inst", "b", ts("2025-12-31T10:00:00Z")), 2, 0),
        with_tokens(evt("inst", "c", ts("2026-01-04T10:00:00Z")), 4, 0),
    ];
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-01-05T00:00:00Z"), events), None).unwrap();
    let s = summary_for(
        &storage, "UTC", WeekStart::Monday, ymd(2025, 12, 28), ymd(2026, 1, 5),
        Granularity::Week, ymd(2026, 1, 5),
    );
    assert_eq!(s.periods.len(), 1);
    assert_eq!(s.periods[0].label, "2026-W01");
    assert_eq!(s.periods[0].start_day, ymd(2025, 12, 29));
    assert_eq!(s.periods[0].end_day, ymd(2026, 1, 4));
    assert_eq!(s.periods[0].sums.input_total_known, Some(7));
}

/// 周日起始：标签用开始日期，不能冒充 ISO 周。
#[test]
fn v04_sunday_week_uses_start_date_label() {
    let (_dir, storage) = temp_storage("v04sun");
    let events = vec![
        with_tokens(evt("inst", "a", ts("2025-12-29T10:00:00Z")), 1, 0),
        with_tokens(evt("inst", "b", ts("2026-01-01T10:00:00Z")), 2, 0),
    ];
    commit_batch(&storage, &batch("inst", "UTC", ts("2026-01-05T00:00:00Z"), events), None).unwrap();
    let s = summary_for(
        &storage, "UTC", WeekStart::Sunday, ymd(2025, 12, 27), ymd(2026, 1, 5),
        Granularity::Week, ymd(2026, 1, 5),
    );
    assert_eq!(s.periods.len(), 1);
    // 周日起始：2025-12-28 开始的一周；标签是开始日期。
    assert_eq!(s.periods[0].label, "2025-12-28");
    assert_eq!(s.periods[0].start_day, ymd(2025, 12, 28));
    assert_eq!(s.periods[0].end_day, ymd(2026, 1, 3));
}

/// DST 23 小时日（America/New_York 2026-03-08 拨快）与 25 小时日（2026-11-01 拨回）；
/// 重复小时内的事件可分辨（UTC offset 不同）但归同一本地日。
#[test]
fn v04_dst_23_and_25_hour_days() {
    let cal = Calendar::new("America/New_York").unwrap();

    // 春季拨快：2026-03-08 02:00 不存在，日本地日 23 小时。
    let (s, e) = cal.day_range_ms(ymd(2026, 3, 8)).unwrap();
    assert_eq!(e - s, 23 * 3_600_000);
    // 间隙两侧事件同属 2026-03-08。
    assert_eq!(cal.local_day_of(ts("2026-03-08T06:30:00Z")).unwrap(), ymd(2026, 3, 8)); // 01:30 EST
    assert_eq!(cal.local_day_of(ts("2026-03-08T07:30:00Z")).unwrap(), ymd(2026, 3, 8)); // 03:30 EDT

    // 秋季拨回：2026-11-01 有 25 小时，01:30 出现两次，offset 可分辨。
    let (s, e) = cal.day_range_ms(ymd(2026, 11, 1)).unwrap();
    assert_eq!(e - s, 25 * 3_600_000);
    let first = ts("2026-11-01T05:30:00Z"); // 01:30 EDT (UTC-4)
    let second = ts("2026-11-01T06:30:00Z"); // 01:30 EST (UTC-5)
    assert_eq!(cal.offset_seconds_at(first).unwrap(), -4 * 3600);
    assert_eq!(cal.offset_seconds_at(second).unwrap(), -5 * 3600);
    assert_eq!(cal.local_day_of(first).unwrap(), ymd(2026, 11, 1));
    assert_eq!(cal.local_day_of(second).unwrap(), ymd(2026, 11, 1));

    // 端到端分桶：两个重复小时事件落在同一本地日。
    let (_dir, storage) = temp_storage("v04dst");
    let events = vec![
        with_tokens(evt("inst", "edt", first), 1, 0),
        with_tokens(evt("inst", "est", second), 2, 0),
    ];
    commit_batch(&storage, &batch("inst", "America/New_York", second + 1000, events), None).unwrap();
    let s = summary_for(
        &storage, "America/New_York", WeekStart::Monday, ymd(2026, 10, 31), ymd(2026, 11, 2),
        Granularity::Day, ymd(2026, 11, 2),
    );
    assert_eq!(s.periods.len(), 1);
    assert_eq!(s.periods[0].label, "2026-11-01");
    assert_eq!(s.periods[0].sums.input_total_known, Some(3));
}

/// 跨 DST 的周/月 UTC 区间保持半开且覆盖整周期。
#[test]
fn v04_week_range_across_dst_is_half_open() {
    let cal = Calendar::new("America/New_York").unwrap();
    // 2026-11-01 所在周（周一起始 2026-10-26..2026-11-01）。
    let start = cal.week_start_of(ymd(2026, 11, 1), WeekStart::Monday);
    assert_eq!(start, ymd(2026, 10, 26));
    let (s, _) = cal.day_range_ms(start).unwrap();
    let next = start.checked_add(jiff::Span::new().days(7)).unwrap();
    let (e, _) = cal.day_range_ms(next).unwrap();
    // 该周包含 25 小时日：总时长 7*24+1 小时。
    assert_eq!(e - s, (7 * 24 + 1) * 3_600_000);
}
