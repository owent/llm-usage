//! Read-only local WorkBuddy checks; print statistics, diagnostic codes and repeat-read results.
//! Usage: cargo run -p llm-usage-core --example real_verify_workbuddy -- <home> <work_dir>

use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::workbuddy::WorkBuddyAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn main() {
    let home = PathBuf::from(std::env::args().nth(1).expect("home argument"));
    let work = PathBuf::from(std::env::args().nth(2).expect("work directory argument"));
    std::fs::create_dir_all(&work).expect("create work dir");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let db_path = work.join(format!(
        "real-check-workbuddy-{}-{nonce}.sqlite",
        std::process::id()
    ));
    let storage = Storage::open(&db_path).expect("open storage");
    let adapter = WorkBuddyAdapter::new();
    let ctx = DiscoverContext {
        home_dir: Some(home),
        env: Default::default(),
        manual_roots: vec![],
    };
    let config = RunConfig {
        timezone: "UTC".into(),
        now_ms: 1_800_000_000_000,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: "real-workbuddy".into(),
        origin_host_id: None,
    };
    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    let files: usize = reports.iter().map(|r| r.files.len()).sum();
    let added: i64 = reports
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    let diagnostics: u64 = reports
        .iter()
        .flat_map(|r| &r.files)
        .map(|f| f.diagnostics)
        .sum();
    let conn = storage.conn();
    let (count, input, output, cache_read, uncached, total): (i64, i64, i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_total),0), COALESCE(SUM(output_total),0), COALESCE(SUM(input_cache_read),0), COALESCE(SUM(input_uncached),0), COALESCE(SUM(total_tokens),0) FROM usage_events WHERE agent='workbuddy'",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
    ).expect("query");
    println!("files={files} added={added} diagnostics={diagnostics} count={count} input={input} output={output} cache_read={cache_read} uncached={uncached} total={total}");
    let reports2 = run_adapter_scan(
        &storage,
        &adapter,
        &ctx,
        &RunConfig {
            run_id_prefix: "real-workbuddy-repeat".into(),
            ..config
        },
    )
    .expect("rescan");
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE agent='workbuddy'",
            [],
            |r| r.get(0),
        )
        .expect("query repeat");
    println!(
        "rescan_added={added2} rescan_count={count2} verdict={}",
        if files > 0
            && count > 0
            && added2 == 0
            && count2 == count
            && input == uncached + cache_read
            && total == input + output
        {
            "PASS"
        } else {
            "CHECK"
        }
    );
    drop(storage);
    std::fs::remove_file(db_path).expect("remove temporary verification database");
}
