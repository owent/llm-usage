//! V04: midnight, leap days, ISO week-year boundaries, Sunday starts and 23/25-hour DST days.
//! Half-open intervals, correct week-year/local dates and distinguishable repeated hours.

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

/// A request spanning midnight defaults to the local day containing completion.
#[test]
fn v04_cross_midnight_belongs_to_completion_day() {
    let (_dir, storage) = temp_storage("v04midnight");
    // UTC start 23:55 on day1; completion 00:10 on day2.
    let mut e = with_tokens(evt("inst", "k1", ts("2026-09-25T00:10:00Z")), 100, 0);
    e.time_basis = llm_usage_core::domain::TimeBasis::SourceCompletion;
    e.interval_start_ms = Some(ts("2026-09-24T23:55:00Z"));
    e.interval_end_ms = Some(ts("2026-09-25T00:10:00Z"));
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-25T01:00:00Z"), vec![e]),
        None,
    )
    .unwrap();

    let s = summary_for(
        &storage,
        "UTC",
        WeekStart::Monday,
        ymd(2026, 9, 24),
        ymd(2026, 9, 25),
        Granularity::Day,
        ymd(2026, 9, 25),
    );
    assert_eq!(s.periods.len(), 1);
    assert_eq!(s.periods[0].label, "2026-09-25");
}

/// Leap day 2024-02-29 is valid; half-open endpoints select the correct day.
#[test]
fn v04_leap_day_and_half_open_boundary() {
    let (_dir, storage) = temp_storage("v04leap");
    let events = vec![
        with_tokens(evt("inst", "end-of-28", ts("2024-02-29T00:00:00Z")), 1, 0), // Exactly the start of February 29.
        with_tokens(evt("inst", "leap-noon", ts("2024-02-29T12:00:00Z")), 2, 0),
        with_tokens(evt("inst", "end-of-29", ts("2024-03-01T00:00:00Z")), 4, 0), // Exactly the start of March 1.
    ];
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2024-03-01T01:00:00Z"), events),
        None,
    )
    .unwrap();
    let s = summary_for(
        &storage,
        "UTC",
        WeekStart::Monday,
        ymd(2024, 2, 28),
        ymd(2024, 3, 1),
        Granularity::Day,
        ymd(2024, 3, 1),
    );
    let day29 = s.periods.iter().find(|p| p.label == "2024-02-29").unwrap();
    assert_eq!(day29.sums.input_total_known, Some(3));
    let mar1 = s.periods.iter().find(|p| p.label == "2024-03-01").unwrap();
    assert_eq!(mar1.sums.input_total_known, Some(4));
    assert!(s.periods.iter().all(|p| p.label != "2024-02-28"));
}

/// 2025-12-29 through 2026-01-04 is ISO 2026-W01; labels include the week-year.
#[test]
fn v04_iso_cross_year_week_label() {
    assert_eq!(iso_week(ymd(2025, 12, 31)), (2025 + 1, 1));
    let (_dir, storage) = temp_storage("v04isoweek");
    let events = vec![
        with_tokens(evt("inst", "a", ts("2025-12-29T10:00:00Z")), 1, 0),
        with_tokens(evt("inst", "b", ts("2025-12-31T10:00:00Z")), 2, 0),
        with_tokens(evt("inst", "c", ts("2026-01-04T10:00:00Z")), 4, 0),
    ];
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-01-05T00:00:00Z"), events),
        None,
    )
    .unwrap();
    let s = summary_for(
        &storage,
        "UTC",
        WeekStart::Monday,
        ymd(2025, 12, 28),
        ymd(2026, 1, 5),
        Granularity::Week,
        ymd(2026, 1, 5),
    );
    assert_eq!(s.periods.len(), 1);
    assert_eq!(s.periods[0].label, "2026-W01");
    assert_eq!(s.periods[0].start_day, ymd(2025, 12, 29));
    assert_eq!(s.periods[0].end_day, ymd(2026, 1, 4));
    assert_eq!(s.periods[0].sums.input_total_known, Some(7));
}

