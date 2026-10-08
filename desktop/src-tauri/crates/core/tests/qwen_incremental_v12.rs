//! Qwen V12: stable repeats, append continuation, partial lines across runs, truncation,
//! same-size replacement/rename detection, bounded-read continuation and same-uuid usage conflicts.

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

fn rec_user(uuid: &str, ts: &str) -> String {
    format!(
        r#"{{"uuid":"{uuid}","sessionId":"syn-sess-inc","timestamp":"{ts}","type":"user","version":"0.5.0","message":{{"role":"user","content":"synthetic"}}}}"#
    )
}

fn rec_assistant(uuid: &str, ts: &str, prompt: i64, candidates: i64, total: i64) -> String {
    format!(
        r#"{{"uuid":"{uuid}","sessionId":"syn-sess-inc","timestamp":"{ts}","type":"assistant","version":"0.5.0","model":"qwen3-coder-plus","usageMetadata":{{"promptTokenCount":{prompt},"candidatesTokenCount":{candidates},"totalTokenCount":{total}}}}}"#
    )
}

/// Base three lines: user, assistant a1(1000/50/1050), assistant a2(2000/100/2100).
/// Manual totals: call_count=2, input_total_known=3000, output_total_known=150;
/// total_tokens_known=3150, cache_read_known=None; unreported cache stays unknown.
fn base_lines() -> Vec<String> {
    vec![
        rec_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        rec_assistant("syn-a-1", "2026-01-05T10:00:05.000Z", 1000, 50, 1050),
        rec_assistant("syn-a-2", "2026-01-05T10:00:08.000Z", 2000, 100, 2100),
    ]
}

fn base_file() -> Vec<u8> {
    // JSONL needs a final newline; without it the last line stays unconsumed partial data.
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
    let dir = TempDir::new("qwen-repeat");
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-r.jsonl", &base_file());
    let (_db, storage) = temp_storage("qwen-repeat");

    let first = run_qwen(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "complete");
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "unchanged", "无新字节 ⇒ 短路");
    assert!(
        second[0].outcome.is_none(),
        "no commit when nothing changed"
    );
    assert_eq!(storage.data_revision().unwrap(), revision_after_first);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(3_000));
}

#[test]
fn appended_lines_are_read_incrementally() {
    let dir = TempDir::new("qwen-append");
    let lines = base_lines();
    let head = format!("{}\n{}\n", lines[0], lines[1]);
    let file_path = dir.path().join("tmp/proj-1/chats/sess-a.jsonl");
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-a.jsonl", head.as_bytes());
    let (_db, storage) = temp_storage("qwen-append");

    let first = run_qwen(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 1);

    let mut full = head.into_bytes();
    full.extend_from_slice(lines[2].as_bytes());
    full.push(b'\n');
    std::fs::write(&file_path, &full).unwrap();
    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 1, "只读追加的行");
    assert_eq!(second[0].files[0].events, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2, "续读不双计");
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
}

#[test]
fn half_line_is_not_consumed_until_completed() {
    let dir = TempDir::new("qwen-half");
    let lines = base_lines();
    let half = &lines[2][..lines[2].len() / 2];
    let partial = format!("{}\n{}\n{half}", lines[0], lines[1]);
    let file_path = dir.path().join("tmp/proj-1/chats/sess-h.jsonl");
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-h.jsonl", partial.as_bytes());
    let (_db, storage) = temp_storage("qwen-half");

    let first = run_qwen(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 2, "半行不前移游标");
    assert_eq!(first[0].files[0].events, 1);

    std::fs::write(&file_path, base_file()).unwrap();
    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 1, "只读补全的那行");
    assert_eq!(second[0].files[0].events, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2, "半行恢复后不双计");
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
}

#[test]
fn truncation_triggers_generation_rescan() {
    let dir = TempDir::new("qwen-trunc");
    let lines = base_lines();
    let file_path = dir.path().join("tmp/proj-1/chats/sess-t.jsonl");
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-t.jsonl", &base_file());
    let (_db, storage) = temp_storage("qwen-trunc");
    run_qwen(&storage, &root, NOW);
    assert_eq!(generation(&storage), 0);

    // Truncate to the first two lines: redetect, generation+1, replay from the beginning.
    let head = format!("{}\n{}\n", lines[0], lines[1]);
    std::fs::remove_file(&file_path).unwrap();
    std::fs::write(&file_path, head.as_bytes()).unwrap();
    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(second[0].files[0].lines_read, 2);
    assert_eq!(generation(&storage), 1);
    // Truncation retains imported history; replaying a1 adds no duplicate event.
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
}

