//! Verified local database snapshots; time/token similarity is never identity.
mod common;
use common::{batch, evt, temp_storage, ts, with_tokens, TempDir};
use llm_usage_core::adapters::zcode::db_backfill::zcode_db_backfill;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::storage::Storage;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_790_553_600_000; // 2026-09-28 UTC
fn database(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("db").join("db.sqlite");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE model_usage (
      id TEXT PRIMARY KEY, attempt_index INTEGER, session_id TEXT,
      provider_id TEXT, model_id TEXT, query_source TEXT,
      completed_at INTEGER, duration_ms INTEGER,
      input_tokens INTEGER, cache_read_input_tokens INTEGER,
      cache_creation_input_tokens INTEGER, output_tokens INTEGER,
      reasoning_tokens INTEGER, provider_total_tokens INTEGER, status TEXT
    )",
    )
    .unwrap();
    path
}
fn add(path: &Path, id: &str, time: i64, input: Option<i64>) {
    Connection::open(path).unwrap().execute(
      "INSERT INTO model_usage VALUES (?1,0,'same-session','provider','Model','main_turn',?2,100,?3,20,0,10,0,NULL,'completed')",
      rusqlite::params![id,time,input]).unwrap();
}
fn totals(storage: &Storage, instance: &str) -> (i64, Option<i64>) {
    storage.conn().query_row("SELECT COALESCE(SUM(call_count),0),SUM(total_known_sum) FROM daily_usage WHERE instance_id=?1", [instance],
      |r| Ok((r.get(0)?,r.get(1)?))).unwrap()
}
#[test]
fn switches_carrier_atomically_without_heuristic_dedup_and_replay_is_stable() {
    let (_dir, storage) = temp_storage("zcode-snapshot");
    let source = TempDir::new("zcode-db");
    let path = database(&source);
    let time = ts("2026-09-27T10:00:00Z");
    add(&path, "native-a", time, Some(100));
    add(&path, "native-b", time, Some(100)); // identical tokens/time, distinct calls
    let mut legacy = with_tokens(evt("zcode@test", "zcode:jsonl:1", time - 20), 100, 10);
    legacy.agent = "zcode".into();
    commit_batch(
        &storage,
        &batch("zcode@test", "UTC", NOW, vec![legacy]),
        None,
    )
    .unwrap();
    let out = zcode_db_backfill(&storage, &path, "zcode@test", "UTC", NOW).unwrap();
    assert_eq!(out.db_rows, 2);
    assert_eq!(totals(&storage, "zcode@test"), (2, Some(220)));
    let revision = storage.data_revision().unwrap();
    let replay = zcode_db_backfill(&storage, &path, "zcode@test", "UTC", NOW + 1).unwrap();
    assert_eq!(replay.matched_existing, 2);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(totals(&storage, "zcode@test"), (2, Some(220)));
    // Same native IDs in another installation must still count.
    zcode_db_backfill(&storage, &path, "zcode@other", "UTC", NOW + 2).unwrap();
    assert_eq!(totals(&storage, "zcode@other"), (2, Some(220)));
    // A changed authoritative record replaces the whole contribution.
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE model_usage SET input_tokens=200 WHERE id='native-a'",
            [],
        )
        .unwrap();
    zcode_db_backfill(&storage, &path, "zcode@test", "UTC", NOW + 3).unwrap();
    assert_eq!(totals(&storage, "zcode@test"), (2, Some(320)));
}
#[test]
fn archive_recovers_expired_days_without_restoring_details_or_erasing_other_sources() {
    let (_dir, storage) = temp_storage("zcode-old");
    let source = TempDir::new("zcode-old-db");
    let path = database(&source);
    let time = ts("2026-09-10T10:00:00Z");
    let mut other = batch(
        "other",
        "UTC",
        NOW,
        vec![with_tokens(evt("other", "one", time), 500, 10)],
    );
    other.timezone = "UTC".into();
    commit_batch(&storage, &other, None).unwrap();
    storage
        .conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    storage
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    storage.conn().execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES ('detail_retention_floor_ms',?1,1,0)",
      [ts("2026-09-20T00:00:00Z").to_string()]).unwrap();
    add(&path, "old-call", time, Some(100));
    zcode_db_backfill(&storage, &path, "zcode@test", "UTC", NOW).unwrap();
    assert_eq!(totals(&storage, "other"), (1, Some(510)));
    assert_eq!(totals(&storage, "zcode@test"), (1, Some(110)));
    let details: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(details, 0);
    zcode_db_backfill(&storage, &path, "zcode@test", "UTC", NOW + 1).unwrap();
    assert_eq!(totals(&storage, "zcode@test"), (1, Some(110)));
}
#[test]
fn source_pruning_preserves_captured_totals_and_invalid_snapshot_rolls_back() {
    let (_dir, storage) = temp_storage("zcode-prune");
    let source = TempDir::new("zcode-prune-db");
    let path = database(&source);
    let time = ts("2026-09-01T10:00:00Z");
    add(&path, "a", time, Some(100));
    add(&path, "b", time + 1, Some(200));
    zcode_db_backfill(
        &storage,
        &path,
        "zcode@test",
        "UTC",
        ts("2026-09-02T00:00:00Z"),
    )
    .unwrap();
    Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM model_usage WHERE id='a'", [])
        .unwrap();
    zcode_db_backfill(
        &storage,
        &path,
        "zcode@test",
        "UTC",
        ts("2026-10-02T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(totals(&storage, "zcode@test"), (2, Some(320)));
    add(&path, "invalid", ts("2026-10-01T00:00:00Z"), Some(-1));
    assert!(zcode_db_backfill(
        &storage,
        &path,
        "zcode@test",
        "UTC",
        ts("2026-10-02T00:00:00Z")
    )
    .is_err());
    assert_eq!(totals(&storage, "zcode@test"), (2, Some(320)));
}
#[test]
fn unknown_input_remains_unknown() {
    let (_dir, storage) = temp_storage("zcode-unknown");
    let source = TempDir::new("zcode-unknown-db");
    let path = database(&source);
    add(&path, "a", ts("2026-09-27T10:00:00Z"), None);
    zcode_db_backfill(&storage, &path, "zcode@test", "UTC", NOW).unwrap();
    assert_eq!(totals(&storage, "zcode@test"), (1, None));
}
#[test]
fn db_only_discovery_and_late_jsonl_are_exclusive() {
    use llm_usage_core::adapters::framework::{
        run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
    };
    use llm_usage_core::adapters::zcode::ZcodeAdapter;
    let (_dir, storage) = temp_storage("zcode-auto");
    let source = TempDir::new("zcode-auto-db");
    let path = database(&source);
    add(&path, "a", ts("2026-09-27T10:00:00Z"), Some(100));
    let ctx = DiscoverContext {
        manual_roots: vec![source.path().into()],
        ..Default::default()
    };
    let mut cfg = RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: llm_usage_core::jobs::TriggerKind::Manual,
        run_id_prefix: "auto-1".into(),
        origin_host_id: None,
    };
    let first = run_adapter_scan(&storage, &ZcodeAdapter::new(), &ctx, &cfg).unwrap();
    assert_eq!(first.len(), 1);
    assert!(first[0].error.is_none(), "{:?}", first[0]);
    let instance = &first[0].instance_id;
    assert_eq!(totals(&storage, instance), (1, Some(110)));
    std::fs::create_dir_all(source.path().join("rollout")).unwrap();
    std::fs::write(
        source.path().join("rollout/model-io-late.jsonl"),
        "{\"type\":\"model_io\",\"sessionId\":\"late\"}\n",
    )
    .unwrap();
    cfg.run_id_prefix = "auto-2".into();
    run_adapter_scan(&storage, &ZcodeAdapter::new(), &ctx, &cfg).unwrap();
    assert_eq!(totals(&storage, instance), (1, Some(110)));
    std::fs::remove_file(&path).unwrap();
    cfg.run_id_prefix = "auto-3".into();
    let missing = run_adapter_scan(&storage, &ZcodeAdapter::new(), &ctx, &cfg).unwrap();
    assert!(missing[0].error.is_some());
    assert_eq!(totals(&storage, instance), (1, Some(110)));
}

#[test]
fn database_scan_keeps_unaccounted_jsonl_alerts_visible() {
    use llm_usage_core::adapters::framework::{
        run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
    };
    use llm_usage_core::adapters::zcode::ZcodeAdapter;
    let (_dir, storage) = temp_storage("zcode-old-alerts");
    let source = TempDir::new("zcode-old-alerts-src");
    let path = database(&source);
    add(&path, "native-a", ts("2026-09-27T10:00:00Z"), Some(100));
    let ctx = DiscoverContext {
        manual_roots: vec![source.path().into()],
        ..Default::default()
    };
    let mut config = RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: llm_usage_core::jobs::TriggerKind::Manual,
        run_id_prefix: "db-alerts-1".into(),
        origin_host_id: None,
    };
    let first = run_adapter_scan(&storage, &ZcodeAdapter::new(), &ctx, &config).unwrap();
    let instance = &first[0].instance_id;
    for (name, status) in [("old-bad", "unsupported"), ("old-partial", "degraded")] {
        storage.conn().execute(
            "INSERT INTO source_files(file_id,instance_id,file_identity,generation,status,first_seen_ms,last_seen_ms) VALUES (?1,?2,?3,0,?4,?5,?5)",
            rusqlite::params![name, instance, name, status, NOW],
        ).unwrap();
    }
    config.now_ms += 1;
    config.run_id_prefix = "db-alerts-2".into();
    run_adapter_scan(&storage, &ZcodeAdapter::new(), &ctx, &config).unwrap();
    let unresolved: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM source_files WHERE instance_id=?1 AND status IN ('unsupported','degraded')",
            [instance],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unresolved, 2);
    let health: String = storage
        .conn()
        .query_row(
            "SELECT health FROM source_instances WHERE instance_id=?1",
            [instance],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(health, "degraded");
    assert_eq!(totals(&storage, instance), (1, Some(110)));
}
