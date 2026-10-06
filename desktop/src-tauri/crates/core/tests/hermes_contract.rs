//! Hermes Agent 适配器约定测试：合成 fixture（目录标 synthetic，
//! A24 固定源码 ef70b3661cbfcf57e583008ad91dd04d8ba46070）经
//! 读取→解析→区间汇总→commit→查询。数值对照
//! tests/fixtures/hermes/*/_expectations.md 的人工核算，不改计算规则。
//!
//! 真实 0.21.5 CLI/续会话及旧处理位置升级回归也在此文件，保持与合成场景区分。

mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::framework::{
    normalize_path, run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
    SourceRunReport,
};
use llm_usage_core::adapters::hermes::HermesAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn hermes_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("hermes")
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

/// 按脱敏数据（{schema:{schema_version,sessions_ddl,session_model_usage_ddl},
/// sessions,session_model_usage}）在 <dir>/hermes-home/state.db 重建 SQLite 库，
/// 返回 hermes home（可作为手工根传入 discover）。
fn build_hermes_db(dir: &TempDir, projection: &serde_json::Value) -> PathBuf {
    let home = dir.path().join("hermes-home");
    std::fs::create_dir_all(&home).unwrap();
    let db_path = home.join("state.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    conn.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL);")
        .unwrap();
    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        rusqlite::params![projection["schema"]["schema_version"].as_i64().unwrap()],
    )
    .unwrap();
    conn.execute_batch(projection["schema"]["sessions_ddl"].as_str().unwrap())
        .unwrap();
    conn.execute_batch(
        projection["schema"]["session_model_usage_ddl"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    insert_rows(
        &conn,
        "sessions",
        projection["sessions"].as_array().unwrap(),
    );
    insert_rows(
        &conn,
        "session_model_usage",
        projection["session_model_usage"].as_array().unwrap(),
    );
    drop(conn);
    home
}

fn build_hermes_db_from_fixture(dir: &TempDir, scenario: &str) -> PathBuf {
    let text =
        std::fs::read_to_string(hermes_fixture(&format!("{scenario}/projection.json"))).unwrap();
    let projection: serde_json::Value = serde_json::from_str(&text).unwrap();
    build_hermes_db(dir, &projection)
}

fn run_hermes(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    let adapter = HermesAdapter::new();
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

fn hermes_instance(root: &Path) -> String {
    format!("hermes@{}", normalize_path(root))
}

/// 一条区间汇总的仅保留白名单字段的数据。
type AggRow = (
    String,
    Option<i64>,
    i64,
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
    Option<i64>,
);

fn aggregate_rows(storage: &Storage, instance: &str) -> Vec<AggRow> {
    let conn = storage.conn();
    let mut stmt = conn
        .prepare(
            "SELECT scope, interval_start_ms, interval_end_ms, input_uncached, \
                    input_cache_read, input_cache_write, output_total, output_reasoning, \
                    total_tokens, source_total, reported_call_count, coverage, time_basis, \
                    source_revision \
             FROM source_aggregates WHERE instance_id = ?1 ORDER BY interval_end_ms",
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
        ))
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

#[test]
fn aggregate_write_failure_rolls_back_cursor_and_allows_retry() {
    use llm_usage_core::jobs::RunStatus;
    let dir = TempDir::new("hermes-write-failure");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-basic-cumulative");
    let (_db, storage) = temp_storage("hermes-write-failure");
    storage.conn().execute_batch("CREATE TRIGGER fail_aggregate BEFORE INSERT ON source_aggregates BEGIN SELECT RAISE(ABORT, 'injected aggregate failure'); END;").unwrap();
    let failed = run_hermes(&storage, &root, NOW);
    assert_eq!(failed[0].finish, RunStatus::Failed);
    assert!(failed[0]
        .error
        .as_deref()
        .unwrap()
        .contains("injected aggregate failure"));
    assert_eq!(storage.data_revision().unwrap(), 0);
    let checkpoints: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        checkpoints, 0,
        "failed aggregate must not advance the source cursor"
    );
    assert_eq!(
        llm_usage_core::jobs::run_status(&storage, failed[0].run_id.as_deref().unwrap()).unwrap(),
        Some(RunStatus::Failed)
    );
    assert!(aggregate_rows(&storage, &hermes_instance(&root)).is_empty());
    storage
        .conn()
        .execute_batch("DROP TRIGGER fail_aggregate")
        .unwrap();
    let retry = run_hermes(&storage, &root, NOW + 1);
    assert_eq!(retry[0].finish, RunStatus::Succeeded);
    let rows = aggregate_rows(&storage, &hermes_instance(&root));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].3, Some(100));
    assert_eq!(rows[0].10, Some(3));
    let revision = storage.data_revision().unwrap();
    run_hermes(&storage, &root, NOW + 2);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(aggregate_rows(&storage, &hermes_instance(&root)).len(), 1);
}

