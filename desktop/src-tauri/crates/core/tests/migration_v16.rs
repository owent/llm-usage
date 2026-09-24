//! V16：迁移版本事务、旧程序拒绝新 schema、迁移失败保留旧库。
//! 另覆盖 schema 表清单与 PRAGMA 合同。

mod common;

use common::TempDir;
use llm_usage_core::storage::schema::{Migration, SCHEMA_VERSION};
use llm_usage_core::storage::{OpenOptions, Storage};
use llm_usage_core::CoreError;

const EXPECTED_TABLES: [&str; 18] = [
    "source_instances",
    "source_files",
    "ingestion_checkpoints",
    "usage_events",
    "event_aliases",
    "source_aggregates",
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
    "schema_migrations",
];

#[test]
fn v16_fresh_open_creates_all_contract_tables() {
    let dir = TempDir::new("v16tables");
    let storage = Storage::open(&dir.db_path()).unwrap();
    assert_eq!(storage.schema_version().unwrap(), SCHEMA_VERSION);
    for table in EXPECTED_TABLES {
        let count: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                rusqlite::params![table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table {table}");
    }
    // 迁移记录按版本落盘。
    let applied: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM schema_migrations WHERE version = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(applied, 1);
}

#[test]
fn v16_pragmas_match_contract() {
    let dir = TempDir::new("v16pragma");
    let storage = Storage::open(&dir.db_path()).unwrap();
    let fk: i64 = storage.conn().pragma_query_value(None, "foreign_keys", |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);
    let journal: String = storage.conn().pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
    assert_eq!(journal.to_lowercase(), "wal");
    let sync: i64 = storage.conn().pragma_query_value(None, "synchronous", |r| r.get(0)).unwrap();
    assert_eq!(sync, 2, "synchronous=FULL");
}

#[test]
fn v16_reopen_is_idempotent() {
    let dir = TempDir::new("v16reopen");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        storage
            .conn()
            .execute(
                "INSERT INTO settings (key, value, schema_version, updated_at_ms) VALUES ('k', 'v', 1, 1)",
                [],
            )
            .unwrap();
    }
    let storage = Storage::open(&dir.db_path()).unwrap();
    assert_eq!(storage.schema_version().unwrap(), SCHEMA_VERSION);
    let value: String = storage
        .conn()
        .query_row("SELECT value FROM settings WHERE key = 'k'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "v");
    // 不重复应用迁移。
    let applied: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(applied, 1);
}

/// 旧程序打开新版本 schema：拒绝写入，不破坏性降级。
#[test]
fn v16_old_program_refuses_newer_schema() {
    let dir = TempDir::new("v16newer");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        // 模拟更新版本程序写入的库。
        storage.conn().pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
    }
    let err = Storage::open_with(
        &dir.db_path(),
        OpenOptions { max_supported_version: Some(SCHEMA_VERSION), ..OpenOptions::default() },
    )
    .unwrap_err();
    match err {
        CoreError::SchemaTooNew { found, supported } => {
            assert_eq!(found, SCHEMA_VERSION + 1);
            assert_eq!(supported, SCHEMA_VERSION);
        }
        other => panic!("expected SchemaTooNew, got {other}"),
    }
    // 库内容未被破坏（以支持新版本的程序打开核验）。
    let storage = Storage::open_with(
        &dir.db_path(),
        OpenOptions { max_supported_version: Some(SCHEMA_VERSION + 1), ..OpenOptions::default() },
    )
    .unwrap();
    assert_eq!(storage.schema_version().unwrap(), SCHEMA_VERSION + 1);
}

/// 迁移失败：该版本回滚，旧库保留且旧程序仍可打开。
#[test]
fn v16_failed_migration_preserves_old_database() {
    let dir = TempDir::new("v16fail");
    // 先建立 v1 库并写入数据。
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        storage
            .conn()
            .execute(
                "INSERT INTO settings (key, value, schema_version, updated_at_ms) VALUES ('keep', 'me', 1, 1)",
                [],
            )
            .unwrap();
    }
    // 模拟一次必然失败的 v2 迁移。
    let bad = Migration { version: 2, name: "bad", sql: "CREATE TABLE broken (col BAD SYNTAX !!!);" };
    let err = Storage::open_with(
        &dir.db_path(),
        OpenOptions { max_supported_version: Some(2), extra_migrations: vec![bad], ..OpenOptions::default() },
    )
    .unwrap_err();
    match err {
        CoreError::MigrationFailed { version, name, .. } => {
            assert_eq!(version, 2);
            assert_eq!(name, "bad");
        }
        other => panic!("expected MigrationFailed, got {other}"),
    }
    // 旧库保留：版本仍是 1，数据仍在，旧程序可正常打开。
    let storage = Storage::open(&dir.db_path()).unwrap();
    assert_eq!(storage.schema_version().unwrap(), 1);
    let value: String = storage
        .conn()
        .query_row("SELECT value FROM settings WHERE key = 'keep'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "me");
    let broken: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'broken'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(broken, 0);
}
