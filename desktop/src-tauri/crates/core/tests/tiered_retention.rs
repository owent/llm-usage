//! Tiered retention (2026-09-26 request): details, hourly, daily and week/month/year archives.
//! Store completed period rows before deleting daily rows; queries combine retained days with archives.
//! Hourly charts use persisted hourly rows after detail deletion.

mod common;

use common::*;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{heatmap_cells, query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy};

#[test]
fn cancellation_after_delete_rolls_back_events_aggregates_revision_and_floors() {
    use llm_usage_core::cancellation::OperationControl;
    use llm_usage_core::retention_tiered::enforce_tiered_retention_controlled;
    use std::sync::Arc;
    let (_dir, storage) = temp_storage("retention-cancel");
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-20T10:00:00Z"),
            vec![with_tokens(
                evt("codex@a", "one", ts("2026-09-20T10:00:00Z")),
                100,
                10,
            )],
        ),
        None,
    )
    .unwrap();
    let before = query_summary(
        &storage,
        &request("2026-09-01", "2026-09-26", Granularity::Day),
    )
    .unwrap();
    let control = Arc::new(OperationControl::default());
    assert!(control.begin());
    let cancelled = Arc::clone(&control);
    storage
        .conn()
        .create_scalar_function(
            "cancel_cleanup_test",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                cancelled.cancel();
                Ok(0)
            },
        )
        .unwrap();
    storage.conn().execute_batch("CREATE TEMP TRIGGER cancel_delete AFTER DELETE ON usage_events BEGIN SELECT cancel_cleanup_test(); END;").unwrap();
    let guard = control.install(storage.conn()).unwrap();
    let result = enforce_tiered_retention_controlled(
        &storage,
        "UTC",
        ts("2026-09-26T12:00:00Z"),
        &TieredRetentionPolicy {
            events_days: 1,
            ..TieredRetentionPolicy::default()
        },
        Some(&control),
    );
    assert!(result.is_err());
    assert!(control.is_cancelled());
    drop(guard);
    let after = query_summary(
        &storage,
        &request("2026-09-01", "2026-09-26", Granularity::Day),
    )
    .unwrap();
    assert_eq!(before.totals, after.totals);
    assert_eq!(before.data_revision, after.data_revision);
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM settings WHERE key='detail_retention_floor_ms' OR key LIKE 'retention_applied:%'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    control.finish();
}

fn request(first: &str, last: &str, granularity: Granularity) -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".to_string(),
        week_start: llm_usage_core::calendar::WeekStart::Monday,
        first_day: llm_usage_core::calendar::parse_date(first).unwrap(),
        last_day: llm_usage_core::calendar::parse_date(last).unwrap(),
        granularity,
        filters: Filters::default(),
        today: llm_usage_core::calendar::parse_date("2026-09-26").unwrap(),
        retention_cutoff: None,
    }
}

fn long_policy() -> TieredRetentionPolicy {
    TieredRetentionPolicy {
        events_days: 366,
        hourly_days: 366,
        daily_days: 366,
        weekly_days: 1095,
        monthly_days: 3650,
        yearly_days: None,
    }
}

fn archive_fixture(storage: &llm_usage_core::storage::Storage) -> i64 {
    // September is complete; its last day still belongs to the current ISO week.
    let now = ts("2026-10-03T12:00:00Z");
    for (source, key, date, input) in [
        ("a", "old-a", "2026-09-30T12:00:00Z", 100),
        ("b", "old-b", "2026-09-30T12:00:00Z", 200),
        ("a", "today-a", "2026-10-03T12:00:00Z", 10),
    ] {
        commit_batch(
            storage,
            &batch(
                source,
                "UTC",
                ts(date),
                vec![with_tokens(evt(source, key, ts(date)), input, input / 10)],
            ),
            None,
        )
        .unwrap();
    }
    assert_eq!(
        enforce_tiered_retention(storage, "UTC", now, &long_policy())
            .unwrap()
            .materialized_period_rows,
        2
    );
    now
}

