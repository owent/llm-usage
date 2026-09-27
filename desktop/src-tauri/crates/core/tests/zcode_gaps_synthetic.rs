//! ZCode 适配器缺口场景：九个合成 fixture（目录/文件头均标 synthetic）全覆盖，
//! 期望值为人工核算（各 fixture 目录 _expectations.md 指向本文件头部注释）。
//!
//! 手工核算（AI SDK 五键 {in, out, total, cr, cw}；anthropic {in, out, cr, cw?}）：
//! - cache-write：{2000,100,2100,800,200} + anthropic{1000,100,800,200}；
//!   1000+800+200=2000 ✓ 双口径一致，0 诊断；uncached=2000-800-200=1000；
//!   汇总 input=2000 out=100 cr=800 cw=Some(200) total=2100。
//! - dual-fallback：rec1 usage 缺席、anthropic{200,50,800,100} 在场 ⇒ 互斥回退
//!   对照口径（input_total=1100 derived、uncached=200 reported、source_total=None）；
//!   rec2 AI SDK{1000,100,1100,400,0} 无对照视图 ⇒ 主口径；
//!   汇总 input=2100 out=150 cr=1200 cw=Some(100) total=2250；1 条 ai_sdk_usage_missing。
//! - dual-mismatch：AI SDK in=1000 vs anthropic 999+400+0=1399 ≠ 1000 ⇒ 1 条
//!   dual_caliber_mismatch；AI SDK 主口径保留（input=1000 total=1100 source=1100）。
//! - epoch-timestamps：completedAt 数字 1800000000000（毫秒）与 1800000000（<1e11
//!   折算秒）⇒ 两事件 occurred_at_ms 均 1,800,000,000,000（2027-01-15 UTC）；
//!   汇总 input=1500 out=150 cr=400 cw=Some(0) total=1650。
//! - future-version：9.9.9 未收录 ⇒ latest_fallback 兼容尝试，结构通过 ⇒
//!   active_compat 标记统计；汇总 input=3000 out=300 cr=1200 total=3300。
//! - missing-request-id：缺 requestId ⇒ seq:syn-sess-1:1 + missing_request_id 诊断；
//!   汇总 input=1000 total=1100。
//! - negative-usage：inputTokens=-5 ⇒ usage_shape_deviation、该条跳过、文件 degraded；
//!   其余正常：input=1000 total=1100。
//! - no-usage：末条 finishReason=null 无 usage ⇒ 正常形状不产事件不失败；
//!   input=1000 total=1100，0 诊断。
//! - undocumented-type：第二条 type=other_event ⇒ 扫描层 fail closed：
//!   事件清空、游标不推进、下轮确定性再拒。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::zcode::ZcodeAdapter;
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

