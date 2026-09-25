//! Kimi Code（A12，M4）V12 增量语义 —— 重复扫描不增量、追加续读（对账累计跨轮
//! 持久）、半行跨轮、截断重扫、预算分段恢复、改名重探测。

mod common;

use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::adapters::jsonl::JsonlLimits;
use llm_usage_core::storage::Storage;

const NOW: i64 = 1_800_000_000_000;

fn budgeted_limits(max_lines: u64) -> ScanLimits {
    ScanLimits {
        jsonl: JsonlLimits {
            chunk_bytes: 4096,
            max_line_bytes: 1024 * 1024,
            max_lines: Some(max_lines),
            time_budget: None,
        },
    }
}

fn rec_usage(io: i64, out: i64, cr: i64, time: i64) -> String {
    format!(
        r#"{{"type":"usage.record","agentId":"main","model":"syn/k3","usage":{{"inputOther":{io},"output":{out},"inputCacheRead":{cr},"inputCacheCreation":0}},"usageScope":"turn","time":{time}}}"#
    )
}

/// 基础文件（3 行）：metadata + 两条 turn 记录
/// {100,50,400} 与 {200,60,800}（total 550/1060）。
fn base_lines() -> Vec<String> {
    vec![
        r#"{"type":"metadata","protocol_version":"1.5","created_at":1767225600000}"#.to_string(),
        rec_usage(100, 50, 400, 1767225601000),
        rec_usage(200, 60, 800, 1767225602000),
    ]
}

fn base_file() -> Vec<u8> {
    format!("{}\n", base_lines().join("\n")).into_bytes()
}

fn generation(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("kimi-code-repeat");
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/session_syn-inc/agents/main/wire.jsonl",
        &base_file(),
    );
    let (_db, storage) = temp_storage("kimi-code-repeat");

    let first = run_kimi_code(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "complete");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_kimi_code(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "unchanged", "无新字节 ⇒ 短路");
    assert!(
        second[0].outcome.is_none(),
        "no commit when nothing changed"
    );
    assert_eq!(storage.data_revision().unwrap(), revision_after_first);

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(1_610));
}

/// 追加续读：活文件追加一条记录，仅新行入账；对账累计跨轮持久
/// （第一轮后回声Σ为 0，追加回声后 matched）。
#[test]
fn append_continues_from_cursor_and_reconciliation_persists() {
    let dir = TempDir::new("kimi-code-append");
    let wire_path = dir
        .path()
        .join("sessions")
        .join("wd_syn")
        .join("session_syn-inc")
        .join("agents")
        .join("main")
        .join("wire.jsonl");
    std::fs::create_dir_all(wire_path.parent().unwrap()).unwrap();
    std::fs::write(&wire_path, &base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-append");

    let first = run_kimi_code(&storage, &root, NOW);
    assert_eq!(first[0].files[0].events, 2);
    // 第一轮无回声（基础文件未带 step.end）：记录侧 2 条，回声 0 ⇒ echo_subset。
    assert!(first[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "echo_subset"));

    // 追加：1 条新 usage.record + 与第一条回声相等的 step.end 回声。
    let echo = r#"{"type":"context.append_loop_event","agentId":"main","event":{"type":"step.end","uuid":"syn-1","turnId":"0","step":1,"messageId":"chatcmpl-syn","usage":{"inputOther":100,"output":50,"inputCacheRead":400,"inputCacheCreation":0},"finishReason":"tool_use"},"time":1767225601100}"#;
    let appended = format!("{}\n{}\n", echo, rec_usage(300, 70, 900, 1767225603000));
    let mut bytes = std::fs::read(&wire_path).unwrap();
    bytes.extend_from_slice(appended.as_bytes());
    std::fs::write(&wire_path, &bytes).unwrap();

    let second = run_kimi_code(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 1, "仅新增 usage.record 入账；回声不产事件");
    // 对账累计跨轮持久：记录侧 3 条 Σ=550+1060+1270=2880，回声侧 550 ⇒ echo_subset。
    assert!(second[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo"
            && r.detail_sum == 2_880
            && r.snapshot_final == Some(550)
            && r.verdict == "echo_subset"));

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.total_tokens_known, Some(1_610 + 1_270));
}

