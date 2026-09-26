//! Codex 适配器合同测试：M0 真实脱敏 fixture（本机 0.155.0-alpha.16.3）
//! 经 读取→解析→标准化→commit_batch→查询 全链路，期望与
//! tests/fixtures/codex/*._expectations.md 的人工核算值一致（含本文件顶部注释中
//! 从 fixture 手工核算的分模型/分段数值）。

mod common;

use common::*;
use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{SourceAdapter, SourceFileRow};
use rusqlite::OptionalExtension;

// 手工核算值（jq 对 fixture 逐条求和，与 _expectations.md 互核）：
// multimodel：gpt-6-astra 169 次 input 23,683,841 cached 22,957,312 output 109,592
//   reasoning 28,759 total 23,793,433；gpt-6-sol 52 次 input 8,377,585
//   cached 8,098,560 output 33,789 reasoning 16,739 total 8,411,374；
//   最终快照 total 31,724,335；compacted 携带 250,108 + 230,364 = 480,472；
//   32,204,807 = 31,724,335 + 480,472。

fn setup_fixture(tag: &str, fixture: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new(tag);
    let jsonl = reconstruct_codex_jsonl(&codex_fixture(fixture));
    let root = codex_root_with_file(&dir, "rollout-reconstructed.jsonl", &jsonl);
    (dir, root)
}

#[test]
fn single_call_full_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("codex-single", "rollout-single-call.sanitized.json");
    let (_db, storage) = temp_storage("codex-single");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 1, "exactly one model call");
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (1, 0, 0));

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(25_558));
    assert_eq!(summary.totals.cache_read_known, Some(4_864));
    assert_eq!(summary.totals.output_total_known, Some(131));
    assert_eq!(summary.totals.total_tokens_known, Some(25_689));

    // auto-review 子代理会话：parent_thread_id ⇒ sub_agent；模型来自 turn_context。
    let (category, model, attribution): (String, String, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, model_raw, model_attribution FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(model, "codex-auto-review");
    assert_eq!(attribution, "provider_mapping");

    // 单次会话：Σ逐次 == 最终快照，对账 matched。
    assert_eq!(report.reconciliations.len(), 1);
    let rec = &report.reconciliations[0];
    assert_eq!(rec.verdict, "matched");
    assert_eq!(rec.detail_sum, 25_689);
    assert_eq!(rec.snapshot_final, Some(25_689));
    assert_eq!(rec.carried_sum, 0);
    assert_eq!(rec.difference, Some(0));

    // 快照存为来源区间汇总（对照，不求和）。
    let snap_total: Option<i64> = storage
        .conn()
        .query_row("SELECT total_tokens FROM source_aggregates", [], |r| {
            r.get(0)
        })
        .optional()
        .unwrap();
    assert_eq!(snap_total, Some(25_689));
    let _ = dir;
}

#[test]
fn calls_49_full_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("codex-49", "rollout-49calls.sanitized.json");
    let (_db, storage) = temp_storage("codex-49");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 49);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 49);
    assert_eq!(summary.totals.input_total_known, Some(4_184_537));
    assert_eq!(summary.totals.cache_read_known, Some(4_022_016));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(4_051));
    assert_eq!(summary.totals.total_tokens_known, Some(4_188_588));
    // 缓存输入占比 = 4,022,016 / 4,184,537 ≈ 96.1%（auto-review 高命中）。
    let ratio = summary.totals.cache_input_ratio().unwrap();
    assert!((ratio.as_f64() - 0.961).abs() < 0.001);
    assert_eq!(summary.totals.ratio_sample_count(), 49);
    // 同一 rollout 一个会话。
    assert_eq!(summary.periods.len(), 1);
    assert_eq!(summary.periods[0].distinct_sessions, Some(1));

    // 推理 token 是 output 子集，不再加总：从事件表直接核验。
    let reasoning: i64 = storage
        .conn()
        .query_row("SELECT SUM(output_reasoning) FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(reasoning, 725);

    let rec = &report.reconciliations[0];
    assert_eq!(rec.verdict, "matched");
    assert_eq!(rec.snapshot_final, Some(4_188_588));
    let _ = dir;
}

