//! OpenCode 适配器约定测试：A17 固定源码 0027387 的合成 fixture 与
//! 1.18.34 官方 CLI / 本地模型的真实脱敏 fixture 经
//! 读取→解析→逐次事件→commit→查询。数值对照
//! tests/fixtures/opencode/*/_expectations.md 的人工核算，不改计算规则。
//!
//! 辅助函数从 tests/common/mod.rs 与 tests/hermes_contract.rs 复制（按任务
//! 约束不改共享 common，避免并行冲突）。

mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::framework::{
    normalize_path, run_adapter_scan, DetectOutcome, DiscoverContext, RunConfig, ScanLimits,
    SourceAdapter, SourceRunReport,
};
use llm_usage_core::adapters::mimo_code::MimoCodeAdapter;
use llm_usage_core::adapters::opencode::OpenCodeAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

#[test]
fn archived_session_flag_does_not_hide_or_duplicate_calls() {
    let dir = TempDir::new("opencode-archived");
    let root = build_opencode_db_from_fixture(&dir, "synthetic-step-finish");
    let source = rusqlite::Connection::open(root.join("opencode.db")).unwrap();
    source
        .execute("UPDATE session SET time_archived=?1", [NOW])
        .unwrap();
    let (_db, storage) = temp_storage("opencode-archived");
    run_opencode(&storage, &root, NOW);
    let before = summary(&storage, "2026-01-01", "2026-12-31").totals;
    assert_eq!(before.call_count, 3);
    source
        .execute("UPDATE session SET time_archived=NULL", [])
        .unwrap();
    run_opencode(&storage, &root, NOW + 1);
    source
        .execute("UPDATE session SET time_archived=?1", [NOW + 2])
        .unwrap();
    run_opencode(&storage, &root, NOW + 2);
    assert_eq!(summary(&storage, "2026-01-01", "2026-12-31").totals, before);
}

fn opencode_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("opencode")
        .join(name)
}

/// serde_json 值 → SQLite 值（复制自 tests/common/mod.rs 的 json_to_sql）。
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

/// 按脱敏数据（{schema:{session_ddl,message_ddl,part_ddl},sessions,messages,parts}）
/// 在 <dir>/opencode/opencode.db 重建 SQLite 库，返回数据目录
///（可作为手工根传入 discover）。
fn build_opencode_db(dir: &TempDir, projection: &serde_json::Value) -> PathBuf {
    let home = dir.path().join("opencode");
    std::fs::create_dir_all(&home).unwrap();
    let db_path = home.join("opencode.db");
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

fn build_opencode_db_from_fixture(dir: &TempDir, scenario: &str) -> PathBuf {
    let text =
        std::fs::read_to_string(opencode_fixture(&format!("{scenario}/projection.json"))).unwrap();
    let projection: serde_json::Value = serde_json::from_str(&text).unwrap();
    build_opencode_db(dir, &projection)
}

fn run_opencode(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    let adapter = OpenCodeAdapter::new();
    run_with(storage, root, now_ms, &adapter, None)
}

fn run_with(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    adapter: &dyn SourceAdapter,
    row_limit: Option<u64>,
) -> Vec<SourceRunReport> {
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.parent().unwrap().to_path_buf()],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits: ScanLimits {
            jsonl: llm_usage_core::adapters::jsonl::JsonlLimits {
                max_lines: row_limit,
                ..Default::default()
            },
        },
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("run-{now_ms}"),
    };
    let reports = run_adapter_scan(storage, adapter, &ctx, &config).unwrap();
    assert_eq!(reports.len(), 1, "expected exactly one discovered root");
    reports
}

fn opencode_instance(root: &Path) -> String {
    format!("opencode@{}", normalize_path(root))
}