#[test]
fn today_increment_keeps_completed_month_values_and_publication_unchanged() {
    let (_dir, storage) = temp_storage("retention-open-period");
    let now = archive_fixture(&storage);
    let snapshot = || {
        storage.conn().prepare("SELECT instance_id,call_count,total_known_sum,materialized_at_ms,data_revision FROM period_usage WHERE granularity='month' ORDER BY instance_id").unwrap()
        .query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<i64>>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?))).unwrap()
        .collect::<Result<Vec<_>,_>>().unwrap()
    };
    let before = snapshot();
    assert_eq!(before.iter().map(|row| row.2.unwrap()).sum::<i64>(), 330);
    commit_batch(
        &storage,
        &batch(
            "a",
            "UTC",
            now + 1000,
            vec![with_tokens(evt("a", "today-b", now + 1000), 20, 2)],
        ),
        None,
    )
    .unwrap();
    let outcome = enforce_tiered_retention(&storage, "UTC", now + 2000, &long_policy()).unwrap();
    assert_eq!(outcome.materialized_period_rows, 0);
    assert_eq!(snapshot(), before);
    assert_eq!(
        query_summary(
            &storage,
            &request("2026-10-03", "2026-10-03", Granularity::Day)
        )
        .unwrap()
        .totals
        .total_tokens_known,
        Some(33)
    );
    let changed_policy = TieredRetentionPolicy {
        weekly_days: 1096,
        ..long_policy()
    };
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", now + 3000, &changed_policy)
            .unwrap()
            .materialized_period_rows,
        2
    );
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", ts("2026-10-05T12:00:00Z"), &changed_policy)
            .unwrap()
            .materialized_period_rows,
        4
    );
    let closed_week: i64 = storage
        .conn()
        .query_row(
            "SELECT SUM(total_known_sum) FROM period_usage WHERE granularity='week'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(closed_week, 363);
}

#[test]
fn completed_month_rebuilds_for_late_data_deletions_missing_rows_and_unknown_watermarks() {
    let (_dir, storage) = temp_storage("retention-dirty-period");
    let now = archive_fixture(&storage);
    let old = ts("2026-09-30T12:00:00Z");
    let month_total = || {
        storage
            .conn()
            .query_row(
                "SELECT SUM(total_known_sum) FROM period_usage WHERE granularity='month'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    // Equal daily cardinality, higher revision, completed month in an open week.
    commit_batch(
        &storage,
        &batch(
            "a",
            "UTC",
            now + 1,
            vec![with_tokens(evt("a", "late", old), 300, 30)],
        ),
        None,
    )
    .unwrap();
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", now + 2, &long_policy())
            .unwrap()
            .materialized_period_rows,
        2
    );
    assert_eq!(month_total(), 660);
    storage
        .conn()
        .execute("DELETE FROM usage_events WHERE source_instance_id='b'", [])
        .unwrap();
    llm_usage_core::ingest::recompute_days_in_tz(&storage, "UTC", old, old, now + 3).unwrap();
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", now + 4, &long_policy())
            .unwrap()
            .materialized_period_rows,
        1
    );
    assert_eq!(month_total(), 440);
    let bump = || {
        let tx = storage.conn().unchecked_transaction().unwrap();
        llm_usage_core::storage::Storage::bump_data_revision_tx(&tx, now).unwrap();
        tx.commit().unwrap();
    };
    storage
        .conn()
        .execute("DELETE FROM period_usage WHERE granularity='month'", [])
        .unwrap();
    bump();
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", now + 5, &long_policy())
            .unwrap()
            .materialized_period_rows,
        1
    );
    assert_eq!(month_total(), 440);
    storage
        .conn()
        .execute(
            "UPDATE settings SET value='invalid' WHERE key='retention_materialization:UTC'",
            [],
        )
        .unwrap();
    bump();
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", now + 6, &long_policy())
            .unwrap()
            .materialized_period_rows,
        1
    );
    // The local processing marker cannot establish validity of imported future revisions.
    storage
        .conn()
        .execute(
            "UPDATE daily_usage SET data_revision=1000000 WHERE local_day='2026-09-30'",
            [],
        )
        .unwrap();
    bump();
    assert_eq!(
        enforce_tiered_retention(&storage, "UTC", now + 7, &long_policy())
            .unwrap()
            .materialized_period_rows,
        1
    );
    assert_eq!(month_total(), 440);
}