fn file_status(storage: &Storage) -> String {
    storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn cache_write_positive_with_consistent_dual_calibers() {
    // 真实样本全 cw=0；本合成场景 cw=200 且 anthropic cache_creation=200 在场，
    // 双口径一致（1000+800+200=2000），0 诊断。
    let jsonl =
        reconstruct_jsonl_projection(&zcode_fixture("synthetic-cache-write").join("records.json"));
    let dir = TempDir::new("zcode-cw-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-cw");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(reports[0].files[0].diagnostics, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(2_000));
    assert_eq!(summary.totals.output_total_known, Some(100));
    assert_eq!(summary.totals.cache_read_known, Some(800));
    assert_eq!(summary.totals.cache_write_known, Some(200));
    assert_eq!(summary.totals.total_tokens_known, Some(2_100));

    let (uncached, cache_write, source_total): (i64, i64, i64) = storage
        .conn()
        .query_row(
            "SELECT input_uncached, input_cache_write, source_total FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(uncached, 1_000, "input_uncached = 2000-(800+200)");
    assert_eq!(cache_write, 200, "cacheWriteTokens 直报");
    assert_eq!(source_total, 2_100);
    let diags: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
}

#[test]
fn dual_caliber_exclusive_fallback_never_sums() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-dual-fallback").join("records.json"),
    );
    let dir = TempDir::new("zcode-df-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-df");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 2);
    assert_eq!(diag_count(&storage, "ai_sdk_usage_missing"), 1);

    // rec1（回退 anthropic 对照口径）：input_total=1100（derived）、
    // uncached=200（reported）、source_total=None（对照口径无 totalTokens）。
    let (input_total, uncached, source_total, cache_write): (i64, i64, Option<i64>, i64) = storage
        .conn()
        .query_row(
            "SELECT input_total, input_uncached, source_total, input_cache_write \
             FROM usage_events WHERE source_record_key = 'zcode:syn-req-1:1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(input_total, 1_100, "200+800+100（derived），绝不相加两口径");
    assert_eq!(uncached, 200);
    assert_eq!(source_total, None);
    assert_eq!(cache_write, 100);

    // rec2（AI SDK 主口径无对照视图）：source_total=1100。
    let source2: Option<i64> = storage
        .conn()
        .query_row(
            "SELECT source_total FROM usage_events WHERE source_record_key = 'zcode:syn-req-2:1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(source2, Some(1_100));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(2_100));
    assert_eq!(summary.totals.output_total_known, Some(150));
    assert_eq!(summary.totals.cache_read_known, Some(1_200));
    assert_eq!(summary.totals.cache_write_known, Some(100));
    assert_eq!(summary.totals.total_tokens_known, Some(2_250));
}

#[test]
fn dual_caliber_mismatch_diagnosed_and_ai_sdk_kept() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-dual-mismatch").join("records.json"),
    );
    let dir = TempDir::new("zcode-dm-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-dm");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(diag_count(&storage, "dual_caliber_mismatch"), 1);

    let (input_total, total, source_total, uncached): (i64, i64, i64, i64) = storage
        .conn()
        .query_row(
            "SELECT input_total, total_tokens, source_total, input_uncached FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(input_total, 1_000, "AI SDK 主口径保留（1000，非 1399）");
    assert_eq!(total, 1_100);
    assert_eq!(source_total, 1_100);
    assert_eq!(uncached, 600);
    // 矛盾是对照结论，不是形状损坏：文件保持 active。
    assert_eq!(file_status(&storage), "active");
}

#[test]
fn epoch_numeric_timestamps_normalize_to_milliseconds() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-epoch-timestamps").join("records.json"),
    );
    let dir = TempDir::new("zcode-ep-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-ep");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 2);

    // 毫秒值 1800000000000 与秒值 1800000000（<1e11 折算）⇒ 同一毫秒。
    let times: Vec<i64> = {
        let conn = storage.conn();
        let mut stmt = conn
            .prepare("SELECT occurred_at_ms FROM usage_events ORDER BY occurred_at_ms")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(times, vec![NOW, NOW]);

    let summary = summary(&storage, "2027-01-15", "2027-01-15");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(1_500));
    assert_eq!(summary.totals.output_total_known, Some(150));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(1_650));
}

#[test]
fn future_version_uses_latest_fallback_with_compat_mark() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-future-version").join("records.json"),
    );
    let dir = TempDir::new("zcode-fv-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-fv");
    let reports = run_zcode(&storage, &root, NOW);

    // 结构通过 ⇒ 数据照常统计（added=2），版本兼容性未验证单独标记。
    assert_eq!(reports[0].outcome.as_ref().unwrap().added, 2);
    assert_eq!(file_status(&storage), "active_compat");
    assert_eq!(diag_count(&storage, "latest_fallback"), 1);
    let (basis, schema): (Option<String>, String) = storage
        .conn()
        .query_row(
            "SELECT parse_basis, schema_version FROM usage_events LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(basis.as_deref(), Some("latest_fallback"));
    assert_eq!(schema, "9.9.9", "未收录版本号如实透传");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(3_000));
    assert_eq!(summary.totals.output_total_known, Some(300));
    assert_eq!(summary.totals.cache_read_known, Some(1_200));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(3_300));
}

