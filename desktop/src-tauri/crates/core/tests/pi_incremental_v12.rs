//! V12：pi 适配器增量与刷新语义 —— 重复扫描不增量、追加续读、半行跨轮、
//! 截断/同长替换/改名重探测、预算分片恢复、矛盾重复条目冲突标记。
//! 场景对照 codex_incremental_v12.rs；基础内容取真实脱敏 fixture
//! session-error-zero-usage（7 行，唯一事件在 L7）。

mod common;

use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::adapters::jsonl::JsonlLimits;

const NOW: i64 = 1_800_000_000_000;
/// 真实 fixture 重建后的会话文件相对路径（sessions/<encoded-cwd>/<file>）。
const REL: &str = "--C--Users-anon--/2026-09-24T16-37-28-439Z_anon-1.jsonl";

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

fn real_fixture_jsonl() -> Vec<u8> {
    reconstruct_jsonl_projection(&pi_fixture("session-error-zero-usage.sanitized.json"))
}

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("pi-v12-repeat");
    let jsonl = real_fixture_jsonl();
    let root = pi_root_with_file(&dir, REL, &jsonl);
    let (_db, storage) = temp_storage("pi-v12-repeat");

    let first = run_pi(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 1);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_pi(&storage, &root, NOW + 1000);
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
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(0));
    let _ = dir;
}

#[test]
fn appended_lines_are_read_incrementally() {
    let dir = TempDir::new("pi-v12-append");
    let jsonl = real_fixture_jsonl();
    // 先写前 6 行（无 usage 载体），再追加第 7 行（assistant）。
    let text = String::from_utf8(jsonl).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    let tail = lines.split_off(6);
    assert_eq!(tail.len(), 1);
    let file_path = dir.path().join("sessions").join(REL);
    let root = pi_root_with_file(&dir, REL, (lines.join("\n") + "\n").as_bytes());
    let (_db, storage) = temp_storage("pi-v12-append");

    let first = run_pi(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 6);
    assert_eq!(first[0].files[0].events, 0);

    std::fs::write(&file_path, text.as_bytes()).unwrap();
    let second = run_pi(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 1,
        "only appended lines are read"
    );
    assert_eq!(second[0].files[0].events, 1);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(0));
    // 追加续读保留会话身份（解析上下文随游标持久化）。
    let session: String = storage
        .conn()
        .query_row("SELECT session_id FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(session, "anon-1");
    let _ = dir;
}

#[test]
fn half_line_is_not_consumed_until_completed() {
    let dir = TempDir::new("pi-v12-half");
    let jsonl = real_fixture_jsonl();
    let text = String::from_utf8(jsonl).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let (head, last) = (lines[..lines.len() - 1].join("\n"), lines[lines.len() - 1]);
    // 写入除最后一条外的全部行 + 最后一条的前半（assistant 事件行不完整）。
    let partial = format!("{head}\n{}", &last[..last.len() / 2]);
    let file_path = dir.path().join("sessions").join(REL);
    let root = pi_root_with_file(&dir, REL, partial.as_bytes());
    let (_db, storage) = temp_storage("pi-v12-half");

    let first = run_pi(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read as usize, lines.len() - 1);
    assert_eq!(first[0].files[0].events, 0, "半行未消费，无事件");

    // 完成最后半行。
    std::fs::write(&file_path, format!("{head}\n{last}\n").as_bytes()).unwrap();
    let second = run_pi(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 1,
        "only the completed half line is new"
    );
    assert_eq!(second[0].files[0].events, 1);
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(
        summary.totals.call_count, 1,
        "no double counting after resume"
    );
    let _ = dir;
}

#[test]
fn truncation_triggers_generation_rescan() {
    let dir = TempDir::new("pi-v12-trunc");
    let jsonl = real_fixture_jsonl();
    let file_path = dir.path().join("sessions").join(REL);
    let root = pi_root_with_file(&dir, REL, &jsonl);
    let (_db, storage) = temp_storage("pi-v12-trunc");
    run_pi(&storage, &root, NOW);

    // 截断为前 6 行（源端极端行为）：重探测 → generation+1 → 从头重扫。
    let text = String::from_utf8(jsonl).unwrap();
    let head: String = text.lines().take(6).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::remove_file(&file_path).unwrap();
    std::fs::write(&file_path, head.as_bytes()).unwrap();
    let second = run_pi(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 6);
    assert_eq!(second[0].files[0].events, 0, "事件行在截除部分");
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1);
    // 已入库历史不因源截断而消失。
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    let _ = dir;
}

#[test]
fn same_size_replacement_rescans_without_dropping_history() {
    let dir = TempDir::new("pi-v12-replace");
    let original = real_fixture_jsonl();
    let file_path = dir.path().join("sessions").join(REL);
    let root = pi_root_with_file(&dir, REL, &original);
    let (_db, storage) = temp_storage("pi-v12-replace");
    run_pi(&storage, &root, NOW);
    let before = storage.data_revision().unwrap();

    // 同长替换：交换两条完整记录行（总字节数不变、每行仍是合法 JSON、
    // 首行仍是 session 头），内容指纹改变必须触发重扫。
    let text = String::from_utf8(original.clone()).unwrap();
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    assert!(lines.len() >= 5, "fixture should have enough lines to swap");
    lines.swap(2, 3);
    let replaced: Vec<u8> = lines.concat().into_bytes();
    assert_eq!(replaced.len(), original.len());
    assert_ne!(replaced, original);
    std::fs::write(&file_path, &replaced).unwrap();
    let second = run_pi(&storage, &root, NOW + 1000);
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1, "same-size replacement bumps generation");
    assert!(storage.data_revision().unwrap() >= before);
    // 重扫产出同一事件（同键同内容）：幂等，不双计。
    assert_eq!(second[0].files[0].lines_read, 7);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.unchanged), (0, 1));
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    let _ = dir;
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("pi-v12-rename");
    let jsonl = real_fixture_jsonl();
    let old_path = dir.path().join("sessions").join(REL);
    let new_rel = "--C--Users-anon--/2026-09-24T16-37-28-439Z_anon-renamed.jsonl";
    let new_path = dir.path().join("sessions").join(new_rel);
    let root = pi_root_with_file(&dir, REL, &jsonl);
    let (_db, storage) = temp_storage("pi-v12-rename");
    run_pi(&storage, &root, NOW);

    std::fs::rename(&old_path, &new_path).unwrap();
    let second = run_pi(&storage, &root, NOW + 1000);
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
    assert!(file_id.ends_with("anon-renamed.jsonl"));
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    let _ = dir;
}