#[test]
fn same_size_replacement_rescans_without_dropping_history() {
    let dir = TempDir::new("qwen-replace");
    // Same-digit assistant numbers produce equal line lengths; swap order without resizing file.
    let line_a = rec_assistant("syn-a-1", "2026-01-05T10:00:05.000Z", 1000, 50, 1050);
    let line_b = rec_assistant("syn-a-2", "2026-01-05T10:00:08.000Z", 2000, 60, 2060);
    assert_eq!(line_a.len(), line_b.len(), "交换样本必须等长");
    let user = rec_user("syn-u-1", "2026-01-05T10:00:00.000Z");
    let original = format!("{user}\n{line_a}\n{line_b}\n");
    let swapped = format!("{user}\n{line_b}\n{line_a}\n");
    assert_eq!(original.len(), swapped.len());
    assert_ne!(original, swapped);
    let file_path = dir.path().join("tmp/proj-1/chats/sess-s.jsonl");
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-s.jsonl", original.as_bytes());
    let (_db, storage) = temp_storage("qwen-replace");
    run_qwen(&storage, &root, NOW);
    assert_eq!(generation(&storage), 0);

    std::fs::write(&file_path, swapped.as_bytes()).unwrap();
    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(
        generation(&storage),
        1,
        "同长替换（首指纹变化）⇒ generation+1"
    );
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 0);
    assert_eq!(outcome.unchanged, 2, "事件按 uuid 身份 upsert 幂等");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2, "重扫不双计");
    assert_eq!(summary.totals.input_total_known, Some(3_000));
    assert_eq!(summary.totals.total_tokens_known, Some(3_110));
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("qwen-rename");
    let old_path = dir.path().join("tmp/proj-1/chats/sess-old.jsonl");
    let new_path = dir.path().join("tmp/proj-1/chats/sess-new.jsonl");
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-old.jsonl", &base_file());
    let (_db, storage) = temp_storage("qwen-rename");
    run_qwen(&storage, &root, NOW);

    std::fs::rename(&old_path, &new_path).unwrap();
    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files.len(), 1, "同一内容流，不算新文件");
    assert_eq!(second[0].files[0].status, "unchanged");
    assert!(second[0].outcome.is_none(), "改名本身不重扫");
    let file_id: String = storage
        .conn()
        .query_row("SELECT file_id FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert!(file_id.ends_with("sess-new.jsonl"));
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
}

#[test]
fn budget_split_resumes_without_duplicates() {
    let dir = TempDir::new("qwen-budget");
    // Five lines: user+a1+user+a2+a3, three calls.
    let lines = [
        rec_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        rec_assistant("syn-a-1", "2026-01-05T10:00:05.000Z", 1000, 50, 1050),
        rec_user("syn-u-2", "2026-01-05T10:00:06.000Z"),
        rec_assistant("syn-a-2", "2026-01-05T10:00:08.000Z", 2000, 100, 2100),
        rec_assistant("syn-a-3", "2026-01-05T10:00:12.000Z", 500, 25, 525),
    ];
    let file = format!("{}\n", lines.join("\n")).into_bytes();
    let root = qwen_root_with_file(&dir, "proj-1/chats/sess-b.jsonl", &file);
    let (_db, storage) = temp_storage("qwen-budget");

    let first = run_qwen_with_limits(&storage, &root, NOW, budgeted_limits(2));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 1);

    let second = run_qwen_with_limits(&storage, &root, NOW + 1000, budgeted_limits(2));
    assert_eq!(second[0].files[0].status, "budget_exhausted");
    assert_eq!(second[0].files[0].lines_read, 2);
    assert_eq!(second[0].files[0].events, 1);

    let third = run_qwen_with_limits(&storage, &root, NOW + 2000, budgeted_limits(100));
    assert_eq!(third[0].files[0].status, "complete");
    assert_eq!(third[0].files[0].events, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3, "分段轮次绝不双计");
    assert_eq!(summary.totals.input_total_known, Some(3_500));
    assert_eq!(summary.totals.total_tokens_known, Some(3_675));
}

#[test]
fn conflicting_duplicate_marks_conflict_and_keeps_existing() {
    let dir = TempDir::new("qwen-conflict");
    let lines = [
        rec_user("syn-u-1", "2026-01-05T10:00:00.000Z"),
        rec_assistant("syn-c-1", "2026-01-05T10:00:05.000Z", 1000, 50, 1050),
    ];
    let file_path = dir.path().join("tmp/proj-1/chats/sess-c.jsonl");
    let root = qwen_root_with_file(
        &dir,
        "proj-1/chats/sess-c.jsonl",
        format!("{}\n", lines.join("\n")).as_bytes(),
    );
    let (_db, storage) = temp_storage("qwen-conflict");
    run_qwen(&storage, &root, NOW);

    // Same uuid, different usageMetadata: newer revision unknown, so conflict.
    let conflict_line = rec_assistant("syn-c-1", "2026-01-05T10:00:10.000Z", 1500, 60, 1560);
    let mut appended = format!("{}\n", lines.join("\n")).into_bytes();
    appended.extend_from_slice(conflict_line.as_bytes());
    appended.push(b'\n');
    std::fs::write(&file_path, &appended).unwrap();
    let second = run_qwen(&storage, &root, NOW + 1000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.conflicts, 1);

    // Do not pick the larger value; retain 1000, mark conflict and record diagnostics.
    let input: i64 = storage
        .conn()
        .query_row(
            "SELECT input_total FROM usage_events WHERE source_record_key = 'qwen:syn-c-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(input, 1000);
    let conflict_flag: i64 = storage
        .conn()
        .query_row(
            "SELECT conflict FROM usage_events WHERE source_record_key = 'qwen:syn-c-1'",
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
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1, "同键冲突不双计");
    assert_eq!(summary.totals.conflict_count, 1);
}