#[test]
fn tiered_retention_prunes_by_layer_and_materializes_history() {
    let (_dir, storage) = temp_storage("tiered");
    // Use completed 2025-12-15 week/month/year plus current-week 09-22/09-25.
    // Ongoing week/month/year starts are protected: today 2026-09-26 protects 2026-01-01 onward.
    // Use 2025 history to exercise daily deletion; see the module's protection rules.
    for (key, day, tokens) in [
        ("r-old", "2025-12-15T10:00:00Z", 100i64),
        ("r-w22", "2026-09-22T10:00:00Z", 100),
        ("r-w25", "2026-09-25T10:00:00Z", 10),
    ] {
        commit_batch(
            &storage,
            &batch(
                "codex@a",
                "UTC",
                ts(day),
                vec![with_tokens(
                    evt("codex@a", key, ts(day)),
                    tokens,
                    tokens / 10,
                )],
            ),
            None,
        )
        .unwrap();
    }
    // Retain details one day, hours two days, days three days, with longer week/month archives.
    let policy = TieredRetentionPolicy {
        events_days: 1,
        hourly_days: 2,
        daily_days: 3,
        weekly_days: 3650,
        monthly_days: 10_950,
        yearly_days: None,
    };
    let outcome =
        enforce_tiered_retention(&storage, "UTC", ts("2026-09-26T12:00:00Z"), &policy).unwrap();
    // All events precede the detail cutoff at today's start and are deleted.
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
    assert_eq!(outcome.deleted_events, 3);
    // Delete hours before 09-25: remove 2025-12-15/09-22, retain 09-25.
    let hourly: Vec<(String, i64)> = storage
        .conn()
        .prepare("SELECT local_day, call_count FROM hourly_usage ORDER BY local_day")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(hourly, vec![("2026-09-25".to_string(), 1)]);
    // Daily cutoff is 09-24; year protection from 2026-01-01 leaves only 2025-12-15 deleted.
    let daily: Vec<String> = storage
        .conn()
        .prepare("SELECT DISTINCT local_day FROM daily_usage ORDER BY local_day")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        daily,
        vec!["2026-09-22".to_string(), "2026-09-25".to_string()]
    );
    assert_eq!(outcome.deleted_daily_rows, 1);
    // Store one 2025 row per week/month/year (one call, 110 tokens). Ongoing current periods
    // use protected daily rows instead of stored period rows.
    assert_eq!(outcome.materialized_period_rows, 3, "2025 周+月+年各一行");
    for granularity in ["week", "month", "year"] {
        let row: (i64, Option<i64>) = storage
            .conn()
            .query_row(
                "SELECT call_count, total_known_sum FROM period_usage WHERE granularity = ?1",
                [granularity],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(row, (1, Some(110)), "{granularity} 行来自 2025-12-15");
    }
    // Cross-year weekly queries combine the archived 2025-12-15 week with daily 2026-09-21 week.
    let s = query_summary(
        &storage,
        &request("2025-12-01", "2026-09-30", Granularity::Week),
    )
    .unwrap();
    assert_eq!(s.periods.len(), 2);
    assert_eq!(s.totals.call_count, 3, "物化 1 + 日行 2");
    assert_eq!(s.totals.total_tokens_known, Some(110 + 110 + 11));
    let materialized_period = s
        .periods
        .iter()
        .find(|p| p.start_day.to_string().starts_with("2025-"))
        .expect("materialized week row visible");
    assert!(materialized_period.partial_history);
    // Repeated retention adds no usage; deleted 2025 days cannot be rematerialized, so archives stay.
    // Existing 2026 days belong to ongoing periods, giving materialized=0.
    let again =
        enforce_tiered_retention(&storage, "UTC", ts("2026-09-26T13:00:00Z"), &policy).unwrap();
    assert_eq!(again.materialized_period_rows, 0);
    let kept: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM period_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kept, 3, "物化行保留不丢");
    let s2 = query_summary(
        &storage,
        &request("2025-12-01", "2026-09-30", Granularity::Week),
    )
    .unwrap();
    assert_eq!(s2.totals.call_count, 3, "重复执行不增量");
    assert_eq!(s2.totals.total_tokens_known, Some(231));
    let daily_heatmap = heatmap_cells(
        &storage,
        "UTC",
        llm_usage_core::calendar::parse_date("2025-12-15").unwrap(),
        llm_usage_core::calendar::parse_date("2026-09-26").unwrap(),
        &Filters::default(),
    )
    .unwrap();
    assert!(!daily_heatmap[0].available, "周期归档不能假装还原出每日值");
    assert_eq!(daily_heatmap.iter().map(|c| c.call_count).sum::<i64>(), 2);
    storage
        .conn()
        .execute(
            "DELETE FROM settings WHERE key = 'daily_retention_floor:UTC'",
            [],
        )
        .unwrap();
    let imported_period_only = heatmap_cells(
        &storage,
        "UTC",
        llm_usage_core::calendar::parse_date("2025-12-15").unwrap(),
        llm_usage_core::calendar::parse_date("2025-12-15").unwrap(),
        &Filters {
            agents: vec!["AGENT-A".into()],
            models: vec!["M".into()],
            ..Filters::default()
        },
    )
    .unwrap();
    assert!(
        !imported_period_only[0].available,
        "缺少截止线的周期归档仍不能显示为零调用"
    );
}

