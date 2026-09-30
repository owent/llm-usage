//! Claude Code 适配器合同测试：合成固定样本即合同样本（synthetic，本机
//! not_found 无真实样本；结构按 A01 文档口径，逐文件证据见各目录
//! _expectations.md）。经 读取→解析→标准化→commit_batch→查询 全链路。
//!
//! 手工核算值（fixture synthetic-contract，逐条核算并用 jq 验算，与
//! _expectations.md 互核）：
//! - syn-req-1：第 3/4/5 行同 requestId 同 usage（内容块拆分重报，仅时间戳
//!   不同）→ 去重后 1 调用；input 100 cache_read 40 cache_creation 10
//!   output 20 → input_total=150（derived）、total_tokens=170（derived）；
//! - syn-req-2：200/0/50/30 → input_total=250、total=280；
//! - syn-req-3：50/25/0/5 → input_total=75、total=80；
//! - 合计：call_count=3、input_total=475、cache_read=65、cache_write=60、
//!   output=55、uncached=350、total_tokens=530。

mod common;

use common::*;
use llm_usage_core::adapters::claude::ClaudeAdapter;
use llm_usage_core::adapters::framework::SourceAdapter;

#[test]
fn contract_fixed_sample_full_pipeline() {
    let (_db, storage) = temp_storage("claude-contract");
    let root = claude_fixture("synthetic-contract");
    let reports = run_claude(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 8);
    assert_eq!(report.files[0].records_seen, 8);
    assert_eq!(
        report.files[0].events, 5,
        "5 assistant entries (3 share syn-req-1)"
    );
    assert_eq!(report.files[0].diagnostics, 0);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (3, 0, 0));
    assert_eq!(
        outcome.unchanged, 2,
        "same requestId re-report (only timestamps differ) is idempotent"
    );
    assert_eq!(outcome.conflicts, 0);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.input_total_known, Some(475));
    assert_eq!(summary.totals.cache_read_known, Some(65));
    assert_eq!(summary.totals.cache_write_known, Some(60));
    assert_eq!(summary.totals.output_total_known, Some(55));
    assert_eq!(summary.totals.uncached_known, Some(350));
    assert_eq!(summary.totals.total_tokens_known, Some(530));
    assert_eq!(summary.periods.len(), 1);
    assert_eq!(summary.periods[0].distinct_sessions, Some(1));

    // 逐键 SQL 核验字段映射：input_uncached=input_tokens（reported）；
    // input_total=input+cache_read+cache_creation（derived）；total_tokens derived。
    let rows: Vec<(String, i64, i64, i64, i64, i64, i64)> = storage
        .conn()
        .prepare(
            "SELECT source_record_key, input_uncached, input_cache_read, input_cache_write,
                    input_total, output_total, total_tokens
             FROM usage_events ORDER BY source_record_key",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            ("req:syn-req-1".to_string(), 100, 40, 10, 150, 20, 170),
            ("req:syn-req-2".to_string(), 200, 0, 50, 250, 30, 280),
            ("req:syn-req-3".to_string(), 50, 25, 0, 75, 5, 80),
        ],
        "per-request mapping: uncached reported, input_total/total derived"
    );

    // 静态字段与 quality 逐键核验（以 syn-req-1 为代表）。
    #[allow(clippy::type_complexity)]
    type UsageEventRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        String,
    );
    let (
        provider,
        category,
        schema_version,
        parser_version,
        session,
        model,
        attribution,
        kind,
        lifecycle,
        time_basis,
        origin,
        parent,
        reasoning,
        source_total,
        error_status,
        cost_minor,
        quality,
    ): UsageEventRow = storage
        .conn()
        .query_row(
            "SELECT provider_id, call_category, schema_version, parser_version, session_id,
                    model_raw, model_attribution, record_kind, lifecycle, time_basis,
                    origin_call_id, parent_session_id, output_reasoning, source_total,
                    error_status, cost_amount_minor, quality_json
             FROM usage_events WHERE source_record_key = 'req:syn-req-1'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                    r.get(14)?,
                    r.get(15)?,
                    r.get(16)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(provider, "anthropic");
    assert_eq!(category, "primary");
    assert_eq!(schema_version, "transcript-doc-1");
    assert_eq!(parser_version, "claude-transcript-doc1");
    assert_eq!(session, "syn-sess-1");
    assert_eq!(model, "syn-claude-model-a");
    assert_eq!(attribution, "request_field");
    assert_eq!(kind, "model_call");
    assert_eq!(lifecycle, "final");
    assert_eq!(time_basis, "source_completion");
    assert_eq!(origin, "syn-req-1");
    assert_eq!(parent, None, "main transcript entry has no parent");
    assert_eq!(reasoning, None, "no reasoning evidence: unknown, not zero");
    assert_eq!(source_total, None, "transcript has no source total field");
    assert_eq!(error_status, None);
    assert_eq!(cost_minor, None, "no cost field locally");
    let quality: serde_json::Value = serde_json::from_str(&quality).unwrap();
    assert_eq!(quality["input_uncached"], "reported");
    assert_eq!(quality["input_cache_read"], "reported");
    assert_eq!(quality["input_cache_write"], "reported");
    assert_eq!(quality["output_total"], "reported");
    assert_eq!(quality["input_total"], "derived");
    assert_eq!(quality["total_tokens"], "derived");
    assert_eq!(quality["output_reasoning"], "unknown");
    assert_eq!(quality["source_total"], "unknown");
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = ClaudeAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "claude");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["transcript-doc-1"])
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
    assert!(
        json["fields"]["tokens"]["availability"]["partial"].is_string(),
        "tokens: 文档级证据 partial"
    );
    assert!(
        json["fields"]["latency"]["availability"]["unavailable"].is_string(),
        "latency: transcript 无逐次延迟字段"
    );
    assert_eq!(
        json["fields"]["per_request_calls"]["availability"],
        "available"
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
    let (_db, storage) = temp_storage("claude-cap");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "claude@test".to_string(),
            agent: "claude-code".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "claude-transcript-jsonl".to_string(),
            location_hint: None,
            parser_version: "claude-transcript-doc1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        1_800_000_000_000,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'claude@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(
        stored["supported_versions"],
        serde_json::json!(["transcript-doc-1"])
    );
}

