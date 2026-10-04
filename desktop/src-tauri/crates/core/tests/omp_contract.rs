//! oh-my-pi（omp）适配器约定测试：M2-B/C 恢复阶段真实脱敏 fixture（本机 omp 18.2.7，
//! glm-reasoning 主会话 / k3-cache-abort 主会话 / CommunityResearch 子 Agent）。
//! 经 读取→解析→标准化→commit_batch→查询，期望与各 _expectations.md
//! 的人工核算一致（jq 逐条验算；input_total 由 input+cacheRead+cacheWrite 派生）。

mod common;

use common::*;
use llm_usage_core::adapters::framework::SourceAdapter;
use llm_usage_core::adapters::omp::OmpAdapter;

/// glm 主会话：7 行仅 L6 assistant 产 1 事件；usage 17542/65/0/0/17607 +
/// reasoningTokens=54；duration 6058.1505→6058、ttft 4360.7924→4361；
/// cost.total=0 不映射；stopReason=stop 无 error_status。
#[test]
fn glm_reasoning_full_pipeline_matches_expectations() {
    let dir = TempDir::new("omp-contract-glm");
    let jsonl = reconstruct_jsonl_projection(&omp_fixture("session-glm-reasoning.sanitized.json"));
    let root = omp_root_with_file(
        &dir,
        "--C--Users-anon--/2026-09-24T16-39-54-647Z_00000000-0000-7000-8000-000000000009.jsonl",
        &jsonl,
    );
    let (_db, storage) = temp_storage("omp-contract-glm");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);

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
    assert_eq!(summary.totals.input_total_known, Some(17_542));
    assert_eq!(summary.totals.uncached_known, Some(17_542));
    assert_eq!(summary.totals.output_total_known, Some(65));
    assert_eq!(summary.totals.cache_read_known, Some(0));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(17_607));

    // SQL 逐字段核验：分类/模型/供应商/版本/延迟/推理/费用/会话身份。
    let row = storage
        .conn()
        .query_row(
            "SELECT source_record_key, call_category, model_raw, provider_id,
                    schema_version, error_status, output_reasoning,
                    cost_amount_minor, cost_currency, cost_kind,
                    session_id, parent_session_id, model_attribution,
                    origin_call_id, duration_ms, ttft_ms
             FROM usage_events",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, String>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, Option<i64>>(14)?,
                    r.get::<_, Option<i64>>(15)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        row.0, "omp:message:anon-5:anon-4:2026-09-24T16:40:03.379Z",
        "事件键 = 条目四元组"
    );
    assert_eq!(row.1, "primary");
    assert_eq!(row.2, "glm-5.3-flash");
    assert_eq!(row.3, "zhipu-coding-plan");
    assert_eq!(row.4, "3");
    assert_eq!(row.5, None, "stopReason=stop 无 error_status");
    assert_eq!(row.6, Some(54), "reasoningTokens⊆output 直报");
    assert_eq!(row.7, None, "cost.total=0 与无价目不可区分，不映射");
    assert_eq!(row.8, None);
    assert_eq!(row.9, None);
    assert_eq!(row.10, "anon-1");
    assert_eq!(row.11, None, "主会话无 parentSession");
    assert_eq!(row.12, "request_field");
    assert_eq!(
        row.13.as_deref(),
        Some("anon-6"),
        "responseId 作 origin_call_id"
    );
    assert_eq!(row.14, Some(6058), "duration 浮点毫秒四舍五入");
    assert_eq!(row.15, Some(4361), "ttft 浮点毫秒四舍五入");

    // 无快照系列：omp 不产对账（无累计快照概念）。
    assert!(report.reconciliations.is_empty());
    let _ = dir;
}

