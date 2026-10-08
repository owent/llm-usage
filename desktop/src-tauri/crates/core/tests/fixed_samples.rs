//! Eleven fixed mathematical examples from validation.md; expectations are literal, not implementation-generated.

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::adapters::codex::{map_codex, CodexUsage};
use llm_usage_core::aggregates::{
    observe_cumulative, sum_exclusive_aggregates, upsert_source_aggregate, AggregateScope,
    Coverage, CumulativeOutcome, SourceAggregateInput,
};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::{FieldQuality, TimeBasis, TokenQuality, TokenUsage};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::metrics::{cache_input_ratio, input_total, total_tokens};
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

/// Example 1: uncached input 100, cache reads 800, cache writes 100 and output 100
/// give input_total 1000, total tokens 1100 and an 80% cached-input ratio.
#[test]
fn sample1_mutually_exclusive_parts() {
    let usage = TokenUsage {
        input_uncached: Some(100),
        input_cache_read: Some(800),
        input_cache_write: Some(100),
        input_total: None,
        output_total: Some(100),
        ..TokenUsage::default()
    };
    let quality = TokenQuality {
        input_uncached: FieldQuality::Reported,
        input_cache_read: FieldQuality::Reported,
        input_cache_write: FieldQuality::Reported,
        output_total: FieldQuality::Reported,
        ..TokenQuality::default()
    };
    assert_eq!(input_total(&usage, &quality).map(|(v, _)| v), Some(1000));
    assert_eq!(total_tokens(&usage, &quality).map(|(v, _)| v), Some(1100));
    let (ratio, n) = cache_input_ratio(&[(Some(1000), Some(800))]);
    assert_eq!(n, 1);
    let ratio = ratio.unwrap();
    assert_eq!((ratio.numerator, ratio.denominator), (800, 1000));
    assert!((ratio.as_f64() - 0.80).abs() < 1e-12);
}

/// Example 2: input_total 1000 includes cache reads 800; output 100 includes reasoning 40.
/// Total tokens are 1100; with verified absent cache writes, uncached input is 200.
#[test]
fn sample2_inclusive_input_no_double_count() {
    let mapped = map_codex(&CodexUsage {
        input_tokens: 1000,
        cached_input_tokens: 800,
        output_tokens: 100,
        reasoning_output_tokens: 40,
        total_tokens: 1100,
        declares_no_cache_creation: true,
    });
    assert_eq!(mapped.usage.input_uncached, Some(200));
    assert_eq!(mapped.usage.input_total, Some(1000));
    assert_eq!(mapped.usage.output_total, Some(100));
    assert_eq!(
        total_tokens(&mapped.usage, &mapped.quality).map(|(v, _)| v),
        Some(1100)
    );
    // Adding cache/reasoning again would wrongly give 1940.
    assert_ne!(
        total_tokens(&mapped.usage, &mapped.quality).map(|(v, _)| v),
        Some(1940)
    );
    assert!(mapped.diagnostics.is_empty());
}

/// Example 3: A has input/cache 100/90 and B has 900/90; the combined ratio is 18%, not 50%.
#[test]
fn sample3_weighted_ratio_not_average() {
    let (merged, n) = cache_input_ratio(&[(Some(100), Some(90)), (Some(900), Some(90))]);
    assert_eq!(n, 2);
    let merged = merged.unwrap();
    assert_eq!((merged.numerator, merged.denominator), (180, 1000));
    assert!((merged.as_f64() - 0.18).abs() < 1e-12);
    // Individual ratios are 90% and 10%; their 50% arithmetic mean is not the combined ratio.
    let (a, _) = cache_input_ratio(&[(Some(100), Some(90))]);
    let (b, _) = cache_input_ratio(&[(Some(900), Some(90))]);
    assert!((a.unwrap().as_f64() - 0.90).abs() < 1e-12);
    assert!((b.unwrap().as_f64() - 0.10).abs() < 1e-12);
    assert!(((a.unwrap().as_f64() + b.unwrap().as_f64()) / 2.0 - 0.50).abs() < 1e-12);
    assert!((merged.as_f64() - 0.50).abs() > 1e-12);
}