#[test]
fn contract_basic_cumulative_row_maps_to_interval_aggregate() {
    let dir = TempDir::new("hermes-basic");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-basic-cumulative");
    let (_db, storage) = temp_storage("hermes-basic");
    let reports = run_hermes(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 1);
    assert_eq!(report.files[0].events, 0, "累计行不产生事件");
    let instance = hermes_instance(&root);

    // 期望见 _expectations.md：1 条区间汇总，全字段对照。
    let rows = aggregate_rows(&storage, &instance);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.0, "session");
    assert_eq!(row.1, Some(1_781_337_600_500), "first_seen 秒→毫秒");
    assert_eq!(row.2, 1_781_341_200_500, "last_seen 秒→毫秒");
    assert_eq!(row.3, Some(100));
    assert_eq!(row.4, Some(50));
    assert_eq!(row.5, Some(10));
    assert_eq!(row.6, Some(40));
    assert_eq!(row.7, Some(5));
    assert_eq!(row.8, Some(200), "未缓存 100 + 读 50 + 写 10 + 输出 40");
    assert_eq!(row.9, None);
    assert_eq!(row.10, Some(3), "api_call_count 作为来源调用汇总");
    assert_eq!(row.11, "exclusive");
    assert_eq!(row.12, "uncertain", "聚合写入边界不是逐请求时间");
    assert_eq!(
        row.13,
        Some(1_781_341_200_500),
        "source_revision = 有效结束毫秒"
    );

    let interval_end_inclusive: i64 = storage
        .conn()
        .query_row(
            "SELECT interval_end_inclusive FROM source_aggregates WHERE instance_id = ?1",
            rusqlite::params![instance],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(interval_end_inclusive, 1, "last_seen 瞬时闭区间语义");

    // api_call_count 不拆 model_call：usage_events 0 条、日汇总零请求零 token。
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
    let sums = summary(&storage, "2026-06-01", "2026-06-30");
    assert_eq!(sums.totals.call_count, 0);
    assert_eq!(sums.totals.input_total_known, None, "不把 100 记入单日");
    assert_eq!(sums.totals.total_tokens_known, None);

    // 互斥求和能力（区间汇总原生范围）。
    let totals = llm_usage_core::aggregates::sum_exclusive_aggregates(&storage, &instance).unwrap();
    assert_eq!(totals.input_uncached, Some(100));
    assert_eq!(totals.input_total, Some(160));
    assert_eq!(totals.input_cache_read, Some(50));
    assert_eq!(totals.input_cache_write, Some(10));
    assert_eq!(totals.output_total, Some(40));
    assert_eq!(totals.output_reasoning, Some(5));
    assert_eq!(totals.total_tokens, Some(200));
    assert_eq!(totals.reported_call_count, Some(3));
    assert_eq!(totals.exclusive_rows, 1);
    assert_eq!(totals.duplicate_rows, 0);

    // 整库 schema_version 不认证逐行客户端版本，仍 latest_fallback。
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
    let _ = dir;
}

