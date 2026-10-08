//! Read-only checks of native GitHub Copilot CLI data from ~/.copilot/session-store.db,
//! assistant_usage_events; first run on 2026-09-29 contained 36 native rows.
//! Output only permitted root/record/event/token/diagnostic counts
//! and rescan stability; do not print paths, session IDs or record content.
//! Usage: cargo run -p llm-usage-core --example real_verify_copilot -- <copilot-root-or-session-store.db> <work_dir>

use llm_usage_core::adapters::copilot::CopilotAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: <copilot_root_or_db> <work_dir>");
    let work = std::env::args()
        .nth(2)
        .expect("usage: <copilot_root_or_db> <work_dir>");
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check-copilot.sqlite");
    let _ = std::fs::remove_file(&db_path);
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = CopilotAdapter::new();
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
        run_id_prefix: "real-copilot".to_string(),
        origin_host_id: None,
    };
    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    for report in &reports {
        // Do not print instance_id: it contains local paths excluded by the output rules above.
        println!(
            "files={} events={} diagnostics={}",
            report.files.len(),
            report.outcome.as_ref().map(|o| o.added).unwrap_or(0),
            report.files.iter().map(|f| f.diagnostics).sum::<u64>(),
        );
    }
    let conn = storage.conn();
    let (count, in_sum, out_sum, cr_sum, cw_sum, r_sum, unc_sum, total_sum): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT COUNT(*), SUM(input_total), SUM(output_total),
                    SUM(input_cache_read), SUM(input_cache_write),
                    SUM(output_reasoning), SUM(input_uncached), SUM(total_tokens)
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
                    r.get(7)?,
                ))
            },
        )
        .expect("query");
    println!(
        "collected: count={count} input={in_sum} output={out_sum} cache_read={cr_sum} cache_write={cw_sum} reasoning={r_sum} uncached={unc_sum} total={total_sum}"
    );
    // Repeated scans add no duplicates.
    let reports2 = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("rescan");
    let _ = reports2;
    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE agent='copilot-cli'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    println!("rescan: count={count2} idempotent={}", count2 == count);
    println!(
        "VERDICT: {}",
        if count2 == count && count > 0 {
            "PASS"
        } else {
            "CHECK"
        }
    );
}
