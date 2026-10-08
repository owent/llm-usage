//! Native schema 24 fields and tests for specific failure conditions.
mod common;
use llm_usage_core::{
    adapters::{built_in_adapters, framework::*, openclaw::OpenClawAdapter},
    jobs::{RunStatus, TriggerKind},
    storage::Storage,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
const NOW: i64 = 1_800_000_000_000;

struct WorkspaceDir(PathBuf);
impl WorkspaceDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../build/plan-final-push/openclaw-tests")
            .join(format!("{}-{nanos}-{tag}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for WorkspaceDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn storage(tag: &str) -> (WorkspaceDir, Storage) {
    let dir = WorkspaceDir::new(tag);
    let db = Storage::open(&dir.path().join("app.sqlite")).unwrap();
    (dir, db)
}
fn projection() -> Value {
    serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/openclaw/real-2026.9.8/projection.json"),
        )
        .unwrap(),
    )
    .unwrap()
}
fn save(root: &Path, value: &Value) -> PathBuf {
    let path = root.join("agents/main/agent/openclaw-agent.sqlite");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=24;
      CREATE TABLE schema_meta(meta_key TEXT PRIMARY KEY,role TEXT,schema_version INTEGER,agent_id TEXT,app_version TEXT);
      CREATE TABLE session_windows(session_id TEXT PRIMARY KEY,session_key TEXT,session_entry_provenance INTEGER,acp_owned INTEGER,plugin_owner_id TEXT,hook_external_content_source TEXT,agent_harness_id TEXT);
      CREATE TABLE transcript_events(session_id TEXT,seq INTEGER,event_json TEXT,created_at INTEGER,event_zstd BLOB,event_utf8_bytes INTEGER,navigation_json TEXT,PRIMARY KEY(session_id,seq));
      CREATE TABLE session_transcript_cold_archives(session_id TEXT PRIMARY KEY);").unwrap();
    let m = &value["schema_meta"];
    db.execute(
        "INSERT INTO schema_meta VALUES(?1,?2,?3,?4,?5)",
        params![
            m["meta_key"].as_str(),
            m["role"].as_str(),
            m["schema_version"].as_i64(),
            m["agent_id"].as_str(),
            m["app_version"].as_str()
        ],
    )
    .unwrap();
    for w in value["session_windows"].as_array().unwrap() {
        db.execute(
            "INSERT INTO session_windows VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                w["session_id"].as_str(),
                w["session_key"].as_str(),
                w["session_entry_provenance"].as_i64(),
                w["acp_owned"].as_i64(),
                w["plugin_owner_id"].as_str(),
                w["hook_external_content_source"].as_str(),
                w["agent_harness_id"].as_str()
            ],
        )
        .unwrap();
    }
    for e in value["transcript_events"].as_array().unwrap() {
        insert(&db, e);
    }
    path
}
fn insert(db: &Connection, e: &Value) {
    db.execute(
        "INSERT INTO transcript_events(session_id,seq,event_json,created_at) VALUES(?1,?2,?3,?4)",
        params![
            e["session_id"].as_str(),
            e["seq"].as_i64(),
            serde_json::to_string(&e["event"]).unwrap(),
            NOW
        ],
    )
    .unwrap();
}
fn context(root: &Path) -> DiscoverContext {
    DiscoverContext {
        manual_roots: vec![root.to_path_buf()],
        ..Default::default()
    }
}
fn config(now: i64) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("openclaw-{now}"),
    }
}
fn run(db: &Storage, root: &Path, now: i64) -> Vec<SourceRunReport> {
    run_adapter_scan(db, &OpenClawAdapter::new(), &context(root), &config(now)).unwrap()
}
fn count(db: &Storage, sql: &str) -> i64 {
    db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn checkpoint(db: &Storage) -> String {
    db.conn()
        .query_row("SELECT cursor_value FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn genuine_registry_read_preserves_positive_api_buckets_and_repeat_identity() {
    let source = WorkspaceDir::new("native");
    let path = save(source.path(), &projection());
    let bytes = std::fs::read(&path).unwrap();
    let (_dir, db) = storage("native-app");
    for now in [NOW, NOW + 1] {
        for adapter in built_in_adapters() {
            run_adapter_scan(&db, adapter.as_ref(), &context(source.path()), &config(now)).unwrap();
        }
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(
        count(&db, "SELECT SUM(input_uncached) FROM usage_events"),
        6157
    );
    assert_eq!(
        count(&db, "SELECT SUM(output_total) FROM usage_events"),
        154
    );
    assert_eq!(
        count(&db, "SELECT SUM(input_cache_read) FROM usage_events"),
        6101
    );
    assert_eq!(count(&db,"SELECT COUNT(*) FROM usage_events WHERE input_total IS NULL AND total_tokens IS NULL AND source_total IS NULL AND input_cache_write IS NULL AND cost_amount_minor IS NULL AND record_kind='usage_observation' AND parse_basis='latest_fallback' AND time_basis='source_start'"),2);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM source_files"), 1);
    let revision = db.data_revision().unwrap();
    run(&db, source.path(), NOW + 2);
    assert_eq!(revision, db.data_revision().unwrap());
    assert_eq!(bytes, std::fs::read(path).unwrap());
}

#[test]
fn real_agent_segment_and_all_manual_roots_env_precedence_are_discovered() {
    let source = WorkspaceDir::new("discovery");
    let root = source.path().join(".openclaw");
    let file = save(&root, &projection());
    let adapter = OpenClawAdapter::new();
    for root in [
        &root,
        &root.join("agents"),
        &root.join("agents/main"),
        file.parent().unwrap(),
        &file,
        source.path(),
    ] {
        let found = adapter.discover(&context(root));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].files, vec![file.clone()]);
    }
    let selected = source.path().join("selected");
    let selected_file = save(&selected, &projection());
    let ctx = DiscoverContext {
        home_dir: Some(source.path().into()),
        env: std::collections::BTreeMap::from([
            (
                "OPENCLAW_HOME".into(),
                source.path().to_string_lossy().into_owned(),
            ),
            ("OPENCLAW_STATE_DIR".into(), "~/selected".into()),
        ]),
        manual_roots: vec![],
    };
    assert_eq!(adapter.discover(&ctx)[0].files, vec![selected_file]);
    let ctx = DiscoverContext {
        home_dir: Some(source.path().into()),
        ..Default::default()
    };
    assert_eq!(adapter.discover(&ctx)[0].files, vec![file]);
}

#[test]
fn unknown_schema_owner_global_and_legacy_inputs_cannot_advance_checkpoint() {
    for sql in [
        "PRAGMA user_version=25;UPDATE schema_meta SET schema_version=25",
        "UPDATE schema_meta SET agent_id='other'",
        "UPDATE schema_meta SET role='global'",
        "PRAGMA user_version=23",
    ] {
        let source = WorkspaceDir::new("identity");
        let file = save(source.path(), &projection());
        Connection::open(file).unwrap().execute_batch(sql).unwrap();
        let (_dir, db) = storage("identity-app");
        run(&db, source.path(), NOW);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 0);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
        assert!(count(&db, "SELECT COUNT(*) FROM diagnostics") > 0);
    }
    let source = WorkspaceDir::new("legacy");
    let root = source.path().join("agents/main/sessions");
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("session.jsonl");
    std::fs::write(
        &file,
        serde_json::to_string(&projection()["transcript_events"][1]["event"]).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        OpenClawAdapter::new().detect(&file).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn migrated_external_and_other_harness_records_stay_isolated() {
    for update in [
        "session_entry_provenance=0",
        "acp_owned=1",
        "plugin_owner_id='external'",
        "hook_external_content_source='webhook'",
        "agent_harness_id='external-cli'",
        "session_key='agent:other:foreign'",
    ] {
        let source = WorkspaceDir::new("provenance");
        let file = save(source.path(), &projection());
        Connection::open(file)
            .unwrap()
            .execute_batch(&format!("UPDATE session_windows SET {update}"))
            .unwrap();
        let (_dir, db) = storage("provenance-app");
        run(&db, source.path(), NOW);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 0);
        assert!(
            count(
                &db,
                "SELECT COUNT(*) FROM diagnostics WHERE code='source_attribution_unverified'"
            ) > 0
        );
    }
}

#[test]
fn bad_native_fields_and_other_transport_do_not_abort_remaining_valid_messages() {
    let mut value = projection();
    let u = &mut value["transcript_events"][1]["event"]["message"]["usage"];
    u["input"] = json!("bad");
    u["cacheWrite"] = json!(-1);
    u["reasoningTokens"] = json!(10000);
    u["totalTokens"] = json!(1);
    let source = WorkspaceDir::new("badfields");
    let file = save(source.path(), &value);
    let (_dir, db) = storage("badfields-app");
    run(&db, source.path(), NOW);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(count(&db,"SELECT COUNT(*) FROM usage_events WHERE input_uncached IS NULL AND input_cache_write IS NULL AND output_reasoning IS NULL"),1);
    assert_eq!(
        count(&db, "SELECT SUM(output_total) FROM usage_events"),
        154
    );
    assert!(
        count(
            &db,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_contradiction'"
        ) > 0
    );
    let conn = Connection::open(file).unwrap();
    let mut other = value["transcript_events"][1].clone();
    other["seq"] = json!(99);
    other["event"]["id"] = json!("unverified-api");
    other["event"]["message"]["api"] = json!("anthropic-messages");
    insert(&conn, &other);
    run(&db, source.path(), NOW + 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    assert!(
        count(
            &db,
            "SELECT COUNT(*) FROM diagnostics WHERE code='unverified_transport'"
        ) > 0
    );
}

#[test]
fn zstd_exact_length_bad_json_sql_types_and_oversized_bodies_are_bounded() {
    let source = WorkspaceDir::new("encoding");
    let file = save(source.path(), &projection());
    let conn = Connection::open(&file).unwrap();
    let text: String = conn
        .query_row(
            "SELECT event_json FROM transcript_events WHERE seq=5",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let compressed = zstd::stream::encode_all(text.as_bytes(), 3).unwrap();
    conn.execute("UPDATE transcript_events SET event_json=NULL,event_zstd=?1,event_utf8_bytes=?2,navigation_json='{}' WHERE seq=5",params![compressed,text.len() as i64]).unwrap();
    let (_dir, db) = storage("encoding-app");
    run(&db, source.path(), NOW);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    conn.execute(
        "UPDATE transcript_events SET event_utf8_bytes=1 WHERE seq=5",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE transcript_events SET event_json=x'ff' WHERE seq=4",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE transcript_events SET event_json='{' WHERE seq=7",
        [],
    )
    .unwrap();
    let mut huge = projection()["transcript_events"][1].clone();
    huge["seq"] = json!(100);
    huge["event"]["id"] = json!("oversized");
    huge["event"]["message"]["content"] = json!("x".repeat(4 * 1024 * 1024 + 1));
    insert(&conn, &huge);
    run(&db, source.path(), NOW + 1);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM usage_events"),
        2,
        "old accepted history survives unreadable source rows"
    );
    for code in [
        "invalid_transcript_encoding",
        "invalid_row_type",
        "invalid_transcript_json",
        "transcript_body_too_large",
    ] {
        assert!(
            db.conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code=?1",
                    [code],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap()
                > 0,
            "{code}"
        );
    }
}

#[test]
fn pagination_rechecks_old_wal_rows_and_whole_db_version_never_certifies_history() {
    let source = WorkspaceDir::new("pagination");
    let file = save(source.path(), &projection());
    let (_dir, db) = storage("pagination-app");
    let mut cfg = config(NOW);
    cfg.limits.jsonl.max_lines = Some(1);
    for n in 0..8 {
        cfg.now_ms = NOW + n;
        cfg.run_id_prefix = format!("page-{n}");
        run_adapter_scan(&db, &OpenClawAdapter::new(), &context(source.path()), &cfg).unwrap();
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    let conn = Connection::open(&file).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;UPDATE schema_meta SET app_version='9999.new';")
        .unwrap();
    let mut new = projection()["transcript_events"][1].clone();
    new["seq"] = json!(2);
    new["event"]["id"] = json!("late-before-cursor");
    insert(&conn, &new);
    for n in 8..16 {
        cfg.now_ms = NOW + n;
        cfg.run_id_prefix = format!("page-{n}");
        run_adapter_scan(&db, &OpenClawAdapter::new(), &context(source.path()), &cfg).unwrap();
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 3);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM usage_events WHERE parse_basis='latest_fallback'"
        ),
        3
    );
}

#[test]
fn transaction_rollback_preserves_cursor_and_retry_including_parallel_scans() {
    let source = WorkspaceDir::new("rollback");
    save(source.path(), &projection());
    let (_dir, db) = storage("rollback-app");
    db.conn().execute_batch("CREATE TRIGGER reject_openclaw BEFORE INSERT ON usage_events BEGIN SELECT RAISE(ABORT,'controlled failure'); END").unwrap();
    assert_eq!(run(&db, source.path(), NOW)[0].finish, RunStatus::Failed);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 0);
    db.conn()
        .execute_batch("DROP TRIGGER reject_openclaw")
        .unwrap();
    run(&db, source.path(), NOW + 1);
    let cursor = checkpoint(&db);
    let revision = db.data_revision().unwrap();
    let adapters = built_in_adapters();
    let contexts = adapters
        .iter()
        .map(|_| context(source.path()))
        .collect::<Vec<_>>();
    let tasks = adapters
        .iter()
        .zip(contexts)
        .map(|(a, ctx)| ParallelScanRequest {
            adapter: a.as_ref(),
            context: ctx,
            config: config(NOW + 2),
            filter: InstanceFilter::default(),
        })
        .collect::<Vec<_>>();
    let locked = std::sync::Mutex::new(db);
    for result in
        run_adapter_scans_parallel(&locked, &tasks, None, std::sync::Arc::new(|| true), None)
    {
        result.unwrap();
    }
    let db = locked.into_inner().unwrap();
    assert_eq!(checkpoint(&db), cursor);
    assert_eq!(db.data_revision().unwrap(), revision);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
}

