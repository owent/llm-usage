//! Regression cases from the M0/M1 review. Expected values follow the data contract.
mod common;

use common::{batch, evt, temp_storage, ts, with_tokens, TempDir};
use llm_usage_core::adapters::usage_map::*;
use llm_usage_core::aggregates::{
    observe_cumulative, sum_exclusive_aggregates, upsert_source_aggregate, AggregateScope,
    Coverage, CumulativeOutcome, SourceAggregateInput,
};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::*;
use llm_usage_core::identity::event_id;
use llm_usage_core::ingest::{commit_batch, CheckpointUpdate};
use llm_usage_core::jobs::{finish_run, start_run, RunStats, RunStatus, TriggerKind};
use llm_usage_core::metrics;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::retention::{enforce_retention, RetentionPolicy};
use llm_usage_core::storage::Storage;

fn now() -> i64 {
    ts("2026-09-24T12:00:00Z")
}

fn request() -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 9, 24),
        last_day: ymd(2026, 9, 24),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: ymd(2026, 9, 24),
        retention_cutoff: None,
    }
}

#[test]
fn missing_mapping_fields_are_not_assumed_zero() {
    let codex = map_codex(&CodexUsage {
        input_tokens: 100,
        cached_input_tokens: 80,
        output_tokens: 10,
        reasoning_output_tokens: 0,
        total_tokens: 110,
        declares_no_cache_creation: false,
    });
    assert_eq!(codex.usage.input_uncached, None);
    assert_eq!(codex.quality.input_uncached, FieldQuality::Unknown);
    let zcode = map_zcode_ai_sdk(&ZcodeAiSdkUsage {
        input_tokens: 100,
        cached_input_tokens: None,
        cache_creation_input_tokens: None,
        output_tokens: 10,
        reasoning_tokens: None,
        total_tokens: None,
    });
    assert_eq!(zcode.usage.input_uncached, None);
    assert_eq!(zcode.usage.total_tokens, Some(110));
    let kilo = map_kilo(&KiloUsage {
        input: 100,
        output: 10,
        reasoning: None,
        cache_read: 0,
        cache_write: 0,
        total: 130,
    });
    assert_eq!(kilo.usage.output_total, None);
    assert_eq!(kilo.quality.output_total, FieldQuality::Unknown);
    assert_eq!(kilo.usage.total_tokens, Some(130));
}

