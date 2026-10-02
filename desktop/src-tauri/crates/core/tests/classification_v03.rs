//! V03：调用/尝试/消息/累计值分类。失败无 usage 计调用不计 token；
//! 未知 token 不补零；无证据不产生 request 数。

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
    // 成功调用，有 usage。
    let call = with_tokens(evt("inst", "call-1", base), 100, 50);
    // transport retry：独立计数，不增加 model_call。
    let mut attempt = evt("inst", "attempt-1", base + 1);
    attempt.record_kind = RecordKind::TransportAttempt;
    // 一条 usage_observation：计入用量，不计入调用数。
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
    // token 合计 = call 100+50 与 observation 200+100；attempt 无 token 贡献。
    assert_eq!(sums.input_total_known, Some(300));
    assert_eq!(sums.output_total_known, Some(150));
}

#[test]
fn v03_failed_call_without_usage_counts_call_not_tokens() {
    let (_dir, storage) = temp_storage("v03fail");
    let base = ts("2026-09-24T10:00:00Z");
    let mut failed = evt("inst", "call-fail", base);
    failed.error_status = Some("http_500".into());
    // usage 全部未知。
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
    // 无用量调用（quality_bucket=unknown）计入 call_count，但不算观测缺字段的未知字段。
    assert_eq!(sums.input_unknown_count, 0);
    assert_eq!(sums.total_tokens_known, None);

    // 质量分区为 unknown（无任何已知 token 字段）。
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
