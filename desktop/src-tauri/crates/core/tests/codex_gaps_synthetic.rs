//! Codex 适配器缺口场景：合成样本（目录/文件头均标 synthetic）与 V17 拒绝。
//! 合成样本覆盖 M0 真实样本缺失的场景：重复 final、子 Agent、cache write>0、
//! 无 usage 的 tool/user 消息、缺 response_id；未知版本/未知格式 fail closed。

mod common;

use common::*;
use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::usage_map::{map_codex_record, CodexRecordUsage};

fn synthetic_root(name: &str) -> std::path::PathBuf {
    codex_fixture(name)
}

#[test]
fn duplicate_final_counts_once() {
    let (_db, storage) = temp_storage("codex-dup");
    let root = synthetic_root("synthetic-duplicate-final");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    // 扫描产出 3 条事件（含重复 final），入库去重后 2 次调用。
    assert_eq!(report.files[0].events, 3);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 2);
    assert_eq!(
        outcome.unchanged, 1,
        "identical duplicate final is idempotent"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(3_000));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.output_total_known, Some(150));
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
    assert_eq!(report.reconciliations[0].verdict, "matched");
}

#[test]
fn subagent_session_category_and_host_mapping() {
    let (_db, storage) = temp_storage("codex-sub");
    let root = synthetic_root("synthetic-subagent");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let (category, host, parent, agent): (String, String, String, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, host_application, parent_session_id, agent FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(
        host, "vscode",
        "codex_vscode originator maps to vscode host"
    );
    assert_eq!(parent, "syn-parent-1");
    assert_eq!(agent, "codex");
}

#[test]
fn cache_write_positive_maps_reported_write_and_derived_uncached() {
    let (_db, storage) = temp_storage("codex-cw");
    let root = synthetic_root("synthetic-cache-write");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let (write, uncached, input, quality): (i64, i64, i64, String) = storage
        .conn()
        .query_row(
            "SELECT input_cache_write, input_uncached, input_total, quality_json FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(write, 100);
    assert_eq!(uncached, 500, "uncached = 1000 - 400 - 100");
    assert_eq!(input, 1000);
    let quality: serde_json::Value = serde_json::from_str(&quality).unwrap();
    assert_eq!(quality["input_cache_write"], "reported");
    assert_eq!(quality["input_uncached"], "derived");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.cache_write_known, Some(100));
    assert_eq!(summary.totals.total_tokens_known, Some(1_050));
}

#[test]
fn tool_and_user_messages_without_usage_produce_no_calls() {
    let (_db, storage) = temp_storage("codex-nu");
    let root = synthetic_root("synthetic-no-usage-messages");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 9);
    assert_eq!(report.files[0].events, 0, "no usage evidence => no events");
    assert_eq!(report.reconciliations[0].verdict, "no_snapshot");
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "no phantom requests from tool/user messages");
}

#[test]
fn missing_response_id_falls_back_to_session_ordinal_identity() {
    let dir = TempDir::new("codex-norid");
    let file = concat!(
        "{\"timestamp\":\"2026-01-05T15:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-nr\",\"session_id\":\"syn-sess-nr\",\"timestamp\":\"2026-01-05T15:00:00.000Z\",\"cli_version\":\"0.155.0-alpha.16.3\",\"model_provider\":\"openai\"}}\n",
        "{\"timestamp\":\"2026-01-05T15:00:01.000Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"t\",\"model\":\"m\"}}\n",
        "{\"timestamp\":\"2026-01-05T15:00:02.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"thread_id\":\"syn-sess-nr\",\"turn_id\":\"t\",\"session_id\":\"syn-sess-nr\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":0,\"cache_write_input_tokens\":0,\"output_tokens\":5,\"reasoning_output_tokens\":0,\"total_tokens\":15}}}\n",
    );
    let root = codex_root_with_file(&dir, "rollout-norid.jsonl", file.as_bytes());
    let (_db, storage) = temp_storage("codex-norid");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let key: String = storage
        .conn()
        .query_row("SELECT source_record_key FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        key, "seq:syn-sess-nr:3",
        "fallback = session UUID + line number"
    );
    let diag: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'missing_response_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);
}

#[test]
fn v17_unknown_version_fails_closed_not_success_zero() {
    let adapter = CodexAdapter::new();
    let path = synthetic_root("synthetic-unknown-version")
        .join("sessions/2026/01/05/rollout-synthetic-uv.jsonl");
    let outcome = adapter.detect(&path).unwrap();
    assert_eq!(
        outcome,
        DetectOutcome::UnsupportedVersion {
            format: "codex-rollout-jsonl".to_string(),
            found: "0.999.0-synthetic".to_string(),
        }
    );

    let (_db, storage) = temp_storage("codex-uv");
    let root = synthetic_root("synthetic-unknown-version");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "unsupported_version");
    assert_eq!(report.files[0].detail.as_deref(), Some("0.999.0-synthetic"));
    assert_eq!(report.files[0].events, 0);
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unsupported_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        diags, 1,
        "explicit refusal recorded; not a silent success-zero"
    );
    let file_status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_status, "unsupported");
}

#[test]
fn v17_unknown_format_fails_closed_not_success_zero() {
    let adapter = CodexAdapter::new();
    let path = synthetic_root("synthetic-not-codex")
        .join("sessions/2026/01/05/rollout-synthetic-nc.jsonl");
    let outcome = adapter.detect(&path).unwrap();
    assert!(matches!(outcome, DetectOutcome::UnknownFormat { .. }));

    let (_db, storage) = temp_storage("codex-nc");
    let root = synthetic_root("synthetic-not-codex");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "unknown_format");
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unknown_format'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diags, 1);
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("codex-empty");
    let path = dir.path().join("rollout-empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = CodexAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn map_codex_record_full_field_contract() {
    // 合成样本：cache_write>0 的六字段映射（synthetic-cache-write 同值）。
    let mapped = map_codex_record(&CodexRecordUsage {
        input_tokens: 1000,
        cached_input_tokens: 400,
        cache_write_input_tokens: 100,
        output_tokens: 50,
        reasoning_output_tokens: 10,
        total_tokens: 1050,
    });
    assert_eq!(mapped.usage.input_uncached, Some(500));
    assert_eq!(mapped.usage.input_cache_write, Some(100));
    assert_eq!(mapped.usage.total_tokens, Some(1050));
    assert_eq!(mapped.usage.source_total, Some(1050));
    assert!(mapped.diagnostics.is_empty());

    // 矛盾不钳制：cached + write > input → uncached 保持未知并进诊断。
    let contradiction = map_codex_record(&CodexRecordUsage {
        input_tokens: 100,
        cached_input_tokens: 90,
        cache_write_input_tokens: 50,
        output_tokens: 10,
        reasoning_output_tokens: 0,
        total_tokens: 110,
    });
    assert_eq!(contradiction.usage.input_uncached, None);
    assert!(contradiction
        .diagnostics
        .iter()
        .any(|d| d.code == "negative_derived_field"));
}
