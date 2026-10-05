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
/// 在 <dir>/opencode-home/opencode.db 重建 SQLite 库，返回数据目录
///（可作为手工根传入 discover）。
fn build_opencode_db(dir: &TempDir, projection: &serde_json::Value) -> PathBuf {
    let home = dir.path().join("opencode-home");
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

fn opencode_instance(root: &Path) -> String {
    format!("opencode@{}", normalize_path(root))
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
        assert_eq!(row.17.as_deref(), Some("latest_fallback"));
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

    // 合成样本的旧版本未获真实认证；注册表仍为空 ⇒ latest_fallback 标记。
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
        src.path().join("opencode-home").join("opencode.db"),
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
        serde_json::json!([]),
        "注册表为空：真实样本通过不替代混合版本及旧游标升级验收"
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
