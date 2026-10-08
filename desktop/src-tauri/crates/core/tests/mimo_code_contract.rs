//! MiMo Code adapter tests use synthetic data from A14 fixed source 456678b.
//! The 2026-09-25 local survey found no installation. Exercise
//! reading, parsing, per-call events, commit and queries. Compare values with
//! manual calculations in tests/fixtures/mimo-code/*/_expectations.md.
//!
//! Helpers were copied from tests/common/mod.rs and tests/hermes_contract.rs;
//! they remain local here, preserving the shared common module.

mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::framework::{
    normalize_path, run_adapter_scan, DetectOutcome, DiscoverContext, RunConfig, ScanLimits,
    SourceAdapter, SourceRunReport,
};
use llm_usage_core::adapters::mimo_code::{MimoCodeAdapter, MIMOCODE_ENV_HOME};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn mimo_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mimo-code")
        .join(name)
}

/// Map serde_json to SQLite values, copied from tests/common/mod.rs json_to_sql.
fn json_to_sql(value: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as Sql;
    match value {
        serde_json::Value::Null => Sql::Null,
        serde_json::Value::Bool(b) => Sql::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Sql::Integer)
            .or_else(|| n.as_f64().map(Sql::Real))
            .unwrap_or(Sql::Null),
        serde_json::Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

fn insert_rows(conn: &rusqlite::Connection, table: &str, rows: &[serde_json::Value]) {
    for row in rows {
        let obj = row.as_object().unwrap();
        let columns: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            columns.join(", "),
            placeholders.join(", ")
        );
        let values: Vec<rusqlite::types::Value> = obj.values().map(json_to_sql).collect();
        conn.execute(
            &sql,
            rusqlite::params_from_iter(values.iter().map(|v| v as &dyn rusqlite::ToSql)),
        )
        .unwrap();
    }
}

/// Rebuild redacted data at <dir>/mimo-home/data/mimocode.db, following MIMOCODE_HOME
/// placement under <home>/data; return the data directory.
fn build_mimo_db(dir: &TempDir, projection: &serde_json::Value) -> PathBuf {
    let home = dir.path().join("mimo-home").join("data");
    std::fs::create_dir_all(&home).unwrap();
    let db_path = home.join("mimocode.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    conn.execute_batch(projection["schema"]["session_ddl"].as_str().unwrap())
        .unwrap();
    conn.execute_batch(projection["schema"]["message_ddl"].as_str().unwrap())
        .unwrap();
    conn.execute_batch(projection["schema"]["part_ddl"].as_str().unwrap())
        .unwrap();
    insert_rows(&conn, "session", projection["sessions"].as_array().unwrap());
    insert_rows(&conn, "message", projection["messages"].as_array().unwrap());
    insert_rows(&conn, "part", projection["parts"].as_array().unwrap());
    drop(conn);
    home
}

fn build_mimo_db_from_fixture(dir: &TempDir, scenario: &str) -> PathBuf {
    let text =
        std::fs::read_to_string(mimo_fixture(&format!("{scenario}/projection.json"))).unwrap();
    let projection: serde_json::Value = serde_json::from_str(&text).unwrap();
    build_mimo_db(dir, &projection)
}

fn run_mimo(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    let adapter = MimoCodeAdapter::new();
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.to_path_buf()],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("run-{now_ms}"),
    };
    let reports = run_adapter_scan(storage, &adapter, &ctx, &config).unwrap();
    assert_eq!(reports.len(), 1, "expected exactly one discovered root");
    reports
}

fn mimo_instance(root: &Path) -> String {
    format!("mimo-code@{}", normalize_path(root))
}

