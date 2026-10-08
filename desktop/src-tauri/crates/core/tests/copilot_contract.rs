//! Copilot CLI format tests: redacted native schema_version=8 sample,
//! extracted 2026-09-29, 36 rows; rebuild SQLite→discover/detect/scan→commit→
//! query; compare manual expectations in tests/fixtures/copilot/_expectations.md.

mod common;

use common::temp_storage;
use llm_usage_core::adapters::copilot::CopilotAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DetectOutcome, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::jobs::TriggerKind;
use std::path::PathBuf;

const NOW: i64 = 1_800_000_000_000;

fn fixture_json() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("copilot")
        .join("assistant-usage-events-v8.sanitized.json")
}

/// Rebuild native-layout SQLite from the redacted sample.
fn build_store(dir: &std::path::Path) -> PathBuf {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(fixture_json()).unwrap()).unwrap();
    let copilot_dir = dir.join(".copilot");
    std::fs::create_dir_all(&copilot_dir).unwrap();
    let db = copilot_dir.join("session-store.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE schema_version (version INTEGER);
         INSERT INTO schema_version VALUES (8);
         CREATE TABLE assistant_usage_events (
           id INTEGER PRIMARY KEY, session_id TEXT, turn_index INTEGER,
           agent_id TEXT, parent_tool_call_id TEXT, model TEXT,
           input_tokens INTEGER, output_tokens INTEGER,
           cache_read_tokens INTEGER, cache_write_tokens INTEGER,
           reasoning_tokens INTEGER, total_nano_aiu REAL, request_multiplier REAL,
           duration_ms REAL, time_to_first_token_ms REAL, initiator TEXT,
           api_endpoint TEXT, reasoning_effort TEXT, finish_reason TEXT,
           content_filter_triggered INTEGER, token_details_json TEXT,
           created_at TEXT, output_ttft_ms REAL, copilot_usage_model TEXT);",
    )
    .unwrap();
    for row in &rows {
        conn.execute(
            "INSERT INTO assistant_usage_events (id, session_id, turn_index, agent_id, model,
             input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
             reasoning_tokens, total_nano_aiu, request_multiplier, duration_ms,
             time_to_first_token_ms, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            rusqlite::params![
                row["id"].as_i64(),
                row["session_id"].as_str(),
                row["turn_index"].as_i64(),
                row["agent_id"].as_str(),
                row["model"].as_str(),
                row["input_tokens"].as_i64(),
                row["output_tokens"].as_i64(),
                row["cache_read_tokens"].as_i64(),
                row["cache_write_tokens"].as_i64(),
                row["reasoning_tokens"].as_i64(),
                row["total_nano_aiu"].as_f64(),
                row["request_multiplier"].as_f64(),
                row["duration_ms"].as_f64(),
                row["time_to_first_token_ms"].as_f64(),
                row["created_at"].as_str(),
            ],
        )
        .unwrap();
    }
    db
}

fn run(storage: &llm_usage_core::storage::Storage, root: &std::path::Path, now_ms: i64) {
    let adapter = CopilotAdapter::new();
    let ctx = DiscoverContext {
        home_dir: Some(root.to_path_buf()),
        env: Default::default(),
        manual_roots: Vec::new(),
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
}

#[test]
fn contract_matches_manual_expectations_and_idempotent() {
    let dir = std::env::temp_dir().join(format!("llm-usage-copilot-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = build_store(&dir);

    // schema_version=8 selects KnownVersion.
    let adapter = CopilotAdapter::new();
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported { .. }
    ));

    let (storage_dir, storage) = temp_storage("copilot-contract");
    run(&storage, &dir, NOW);
    let conn = storage.conn();
    let (count, in_sum, out_sum, cr_sum, cw_sum, unc_sum, total_sum): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(input_total), SUM(output_total),
                    SUM(input_cache_read), SUM(input_cache_write),
                    SUM(input_uncached), SUM(total_tokens)
             FROM usage_events WHERE agent='copilot-cli'",
            [],
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
    // Manually calculated tests/fixtures/copilot/_expectations.md expectations.
    assert_eq!(count, 36);
    assert_eq!(in_sum, 4_649_981);
    assert_eq!(out_sum, 34_157);
    assert_eq!(cr_sum, 4_416_791);
    assert_eq!(cw_sum, 233_118);
    assert_eq!(unc_sum, 72, "input−read−write 派生");
    assert_eq!(total_sum, 4_684_138, "input+output 派生");
    // Reasoning is reported separately, outside derived total.
    let reasoning_sum: i64 = conn
        .query_row(
            "SELECT SUM(output_reasoning) FROM usage_events WHERE agent='copilot-cli'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reasoning_sum, 17_678);
    // Latency fields are present.
    let with_ttft: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE agent='copilot-cli' AND ttft_ms IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(with_ttft > 0);

    // V12 repeated scans add no duplicates.
    run(&storage, &dir, NOW + 60_000);
    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE agent='copilot-cli'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count2, 36, "重复扫描不增量");

    let _ = std::fs::remove_dir_all(&dir);
    drop(storage_dir);
}
