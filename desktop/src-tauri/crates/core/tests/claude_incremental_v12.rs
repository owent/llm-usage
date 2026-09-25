//! V12：Claude Code 适配器增量与刷新语义 —— 重复扫描不增量、追加续读、半行跨轮、
//! 截断/同长替换/改名重探测、预算分片恢复、矛盾重复（同 requestId 不同 usage）
//! 冲突标记。内容均为合成（syn- 前缀 ID），期望值逐条人工核算。

mod common;

use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::adapters::jsonl::JsonlLimits;

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

/// 合成 assistant 条目（usage 四字段，无缓存）：input_total=input、
/// total=input+output。
fn assistant_line(req: &str, uuid: &str, time: &str, input: i64, output: i64) -> String {
    format!(
        "{{\"type\":\"assistant\",\"timestamp\":\"{time}\",\"sessionId\":\"syn-sess-v12\",\"requestId\":\"{req}\",\"uuid\":\"{uuid}\",\"isSidechain\":false,\"message\":{{\"id\":\"syn-msg-{uuid}\",\"model\":\"syn-claude-model-a\",\"usage\":{{\"input_tokens\":{input},\"output_tokens\":{output},\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0}}}}}}\n"
    )
}

fn user_line(uuid: &str, time: &str) -> String {
    format!(
        "{{\"type\":\"user\",\"timestamp\":\"{time}\",\"sessionId\":\"syn-sess-v12\",\"uuid\":\"{uuid}\",\"message\":{{\"role\":\"user\"}}}}\n"
    )
}

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("claude-v12-repeat");
    let file = user_line("syn-uuid-r0", "2026-09-24T10:00:00.000Z")
        + &assistant_line(
            "syn-req-r1",
            "syn-uuid-r1",
            "2026-09-24T10:00:05.000Z",
            100,
            10,
        )
        + &assistant_line(
            "syn-req-r2",
            "syn-uuid-r2",
            "2026-09-24T10:00:06.000Z",
            200,
            20,
        );
    let root = claude_root_with_file(&dir, "proj/sess-v12.jsonl", file.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-repeat");

    let first = run_claude(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_claude(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].status, "unchanged",
        "no new bytes => short-circuit"
    );
    assert!(
        second[0].outcome.is_none(),
        "no commit when nothing changed"
    );
    assert_eq!(storage.data_revision().unwrap(), revision_after_first);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(300));
    assert_eq!(summary.totals.output_total_known, Some(30));
    assert_eq!(summary.totals.total_tokens_known, Some(330));
}

#[test]
fn appended_lines_are_read_incrementally() {
    let dir = TempDir::new("claude-v12-append");
    let head = user_line("syn-uuid-a0", "2026-09-24T10:00:00.000Z")
        + &user_line("syn-uuid-a1", "2026-09-24T10:00:01.000Z");
    let file_path = dir.path().join("projects/proj/sess-v12.jsonl");
    let root = claude_root_with_file(&dir, "proj/sess-v12.jsonl", head.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-append");

    let first = run_claude(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "complete");
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 0, "user lines carry no usage");

    let tail = assistant_line(
        "syn-req-a1",
        "syn-uuid-a2",
        "2026-09-24T10:00:05.000Z",
        100,
        10,
    ) + &assistant_line(
        "syn-req-a2",
        "syn-uuid-a3",
        "2026-09-24T10:00:06.000Z",
        200,
        20,
    );
    std::fs::write(&file_path, (head + &tail).as_bytes()).unwrap();
    let second = run_claude(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 2,
        "only appended lines are read"
    );
    assert_eq!(second[0].files[0].events, 2);
    assert_eq!(second[0].outcome.as_ref().unwrap().added, 2);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(300));
    assert_eq!(summary.totals.total_tokens_known, Some(330));
}