/// Sunday-start weeks use their start date as the label.
#[test]
fn v04_sunday_week_uses_start_date_label() {
    let (_dir, storage) = temp_storage("v04sun");
    let events = vec![
        with_tokens(evt("inst", "a", ts("2025-12-29T10:00:00Z")), 1, 0),
        with_tokens(evt("inst", "b", ts("2026-01-01T10:00:00Z")), 2, 0),
    ];
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-01-05T00:00:00Z"), events),
        None,
    )
    .unwrap();
    let s = summary_for(
        &storage,
        "UTC",
        WeekStart::Sunday,
        ymd(2025, 12, 27),
        ymd(2026, 1, 5),
        Granularity::Week,
        ymd(2026, 1, 5),
    );
    assert_eq!(s.periods.len(), 1);
    // Sunday-start week begins 2025-12-28; its start date is its label.
    assert_eq!(s.periods[0].label, "2025-12-28");
    assert_eq!(s.periods[0].start_day, ymd(2025, 12, 28));
    assert_eq!(s.periods[0].end_day, ymd(2026, 1, 3));
}

/// America/New_York DST gives 23 hours on 2026-03-08 and 25 hours on 2026-11-01.
/// Repeated-hour instants have different UTC offsets but belong to the same local day.
#[test]
fn v04_dst_23_and_25_hour_days() {
    let cal = Calendar::new("America/New_York").unwrap();

    // Spring advance: 2026-03-08 02:00 does not exist; this local day has 23 hours.
    let (s, e) = cal.day_range_ms(ymd(2026, 3, 8)).unwrap();
    assert_eq!(e - s, 23 * 3_600_000);
    // Instants on both sides of the gap belong to 2026-03-08.
    assert_eq!(
        cal.local_day_of(ts("2026-03-08T06:30:00Z")).unwrap(),
        ymd(2026, 3, 8)
    ); // Local time 01:30 EST.
    assert_eq!(
        cal.local_day_of(ts("2026-03-08T07:30:00Z")).unwrap(),
        ymd(2026, 3, 8)
    ); // Local time 03:30 EDT.

    // Fall retreat: 2026-11-01 has 25 hours; 01:30 occurs twice with different offsets.
    let (s, e) = cal.day_range_ms(ymd(2026, 11, 1)).unwrap();
    assert_eq!(e - s, 25 * 3_600_000);
    let first = ts("2026-11-01T05:30:00Z"); // Local time 01:30 EDT (UTC-4).
    let second = ts("2026-11-01T06:30:00Z"); // Local time 01:30 EST (UTC-5).
    assert_eq!(cal.offset_seconds_at(first).unwrap(), -4 * 3600);
    assert_eq!(cal.offset_seconds_at(second).unwrap(), -5 * 3600);
    assert_eq!(cal.local_day_of(first).unwrap(), ymd(2026, 11, 1));
    assert_eq!(cal.local_day_of(second).unwrap(), ymd(2026, 11, 1));

    // Full ingestion/query path groups both repeated-hour events into the same local day.
    let (_dir, storage) = temp_storage("v04dst");
    let events = vec![
        with_tokens(evt("inst", "edt", first), 1, 0),
        with_tokens(evt("inst", "est", second), 2, 0),
    ];
    commit_batch(
        &storage,
        &batch("inst", "America/New_York", second + 1000, events),
        None,
    )
    .unwrap();
    let s = summary_for(
        &storage,
        "America/New_York",
        WeekStart::Monday,
        ymd(2026, 10, 31),
        ymd(2026, 11, 2),
        Granularity::Day,
        ymd(2026, 11, 2),
    );
    assert_eq!(s.periods.len(), 1);
    assert_eq!(s.periods[0].label, "2026-11-01");
    assert_eq!(s.periods[0].sums.input_total_known, Some(3));
}

/// The UTC interval for a week spanning DST covers the complete half-open week.
#[test]
fn v04_week_range_across_dst_is_half_open() {
    let cal = Calendar::new("America/New_York").unwrap();
    // Monday-start week containing 2026-11-01: 2026-10-26 through 2026-11-01.
    let start = cal.week_start_of(ymd(2026, 11, 1), WeekStart::Monday);
    assert_eq!(start, ymd(2026, 10, 26));
    let (s, _) = cal.day_range_ms(start).unwrap();
    let next = start.checked_add(jiff::Span::new().days(7)).unwrap();
    let (e, _) = cal.day_range_ms(next).unwrap();
    // This week contains a 25-hour day: 7*24+1 hours in total.
    assert_eq!(e - s, (7 * 24 + 1) * 3_600_000);
}
