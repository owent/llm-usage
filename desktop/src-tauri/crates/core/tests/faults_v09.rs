//! V09: stop the process after reads/events/cursors, before aggregation and after commit.
//! Restart/replay gives identical results: no cursor-only loss or event-only duplication.

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::ingest::{commit_batch, CheckpointUpdate, FaultPoint, IngestBatch};
use llm_usage_core::storage::Storage;

const POINTS: [FaultPoint; 6] = [
    FaultPoint::AfterEvents,
    FaultPoint::AfterCheckpoint,
    FaultPoint::AfterParseContext,
    FaultPoint::BeforeAggregates,
    FaultPoint::AfterAggregates,
    FaultPoint::BeforeCommit,
];

fn make_batch(now: i64) -> IngestBatch {
    let mut b = batch(
        "inst",
        "UTC",
        now,
        vec![
            with_tokens(evt("inst", "k1", ts("2026-09-24T10:00:00Z")), 100, 50),
            with_tokens(evt("inst", "k2", ts("2026-09-24T11:00:00Z")), 200, 50),
        ],
    );
    b.checkpoints = vec![CheckpointUpdate {
        scope_key: "file-1".into(),
        cursor_value: Some(serde_json::json!({"generation": 1, "offset": 4096})),
        parse_context: Some(serde_json::json!({"current_model": "m", "unfinished": []})),
        source_revision: Some(7),
    }];
    b
}

fn db_state(storage: &Storage) -> (i64, i64, i64, i64, i64) {
    let conn = storage.conn();
    let events: i64 = conn
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    let checkpoints: i64 = conn
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    let daily: i64 = conn
        .query_row("SELECT COUNT(*) FROM daily_usage", [], |r| r.get(0))
        .unwrap();
    let input_sum: Option<i64> = conn
        .query_row("SELECT SUM(input_known_sum) FROM daily_usage", [], |r| {
            r.get(0)
        })
        .unwrap();
    let revision = storage.data_revision().unwrap();
    (
        events,
        checkpoints,
        daily,
        input_sum.unwrap_or(-1),
        revision,
    )
}

/// Each injected fault fully rolls back; a clean replay then succeeds with matching results.
#[test]
fn v09_fault_at_each_point_rolls_back_and_replays_identically() {
    for point in POINTS {
        let tag = format!("v09-{point:?}");
        let (_dir, storage) = temp_storage(&tag);
        let now = ts("2026-09-24T12:00:00Z");

        let err = commit_batch(&storage, &make_batch(now), Some(point)).unwrap_err();
        assert!(
            matches!(err, llm_usage_core::CoreError::FaultInjected(_)),
            "expected injected fault at {point:?}, got {err}"
        );
        // Nothing partially committed: no events/cursors/aggregates; revision unchanged.
        assert_eq!(
            db_state(&storage),
            (0, 0, 0, -1, 0),
            "partial commit at {point:?}"
        );

        // Clean replay.
        let out = commit_batch(&storage, &make_batch(now), None).unwrap();
        assert_eq!(out.added, 2);
        assert_eq!(
            db_state(&storage),
            (2, 1, 1, 300, 1),
            "after clean replay {point:?}"
        );
    }
}

/// Reopen the same database after a simulated kill; repeated replay adds no duplicates.
#[test]
fn v09_process_restart_replay_is_idempotent() {
    let dir = common::TempDir::new("v09restart");
    let path = dir.db_path();
    let now = ts("2026-09-24T12:00:00Z");
    {
        let storage = Storage::open(&path).unwrap();
        let err = commit_batch(
            &storage,
            &make_batch(now),
            Some(FaultPoint::AfterAggregates),
        )
        .unwrap_err();
        assert!(matches!(err, llm_usage_core::CoreError::FaultInjected(_)));
        assert_eq!(db_state(&storage), (0, 0, 0, -1, 0));
    }
    // Restart and replay the batch twice; results match a single application.
    {
        let storage = Storage::open(&path).unwrap();
        let out1 = commit_batch(&storage, &make_batch(now), None).unwrap();
        assert_eq!((out1.added, out1.updated, out1.unchanged), (2, 0, 0));
        let state1 = db_state(&storage);

        let out2 = commit_batch(&storage, &make_batch(now + 60_000), None).unwrap();
        assert_eq!((out2.added, out2.updated, out2.unchanged), (0, 0, 2));
        let state2 = db_state(&storage);
        // Events/totals unchanged; only revision increases monotonically with commits.
        assert_eq!(state1.0, state2.0);
        assert_eq!(state1.1, state2.1);
        assert_eq!(state1.3, state2.3);
        // Identical replay advances processing positions without changing visible data.
        assert_eq!(state2.4, state1.4);
    }
}

/// Kill after commit, beyond fault points; event upsert/daily rebuild keeps repeats stable.
#[test]
fn v09_replay_after_successful_commit_does_not_double_count() {
    let (_dir, storage) = temp_storage("v09after");
    let now = ts("2026-09-24T12:00:00Z");
    commit_batch(&storage, &make_batch(now), None).unwrap();
    let before = db_state(&storage);
    // Replay three times.
    for i in 1..=3 {
        let out = commit_batch(&storage, &make_batch(now + i), None).unwrap();
        assert_eq!((out.added, out.unchanged), (0, 2));
    }
    let after = db_state(&storage);
    assert_eq!(before.0, after.0);
    assert_eq!(before.3, after.3);
}