#[test]
fn contract_cross_day_row_stays_single_interval_not_day_split() {
    let dir = TempDir::new("hermes-cross-day");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-cross-day");
    let (_db, storage) = temp_storage("hermes-cross-day");
    run_hermes(&storage, &root, NOW);

    let instance = hermes_instance(&root);
    let rows = aggregate_rows(&storage, &instance);
    assert_eq!(rows.len(), 1, "两日累计行保持 1 条区间汇总");
    assert_eq!(rows[0].1, Some(1_790_157_600_000));
    assert_eq!(rows[0].2, 1_790_361_000_000);
    assert_eq!(rows[0].10, Some(3), "api_call_count=3 不拆成 3 条调用");

    // 无详单 ⇒ 三天日汇总均零请求、零 token（不摊分、不落最后一天）。
    let sums = summary(&storage, "2026-09-20", "2026-09-30");
    assert_eq!(sums.totals.call_count, 0);
    assert_eq!(sums.totals.input_total_known, None);
    assert_eq!(sums.totals.total_tokens_known, None);

    let totals = llm_usage_core::aggregates::sum_exclusive_aggregates(&storage, &instance).unwrap();
    assert_eq!(totals.input_uncached, Some(1000));
    assert_eq!(totals.input_total, None);
    assert_eq!(totals.output_total, Some(200));
    assert_eq!(totals.reported_call_count, Some(3));
    let _ = dir;
}

#[test]
fn contract_auxiliary_task_rows_are_exclusive_not_added_twice() {
    let dir = TempDir::new("hermes-aux");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-aux-task-mutex");
    let (_db, storage) = temp_storage("hermes-aux");
    run_hermes(&storage, &root, NOW);

    let instance = hermes_instance(&root);
    let rows = aggregate_rows(&storage, &instance);
    assert_eq!(rows.len(), 2, "主行 + task 辅助行各自一条");
    assert!(rows.iter().all(|r| r.11 == "exclusive"));

    let totals = llm_usage_core::aggregates::sum_exclusive_aggregates(&storage, &instance).unwrap();
    // V03 固定数学样本：100 + 20 = 120，不是 220（辅助不折算进主会话再计一次）。
    assert_eq!(totals.input_uncached, Some(120));
    assert_eq!(totals.input_total, None);
    assert_eq!(totals.reported_call_count, Some(4));
    assert_eq!(totals.exclusive_rows, 2);
    assert_eq!(totals.duplicate_rows, 0);
    assert_eq!(totals.overlap_unknown_rows, 0);
    let _ = dir;
}

#[test]
fn contract_v20_backfill_row_uses_session_window_and_compression_child_not_duplicated() {
    let dir = TempDir::new("hermes-v20");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-v20-backfill-compression");
    let (_db, storage) = temp_storage("hermes-v20");
    run_hermes(&storage, &root, NOW);

    let instance = hermes_instance(&root);
    let rows = aggregate_rows(&storage, &instance);
    assert_eq!(rows.len(), 2);
    // 回填行：first_seen NULL ⇒ interval_start NULL；区间端点回退 session ended_at。
    let backfill = rows
        .iter()
        .find(|r| r.3 == Some(500))
        .expect("backfill row (input=500)");
    assert_eq!(backfill.1, None);
    assert_eq!(backfill.2, 1_789_905_600_000, "回退父会话 ended_at");
    // 子会话实时行照常。
    let live = rows.iter().find(|r| r.3 == Some(300)).expect("live row");
    assert_eq!(live.1, Some(1_789_905_660_500));
    assert_eq!(live.2, 1_789_916_000_000);
    assert_eq!(live.10, Some(2));

    // 压缩继承不双计：500 + 300 = 800（父汇总不复制给子）。
    let totals = llm_usage_core::aggregates::sum_exclusive_aggregates(&storage, &instance).unwrap();
    assert_eq!(totals.input_uncached, Some(800));
    assert_eq!(totals.input_total, None);
    assert_eq!(totals.reported_call_count, Some(2));
    let _ = dir;
}