/// Example 4: input 100 with unknown output keeps known input 100 and a missing complete total.
#[test]
fn sample4_unknown_output_not_a_total() {
    // Pure-function check.
    let usage = TokenUsage {
        input_total: Some(100),
        ..TokenUsage::default()
    };
    let quality = TokenQuality {
        input_total: FieldQuality::Reported,
        ..TokenQuality::default()
    };
    assert_eq!(total_tokens(&usage, &quality), None);

    // Stored daily input is 100; output/total remain NULL without substituting zero.
    let (_dir, storage) = temp_storage("sample4");
    let mut e = evt("inst", "k1", ts("2026-09-24T12:00:00Z"));
    e.usage.input_total = Some(100);
    e.quality.input_total = FieldQuality::Reported;
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T13:00:00Z"), vec![e]),
        None,
    )
    .unwrap();
    let summary = query_summary(
        &storage,
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
    .unwrap();
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.input_total_known, Some(100));
    assert_eq!(sums.output_total_known, None);
    assert_eq!(sums.total_tokens_known, None);
    assert_eq!(sums.input_known_count, 1);
    assert_eq!(sums.output_unknown_count, 1);
}

/// Example 5: revising one request from 100 to 80 leaves value 80 and one call, neither 180 nor MAX=100.
#[test]
fn sample5_correction_replaces_old_contribution() {
    let (_dir, storage) = temp_storage("sample5");
    let mut e1 = with_tokens(evt("inst", "req-1", ts("2026-09-24T10:00:00Z")), 100, 0);
    e1.source_revision = Some(1);
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T10:01:00Z"), vec![e1]),
        None,
    )
    .unwrap();

    let mut e2 = with_tokens(evt("inst", "req-1", ts("2026-09-24T10:00:00Z")), 80, 0);
    e2.source_revision = Some(2);
    e2.lifecycle = llm_usage_core::domain::Lifecycle::Corrected;
    let outcome = commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T10:02:00Z"), vec![e2]),
        None,
    )
    .unwrap();
    assert_eq!(outcome.updated, 1);

    let summary = query_summary(
        &storage,
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
    .unwrap();
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.input_total_known, Some(80));
    assert_eq!(sums.call_count, 1);
}

/// Example 6: two stable request ids each report 100; total 200 and two calls despite identical usage.
#[test]
fn sample6_identical_content_different_ids_counts_twice() {
    let (_dir, storage) = temp_storage("sample6");
    let ms = ts("2026-09-24T10:00:00Z");
    let e1 = with_tokens(evt("inst", "req-a", ms), 100, 0);
    let e2 = with_tokens(evt("inst", "req-b", ms), 100, 0);
    let outcome = commit_batch(
        &storage,
        &batch("inst", "UTC", ms + 1000, vec![e1, e2]),
        None,
    )
    .unwrap();
    assert_eq!(outcome.added, 2);
    let summary = query_summary(
        &storage,
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
    .unwrap();
    assert_eq!(summary.periods[0].sums.input_total_known, Some(200));
    assert_eq!(summary.periods[0].sums.call_count, 2);
}

/// Example 7: one DSH attempt streams 80 then finishes 100; a retry finishes 40, giving 140.
#[test]
fn sample7_dsh_attempt_stream_then_retry() {
    let (_dir, storage) = temp_storage("sample7");
    let base = ts("2026-09-24T10:00:00Z");
    let mut partial = with_tokens(evt("inst", "attempt-1", base), 80, 0);
    partial.lifecycle = llm_usage_core::domain::Lifecycle::Partial;
    partial.attempt_id = Some("attempt-1".into());
    let mut fin = with_tokens(evt("inst", "attempt-1", base), 100, 0);
    fin.attempt_id = Some("attempt-1".into());
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 1000, vec![partial]),
        None,
    )
    .unwrap();
    // Final usage replaces streamed usage for the same attempt.
    let out = commit_batch(
        &storage,
        &batch("inst", "UTC", base + 2000, vec![fin]),
        None,
    )
    .unwrap();
    assert_eq!(out.updated, 1);
    // The retry creates a distinct attempt.
    let mut retry = with_tokens(evt("inst", "attempt-2", base + 5000), 40, 0);
    retry.attempt_id = Some("attempt-2".into());
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 6000, vec![retry]),
        None,
    )
    .unwrap();

    let summary = query_summary(
        &storage,
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
    .unwrap();
    assert_eq!(summary.periods[0].sums.input_total_known, Some(140));
    assert_eq!(summary.periods[0].sums.call_count, 2);
}

