//! Gemini boundary scenarios, with synthetic labels in directories and headers.
//! Reject user tokens, undocumented message types and top-level schema deviations;
//! token-shape errors do not reject the whole file; cover one-time extra-key diagnostics, missing-ID index fallback, BOM and detection.
//! Expectations are manually calculated in each sample _expectations.md.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::gemini::GeminiAdapter;
use llm_usage_core::storage::Storage;

const NOW: i64 = 1_800_000_000_000;

fn diag_count(storage: &Storage, code: &str) -> i64 {
    storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = ?1",
            [code],
            |r| r.get(0),
        )
        .unwrap()
}

fn event_count(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap()
}

fn checkpoint_count(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn user_message_with_tokens_fails_closed_and_never_advances() {
    let (_db, storage) = temp_storage("gemini-uou");
    let root = gemini_fixture("synthetic-usage-on-user");

    let first = run_gemini(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "pending");
    assert_eq!(first[0].files[0].events, 0);
    assert_eq!(event_count(&storage), 0, "fail closed：0 事件入库");
    assert_eq!(diag_count(&storage, "usage_on_unexpected_message_type"), 1);
    assert_eq!(checkpoint_count(&storage), 0, "游标不推进（无 checkpoint）");

    // Second scan still rejects rather than returning silent zero success; one diagnostic per run.
    let second = run_gemini(&storage, &root, NOW + 1000);
    assert_eq!(
        second[0].files[0].status, "pending",
        "游标未推进，不触发无变化短路"
    );
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "usage_on_unexpected_message_type"), 2);
}

#[test]
fn undocumented_message_type_fails_closed() {
    let (_db, storage) = temp_storage("gemini-undoc");
    let root = gemini_fixture("synthetic-undocumented-type");
    let reports = run_gemini(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "pending");
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "undocumented_message_type"), 1);
    assert_eq!(checkpoint_count(&storage), 0);
}

#[test]
fn session_schema_deviation_fails_closed_per_file() {
    let (_db, storage) = temp_storage("gemini-dev");
    let root = gemini_fixture("synthetic-schema-deviation");
    let reports = run_gemini(&storage, &root, NOW);
    assert_eq!(reports[0].files.len(), 2);
    for file in &reports[0].files {
        assert_eq!(file.status, "pending", "{} 应 fail closed", file.file_id);
        assert_eq!(file.events, 0);
    }
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "session_schema_deviation"), 2);
}

#[test]
fn negative_token_skips_message_without_failing_closed() {
    let (_db, storage) = temp_storage("gemini-neg");
    let root = gemini_fixture("synthetic-negative-tokens");
    let reports = run_gemini(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1, "负值消息跳过，其余正常入账");
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
    // Source-file degraded health is visible.
    let file_status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_status, "degraded");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(1_000));
    assert_eq!(summary.totals.output_total_known, Some(50));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.total_tokens_known, Some(1_050));
    // Skip negative-valued message: no gemini:syn-sess-neg:syn-msg-bad event.
    let bad: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events \
             WHERE source_record_key = 'gemini:syn-sess-neg:syn-msg-bad'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0);
}

#[test]
fn unmapped_usage_keys_reported_once_per_scan() {
    let (_db, storage) = temp_storage("gemini-unmapped");
    let root = gemini_fixture("synthetic-unmapped-keys");
    let reports = run_gemini(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 2, "额外键不阻塞已映射字段入账");
    assert_eq!(
        diag_count(&storage, "unmapped_usage_keys"),
        1,
        "两条带额外键的消息只记一次诊断"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(300));
    assert_eq!(summary.totals.output_total_known, Some(50));
    assert_eq!(summary.totals.total_tokens_known, Some(350));
    assert_eq!(
        summary.totals.cache_read_known, None,
        "两条消息均未直报 cached，未知不补零"
    );
}

#[test]
fn missing_message_id_falls_back_to_array_index() {
    let (_db, storage) = temp_storage("gemini-noid");
    let root = gemini_fixture("synthetic-missing-id");
    let reports = run_gemini(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1);
    let (key, origin): (String, Option<String>) = storage
        .conn()
        .query_row(
            "SELECT source_record_key, origin_call_id FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(key, "gemini:syn-sess-noid:idx-1", "回退身份 = 数组下标");
    assert_eq!(origin, None);
    assert_eq!(diag_count(&storage, "missing_message_id"), 1);
}

#[test]
fn bom_prefixed_session_parses_normally() {
    let (_db, storage) = temp_storage("gemini-bom");
    let root = gemini_fixture("synthetic-bom");
    let reports = run_gemini(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(reports[0].files[0].diagnostics, 0);
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(120));
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("gemini-empty");
    let path = dir.path().join("session-empty.json");
    std::fs::write(&path, b"").unwrap();
    let adapter = GeminiAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn detect_pending_on_session_id_only() {
    // sessionId without messages may be an unfinished initial write; retry detection next run.
    let dir = TempDir::new("gemini-sidonly");
    let path = dir.path().join("session-sid.json");
    std::fs::write(&path, br#"{"sessionId": "syn-x""#).unwrap();
    let adapter = GeminiAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn detect_unknown_format_on_non_json_prefix() {
    let dir = TempDir::new("gemini-nonjson");
    let path = dir.path().join("session-nj.json");
    std::fs::write(&path, b"not a json document at all").unwrap();
    let adapter = GeminiAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn detect_unknown_format_on_missing_fingerprints() {
    let dir = TempDir::new("gemini-nofp");
    let path = dir.path().join("session-nf.json");
    std::fs::write(&path, br#"{"foo": 1}"#).unwrap();
    let adapter = GeminiAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}