/// The former parser's full event and checkpoint contract, not a modified hash field.
struct LegacyOpenCode(std::sync::Mutex<Vec<llm_usage_core::domain::EventInput>>);
impl SourceAdapter for LegacyOpenCode {
    fn adapter_id(&self) -> &'static str {
        "opencode"
    }
    fn agent(&self) -> &'static str {
        "opencode"
    }
    fn discover(
        &self,
        ctx: &DiscoverContext,
    ) -> Vec<llm_usage_core::adapters::framework::DiscoveredRoot> {
        OpenCodeAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &llm_usage_core::adapters::framework::DiscoveredRoot) -> String {
        OpenCodeAdapter::new().instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, llm_usage_core::error::CoreError> {
        let mut result = OpenCodeAdapter::new().detect(path)?;
        if let DetectOutcome::Supported { basis, .. } = &mut result {
            *basis = llm_usage_core::domain::VersionBasis::LatestFallback;
        }
        Ok(result)
    }
    fn scan(
        &self,
        target: &llm_usage_core::adapters::framework::ScanTarget,
        stored: &llm_usage_core::adapters::framework::StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<llm_usage_core::adapters::framework::ScanOutcome, llm_usage_core::error::CoreError>
    {
        let mut result = OpenCodeAdapter::new().scan(target, stored, limits, now)?;
        for event in &mut result.events {
            event.parser_version = "opencode-step-finish-parts-1".into();
            event.parse_basis = Some(llm_usage_core::domain::VersionBasis::LatestFallback);
        }
        result.parse_context = Some(
            serde_json::json!({"schema_fingerprint": result.parse_context.as_ref().unwrap()["schema_fingerprint"], "version_basis":"latest_fallback", "db_version":"1.18.34"}),
        );
        result
            .cursor
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("continuation");
        result
            .cursor
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("window_start_ms");
        *self.0.lock().unwrap() = result.events.clone();
        Ok(result)
    }
    fn capability(&self) -> llm_usage_core::adapters::framework::CapabilityTable {
        let mut capability = OpenCodeAdapter::new().capability();
        capability.supported_versions.clear();
        capability.maintenance["parser_version"] = "opencode-step-finish-parts-1".into();
        capability
    }
}

fn projection() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(opencode_fixture(
            "real-1.18.34-local-controlled/projection.json",
        ))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn unchanged_old_library_upgrades_full_digests_and_preserves_history() {
    for legacy_hash in [false, true] {
        let dir = TempDir::new("opencode-policy-upgrade");
        let mut sample = projection();
        let mut later = sample["parts"][0].clone();
        later["id"] = "synthetic-later-control".into();
        for field in ["time_created", "time_updated"] {
            later[field] = (later[field].as_i64().unwrap() + 120_000).into();
        }
        sample["parts"].as_array_mut().unwrap().push(later);
        // Synthetic continuation of the genuine wire shape, with matching session counters.
        for field in ["tokens_input", "tokens_output"] {
            sample["sessions"][0][field] =
                (sample["sessions"][0][field].as_i64().unwrap() * 2).into();
        }
        let root = build_opencode_db(&dir, &sample);
        let (_db, storage) = temp_storage("opencode-policy-upgrade");
        let old = LegacyOpenCode(Default::default());
        run_with(&storage, &root, NOW, &old, None);
        let old_events = old.0.lock().unwrap().clone();
        for event in &old_events {
            let digest = if legacy_hash {
                llm_usage_core::identity::content_hash(event)
            } else {
                llm_usage_core::identity::event_content_hash(event)
            };
            storage
                .conn()
                .execute(
                    "UPDATE usage_events SET content_hash=?1 WHERE source_record_key=?2",
                    rusqlite::params![digest, event.source_record_key],
                )
                .unwrap();
        }
        storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) VALUES(?1,'retained_audit','synthetic audit',?2)", rusqlite::params![opencode_instance(&root), NOW]).unwrap();
        let before = summary(&storage, "2026-01-01", "2026-12-31").totals;
        let source_before = std::fs::read(root.join("opencode.db")).unwrap();
        let report = run_opencode(&storage, &root, NOW + 1);
        assert_eq!(report[0].outcome.as_ref().unwrap().added, 0);
        assert_eq!(
            report[0].outcome.as_ref().unwrap().updated,
            2,
            "report: {report:?}"
        );
        assert_eq!(summary(&storage, "2026-01-01", "2026-12-31").totals, before);
        assert_eq!(
            std::fs::read(root.join("opencode.db")).unwrap(),
            source_before
        );
        let known: i64 = storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE parse_basis='known_version' AND parser_version='opencode-step-finish-parts-2' AND conflict=0", [], |r|r.get(0)).unwrap();
        assert_eq!(known, 2);
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='retained_audit'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        let repeat = run_opencode(&storage, &root, NOW + 2);
        assert_eq!(repeat[0].outcome.as_ref().unwrap().updated, 0);
        assert_eq!(summary(&storage, "2026-01-01", "2026-12-31").totals, before);
    }
}

