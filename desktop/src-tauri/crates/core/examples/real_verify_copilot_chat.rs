//! Read-only checks of native VS Code Copilot Chat sessions
//! at workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl.
//! First run: 2026-10-01, VS Code 1.140.0, ten requests.
//! Output only permitted root/event/token/distinct-model/diagnostic counts
//! and rescan stability, without paths, session IDs or record content.
//! Usage: cargo run -p llm-usage-core --example real_verify_copilot_chat -- <chatSessions-dir-or-jsonl-or-workspaceStorage> <work_dir> [--reuse] [--timezone=Asia/Shanghai]
//! --reuse preserves old cursors only in isolated build/ databases; never pass an active user DB.

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
    let mut reuse = false;
    let mut timezone = "UTC".to_string();
    for arg in std::env::args().skip(3) {
        if arg == "--reuse" {
            reuse = true;
        } else if let Some(value) = arg.strip_prefix("--timezone=") {
            timezone = value.to_string();
        } else {
            panic!("unrecognized verification option");
        }
    }
    std::fs::create_dir_all(&work).expect("create work dir");
    let db_path = PathBuf::from(&work).join("real-check-copilot-chat.sqlite");
    if reuse {
        assert!(
            db_path.is_file(),
            "--reuse requires an existing isolated verification DB"
        );
    } else {
        let _ = std::fs::remove_file(&db_path);
    }
    let storage = Storage::open(&db_path).expect("open storage");

    let adapter = CopilotChatAdapter::new();
    let now_ms = jiff::Timestamp::now().as_millisecond();
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![PathBuf::from(root)],
    };
    let config = RunConfig {
        timezone,
        now_ms,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: "real-copilot-chat".to_string(),
        origin_host_id: None,
    };
    let reports = run_adapter_scan(&storage, &adapter, &ctx, &config).expect("scan");
    for report in &reports {
        // Do not print instance_id, which contains local paths excluded by the output rules.
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
    // Repeated scans add nothing when content is unchanged.
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
