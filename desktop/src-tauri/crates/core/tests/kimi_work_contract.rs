//! Kimi Work（A13，M4）约定测试：真实脱敏 fixture（conv-main + agent-44-subagent，
//! 本机内嵌 kimi-code home / wire protocol_version=1.4，2026-09-25 提取）经
//! 读取→解析→标准化→commit_batch→查询。期望值为人工核算，
//! 见 tests/fixtures/kimi-work/*.sanitized.json 同名 _expectations.md。
//! 与 Kimi Code（1.5）的实读差异在断言中逐项固定：目录布局 conv-*、
//! usage.record 无 agentId、model 为裸 id、注册表锚点 1.4。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::kimi_work::KimiWorkAdapter;

// 手工核算（对照 _expectations.md；roundtrip jq 独立复核相等）：
// conv-main：38 turn（primary）；io 47,574 / cr 1,310,720 / out 18,658。
// agent-44：7 turn（sub_agent）+ 1 session（auxiliary）；
//   io 204,485+29,663 / cr 374,784+183,040 / out 23,343+12,440。
// 合计 46 事件：input_uncached 281,722；input_total 2,150,266；total 2,204,707。

const NOW: i64 = 1_800_000_000_000;

#[test]
fn contract_full_pipeline_matches_expectations() {
    let (_db, storage) = temp_storage("kimi-work-contract");
    // 两个真实会话（不同 wd/conv 目录，同实例根）。
    let conv_wire = reconstruct_kimi_wire(&kimi_work_fixture("conv-main.sanitized.json"));
    let sub_wire = reconstruct_kimi_wire(&kimi_work_fixture("agent-44-subagent.sanitized.json"));
    let dir = TempDir::new("kimi-work-real");
    let root = kimi_root_with_file(
        &dir,
        "wd_a/conv_syn-real/agents/main/wire.jsonl",
        &conv_wire,
    );
    let sub_path = dir
        .path()
        .join("sessions")
        .join("wd_b")
        .join("conv_syn-sub")
        .join("agents")
        .join("agent-44")
        .join("wire.jsonl");
    std::fs::create_dir_all(sub_path.parent().unwrap()).unwrap();
    std::fs::write(sub_path, &sub_wire).unwrap();

    let reports = run_kimi_work(&storage, &root, NOW);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].files.len(), 2);
    assert!(reports[0].files.iter().all(|f| f.status == "complete"));
    assert_eq!(reports[0].files[0].events + reports[0].files[1].events, 46);

    let outcome = reports[0].outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (46, 0, 0));

    // 全事件白名单核对（跨两日区间覆盖 2026-07-18 与 2026-09-13）。
    let summary = summary(&storage, "2026-07-18", "2026-09-13");
    assert_eq!(summary.totals.call_count, 46);
    assert_eq!(summary.totals.uncached_known, Some(281_722));
    assert_eq!(summary.totals.input_total_known, Some(2_150_266));
    assert_eq!(summary.totals.cache_read_known, Some(1_868_544));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(54_441));
    assert_eq!(summary.totals.total_tokens_known, Some(2_204_707));

    let conn = storage.conn();
    // 分类：38 primary + 1 auxiliary（agent-44 压缩摘要）+ 7 sub_agent。
    let (primary, auxiliary, sub_agent): (i64, i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'auxiliary'), \
             SUM(call_category = 'sub_agent') FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((primary, auxiliary, sub_agent), (38, 1, 7));

    // Agent 分列：kimi-work（不与 kimi-code 混列，A12/A13）。
    let agents: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare("SELECT agent, COUNT(*) FROM usage_events GROUP BY agent")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(agents, vec![("kimi-work".to_string(), 46)]);

    // 事件键命名空间与 schema：1.4 已验证锚点。
    let (key, schema, basis): (String, String, String) = conn
        .query_row(
            "SELECT source_record_key, schema_version, parse_basis FROM usage_events \
             WHERE source_record_key = 'kimi-work:usage:conv_syn-real:main:1789304762761:0'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(key, "kimi-work:usage:conv_syn-real:main:1789304762761:0");
    assert_eq!(schema, "1.4");
    assert_eq!(basis, "known_version");

    // 模型白名单：1.4 裸 id 原样入账。
    let models: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT model_raw, COUNT(*) FROM usage_events GROUP BY model_raw ORDER BY model_raw",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        models,
        vec![
            ("k28-agent-preview".to_string(), 38),
            ("k3-agent".to_string(), 8),
        ]
    );

    // 会话身份：conv 目录名（1.4 无 agentId 字段，身份来自路径）。
    let sub_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE session_id = 'conv_syn-sub'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sub_rows, 8, "agent-44 的 8 条（7 turn + 1 session）");

    // 诊断：真实 fixture 无坏形状 ⇒ 0。
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);

    // 回声对账（记录侧只算 turn scope；session scope 无回声）：两文件均 matched——
    // conv-main 38==38；agent-44 7 turn==7 回声（1 条 session scope 不参与）。
    let matched = reports[0]
        .reconciliations
        .iter()
        .filter(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "matched")
        .count();
    assert_eq!(matched, 2, "两文件的回声对账均 matched");

    // 幂等：二次扫描无新增。
    let reports2 = run_kimi_work(&storage, &root, NOW + 1000);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0, "重复扫描不增量（V12）");
}

