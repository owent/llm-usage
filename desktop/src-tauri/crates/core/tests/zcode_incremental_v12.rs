//! ZCode V12 增量语义：重复扫描不增量、追加续读、半行跨轮、截断重扫、
//! 改名保身份、预算分片恢复。样本为真实脱敏 fixture（real-main-session，
//! 4 条记录，人工核算见 _expectations.md：input 合计 1,567,484、total 1,568,658）。

mod common;

use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::adapters::jsonl::JsonlLimits;

const NOW: i64 = 1_800_000_000_000;

fn main_jsonl() -> Vec<u8> {
    reconstruct_jsonl_projection(&zcode_fixture("real-main-session").join("sanitized.json"))
}

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

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("zcode-v12-repeat");
    let root = zcode_root_with_file(&dir, "model-io-r.jsonl", &main_jsonl());
    let (_db, storage) = temp_storage("zcode-v12-repeat");

    let first = run_zcode(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 4);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_zcode(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].status, "unchanged",
        "no new bytes => short-circuit"
    );
    assert!(
        second[0].outcome.is_none(),
        "no commit when nothing changed"
    );
    assert_eq!(storage.data_revision().unwrap(), revision_after_first);

    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 4);
    assert_eq!(summary.totals.input_total_known, Some(1_567_484));
}

#[test]
fn appended_lines_are_read_incrementally() {
    let dir = TempDir::new("zcode-v12-append");
    let jsonl = main_jsonl();
    let text = String::from_utf8(jsonl.clone()).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4);
    lines.truncate(2);
    let file_path = dir.path().join("rollout/model-io-a.jsonl");
    let root = zcode_root_with_file(
        &dir,
        "model-io-a.jsonl",
        (lines.join("\n") + "\n").as_bytes(),
    );
    let (_db, storage) = temp_storage("zcode-v12-append");

    let first = run_zcode(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 2);

    std::fs::write(&file_path, text.as_bytes()).unwrap();
    let second = run_zcode(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 2,
        "only appended lines are read"
    );
    assert_eq!(second[0].files[0].events, 2);

    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 4);
    assert_eq!(summary.totals.input_total_known, Some(1_567_484));
    assert_eq!(summary.totals.total_tokens_known, Some(1_568_658));
}

#[test]
fn half_line_is_not_consumed_until_completed() {
    let dir = TempDir::new("zcode-v12-half");
    let jsonl = main_jsonl();
    let text = String::from_utf8(jsonl).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let head = lines[..lines.len() - 1].join("\n");
    let last = lines[lines.len() - 1];
    // 写入前 3 条完整行 + 第 4 条的前半（无行终止符）。
    let partial = format!("{head}\n{}", &last[..last.len() / 2]);
    let file_path = dir.path().join("rollout/model-io-h.jsonl");
    let root = zcode_root_with_file(&dir, "model-io-h.jsonl", partial.as_bytes());
    let (_db, storage) = temp_storage("zcode-v12-half");

    let first = run_zcode(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read as usize, lines.len() - 1);
    assert_eq!(first[0].files[0].events, 3, "前 3 条记录完整入账");

    // 补全最后半行。
    std::fs::write(&file_path, format!("{head}\n{last}\n").as_bytes()).unwrap();
    let second = run_zcode(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 1,
        "only the completed half line is new"
    );
    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(
        summary.totals.call_count, 4,
        "no double counting after resume"
    );
}

#[test]
fn truncation_triggers_generation_rescan_and_keeps_history() {
    let dir = TempDir::new("zcode-v12-trunc");
    let jsonl = main_jsonl();
    let file_path = dir.path().join("rollout/model-io-t.jsonl");
    let root = zcode_root_with_file(&dir, "model-io-t.jsonl", &jsonl);
    let (_db, storage) = temp_storage("zcode-v12-trunc");
    run_zcode(&storage, &root, NOW);

    // 截断为前 2 行（源端极端行为）：重探测 → generation+1 → 从头重扫。
    let text = String::from_utf8(jsonl).unwrap();
    let head: String = text.lines().take(2).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::remove_file(&file_path).unwrap();
    std::fs::write(&file_path, head.as_bytes()).unwrap();
    let second = run_zcode(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 2);
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1);
    // 重扫的 2 条与已入库内容相同（unchanged），已入库历史不因源截断消失。
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.unchanged), (0, 2));
    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 4);
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("zcode-v12-rename");
    let jsonl = main_jsonl();
    let old_path = dir.path().join("rollout/model-io-old.jsonl");
    let new_path = dir.path().join("rollout/model-io-new.jsonl");
    let root = zcode_root_with_file(&dir, "model-io-old.jsonl", &jsonl);
    let (_db, storage) = temp_storage("zcode-v12-rename");
    run_zcode(&storage, &root, NOW);

    std::fs::rename(&old_path, &new_path).unwrap();
    let second = run_zcode(&storage, &root, NOW + 1000);
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
    assert!(file_id.ends_with("model-io-new.jsonl"));
    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 4);
}

#[test]
fn budget_split_resumes_without_duplicates() {
    let dir = TempDir::new("zcode-v12-budget");
    let root = zcode_root_with_file(&dir, "model-io-b.jsonl", &main_jsonl());
    let (_db, storage) = temp_storage("zcode-v12-budget");

    let first = run_zcode_with_limits(&storage, &root, NOW, budgeted_limits(2));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].files[0].lines_read, 2);
    assert_eq!(first[0].files[0].events, 2);

    let second = run_zcode_with_limits(&storage, &root, NOW + 1000, budgeted_limits(100));
    assert_eq!(second[0].files[0].status, "complete");
    assert_eq!(second[0].files[0].lines_read, 2);
    assert_eq!(second[0].files[0].events, 2);

    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(
        summary.totals.call_count, 4,
        "split rounds never double count"
    );
    assert_eq!(summary.totals.input_total_known, Some(1_567_484));
    assert_eq!(summary.totals.total_tokens_known, Some(1_568_658));
}