#[test]
fn half_line_is_not_consumed_until_completed() {
    let dir = TempDir::new("claude-v12-half");
    let l1 = user_line("syn-uuid-h0", "2026-09-24T10:00:00.000Z");
    let l2 = assistant_line(
        "syn-req-h1",
        "syn-uuid-h1",
        "2026-09-24T10:00:05.000Z",
        100,
        10,
    );
    let l3 = assistant_line(
        "syn-req-h2",
        "syn-uuid-h2",
        "2026-09-24T10:00:06.000Z",
        200,
        20,
    );
    // 写入前两条完整行 + 第三条的前半（无换行符）。
    let partial = format!("{l1}{l2}{}", &l3[..l3.len() / 2]);
    let file_path = dir.path().join("projects/proj/sess-v12.jsonl");
    let root = claude_root_with_file(&dir, "proj/sess-v12.jsonl", partial.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-half");

    let first = run_claude(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "complete");
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 1);

    std::fs::write(&file_path, format!("{l1}{l2}{l3}").as_bytes()).unwrap();
    let second = run_claude(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 1,
        "only the completed half line is new"
    );
    assert_eq!(second[0].files[0].events, 1);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(
        summary.totals.call_count, 2,
        "no double counting after resume"
    );
    assert_eq!(summary.totals.input_total_known, Some(300));
    assert_eq!(summary.totals.total_tokens_known, Some(330));
}

#[test]
fn truncation_triggers_generation_rescan() {
    let dir = TempDir::new("claude-v12-trunc");
    let l1 = user_line("syn-uuid-t0", "2026-09-24T10:00:00.000Z");
    let l2 = assistant_line(
        "syn-req-t1",
        "syn-uuid-t1",
        "2026-09-24T10:00:05.000Z",
        100,
        10,
    );
    let l3 = assistant_line(
        "syn-req-t2",
        "syn-uuid-t2",
        "2026-09-24T10:00:06.000Z",
        200,
        20,
    );
    let l4 = user_line("syn-uuid-t3", "2026-09-24T10:00:07.000Z");
    let file_path = dir.path().join("projects/proj/sess-v12.jsonl");
    let root = claude_root_with_file(
        &dir,
        "proj/sess-v12.jsonl",
        format!("{l1}{l2}{l3}{l4}").as_bytes(),
    );
    let (_db, storage) = temp_storage("claude-v12-trunc");
    let first = run_claude(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);

    // 截断为前 2 行（源端极端行为）：重探测 → generation+1 → 从头重扫。
    std::fs::remove_file(&file_path).unwrap();
    std::fs::write(&file_path, format!("{l1}{l2}").as_bytes()).unwrap();
    let second = run_claude(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 2);
    assert_eq!(second[0].files[0].events, 1, "rescan re-reads syn-req-t1");
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1);
    // 已入库历史不因源截断而消失；重扫的 syn-req-t1 逐字段相同 → 幂等 Keep。
    assert_eq!(second[0].outcome.as_ref().unwrap().unchanged, 1);
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(330));
}

#[test]
fn same_size_replacement_rescans_without_dropping_history() {
    let dir = TempDir::new("claude-v12-replace");
    let l1 = user_line("syn-uuid-s0", "2026-09-24T10:00:00.000Z");
    // 两条 assistant 行严格等长（requestId/uuid 等长、数值位数相同），交换即同长替换。
    let l2 = assistant_line(
        "syn-req-s1",
        "syn-uuid-s1",
        "2026-09-24T10:00:05.000Z",
        100,
        10,
    );
    let l3 = assistant_line(
        "syn-req-s2",
        "syn-uuid-s2",
        "2026-09-24T10:00:06.000Z",
        200,
        20,
    );
    assert_eq!(l2.len(), l3.len(), "same-size swap precondition");
    let original = format!("{l1}{l2}{l3}");
    let file_path = dir.path().join("projects/proj/sess-v12.jsonl");
    let root = claude_root_with_file(&dir, "proj/sess-v12.jsonl", original.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-replace");
    run_claude(&storage, &root, NOW);
    let before = storage.data_revision().unwrap();

    let replaced = format!("{l1}{l3}{l2}");
    assert_eq!(replaced.len(), original.len());
    assert_ne!(replaced, original);
    std::fs::write(&file_path, replaced.as_bytes()).unwrap();
    let second = run_claude(&storage, &root, NOW + 1000);
    // 内容指纹变化触发重扫；两条事件逐字段相同 → 幂等 Keep，不双计。
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1, "same-size replacement bumps generation");
    assert_eq!(second[0].files[0].lines_read, 3);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.unchanged), (0, 2));
    assert!(storage.data_revision().unwrap() >= before);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(330));
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("claude-v12-rename");
    let file = user_line("syn-uuid-n0", "2026-09-24T10:00:00.000Z")
        + &assistant_line(
            "syn-req-n1",
            "syn-uuid-n1",
            "2026-09-24T10:00:05.000Z",
            100,
            10,
        );
    let root = claude_root_with_file(&dir, "proj/sess-old.jsonl", file.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-rename");
    run_claude(&storage, &root, NOW);

    std::fs::rename(
        dir.path().join("projects/proj/sess-old.jsonl"),
        dir.path().join("projects/proj/sess-new.jsonl"),
    )
    .unwrap();
    let second = run_claude(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files.len(),
        1,
        "same content stream, not a new file"
    );
    assert_eq!(second[0].files[0].status, "unchanged");
    assert!(second[0].outcome.is_none(), "rename alone does not rescan");
    let file_id: String = storage
        .conn()
        .query_row("SELECT file_id FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert!(file_id.ends_with("sess-new.jsonl"));

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(110));
}