#[test]
fn contract_full_chain_matches_manual_expectations() {
    let dir = TempDir::new("mimo-contract");
    let root = build_mimo_db_from_fixture(&dir, "synthetic-step-finish");
    let (_db, storage) = temp_storage("mimo-contract");
    let reports = run_mimo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 3);
    assert_eq!(report.files[0].events, 3);
    let instance = mimo_instance(&root);

    let conn = storage.conn();
    let events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE source_instance_id = ?1",
            rusqlite::params![instance],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(events, 3);

    type P1Row = (
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
        String,
    );
    let p1: P1Row = conn
        .query_row(
            "SELECT call_category, input_total, output_total, total_tokens, source_total, \
                    input_cache_read, input_cache_write, cost_amount_minor, model_raw, provider_id, time_basis \
             FROM usage_events WHERE source_instance_id = ?1 AND source_record_key = ?2",
            rusqlite::params![instance, "mimo-code:part:part_syn_1"],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(p1.0, "primary");
    assert_eq!(p1.1, Some(9500), "input_total=in+cr+cw 派生");
    assert_eq!(p1.2, Some(250), "output_total=out+reason 派生");
    assert_eq!(p1.3, Some(9750));
    assert_eq!(p1.4, Some(9750), "直报 total 对照一致");
    assert_eq!(p1.5, Some(8000));
    assert_eq!(p1.6, Some(500));
    assert_eq!(p1.7, Some(12_000), "cost micro-USD estimated");
    assert_eq!(
        p1.8,
        Some("mimo-latest".to_string()),
        "message.data.modelID join"
    );
    assert_eq!(p1.9, Some("mimo".to_string()));
    assert_eq!(p1.10, "observed_at");

    type SubRow = (String, String);
    let sub: SubRow = conn
        .query_row(
            "SELECT call_category, agent FROM usage_events \
             WHERE source_instance_id = ?1 AND source_record_key = ?2",
            rusqlite::params![instance, "mimo-code:part:part_syn_3"],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(sub.0, "sub_agent", "session.parent_id 非空");
    assert_eq!(sub.1, "mimo-code");

    // MiMo session has no cumulative columns, so no reconciliation target is invented.
    assert!(report.reconciliations.is_empty());

    // Compare totals with _expectations.md.
    let s = summary(&storage, "2026-06-13", "2026-06-13");
    assert_eq!(s.totals.call_count, 3);
    assert_eq!(s.totals.input_total_known, Some(20300));
    assert_eq!(s.totals.cache_read_known, Some(18000));
    assert_eq!(s.totals.cache_write_known, Some(600));
    assert_eq!(s.totals.output_total_known, Some(380));
    assert_eq!(s.totals.total_tokens_known, Some(20680));

    // This synthetic record's version is unverified and retains latest_fallback.
    let fallback: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'latest_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fallback, 1);

    // Repeated scanning adds no usage.
    run_mimo(&storage, &root, NOW + 1_000);
    let s = summary(&storage, "2026-06-13", "2026-06-13");
    assert_eq!(s.totals.call_count, 3, "重复扫描不增量");
    let _ = dir;
}

#[test]
fn contract_unknown_version_is_latest_fallback_not_rejected() {
    let dir = TempDir::new("mimo-unknown-version");
    let root = build_mimo_db_from_fixture(&dir, "synthetic-unknown-version");
    let (_db, storage) = temp_storage("mimo-unknown-version");
    let reports = run_mimo(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1, "兼容尝试数据照常入库");
    let instance = mimo_instance(&root);
    let basis: Option<String> = storage
        .conn()
        .query_row(
            "SELECT parse_basis FROM usage_events WHERE source_instance_id = ?1",
            rusqlite::params![instance],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(basis.as_deref(), Some("latest_fallback"));
    let _ = dir;
}

#[test]
fn unknown_format_fail_closed_and_family_fingerprint_exclusive() {
    let dir = TempDir::new("mimo-bogus");
    let home = dir.path().join("mimo-home");
    std::fs::create_dir_all(&home).unwrap();
    // Not SQLite.
    std::fs::write(home.join("mimocode.db"), b"not a sqlite file").unwrap();
    assert!(matches!(
        MimoCodeAdapter::new()
            .detect(&home.join("mimocode.db"))
            .unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
    // SQLite with missing required tables.
    let empty_db = home.join("other.db");
    {
        let conn = rusqlite::Connection::open(&empty_db).unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    assert!(matches!(
        MimoCodeAdapter::new().detect(&empty_db).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));

    // OpenCode message lacks agent_id and fails the MiMo fingerprint; forks do not establish compatibility.
    let oc_dir = TempDir::new("mimo-vs-opencode");
    let oc_text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("opencode")
            .join("synthetic-step-finish")
            .join("projection.json"),
    )
    .unwrap();
    let oc_projection: serde_json::Value = serde_json::from_str(&oc_text).unwrap();
    let oc_home = oc_dir.path().join("opencode-home");
    std::fs::create_dir_all(&oc_home).unwrap();
    {
        let conn = rusqlite::Connection::open(oc_home.join("opencode.db")).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        conn.execute_batch(oc_projection["schema"]["session_ddl"].as_str().unwrap())
            .unwrap();
        conn.execute_batch(oc_projection["schema"]["message_ddl"].as_str().unwrap())
            .unwrap();
        conn.execute_batch(oc_projection["schema"]["part_ddl"].as_str().unwrap())
            .unwrap();
        insert_rows(
            &conn,
            "session",
            oc_projection["sessions"].as_array().unwrap(),
        );
        insert_rows(
            &conn,
            "message",
            oc_projection["messages"].as_array().unwrap(),
        );
        insert_rows(&conn, "part", oc_projection["parts"].as_array().unwrap());
    }
    assert!(matches!(
        MimoCodeAdapter::new()
            .detect(&oc_home.join("opencode.db"))
            .unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
    // opencode_contract.rs checks the reverse; confirm MiMo's own database passes here.
    let good_dir = TempDir::new("mimo-good");
    let good = build_mimo_db_from_fixture(&good_dir, "synthetic-unknown-version");
    let supported = MimoCodeAdapter::new()
        .detect(&good.join("mimocode.db"))
        .unwrap();
    assert!(matches!(supported, DetectOutcome::Supported { .. }));
    let _ = (oc_dir, good_dir);
}

#[test]
fn discover_respects_mimocode_home_env_and_xdg_defaults() {
    let dir = TempDir::new("mimo-discover");
    let root = build_mimo_db_from_fixture(&dir, "synthetic-step-finish");
    let adapter = MimoCodeAdapter::new();
    // MIMOCODE_HOME resolves to <home>/data through resolveMimocodeHome mimocode_home mode.
    let ctx = DiscoverContext {
        home_dir: None,
        env: std::collections::BTreeMap::from([(
            MIMOCODE_ENV_HOME.to_string(),
            dir.path().join("mimo-home").to_string_lossy().to_string(),
        )]),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1, "MIMOCODE_HOME/data 直接命中 mimocode.db");
    assert!(roots[0].files[0]
        .file_name()
        .is_some_and(|n| n == "mimocode.db"));
    assert!(matches!(
        roots[0].basis,
        llm_usage_core::adapters::framework::RootBasis::EnvOverride(_)
    ));
    assert_eq!(normalize_path(&roots[0].root), normalize_path(&root));

    // A manual MIMOCODE_HOME-style parent resolves to <root>/data.
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![dir.path().join("mimo-home")],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(normalize_path(&roots[0].root), normalize_path(&root));

    // A different parent layout produces no root; do not recursively guess by filename.
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![dir.path().to_path_buf()],
    };
    assert!(adapter.discover(&ctx).is_empty());

    // An XDG_DATA_HOME candidate without mimocode*.db produces no root.
    let xdg = TempDir::new("mimo-xdg-empty");
    let ctx = DiscoverContext {
        home_dir: None,
        env: std::collections::BTreeMap::from([(
            "XDG_DATA_HOME".to_string(),
            xdg.path().to_string_lossy().to_string(),
        )]),
        manual_roots: vec![],
    };
    assert!(adapter.discover(&ctx).is_empty());
    let _ = (dir, xdg);
}

#[test]
fn capability_table_retains_scoped_real_evidence_and_independent_registry() {
    let adapter = MimoCodeAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "mimo-code");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!([]),
        "单一路线不认证整个客户端版本"
    );
    assert_eq!(
        json["maintenance"]["evidence_level"]
            .as_str()
            .map(|s| s.starts_with("real-container")),
        Some(true),
        "能力声明限定真实验收路线",
    );
    for key in [
        "tokens",
        "cache_read",
        "cache_write",
        "per_request_calls",
        "model",
        "time",
        "cost",
        "latency",
    ] {
        assert!(
            json["fields"].get(key).is_some(),
            "capability fields missing {key}"
        );
    }
    assert!(
        json["fields"]["model"].get("partial").is_some()
            || json["fields"]["model"]["availability"]
                .get("partial")
                .is_some()
    );
    assert!(json["fields"]["latency"]["availability"]
        .get("unavailable")
        .is_some());
    assert!(!cap.limitations.is_empty());
    assert!(cap.limitations.iter().any(|l| l.contains("仅 0.1.15")));
    assert!(cap.limitations.iter().any(|l| l.contains("length")));
    // MiMo remains independent of OpenCode statistics.
    assert_eq!(
        json["detection"]["magic"],
        serde_json::json!("SQLite + part/session/message 关键列（schema 指纹；message 须含 agent_id 列 ⇒ 与 OpenCode 库互斥，不从 fork 关系推兼容）")
    );
}
