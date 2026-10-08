//! Read-only checks of native Visual Studio Copilot telemetry
//! at %TEMP%\VSGitHubCopilotLogs\traces\*.jsonl.
//! First run: 2026-10-01, VS 18 Community, two chat spans.
//! Output only permitted statistics: files, events, tokens, distinct models, diagnostics,
//! and rescan stability; no paths, trace/span IDs or attribute bodies.
//! Usage: cargo run -p llm-usage-core --example real_verify_vs_copilot -- <traces-dir-or-jsonl-or-log-root> <work_dir>

use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::vs_copilot::VsCopilotAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

type UsageSummary = (
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    i64,
);

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: <traces_root_or_file> <work_dir>");
    let work = std::env::args()
        .nth(2)
        .expect("usage: <traces_root_or_file> <work_dir>");
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check-vs-copilot.sqlite");
    let _ = std::fs::remove_file(&db_path);
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = VsCopilotAdapter::new();
    let now_ms = 1_800_000_000_000_i64;
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![PathBuf::from(root)],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: "real-vs-copilot".to_string(),
        origin_host_id: None,
    };
    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    for report in &reports {
        println!(
            "files={} events={} diagnostics={}",
            report.files.len(),
            report.outcome.as_ref().map(|o| o.added).unwrap_or(0),
            report.files.iter().map(|f| f.diagnostics).sum::<u64>(),
        );
    }
    let conn = storage.conn();
    let (count, in_sum, out_sum, cr_sum, cw_sum, r_sum, models): UsageSummary = conn
        .query_row(
            "SELECT COUNT(*), SUM(input_total), SUM(output_total),
                    SUM(input_cache_read), SUM(input_cache_write),
                    SUM(output_reasoning), COUNT(DISTINCT model_raw)
             FROM usage_events WHERE agent='vs-copilot'",
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
        .expect("query");
    println!(
        "collected: count={count} input={in_sum:?} output={out_sum:?} cache_read={cr_sum:?} cache_write={cw_sum:?} reasoning={r_sum:?} models={models}"
    );
    let reports2 = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("rescan");
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE agent='vs-copilot'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    println!(
        "rescan: count={count2} added2={added2} idempotent={}",
        count2 == count
    );
    println!(
        "VERDICT: {}",
        if count2 == count && count > 0 && added2 == 0 {
            "PASS"
        } else {
            "CHECK"
        }
    );
}