#[test]
fn mapping_cache_sum_does_not_panic_on_overflow() {
    let result = map_zcode_ai_sdk(&ZcodeAiSdkUsage {
        input_tokens: 100,
        cached_input_tokens: Some(i64::MAX),
        cache_creation_input_tokens: Some(1),
        output_tokens: 10,
        reasoning_tokens: None,
        total_tokens: None,
    });
    assert_eq!(result.usage.input_uncached, None);
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn source_total_remains_distinct_from_normalized_total() {
    let result = map_codex(&CodexUsage {
        input_tokens: 100,
        cached_input_tokens: 0,
        output_tokens: 10,
        reasoning_output_tokens: 0,
        total_tokens: 999,
        declares_no_cache_creation: true,
    });
    assert_eq!(result.usage.total_tokens, Some(110));
    assert_eq!(result.usage.source_total, Some(999));
    assert_eq!(result.quality.total_tokens, FieldQuality::Derived);
}

#[test]
fn derived_totals_preserve_estimated_quality() {
    let mut e = with_tokens(evt("inst", "e", now()), 100, 10);
    e.quality.input_total = FieldQuality::Estimated;
    assert_eq!(
        metrics::total_tokens(&e.usage, &e.quality),
        Some((110, FieldQuality::Estimated))
    );
}

#[test]
fn default_summary_excludes_estimates_from_known_tokens_but_counts_calls() {
    let (_dir, storage) = temp_storage("review-estimates");
    let mut estimate = with_tokens(evt("inst", "estimate", now()), 900, 10);
    estimate.quality.input_total = FieldQuality::Estimated;
    estimate.quality.total_tokens = FieldQuality::Estimated;
    let known = with_tokens(evt("inst", "known", now()), 100, 10);
    commit_batch(
        &storage,
        &batch("inst", "UTC", now(), vec![known, estimate]),
        None,
    )
    .unwrap();
    let s = query_summary(&storage, &request()).unwrap();
    assert_eq!(s.totals.input_total_known, Some(100));
    assert_eq!(s.totals.output_total_known, Some(20));
    assert_eq!(s.totals.total_tokens_known, Some(110));
    assert_eq!(s.totals.input_known_count, 1);
    assert_eq!(s.totals.input_unknown_count, 1);
    assert_eq!(s.totals.call_count, 2);
}

#[test]
fn summary_detail_metrics_use_the_selected_days_and_include_sessionless_activity() {
    let (_dir, storage) = temp_storage("review-range");
    let mut before = with_tokens(evt("inst", "before", ts("2026-09-21T12:00:00Z")), 50, 0);
    before.session_id = Some("outside".into());
    let inside = with_tokens(evt("inst", "inside", now()), 10, 0);
    commit_batch(
        &storage,
        &batch("inst", "UTC", now(), vec![before, inside]),
        None,
    )
    .unwrap();
    for granularity in [Granularity::Day, Granularity::Week, Granularity::Month] {
        let mut r = request();
        r.granularity = granularity;
        let s = query_summary(&storage, &r).unwrap();
        assert_eq!(s.totals.input_total_known, Some(10));
        assert_eq!(s.periods[0].distinct_sessions, Some(0));
        assert_eq!(s.periods[0].active_days, Some(1));
    }
}

#[test]
fn summary_filters_apply_equally_to_details_and_excluded_counts() {
    let (_dir, storage) = temp_storage("review-filters");
    let mut known = with_tokens(evt("inst", "known", now()), 10, 0);
    known.agent = "unknown".into();
    known.model_raw = Some(String::new());
    known.session_id = Some("s".into());
    let mut excluded = known.clone();
    excluded.source_record_key = "excluded".into();
    excluded.agent = "other".into();
    excluded.attribution_status = AttributionStatus::Excluded;
    commit_batch(
        &storage,
        &batch("inst", "UTC", now(), vec![known, excluded]),
        None,
    )
    .unwrap();
    let mut r = request();
    r.filters.agents = vec!["unknown".into()];
    r.filters.models = vec!["unknown".into()];
    let s = query_summary(&storage, &r).unwrap();
    assert_eq!(s.totals.call_count, 1);
    assert_eq!(s.periods[0].distinct_sessions, Some(1));
    assert_eq!(s.excluded_event_count, 0);
}

#[test]
fn aggregate_kinds_cannot_enter_event_daily_totals() {
    for kind in [
        RecordKind::CumulativeSnapshot,
        RecordKind::IntervalAggregate,
        RecordKind::QuotaSnapshot,
    ] {
        let (_dir, storage) = temp_storage("review-kinds");
        let mut e = with_tokens(evt("inst", "e", now()), 100, 0);
        e.record_kind = kind;
        let out = commit_batch(&storage, &batch("inst", "UTC", now(), vec![e]), None).unwrap();
        assert_eq!(out.errors, 1);
        assert_eq!(out.added, 0);
    }
}

#[test]
fn batch_source_and_job_must_match_before_committing() {
    let (_dir, storage) = temp_storage("review-batch");
    let mut b = batch(
        "inst",
        "UTC",
        now(),
        vec![with_tokens(evt("other", "e", now()), 1, 0)],
    );
    b.checkpoints.push(CheckpointUpdate {
        scope_key: "file".into(),
        cursor_value: Some(10.into()),
        parse_context: None,
        source_revision: None,
    });
    assert!(commit_batch(&storage, &b, None).is_err());
    assert_eq!(storage.data_revision().unwrap(), 0);
    b.events[0].source_instance_id = "inst".into();
    b.run_id = Some("missing".into());
    assert!(commit_batch(&storage, &b, None).is_err());
    start_run(&storage, "other-run", "other", TriggerKind::Manual, now()).unwrap();
    b.run_id = Some("other-run".into());
    assert!(commit_batch(&storage, &b, None).is_err());
    start_run(&storage, "done", "inst", TriggerKind::Manual, now()).unwrap();
    finish_run(
        &storage,
        "done",
        RunStatus::Succeeded,
        RunStats::default(),
        None,
        now(),
    )
    .unwrap();
    b.run_id = Some("done".into());
    assert!(commit_batch(&storage, &b, None).is_err());
    assert_eq!(storage.data_revision().unwrap(), 0);
}

#[test]
fn identity_components_cannot_collide_at_separator() {
    assert_ne!(event_id("a#b", "c"), event_id("a", "b#c"));
    let (_dir, storage) = temp_storage("review-identity");
    for (instance, key) in [("a#b", "c"), ("a", "b#c")] {
        commit_batch(
            &storage,
            &batch(
                instance,
                "UTC",
                now(),
                vec![with_tokens(evt(instance, key, now()), 1, 0)],
            ),
            None,
        )
        .unwrap();
    }
    assert_eq!(
        query_summary(&storage, &request())
            .unwrap()
            .totals
            .call_count,
        2
    );
}

#[test]
fn rescanning_at_a_new_observation_time_is_not_a_conflict() {
    let (_dir, storage) = temp_storage("review-rescan");
    let mut e = with_tokens(evt("inst", "e", now()), 10, 0);
    e.observed_at_ms = Some(now());
    commit_batch(
        &storage,
        &batch("inst", "UTC", now(), vec![e.clone()]),
        None,
    )
    .unwrap();
    e.observed_at_ms = Some(now() + 1000);
    let out = commit_batch(&storage, &batch("inst", "UTC", now() + 1000, vec![e]), None).unwrap();
    assert_eq!(out.unchanged, 1);
    assert_eq!(out.conflicts, 0);
}

fn policy() -> RetentionPolicy {
    RetentionPolicy {
        detail_days: 7,
        diagnostics_days: 30,
        hard_max_days: Some(7),
    }
}

#[test]
fn retention_removes_aliases_before_expired_events() {
    let (_dir, storage) = temp_storage("review-alias-cleanup");
    let old = ts("2026-09-15T12:00:00Z");
    commit_batch(
        &storage,
        &batch(
            "inst",
            "UTC",
            now(),
            vec![evt("inst", "old", old), evt("inst", "new", now())],
        ),
        None,
    )
    .unwrap();
    storage
        .conn()
        .execute(
            "INSERT INTO event_aliases VALUES (?1, ?2, 'test', ?3)",
            rusqlite::params![event_id("inst", "old"), event_id("inst", "new"), now()],
        )
        .unwrap();
    let out = enforce_retention(&storage, "UTC", now(), &policy()).unwrap();
    assert_eq!(out.deleted_events, 1);
    let aliases: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM event_aliases", [], |r| r.get(0))
        .unwrap();
    assert_eq!(aliases, 0);
}

#[test]
fn retention_cutoff_survives_restart_and_missing_batch_cutoff() {
    let (dir, storage) = temp_storage("review-retention-replay");
    enforce_retention(&storage, "UTC", now(), &policy()).unwrap();
    drop(storage);
    let storage = Storage::open(&dir.db_path()).unwrap();
    let old = ts("2026-09-15T12:00:00Z");
    let out = commit_batch(
        &storage,
        &batch(
            "inst",
            "UTC",
            now(),
            vec![with_tokens(evt("inst", "old", old), 999, 0)],
        ),
        None,
    )
    .unwrap();
    assert_eq!(out.skipped, 1);
    assert_eq!(out.added, 0);
}

#[test]
fn hard_retention_also_limits_diagnostics() {
    let (_dir, storage) = temp_storage("review-hard-diag");
    storage
        .conn()
        .execute(
            "INSERT INTO diagnostics (code, message, created_ms) VALUES ('old', 'old', ?1)",
            [ts("2026-09-15T12:00:00Z")],
        )
        .unwrap();
    assert_eq!(
        enforce_retention(&storage, "UTC", now(), &policy())
            .unwrap()
            .deleted_diagnostics,
        1
    );
}

#[test]
fn cumulative_reset_applies_even_when_new_counter_is_higher() {
    let (prev, _) = observe_cumulative("series", None, 100, now(), false);
    let (_, result) = observe_cumulative("series", Some(&prev), 150, now() + 1000, true);
    assert_eq!(result, CumulativeOutcome::Reset { new_baseline: 150 });
}

#[test]
fn cumulative_late_observation_does_not_move_baseline() {
    let (prev, _) = observe_cumulative("series", None, 100, now(), false);
    let (next, _) = observe_cumulative("series", Some(&prev), 150, now() - 1000, false);
    assert_eq!(next, prev);
    let (_, outcome) = observe_cumulative("series", Some(&next), 200, now() + 1000, false);
    assert_eq!(outcome, CumulativeOutcome::Delta { amount: 100 });
}

#[test]
fn newer_schema_is_rejected_without_changing_journal_mode() {
    let dir = TempDir::new("review-new-schema");
    {
        let conn = rusqlite::Connection::open(dir.db_path()).unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
    }
    assert!(Storage::open(&dir.db_path()).is_err());
    let conn = rusqlite::Connection::open(dir.db_path()).unwrap();
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
}

fn aggregate() -> SourceAggregateInput {
    SourceAggregateInput {
        instance_id: "inst".into(),
        scope: AggregateScope::Session,
        scope_key: "s".into(),
        interval_start_ms: Some(now() - 1000),
        interval_end_ms: now(),
        interval_end_inclusive: false,
        usage: TokenUsage {
            input_total: Some(100),
            ..TokenUsage::default()
        },
        quality: TokenQuality {
            input_total: FieldQuality::Reported,
            ..TokenQuality::default()
        },
        reported_call_count: None,
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::SourceCompletion,
        source_revision: Some(1),
    }
}

#[test]
fn aggregate_equal_revision_conflict_keeps_the_authoritative_value() {
    let (_dir, storage) = temp_storage("review-aggregate-conflict");
    let mut a = aggregate();
    upsert_source_aggregate(&storage, &a, now()).unwrap();
    a.usage.input_total = Some(900);
    assert!(!upsert_source_aggregate(&storage, &a, now()).unwrap());
    assert_eq!(
        sum_exclusive_aggregates(&storage, "inst")
            .unwrap()
            .input_total,
        Some(100)
    );
    let count: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'aggregate_conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    a.source_revision = Some(2);
    assert!(upsert_source_aggregate(&storage, &a, now()).unwrap());
    assert_eq!(
        sum_exclusive_aggregates(&storage, "inst")
            .unwrap()
            .input_total,
        Some(900)
    );
}

#[test]
fn aggregate_changes_advance_revision_and_failures_roll_back_diagnostics() {
    let (_dir, storage) = temp_storage("review-aggregate-atomic");
    let before = storage.data_revision().unwrap();
    let mut a = aggregate();
    upsert_source_aggregate(&storage, &a, now()).unwrap();
    assert!(storage.data_revision().unwrap() > before);
    storage.conn().execute_batch("CREATE TRIGGER reject_aggregate BEFORE UPDATE ON source_aggregates BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    a.source_revision = Some(2);
    a.usage.input_cache_read = Some(900);
    a.quality.input_cache_read = FieldQuality::Reported;
    assert!(upsert_source_aggregate(&storage, &a, now()).is_err());
    let diagnostics: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diagnostics, 0);
}

#[test]
fn upgrade_rebuilds_known_usage_and_preserves_aliases_and_replay() {
    let dir = TempDir::new("review-upgrade");
    let storage = Storage::open_with(
        &dir.db_path(),
        llm_usage_core::storage::OpenOptions {
            max_supported_version: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let mut e = with_tokens(evt("a#b", "c", now()), 100, 10);
    e.observed_at_ms = Some(now());
    e.quality.input_total = FieldQuality::Estimated;
    e.quality.total_tokens = FieldQuality::Estimated;
    commit_batch(&storage, &batch("a#b", "UTC", now(), vec![e.clone()]), None).unwrap();
    // 模拟 v1 持久化格式，包含旧摘要与曾错误计入的估算值。
    storage
        .conn()
        .execute(
            "UPDATE usage_events SET event_id = 'a#b#c', content_hash = ?1",
            [llm_usage_core::identity::content_hash(&e)],
        )
        .unwrap();
    storage.conn().execute_batch("INSERT INTO event_aliases VALUES ('a#b#c', 'a#b#c', 'test', 1); UPDATE daily_usage SET input_known_sum = 100, input_known_count = 1, input_unknown_count = 0;").unwrap();
    drop(storage);
    let storage = Storage::open(&dir.db_path()).unwrap();
    let id: String = storage
        .conn()
        .query_row("SELECT canonical_event_id FROM event_aliases", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(id, event_id("a#b", "c"));
    let s = query_summary(&storage, &request()).unwrap();
    assert_eq!(s.totals.input_total_known, None);
    assert_eq!(s.totals.output_total_known, Some(10));
    e.observed_at_ms = Some(now() + 1000);
    assert_eq!(
        commit_batch(&storage, &batch("a#b", "UTC", now(), vec![e]), None)
            .unwrap()
            .unchanged,
        1
    );
    commit_batch(
        &storage,
        &batch("a", "UTC", now(), vec![evt("a", "b#c", now())]),
        None,
    )
    .unwrap();
}

#[test]
fn query_revision_and_counts_remain_consistent_during_writes() {
    let (dir, reader) = temp_storage("review-snapshot");
    let writer = Storage::open(&dir.db_path()).unwrap();
    let thread = std::thread::spawn(move || {
        for n in 0..60 {
            commit_batch(
                &writer,
                &batch(
                    "inst",
                    "UTC",
                    now(),
                    vec![with_tokens(evt("inst", &n.to_string(), now()), 1, 0)],
                ),
                None,
            )
            .unwrap();
        }
    });
    for _ in 0..100 {
        let s = query_summary(&reader, &request()).unwrap();
        assert_eq!(s.totals.call_count, s.data_revision);
        assert_eq!(s.totals.input_total_known.unwrap_or(0), s.data_revision);
    }
    thread.join().unwrap();
    assert_eq!(
        query_summary(&reader, &request())
            .unwrap()
            .totals
            .call_count,
        60
    );
}

#[test]
fn aggregate_unknown_calls_and_estimates_are_not_reported_as_known_zero_or_usage() {
    let (_dir, storage) = temp_storage("review-aggregate-unknown");
    let mut a = aggregate();
    a.quality.input_total = FieldQuality::Estimated;
    upsert_source_aggregate(&storage, &a, now()).unwrap();
    let total = sum_exclusive_aggregates(&storage, "inst").unwrap();
    assert_eq!(total.input_total, None);
    assert_eq!(total.reported_call_count, None);
    assert_eq!(total.call_count_unknown_rows, 1);
    a.scope_key = "known-zero".into();
    a.reported_call_count = Some(0);
    upsert_source_aggregate(&storage, &a, now()).unwrap();
    let total = sum_exclusive_aggregates(&storage, "inst").unwrap();
    assert_eq!(total.reported_call_count, Some(0));
    assert_eq!(total.call_count_known_rows, 1);
    assert_eq!(total.call_count_unknown_rows, 1);
}

#[test]
fn ratio_accumulation_does_not_truncate_large_integer_sums() {
    let (ratio, samples) =
        metrics::cache_input_ratio(&[(Some(MAX_TOKEN_VALUE), Some(MAX_TOKEN_VALUE)); 2]);
    assert_eq!(samples, 2);
    let ratio = ratio.unwrap();
    assert_eq!(ratio.numerator, 1_i128 << 63);
    assert_eq!(ratio.denominator, 1_i128 << 63);
    assert_eq!(ratio.as_f64(), 1.0);
}

#[test]
fn hard_retention_removes_native_aggregates_whose_intervals_include_expired_data() {
    let (_dir, storage) = temp_storage("review-hard-aggregate");
    let mut a = aggregate();
    a.interval_start_ms = Some(ts("2026-09-15T12:00:00Z"));
    upsert_source_aggregate(&storage, &a, now()).unwrap();
    enforce_retention(&storage, "UTC", now(), &policy()).unwrap();
    assert_eq!(
        sum_exclusive_aggregates(&storage, "inst")
            .unwrap()
            .exclusive_rows,
        0
    );
    assert!(!upsert_source_aggregate(&storage, &a, now()).unwrap());
    assert_eq!(
        sum_exclusive_aggregates(&storage, "inst")
            .unwrap()
            .exclusive_rows,
        0
    );
}

#[test]
fn inconsistent_quality_is_rejected_instead_of_counted_as_known_usage() {
    let (_dir, storage) = temp_storage("review-quality-validation");
    let mut e = evt("inst", "e", now());
    e.usage.input_total = Some(100);
    let result = commit_batch(&storage, &batch("inst", "UTC", now(), vec![e]), None).unwrap();
    assert_eq!((result.errors, result.added), (1, 0));
    let mut a = aggregate();
    a.quality.input_total = FieldQuality::Unknown;
    assert!(upsert_source_aggregate(&storage, &a, now()).is_err());
    a.usage.input_total = None;
    a.quality.input_total = FieldQuality::Reported;
    assert!(upsert_source_aggregate(&storage, &a, now()).is_err());
}

#[test]
fn upgrade_failure_rolls_back_identity_aliases_and_schema_together() {
    let dir = TempDir::new("review-upgrade-rollback");
    let storage = Storage::open_with(
        &dir.db_path(),
        llm_usage_core::storage::OpenOptions {
            max_supported_version: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    commit_batch(
        &storage,
        &batch(
            "a#b",
            "UTC",
            now(),
            vec![with_tokens(evt("a#b", "c", now()), 10, 0)],
        ),
        None,
    )
    .unwrap();
    storage.conn().execute_batch("UPDATE usage_events SET event_id = 'a#b#c';
        INSERT INTO event_aliases VALUES ('a#b#c', 'a#b#c', 'test', 1);
        CREATE TRIGGER block_rebuild BEFORE DELETE ON daily_usage BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    drop(storage);
    assert!(matches!(
        Storage::open(&dir.db_path()),
        Err(llm_usage_core::CoreError::MigrationFailed { version: 2, .. })
    ));
    let conn = rusqlite::Connection::open(dir.db_path()).unwrap();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1);
    let ids: (String, String) = conn
        .query_row(
            "SELECT event_id, canonical_event_id FROM usage_events, event_aliases",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ids, ("a#b#c".into(), "a#b#c".into()));
    let total: i64 = conn
        .query_row("SELECT input_known_sum FROM daily_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, 10);
}

#[test]
fn upgrade_marks_sealed_estimates_unavailable_without_inventing_known_values() {
    let dir = TempDir::new("review-upgrade-sealed");
    let storage = Storage::open_with(
        &dir.db_path(),
        llm_usage_core::storage::OpenOptions {
            max_supported_version: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let mut e = with_tokens(evt("inst", "e", ts("2026-09-15T12:00:00Z")), 100, 10);
    e.quality.input_total = FieldQuality::Estimated;
    e.quality.total_tokens = FieldQuality::Estimated;
    commit_batch(&storage, &batch("inst", "UTC", now(), vec![e]), None).unwrap();
    enforce_retention(
        &storage,
        "UTC",
        now(),
        &RetentionPolicy {
            hard_max_days: None,
            ..policy()
        },
    )
    .unwrap();
    storage.conn().execute_batch("UPDATE daily_usage SET input_known_sum = 100, input_known_count = 1, input_unknown_count = 0;").unwrap();
    drop(storage);
    let storage = Storage::open(&dir.db_path()).unwrap();
    let mut r = request();
    r.first_day = ymd(2026, 9, 15);
    r.last_day = r.first_day;
    let s = query_summary(&storage, &r).unwrap();
    assert_eq!(s.totals.input_total_known, None);
    assert_eq!(s.totals.input_unknown_count, 1);
    assert_eq!(s.totals.call_count, 1);
    assert!(s.periods[0].partial_history);
    let diagnostics: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'sealed_estimate_unavailable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diagnostics, 1);
}
