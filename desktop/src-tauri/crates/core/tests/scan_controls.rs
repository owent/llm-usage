mod common;
use common::*;
use llm_usage_core::{
    adapters::{
        jsonl::{self, JsonlLimits, StopReason},
        run_policy,
    },
    error::CoreError,
};
use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[test]
fn read_to_end_returns_typed_cancellation_and_nested_scope_restores_control() {
    let scope = run_policy::enter(Some(Instant::now()), None);
    let mut bytes = Vec::new();
    let err = run_policy::checked_reader(std::io::Cursor::new(b"private fixture"))
        .read_to_end(&mut bytes)
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Other);
    assert!(matches!(CoreError::from(err), CoreError::Interrupted(_)));
    assert!(bytes.is_empty());
    assert!(run_policy::json_from_str::<serde_json::Value>("{\"token\":1}").is_err());
    drop(scope);
    assert!(run_policy::check().is_ok());
    assert!(run_policy::json_from_str::<serde_json::Value>("{\"token\":1}").is_ok());
    let outer = run_policy::enter(None, Some(Arc::new(|| true)));
    run_policy::check().unwrap();
    {
        let _inner = run_policy::enter(None, Some(Arc::new(|| false)));
        assert!(matches!(
            run_policy::check(),
            Err(CoreError::Interrupted(_))
        ));
    }
    run_policy::check().unwrap();
    drop(outer);
}

#[test]
fn cancellation_is_checked_between_stream_chunks_without_replaying_partial_bytes() {
    struct SlowReader {
        paused: Arc<AtomicBool>,
        reads: usize,
    }
    impl Read for SlowReader {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            self.reads += 1;
            assert_eq!(self.reads, 1, "cancelled input must not be read again");
            bytes[0] = b'x';
            self.paused.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(25));
            Ok(1)
        }
    }
    let paused = Arc::new(AtomicBool::new(false));
    let flag = paused.clone();
    let _scope = run_policy::enter(None, Some(Arc::new(move || !flag.load(Ordering::SeqCst))));
    let mut bytes = Vec::new();
    let err = run_policy::checked_reader(SlowReader { paused, reads: 0 })
        .read_to_end(&mut bytes)
        .unwrap_err();
    assert!(matches!(CoreError::from(err), CoreError::Interrupted(_)));
    assert_eq!(bytes, b"x");
}

#[test]
fn sqlite_vm_interrupt_rolls_back_and_removes_hook_before_next_job() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE confirmed (n INTEGER)")
        .unwrap();
    let _control = run_policy::enter(Some(Instant::now() + Duration::from_millis(20)), None);
    {
        let _sql = run_policy::SqliteScope::new(&conn).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        tx.execute("INSERT INTO confirmed VALUES(1)", []).unwrap();
        let err = tx.query_row("WITH RECURSIVE work(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM work WHERE x<100000000) SELECT SUM(x) FROM work", [], |r| r.get::<_, i64>(0)).unwrap_err();
        assert!(
            matches!(err, rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::OperationInterrupted)
        );
    }
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM confirmed", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(conn.query_row("WITH RECURSIVE work(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM work WHERE x<2000) SELECT MAX(x) FROM work", [], |r| r.get::<_, i64>(0)).unwrap(), 2000, "expired TLS must not leak a callback into the next database job");
}

#[test]
fn default_jsonl_window_resumes_complete_lines_and_larger_line_cap_can_progress() {
    let dir = TempDir::new("scan-window");
    let path = dir.path().join("window.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    let line = format!("{}\n", "x".repeat(1023));
    for _ in 0..32770 {
        file.write_all(line.as_bytes()).unwrap();
    }
    drop(file);
    let limits = JsonlLimits {
        time_budget: None,
        ..Default::default()
    };
    let first = jsonl::read_jsonl(&path, 0, 1, &limits).unwrap();
    assert_eq!(first.stop, StopReason::LineBudget);
    assert_eq!(first.next_offset, 32 * 1024 * 1024);
    assert_eq!(first.lines.len(), 32768);
    let next =
        jsonl::read_jsonl(&path, first.next_offset, first.next_line_number, &limits).unwrap();
    assert_eq!(next.stop, StopReason::Eof);
    assert_eq!(next.lines.len(), 2);
    assert_eq!(next.next_line_number, 32771);
    let cap = 33 * 1024 * 1024;
    std::fs::write(&path, format!("{}\n", "x".repeat(cap))).unwrap();
    let limits = JsonlLimits {
        max_line_bytes: cap + 1,
        time_budget: None,
        ..Default::default()
    };
    let outcome = jsonl::read_jsonl(&path, 0, 1, &limits).unwrap();
    assert_eq!(outcome.lines.len(), 1);
    assert_eq!(outcome.next_offset, cap as u64 + 1);
}

#[test]
fn stopped_retention_and_cost_rebuild_preserve_confirmed_data_and_revision() {
    use llm_usage_core::{
        ingest::commit_batch,
        pricing::EstimateOptions,
        retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy},
    };
    let (_dir, storage) = temp_storage("scan-post-controls");
    let now = ts("2026-09-28T12:00:00Z");
    let at = ts("2026-09-26T12:00:00Z");
    commit_batch(
        &storage,
        &batch(
            "post-source",
            "UTC",
            now,
            vec![with_tokens(evt("post-source", "one", at), 10, 5)],
        ),
        None,
    )
    .unwrap();
    storage
        .recompute_cost_day("UTC", "2026-09-26", now, &EstimateOptions::default())
        .unwrap();
    let revision = storage.data_revision().unwrap();
    let cost: Vec<(String, i64)> = storage
        .conn()
        .prepare("SELECT currency,total_amount_minor FROM daily_cost_usage ORDER BY currency")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!cost.is_empty());
    {
        let _control = run_policy::enter(Some(Instant::now()), None);
        let policy = TieredRetentionPolicy {
            events_days: 1,
            ..Default::default()
        };
        assert!(matches!(
            enforce_tiered_retention(&storage, "UTC", now, &policy),
            Err(CoreError::Interrupted(_))
        ));
        assert!(matches!(
            storage.recompute_cost_day("UTC", "2026-09-26", now, &EstimateOptions::default()),
            Err(CoreError::Interrupted(_))
        ));
    }
    let after: Vec<(String, i64)> = storage
        .conn()
        .prepare("SELECT currency,total_amount_minor FROM daily_cost_usage ORDER BY currency")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(after, cost);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