/// Example 8: cumulative 100->150->150 gives deltas 50 and 0; a verified new process starts at 20.
/// Keep the initial 100 as its source interval, without assigning it to today.
#[test]
fn sample8_cumulative_deltas_and_reset() {
    let t1 = ts("2026-09-23T08:00:00Z");
    let t2 = ts("2026-09-24T08:00:00Z");
    let t3 = ts("2026-09-24T09:00:00Z");
    let t4 = ts("2026-09-24T10:00:00Z");

    let (state, out1) = observe_cumulative("series-1", None, 100, t1, false);
    assert_eq!(
        out1,
        CumulativeOutcome::FirstObservation { native_total: 100 }
    );
    let (state, out2) = observe_cumulative("series-1", Some(&state), 150, t2, false);
    assert_eq!(out2, CumulativeOutcome::Delta { amount: 50 });
    let (state, out3) = observe_cumulative("series-1", Some(&state), 150, t3, false);
    assert_eq!(out3, CumulativeOutcome::Delta { amount: 0 });
    // A verified new process resets the baseline to 20 for its new interval.
    let (_state, out4) = observe_cumulative("series-1", Some(&state), 20, t4, true);
    assert_eq!(out4, CumulativeOutcome::Reset { new_baseline: 20 });
    // An unverified decrease cannot reset to zero and be accumulated anew.
    let (state_x, _) = observe_cumulative("series-x", None, 100, t1, false);
    let (_s, out_y) = observe_cumulative("series-x", Some(&state_x), 30, t2, false);
    assert_eq!(
        out_y,
        CumulativeOutcome::Regression {
            previous: 100,
            observed: 30
        }
    );

    // Preserve the initial 100 as a native source interval with unknown start, outside daily totals.
    let (_dir, storage) = temp_storage("sample8");
    let changed = upsert_source_aggregate(
        &storage,
        &SourceAggregateInput {
            instance_id: "inst".into(),
            scope: AggregateScope::ProcessSeries,
            scope_key: "series-1:first".into(),
            interval_start_ms: None,
            interval_end_ms: t1,
            interval_end_inclusive: true,
            usage: TokenUsage {
                total_tokens: Some(100),
                ..TokenUsage::default()
            },
            quality: TokenQuality {
                total_tokens: FieldQuality::Reported,
                ..TokenQuality::default()
            },
            reported_call_count: None,
            coverage: Coverage::OverlapUnknown,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: None,
        },
        t1,
    )
    .unwrap();
    assert!(changed);
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
    // No per-call events exist, so daily totals stay empty instead of receiving the interval 100.
    assert!(summary.periods.is_empty());
    assert_eq!(summary.totals.event_count, 0);
}

/// Example 9: one session active on two days has weekly/monthly DISTINCT session=1 and active_days=2.
#[test]
fn sample9_distinct_session_across_days() {
    let (_dir, storage) = temp_storage("sample9");
    let mut e1 = with_tokens(evt("inst", "k1", ts("2026-09-23T12:00:00Z")), 10, 5);
    e1.session_id = Some("sess-1".into());
    let mut e2 = with_tokens(evt("inst", "k2", ts("2026-09-24T12:00:00Z")), 20, 5);
    e2.session_id = Some("sess-1".into());
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T13:00:00Z"), vec![e1, e2]),
        None,
    )
    .unwrap();

    for granularity in [Granularity::Week, Granularity::Month] {
        let summary = query_summary(
            &storage,
            &SummaryRequest {
                timezone: "UTC".into(),
                week_start: WeekStart::Monday,
                first_day: ymd(2026, 9, 21),
                last_day: ymd(2026, 9, 30),
                granularity,
                filters: Filters::default(),
                today: ymd(2026, 9, 24),
                retention_cutoff: None,
            },
        )
        .unwrap();
        assert_eq!(summary.periods.len(), 1);
        let period = &summary.periods[0];
        assert_eq!(period.distinct_sessions, Some(1));
        assert_eq!(period.active_days, Some(2));
    }
}