// 手工核算值（synthetic-auxiliary-carriers，8 行 6 事件，行序：session/model_change/
// assistant(带 usage)/assistant(无 usage)/usage/compaction/branch_summary/toolResult）：
// 分段 3+4+100 行预算 ⇒ 事件 1+4+1；合计 call_count=6、input_total=2915、
// cache_read=1190、cache_write=110、output=355、total=3270。
#[test]
fn budget_split_resumes_without_duplicates() {
    let dir = TempDir::new("pi-v12-budget");
    // 复制合成 fixture 到临时目录（避免改动仓库内 fixture）。
    let jsonl = std::fs::read(
        pi_fixture("synthetic-auxiliary-carriers")
            .join("sessions/--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-aux.jsonl"),
    )
    .unwrap();
    let root = pi_root_with_file(
        &dir,
        "--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-aux.jsonl",
        &jsonl,
    );
    let (_db, storage) = temp_storage("pi-v12-budget");

    let first = run_pi_with_limits(&storage, &root, NOW, budgeted_limits(3));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].files[0].lines_read, 3);
    assert_eq!(first[0].files[0].events, 1, "L3 assistant 已入");

    let second = run_pi_with_limits(&storage, &root, NOW + 1000, budgeted_limits(4));
    assert_eq!(second[0].files[0].status, "budget_exhausted");
    assert_eq!(second[0].files[0].lines_read, 4);
    assert_eq!(second[0].files[0].events, 4);

    let third = run_pi_with_limits(&storage, &root, NOW + 2000, budgeted_limits(100));
    assert_eq!(third[0].files[0].status, "complete");
    assert_eq!(third[0].files[0].events, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(
        summary.totals.call_count, 6,
        "split rounds never double count"
    );
    assert_eq!(summary.totals.input_total_known, Some(2_915));
    assert_eq!(summary.totals.cache_read_known, Some(1_190));
    assert_eq!(summary.totals.cache_write_known, Some(110));
    assert_eq!(summary.totals.output_total_known, Some(355));
    assert_eq!(summary.totals.total_tokens_known, Some(3_270));
    assert_eq!(
        summary.totals.input_unknown_count, 0,
        "无 usage 的 assistant 计调用，但不算未知字段"
    );
    let _ = dir;
}

// 手工核算值：syn-cf-1 首轮 input=1000/cacheRead=400 ⇒ input_total=1400、total=1450；
// 追加同四元组不同 usage（input=1500）⇒ 无先后权威证据 → conflict，已存值保持。
#[test]
fn conflicting_duplicate_entry_marks_conflict_and_keeps_existing() {
    let dir = TempDir::new("pi-v12-conflict");
    let base = concat!(
        "{\"type\":\"session\",\"version\":3,\"id\":\"syn-sess-cf\",\"timestamp\":\"2026-01-05T10:00:00.000Z\"}\n",
        "{\"type\":\"message\",\"id\":\"syn-cf-1\",\"parentId\":null,\"timestamp\":\"2026-01-05T10:00:01.000Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"syn-prov\",\"model\":\"syn-model-a\",\"usage\":{\"input\":1000,\"output\":50,\"cacheRead\":400,\"cacheWrite\":0,\"totalTokens\":1450},\"stopReason\":\"stop\"}}\n",
    );
    let rel = "--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-cf.jsonl";
    let file_path = dir.path().join("sessions").join(rel);
    let root = pi_root_with_file(&dir, rel, base.as_bytes());
    let (_db, storage) = temp_storage("pi-v12-conflict");
    run_pi(&storage, &root, NOW);

    // 追加同条目四元组（type/id/parentId/timestamp 相同）但 usage 数值不同的记录。
    let conflict_line = "{\"type\":\"message\",\"id\":\"syn-cf-1\",\"parentId\":null,\"timestamp\":\"2026-01-05T10:00:01.000Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"syn-prov\",\"model\":\"syn-model-a\",\"usage\":{\"input\":1500,\"output\":50,\"cacheRead\":400,\"cacheWrite\":0,\"totalTokens\":1950},\"stopReason\":\"stop\"}}\n";
    let mut appended = base.as_bytes().to_vec();
    appended.extend_from_slice(conflict_line.as_bytes());
    std::fs::write(&file_path, &appended).unwrap();
    let second = run_pi(&storage, &root, NOW + 1000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.conflicts, 1);

    let key = "pi:message:syn-cf-1:-:2026-01-05T10:00:01.000Z";
    // 不任意择大：已存值保持 1400，冲突标记并记诊断。
    let (input, conflict_flag): (i64, i64) = storage
        .conn()
        .query_row(
            "SELECT input_total, conflict FROM usage_events WHERE source_record_key = ?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(input, 1400);
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
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.conflict_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(1400));
    assert_eq!(summary.totals.total_tokens_known, Some(1450));
    let _ = dir;
}
