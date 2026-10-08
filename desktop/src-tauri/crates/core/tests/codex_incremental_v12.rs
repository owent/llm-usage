//! V12 incremental refresh: repeat scans, appended lines, incomplete lines across runs, truncation,
//! same-size replacement, rename detection, bounded scan resumption and conflicting duplicate finals.

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

#[test]
fn repeat_scan_does_not_increment() {
    let dir = TempDir::new("v12-repeat");
    let jsonl = reconstruct_codex_jsonl(&codex_fixture("rollout-49calls.sanitized.json"));
    let root = codex_root_with_file(&dir, "rollout-r.jsonl", &jsonl);
    let (_db, storage) = temp_storage("v12-repeat");

    let first = run_codex(&storage, &root, NOW);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 49);
    let revision_after_first = storage.data_revision().unwrap();

    let second = run_codex(&storage, &root, NOW + 1000);
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
    assert_eq!(summary.totals.call_count, 49);
    assert_eq!(summary.totals.input_total_known, Some(4_184_537));
}

#[test]
fn appended_lines_are_read_incrementally() {
    let dir = TempDir::new("v12-append");
    let jsonl = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    // Write ten lines without usage, then append the remaining seven.
    let text = String::from_utf8(jsonl).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    let tail = lines.split_off(10);
    let file_path = dir.path().join("sessions/2026/09/24/rollout-a.jsonl");
    let root = codex_root_with_file(
        &dir,
        "rollout-a.jsonl",
        (lines.join("\n") + "\n").as_bytes(),
    );
    let (_db, storage) = temp_storage("v12-append");

    let first = run_codex(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read, 10);
    assert_eq!(first[0].files[0].events, 0);

    std::fs::write(&file_path, (text.clone() + "").as_bytes()).unwrap();
    let second = run_codex(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 7,
        "only appended lines are read"
    );
    assert_eq!(second[0].files[0].events, 1);
    let _ = tail;

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(25_689));
    assert_eq!(second[0].reconciliations[0].verdict, "matched");
}

#[test]
fn half_line_is_not_consumed_until_completed() {
    let dir = TempDir::new("v12-half");
    let jsonl = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    let text = String::from_utf8(jsonl).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let (head, last) = (lines[..lines.len() - 1].join("\n"), lines[lines.len() - 1]);
    // Write every complete line except the last, plus the first half of that last line.
    let partial = format!("{head}\n{}", &last[..last.len() / 2]);
    let file_path = dir.path().join("sessions/2026/09/24/rollout-h.jsonl");
    let root = codex_root_with_file(&dir, "rollout-h.jsonl", partial.as_bytes());
    let (_db, storage) = temp_storage("v12-half");

    let first = run_codex(&storage, &root, NOW);
    assert_eq!(first[0].files[0].lines_read as usize, lines.len() - 1);
    assert_eq!(
        first[0].files[0].events, 1,
        "usage record on line 15 is complete"
    );

    // Complete the final task_complete line.
    std::fs::write(&file_path, format!("{head}\n{last}\n").as_bytes()).unwrap();
    let second = run_codex(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].lines_read, 1,
        "only the completed half line is new"
    );
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(
        summary.totals.call_count, 1,
        "no double counting after resume"
    );
}

#[test]
fn truncation_triggers_generation_rescan() {
    let dir = TempDir::new("v12-trunc");
    let jsonl = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    let file_path = dir.path().join("sessions/2026/09/24/rollout-t.jsonl");
    let root = codex_root_with_file(&dir, "rollout-t.jsonl", &jsonl);
    let (_db, storage) = temp_storage("v12-trunc");
    run_codex(&storage, &root, NOW);

    // Truncate to ten lines: redetect, increment generation and rescan from the start.
    let text = String::from_utf8(jsonl).unwrap();
    let head: String = text.lines().take(10).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::remove_file(&file_path).unwrap();
    std::fs::write(&file_path, head.as_bytes()).unwrap();
    let second = run_codex(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].lines_read, 10);
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1);
    // Retain stored history after truncation; the remaining ten lines contain no usage events.
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
}