#[test]
fn hourly_table_survives_detail_prune() {
    let (_dir, storage) = temp_storage("hourly-survive");
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-25T10:00:00Z"),
            vec![with_tokens(
                evt("codex@a", "r-h", ts("2026-09-25T10:00:00Z")),
                30,
                3,
            )],
        ),
        None,
    )
    .unwrap();
    let policy = TieredRetentionPolicy {
        events_days: 1,
        hourly_days: 30,
        ..TieredRetentionPolicy::default()
    };
    enforce_tiered_retention(&storage, "UTC", ts("2026-09-26T12:00:00Z"), &policy).unwrap();
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "明细已删");
    let buckets = llm_usage_core::query::hourly_breakdown(
        &storage,
        "UTC",
        llm_usage_core::calendar::parse_date("2026-09-25").unwrap(),
        &Filters::default(),
    )
    .unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].hour, 10);
    assert_eq!(buckets[0].call_count, 1);
    assert_eq!(buckets[0].total_tokens_known, Some(33));
}

#[test]
fn tiered_policy_validates_ordering() {
    // Details may outlive hourly rows; this redundancy loses no data.
    let redundant = TieredRetentionPolicy {
        events_days: 10,
        hourly_days: 5,
        ..TieredRetentionPolicy::default()
    };
    assert!(redundant.validate().is_ok());
    // Coarser summaries cannot have a shorter retention range than finer summaries.
    let bad = TieredRetentionPolicy {
        hourly_days: 90,
        daily_days: 30,
        ..TieredRetentionPolicy::default()
    };
    assert!(enforce_tiered_retention(&storage_none(), "UTC", 0, &bad).is_err());
}

fn storage_none() -> llm_usage_core::storage::Storage {
    // Use an in-memory database while testing rejection of an invalid policy.
    llm_usage_core::storage::Storage::open_in_memory().unwrap()
}

#[test]
fn debug_hourly_rows() {
    let (_dir, storage) = temp_storage("dbg-hourly");
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-25T10:00:00Z"),
            vec![with_tokens(
                evt("codex@a", "r-dbg", ts("2026-09-25T10:00:00Z")),
                30,
                3,
            )],
        ),
        None,
    )
    .unwrap();
    let rows: Vec<(String, i64, i64, Option<i64>)> = storage
        .conn()
        .prepare("SELECT local_day, hour, call_count, total_known_sum FROM hourly_usage")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    println!("hourly rows: {rows:?}");
}