#[test]
fn capability_table_preserves_real_sample_and_mixed_history_boundaries() {
    let adapter = HermesAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "hermes");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!([]),
        "整库 schema_version 不认证逐行客户端版本"
    );
    assert_eq!(json["discovery"]["env_override"], "HERMES_HOME");
    assert_eq!(
        json["maintenance"]["evidence_level"]
            .as_str()
            .map(|s| s.starts_with("real-local")),
        Some(true),
        "能力声明标注真实 CLI 样本边界",
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
    // 逐次请求/时间不可用（累计表语义），token 为 Partial(待真实样本)。
    assert!(json["fields"]["per_request_calls"]["availability"]
        .get("unavailable")
        .is_some());
    assert!(json["fields"]["time"]["availability"]
        .get("unavailable")
        .is_some());
    assert!(json["fields"]["tokens"]["availability"]
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
    assert!(cap
        .limitations
        .iter()
        .any(|l| l.contains("latest_fallback")));
}

#[test]
fn discover_respects_hermes_home_env_override() {
    let dir = TempDir::new("hermes-env");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-basic-cumulative");
    let adapter = HermesAdapter::new();
    let ctx = DiscoverContext {
        home_dir: None,
        env: std::collections::BTreeMap::from([(
            "HERMES_HOME".to_string(),
            root.to_string_lossy().to_string(),
        )]),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1, "HERMES_HOME 覆盖直接命中 state.db");
    assert!(roots[0].files[0]
        .file_name()
        .is_some_and(|n| n == "state.db"));
    assert!(matches!(
        roots[0].basis,
        llm_usage_core::adapters::framework::RootBasis::EnvOverride(_)
    ));
    let _ = dir;
}

#[cfg(windows)]
#[test]
fn discover_windows_localappdata_default_and_named_profile() {
    let dir = TempDir::new("hermes-winhome");
    // %LOCALAPPDATA%/hermes/state.db + profiles/coder/state.db（带身份标记）。
    let base = dir.path().join("local");
    let home = base.join("hermes");
    std::fs::create_dir_all(home.join("profiles").join("coder")).unwrap();
    {
        let conn = rusqlite::Connection::open(home.join("state.db")).unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    std::fs::write(
        home.join("profiles").join("coder").join("config.yaml"),
        "syn\n",
    )
    .unwrap();
    {
        let conn = rusqlite::Connection::open(home.join("profiles").join("coder").join("state.db"))
            .unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    // 固定源码 _PROFILE_IDENTITY_MARKERS 含 state.db 本身：profiles/<name>/
    // 只要有 state.db 就算 profile 身份（上游对空壳目录另有 servable 判定，
    // 采集层按文件存在即身份）。
    std::fs::create_dir_all(home.join("profiles").join("second")).unwrap();
    {
        let conn =
            rusqlite::Connection::open(home.join("profiles").join("second").join("state.db"))
                .unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    let adapter = HermesAdapter::new();
    let ctx = DiscoverContext {
        home_dir: Some(dir.path().to_path_buf()),
        env: std::collections::BTreeMap::from([(
            "LOCALAPPDATA".to_string(),
            base.to_string_lossy().to_string(),
        )]),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    let mut roots: Vec<_> = roots.into_iter().map(|r| normalize_path(&r.root)).collect();
    roots.sort();
    assert_eq!(
        roots,
        vec![
            normalize_path(&home),
            normalize_path(&home.join("profiles").join("coder")),
            normalize_path(&home.join("profiles").join("second")),
        ],
        "默认 home + 每个带 state.db 的命名 profile 各一个实例"
    );
    let _ = dir;
}

#[cfg(not(windows))]
#[test]
fn discover_unix_default_home_dot_hermes() {
    let dir = TempDir::new("hermes-unixhome");
    let home = dir.path().join(".hermes");
    std::fs::create_dir_all(&home).unwrap();
    {
        let conn = rusqlite::Connection::open(home.join("state.db")).unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    let adapter = HermesAdapter::new();
    let ctx = DiscoverContext {
        home_dir: Some(dir.path().to_path_buf()),
        env: Default::default(),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(normalize_path(&roots[0].root), normalize_path(&home));
    let _ = dir;
}

#[test]
fn discover_from_manual_parent_root_and_empty_yields_none() {
    let dir = TempDir::new("hermes-manual");
    let root = build_hermes_db_from_fixture(&dir, "synthetic-basic-cumulative");
    let adapter = HermesAdapter::new();
    // 手工根传 hermes home 的父目录（用户 home 语义）：有界深度内定位。
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![dir.path().to_path_buf()],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1, "父目录手工根仍定位 state.db");
    assert_eq!(normalize_path(&roots[0].root), normalize_path(&root));

    // 空目录不产出根。
    let empty = TempDir::new("hermes-empty");
    let ctx = DiscoverContext {
        home_dir: Some(empty.path().to_path_buf()),
        env: Default::default(),
        manual_roots: vec![],
    };
    assert!(adapter.discover(&ctx).is_empty());
    let _ = (dir, empty);
}

fn real_context(root: &Path) -> DiscoverContext {
    DiscoverContext {
        home_dir: None,
        env: std::collections::BTreeMap::from([(
            "HERMES_HOME".into(),
            root.to_string_lossy().into_owned(),
        )]),
        manual_roots: vec![],
    }
}

fn real_config(id: &str, now: i64) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: id.into(),
    }
}

fn count_code(storage: &Storage, code: &str) -> i64 {
    storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code=?1",
            [code],
            |r| r.get(0),
        )
        .unwrap()
}

fn check_real(storage: &Storage) {
    let row: [Option<i64>; 8] = storage.conn().query_row("SELECT input_uncached,input_cache_read,input_cache_write,input_total,output_total,output_reasoning,total_tokens,reported_call_count FROM source_aggregates", [], |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?])).unwrap();
    assert_eq!(
        row,
        [
            Some(849),
            Some(812),
            None,
            None,
            Some(4),
            None,
            None,
            Some(2)
        ]
    );
    let quality: String = storage
        .conn()
        .query_row("SELECT quality_json FROM source_aggregates", [], |r| {
            r.get(0)
        })
        .unwrap();
    let quality: llm_usage_core::domain::TokenQuality = serde_json::from_str(&quality).unwrap();
    use llm_usage_core::domain::FieldQuality as Q;
    assert_eq!(
        (
            quality.input_uncached,
            quality.input_cache_read,
            quality.output_total
        ),
        (Q::Reported, Q::Reported, Q::Reported)
    );
    assert_eq!(
        (
            quality.input_total,
            quality.input_cache_write,
            quality.output_reasoning,
            quality.total_tokens
        ),
        (Q::Unknown, Q::Unknown, Q::Unknown, Q::Unknown)
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let totals = summary(storage, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            totals.call_count,
            totals.input_total_known,
            totals.total_tokens_known
        ),
        (0, None, None)
    );
}

#[test]
fn real_cli_resume_matches_api_and_native_buckets_through_entire_registry() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/hermes/real-0.21.5/api-usage.json")).unwrap();
    let cli: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/hermes/real-0.21.5/cli-usage.json")).unwrap();
    let mut uncached = 0;
    for (response, report) in api.as_array().unwrap().iter().zip(cli.as_array().unwrap()) {
        let input = response["usage"]["prompt_tokens"].as_i64().unwrap();
        let cache = response["usage"]["prompt_tokens_details"]["cached_tokens"]
            .as_i64()
            .unwrap();
        assert_eq!(report["input_tokens"], input - cache);
        assert_eq!(report["cache_read_tokens"], cache);
        assert_eq!(
            report["output_tokens"],
            response["usage"]["completion_tokens"]
        );
        assert_eq!(report["total_tokens"], response["usage"]["total_tokens"]);
        assert_eq!(report["completed"], true);
        uncached += input - cache;
    }
    assert_eq!(uncached, 849);
    let dir = TempDir::new("hermes-real");
    let root = build_hermes_db_from_fixture(&dir, "real-0.21.5");
    let original = std::fs::read(root.join("state.db")).unwrap();
    let ctx = real_context(&root);
    let (_db, storage) = temp_storage("hermes-real");
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &real_config(&format!("real-{pass}-{}", adapter.adapter_id()), NOW + pass),
            )
            .unwrap();
            if adapter.adapter_id() == "hermes" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
                assert_eq!(reports[0].files[0].records_seen, 1);
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        check_real(&storage);
        assert_eq!(std::fs::read(root.join("state.db")).unwrap(), original);
    }
    let basis: String = storage
        .conn()
        .query_row("SELECT format_status FROM source_files", [], |r| r.get(0))
        .unwrap();
    let basis: serde_json::Value = serde_json::from_str(&basis).unwrap();
    assert_eq!(basis["basis"], "latest_fallback");
    let instance = hermes_instance(&root);
    let totals = llm_usage_core::aggregates::sum_exclusive_aggregates(&storage, &instance).unwrap();
    assert_eq!(
        (
            totals.input_uncached,
            totals.input_cache_read,
            totals.input_total,
            totals.total_tokens,
            totals.reported_call_count
        ),
        (Some(849), Some(812), None, None, Some(2))
    );
}