/// 两产品实例身份天然分离：同一目录树分别经 kimi-code/kimi-work 手工根扫描
/// 产生不同实例与 Agent 分列（防 A12/A13 合并计账）。
#[test]
fn instance_identity_separates_from_kimi_code() {
    let dir = TempDir::new("kimi-work-ident");
    let wire = concat!(
        r#"{"type":"metadata","protocol_version":"1.4","created_at":1767225600000}"#,
        "\n",
        r#"{"type":"usage.record","model":"k3-agent","usage":{"inputOther":10,"output":5,"inputCacheRead":0,"inputCacheCreation":0},"usageScope":"turn","time":1767225601000}"#,
        "\n",
    );
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/conv_syn-1/agents/main/wire.jsonl",
        wire.as_bytes(),
    );
    let (_db, storage) = temp_storage("kimi-work-ident");

    let adapter = KimiWorkAdapter::new();
    let ctx = llm_usage_core::adapters::framework::DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.clone()],
    };
    let reports = framework::run_adapter_scan(
        &storage,
        &adapter,
        &ctx,
        &framework::RunConfig {
            timezone: "UTC".to_string(),
            now_ms: NOW,
            limits: Default::default(),
            trigger: llm_usage_core::jobs::TriggerKind::Manual,
            origin_host_id: None,
            run_id_prefix: "ident-work".to_string(),
        },
    )
    .unwrap();
    assert_eq!(reports.len(), 1);
    assert!(
        reports[0].instance_id.starts_with("kimi-work@"),
        "实例前缀独立"
    );

    let (agent, key): (String, String) = storage
        .conn()
        .query_row(
            "SELECT agent, source_record_key FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(agent, "kimi-work");
    assert!(key.starts_with("kimi-work:usage:"));
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = KimiWorkAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "kimi-work");
    assert_eq!(json["supported_versions"], serde_json::json!(["1.4"]));
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
        assert!(json["fields"].get(key).is_some(), "missing {key}");
    }
    assert_eq!(json["fields"]["tokens"]["availability"], "available");
    // 发现声明如实记录：候选为本机实测推导 + manual，无 env 覆盖。
    assert_eq!(json["discovery"]["env_override"], serde_json::Value::Null);
    let basis: &str = json["discovery"]["default_root_basis"].as_str().unwrap();
    assert!(
        basis.contains("非官方文档默认"),
        "default_root_basis={basis}"
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
        assert!(json.get(section).is_some(), "missing {section}");
    }
    assert!(!cap.limitations.is_empty());

    let (_db, storage) = temp_storage("kimi-work-cap");
    framework::upsert_source_instance(
        &storage,
        &framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "kimi-work@test".to_string(),
            agent: "kimi-work".to_string(),
            host_application: Some("daimon".to_string()),
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "kimi-wire-jsonl".to_string(),
            location_hint: None,
            parser_version: "kimi-wire-14-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        NOW,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'kimi-work@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored["adapter_id"], "kimi-work");
}