#[test]
fn future_version_keeps_valid_calls_but_flags_new_invalid_usage() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-future-version").join("records.json"),
    );
    let dir = TempDir::new("zcode-fv-partial-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_db, storage) = temp_storage("zcode-fv-partial");
    run_zcode(&storage, &root, NOW);
    assert_eq!(file_status(&storage), "active_compat");

    let mut bad: serde_json::Value =
        serde_json::from_str(String::from_utf8(jsonl).unwrap().lines().next().unwrap()).unwrap();
    bad["requestId"] = serde_json::json!("syn-invalid-new-request");
    bad["response"]["usage"]["inputTokens"] = serde_json::json!(-1);
    let file = root.join("rollout/model-io-synthetic.jsonl");
    let mut appended = serde_json::to_vec(&bad).unwrap();
    appended.push(b'\n');
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(file)
        .unwrap()
        .write_all(&appended)
        .unwrap();

    run_zcode(&storage, &root, NOW + 1);
    assert_eq!(file_status(&storage), "degraded");
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
    assert_eq!(event_count(&storage), 2, "validated calls remain counted");
    let health: String = storage
        .conn()
        .query_row("SELECT health FROM source_instances", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        health, "degraded",
        "bad records must not hide behind compatibility status"
    );
    let format_status: String = storage
        .conn()
        .query_row("SELECT format_status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert!(format_status.contains("latest_fallback"));
}

#[test]
fn missing_request_id_falls_back_to_session_line_identity() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-missing-request-id").join("records.json"),
    );
    let dir = TempDir::new("zcode-mr-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-mr");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(diag_count(&storage, "missing_request_id"), 1);

    let (key, origin, attempt, session): (String, Option<String>, Option<String>, Option<String>) =
        storage
            .conn()
            .query_row(
                "SELECT source_record_key, origin_call_id, attempt_id, session_id FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
    assert_eq!(key, "seq:syn-sess-1:1", "回退身份 = sessionId + 行号");
    assert_eq!(origin, None);
    assert_eq!(attempt.as_deref(), Some("1"));
    assert_eq!(session.as_deref(), Some("syn-sess-1"));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(1_000));
    assert_eq!(summary.totals.total_tokens_known, Some(1_100));
}

#[test]
fn negative_usage_skips_record_and_degrades_file() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-negative-usage").join("records.json"),
    );
    let dir = TempDir::new("zcode-ng-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-ng");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1, "负值记录跳过，其余正常入账");
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
    assert_eq!(file_status(&storage), "degraded");

    let bad: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE source_record_key = 'zcode:syn-bad:1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(1_000));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.total_tokens_known, Some(1_100));
}

#[test]
fn in_flight_tail_without_usage_is_normal_shape() {
    let jsonl =
        reconstruct_jsonl_projection(&zcode_fixture("synthetic-no-usage").join("records.json"));
    let dir = TempDir::new("zcode-nu-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-nu");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].records_seen, 2);
    assert_eq!(
        reports[0].files[0].events, 1,
        "finishReason=null 无 usage 不产事件"
    );
    assert_eq!(reports[0].files[0].diagnostics, 0, "正常形状无诊断");
    assert_eq!(file_status(&storage), "active");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(1_000));
    assert_eq!(summary.totals.total_tokens_known, Some(1_100));
}

#[test]
fn session_title_classified_auxiliary_and_counted() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-session-title").join("records.json"),
    );
    let dir = TempDir::new("zcode-st-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-st");
    let reports = run_zcode(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 2);

    let (aux, primary): (i64, i64) = storage
        .conn()
        .query_row(
            "SELECT SUM(call_category = 'auxiliary'), SUM(call_category = 'primary') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (aux, primary),
        (1, 1),
        "session_title ⇒ auxiliary，照常计入"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(1_050));
    assert_eq!(summary.totals.output_total_known, Some(110));
    assert_eq!(summary.totals.total_tokens_known, Some(1_160));
}

