//! 对本机真实 Qwen Code 数据做只读核对（已获用户允许的本机数据验证）。
//! 只输出白名单聚合：文件数、记录数、事件数、模型调用、token 合计、时间范围、
//! 对账结论与诊断计数；不打印路径、会话 ID、模型以外的任何记录内容。
//! 用法：cargo run -p llm-usage-core --example real_verify_qwen -- <qwen_root> <work_dir>

use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::qwen::QwenAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: <qwen_root> <work_dir>");
    let work = std::env::args()
        .nth(2)
        .expect("usage: <qwen_root> <work_dir>");
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check.sqlite");
    let _ = std::fs::remove_file(&db_path);
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = QwenAdapter::new();
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
    // 二次扫描：幂等（重复扫描不增量）。
    let reports2 = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("rescan");
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    println!("rescan_added={added2}");

    // 汇总查询白名单核对：调用数与 token 合计（UTC）。
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
        "summary calls={} input_total_known={:?} cache_read_known={:?} output_total_known={:?} total_tokens_known={:?}",
        summary.totals.call_count,
        summary.totals.input_total_known,
        summary.totals.cache_read_known,
        summary.totals.output_total_known,
        summary.totals.total_tokens_known
    );
}