#[test]
fn same_size_replacement_rescans_without_dropping_history() {
    let dir = TempDir::new("v12-replace");
    let one = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    let file_path = dir.path().join("sessions/2026/09/24/rollout-s.jsonl");
    let root = codex_root_with_file(&dir, "rollout-s.jsonl", &one);
    let (_db, storage) = temp_storage("v12-replace");
    run_codex(&storage, &root, NOW);
    let before = storage.data_revision().unwrap();

    // Same-size replacement swaps two complete valid JSON records with a supported cli_version,
    // changing the content fingerprint and requiring a rescan.
    let text = String::from_utf8(one.clone()).unwrap();
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    assert!(lines.len() >= 5, "fixture should have enough lines to swap");
    lines.swap(3, 4);
    let replaced: Vec<u8> = lines.concat().into_bytes();
    assert_eq!(replaced.len(), one.len());
    assert_ne!(replaced, one);
    std::fs::write(&file_path, &replaced).unwrap();
    let second = run_codex(&storage, &root, NOW + 1000);
    // Reordered lines change the fingerprint; the supported format is rescanned line by line.
    let generation: i64 = storage
        .conn()
        .query_row("SELECT generation FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(generation, 1, "same-size replacement bumps generation");
    assert!(storage.data_revision().unwrap() >= before);
    // Retain original session history while parsing the replacement content independently.
    assert!(second[0].files[0].lines_read > 0);
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert!(summary.totals.call_count >= 1);
}

#[test]
fn rename_keeps_identity_and_cursor() {
    let dir = TempDir::new("v12-rename");
    let jsonl = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    let old_path = dir.path().join("sessions/2026/09/24/rollout-old.jsonl");
    let new_path = dir.path().join("sessions/2026/09/24/rollout-new.jsonl");
    let root = codex_root_with_file(&dir, "rollout-old.jsonl", &jsonl);
    let (_db, storage) = temp_storage("v12-rename");
    run_codex(&storage, &root, NOW);

    std::fs::rename(&old_path, &new_path).unwrap();
    let second = run_codex(&storage, &root, NOW + 1000);
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
    assert!(file_id.ends_with("rollout-new.jsonl"));
    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
}

#[test]
fn budget_split_resumes_without_duplicates() {
    let dir = TempDir::new("v12-budget");
    let root_path = codex_fixture("synthetic-duplicate-final");
    // Copy synthetic test data to a temporary directory, preserving repository samples.
    let jsonl =
        std::fs::read(root_path.join("sessions/2026/01/05/rollout-synthetic-dup.jsonl")).unwrap();
    let root = codex_root_with_file(&dir, "rollout-b.jsonl", &jsonl);
    let (_db, storage) = temp_storage("v12-budget");

    let first = run_codex_with_limits(&storage, &root, NOW, budgeted_limits(3));
    assert_eq!(first[0].files[0].status, "budget_exhausted");
    assert_eq!(
        first[0].finish,
        llm_usage_core::jobs::RunStatus::Interrupted
    );
    assert_eq!(first[0].files[0].lines_read, 3);
    assert_eq!(first[0].files[0].events, 0, "usage lines not reached yet");

    let second = run_codex_with_limits(&storage, &root, NOW + 1000, budgeted_limits(4));
    assert_eq!(second[0].files[0].status, "budget_exhausted");
    assert_eq!(second[0].files[0].events, 3);

    let third = run_codex_with_limits(&storage, &root, NOW + 2000, budgeted_limits(100));
    assert_eq!(third[0].files[0].status, "complete");
    assert_eq!(third[0].reconciliations[0].verdict, "matched");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(
        summary.totals.call_count, 2,
        "split rounds never double count"
    );
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
}

#[test]
fn conflicting_duplicate_final_marks_conflict_and_keeps_existing() {
    let dir = TempDir::new("v12-conflict");
    let jsonl = std::fs::read(
        codex_fixture("synthetic-duplicate-final")
            .join("sessions/2026/01/05/rollout-synthetic-dup.jsonl"),
    )
    .unwrap();
    let file_path = dir.path().join("sessions/2026/09/24/rollout-c.jsonl");
    let root = codex_root_with_file(&dir, "rollout-c.jsonl", &jsonl);
    let (_db, storage) = temp_storage("v12-conflict");
    run_codex(&storage, &root, NOW);

    // Append a final with the same response_id and different values; unknown revision order means conflict.
    let conflict_line = "{\"timestamp\":\"2026-01-05T10:00:10.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"thread_id\":\"syn-sess-dup\",\"turn_id\":\"syn-turn-1\",\"session_id\":\"syn-sess-dup\",\"response_id\":\"syn-resp-1\",\"usage\":{\"input_tokens\":1500,\"cached_input_tokens\":400,\"cache_write_input_tokens\":0,\"output_tokens\":50,\"reasoning_output_tokens\":10,\"total_tokens\":1550}}}\n";
    let mut appended = jsonl.clone();
    appended.extend_from_slice(conflict_line.as_bytes());
    std::fs::write(&file_path, &appended).unwrap();
    let second = run_codex(&storage, &root, NOW + 1000);
    let outcome = second[0].outcome.as_ref().unwrap();
    assert_eq!(outcome.conflicts, 1);

    // Do not choose the larger value: retain 1000 and record the conflict diagnostic.
    let input: i64 = storage
        .conn()
        .query_row(
            "SELECT input_total FROM usage_events WHERE source_record_key = 'resp:syn-resp-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(input, 1000);
    let conflict_flag: i64 = storage
        .conn()
        .query_row(
            "SELECT conflict FROM usage_events WHERE source_record_key = 'resp:syn-resp-1'",
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
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.conflict_count, 1);
}
