//! Read-only checks of native Kilo Code CLI ~/.local/share/kilo/kilo.db,
//! authorized by the user. Output only permitted file/record/event counts,
//! sessions, category/lifecycle/compatibility distributions, tokens, reconciliation and diagnostics.
//! No paths, session/message IDs or record content beyond models.
//! Source is read-only; if needed use Online Backup, then remove the staged copy.
//! Usage: cargo run -p llm-usage-core --example real_verify_kilo -- <kilo-root-or-parent-or-user-home> <work_dir>

use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::kilo::KiloAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: <kilo_root_or_parent> <work_dir>");
    let work = std::env::args()
        .nth(2)
        .expect("usage: <kilo_root_or_parent> <work_dir>");
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check-kilo.sqlite");
    let _ = std::fs::remove_file(&db_path);
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = KiloAdapter::new();
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
        origin_host_id: None,
        run_id_prefix: format!("real-kilo-{now_ms}"),
    };

    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    let mut reconciliations = Vec::new();
    for report in &reports {
        let mut files_complete = 0u64;
        let mut files_other = 0u64;
        let mut records = 0u64;
        let mut events = 0u64;
        let mut diagnostics = 0u64;
        for f in &report.files {
            if f.status == "complete" {
                files_complete += 1;
            } else {
                files_other += 1;
            }
            records += f.records_seen;
            events += f.events;
            diagnostics += f.diagnostics;
        }
        println!("source_files_complete={files_complete} other_status={files_other}");
        println!("records_seen={records} events={events} diagnostics={diagnostics}");
        if let Some(o) = &report.outcome {
            println!(
                "batch added={} updated={} unchanged={} skipped={} conflicts={} revision={}",
                o.added, o.updated, o.unchanged, o.skipped, o.conflicts, o.data_revision
            );
        }
        reconciliations.extend(report.reconciliations.iter().cloned());
    }
    // Session cumulative comparisons expose permitted counts without IDs.
    let matched = reconciliations
        .iter()
        .filter(|r| r.verdict == "matched")
        .count();
    let mismatch = reconciliations
        .iter()
        .filter(|r| r.verdict == "mismatch")
        .count();
    let no_snapshot = reconciliations
        .iter()
        .filter(|r| r.verdict == "no_snapshot")
        .count();
    println!(
        "reconcile sessions_total={} matched={matched} mismatch={mismatch} no_snapshot={no_snapshot}",
        reconciliations.len()
    );

    // A second scan adds no duplicate records.
    let reports2 = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("rescan");
    let (added2, updated2): (i64, i64) = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| (o.added, o.updated)))
        .fold((0, 0), |(a, u), (x, y)| (a + x, u + y));
    println!("rescan_added={added2} rescan_updated={updated2}");

    // Permitted distributions and sums across the full UTC interval.
    let conn = storage.conn();
    let (sessions, primary, sub, partial, known, fallback): (i64, i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(DISTINCT session_id), \
                    SUM(call_category = 'primary'), SUM(call_category = 'sub_agent'), \
                    SUM(lifecycle = 'partial'), \
                    SUM(parse_basis = 'known_version'), SUM(parse_basis = 'latest_fallback') \
             FROM usage_events",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .expect("distribution query");
    println!(
        "events sessions={sessions} primary={primary} sub_agent={sub} partial={partial} known_version={known} latest_fallback={fallback}"
    );

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
}