struct LegacyHermes(&'static str);
impl SourceAdapter for LegacyHermes {
    fn adapter_id(&self) -> &'static str {
        "hermes"
    }
    fn agent(&self) -> &'static str {
        "hermes-agent"
    }
    fn discover(
        &self,
        ctx: &DiscoverContext,
    ) -> Vec<llm_usage_core::adapters::framework::DiscoveredRoot> {
        HermesAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &llm_usage_core::adapters::framework::DiscoveredRoot) -> String {
        HermesAdapter::new().instance_id(root)
    }
    fn detect(
        &self,
        path: &Path,
    ) -> Result<llm_usage_core::adapters::framework::DetectOutcome, llm_usage_core::error::CoreError>
    {
        HermesAdapter::new().detect(path)
    }
    fn scan(
        &self,
        target: &llm_usage_core::adapters::framework::ScanTarget,
        stored: &llm_usage_core::adapters::framework::StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<llm_usage_core::adapters::framework::ScanOutcome, llm_usage_core::error::CoreError>
    {
        use llm_usage_core::domain::FieldQuality as Q;
        let mut result = HermesAdapter::new().scan(target, stored, limits, now)?;
        for a in &mut result.aggregates {
            a.usage.input_total = Some(a.usage.input_uncached.unwrap_or(0));
            a.quality.input_total = Q::Reported;
            a.usage.input_uncached = None;
            a.quality.input_uncached = Q::Unknown;
            a.usage.total_tokens = None;
            a.quality.total_tokens = Q::Unknown;
            for (value, quality) in [
                (
                    &mut a.usage.input_cache_read,
                    &mut a.quality.input_cache_read,
                ),
                (
                    &mut a.usage.input_cache_write,
                    &mut a.quality.input_cache_write,
                ),
                (&mut a.usage.output_total, &mut a.quality.output_total),
                (
                    &mut a.usage.output_reasoning,
                    &mut a.quality.output_reasoning,
                ),
            ] {
                *value = Some(value.unwrap_or(0));
                *quality = Q::Reported;
            }
            a.reported_call_count = Some(a.reported_call_count.unwrap_or(0));
            match self.0 {
                "input" => a.usage.input_total = Some(850),
                "output" => a.usage.output_total = Some(5),
                "quality" => a.quality.output_total = Q::Derived,
                "coverage" => a.coverage = llm_usage_core::aggregates::Coverage::OverlapUnknown,
                "time" => a.interval_start_ms = a.interval_start_ms.map(|v| v + 1),
                "revision" => a.source_revision = a.source_revision.map(|v| v + 1),
                "calls" => a.reported_call_count = Some(3),
                _ => {}
            }
        }
        Ok(result)
    }
    fn capability(&self) -> llm_usage_core::adapters::framework::CapabilityTable {
        let mut c = HermesAdapter::new().capability();
        c.maintenance["parser_version"] = "hermes-session-model-usage-1".into();
        c
    }
}

