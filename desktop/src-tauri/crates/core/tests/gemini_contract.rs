//! Gemini CLI 适配器合同测试：合成固定样本（本机 not_found，全部 fixture 合成，
//! 数值为人工核算，见各 fixture 目录 _expectations.md 与本文件头部注释）经
//! 读取→解析→标准化→commit_batch→查询 全链路。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::gemini::GeminiAdapter;

// 手工核算值（对照 tests/fixtures/gemini/synthetic-contract/_expectations.md）：
// syn-msg-1 tokens {input 1000, output 50, cached 400, thoughts 10, tool 5, total 1050}；
// syn-msg-2 tokens {input 2000, output 100, total 2100}；syn-msg-3 无 tokens 不产事件。
// 汇总：call_count=2；input_total=1000+2000=3000；output_total=50+100=150；
// cache_read=400（仅 msg-1 直报）；total_tokens=1050+2100=3150（只取直报 total）。
// thoughts/tool 不并入任何字段；cache_write 格式内无字段 → known=None（未知不补零）。

const NOW: i64 = 1_800_000_000_000;

#[test]
fn contract_full_pipeline_matches_expectations() {
    let (_db, storage) = temp_storage("gemini-contract");
    let root = gemini_fixture("synthetic-contract");
    let reports = run_gemini(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 4);
    assert_eq!(
        report.files[0].events, 2,
        "只有带 tokens 的 gemini 消息产事件"
    );
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (2, 0, 0));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(3_000));
    assert_eq!(summary.totals.output_total_known, Some(150));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(
        summary.totals.cache_write_known, None,
        "格式内无缓存创建字段，未知不补零"
    );
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));

    // 逐事件 SQL 核验：字段映射 + thoughts/tool 不并入任何桶。
    let conn = storage.conn();
    let row1: (String, i64, i64, i64, i64, String, String, String, String) = conn
        .query_row(
            "SELECT source_record_key, input_total, input_cache_read, output_total, \
             total_tokens, provider_id, session_id, model_raw, schema_version \
             FROM usage_events WHERE source_record_key = 'gemini:syn-sess-g1:syn-msg-1'",
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
    assert_eq!(row1.0, "gemini:syn-sess-g1:syn-msg-1");
    assert_eq!(row1.1, 1000, "input_total = tokens.input reported");
    assert_eq!(row1.2, 400, "cached → input_cache_read");
    assert_eq!(row1.3, 50);
    assert_eq!(row1.4, 1050, "total_tokens 只取直报 tokens.total");
    assert_eq!(row1.5, "google");
    assert_eq!(row1.6, "syn-sess-g1");
    assert_eq!(row1.7, "gemini-3.0-flash");
    assert_eq!(row1.8, "session-doc-1");

    // thoughts=10 / tool=5 不并入任何字段：reasoning/uncached/source_total 均为 NULL。
    let nulls: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT output_reasoning, input_uncached, source_total, input_cache_write \
             FROM usage_events WHERE source_record_key = 'gemini:syn-sess-g1:syn-msg-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(nulls, (None, None, None, None));

    // syn-msg-2：cached 缺省 → input_cache_read 未知（不补零）。
    let (total2, cached2): (i64, Option<i64>) = conn
        .query_row(
            "SELECT total_tokens, input_cache_read FROM usage_events \
             WHERE source_record_key = 'gemini:syn-sess-g1:syn-msg-2'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(total2, 2100);
    assert_eq!(cached2, None);

    // 事件属性：primary/final/model_call；origin_call_id = message.id。
    let (category, kind, origin): (String, String, Option<String>) = conn
        .query_row(
            "SELECT call_category, record_kind, origin_call_id FROM usage_events \
             WHERE source_record_key = 'gemini:syn-sess-g1:syn-msg-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(category, "primary");
    assert_eq!(kind, "model_call");
    assert_eq!(origin.as_deref(), Some("syn-msg-1"));

    // 质量位：直报字段 reported。
    let quality: String = conn
        .query_row(
            "SELECT quality_json FROM usage_events \
             WHERE source_record_key = 'gemini:syn-sess-g1:syn-msg-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let quality: serde_json::Value = serde_json::from_str(&quality).unwrap();
    assert_eq!(quality["input_total"], "reported");
    assert_eq!(quality["input_cache_read"], "reported");
    assert_eq!(quality["total_tokens"], "reported");
    assert_eq!(quality["output_reasoning"], "unknown");

    // 无诊断。
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = GeminiAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "gemini");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["session-doc-1"])
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
    assert!(
        json["fields"]["latency"]["availability"]
            .get("unavailable")
            .is_some(),
        "会话 JSON 无逐次延迟字段"
    );
    assert!(
        json["fields"]["tokens"]["availability"]
            .get("partial")
            .is_some(),
        "tokens 为文档级证据 partial"
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
    let (_db, storage) = temp_storage("gemini-cap");
    framework::upsert_source_instance(
        &storage,
        &framework::SourceInstanceInput {
            instance_id: "gemini@test".to_string(),
            agent: "gemini-cli".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "gemini-session-json".to_string(),
            location_hint: None,
            parser_version: "gemini-session-doc1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        NOW,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'gemini@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored["adapter_id"], "gemini");
}

#[test]
fn discover_enumerates_tmp_chats_layout() {
    // 发现语义：manual root 下 tmp/<project_hash>/chats/*.json 被枚举。
    let adapter = GeminiAdapter::new();
    let root = gemini_fixture("synthetic-contract");
    let ctx = framework::DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.clone()],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].files.len(), 1);
    assert!(
        roots[0].files[0].ends_with("session-2026-01-05T10-00-syn1.json"),
        "发现命中 chats 下的会话 JSON"
    );
    // 缺 tmp 目录的 root 不产生实例。
    let empty = TempDir::new("gemini-nodir");
    let ctx = framework::DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![empty.path().to_path_buf()],
    };
    assert!(adapter.discover(&ctx).is_empty());
}
