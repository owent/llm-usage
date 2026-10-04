//! ZCode 适配器约定测试：真实脱敏 fixture（本机 ZCode 3.14.3 只读提取，
//! 2026-09-25，tests/fixtures/zcode/real-main-session 与 real-subagent）
//! 经 读取→探测→解析→标准化→commit_batch→查询，对 _expectations.md
//! 的人工核算值逐项断言。
//!
//! 两个 usage 视图的处理规则（同记录两个 usage 视图，互斥不混算）：
//! - AI SDK `response.usage`（camelCase 五键，**主视图**）：inputTokens 含缓存读；
//! - anthropic `response.providerMetadata.anthropic.usage`（snake_case，**对照视图**）：
//!   input_tokens 不含缓存；一致性校验 in+cr+cw == inputTokens、out == outputTokens。
//!
//! 手工核算（real-main-session，4 条全 main_turn）：
//! in 391115+391297+392343+392729=1,567,484；out 120+595+345+114=1,174；
//! cr 390976+391104+391296+392320=1,565,696；cw 全 0；
//! total 391235+391892+392688+392843=1,568,658；uncached 和 1788。
//! 手工核算（real-subagent，4 条全 subagent）：
//! in=596,365；out=4,594；cr=587,328；total=600,959；uncached 和 9037。
//! 合并：8 事件；in=2,163,849；out=5,768；cr=2,153,024；cw=Some(0)；
//! total=2,169,617；uncached 合计 10,825（=2,163,849−2,153,024−0）；
//! anthropic 侧 in_tokens 合计 10,825 + cr 2,153,024 = 2,163,849 与 AI SDK 互斥一致。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::zcode::ZcodeAdapter;

const NOW: i64 = 1_800_000_000_000;

fn main_jsonl() -> Vec<u8> {
    reconstruct_jsonl_projection(&zcode_fixture("real-main-session").join("sanitized.json"))
}

fn subagent_jsonl() -> Vec<u8> {
    reconstruct_jsonl_projection(&zcode_fixture("real-subagent").join("sanitized.json"))
}

#[test]
fn contract_main_session_matches_expectations() {
    let (_db, storage) = temp_storage("zcode-contract-main");
    let dir = TempDir::new("zcode-contract-main-src");
    let root = zcode_root_with_file(&dir, "model-io-sess_anon-1.jsonl", &main_jsonl());
    let reports = run_zcode(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 4);
    assert_eq!(report.files[0].lines_read, 4);
    assert_eq!(report.files[0].events, 4);
    assert_eq!(report.files[0].diagnostics, 0, "双口径逐条一致，无诊断");
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (4, 0, 0));

    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 4);
    assert_eq!(summary.totals.input_total_known, Some(1_567_484));
    assert_eq!(summary.totals.output_total_known, Some(1_174));
    assert_eq!(summary.totals.cache_read_known, Some(1_565_696));
    assert_eq!(
        summary.totals.cache_write_known,
        Some(0),
        "cacheWriteTokens 全 0 直报（reported），不是未知"
    );
    assert_eq!(summary.totals.total_tokens_known, Some(1_568_658));

    let conn = storage.conn();
    // 首条事件（line 1，requestId anon-2，attempt 1）逐字段核对。
    type RowRow = (
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<i64>,
        i64,
        Option<i64>,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    );
    let row: RowRow = conn
        .query_row(
            "SELECT input_total, input_cache_read, output_total, total_tokens, \
             input_uncached, output_reasoning, source_total, duration_ms, \
             session_id, model_raw, schema_version, provider_id, origin_call_id, \
             attempt_id, occurred_at_ms \
             FROM usage_events WHERE source_record_key = 'zcode:anon-2:1'",
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
                ))
            },
        )
        .unwrap();
    assert_eq!(
        row.0, 391_115,
        "input_total = inputTokens reported（含缓存读）"
    );
    assert_eq!(row.1, 390_976);
    assert_eq!(row.2, 120);
    assert_eq!(row.3, 391_235, "total_tokens = in+out（derived）");
    assert_eq!(row.4, 139, "input_uncached = 391115-(390976+0)（derived）");
    assert_eq!(row.5, None, "reasoning 未知不补零");
    assert_eq!(row.6, 391_235, "source_total = totalTokens reported");
    assert_eq!(row.7, Some(4132), "durationMs 直报");
    assert_eq!(row.8, "sess_anon-1");
    assert_eq!(row.9, "GLM-5.3");
    assert_eq!(row.10, "3.14.3", "schema_version = x-zcode-app-version");
    assert_eq!(
        row.11.as_deref(),
        Some("account:bigmodel-individual-coding-plan")
    );
    assert_eq!(
        row.12.as_deref(),
        Some("anon-2"),
        "origin_call_id = requestId"
    );
    assert_eq!(row.13.as_deref(), Some("1"));
    assert_eq!(
        row.14,
        ts("2026-09-25T09:10:45.285Z"),
        "occurred_at = completedAt"
    );

    // 身份四键 = zcode:{requestId}:{attempt}；全部 main_turn ⇒ primary。
    let keys: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT source_record_key, call_category FROM usage_events ORDER BY occurred_at_ms",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        keys,
        vec![
            ("zcode:anon-2:1".to_string(), "primary".to_string()),
            ("zcode:anon-6:1".to_string(), "primary".to_string()),
            ("zcode:anon-8:1".to_string(), "primary".to_string()),
            ("zcode:anon-10:1".to_string(), "primary".to_string()),
        ]
    );

    // 解析依据：3.14.3 已收录 ⇒ known_version；时间基准 source_completion。
    let (basis, time_basis, agent, parse_basis): (Option<String>, String, String, Option<String>) =
        conn.query_row(
            "SELECT schema_version, time_basis, agent, parse_basis FROM usage_events LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(basis.as_deref(), Some("3.14.3"));
    assert_eq!(time_basis, "source_completion");
    assert_eq!(agent, "zcode");
    assert_eq!(parse_basis.as_deref(), Some("known_version"));

    // 无任何诊断。
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
}