/// Example 10: a Hermes two-day cumulative row has token=1000/api_call_count=3 and only first_seen/last_seen.
/// Keep an interval aggregate, without three invented model_call events or assigning 1000 to the last day; rereads add nothing.
#[test]
fn sample10_hermes_interval_aggregate() {
    let (_dir, storage) = temp_storage("sample10");
    let first_seen = ts("2026-09-22T10:00:00Z");
    let last_seen = ts("2026-09-23T18:00:00Z");
    let input = SourceAggregateInput {
        instance_id: "hermes".into(),
        scope: AggregateScope::Custom,
        scope_key: "hermes-row-1".into(),
        interval_start_ms: Some(first_seen),
        interval_end_ms: last_seen,
        interval_end_inclusive: true,
        usage: TokenUsage {
            total_tokens: Some(1000),
            ..TokenUsage::default()
        },
        quality: TokenQuality {
            total_tokens: FieldQuality::Reported,
            ..TokenQuality::default()
        },
        reported_call_count: Some(3),
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::Uncertain,
        source_revision: None,
    };
    assert!(upsert_source_aggregate(&storage, &input, last_seen).unwrap());
    // Repeating the same aggregate adds no usage.
    assert!(!upsert_source_aggregate(&storage, &input, last_seen + 1000).unwrap());

    let totals = sum_exclusive_aggregates(&storage, "hermes").unwrap();
    assert_eq!(totals.total_tokens, Some(1000));
    assert_eq!(totals.reported_call_count, Some(3));

    // Create no per-call model_call records and no last-day daily total of 1000.
    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 22),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(summary.totals.event_count, 0);
    assert_eq!(summary.totals.call_count, 0);
    assert_eq!(summary.totals.total_tokens_known, None);
}

/// Example 11: Hermes primary-model cumulative 100, separate auxiliary-task 20 and sessions primary-loop 100
/// sum to 120 for verified exclusive model/task coverage; exclude the duplicate session total instead of summing 220.
#[test]
fn sample11_disjoint_coverage_sums_to_120() {
    let (_dir, storage) = temp_storage("sample11");
    let t0 = ts("2026-09-20T00:00:00Z");
    let t1 = ts("2026-09-24T00:00:00Z");
    let mk = |key: &str, total: i64, coverage: Coverage, dup: Option<&str>| SourceAggregateInput {
        instance_id: "hermes".into(),
        scope: AggregateScope::ModelSeries,
        scope_key: key.into(),
        interval_start_ms: Some(t0),
        interval_end_ms: t1,
        interval_end_inclusive: false,
        usage: TokenUsage {
            total_tokens: Some(total),
            ..TokenUsage::default()
        },
        quality: TokenQuality {
            total_tokens: FieldQuality::Reported,
            ..TokenQuality::default()
        },
        reported_call_count: None,
        coverage,
        duplicate_of: dup.map(str::to_string),
        time_basis: TimeBasis::Uncertain,
        source_revision: None,
    };
    upsert_source_aggregate(
        &storage,
        &mk("model-main", 100, Coverage::Exclusive, None),
        t1,
    )
    .unwrap();
    upsert_source_aggregate(&storage, &mk("task-aux", 20, Coverage::Exclusive, None), t1).unwrap();
    // sessions primary-loop duplicates model-main coverage and remains a comparison row.
    upsert_source_aggregate(
        &storage,
        &mk(
            "sessions-main-loop",
            100,
            Coverage::Duplicate,
            Some("model-main"),
        ),
        t1,
    )
    .unwrap();

    let totals = sum_exclusive_aggregates(&storage, "hermes").unwrap();
    assert_eq!(totals.total_tokens, Some(120));
    assert_ne!(totals.total_tokens, Some(220));
    assert_eq!(totals.exclusive_rows, 2);
    assert_eq!(totals.duplicate_rows, 1);
}