/// 半行跨轮：末行无换行不消费，补齐换行后下轮读完整行。
#[test]
fn half_line_is_not_consumed() {
    let dir = TempDir::new("kimi-code-half");
    let wire_path = dir
        .path()
        .join("sessions")
        .join("wd_syn")
        .join("session_syn-inc")
        .join("agents")
        .join("main")
        .join("wire.jsonl");
    std::fs::create_dir_all(wire_path.parent().unwrap()).unwrap();
    // metadata + 第一条完整；第二条无换行（半行）。
    let mut text = format!("{}\n", base_lines()[..2].join("\n"));
    text.push_str(&rec_usage(300, 70, 900, 1767225603000));
    std::fs::write(&wire_path, text.as_bytes()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-half");

    let first = run_kimi_code(&storage, &root, NOW);
    assert_eq!(first[0].files[0].events, 1, "半行不消费");

    let mut bytes = std::fs::read(&wire_path).unwrap();
    bytes.push(b'\n');
    std::fs::write(&wire_path, &bytes).unwrap();
    let second = run_kimi_code(&storage, &root, NOW + 1000);
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1);
    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
}

/// 截断重写：generation 递增、整文件重扫；已入库历史不因源截断而消失
/// （V12 语义，同 codex），重扫仅新增新键事件。
#[test]
fn truncation_triggers_rescan() {
    let dir = TempDir::new("kimi-code-trunc");
    let wire_path = dir
        .path()
        .join("sessions")
        .join("wd_syn")
        .join("session_syn-inc")
        .join("agents")
        .join("main")
        .join("wire.jsonl");
    std::fs::create_dir_all(wire_path.parent().unwrap()).unwrap();
    std::fs::write(&wire_path, &base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-trunc");

    let _ = run_kimi_code(&storage, &root, NOW);
    assert_eq!(generation(&storage), 0);

    // 截断为首行（metadata）+ 换一条不同 usage。
    std::fs::write(
        &wire_path,
        format!(
            "{}\n{}\n",
            base_lines()[0],
            rec_usage(500, 80, 0, 1767225609000)
        )
        .as_bytes(),
    )
    .unwrap();
    let second = run_kimi_code(&storage, &root, NOW + 1000);
    assert!(second[0].files[0].status == "complete");
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1, "仅新键事件");
    assert_eq!(generation(&storage), 1, "重扫代数递增");

    // 已入库历史不因源截断而消失：旧 2 条 + 新 1 条。
    let summary = summary(&storage, "2026-01-01", "2026-01-02");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.total_tokens_known, Some(550 + 1_060 + 580));
}

/// 行数预算分段：预算耗尽停在完整行边界，下轮续读；游标推进按行消费。
#[test]
fn line_budget_resumes() {
    let dir = TempDir::new("kimi-code-budget");
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/session_syn-inc/agents/main/wire.jsonl",
        &base_file(),
    );
    let (_db, storage) = temp_storage("kimi-code-budget");

    // 预算 2 行：metadata + 第一条记录；状态 budget_exhausted。
    let first = run_kimi_code_with_limits(&storage, &root, NOW, budgeted_limits(2));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 1);
    // 对账只应在读到文件尾时进行：本轮不产生 matched 行。
    assert!(first[0].reconciliations.is_empty());

    // 下轮无预算：续读剩余 1 行并完结。
    let second = run_kimi_code(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 1);
    assert!(second[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "echo_subset"));

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(1_610));
}

/// 改名重探测：同内容流换路径（文件名保持 wire.jsonl，换代理目录）按
/// file_identity 命中，事件不重复。
#[test]
fn rename_keeps_identity() {
    let dir = TempDir::new("kimi-code-rename");
    let agents = dir
        .path()
        .join("sessions")
        .join("wd_syn")
        .join("session_syn-inc")
        .join("agents");
    std::fs::create_dir_all(agents.join("main")).unwrap();
    std::fs::write(agents.join("main").join("wire.jsonl"), &base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-rename");

    let _ = run_kimi_code(&storage, &root, NOW);
    // 换代理目录（文件内容流不变，仍在发现范围内）。
    std::fs::create_dir_all(agents.join("agent-2")).unwrap();
    std::fs::rename(
        agents.join("main").join("wire.jsonl"),
        agents.join("agent-2").join("wire.jsonl"),
    )
    .unwrap();
    let second = run_kimi_code(&storage, &root, NOW + 1000);
    let added: i64 = second
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added, 0, "同内容流改名不产生新事件");
    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
}
