//! Qwen Code 适配器合同测试：合成固定样本（本机 not_found，全部 fixture 合成，
//! 数值为人工核算，见 tests/fixtures/qwen/synthetic-contract/_expectations.md 与
//! 本文件头部注释）经 读取→解析→标准化→commit_batch→查询 全链路。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::qwen::QwenAdapter;

// 手工核算值（对照 fixture _expectations.md）：
// syn-a-1（primary）{prompt 1000, candidates 50, cached 400, thoughts 10, toolUse 5, total 1050}；
// syn-a-2（isSidechain ⇒ sub_agent）{prompt 2000, candidates 100, cached 0, total 2100}；
// syn-a-3（agentId ⇒ sub_agent）{prompt 500, candidates 25, total 525}。
// 汇总：call_count=3；input_total=1000+2000+500=3500；output_total=50+100+25=175；
// cache_read=400+0=400（a3 未直报，未知不补零）；total_tokens=1050+2100+525=3675。
// thoughts/toolUsePrompt 不并入任何字段；goal_state 的 tokensUsed=4,000,000,000 是
// 跨 turn 累计表，明确忽略；chat_compression/session_model 不产事件。

const NOW: i64 = 1_800_000_000_000;

#[test]
fn contract_full_pipeline_matches_expectations() {
    let (_db, storage) = temp_storage("qwen-contract");
    let root = qwen_fixture("synthetic-contract");
    let reports = run_qwen(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 8);
    assert_eq!(report.files[0].lines_read, 8);
    assert_eq!(report.files[0].events, 3);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (3, 0, 0));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.input_total_known, Some(3_500));
    assert_eq!(summary.totals.output_total_known, Some(175));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(
        summary.totals.cache_write_known, None,
        "格式内无缓存创建字段，未知不补零"
    );
    assert_eq!(summary.totals.total_tokens_known, Some(3_675));

    let conn = storage.conn();
    // 分类：1 primary + 2 sub_agent（isSidechain / agentId）。
    let (primary, sub): (i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'sub_agent') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((primary, sub), (1, 2));

    // 事件键 = qwen:{uuid}；schema_version = record.version 逐条透传；provider 无字段。
    let row1: (
        String,
        i64,
        i64,
        i64,
        i64,
        Option<String>,
        String,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT source_record_key, input_total, input_cache_read, output_total, \
             total_tokens, provider_id, session_id, model_raw, schema_version \
             FROM usage_events WHERE source_record_key = 'qwen:syn-a-1'",
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
                ))
            },
        )
        .unwrap();
    assert_eq!(row1.0, "qwen:syn-a-1");
    assert_eq!(row1.1, 1000, "input_total = promptTokenCount reported");
    assert_eq!(row1.2, 400, "cachedContentTokenCount → input_cache_read");
    assert_eq!(row1.3, 50);
    assert_eq!(row1.4, 1050, "total_tokens 只取直报 totalTokenCount");
    assert_eq!(row1.5, None, "ChatRecord 无 provider 字段");
    assert_eq!(row1.6, "syn-sess-1");
    assert_eq!(row1.7, "qwen3-coder-plus");
    assert_eq!(row1.8, "0.5.0", "schema_version = record.version");

    // thoughts=10 / toolUsePrompt=5 不并入任何桶：reasoning/uncached/source_total 全 NULL。
    let nulls: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT output_reasoning, input_uncached, source_total, input_cache_write \
             FROM usage_events WHERE source_record_key = 'qwen:syn-a-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(nulls, (None, None, None, None));

    // goal_state 的 4,000,000,000 累计表与压缩/控制记录不进任何汇总：
    // 上面 total_tokens_known=3675 已含此断言；再核验全表 token 合计无巨额值。
    let max_total: Option<i64> = conn
        .query_row("SELECT MAX(total_tokens) FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(max_total, Some(2_100));

    // sub_agent 两条的分类与身份。
    let categories: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT source_record_key, call_category FROM usage_events \
                 ORDER BY source_record_key",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        categories,
        vec![
            ("qwen:syn-a-1".to_string(), "primary".to_string()),
            ("qwen:syn-a-2".to_string(), "sub_agent".to_string()),
            ("qwen:syn-a-3".to_string(), "sub_agent".to_string()),
        ]
    );

    // 无诊断。
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = QwenAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "qwen");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["chatrecord-085e98c0"])
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
    assert_eq!(json["fields"]["cache_read"]["availability"], "available");
    assert!(
        json["fields"]["latency"]["availability"]
            .get("unavailable")
            .is_some(),
        "ChatRecord 无逐次延迟字段"
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
    // 能力声明可落库（source_instances.capabilities）roundtrip。
    let (_db, storage) = temp_storage("qwen-cap");
    framework::upsert_source_instance(
        &storage,
        &framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "qwen@test".to_string(),
            agent: "qwen-code".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "qwen-chatrecord-jsonl".to_string(),
            location_hint: None,
            parser_version: "qwen-chatrecord-085e98c0-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        NOW,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'qwen@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored["adapter_id"], "qwen");
}
