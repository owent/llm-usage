//! Qwen Code 适配器缺口场景：合成样本（目录/文件头均标 synthetic）。
//! 覆盖 fail closed（未知 type / 未知 subtype / 非 assistant 携带 usageMetadata）、
//! 无 usageMetadata 正常形状、负值跳过、额外键一次性诊断、缺 uuid 行号回退、detect 直测。
//! 期望值均为人工核算（见各 fixture 目录 _expectations.md）。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::qwen::QwenAdapter;
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
fn undocumented_record_type_fails_closed_and_never_advances() {
    let (_db, storage) = temp_storage("qwen-ut");
    let root = qwen_fixture("synthetic-undocumented-type");

    let first = run_qwen(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "pending");
    assert_eq!(first[0].files[0].events, 0);
    assert_eq!(event_count(&storage), 0, "fail closed：本轮事件清空");
    assert_eq!(diag_count(&storage, "undocumented_record_type"), 1);
    assert_eq!(checkpoint_count(&storage), 0, "游标不推进（无 checkpoint）");

    // 二次扫描：确定性再拒，诊断每轮一条。
    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "pending");
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "undocumented_record_type"), 2);
}

#[test]
fn undocumented_record_subtype_fails_closed() {
    let (_db, storage) = temp_storage("qwen-us");
    let root = qwen_fixture("synthetic-undocumented-subtype");
    let reports = run_qwen(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "pending");
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "undocumented_record_subtype"), 1);
    assert_eq!(checkpoint_count(&storage), 0);
}

#[test]
fn usage_on_tool_result_fails_closed() {
    let (_db, storage) = temp_storage("qwen-uotr");
    let root = qwen_fixture("synthetic-usage-on-tool-result");

    let first = run_qwen(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "pending");
    assert_eq!(
        event_count(&storage),
        0,
        "fail closed：已解析 assistant 事件清空"
    );
    assert_eq!(diag_count(&storage, "usage_on_unexpected_record_type"), 1);
    assert_eq!(checkpoint_count(&storage), 0);

    let second = run_qwen(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "pending");
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "usage_on_unexpected_record_type"), 2);
}

#[test]
fn assistant_without_usage_metadata_is_normal_shape() {
    let (_db, storage) = temp_storage("qwen-num");
    let root = qwen_fixture("synthetic-no-usage-metadata");
    let reports = run_qwen(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].records_seen, 3);
    assert_eq!(reports[0].files[0].events, 1, "无 usageMetadata 不产事件");
    assert_eq!(reports[0].files[0].diagnostics, 0, "正常形状无诊断");
    let diags: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(100));
    assert_eq!(summary.totals.total_tokens_known, Some(120));
}

#[test]
fn negative_usage_skips_record_without_failing_closed() {
    let (_db, storage) = temp_storage("qwen-neg");
    let root = qwen_fixture("synthetic-negative-usage");
    let reports = run_qwen(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1, "负值记录跳过，其余正常入账");
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
    let file_status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_status, "degraded");

    let bad: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE source_record_key = 'qwen:syn-a-bad'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(1_000));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.total_tokens_known, Some(1_050));
}

#[test]
fn unmapped_usage_keys_reported_once_per_file() {
    let (_db, storage) = temp_storage("qwen-unk");
    let root = qwen_fixture("synthetic-unmapped-keys");
    let reports = run_qwen(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 2, "额外键不阻塞已映射字段入账");
    assert_eq!(
        diag_count(&storage, "unmapped_usage_keys"),
        1,
        "两条带额外键的记录只记一次诊断"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(300));
    assert_eq!(summary.totals.output_total_known, Some(50));
    assert_eq!(summary.totals.total_tokens_known, Some(350));
    assert_eq!(
        summary.totals.cache_read_known, None,
        "两条均未直报 cached，未知不补零"
    );
}

#[test]
fn missing_uuid_falls_back_to_session_line_identity() {
    let (_db, storage) = temp_storage("qwen-mu");
    let root = qwen_fixture("synthetic-missing-uuid");
    let reports = run_qwen(&storage, &root, NOW);
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
    assert_eq!(key, "seq:syn-sess-mu:2", "回退身份 = sessionId + 行号");
    assert_eq!(origin, None);
    assert_eq!(diag_count(&storage, "missing_uuid"), 1);
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("qwen-empty");
    let path = dir.path().join("sess-empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = QwenAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn detect_unknown_format_on_missing_identity_fields() {
    // 首行 type 合法但缺 uuid/sessionId/timestamp 必填身份字段。
    let dir = TempDir::new("qwen-noident");
    let path = dir.path().join("sess-noident.jsonl");
    std::fs::write(
        &path,
        br#"{"type":"assistant","message":{"role":"assistant","content":"synthetic"}}
"#,
    )
    .unwrap();
    let adapter = QwenAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn detect_unknown_format_on_first_line_type_outside_set() {
    let dir = TempDir::new("qwen-badtype");
    let path = dir.path().join("sess-badtype.jsonl");
    std::fs::write(
        &path,
        br#"{"uuid":"syn-x","sessionId":"syn-s","timestamp":"2026-01-05T10:00:00.000Z","type":"brand_new"}
"#,
    )
    .unwrap();
    let adapter = QwenAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn detect_unknown_format_on_non_json_first_line() {
    let dir = TempDir::new("qwen-nonjson");
    let path = dir.path().join("sess-nonjson.jsonl");
    std::fs::write(
        &path,
        b"not a json line
",
    )
    .unwrap();
    let adapter = QwenAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}