#[test]
fn cold_archive_gap_is_visible_and_vanished_hot_rows_keep_saved_history() {
    let source = WorkspaceDir::new("cold");
    let file = save(source.path(), &projection());
    let (_dir, db) = storage("cold-app");
    run(&db, source.path(), NOW);
    let conn = Connection::open(file).unwrap();
    conn.execute_batch("DELETE FROM transcript_events;INSERT INTO session_transcript_cold_archives VALUES('session-redacted')").unwrap();
    run(&db, source.path(), NOW + 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    assert!(
        count(
            &db,
            "SELECT COUNT(*) FROM diagnostics WHERE code='cold_archive_coverage_incomplete'"
        ) > 0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM source_instances WHERE health='degraded'"
        ),
        1
    );
}

#[test]
fn default_zero_buckets_do_not_become_tokens_cost_or_calls() {
    let mut value = projection();
    for row in value["transcript_events"].as_array_mut().unwrap() {
        if row["event"]["message"]["role"] == "assistant" {
            row["event"]["message"]["usage"] = json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"total":0}});
        }
    }
    let source = WorkspaceDir::new("zero");
    save(source.path(), &value);
    let (_dir, db) = storage("zero-app");
    run(&db, source.path(), NOW);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_unreported'"
        ),
        2
    );
}

#[test]
fn native_duplicate_entry_ids_keep_conflict_evidence_without_double_counting() {
    let source = WorkspaceDir::new("conflict");
    let file = save(source.path(), &projection());
    let conn = Connection::open(file).unwrap();
    let mut duplicate = projection()["transcript_events"][1].clone();
    duplicate["seq"] = json!(100);
    insert(&conn, &duplicate);
    let (_dir, db) = storage("conflict-app");
    run(&db, source.path(), NOW);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    duplicate["seq"] = json!(101);
    duplicate["event"]["message"]["usage"]["input"] = json!(6106);
    insert(&conn, &duplicate);
    run(&db, source.path(), NOW + 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM usage_events WHERE conflict=1"),
        1
    );
    assert!(
        count(
            &db,
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'"
        ) > 0
    );
}