#[test]
fn budget_split_resumes_without_duplicates() {
    let dir = TempDir::new("claude-v12-budget");
    // 5 行：user + assistant ×4（每行一个调用）。
    let file = user_line("syn-uuid-b0", "2026-09-24T10:00:00.000Z")
        + &assistant_line(
            "syn-req-b1",
            "syn-uuid-b1",
            "2026-09-24T10:00:05.000Z",
            10,
            1,
        )
        + &assistant_line(
            "syn-req-b2",
            "syn-uuid-b2",
            "2026-09-24T10:00:06.000Z",
            20,
            2,
        )
        + &assistant_line(
            "syn-req-b3",
            "syn-uuid-b3",
            "2026-09-24T10:00:07.000Z",
            30,
            3,
        )
        + &assistant_line(
            "syn-req-b4",
            "syn-uuid-b4",
            "2026-09-24T10:00:08.000Z",
            40,
            4,
        );
    let root = claude_root_with_file(&dir, "proj/sess-v12.jsonl", file.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-budget");

    let first = run_claude_with_limits(&storage, &root, NOW, budgeted_limits(2));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 1);

    let second = run_claude_with_limits(&storage, &root, NOW + 1000, budgeted_limits(2));
    assert_eq!(second[0].files[0].status, "budget_exhausted");
    assert_eq!(second[0].files[0].lines_read, 2);
    assert_eq!(second[0].files[0].events, 2);

    let third = run_claude_with_limits(&storage, &root, NOW + 2000, budgeted_limits(100));
    assert_eq!(third[0].files[0].status, "complete");
    assert_eq!(third[0].files[0].lines_read, 1);
    assert_eq!(third[0].files[0].events, 1);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(
        summary.totals.call_count, 4,
        "split rounds never double count"
    );
    assert_eq!(summary.totals.input_total_known, Some(100));
    assert_eq!(summary.totals.output_total_known, Some(10));
    assert_eq!(summary.totals.total_tokens_known, Some(110));
}

#[test]
fn conflicting_duplicate_marks_conflict_and_keeps_existing() {
    let dir = TempDir::new("claude-v12-conflict");
    let file = user_line("syn-uuid-c0", "2026-09-24T10:00:00.000Z")
        + &assistant_line(
            "syn-req-c1",
            "syn-uuid-c1",
            "2026-09-24T10:00:05.000Z",
            100,
            10,
        );
    let file_path = dir.path().join("projects/proj/sess-v12.jsonl");
    let root = claude_root_with_file(&dir, "proj/sess-v12.jsonl", file.as_bytes());
    let (_db, storage) = temp_storage("claude-v12-conflict");
    run_claude(&storage, &root, NOW);

    // 追加同 requestId 但 usage 不同的重报（无先后权威证据 → conflict）。
    let conflict = file
        + &assistant_line(
            "syn-req-c1",
            "syn-uuid-c2",
            "2026-09-24T10:00:09.000Z",
            150,
            10,
        );
    std::fs::write(&file_path, conflict.as_bytes()).unwrap();
    let second = run_claude(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 1);
    assert_eq!(second[0].files[0].events, 1);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.conflicts, 1);

    // 不任意择大：已存值保持 input_total=100，冲突记诊断。
    let input: i64 = storage
        .conn()
        .query_row(
            "SELECT input_total FROM usage_events WHERE source_record_key = 'req:syn-req-c1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(input, 100);
    let conflict_flag: i64 = storage
        .conn()
        .query_row(
            "SELECT conflict FROM usage_events WHERE source_record_key = 'req:syn-req-c1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conflict_flag, 1);
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'update_conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diags, 1);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.conflict_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(110));
}