#[test]
fn real_unchanged_old_watermark_replays_exact_policy_and_rolls_back_checkpoint_failure() {
    use llm_usage_core::adapters::framework::{
        run_adapter_scans_parallel, InstanceFilter, ParallelScanRequest,
    };
    for fail_once in [false, true] {
        let dir = TempDir::new("hermes-upgrade");
        let root = build_hermes_db_from_fixture(&dir, "real-0.21.5");
        let original = std::fs::read(root.join("state.db")).unwrap();
        let ctx = real_context(&root);
        let (_db, storage) = temp_storage("hermes-upgrade");
        run_adapter_scan(
            &storage,
            &LegacyHermes("exact"),
            &ctx,
            &real_config("old", NOW),
        )
        .unwrap();
        let old: (String,String,i64,i64) = storage.conn().query_row("SELECT aggregate_id,content_hash,source_revision,created_at_ms FROM source_aggregates",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        // Beyond overlap: only the parser reset can revisit the unchanged old row.
        storage.conn().execute("UPDATE ingestion_checkpoints SET cursor_value=json_set(cursor_value,'$.watermark_ms',?1)",[NOW]).unwrap();
        storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','prior diagnostic',?1 FROM source_instances",[NOW]).unwrap();
        if fail_once {
            storage.conn().execute_batch("CREATE TRIGGER fail_hermes_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
            let failed = run_adapter_scan(
                &storage,
                &HermesAdapter::new(),
                &ctx,
                &real_config("failed", NOW + 1),
            )
            .unwrap();
            assert_eq!(failed[0].finish, llm_usage_core::jobs::RunStatus::Failed);
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT content_hash FROM source_aggregates", [], |r| r
                        .get::<_, String>(
                        0
                    ))
                    .unwrap(),
                old.1
            );
            assert_eq!(count_code(&storage, "aggregate_parser_policy_upgrade"), 0);
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            storage
                .conn()
                .execute_batch("DROP TRIGGER fail_hermes_checkpoint")
                .unwrap();
        }
        let adapter = HermesAdapter::new();
        let requests = [ParallelScanRequest {
            adapter: &adapter,
            context: ctx.clone(),
            config: real_config("upgrade", NOW + 2),
            filter: InstanceFilter::default(),
        }];
        let locked = std::sync::Mutex::new(storage);
        let results = run_adapter_scans_parallel(
            &locked,
            &requests,
            None,
            std::sync::Arc::new(|| true),
            None,
        );
        let reports = results.into_iter().next().unwrap().unwrap();
        assert!(reports.iter().all(|r| r.error.is_none()));
        let storage = locked.into_inner().unwrap();
        check_real(&storage);
        let current: (String, i64, i64) = storage
            .conn()
            .query_row(
                "SELECT aggregate_id,source_revision,created_at_ms FROM source_aggregates",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(current, (old.0, old.2, old.3));
        let revision = storage.data_revision().unwrap();
        run_adapter_scan(&storage, &adapter, &ctx, &real_config("repeat", NOW + 3)).unwrap();
        check_real(&storage);
        assert_eq!(storage.data_revision().unwrap(), revision);
        assert_eq!(count_code(&storage, "aggregate_parser_policy_upgrade"), 1);
        assert_eq!(count_code(&storage, "aggregate_conflict"), 0);
        assert_eq!(count_code(&storage, "preserved_history"), 1);
        assert_eq!(std::fs::read(root.join("state.db")).unwrap(), original);
    }
}

#[test]
fn hermes_policy_correction_protects_positive_counts_quality_coverage_time_and_revision() {
    for variant in [
        "input", "output", "quality", "coverage", "time", "revision", "calls",
    ] {
        let dir = TempDir::new("hermes-conflict");
        let root = build_hermes_db_from_fixture(&dir, "real-0.21.5");
        let ctx = real_context(&root);
        let (_db, storage) = temp_storage("hermes-conflict");
        run_adapter_scan(
            &storage,
            &LegacyHermes(variant),
            &ctx,
            &real_config("old", NOW),
        )
        .unwrap();
        let old: String = storage
            .conn()
            .query_row("SELECT content_hash FROM source_aggregates", [], |r| {
                r.get(0)
            })
            .unwrap();
        run_adapter_scan(
            &storage,
            &HermesAdapter::new(),
            &ctx,
            &real_config("current", NOW + 1),
        )
        .unwrap();
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT content_hash FROM source_aggregates", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            old,
            "{variant}"
        );
        assert_eq!(
            count_code(&storage, "aggregate_parser_policy_upgrade"),
            0,
            "{variant}"
        );
        assert_eq!(
            count_code(&storage, "aggregate_conflict"),
            i64::from(variant != "revision"),
            "{variant}"
        );
    }
}