#[test]
fn undocumented_type_fails_closed_and_never_advances() {
    let jsonl = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-undocumented-type").join("records.json"),
    );
    let dir = TempDir::new("zcode-ut-src");
    let root = zcode_root_with_file(&dir, "model-io-synthetic.jsonl", &jsonl);
    let (_d, storage) = temp_storage("zcode-ut");

    let first = run_zcode(&storage, &root, NOW);
    assert_eq!(first[0].files[0].status, "pending");
    assert_eq!(first[0].files[0].events, 0);
    assert_eq!(
        event_count(&storage),
        0,
        "fail closed：已解析 model_io 事件清空"
    );
    assert_eq!(diag_count(&storage, "undocumented_record_type"), 1);
    assert_eq!(checkpoint_count(&storage), 0, "游标不推进（无 checkpoint）");

    // 二次扫描：确定性再拒，诊断每轮一条。
    let second = run_zcode(&storage, &root, NOW + 1000);
    assert_eq!(second[0].files[0].status, "pending");
    assert_eq!(event_count(&storage), 0);
    assert_eq!(diag_count(&storage, "undocumented_record_type"), 2);
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("zcode-empty");
    let path = dir.path().join("model-io-empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = ZcodeAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn detect_unknown_format_on_non_json_first_line() {
    let dir = TempDir::new("zcode-nonjson");
    let path = dir.path().join("model-io-nonjson.jsonl");
    std::fs::write(&path, b"not a json line\n").unwrap();
    let adapter = ZcodeAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn detect_unknown_format_on_type_outside_model_io() {
    let dir = TempDir::new("zcode-badtype");
    let path = dir.path().join("model-io-badtype.jsonl");
    std::fs::write(
        &path,
        br#"{"type":"other_event","sessionId":"syn-s","payload":{"note":"synthetic"}}
"#,
    )
    .unwrap();
    let adapter = ZcodeAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn detect_unknown_format_without_session_identity() {
    // type=model_io 但缺 sessionId 身份字段：fail closed。
    let dir = TempDir::new("zcode-noident");
    let path = dir.path().join("model-io-noident.jsonl");
    std::fs::write(
        &path,
        br#"{"type":"model_io","requestId":"syn-r","attempt":1}
"#,
    )
    .unwrap();
    let adapter = ZcodeAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

#[test]
fn detect_dispatches_by_version_anchor() {
    let adapter = ZcodeAdapter::new();
    // 已收录 3.14.3（真实 fixture 首行）⇒ KnownVersion。
    let jsonl =
        reconstruct_jsonl_projection(&zcode_fixture("real-main-session").join("sanitized.json"));
    let dir = TempDir::new("zcode-ver");
    let known = dir.path().join("model-io-known.jsonl");
    std::fs::write(&known, &jsonl).unwrap();
    match adapter.detect(&known).unwrap() {
        DetectOutcome::Supported {
            format,
            format_version,
            basis,
        } => {
            assert_eq!(format, "zcode-modelio-jsonl");
            assert_eq!(format_version.as_deref(), Some("3.14.3"));
            assert_eq!(basis, llm_usage_core::domain::VersionBasis::KnownVersion);
        }
        other => panic!("expected Supported, got {other:?}"),
    }
    // 未收录 9.9.9（synthetic-future-version 首行）⇒ LatestFallback 兼容尝试。
    let future = reconstruct_jsonl_projection(
        &zcode_fixture("synthetic-future-version").join("records.json"),
    );
    let unknown = dir.path().join("model-io-future.jsonl");
    std::fs::write(&unknown, &future).unwrap();
    match adapter.detect(&unknown).unwrap() {
        DetectOutcome::Supported {
            format_version,
            basis,
            ..
        } => {
            assert_eq!(format_version.as_deref(), Some("9.9.9"));
            assert_eq!(basis, llm_usage_core::domain::VersionBasis::LatestFallback);
        }
        other => panic!("expected Supported, got {other:?}"),
    }
}