#[test]
fn contract_subagent_matches_expectations() {
    let (_db, storage) = temp_storage("zcode-contract-sub");
    let dir = TempDir::new("zcode-contract-sub-src");
    let root = zcode_root_with_file(
        &dir,
        "model-io-sess_subagent_agent_anon-19.jsonl",
        &subagent_jsonl(),
    );
    let reports = run_zcode(&storage, &root, NOW);

    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 4);
    assert_eq!(reports[0].files[0].diagnostics, 0);

    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 4);
    assert_eq!(summary.totals.input_total_known, Some(596_365));
    assert_eq!(summary.totals.output_total_known, Some(4_594));
    assert_eq!(summary.totals.cache_read_known, Some(587_328));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(600_959));

    // querySource=subagent ⇒ sub_agent；首条 input_uncached=145225-(142656+0)=2569。
    let (category, uncached, session): (String, i64, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, input_uncached, session_id \
             FROM usage_events WHERE source_record_key = 'zcode:anon-20:1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(uncached, 2_569);
    assert_eq!(session, "sess_subagent_agent_anon-19");
}

#[test]
fn contract_merged_roots_match_combined_expectations() {
    // 两文件放同一 rollout 根：8 事件、4 primary + 4 sub_agent、合并合计。
    let (_db, storage) = temp_storage("zcode-contract-merged");
    let dir = TempDir::new("zcode-contract-merged-src");
    let root = zcode_root_with_file(&dir, "model-io-sess_anon-1.jsonl", &main_jsonl());
    std::fs::write(
        dir.path()
            .join("rollout/model-io-sess_subagent_agent_anon-19.jsonl"),
        subagent_jsonl(),
    )
    .unwrap();
    let reports = run_zcode(&storage, &root, NOW);

    assert_eq!(reports[0].files.len(), 2);
    assert_eq!(reports[0].outcome.as_ref().unwrap().added, 8);

    let summary = summary(&storage, "2026-09-25", "2026-09-25");
    assert_eq!(summary.totals.call_count, 8);
    assert_eq!(summary.totals.input_total_known, Some(2_163_849));
    assert_eq!(summary.totals.output_total_known, Some(5_768));
    assert_eq!(summary.totals.cache_read_known, Some(2_153_024));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(2_169_617));

    let conn = storage.conn();
    let (primary, sub): (i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'sub_agent') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((primary, sub), (4, 4));

    // input_uncached 合计 10,825：与 anthropic 侧互斥关系一致（不相加、不双计）。
    let uncached_sum: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(input_uncached), 0) FROM usage_events",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(uncached_sum, 10_825);

    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0, "双口径逐条一致，合并后仍无诊断");
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = ZcodeAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "zcode");
    assert_eq!(json["supported_versions"], serde_json::json!(["3.14.3"]));
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
        json["fields"]["cache_write"]["availability"]
            .get("partial")
            .is_some(),
        "真实样本 cache_write 全 0，标注 partial"
    );
    assert_eq!(json["fields"]["latency"]["availability"], "available");
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
    assert_eq!(
        json["detection"]["version_field"],
        "request.headers[\"x-zcode-app-version\"]"
    );
    assert!(
        json["dedup"]["primary"]
            .as_str()
            .unwrap()
            .starts_with("zcodedb:{model_usage.id}"),
        "数据库使用原生调用身份"
    );
    assert!(
        json["dedup"]["primary"]
            .as_str()
            .unwrap()
            .contains("zcode:{requestId}:{attempt}"),
        "保留无数据库来源的 JSONL 身份合同"
    );
    assert!(!cap.limitations.is_empty());
    // 能力声明可落库（source_instances.capabilities）roundtrip。
    let (_db, storage) = temp_storage("zcode-cap");
    framework::upsert_source_instance(
        &storage,
        &framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "zcode@test".to_string(),
            agent: "zcode".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "zcode-modelio-jsonl".to_string(),
            location_hint: None,
            parser_version: "zcode-modelio-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        NOW,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'zcode@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored["adapter_id"], "zcode");
}