#[test]
fn mixed_versions_empty_sessions_and_dense_pages_keep_each_records_basis() {
    let dir = TempDir::new("opencode-mixed-pages");
    let mut sample = projection();
    let mut unknown = sample["sessions"][0].clone();
    unknown["id"] = "synthetic-unknown-session".into();
    unknown["version"] = "0.999.0".into();
    let mut empty = unknown.clone();
    empty["id"] = "synthetic-empty-session".into();
    empty["version"] = "99.99.99".into();
    for field in ["tokens_input", "tokens_output"] {
        empty[field] = 0.into();
    }
    sample["sessions"]
        .as_array_mut()
        .unwrap()
        .extend([unknown, empty]);
    let mut part = sample["parts"][0].clone();
    part["id"] = "synthetic-unknown-part".into();
    part["session_id"] = "synthetic-unknown-session".into();
    sample["parts"].as_array_mut().unwrap().push(part);
    let root = build_opencode_db(&dir, &sample);
    let (_db, storage) = temp_storage("opencode-mixed-pages");
    let adapter = OpenCodeAdapter::new();
    assert!(matches!(
        adapter.detect(&root.join("opencode.db")).unwrap(),
        DetectOutcome::Supported {
            basis: llm_usage_core::domain::VersionBasis::LatestFallback,
            ..
        }
    ));
    let first = run_with(&storage, &root, NOW, &adapter, Some(1));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    let context: String = storage
        .conn()
        .query_row("SELECT parse_context FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&context).unwrap()["scan_policy_version"]
            .is_null()
    );
    let cursor_before: String = storage
        .conn()
        .query_row("SELECT cursor_value FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    storage.conn().execute_batch("CREATE TRIGGER abort_opencode_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'synthetic transaction failure'); END;").unwrap();
    let failed = run_with(&storage, &root, NOW + 1, &adapter, Some(1));
    assert!(failed[0].error.is_some());
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT cursor_value FROM ingestion_checkpoints", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        cursor_before
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    storage
        .conn()
        .execute_batch("DROP TRIGGER abort_opencode_checkpoint;")
        .unwrap();
    let second = run_with(&storage, &root, NOW + 2, &adapter, Some(1));
    assert_eq!(second[0].files[0].status, "complete");
    let rows = event_rows(&storage, &opencode_instance(&root));
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .filter(|e| e.17.as_deref() == Some("known_version"))
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|e| e.17.as_deref() == Some("latest_fallback"))
            .count(),
        1
    );
    let status: String = storage
        .conn()
        .query_row("SELECT format_status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&status).unwrap()["basis"],
        "latest_fallback"
    );
    // A new known-only window must not erase the older unknown record's compatibility.
    let conn = rusqlite::Connection::open(root.join("opencode.db")).unwrap();
    conn.execute(
        "UPDATE part SET time_updated=time_updated+120000 WHERE id <> 'synthetic-unknown-part'",
        [],
    )
    .unwrap();
    run_opencode(&storage, &root, NOW + 3);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "active_compat");
}

#[test]
fn empty_session_never_certifies_usage_and_invalid_snapshot_retries() {
    let dir = TempDir::new("opencode-empty-and-bad");
    let mut sample = projection();
    sample["parts"] = serde_json::json!([]);
    let root = build_opencode_db(&dir, &sample);
    let adapter = OpenCodeAdapter::new();
    assert_eq!(
        adapter.detect(&root.join("opencode.db")).unwrap(),
        DetectOutcome::Pending
    );
    let conn = rusqlite::Connection::open(root.join("opencode.db")).unwrap();
    let mut bad = projection()["parts"][0].clone();
    bad["data"] = "{bad".into();
    insert_rows(&conn, "part", &[bad]);
    let (_db, storage) = temp_storage("opencode-empty-and-bad");
    run_opencode(&storage, &root, NOW);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let context: String = storage
        .conn()
        .query_row("SELECT parse_context FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&context).unwrap()["scan_policy_version"]
            .is_null()
    );
    conn.execute(
        "UPDATE part SET data=?1",
        [projection()["parts"][0]["data"].to_string()],
    )
    .unwrap();
    run_opencode(&storage, &root, NOW + 1);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let context: String = storage
        .conn()
        .query_row("SELECT parse_context FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&context).unwrap()["scan_policy_version"],
        "opencode-step-finish-parts-2"
    );
    assert!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code='bad_data_json'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap()
            > 0
    );
}

