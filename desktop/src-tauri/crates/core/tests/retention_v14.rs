//! V14 retention cutoff, archived summaries, expired-data exclusion, no duplicate archive accumulation and hard maximum retention.
//! Cutoff is today minus D−1 local days in the selected timezone; retain today and the previous D−1 days.
//! Tests follow the configured 7–3650-day detail-retention range.

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::retention::{enforce_retention, RetentionPolicy, SEAL_FIELD_VERSION};

const TODAY: &str = "2026-09-24T12:00:00Z";

/// Ten days, 09-15 through 09-24: one event per day, token value equal to the day number.
fn seed_ten_days(storage: &llm_usage_core::storage::Storage) {
    let events: Vec<_> = (15..=24)
        .map(|d| {
            with_tokens(
                evt(
                    "inst",
                    &format!("d{d}"),
                    ts(&format!("2026-09-{d}T10:00:00Z")),
                ),
                i64::from(d),
                0,
            )
        })
        .collect();
    commit_batch(storage, &batch("inst", "UTC", ts(TODAY), events), None).unwrap();
}

fn daily_rows(storage: &llm_usage_core::storage::Storage) -> Vec<(String, i64, i64)> {
    let mut stmt = storage
        .conn()
        .prepare("SELECT local_day, input_known_sum, sealed FROM daily_usage ORDER BY local_day")
        .unwrap();
    stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

#[test]
fn v14_retention_cutoff_seals_expired_days() {
    let (_dir, storage) = temp_storage("v14seal");
    seed_ten_days(&storage);

    let policy = RetentionPolicy {
        detail_days: 7,
        diagnostics_days: 7,
        hard_max_days: None,
    };
    let outcome = enforce_retention(&storage, "UTC", ts(TODAY), &policy).unwrap();

    // Today 09-24 minus six days is 09-18; 09-18 through 09-24 retains exactly seven local days.
    assert_eq!(outcome.cutoff_day, ymd(2026, 9, 18));
    assert_eq!(
        outcome.sealed_days,
        vec![
            "2026-09-15".to_string(),
            "2026-09-16".to_string(),
            "2026-09-17".to_string()
        ]
    );
    assert_eq!(outcome.deleted_events, 3);

    // Retain exactly seven days of details.
    let remaining: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 7);

    // Archive expired-day summaries with timezone, field/source-selection versions and their original values.
    let rows = daily_rows(&storage);
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[0], ("2026-09-15".to_string(), 15, 1));
    assert_eq!(rows[2], ("2026-09-17".to_string(), 17, 1));
    assert_eq!(rows[3], ("2026-09-18".to_string(), 18, 0));
    assert_eq!(rows[9], ("2026-09-24".to_string(), 24, 0));
    let (seal_tz, seal_field, seal_source): (String, String, String) = storage
        .conn()
        .query_row(
            "SELECT seal_tz, seal_field_version, seal_source_version FROM daily_usage WHERE local_day = '2026-09-15'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(seal_tz, "UTC");
    assert_eq!(seal_field, SEAL_FIELD_VERSION);
    assert!(seal_source.contains("inst"));

    // Archiving leaves a published aggregate-generation record.
    let generation: String = storage
        .conn()
        .query_row(
            "SELECT status FROM aggregate_generations WHERE kind = 'retention_seal' ORDER BY generation_id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(generation, "published");
}

#[test]
fn v14_expired_events_do_not_resurrect_and_sealed_day_not_appended() {
    let (_dir, storage) = temp_storage("v14norevive");
    seed_ten_days(&storage);
    let policy = RetentionPolicy {
        detail_days: 7,
        diagnostics_days: 7,
        hard_max_days: None,
    };
    let outcome = enforce_retention(&storage, "UTC", ts(TODAY), &policy).unwrap();

    // A rescan with the cutoff excludes expired events and records diagnostics while leaving source logs intact.
    let mut late = batch(
        "inst",
        "UTC",
        ts("2026-09-24T13:00:00Z"),
        vec![with_tokens(
            evt("inst", "d15-again", ts("2026-09-15T11:00:00Z")),
            999,
            0,
        )],
    );
    late.retention_cutoff_ms = Some(outcome.cutoff_ms);
    let out = commit_batch(&storage, &late, None).unwrap();
    assert_eq!(out.skipped, 1);
    assert_eq!(out.added, 0);
    let diag: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'expired_by_retention'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);

    // Even without a supplied cutoff, archived days cannot be recomputed and appended.
    let unguarded = batch(
        "inst",
        "UTC",
        ts("2026-09-24T13:30:00Z"),
        vec![with_tokens(
            evt("inst", "d15-unguarded", ts("2026-09-15T12:00:00Z")),
            999,
            0,
        )],
    );
    commit_batch(&storage, &unguarded, None).unwrap();
    let rows = daily_rows(&storage);
    assert_eq!(rows[0], ("2026-09-15".to_string(), 15, 1));

    // Queries still include archived rows, preserving the total.
    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 15),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: Some(outcome.cutoff_day),
        },
    )
    .unwrap();
    assert_eq!(
        summary.totals.input_total_known,
        Some((15..=24).sum::<i64>())
    );
    // Deleted archived details leave distinct sessions unknown and mark partial history.
    let sealed_period = summary
        .periods
        .iter()
        .find(|p| p.label == "2026-09-15")
        .unwrap();
    assert_eq!(sealed_period.distinct_sessions, None);
    assert!(sealed_period.partial_history);
    let fresh_period = summary
        .periods
        .iter()
        .find(|p| p.label == "2026-09-24")
        .unwrap();
    assert_eq!(
        fresh_period.distinct_sessions, None,
        "fixture has no session identities"
    );
    assert!(!fresh_period.partial_history);
}

