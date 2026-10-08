//! V12: oh-my-pi (omp) repeated reads, appended lines, partial lines across rounds,
//! truncation/equal-length replacement/rename detection, bounded resume and conflicting duplicates.
//! Scenarios correspond to pi_incremental_v12.rs; the base is the real sanitized
//! session-glm-reasoning sample: 7 lines, one event at L6 assistant and L7 custom.

mod common;

use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::adapters::jsonl::JsonlLimits;

const NOW: i64 = 1_800_000_000_000;
/// Relative reconstructed real-sample path: sessions/<encoded-cwd>/<file>.
const REL: &str =
    "--C--Users-anon--/2026-09-24T16-39-54-647Z_00000000-0000-7000-8000-000000000009.jsonl";

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
    reconstruct_jsonl_projection(&omp_fixture("session-glm-reasoning.sanitized.json"))
}

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("omp-v12-repeat");
    let jsonl = real_fixture_jsonl();
    let root = omp_root_with_file(&dir, REL, &jsonl);
    let (_db, storage) = temp_storage("omp-v12-repeat");

    let first = run_omp(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 1);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_omp(&storage, &root, NOW + 1000);
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
    assert_eq!(summary.totals.total_tokens_known, Some(17_607));
    let _ = dir;
}

#[test]
fn appended_lines_are_read_incrementally() {
    let dir = TempDir::new("omp-v12-append");
    let jsonl = real_fixture_jsonl();
    // Write the first five lines, title/session/model_change/thinking_level_change/user, without events;
    // then append L6 assistant with its event and L7 custom.
    let text = String::from_utf8(jsonl).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    let tail = lines.split_off(5);
    assert_eq!(tail.len(), 2);
    let file_path = dir.path().join("sessions").join(REL);
    let root = omp_root_with_file(&dir, REL, (lines.join("\n") + "\n").as_bytes());
    let (_db, storage) = temp_storage("omp-v12-append");

    let first = run_omp(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 5);
    assert_eq!(first[0].files[0].events, 0);

    std::fs::write(&file_path, text.as_bytes()).unwrap();
    let second = run_omp(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 2,
        "only appended lines are read"
    );
    assert_eq!(second[0].files[0].events, 1);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(17_542));
    // Appended reads retain session identity through parser context persisted with the cursor.
    let session: String = storage
        .conn()
        .query_row("SELECT session_id FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(session, "anon-1");
    let _ = dir;
}

#[test]
fn half_line_is_not_consumed_until_completed() {
    let dir = TempDir::new("omp-v12-half");
    let jsonl = real_fixture_jsonl();
    let text = String::from_utf8(jsonl).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // Write five complete lines plus the first half of the L6 assistant event.
    let head = lines[..5].join("\n");
    let event_line = lines[5];
    let partial = format!("{head}\n{}", &event_line[..event_line.len() / 2]);
    let file_path = dir.path().join("sessions").join(REL);
    let root = omp_root_with_file(&dir, REL, partial.as_bytes());
    let (_db, storage) = temp_storage("omp-v12-half");

    let first = run_omp(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 5);
    assert_eq!(first[0].files[0].events, 0, "半行未消费，无事件");

    // Complete that partial line and add L7 custom.
    std::fs::write(&file_path, text.as_bytes()).unwrap();
    let second = run_omp(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 2,
        "only the completed half line and beyond are new"
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
    let dir = TempDir::new("omp-v12-trunc");
    let jsonl = real_fixture_jsonl();
    let file_path = dir.path().join("sessions").join(REL);
    let root = omp_root_with_file(&dir, REL, &jsonl);
    let (_db, storage) = temp_storage("omp-v12-trunc");
    run_omp(&storage, &root, NOW);

    // Truncating to five lines triggers detection, generation+1 and a scan from the start.
    let text = String::from_utf8(jsonl).unwrap();
    let head: String = text.lines().take(5).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::remove_file(&file_path).unwrap();
    std::fs::write(&file_path, head.as_bytes()).unwrap();
    let second = run_omp(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 5);
    assert_eq!(second[0].files[0].events, 0, "事件行在截除部分");
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1);
    // Truncation does not remove already imported history.
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    let _ = dir;
}

#[test]
fn same_size_replacement_rescans_without_dropping_history() {
    let dir = TempDir::new("omp-v12-replace");
    let original = real_fixture_jsonl();
    let file_path = dir.path().join("sessions").join(REL);
    let root = omp_root_with_file(&dir, REL, &original);
    let (_db, storage) = temp_storage("omp-v12-replace");
    run_omp(&storage, &root, NOW);
    let before = storage.data_revision().unwrap();

    // Equal-length replacement swaps L3 model_change and L4 thinking_level_change.
    // Byte length and valid JSON lines stay intact; L1 title and L2 session remain unchanged.
    // A changed content fingerprint must trigger a rescan.
    let text = String::from_utf8(original.clone()).unwrap();
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    assert!(lines.len() >= 5, "fixture should have enough lines to swap");
    lines.swap(2, 3);
    let replaced: Vec<u8> = lines.concat().into_bytes();
    assert_eq!(replaced.len(), original.len());
    assert_ne!(replaced, original);
    std::fs::write(&file_path, &replaced).unwrap();
    let second = run_omp(&storage, &root, NOW + 1000);
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1, "same-size replacement bumps generation");
    assert!(storage.data_revision().unwrap() >= before);
    // Rescanning the same key/content is idempotent and adds no duplicate event.
    assert_eq!(second[0].files[0].lines_read, 7);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.unchanged), (0, 1));
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    let _ = dir;
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("omp-v12-rename");
    let jsonl = real_fixture_jsonl();
    let old_path = dir.path().join("sessions").join(REL);
    let new_rel = "--C--Users-anon--/2026-09-24T16-39-54-647Z_anon-renamed.jsonl";
    let new_path = dir.path().join("sessions").join(new_rel);
    let root = omp_root_with_file(&dir, REL, &jsonl);
    let (_db, storage) = temp_storage("omp-v12-rename");
    run_omp(&storage, &root, NOW);

    std::fs::rename(&old_path, &new_path).unwrap();
    let second = run_omp(&storage, &root, NOW + 1000);
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