#[test]
fn real_local_model_steps_match_api_usage_and_remain_idempotent() {
    for (scenario, uncached, cache_read) in [
        ("real-1.18.34-local-default", 295, 3),
        ("real-1.18.34-local-controlled", 298, 0),
    ] {
        let tag = format!(
            "{scenario}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = TempDir::new(&tag);
        let root = build_opencode_db_from_fixture(&dir, scenario);
        let (_db, storage) = temp_storage(&tag);
        let reports = run_opencode(&storage, &root, NOW);
        assert_eq!(reports[0].files[0].events, 1);
        assert_eq!(reports[0].reconciliations.len(), 1);
        assert_eq!(reports[0].reconciliations[0].verdict, "matched");
        let instance = opencode_instance(&root);
        let before = event_rows(&storage, &instance);
        assert_eq!(
            before.len(),
            1,
            "标题调用不在 step-finish 载体内，不补造事件"
        );
        let row = &before[0];
        assert_eq!(row.0, "opencode:part:part_real_local_1");
        assert_eq!(row.2, "primary");
        assert_eq!(row.4, Some(uncached));
        assert_eq!(row.5, Some(cache_read));
        assert_eq!(row.6, Some(0));
        assert_eq!(row.7, Some(298), "API 直报含缓存输入");
        assert_eq!(row.8, Some(1));
        assert_eq!(row.9, Some(0));
        assert_eq!(row.10, Some(299));
        assert_eq!(row.11, Some(299));
        assert_eq!(row.13, "observed_at");
        assert_eq!(row.15.as_deref(), Some("qwen2.5-0.5b-local"));
        assert_eq!(row.16.as_deref(), Some("llama.cpp"));
        assert_eq!(row.17.as_deref(), Some("known_version"));
        let version: String = storage
            .conn()
            .query_row("SELECT schema_version FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, "1.18.34");
        let totals = summary(&storage, "2026-10-05", "2026-10-05").totals;
        assert_eq!(totals.call_count, 1);
        assert_eq!(totals.input_total_known, Some(298));
        assert_eq!(totals.cache_read_known, Some(cache_read));
        assert_eq!(totals.total_tokens_known, Some(299));
        run_opencode(&storage, &root, NOW + 1_000);
        let after = event_rows(&storage, &instance);
        assert_eq!(after.len(), before.len());
        // 大元组不实现 PartialEq；分成互斥两组核对全部白名单字段。
        for (actual, expected) in after.iter().zip(&before) {
            assert_eq!(
                (
                    &actual.0, &actual.1, &actual.2, actual.3, actual.4, actual.5, actual.6,
                    actual.7, actual.8, actual.9
                ),
                (
                    &expected.0,
                    &expected.1,
                    &expected.2,
                    expected.3,
                    expected.4,
                    expected.5,
                    expected.6,
                    expected.7,
                    expected.8,
                    expected.9
                ),
            );
            assert_eq!(
                (
                    actual.10, actual.11, actual.12, &actual.13, &actual.14, &actual.15,
                    &actual.16, &actual.17, actual.18
                ),
                (
                    expected.10,
                    expected.11,
                    expected.12,
                    &expected.13,
                    &expected.14,
                    &expected.15,
                    &expected.16,
                    &expected.17,
                    expected.18
                ),
                "重叠扫描不改变事件或修订",
            );
        }
        assert_eq!(summary(&storage, "2026-10-05", "2026-10-05").totals, totals);
    }
}

/// 事件仅保留白名单字段的数据（一行一条逐次事件）。
type EventRow = (
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);

fn event_rows(storage: &Storage, instance: &str) -> Vec<EventRow> {
    let conn = storage.conn();
    let mut stmt = conn
        .prepare(
            "SELECT source_record_key, agent, call_category, occurred_at_ms, \
                    input_uncached, input_cache_read, input_cache_write, input_total, \
                    output_total, output_reasoning, total_tokens, source_total, \
                    cost_amount_minor, time_basis, lifecycle, model_raw, provider_id, \
                    parse_basis, source_revision \
             FROM usage_events WHERE source_instance_id = ?1 ORDER BY source_record_key",
        )
        .unwrap();
    stmt.query_map(rusqlite::params![instance], |r| {
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
            r.get(11)?,
            r.get(12)?,
            r.get(13)?,
            r.get(14)?,
            r.get(15)?,
            r.get(16)?,
            r.get(17)?,
            r.get(18)?,
        ))
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

#[test]
fn contract_full_chain_matches_manual_expectations() {
    let dir = TempDir::new("opencode-contract");
    let root = build_opencode_db_from_fixture(&dir, "synthetic-step-finish");
    let (_db, storage) = temp_storage("opencode-contract");
    let reports = run_opencode(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 3, "3 条 step-finish 部件");
    assert_eq!(report.files[0].events, 3);
    let instance = opencode_instance(&root);

    // 期望见 _expectations.md：3 条逐次事件，全字段对照。
    let rows = event_rows(&storage, &instance);
    assert_eq!(rows.len(), 3);
    let by_key = |k: &str| rows.iter().find(|r| r.0 == k).unwrap();

    let p1 = by_key("opencode:part:part_syn_1");
    assert_eq!(p1.1, "opencode");
    assert_eq!(p1.2, "primary");
    assert_eq!(p1.3, 1_781_337_600_500);
    assert_eq!(p1.4, Some(1000), "input=未缓存输入（pinned tokens()）");
    assert_eq!(p1.5, Some(8000));
    assert_eq!(p1.6, Some(500));
    assert_eq!(p1.7, Some(9500), "input_total=in+cr+cw 派生");
    assert_eq!(p1.8, Some(250), "output_total=out+reason 派生");
    assert_eq!(p1.9, Some(50));
    assert_eq!(p1.10, Some(9750), "五字段之和");
    assert_eq!(p1.11, Some(9750), "直报 total 对照一致");
    assert_eq!(p1.12, Some(12_000), "cost micro-USD estimated");
    assert_eq!(p1.13, "observed_at", "part 行时间是投影写入时刻");
    assert_eq!(p1.14, "final");
    assert_eq!(p1.15, Some("claude-sonnet-4-6".to_string()));
    assert_eq!(p1.16, Some("anthropic".to_string()));
    assert_eq!(p1.17, Some("latest_fallback".to_string()));
    assert_eq!(p1.18, Some(1_781_337_600_500));

    let p2 = by_key("opencode:part:part_syn_2");
    assert_eq!(p2.7, Some(9500));
    assert_eq!(p2.8, Some(80));
    assert_eq!(p2.10, Some(9580), "无 total 字段仍派生");
    assert_eq!(p2.11, None, "source_total 缺失保持未知");

    let p3 = by_key("opencode:part:part_syn_3");
    assert_eq!(p3.2, "sub_agent", "session.parent_id 非空");
    assert_eq!(p3.7, Some(1300));
    assert_eq!(p3.8, Some(50));
    assert_eq!(p3.10, Some(1350));
    assert_eq!(p3.11, Some(1350));
    assert_eq!(p3.15, Some("gpt-5.2".to_string()));
    assert_eq!(p3.16, Some("openai".to_string()));

    // text 部件不产事件（SQL 层过滤）；汇总对照 _expectations.md。
    let s = summary(&storage, "2026-06-13", "2026-06-13");
    assert_eq!(s.totals.call_count, 3);
    assert_eq!(s.totals.input_total_known, Some(20300));
    assert_eq!(s.totals.cache_read_known, Some(18000));
    assert_eq!(s.totals.cache_write_known, Some(600));
    assert_eq!(s.totals.output_total_known, Some(380));
    assert_eq!(s.totals.total_tokens_known, Some(20680));

    // 会话累计对账：两会话均 matched（五列 = Σ 部件，projector applyUsage）。
    assert_eq!(report.reconciliations.len(), 2);
    assert!(report
        .reconciliations
        .iter()
        .all(|r| r.verdict == "matched"));

    // 合成样本的旧版本未获真实认证，仍保留 latest_fallback 标记。
    let fallback: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'latest_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fallback, 1);
    let basis: String = storage
        .conn()
        .query_row("SELECT format_status FROM source_files", [], |r| {
            r.get::<_, String>(0)
        })
        .and_then(|json| {
            serde_json::from_str::<serde_json::Value>(&json)
                .map(|v| v["basis"].as_str().unwrap_or("").to_string())
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
        .unwrap();
    assert_eq!(basis, "latest_fallback");

    // 幂等：重复扫描不增量。
    run_opencode(&storage, &root, NOW + 1_000);
    let s = summary(&storage, "2026-06-13", "2026-06-13");
    assert_eq!(s.totals.call_count, 3, "重复扫描不增量");
    let _ = dir;
}

#[test]
fn contract_unknown_version_is_latest_fallback_not_rejected() {
    let dir = TempDir::new("opencode-unknown-version");
    let root = build_opencode_db_from_fixture(&dir, "synthetic-unknown-version");
    let (_db, storage) = temp_storage("opencode-unknown-version");
    let reports = run_opencode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1, "兼容尝试数据照常入库");
    let instance = opencode_instance(&root);
    let rows = event_rows(&storage, &instance);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].17, Some("latest_fallback".to_string()));
    let found: Option<String> = storage
        .conn()
        .query_row("SELECT format_status FROM source_files", [], |r| {
            r.get::<_, String>(0)
        })
        .and_then(|json| {
            serde_json::from_str::<serde_json::Value>(&json)
                .map(|v| v["found_version"].as_str().map(str::to_string))
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
        .unwrap();
    assert_eq!(found.as_deref(), Some("9.9.9-syn-uncollected"));
    let _ = dir;
}

#[test]
fn unknown_format_fail_closed_for_bogus_sqlite_and_missing_tables() {
    // 非 SQLite 文件。
    let dir = TempDir::new("opencode-bogus");
    let home = dir.path().join("opencode-home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("opencode.db"), b"not a sqlite file at all").unwrap();
    let outcome = OpenCodeAdapter::new()
        .detect(&home.join("opencode.db"))
        .unwrap();
    assert!(matches!(outcome, DetectOutcome::UnknownFormat { .. }));

    // SQLite 但无三表。
    let empty_db = home.join("other.db");
    {
        let conn = rusqlite::Connection::open(&empty_db).unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    let outcome = OpenCodeAdapter::new().detect(&empty_db).unwrap();
    assert!(matches!(outcome, DetectOutcome::UnknownFormat { .. }));

    // 仅新 core 派生视图层（session_message）而无 part 表 ⇒ 拒绝并注明待核验。
    let core_db = home.join("core-only.db");
    {
        let conn = rusqlite::Connection::open(&core_db).unwrap();
        conn.execute_batch("CREATE TABLE session_message (id text PRIMARY KEY)")
            .unwrap();
    }
    let outcome = OpenCodeAdapter::new().detect(&core_db).unwrap();
    match outcome {
        DetectOutcome::UnknownFormat { reason } => {
            assert!(
                reason.contains("session_message"),
                "区分新 core 数据层：{reason}"
            );
        }
        other => panic!("expected UnknownFormat, got {other:?}"),
    }

    // 空库（两表皆空）⇒ Pending 下轮重探。
    let pending_db = home.join("pending.db");
    {
        let conn = rusqlite::Connection::open(&pending_db).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (id text PRIMARY KEY, project_id text NOT NULL, parent_id text, \
             slug text NOT NULL, directory text NOT NULL, title text NOT NULL, version text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, time_compacting integer, \
             time_archived integer, agent text, model text, cost real NOT NULL DEFAULT 0, \
             tokens_input integer NOT NULL DEFAULT 0, tokens_output integer NOT NULL DEFAULT 0, \
             tokens_reasoning integer NOT NULL DEFAULT 0, tokens_cache_read integer NOT NULL DEFAULT 0, \
             tokens_cache_write integer NOT NULL DEFAULT 0); \
             CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL); \
             CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL, session_id text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);",
        )
        .unwrap();
    }
    let outcome = OpenCodeAdapter::new().detect(&pending_db).unwrap();
    assert_eq!(outcome, DetectOutcome::Pending);
    let _ = dir;
}

#[test]
fn family_fingerprints_are_mutually_exclusive_not_assumed_compatible() {
    // OpenCode 库（session 含 tokens_* 五列、message 无 agent_id）不能过 MiMo 指纹。
    let oc_dir = TempDir::new("opencode-vs-mimo");
    let oc_root = build_opencode_db_from_fixture(&oc_dir, "synthetic-step-finish");
    let oc_db = oc_root.join("opencode.db");
    match MimoCodeAdapter::new().detect(&oc_db).unwrap() {
        DetectOutcome::UnknownFormat { reason } => {
            assert!(
                reason.contains("agent_id") || reason.contains("mimocode"),
                "拒绝理由应指向产品指纹：{reason}"
            );
        }
        other => panic!("MiMo detect must reject opencode db, got {other:?}"),
    }
    // 反向：MiMo 库（无 tokens_* 列）不能过 OpenCode 指纹。
    let mimo_dir = TempDir::new("mimo-vs-opencode");
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("mimo-code")
            .join("synthetic-step-finish")
            .join("projection.json"),
    )
    .unwrap();
    let projection: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mimo_home = mimo_dir.path().join("mimo-home");
    std::fs::create_dir_all(&mimo_home).unwrap();
    {
        let conn = rusqlite::Connection::open(mimo_home.join("mimocode.db")).unwrap();
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
    }
    let outcome = OpenCodeAdapter::new()
        .detect(&mimo_home.join("mimocode.db"))
        .unwrap();
    assert!(
        matches!(outcome, DetectOutcome::UnknownFormat { .. }),
        "OpenCode detect must reject mimocode db (no tokens_* columns)"
    );
    let _ = (oc_dir, mimo_dir);
}

#[test]
fn discover_respects_xdg_data_home_and_manual_parent_root() {
    let dir = TempDir::new("opencode-discover");
    let src = TempDir::new("opencode-discover-src");
    let _ = build_opencode_db_from_fixture(&src, "synthetic-step-finish");
    // XDG 布局：<XDG_DATA_HOME>/opencode/opencode.db。
    let xdg_base = dir.path().join("xdgroot");
    let xdg_home = xdg_base.join("opencode");
    std::fs::create_dir_all(&xdg_home).unwrap();
    std::fs::copy(
        src.path().join("opencode").join("opencode.db"),
        xdg_home.join("opencode.db"),
    )
    .unwrap();
    let adapter = OpenCodeAdapter::new();
    // XDG_DATA_HOME 覆盖（固定源码 global.ts xdgData 解析）。
    let ctx = DiscoverContext {
        home_dir: None,
        env: std::collections::BTreeMap::from([(
            "XDG_DATA_HOME".to_string(),
            xdg_base.to_string_lossy().to_string(),
        )]),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(
        roots.len(),
        1,
        "XDG_DATA_HOME/opencode 直接命中 opencode.db"
    );
    assert!(roots[0].files[0]
        .file_name()
        .is_some_and(|n| n == "opencode.db"));
    assert!(matches!(
        roots[0].basis,
        llm_usage_core::adapters::framework::RootBasis::EnvOverride(_)
    ));
    assert_eq!(normalize_path(&roots[0].root), normalize_path(&xdg_home));

    // 手工根传数据目录的父目录：<root>/opencode 形状定位。
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![xdg_base.to_path_buf()],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(normalize_path(&roots[0].root), normalize_path(&xdg_home));

    // 手工根语义按数据目录名限定：不按文件名递归——grandparent 目录
    //（无名为 opencode 的数据目录形状）不产出根，防止误触同血统产品的
    // opencode-rc.db（kilo 目录实测存在同名前缀库）。
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![dir.path().to_path_buf()],
    };
    assert!(
        adapter.discover(&ctx).is_empty(),
        "数据目录名之外不产出根（防 kilo opencode-rc.db 碰撞）"
    );

    // 默认 home 候选（~/.local/share/opencode）在临时 home 下为空 ⇒ 无根。
    let empty = TempDir::new("opencode-empty");
    let ctx = DiscoverContext {
        home_dir: Some(empty.path().to_path_buf()),
        env: Default::default(),
        manual_roots: vec![],
    };
    assert!(adapter.discover(&ctx).is_empty());
    let _ = (dir, src, empty);
}

#[test]
fn capability_table_keeps_real_sample_scope_and_compatibility_limits() {
    let adapter = OpenCodeAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "opencode");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["1.18.34"]),
        "仅认证真实样本覆盖的版本"
    );
    assert_eq!(
        json["maintenance"]["evidence_level"]
            .as_str()
            .map(|s| s.starts_with("real-local")),
        Some(true),
        "能力声明限定真实本地版本样本",
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
    assert!(json["fields"]["latency"]["availability"]
        .get("unavailable")
        .is_some());
    assert!(json["fields"]["per_request_calls"]["availability"]
        .get("partial")
        .is_some());
    for section in [
        "discovery",
        "detection",
        "lifecycle",
        "incremental",
        "dedup",
        "integrity",
        "maintenance",
        "scheduling",
    ] {
        assert!(json.get(section).is_some(), "capability missing {section}");
    }
    assert!(!cap.limitations.is_empty());
    assert!(cap.limitations.iter().any(|l| l.contains("标题")));
    assert!(cap
        .limitations
        .iter()
        .any(|l| l.contains("latest_fallback")));
}
