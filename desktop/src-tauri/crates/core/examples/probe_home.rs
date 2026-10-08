//! Read-only source probe: run all or selected built-in adapters for a supplied home,
//! discover→detect→scan→commit in memory, without disk writes; print instance results.
//! Checks real WSL/external-home data and diagnoses missing sources.
//!
//! Usage: cargo run --release -p llm-usage-core --example probe_home -- <home> [adapter_id ...]
//! Windows example for a WSL home: ... -- '\\wsl.localhost\Debian\home\<user>'
//! Or select adapters: ... -- /home/<user> opencode claude gemini qwen.
use llm_usage_core::adapters::built_in_adapters;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: probe_home <home-dir> [adapter_id ...]");
        std::process::exit(2);
    }
    let home = std::path::PathBuf::from(&args[0]);
    let only: Vec<String> = args[1..].to_vec();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let storage = Storage::open_in_memory().expect("open in-memory storage");
    let mut total_adapters = 0usize;
    let mut reports_total = 0usize;
    for adapter in built_in_adapters() {
        let id = adapter.adapter_id().to_string();
        if !only.is_empty() && !only.contains(&id) {
            continue;
        }
        total_adapters += 1;
        let ctx = DiscoverContext {
            home_dir: Some(home.clone()),
            env: Default::default(),
            manual_roots: vec![],
        };
        let config = RunConfig {
            timezone: "UTC".to_string(),
            now_ms: now,
            limits: ScanLimits::default(),
            trigger: TriggerKind::Manual,
            origin_host_id: None,
            run_id_prefix: format!("probe-{now}-{id}"),
        };
        match run_adapter_scan(&storage, adapter.as_ref(), &ctx, &config) {
            Ok(reports) => {
                if reports.is_empty() {
                    println!("[{id}] no roots discovered");
                }
                for r in &reports {
                    reports_total += 1;
                    let (added, updated, unchanged, skipped, errors, conflicts) = r
                        .outcome
                        .as_ref()
                        .map(|o| {
                            (
                                o.added,
                                o.updated,
                                o.unchanged,
                                o.skipped,
                                o.errors,
                                o.conflicts,
                            )
                        })
                        .unwrap_or((0, 0, 0, 0, 0, 0));
                    println!(
                        "[{id}] instance={} finish={:?} files={} events: added={} updated={} unchanged={} skipped={} errors={} conflicts={}{}",
                        r.instance_id,
                        r.finish,
                        r.files.len(),
                        added,
                        updated,
                        unchanged,
                        skipped,
                        errors,
                        conflicts,
                        r.error.as_ref().map(|e| format!(" error={e}")).unwrap_or_default(),
                    );
                    for f in &r.files {
                        println!(
                            "    file {} {} (seen={} events={}){}",
                            f.file_id,
                            f.status,
                            f.records_seen,
                            f.events,
                            f.detail
                                .as_ref()
                                .map(|d| format!(" detail={d}"))
                                .unwrap_or_default(),
                        );
                    }
                }
            }
            Err(e) => println!("[{id}] ADAPTER ERROR: {e}"),
        }
    }
    // Print counts/group distributions only, without event bodies.
    let (events, sessions, models): (i64, i64, i64) = {
        let conn = storage.conn();
        let events: i64 = conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
            .unwrap_or(0);
        let sessions: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT COALESCE(session_id,'')) FROM usage_events WHERE session_id IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let models: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT COALESCE(model_raw,'')) FROM usage_events",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (events, sessions, models)
    };
    println!(
        "SUMMARY adapters={total_adapters} instances={reports_total} events={events} sessions={sessions} distinct_models={models}"
    );
}