#[test]
fn v14_hard_max_retention_drops_old_daily_and_quota() {
    let (_dir, storage) = temp_storage("v14hard");
    seed_ten_days(&storage);
    llm_usage_core::aggregates::insert_quota_snapshot(
        &storage,
        &llm_usage_core::aggregates::QuotaSnapshotInput {
            quota_id: "q1".into(),
            instance_id: "inst".into(),
            observed_at_ms: ts("2026-09-15T10:00:00Z"),
            kind: "credits".into(),
            quantity_minor: Some(500),
            unit: "credit".into(),
            window_start_ms: None,
            window_end_ms: None,
            locality_verified: true,
            detail: None,
        },
    )
    .unwrap();

    // After archiving with seven detail days, an eight-day hard maximum also deletes 09-15/16 summaries.
    let seal_policy = RetentionPolicy {
        detail_days: 7,
        diagnostics_days: 7,
        hard_max_days: None,
    };
    enforce_retention(&storage, "UTC", ts(TODAY), &seal_policy).unwrap();
    let hard_policy = RetentionPolicy {
        detail_days: 7,
        diagnostics_days: 7,
        hard_max_days: Some(8),
    };
    let outcome = enforce_retention(&storage, "UTC", ts(TODAY), &hard_policy).unwrap();

    assert_eq!(outcome.deleted_daily_rows, 2);
    assert_eq!(outcome.deleted_quota_rows, 1);
    let days: Vec<String> = daily_rows(&storage).into_iter().map(|r| r.0).collect();
    assert_eq!(days.len(), 8);
    assert_eq!(days[0], "2026-09-17");
    assert_eq!(days[7], "2026-09-24");
}

#[test]
fn v14_diagnostics_retention() {
    let (_dir, storage) = temp_storage("v14diag");
    let old_ms = ts("2026-09-10T00:00:00Z");
    let new_ms = ts("2026-09-24T00:00:00Z");
    for (code, ms) in [("old", old_ms), ("new", new_ms)] {
        storage
            .conn()
            .execute(
                "INSERT INTO diagnostics (batch_id, run_id, instance_id, event_id, code, field, position, message, created_ms)
                 VALUES (NULL, NULL, 'inst', NULL, ?1, NULL, NULL, 'm', ?2)",
                rusqlite::params![code, ms],
            )
            .unwrap();
    }
    let policy = RetentionPolicy {
        detail_days: 7,
        diagnostics_days: 7,
        hard_max_days: None,
    };
    let outcome = enforce_retention(&storage, "UTC", ts(TODAY), &policy).unwrap();
    assert_eq!(outcome.deleted_diagnostics, 1);
    let remaining: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 1);
}

#[test]
fn v14_capacity_footprint_counts_main_wal_and_backups() {
    let (dir, storage) = temp_storage("v14cap");
    seed_ten_days(&storage);
    let backup_dir = dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).unwrap();
    std::fs::write(backup_dir.join("backup-1.db"), b"x".repeat(1234)).unwrap();

    let fp =
        llm_usage_core::retention::storage_footprint(&dir.db_path(), Some(&backup_dir)).unwrap();
    assert!(fp.main_db_bytes > 0);
    assert_eq!(fp.backup_bytes, 1234);
    assert_eq!(
        fp.total_bytes,
        fp.main_db_bytes + fp.wal_bytes + fp.shm_bytes + fp.backup_bytes
    );
}