// Manual synthetic-auxiliary-carriers totals: 9 lines, 6 events, ordered title/session/
// model_change/assistant(with usage)/assistant(without usage)/usage/compaction/
// branch_summary/toolResult. Line limits 4+4+100 produce events 1+4+1.
// Totals: call_count=6, derived input_total=1625, cache_read=10, cache_write=1005,
// output=305, total=1930, input_unknown_count=0; syn-a2 counts a call without unknown-field denominators.
#[test]
fn budget_split_resumes_without_duplicates() {
    let dir = TempDir::new("omp-v12-budget");
    // Copy the synthetic sample into a temporary directory, leaving repository samples intact.
    let jsonl = std::fs::read(omp_fixture("synthetic-auxiliary-carriers").join(
        "sessions/--C--syn--/2026-01-05T10-00-00-040Z_00000000-0000-7000-8000-00000000d040.jsonl",
    ))
    .unwrap();
    let root = omp_root_with_file(
        &dir,
        "--C--syn--/2026-01-05T10-00-00-040Z_00000000-0000-7000-8000-00000000d040.jsonl",
        &jsonl,
    );
    let (_db, storage) = temp_storage("omp-v12-budget");

    let first = run_omp_with_limits(&storage, &root, NOW, budgeted_limits(4));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(first[0].files[0].lines_read, 4);
    assert_eq!(first[0].files[0].events, 1, "L4 assistant 已入");

    let second = run_omp_with_limits(&storage, &root, NOW + 1000, budgeted_limits(4));
    assert_eq!(second[0].files[0].status, "budget_exhausted");
    assert_eq!(second[0].files[0].lines_read, 4);
    assert_eq!(second[0].files[0].events, 4);

    let third = run_omp_with_limits(&storage, &root, NOW + 2000, budgeted_limits(100));
    assert_eq!(third[0].files[0].status, "complete");
    assert_eq!(third[0].files[0].events, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(
        summary.totals.call_count, 6,
        "split rounds never double count"
    );
    assert_eq!(summary.totals.input_total_known, Some(1_625));
    assert_eq!(summary.totals.cache_read_known, Some(10));
    assert_eq!(summary.totals.cache_write_known, Some(1_005));
    assert_eq!(summary.totals.output_total_known, Some(305));
    assert_eq!(summary.totals.total_tokens_known, Some(1_930));
    assert_eq!(
        summary.totals.input_unknown_count, 0,
        "无 usage 的 assistant 计调用，但不算未知字段"
    );
    let _ = dir;
}

// syn-cf-1 first read: input=1000/cacheRead=400 gives input_total=1400 and total=1450.
// Same tuple with input=1500 has no proven revision order; mark conflict and retain existing values.
#[test]
fn conflicting_duplicate_entry_marks_conflict_and_keeps_existing() {
    let dir = TempDir::new("omp-v12-conflict");
    let base = concat!(
        "{\"type\":\"title\",\"v\":1,\"title\":\"syn-cf\",\"updatedAt\":\"2026-01-05T10:00:00.000Z\"}\n",
        "{\"type\":\"session\",\"version\":3,\"id\":\"syn-omp-cf\",\"timestamp\":\"2026-01-05T10:00:00.000Z\",\"cwd\":\"syn-cwd\"}\n",
        "{\"type\":\"message\",\"id\":\"syn-cf-1\",\"parentId\":null,\"timestamp\":\"2026-01-05T10:00:01.000Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"syn-provider\",\"model\":\"syn-model-a\",\"usage\":{\"input\":1000,\"output\":50,\"cacheRead\":400,\"cacheWrite\":0,\"totalTokens\":1450},\"stopReason\":\"stop\",\"responseId\":\"syn-r1\",\"duration\":100.4,\"ttft\":50.4,\"timestamp\":1789706401000}}\n",
    );
    let rel = "--C--syn--/2026-01-05T10-00-00-000Z_00000000-0000-7000-8000-00000000cf00.jsonl";
    let file_path = dir.path().join("sessions").join(rel);
    let root = omp_root_with_file(&dir, rel, base.as_bytes());
    let (_db, storage) = temp_storage("omp-v12-conflict");
    run_omp(&storage, &root, NOW);

    // Append a record with the same type/id/parentId/timestamp but different usage.
    let conflict_line = "{\"type\":\"message\",\"id\":\"syn-cf-1\",\"parentId\":null,\"timestamp\":\"2026-01-05T10:00:01.000Z\",\"message\":{\"role\":\"assistant\",\"provider\":\"syn-provider\",\"model\":\"syn-model-a\",\"usage\":{\"input\":1500,\"output\":50,\"cacheRead\":400,\"cacheWrite\":0,\"totalTokens\":1950},\"stopReason\":\"stop\",\"responseId\":\"syn-r1\",\"duration\":100.4,\"ttft\":50.4,\"timestamp\":1789706401000}}\n";
    let mut appended = base.as_bytes().to_vec();
    appended.extend_from_slice(conflict_line.as_bytes());
    std::fs::write(&file_path, &appended).unwrap();
    let second = run_omp(&storage, &root, NOW + 1000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.conflicts, 1);

    let key = "omp:message:syn-cf-1:-:2026-01-05T10:00:01.000Z";
    // Retain input 1400 instead of selecting the greater value; flag and diagnose conflict.
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
