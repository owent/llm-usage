//! 对本机真实 VS Code Copilot Chat 会话日志
//! （workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl）做只读核对
//! （2026-10-01 首次执行：VS Code 1.140.0，10 请求）。
//! 只输出白名单聚合：根数、事件数、token 合计、模型去重计数、诊断计数、
//! 以及重扫幂等性；不打印路径、会话 ID 或记录内容。
//! 用法：cargo run -p llm-usage-core --example real_verify_copilot_chat -- <chatSessions目录或单个.jsonl或workspaceStorage目录> <work_dir>

use llm_usage_core::adapters::copilot_chat::CopilotChatAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: <chat_sessions_root_or_file> <work_dir>");
    let work = std::env::args()
        .nth(2)
        .expect("usage: <chat_sessions_root_or_file> <work_dir>");
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check-copilot-chat.sqlite");
    let _ = std::fs::remove_file(&db_path);
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = CopilotChatAdapter::new();
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
        run_id_prefix: "real-copilot-chat".to_string(),
        origin_host_id: None,
    };
    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    for report in &reports {
        // 不打印 instance_id（含本机根路径，见文件头白名单声明）。
        println!(
            "files={} events={} diagnostics={}",
            report.files.len(),
            report.outcome.as_ref().map(|o| o.added).unwrap_or(0),
            report.files.iter().map(|f| f.diagnostics).sum::<u64>(),
        );
    }
    let conn = storage.conn();
    let (count, observations, in_sum, out_sum, total_sum, models, partials): (i64, i64, Option<i64>, Option<i64>, Option<i64>, i64, i64) =
        conn.query_row(
            "SELECT SUM(record_kind='model_call'),SUM(record_kind='usage_observation'), SUM(input_total), SUM(output_total), SUM(total_tokens),
                    COUNT(DISTINCT model_raw),
                    SUM(CASE WHEN lifecycle='partial' THEN 1 ELSE 0 END)
             FROM usage_events WHERE agent='vscode-copilot-chat' AND attribution_status='verified'",
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
        "collected: calls={count} observations={observations} input={in_sum:?} output={out_sum:?} total={total_sum:?} models={models} partial={partials}"
    );
    // 幂等：重复扫描不增量（同内容 unchanged）。
    let reports2 = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("rescan");
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    let count2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE agent='vscode-copilot-chat' AND attribution_status='verified' AND record_kind='model_call'",
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