#[test]
fn bad_type_and_negative_synthetic_rows_retain_valid_unknown_version_aggregates() {
    let dir = TempDir::new("hermes-bad-type");
    let root = build_hermes_db_from_fixture(&dir, "real-0.21.5");
    let db = rusqlite::Connection::open(root.join("state.db")).unwrap();
    // Fault controls in a disposable copy; the committed real fixture is untouched.
    for (task, input, end) in [
        (
            "bad-type",
            rusqlite::types::Value::Text("invalid".into()),
            1791281510.0,
        ),
        (
            "negative",
            rusqlite::types::Value::Integer(-1),
            1791281511.0,
        ),
    ] {
        db.execute("INSERT INTO session_model_usage(session_id,model,billing_provider,billing_base_url,billing_mode,task,input_tokens,first_seen,last_seen) SELECT session_id,model,billing_provider,billing_base_url,billing_mode,?1,?2,first_seen,?3 FROM session_model_usage WHERE task=''",rusqlite::params![task,input,end]).unwrap();
    }
    drop(db);
    let (_db, storage) = temp_storage("hermes-bad-type");
    let reports = run_hermes(&storage, &root, NOW);
    assert!(reports[0].error.is_none());
    assert_eq!(reports[0].files[0].records_seen, 3);
    check_real(&storage);
    assert_eq!(count_code(&storage, "invalid_row_type"), 1);
    assert_eq!(count_code(&storage, "negative_counter"), 1);
    assert_eq!(count_code(&storage, "unknown_version_fallback_failed"), 0);
    let health: String = storage
        .conn()
        .query_row("SELECT health FROM source_instances", [], |r| r.get(0))
        .unwrap();
    assert_eq!(health, "degraded");
}

#[test]
fn all_default_zero_synthetic_row_remains_unknown_including_call_count() {
    let dir = TempDir::new("hermes-default-zero");
    let root = build_hermes_db_from_fixture(&dir, "real-0.21.5");
    let db = rusqlite::Connection::open(root.join("state.db")).unwrap();
    db.execute_batch("UPDATE session_model_usage SET input_tokens=0,output_tokens=0,cache_read_tokens=0,cache_write_tokens=0,reasoning_tokens=0,api_call_count=0").unwrap();
    drop(db);
    let (_db, storage) = temp_storage("hermes-default-zero");
    run_hermes(&storage, &root, NOW);
    let values: [Option<i64>;8] = storage.conn().query_row("SELECT input_uncached,input_cache_read,input_cache_write,input_total,output_total,output_reasoning,total_tokens,reported_call_count FROM source_aggregates",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?])).unwrap();
    assert_eq!(values, [None; 8]);
}
