//! V16（预发布阶段简化版）：schema 建库、幂等重开、PRAGMA 约定、
//! 版本不匹配拒绝打开（提示重建而非迁移）。

mod common;

use common::TempDir;
use llm_usage_core::storage::schema::SCHEMA_VERSION;
use llm_usage_core::storage::Storage;
use llm_usage_core::CoreError;

const EXPECTED_TABLES: [&str; 22] = [
    "source_instances",
    "origin_hosts",
    "origin_host_names",
    "users",
    "source_files",
    "ingestion_checkpoints",
    "usage_events",
    "event_aliases",
    "source_aggregates",
    "hourly_usage",
    "period_usage",
    "quota_snapshots",
    "daily_usage",
    "aggregate_generations",
    "settings",
    "model_aliases",
    "price_versions",
    "extraction_schedules",
    "schedule_state",
    "ingest_runs",
    "diagnostics",
    "import_manifests",
];

#[test]
fn v16_fresh_open_creates_all_contract_tables() {
    let dir = TempDir::new("v16-fresh");
    let storage = Storage::open(&dir.db_path()).unwrap();
    assert_eq!(storage.schema_version().unwrap(), SCHEMA_VERSION);
    for table in EXPECTED_TABLES {
        let count: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "table {table} missing");
    }
}

#[test]
fn v16_reopen_is_idempotent() {
    let dir = TempDir::new("v16-reopen");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        storage
            .conn()
            .execute(
                "INSERT INTO settings (key, value, schema_version, updated_at_ms)
                 VALUES ('test', 'ok', 1, 1)",
                [],
            )
            .unwrap();
    }
    let storage = Storage::open(&dir.db_path()).unwrap();
    let value: String = storage
        .conn()
        .query_row("SELECT value FROM settings WHERE key = 'test'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, "ok");
}

#[test]
fn v16_pragmas_match_contract() {
    let dir = TempDir::new("v16-pragmas");
    let storage = Storage::open(&dir.db_path()).unwrap();
    let journal: String = storage
        .conn()
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(journal.to_lowercase(), "wal");
    let sync: i64 = storage
        .conn()
        .pragma_query_value(None, "synchronous", |r| r.get(0))
        .unwrap();
    assert_eq!(sync, 2, "synchronous=FULL");
}

#[test]
fn v16_schema_mismatch_rejected() {
    // 用错误版本号打开 ⇒ 报错（应用层提示重建或退出）。
    let dir = TempDir::new("v16-mismatch");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        storage
            .conn()
            .pragma_update(None, "user_version", 999u32)
            .unwrap();
    }
    let result = Storage::open(&dir.db_path());
    assert!(result.is_err());
    match result.unwrap_err() {
        CoreError::SchemaMismatch { found, expected } => {
            assert_eq!(found, 999);
            assert_eq!(expected, SCHEMA_VERSION);
        }
        CoreError::SchemaTooNew { found, supported } => {
            assert_eq!(found, 999);
            assert_eq!(supported, SCHEMA_VERSION);
        }
        other => panic!("expected schema error, got {other:?}"),
    }
}
