//! Missing-field counts: no usage, partial usage, zeros and estimates use the same daily/hourly rules.
mod common;

use common::{batch, evt, temp_storage, ts};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::{FieldQuality, QualityBucket, RecordKind};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

#[test]
fn field_gaps_preserve_partial_usage_and_zero_across_day_and_hour() {
    let (_dir, storage) = temp_storage("field-gaps");
    let base = ts("2026-09-24T10:00:00Z");
    let mut events = Vec::new();
    for key in [
        "call-only",
        "observation-only",
        "output-zero",
        "input-only",
        "cache-only",
        "reasoning-only",
        "source-total-only",
        "estimated",
    ] {
        let mut e = evt("inst", key, base);
        e.model_raw = Some(key.into());
        match key {
            "observation-only" => e.record_kind = RecordKind::UsageObservation,
            "output-zero" => {
                e.usage.output_total = Some(0);
                e.quality.output_total = FieldQuality::Reported;
            }
            "input-only" => {
                e.usage.input_total = Some(100);
                e.quality.input_total = FieldQuality::Reported;
            }
            "cache-only" => {
                e.usage.input_cache_read = Some(40);
                e.quality.input_cache_read = FieldQuality::Reported;
            }
            "reasoning-only" => {
                e.usage.output_reasoning = Some(2);
                e.quality.output_reasoning = FieldQuality::Reported;
            }
            "source-total-only" => {
                e.usage.source_total = Some(7);
                e.quality.source_total = FieldQuality::Reported;
            }
            "estimated" => {
                e.usage.input_total = Some(120);
                e.quality.input_total = FieldQuality::Estimated;
            }
            _ => {}
        }
        events.push(e);
    }
    // Transport attempts cannot contribute usage or missing-field counts, even with tokens.
    let mut attempt = evt("inst", "retry", base);
    attempt.model_raw = Some("transport".into());
    attempt.record_kind = RecordKind::TransportAttempt;
    attempt.usage.input_total = Some(50);
    attempt.quality.input_total = FieldQuality::Reported;
    events.push(attempt);
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 1, events.clone()),
        None,
    )
    .unwrap();
    for granularity in [
        Granularity::Hour,
        Granularity::Day,
        Granularity::Week,
        Granularity::Month,
    ] {
        let request = SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 24),
            last_day: ymd(2026, 9, 24),
            today: ymd(2026, 9, 24),
            granularity,
            filters: Filters::default(),
            retention_cutoff: None,
        };
        let summary = query_summary(&storage, &request).unwrap();
        assert_eq!(summary.model_breakdown.len(), 9);
        for (model, gaps, output, calls, attempts, observations) in [
            ("call-only", (0, 0, 0), None, 1, 0, 0),
            ("observation-only", (0, 0, 0), None, 0, 0, 1),
            ("output-zero", (1, 0, 1), Some(0), 1, 0, 0),
            ("input-only", (0, 1, 1), None, 1, 0, 0),
            ("cache-only", (1, 1, 1), None, 1, 0, 0),
            ("reasoning-only", (1, 1, 1), None, 1, 0, 0),
            ("source-total-only", (1, 1, 1), None, 1, 0, 0),
            ("estimated", (1, 1, 1), None, 1, 0, 0),
            ("transport", (0, 0, 0), None, 0, 1, 0),
        ] {
            let sums = &summary
                .model_breakdown
                .iter()
                .find(|r| r.model_raw.as_deref() == Some(model))
                .unwrap()
                .sums;
            assert_eq!(
                (
                    sums.input_unknown_count,
                    sums.output_unknown_count,
                    sums.total_unknown_count
                ),
                gaps,
                "{granularity:?} / {model}"
            );
            assert_eq!(
                (sums.call_count, sums.attempt_count, sums.observation_count),
                (calls, attempts, observations)
            );
            assert_eq!(
                (
                    sums.input_total_known,
                    sums.output_total_known,
                    sums.total_tokens_known
                ),
                ((model == "input-only").then_some(100), output, None)
            );
        }
        let mut unknown_request = request.clone();
        unknown_request.filters.quality_buckets = vec![QualityBucket::Unknown];
        let unknown = query_summary(&storage, &unknown_request).unwrap();
        assert_eq!(
            unknown.model_breakdown.len(),
            2,
            "only truly empty usage belongs to unknown quality"
        );
    }
    let repeat = commit_batch(&storage, &batch("inst", "UTC", base + 2, events), None).unwrap();
    assert_eq!((repeat.added, repeat.updated, repeat.unchanged), (0, 0, 9));
}
