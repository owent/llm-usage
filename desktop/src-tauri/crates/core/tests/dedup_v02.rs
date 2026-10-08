//! V02 repeats/corrections/disorder: identical final counts once; corrections update prior totals.
//! Disorder never silently overwrites; undecidable order keeps conflict instead of MAX.

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::Lifecycle;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

fn day_input_total(storage: &llm_usage_core::storage::Storage) -> Option<i64> {
    query_summary(
        storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 24),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap()
    .periods
    .first()
    .map(|p| p.sums.input_total_known)
    .unwrap_or(None)
}

#[test]
fn v02_duplicate_final_is_idempotent() {
    let (_dir, storage) = temp_storage("v02dup");
    let ms = ts("2026-09-24T10:00:00Z");
    let make = || with_tokens(evt("inst", "req-1", ms), 100, 0);
    let o1 = commit_batch(&storage, &batch("inst", "UTC", ms, vec![make()]), None).unwrap();
    assert_eq!(o1.added, 1);
    // Identical final for the same request is retained without duplicates.
    let o2 = commit_batch(&storage, &batch("inst", "UTC", ms + 1, vec![make()]), None).unwrap();
    assert_eq!(o2.added, 0);
    assert_eq!(o2.unchanged, 1);
    assert_eq!(day_input_total(&storage), Some(100));
}

#[test]
fn v02_correction_by_revision_replaces() {
    let (_dir, storage) = temp_storage("v02corr");
    let ms = ts("2026-09-24T10:00:00Z");
    let mut e1 = with_tokens(evt("inst", "req-1", ms), 100, 0);
    e1.source_revision = Some(1);
    commit_batch(&storage, &batch("inst", "UTC", ms, vec![e1]), None).unwrap();
    let mut e2 = with_tokens(evt("inst", "req-1", ms), 60, 0);
    e2.source_revision = Some(2);
    e2.lifecycle = Lifecycle::Corrected;
    let out = commit_batch(&storage, &batch("inst", "UTC", ms + 1, vec![e2]), None).unwrap();
    assert_eq!(out.updated, 1);
    assert_eq!(day_input_total(&storage), Some(60));
}

#[test]
fn v02_out_of_order_lower_revision_does_not_overwrite() {
    let (_dir, storage) = temp_storage("v02ooo");
    let ms = ts("2026-09-24T10:00:00Z");
    let mut newer = with_tokens(evt("inst", "req-1", ms), 60, 0);
    newer.source_revision = Some(2);
    newer.lifecycle = Lifecycle::Corrected;
    commit_batch(&storage, &batch("inst", "UTC", ms, vec![newer]), None).unwrap();
    // A late old revision keeps existing state without overwriting.
    let mut older = with_tokens(evt("inst", "req-1", ms), 100, 0);
    older.source_revision = Some(1);
    let out = commit_batch(&storage, &batch("inst", "UTC", ms + 1, vec![older]), None).unwrap();
    assert_eq!(out.unchanged, 1);
    assert_eq!(out.updated, 0);
    assert_eq!(day_input_total(&storage), Some(60));
}

#[test]
fn v02_same_revision_different_content_is_conflict_not_max() {
    let (_dir, storage) = temp_storage("v02conf");
    let ms = ts("2026-09-24T10:00:00Z");
    let mut e1 = with_tokens(evt("inst", "req-1", ms), 100, 0);
    e1.source_revision = Some(5);
    commit_batch(&storage, &batch("inst", "UTC", ms, vec![e1]), None).unwrap();
    // Equal revision/different content keeps existing data and records conflict, without MAX.
    let mut e2 = with_tokens(evt("inst", "req-1", ms), 999, 0);
    e2.source_revision = Some(5);
    let out = commit_batch(&storage, &batch("inst", "UTC", ms + 1, vec![e2]), None).unwrap();
    assert_eq!(out.conflicts, 1);
    assert_eq!(day_input_total(&storage), Some(100));
    // Diagnostics and daily conflict counts are visible.
    let diag_count: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'update_conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag_count, 1);
    let conflict_sum: i64 = storage
        .conn()
        .query_row("SELECT SUM(conflict_count) FROM daily_usage", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(conflict_sum, 1);
}

#[test]
fn v02_lifecycle_ordering_without_revision() {
    let (_dir, storage) = temp_storage("v02life");
    let ms = ts("2026-09-24T10:00:00Z");
    // A partial arriving after final cannot replace final.
    let fin = with_tokens(evt("inst", "req-1", ms), 100, 0);
    commit_batch(&storage, &batch("inst", "UTC", ms, vec![fin]), None).unwrap();
    let mut partial = with_tokens(evt("inst", "req-1", ms), 80, 0);
    partial.lifecycle = Lifecycle::Partial;
    let out = commit_batch(&storage, &batch("inst", "UTC", ms + 1, vec![partial]), None).unwrap();
    assert_eq!(out.unchanged, 1);
    assert_eq!(day_input_total(&storage), Some(100));

    // corrected replaces final.
    let mut corrected = with_tokens(evt("inst", "req-1", ms), 90, 0);
    corrected.lifecycle = Lifecycle::Corrected;
    let out = commit_batch(
        &storage,
        &batch("inst", "UTC", ms + 2, vec![corrected]),
        None,
    )
    .unwrap();
    assert_eq!(out.updated, 1);
    assert_eq!(day_input_total(&storage), Some(90));
}

/// A correction moving to another day removes the old contribution and adds the new one.
#[test]
fn v02_correction_moves_day_contribution() {
    let (_dir, storage) = temp_storage("v02move");
    let day1 = ts("2026-09-23T10:00:00Z");
    let day2 = ts("2026-09-24T10:00:00Z");
    let mut e1 = with_tokens(evt("inst", "req-1", day1), 100, 0);
    e1.source_revision = Some(1);
    commit_batch(&storage, &batch("inst", "UTC", day1, vec![e1]), None).unwrap();

    let mut e2 = with_tokens(evt("inst", "req-1", day2), 100, 0);
    e2.source_revision = Some(2);
    e2.lifecycle = Lifecycle::Corrected;
    commit_batch(&storage, &batch("inst", "UTC", day2, vec![e2]), None).unwrap();

    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 23),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(summary.periods.len(), 1);
    assert_eq!(summary.periods[0].label, "2026-09-24");
    assert_eq!(summary.periods[0].sums.input_total_known, Some(100));
}
