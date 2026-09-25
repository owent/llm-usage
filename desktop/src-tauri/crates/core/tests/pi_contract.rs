//! pi 适配器合同测试：M2-B/C 恢复阶段真实脱敏 fixture（本机 pi 0.87.1，
//! session-error-zero-usage）。经 读取→解析→标准化→commit_batch→查询 全链路，
//! 期望与 tests/fixtures/pi/session-error-zero-usage._expectations.md 的人工核算一致。

mod common;

use common::*;
use llm_usage_core::adapters::framework::SourceAdapter;
use llm_usage_core::adapters::pi::PiAdapter;

// 手工核算值（jq 对 fixture 逐条核验，与 _expectations.md 互核）：
// 7 行记录（session/model_change/thinking_level_change/custom/message(system)/
// custom_message/message(assistant)），仅 L7 assistant 产 1 事件；
// usage 五字段全为直报 0（reported 零不是 unknown），totalTokens=0；
// 无 reasoning 字段（保持 unknown）；cost.total=0 ⇒ 不映射费用；
// stopReason=error ⇒ error_status="error"；无 responseId/duration/ttft。

fn setup_fixture(tag: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new(tag);
    let jsonl =
        reconstruct_jsonl_projection(&pi_fixture("session-error-zero-usage.sanitized.json"));
    let root = pi_root_with_file(
        &dir,
        "--C--Users-anon--/2026-09-24T16-37-28-439Z_anon-1.jsonl",
        &jsonl,
    );
    (dir, root)
}

#[test]
fn error_zero_usage_full_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("pi-contract");
    let (_db, storage) = temp_storage("pi-contract");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 7);
    assert_eq!(report.files[0].records_seen, 7);
    assert_eq!(report.files[0].events, 1, "exactly one assistant message");
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (1, 0, 0));

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    // 直报 0 是已知值，不是 unknown。
    assert_eq!(summary.totals.input_total_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(0));
    assert_eq!(summary.totals.cache_read_known, Some(0));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(0));
    assert_eq!(summary.totals.uncached_known, Some(0));

    // SQL 逐字段核验：分类/模型/供应商/版本/错误状态/未知字段。
    let row = storage
        .conn()
        .query_row(
            "SELECT source_record_key, call_category, model_raw, provider_id,
                    schema_version, error_status, output_reasoning,
                    cost_amount_minor, cost_currency, cost_kind,
                    session_id, parent_session_id, model_attribution
             FROM usage_events",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, String>(12)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        row.0, "pi:message:anon-7:anon-6:2026-09-24T16:38:24.776Z",
        "事件键 = 条目四元组"
    );
    assert_eq!(row.1, "primary");
    assert_eq!(row.2, "kimi-for-coding");
    assert_eq!(row.3, "kimi-coding");
    assert_eq!(row.4, "3");
    assert_eq!(row.5, "error");
    assert_eq!(row.6, None, "reasoning 未报告，保持 unknown");
    assert_eq!(row.7, None, "cost.total=0 与无价目不可区分，不映射");
    assert_eq!(row.8, None);
    assert_eq!(row.9, None);
    assert_eq!(row.10, "anon-1");
    assert_eq!(row.11, None, "主会话无 parentSession");
    assert_eq!(row.12, "request_field");

    // 无快照系列：pi 不产对账（无累计快照概念）。
    assert!(report.reconciliations.is_empty());
    let _ = dir;
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = PiAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "pi");
    assert_eq!(json["supported_versions"], serde_json::json!(["3"]));
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
    assert_eq!(json["fields"]["tokens"]["availability"], "available");
    assert!(
        json["fields"]["cost"]["availability"]
            .get("partial")
            .is_some(),
        "cost 为 Agent 价目估算：partial"
    );
    assert!(
        json["fields"]["latency"]["availability"]
            .get("unavailable")
            .is_some(),
        "pi 会话无逐次延迟字段"
    );
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
    let (_db, storage) = temp_storage("pi-cap");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "pi@test".to_string(),
            agent: "pi".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "pi-session-jsonl".to_string(),
            location_hint: None,
            parser_version: "pi-session-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        1_800_000_000_000,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'pi@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored, json, "capabilities 落库 roundtrip 不失真");
}
