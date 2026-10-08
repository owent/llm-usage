//! Legacy samples with invented schemas remain rejected after adding the native
//! schema 24 reader; runtime acceptance is in openclaw_runtime_contract.

mod common;

use common::temp_storage;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::adapters::openclaw::OpenClawAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn openclaw_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("openclaw")
        .join(name)
}

/// Rebuild layout.json in a temporary directory: placeholder SQLite tables and JSON/JSONL data.
fn rebuild_layout(dir: &Path, layout: &serde_json::Value) {
    for file in layout["files"].as_array().unwrap() {
        let path = dir.join(file["path"].as_str().unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        match file["kind"].as_str().unwrap() {
            "sqlite" => {
                let conn = rusqlite::Connection::open(&path).unwrap();
                for ddl in file["placeholder_tables"].as_array().unwrap() {
                    conn.execute_batch(ddl.as_str().unwrap()).unwrap();
                }
            }
            "json" => {
                std::fs::write(&path, serde_json::to_vec(&file["content"]).unwrap()).unwrap();
            }
            "jsonl" => {
                let mut out = Vec::new();
                for record in file["records"].as_array().unwrap() {
                    out.extend_from_slice(serde_json::to_string(record).unwrap().as_bytes());
                    out.push(b'\n');
                }
                std::fs::write(&path, out).unwrap();
            }
            other => panic!("unknown layout kind {other}"),
        }
    }
}

fn rebuild_from_fixture(name: &str) -> (common::TempDir, PathBuf) {
    let dir = common::TempDir::new("openclaw");
    let layout: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(openclaw_fixture(&format!("{name}/layout.json"))).unwrap(),
    )
    .unwrap();
    let root = dir.path().join(".openclaw");
    std::fs::create_dir_all(&root).unwrap();
    rebuild_layout(&root, &layout);
    (dir, root)
}

fn run_openclaw(storage: &Storage, root: &Path, now_ms: i64) -> usize {
    let adapter = OpenClawAdapter::new();
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
    assert_eq!(reports.len(), 1, "发现 agents/main 实例根");
    reports[0].files.len()
}

#[test]
fn invented_runtime_schema_still_fails_closed() {
    let (_dir, storage) = temp_storage("openclaw-runtime");
    let (dir, root) = rebuild_from_fixture("synthetic-runtime-store");
    let files = run_openclaw(&storage, &root, NOW);
    assert!(files >= 1, "库文件被定位");
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "无文档 schema ⇒ 不读表不产零值");
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unknown_format'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(diags >= 1, "fail closed 有诊断（不是成功 0 条）");
    // Repeat scans retain the same state.
    run_openclaw(&storage, &root, NOW + 1_000);
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
    let _ = dir;
}

#[test]
fn legacy_archive_is_migration_input_not_usage_source() {
    let (_dir, storage) = temp_storage("openclaw-archive");
    let (dir, root) = rebuild_from_fixture("synthetic-legacy-archive");
    let files = run_openclaw(&storage, &root, NOW);
    assert!(files >= 2, "归档两文件都被定位");
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "1200+300/60+15 等占位数值不入账");
    let _ = dir;
}

#[test]
fn capability_records_real_cli_scope_and_unverified_boundaries() {
    let cap = OpenClawAdapter::new().capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "openclaw");
    let text = serde_json::to_string(&json).unwrap();
    assert!(text.contains("待证") || text.contains("fail closed") || text.contains("文档"));
    assert!(!cap.limitations.is_empty());
    assert_eq!(json["maintenance"]["evidence_level"], "real-local-cli");
    assert!(
        cap.supported_versions.is_empty(),
        "whole-db app version cannot certify historical rows"
    );
}