/// k3 主会话：31 行 7 事件全 primary（6 toolUse + 1 aborted）；aborted 条目
/// 无 duration/ttft。汇总 input_total=202560（派生 21312+181248+0）。
#[test]
fn k3_cache_abort_full_pipeline_matches_expectations() {
    let dir = TempDir::new("omp-contract-k3");
    let jsonl = reconstruct_jsonl_projection(&omp_fixture("session-k3-cache-abort.sanitized.json"));
    let root = omp_root_with_file(
        &dir,
        "--D--workspace-anon--/2026-09-15T17-33-37-534Z_00000000-0000-7000-8000-000000000010.jsonl",
        &jsonl,
    );
    let (_db, storage) = temp_storage("omp-contract-k3");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 31);
    assert_eq!(report.files[0].records_seen, 31);
    assert_eq!(report.files[0].events, 7, "7 assistant messages with usage");
    assert_eq!(report.outcome.as_ref().unwrap().added, 7);

    let summary = summary(&storage, "2026-09-15", "2026-09-15");
    assert_eq!(summary.totals.call_count, 7);
    assert_eq!(
        summary.totals.input_total_known,
        Some(202_560),
        "派生口径 21312+181248+0"
    );
    assert_eq!(summary.totals.uncached_known, Some(21_312));
    assert_eq!(summary.totals.cache_read_known, Some(181_248));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(4_828));
    assert_eq!(summary.totals.total_tokens_known, Some(207_388));

    // 类别/模型/供应商：全部 primary、k3-256k/kimi-code。
    let mut stmt = storage
        .conn()
        .prepare("SELECT call_category, COUNT(*) FROM usage_events GROUP BY call_category")
        .unwrap();
    let categories: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(categories, vec![("primary".to_string(), 7)]);
    let models: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE model_raw = 'k3-256k' AND provider_id = 'kimi-code'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(models, 7);

    // aborted 条目：error_status=aborted，output=0 是报告值，无延迟字段。
    let aborted = storage
        .conn()
        .query_row(
            "SELECT error_status, output_total, duration_ms, ttft_ms FROM usage_events
             WHERE source_record_key = 'omp:message:anon-36:anon-35:2026-09-15T17:37:42.581Z'",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(aborted.0, "aborted");
    assert_eq!(aborted.1, 0, "aborted 直报 output=0，不是 unknown");
    assert_eq!(aborted.2, None);
    assert_eq!(aborted.3, None);

    // 首条 toolUse 延迟取整：duration 10312.2929→10312、ttft 2587.1515→2587。
    let first = storage
        .conn()
        .query_row(
            "SELECT duration_ms, ttft_ms, input_total FROM usage_events
             WHERE source_record_key = 'omp:message:anon-5:anon-4:2026-09-15T17:34:05.160Z'",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(first, (10312, 2587, 17_548));
    let _ = dir;
}

/// 子 Agent 文件（真实 CommunityResearch，41 行 5 事件）：路径形状推定父会话；
/// 自有 session 头无 parentSession；全 sub_agent；duration/ttft 四舍五入。
#[test]
fn subagent_community_research_full_pipeline_matches_expectations() {
    let dir = TempDir::new("omp-contract-sub");
    let jsonl =
        reconstruct_jsonl_projection(&omp_fixture("subagent-community-research.sanitized.json"));
    let parent_uuid = "00000000-0000-7000-8000-000000000001";
    let root = omp_root_with_file(
        &dir,
        "--D--anonymized--/2026-08-21T02-19-22-638Z_00000000-0000-7000-8000-000000000001/CommunityResearch.jsonl",
        &jsonl,
    );
    let (_db, storage) = temp_storage("omp-contract-sub");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 41);
    assert_eq!(report.files[0].records_seen, 41);
    assert_eq!(report.files[0].events, 5);
    assert_eq!(report.outcome.as_ref().unwrap().added, 5);

    let summary = summary(&storage, "2026-08-21", "2026-08-21");
    assert_eq!(summary.totals.call_count, 5);
    assert_eq!(
        summary.totals.input_total_known,
        Some(78_310),
        "派生口径 20710+57600+0"
    );
    assert_eq!(summary.totals.uncached_known, Some(20_710));
    assert_eq!(summary.totals.cache_read_known, Some(57_600));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(6_135));
    assert_eq!(summary.totals.total_tokens_known, Some(84_445));

    // 全部 sub_agent；parent 来自目录名（首个下划线后部分）；自有 session 头 anon-1。
    let (category, session, parent): (String, String, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, session_id, parent_session_id FROM usage_events
             WHERE source_record_key = 'omp:message:anon-6:anon-5:2026-08-21T02:35:32.158Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(session, "anon-1", "子 Agent 自有会话头");
    assert_eq!(parent, parent_uuid, "父子关联来自 <ts>_<uuid> 目录名");
    let all_sub: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE call_category = 'sub_agent' AND parent_session_id = ?1",
            [parent_uuid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(all_sub, 5);

    // 逐条延迟取整（四舍五入）：17309.34289999993→17309、6857.021999999997→6857；
    // 14649.890800000052→14650、6983.533100000001→6984。
    let latency = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT duration_ms, ttft_ms FROM usage_events WHERE source_record_key = ?1",
                [key],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )
            .unwrap()
    };
    assert_eq!(
        latency("omp:message:anon-6:anon-5:2026-08-21T02:35:32.158Z"),
        (17_309, 6_857)
    );
    assert_eq!(
        latency("omp:message:anon-17:anon-16:2026-08-21T02:35:51.596Z"),
        (14_650, 6_984)
    );
    assert_eq!(
        latency("omp:message:anon-27:anon-26:2026-08-21T02:36:05.918Z"),
        (8_387, 1_650)
    );
    assert_eq!(
        latency("omp:message:anon-35:anon-34:2026-08-21T02:36:26.976Z"),
        (14_961, 7_236)
    );
    assert_eq!(
        latency("omp:message:anon-41:anon-40:2026-08-21T02:38:35.737Z"),
        (126_737, 9_318)
    );
    let _ = dir;
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = OmpAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "omp");
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
    assert_eq!(
        json["fields"]["latency"]["availability"], "available",
        "omp assistant 自带 duration/ttft 浮点毫秒"
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
    let (_db, storage) = temp_storage("omp-cap");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "omp@test".to_string(),
            agent: "oh-my-pi".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "omp-session-jsonl".to_string(),
            location_hint: None,
            parser_version: "omp-session-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        1_800_000_000_000,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'omp@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored, json, "capabilities 落库 roundtrip 不失真");
}
