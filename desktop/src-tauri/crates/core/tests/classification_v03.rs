//! V03 call/attempt/message/cumulative categories; failed calls without usage count calls, not tokens.
//! Missing tokens stay unknown; unobserved calls do not create request counts.

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::{QualityBucket, RecordKind};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

fn summarize(storage: &llm_usage_core::storage::Storage) -> llm_usage_core::query::Summary {
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
}

#[test]
fn v03_call_attempt_observation_classified_separately() {
    let (_dir, storage) = temp_storage("v03class");
    let base = ts("2026-09-24T10:00:00Z");
    // Successful call with usage.
    let call = with_tokens(evt("inst", "call-1", base), 100, 50);
    // Transport retry is counted separately and does not increase model_call.
    let mut attempt = evt("inst", "attempt-1", base + 1);
    attempt.record_kind = RecordKind::TransportAttempt;
    // usage_observation contributes usage, not calls.
    let mut observation = with_tokens(evt("inst", "obs-1", base + 2), 200, 100);
    observation.record_kind = RecordKind::UsageObservation;
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 3, vec![call, attempt, observation]),
        None,
    )
    .unwrap();

    let summary = summarize(&storage);
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.call_count, 1);
    assert_eq!(sums.attempt_count, 1);
    assert_eq!(sums.observation_count, 1);
    assert_eq!(sums.event_count, 3);
    // Tokens: call 100+50 plus observation 200+100; attempt contributes none.
    assert_eq!(sums.input_total_known, Some(300));
    assert_eq!(sums.output_total_known, Some(150));
}

#[test]
fn v03_failed_call_without_usage_counts_call_not_tokens() {
    let (_dir, storage) = temp_storage("v03fail");
    let base = ts("2026-09-24T10:00:00Z");
    let mut failed = evt("inst", "call-fail", base);
    failed.error_status = Some("http_500".into());
    // All usage fields are unknown.
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 1, vec![failed]),
        None,
    )
    .unwrap();

    let summary = summarize(&storage);
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.call_count, 1);
    assert_eq!(sums.input_total_known, None);
    // Unknown-quality usage-free calls increase call_count without adding unknown-field observations.
    assert_eq!(sums.input_unknown_count, 0);
    assert_eq!(sums.total_tokens_known, None);

    // Quality partition is unknown because no token field is known.
    let bucket: String = storage
        .conn()
        .query_row("SELECT quality_bucket FROM daily_usage LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(bucket, QualityBucket::Unknown.as_str());
}

#[test]
fn v03_usage_observation_alone_produces_no_request_count() {
    let (_dir, storage) = temp_storage("v03obs");
    let base = ts("2026-09-24T10:00:00Z");
    let mut observation = with_tokens(evt("inst", "obs-1", base), 500, 0);
    observation.record_kind = RecordKind::UsageObservation;
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 1, vec![observation]),
        None,
    )
    .unwrap();
    let summary = summarize(&storage);
    assert_eq!(summary.periods[0].sums.call_count, 0);
    assert_eq!(summary.periods[0].sums.input_total_known, Some(500));
}
