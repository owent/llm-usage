//! Kimi Code (A12, M4) V12: repeat deduplication, appended reads and persisted reconciliation,
//! partial lines, truncation rescans, bounded batches with resumption and renamed-file detection.

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

/// Base file: three lines, metadata and two turn records
/// {100,50,400} and {200,60,800}, with totals 550/1060.
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

/// Appended reads add only new records and preserve cumulative reconciliation across rounds.
/// Echo starts at zero; the appended echo still covers only part of the record sum.
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
    std::fs::write(&wire_path, base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-append");

    let first = run_kimi_code(&storage, &root, NOW);
    assert_eq!(first[0].files[0].events, 2);
    // Base has no step.end: two usage records and zero echo give echo_subset.
    assert!(first[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "echo_subset"));

    // Append one usage.record and a step.end echo matching the first existing usage record.
    let echo = r#"{"type":"context.append_loop_event","agentId":"main","event":{"type":"step.end","uuid":"syn-1","turnId":"0","step":1,"messageId":"chatcmpl-syn","usage":{"inputOther":100,"output":50,"inputCacheRead":400,"inputCacheCreation":0},"finishReason":"tool_use"},"time":1767225601100}"#;
    let appended = format!("{}\n{}\n", echo, rec_usage(300, 70, 900, 1767225603000));
    let mut bytes = std::fs::read(&wire_path).unwrap();
    bytes.extend_from_slice(appended.as_bytes());
    std::fs::write(&wire_path, &bytes).unwrap();

    let second = run_kimi_code(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 1, "仅新增 usage.record 入账；回声不产事件");
    // Persisted sums: three records 550+1060+1270=2880 versus echo 550, giving echo_subset.
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

/// Leave a final line without newline unread; consume it after the newline is appended.
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
    // Metadata and the first record are complete; the last record has no newline.
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

/// Truncation increments generation and rescans; stored history survives source truncation
/// under V12, as for Codex. Rescanning adds only events with new keys.
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
    std::fs::write(&wire_path, base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-trunc");

    let _ = run_kimi_code(&storage, &root, NOW);
    assert_eq!(generation(&storage), 0);

    // Rewrite with metadata and one different usage record.
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

    // Keep two historical events and add one new event despite source truncation.
    let summary = summary(&storage, "2026-01-01", "2026-01-02");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.total_tokens_known, Some(550 + 1_060 + 580));
}

/// Stop at a complete line on reaching the line limit; resume from the consumed-line cursor.
#[test]
fn line_budget_resumes() {
    let dir = TempDir::new("kimi-code-budget");
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/session_syn-inc/agents/main/wire.jsonl",
        &base_file(),
    );
    let (_db, storage) = temp_storage("kimi-code-budget");

    // Limit this round to two lines: metadata and first record; status budget_exhausted.
    let first = run_kimi_code_with_limits(&storage, &root, NOW, budgeted_limits(2));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 1);
    // Reconcile only at EOF; this limited round produces no reconciliation row.
    assert!(first[0].reconciliations.is_empty());

    // The next normal-limit round reads the remaining record and completes.
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

/// Rename the unchanged stream while keeping wire.jsonl and changing its agent directory.
/// file_identity matches the stream, preventing duplicate events.
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
    std::fs::write(agents.join("main").join("wire.jsonl"), base_file()).unwrap();
    let root = dir.path().to_path_buf();
    let (_db, storage) = temp_storage("kimi-code-rename");

    let _ = run_kimi_code(&storage, &root, NOW);
    // Change the agent directory; the unchanged stream remains discoverable.
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