#[test]
fn multimodel_compaction_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("codex-mm", "rollout-multimodel-full.sanitized.json");
    let (_db, storage) = temp_storage("codex-mm");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(
        report.files[0].events, 221,
        "token_usage_record 计数即调用数"
    );

    let summary = summary(&storage, "2026-09-23", "2026-09-24");
    assert_eq!(summary.totals.call_count, 221);
    assert_eq!(summary.totals.input_total_known, Some(32_061_426));
    assert_eq!(summary.totals.cache_read_known, Some(31_055_872));
    assert_eq!(summary.totals.output_total_known, Some(143_381));
    assert_eq!(summary.totals.total_tokens_known, Some(32_204_807));

    // 模型按 turn_context 位置归属：两模型分段（人工核算值见文件头注释）。
    let breakdown: std::collections::BTreeMap<String, (i64, i64, i64)> = summary
        .model_breakdown
        .iter()
        .map(|row| {
            (
                row.model_raw.clone().unwrap_or_default(),
                (
                    row.sums.call_count,
                    row.sums.input_total_known.unwrap_or(0),
                    row.sums.total_tokens_known.unwrap_or(0),
                ),
            )
        })
        .collect();
    assert_eq!(breakdown["gpt-6-astra"], (169, 23_683_841, 23_793_433));
    assert_eq!(breakdown["gpt-6-sol"], (52, 8_377_585, 8_411_374));
    assert_eq!(
        breakdown.len(),
        2,
        "no unknown-model row: turn_context covers all calls"
    );

    // compaction 重置快照：Σ逐次 == 最终快照 + 携带记录（480,472）。
    let rec = &report.reconciliations[0];
    assert_eq!(rec.verdict, "matched");
    assert_eq!(rec.detail_sum, 32_204_807);
    assert_eq!(rec.snapshot_final, Some(31_724_335));
    assert_eq!(rec.carried_sum, 480_472);
    assert_eq!(rec.difference, Some(0));

    // 快照区间汇总保存最终值（对照），不是逐次合计。
    let snap_total: i64 = storage
        .conn()
        .query_row("SELECT total_tokens FROM source_aggregates", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(snap_total, 31_724_335);

    // 主会话（无 parent_thread_id）：category=primary。
    let categories: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE call_category = 'primary'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(categories, 221);
    let _ = dir;
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = CodexAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "codex");
    // supported_versions 由版本注册表生成（M2-D 逐版本 fixture + 2026-09-26
    // 0.139–0.151 旧载体取证登记）。
    assert_eq!(
        json["supported_versions"],
        serde_json::json!([
            "0.155.0-alpha.16.3",
            "0.154.0-alpha.6.2",
            "0.154.0-alpha.6.1",
            "0.153.0",
            "0.151.0-alpha.7.1",
            "0.149.0-alpha.4.1",
            "0.148.0-alpha.9",
            "0.147.0-alpha.6.5",
            "0.146.0-alpha.9.2",
            "0.146.0-alpha.3.1",
            "0.146.0-alpha.3",
            "0.145.0-alpha.27",
            "0.145.0-alpha.18",
            "0.144.5",
            "0.144.2",
            "0.144.0-alpha.4",
            "0.142.5",
            "0.142.4",
            "0.142.3",
            "0.142.2",
            "0.142.0",
            "0.142.0-alpha.6",
            "0.142.0-alpha.1",
            "0.140.0-alpha.2",
            "0.139.0"
        ])
    );
    // 字段能力八项齐全。
    for key in [
        "tokens",
        "cache_read",
        "cache_write",
        "per_request_calls",
        "model",
        "time",
        "cost",
        "latency",
    ] {
        assert!(
            json["fields"].get(key).is_some(),
            "capability fields missing {key}"
        );
    }
    assert_eq!(
        json["fields"]["cost"]["availability"],
        serde_json::json!({"unavailable": "本地无费用字段；远端账单/账号不接入"})
    );
    assert_eq!(json["fields"]["tokens"]["availability"], "available");
    for section in [
        "discovery",
        "detection",
        "lifecycle",
        "incremental",
        "dedup",
        "integrity",
        "maintenance",
        "scheduling",
    ] {
        assert!(json.get(section).is_some(), "capability missing {section}");
    }
    assert!(!cap.limitations.is_empty());
    // 能力声明可落库（source_instances.capabilities）。
    let (_db, storage) = temp_storage("codex-cap");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "codex@test".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        1_800_000_000_000,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'codex@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
}

#[test]
fn source_file_row_composite_roundtrip() {
    let (_db, storage) = temp_storage("codex-filerow");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "codex@test".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({}),
            health: "ok".to_string(),
        },
        1,
    )
    .unwrap();
    let row = SourceFileRow {
        file_id: "p".to_string(),
        file_identity: "id-1".to_string(),
        generation: 3,
        len: 1234,
        mtime_ms: 55,
        created_ms: Some(77),
        head_hash: 0xABCD,
        head_len: 1234,
        tail_hash: 0xEF01,
        status: "active".to_string(),
        format_status: None,
    };
    llm_usage_core::adapters::framework::upsert_source_file(&storage, "codex@test", &row, 1)
        .unwrap();
    let loaded = llm_usage_core::adapters::framework::load_source_file(&storage, "codex@test", "p")
        .unwrap()
        .unwrap();
    assert_eq!(loaded, row);
    // 改名：同身份新路径按身份命中。
    let by_identity = llm_usage_core::adapters::framework::find_source_file_by_identity(
        &storage,
        "codex@test",
        "id-1",
    )
    .unwrap()
    .unwrap();
    assert_eq!(by_identity.file_id, "p");
}