/// 真实 2.1.197 样本（WSL 脱敏提取）：queue-operation 开头 + attachment/
/// last-prompt 元数据 + `<synthetic>` 占位 assistant——0 事件、
/// synthetic 诊断、探测放行；重复扫描不增量。
#[test]
fn contract_real_2_1_197_queue_metadata_and_synthetic() {
    let (_db, storage) = temp_storage("claude-real-217");
    let root = claude_fixture("real-2.1.197-queue-metadata");
    let reports = run_claude(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 8);
    assert_eq!(report.files[0].records_seen, 8);
    assert_eq!(
        report.files[0].events, 0,
        "synthetic placeholder is not a call"
    );
    // 空批次（0 事件）无提交结果；有也必须是零增量。
    assert_eq!(
        report
            .outcome
            .as_ref()
            .map(|o| (o.added, o.updated, o.errors))
            .unwrap_or((0, 0, 0)),
        (0, 0, 0)
    );
    // synthetic 跳过以诊断留痕（不含正文）。
    let diag: Vec<String> = storage
        .conn()
        .prepare("SELECT code FROM diagnostics ORDER BY code")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert!(diag.contains(&"synthetic_assistant_skipped".to_string()));

    // 重复扫描不增量。
    let reports2 = run_claude(&storage, &root, 1_800_000_001_000);
    assert_eq!(
        reports2[0]
            .outcome
            .as_ref()
            .map(|o| (o.added, o.updated, o.errors))
            .unwrap_or((0, 0, 0)),
        (0, 0, 0)
    );
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
}

/// queue-operation/attachment 携带 usage 字段 ⇒ 整文件 fail closed
/// （非用量载体带 usage 是格式偏离，不用静默跳过掩盖）。
#[test]
fn contract_real_2_1_197_queue_metadata_rejects_usage_carriers() {
    let (_db, storage) = temp_storage("claude-real-217-guard");
    let dir = TempDir::new("claude-real-217-guard");
    let root = claude_root_with_file(
        &dir,
        "proj/real-anon-sess-2.jsonl",
        b"{\"type\":\"queue-operation\",\"timestamp\":\"2026-09-30T10:41:53.368Z\",\"sessionId\":\"s\",\"usage\":{\"input_tokens\":1}}\n",
    );
    let reports = run_claude(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    // fail closed 合同：状态 pending（游标保持文件头等待受控重试），不产事件。
    assert_eq!(report.files[0].status, "pending");
    assert!(report.files[0]
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("usage_on_unexpected_record_type")
        || storage
            .conn()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM diagnostics WHERE code='usage_on_unexpected_record_type')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false));
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
}
