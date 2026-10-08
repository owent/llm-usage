//! Read-only checks of native local ZCode data, authorized by the user.
//! Output only permitted aggregates: file/record/event/token counts, stable rescans,
//! db.sqlite comparison counts for sum(model_usage)==turn_usage and diagnostics;
//! never print paths, session IDs, requestId, message text or other record content.
//! Read model_usage into an isolated app DB; turn_usage is comparison-only.
//! Usage: cargo run -p llm-usage-core --example real_verify_zcode -- <zcode_cli_root> <work_dir>
//! <zcode_cli_root> contains rollout/ and db/, for example <home>/.zcode/cli.

use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::zcode::{db_reconciliation, ZcodeAdapter};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: <zcode_cli_root> <work_dir>");
    let work = std::env::args()
        .nth(2)
        .expect("usage: <zcode_cli_root> <work_dir>");
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check.sqlite");
    let _ = std::fs::remove_file(&db_path);
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = ZcodeAdapter::new();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![PathBuf::from(&root)],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("real-{now_ms}"),
    };

    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    for report in &reports {
        let mut files_complete = 0u64;
        let mut files_other = 0u64;
        let mut lines = 0u64;
        let mut records = 0u64;
        let mut events = 0u64;
        let mut diagnostics = 0u64;
        for f in &report.files {
            if f.status == "complete" {
                files_complete += 1;
            } else {
                files_other += 1;
            }
            lines += f.lines_read;
            records += f.records_seen;
            events += f.events;
            diagnostics += f.diagnostics;
        }
        println!("source_files_complete={files_complete} other_status={files_other}");
        println!(
            "lines_read={lines} records_seen={records} events={events} diagnostics={diagnostics}"
        );
        if let Some(o) = &report.outcome {
            println!(
                "batch added={} updated={} unchanged={} skipped={} conflicts={} revision={}",
                o.added, o.updated, o.unchanged, o.skipped, o.conflicts, o.data_revision
            );
        }
    }
    // A second scan must add nothing for unchanged data.
    assert!(!reports.is_empty());
    assert!(reports.iter().all(|r| r.error.is_none()));
    let revision = storage.data_revision().unwrap();
    let config2 = RunConfig {
        run_id_prefix: format!("rescan-{now_ms}"),
        ..config.clone()
    };
    let reports2 = run_adapter_scan(&storage, &adapter, &ctx, &config2).expect("rescan");
    assert!(reports2.iter().all(|r| r.error.is_none()));
    assert_eq!(storage.data_revision().unwrap(), revision);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    println!("rescan_added={added2}");
    assert_eq!(added2, 0);

    // Check only permitted UTC full-range call/token aggregates.
    let summary = llm_usage_core::query::query_summary(
        &storage,
        &llm_usage_core::query::SummaryRequest {
            timezone: "UTC".to_string(),
            week_start: llm_usage_core::calendar::WeekStart::Monday,
            first_day: llm_usage_core::calendar::parse_date("2020-01-01").unwrap(),
            last_day: llm_usage_core::calendar::parse_date("2100-01-01").unwrap(),
            granularity: llm_usage_core::query::Granularity::Day,
            filters: llm_usage_core::query::Filters::default(),
            today: llm_usage_core::calendar::parse_date("2100-01-01").unwrap(),
            retention_cutoff: None,
        },
    )
    .expect("summary");
    println!(
        "summary calls={} input_total_known={:?} cache_read_known={:?} cache_write_known={:?} output_total_known={:?} total_tokens_known={:?}",
        summary.totals.call_count,
        summary.totals.input_total_known,
        summary.totals.cache_read_known,
        summary.totals.cache_write_known,
        summary.totals.output_total_known,
        summary.totals.total_tokens_known
    );
    // Classification counts only, without record content.
    let (primary, sub, aux, unknown): (i64, i64, i64, i64) = storage
        .conn()
        .query_row(
            "SELECT COALESCE(SUM(call_category='primary'),0), \
                    COALESCE(SUM(call_category='sub_agent'),0), \
                    COALESCE(SUM(call_category='auxiliary'),0), \
                    COALESCE(SUM(call_category='unknown'),0) FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("categories");
    println!("categories primary={primary} sub_agent={sub} auxiliary={aux} unknown={unknown}");
    let diag_total: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .expect("diagnostics");
    println!("diagnostics_total={diag_total}");

    // Read-only cli/db/db.sqlite comparison: sum(model_usage)==turn_usage; not usage input.
    let db_sqlite = PathBuf::from(&root).join("db").join("db.sqlite");
    match db_reconciliation(&db_sqlite) {
        Ok(rec) => {
            println!(
                "db_reconciliation model_rows={} turn_rows={} turns_total={} matched={} mismatched={} without_model_rows={} model_computed_total_sum={} turn_computed_total_sum={}",
                rec.model_rows,
                rec.turn_rows,
                rec.turns_total,
                rec.turns_matched,
                rec.turns_mismatched,
                rec.turns_without_model_rows,
                rec.model_computed_total_sum,
                rec.turn_computed_total_sum
            );
        }
        Err(e) => println!("db_reconciliation skipped/failed: {e}"),
    }
}
