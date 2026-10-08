//! Zoo Code adapter tests use synthetic data based on A19 fixed source f780647.
//! The 2026-09-25 local survey found no installation. Exercise
//! reading, consolidateApiRequests LIFO merging and consolidateTokenUsage accounting,
//! then commit/query. Compare values with manually calculated
//! tests/fixtures/zoo/*/_expectations.md. Helpers remain local to this file.

mod common;

use common::{summary, temp_storage};
use llm_usage_core::adapters::framework::{
    normalize_path, run_adapter_scan, DetectOutcome, DiscoverContext, RunConfig, ScanLimits,
    SourceAdapter, SourceRunReport,
};
use llm_usage_core::adapters::zoo::ZooAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn zoo_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("zoo")
        .join(name)
}

fn run_zoo(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    let adapter = ZooAdapter::new();
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
    assert_eq!(reports.len(), 1);
    reports
}

fn zoo_instance(root: &Path) -> String {
    // Discovery uses tasks as the instance root, resolving a manual parent's tasks child.
    format!("zoo@{}", normalize_path(&root.join("tasks")))
}

#[test]
fn contract_full_chain_matches_manual_expectations() {
    let (_dir, storage) = temp_storage("zoo-contract");
    let root = zoo_fixture("synthetic-contract");
    let reports = run_zoo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(
        report.files[0].records_seen, 6,
        "全部消息计数（含被合并的 finished）"
    );
    assert_eq!(report.files[0].events, 3, "两对合并请求 + 一条 condense");
    let instance = zoo_instance(&root);

    let conn = storage.conn();
    // Request 1: merge started+finished; finished fields override started fields.
    type Req1Row = (
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let req1: Req1Row = conn
        .query_row(
            "SELECT call_category, input_total, input_cache_read, input_cache_write, \
                    input_uncached, output_total, total_tokens, cost_amount_minor, \
                    source_revision, time_basis \
             FROM usage_events WHERE source_instance_id = ?1 AND source_record_key = ?2",
            rusqlite::params![instance, "syn-zoo-1:api_req_started:1781337600500"],
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
                ))
            },
        )
        .unwrap();
    assert_eq!(req1.0, "primary");
    assert_eq!(req1.1, Some(1000), "tokensIn 含缓存：input_total 直报");
    assert_eq!(req1.2, Some(8000), "cacheReads 是 tokensIn 子集");
    assert_eq!(req1.3, Some(100));
    assert_eq!(req1.4, None, "精确包含集合未证不拆未缓存输入");
    assert_eq!(req1.5, Some(200));
    assert_eq!(req1.6, Some(1200), "上游 contextTokens 算术 in+out");
    assert_eq!(req1.7, Some(12_000), "cost（合并后）micro-USD estimated");
    assert_eq!(req1.8, None);
    assert_eq!(req1.9, Some("source_start".to_string()));

    // Request 2: merge the second pair.
    type Req2Row = (Option<i64>, Option<i64>, Option<i64>);
    let req2: Req2Row = conn
        .query_row(
            "SELECT input_total, total_tokens, cost_amount_minor \
             FROM usage_events WHERE source_instance_id = ?1 AND source_record_key = ?2",
            rusqlite::params![instance, "syn-zoo-1:api_req_started:1781339000000"],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(req2.0, Some(500));
    assert_eq!(req2.1, Some(580));
    assert_eq!(req2.2, Some(6_000));

    // condense_context is auxiliary; tokens remain unknown and cost is mapped.
    type CondenseRow = (
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let condense: CondenseRow = conn
        .query_row(
            "SELECT call_category, input_total, output_total, total_tokens, \
                    cost_amount_minor, source_revision, time_basis \
             FROM usage_events WHERE source_instance_id = ?1 AND source_record_key = ?2",
            rusqlite::params![instance, "syn-zoo-1:condense_context:1781338000000"],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(condense.0, "auxiliary");
    assert_eq!(condense.1, None, "contextCondense 无 token 字段，不补零");
    assert_eq!(condense.2, None);
    assert_eq!(condense.3, None);
    assert_eq!(
        condense.4,
        Some(3_000),
        "contextCondense.cost 计入上游 totalCost"
    );
    assert_eq!(condense.5, None);
    assert_eq!(condense.6, Some("uncertain".to_string()));

    // Compare totals with _expectations.md for 2026-06-13 UTC.
    let s = summary(&storage, "2026-06-13", "2026-06-13");
    assert_eq!(s.totals.call_count, 3, "2 primary + 1 auxiliary");
    assert_eq!(s.totals.input_total_known, Some(1500));
    assert_eq!(s.totals.cache_read_known, Some(17000));
    assert_eq!(s.totals.cache_write_known, Some(100));
    assert_eq!(s.totals.output_total_known, Some(280));
    assert_eq!(s.totals.total_tokens_known, Some(1780));

    // An unmatched started record creates one diagnostic and no event.
    let diag: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'usage_carrier_without_numbers'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);
    // The document format is KnownVersion; no latest_fallback diagnostic.
    let fallback: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'latest_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fallback, 0);

    // Repeated scanning adds no usage.
    run_zoo(&storage, &root, NOW + 1_000);
    let s = summary(&storage, "2026-06-13", "2026-06-13");
    assert_eq!(s.totals.call_count, 3, "重复扫描不增量");
}

#[test]
fn undocumented_say_kind_fails_closed_whole_file() {
    let (_dir, storage) = temp_storage("zoo-undocumented");
    let root = zoo_fixture("synthetic-undocumented-say");
    let reports = run_zoo(&storage, &root, NOW);
    // type/say fingerprint enables scanning; an undocumented say kind rejects the entire file.
    assert_eq!(
        reports[0].files[0].status, "pending",
        "fail closed：游标不推进"
    );
    assert_eq!(reports[0].files[0].events, 0);
    assert!(reports[0].files[0].diagnostics >= 1);
    let codes: Vec<String> = {
        let conn = storage.conn();
        let mut stmt = conn.prepare("SELECT code FROM diagnostics").unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert!(codes.iter().any(|c| c == "undocumented_say_kind"));
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
}

#[test]
fn not_array_detects_unknown_format() {
    let (_dir, storage) = temp_storage("zoo-not-array");
    let root = zoo_fixture("synthetic-not-array");
    let adapter = ZooAdapter::new();
    // Direct detection: a prefix other than [ is UnknownFormat.
    let file = root
        .join("tasks")
        .join("syn-zoo-3")
        .join("ui_messages.json");
    let outcome = adapter.detect(&file).unwrap();
    assert!(matches!(outcome, DetectOutcome::UnknownFormat { .. }));
    // Full scan: detection rejects the file as unknown_format, with zero events.
    let reports = run_zoo(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "unknown_format");
    assert_eq!(reports[0].files[0].events, 0);
}

#[test]
fn discover_cli_default_and_vscode_global_storage_and_manual() {
    use llm_usage_core::adapters::framework::RootBasis;
    let dir = common::TempDir::new("zoo-discover");
    // CLI default: ~/.vscode-mock/global-storage/tasks/<id>/ui_messages.json.
    let cli_home = dir.path().join("home1");
    let cli_tasks = cli_home
        .join(".vscode-mock")
        .join("global-storage")
        .join("tasks")
        .join("syn-zoo-cli");
    std::fs::create_dir_all(&cli_tasks).unwrap();
    std::fs::write(cli_tasks.join("ui_messages.json"), "[]").unwrap();
    let adapter = ZooAdapter::new();
    let ctx = DiscoverContext {
        home_dir: Some(cli_home.clone()),
        env: Default::default(),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1, "CLI 缺省存储命中");
    assert!(roots[0]
        .files
        .iter()
        .any(|f| f.to_string_lossy().contains("syn-zoo-cli")));
    assert!(matches!(roots[0].basis, RootBasis::DefaultHome));

    // VS Code extension globalStorage under the Windows APPDATA layout.
    let appdata = dir.path().join("appdata");
    let vscode_tasks = appdata
        .join("Code")
        .join("User")
        .join("globalStorage")
        .join("zoocodeorganization.zoo-code")
        .join("tasks")
        .join("syn-zoo-vscode");
    std::fs::create_dir_all(&vscode_tasks).unwrap();
    std::fs::write(vscode_tasks.join("ui_messages.json"), "[]").unwrap();
    let ctx = DiscoverContext {
        home_dir: Some(cli_home.clone()),
        env: std::collections::BTreeMap::from([(
            "APPDATA".to_string(),
            appdata.to_string_lossy().to_string(),
        )]),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 2, "CLI 缺省 + VS Code globalStorage 各一个根");
    let normalized: Vec<String> = roots.iter().map(|r| normalize_path(&r.root)).collect();
    assert!(normalized
        .iter()
        .any(|p| p.contains("zoocodeorganization.zoo-code")));

    // Manual roots also resolve to tasks, as separately implemented by Cline.
    let manual_root = zoo_fixture("synthetic-contract");
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![manual_root.clone()],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(
        normalize_path(&roots[0].root),
        normalize_path(&manual_root.join("tasks"))
    );
    let _ = dir;
}

#[test]
fn capability_table_separates_real_sample_from_other_paths() {
    let adapter = ZooAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "zoo");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["zoo-ui-messages-doc-1"]),
        "文档级锚点唯一条目（待真实样本）"
    );
    assert_eq!(
        json["maintenance"]["evidence_level"]
            .as_str()
            .map(|s| s.starts_with("real-container")),
        Some(true),
        "真实容器样本与其余文档级路径分开声明",
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
    assert!(json["fields"]["model"]["availability"]
        .get("unavailable")
        .is_some());
    assert!(!cap.limitations.is_empty());
    assert!(cap.limitations.iter().any(|l| l.contains("仅 3.86.0")));
    assert!(cap
        .limitations
        .iter()
        .any(|l| l.contains("CLI") && l.contains("未实测")));
    // Zoo is an independent product; Roo Code rules cannot replace its A19 rules.
    assert!(json["product"]
        .as_str()
        .is_some_and(|p| p.contains("独立产品")));
}
