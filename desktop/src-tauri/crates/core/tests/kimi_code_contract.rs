//! Kimi Code（A12，M4）合同测试：真实脱敏 fixture（session-main + subagent-agent-0，
//! 本机 desktop 1.0.3 / wire protocol_version=1.5，2026-09-25 提取）经
//! 读取→解析→标准化→commit_batch→查询 全链路。期望值为人工核算，
//! 见 tests/fixtures/kimi-code/*.sanitized.json 同名 _expectations.md。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::kimi_code::KimiCodeAdapter;

// 手工核算（对照 _expectations.md；roundtrip jq 独立复核相等）：
// main：369 turn（primary）+ 3 session（compaction ⇒ auxiliary）= 372；
//   inputOther 1,506,693 / cacheRead 47,066,368 / output 257,373 / creation 0。
// agent-0：150 turn（agents/agent-0 ⇒ sub_agent）；io 198,401 / cr 18,428,416 / out 72,798。
// 合计 522 事件：input_uncached 1,705,094；input_total 67,199,878；total 67,530,049。
// subagent.completed(agent-0, time=1790269234273) 的 usage {66876,4631,1264384,0}
// 是子代理 wire 截至该时刻 Σ 的快照 ⇒ 不产事件（防双计）；
// step.end 回声 Σ（368 条）小于记录侧（打断步无回声）⇒ 只计记录。

const NOW: i64 = 1_800_000_000_000;
/// main fixture 中 subagent.completed 的 time（子代理对账上界，白名单数字）。
const SUBAGENT_COMPLETED_MS: i64 = 1_790_269_234_273;

#[test]
fn contract_full_pipeline_matches_expectations() {
    let (_db, storage) = temp_storage("kimi-code-contract");
    // 单会话布局：sessions/<wd>/<session>/agents/{main,agent-0}/wire.jsonl。
    let main_wire = reconstruct_kimi_wire(&kimi_code_fixture("session-main.sanitized.json"));
    let sub_wire = reconstruct_kimi_wire(&kimi_code_fixture("subagent-agent-0.sanitized.json"));
    let dir = TempDir::new("kimi-code-real");
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/session_syn-real/agents/main/wire.jsonl",
        &main_wire,
    );
    let sub_path = dir
        .path()
        .join("sessions")
        .join("wd_syn")
        .join("session_syn-real")
        .join("agents")
        .join("agent-0")
        .join("wire.jsonl");
    std::fs::create_dir_all(sub_path.parent().unwrap()).unwrap();
    std::fs::write(&sub_path, &sub_wire).unwrap();

    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports.len(), 1);
    // 字典序：agents/agent-0 先扫，agents/main 后扫。
    assert_eq!(reports[0].files.len(), 2);
    let (sub_file, main_file) = (&reports[0].files[0], &reports[0].files[1]);
    assert_eq!(sub_file.status, "complete");
    assert_eq!(main_file.status, "complete");
    assert_eq!(sub_file.events, 150, "agent-0 wire 逐次记录");
    assert_eq!(
        main_file.events, 372,
        "369 turn + 3 session；completed/回声不计"
    );

    let outcome = reports[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.errors),
        (522, 0, 0)
    );

    // 全事件白名单核对（两日：2026-09-24/25 UTC）。
    let summary = summary(&storage, "2026-09-24", "2026-09-25");
    assert_eq!(summary.totals.call_count, 522);
    assert_eq!(summary.totals.uncached_known, Some(1_705_094));
    assert_eq!(summary.totals.input_total_known, Some(67_199_878));
    assert_eq!(summary.totals.cache_read_known, Some(65_494_784));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(330_171));
    assert_eq!(summary.totals.total_tokens_known, Some(67_530_049));

    let conn = storage.conn();
    // 分类：main 文件 369 primary + 3 auxiliary；agent-0 150 sub_agent。
    let (primary, auxiliary, sub_agent): (i64, i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'auxiliary'), \
             SUM(call_category = 'sub_agent') FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((primary, auxiliary, sub_agent), (369, 3, 150));

    // Agent 分列：kimi-code（两文件同 Agent；实例身份同根）。
    let agents: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare("SELECT agent, COUNT(*) FROM usage_events GROUP BY agent")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        agents,
        vec![("kimi-code".to_string(), 522)],
        "kimi-work 不混入（A12/A13 分列）"
    );

    // 会话/子代理身份：session_id 来自路径（agents/<id> 归属子代理）。
    let sub_agent_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE call_category = 'sub_agent' \
             AND session_id = 'session_syn-real'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sub_agent_rows, 150);

    // 事件键形状：kimi-code:usage:{time 毫秒}:{同毫秒序号}；schema_version=1.5。
    let (key, schema, basis): (String, String, String) = conn
        .query_row(
            "SELECT source_record_key, schema_version, parse_basis FROM usage_events \
             WHERE source_record_key = 'kimi-code:usage:session_syn-real:main:1790268536813:0'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(key, "kimi-code:usage:session_syn-real:main:1790268536813:0");
    assert_eq!(schema, "1.5");
    assert_eq!(basis, "known_version", "1.5 已验证锚点");

    // usage.record 无 uuid/messageId：origin_call_id 不伪造。
    let origins: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE origin_call_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(origins, 0);

    // 模型白名单（model 原样入账，alias/model 组合串不拆）。
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
            ("Kimi For Coding - Backup/k3-256k".to_string(), 324),
            ("kimi-code/k3-256k".to_string(), 177),
            ("kimi-code/kimi-for-coding".to_string(), 21),
        ]
    );

    // 诊断：真实 fixture 无坏形状 ⇒ 0 诊断。
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);

    // 回声对账（run 报告白名单）：agent-0 记录==回声（matched）；main 记录>回声
    // （369 turn 中 1 条被打断步无回声 + 3 session scope 无回声 ⇒ echo_subset）；
    // completed 快照行 no_detail_in_file（明细在子代理 wire）。
    let recs = &reports[0].reconciliations;
    assert!(recs
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "matched"));
    assert!(recs.iter().any(|r| r.series == "kimi_wire_step_end_echo"
        && r.verdict == "echo_subset"
        && r.snapshot_final.is_some()
        && matches!(r.difference, Some(d) if d > 0)
        && r.detail_sum > r.snapshot_final.unwrap_or(0)));
    assert!(recs
        .iter()
        .any(|r| r.series.starts_with("kimi_subagent_completed_snapshot")
            && r.verdict == "no_detail_in_file"));

    // 幂等：二次扫描无新增。
    let reports2 = run_kimi_code(&storage, &root, NOW + 1000);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0, "重复扫描不增量（V12）");
}

/// M0 结论复证：subagent.completed.usage == 子代理 wire 截至其 time 的逐次 Σ。
/// （主线 completed.usage {66876,4631,1264384,0} vs agent-0 前 22 条 wire。）
#[test]
fn contract_subagent_completed_reconciles_with_subagent_wire() {
    let (_db, storage) = temp_storage("kimi-code-sub");
    let main_wire = reconstruct_kimi_wire(&kimi_code_fixture("session-main.sanitized.json"));
    let sub_wire = reconstruct_kimi_wire(&kimi_code_fixture("subagent-agent-0.sanitized.json"));
    let dir = TempDir::new("kimi-code-subrecon");
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/session_syn-real/agents/main/wire.jsonl",
        &main_wire,
    );
    let sub_path = dir
        .path()
        .join("sessions")
        .join("wd_syn")
        .join("session_syn-real")
        .join("agents")
        .join("agent-0")
        .join("wire.jsonl");
    std::fs::create_dir_all(sub_path.parent().unwrap()).unwrap();
    std::fs::write(&sub_path, &sub_wire).unwrap();
    run_kimi_code(&storage, &root, NOW);

    let conn = storage.conn();
    // completed.usage（人工核算自 fixture）：inputOther=66876, output=4631,
    // cacheRead=1264384, creation=0；对账对象 = agent-0 事件中 time ≤ completed 的 Σ。
    let (io, out, cr, cc, n): (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT \
                COALESCE(SUM(input_uncached), 0), COALESCE(SUM(output_total), 0), \
                COALESCE(SUM(input_cache_read), 0), COALESCE(SUM(input_cache_write), 0), \
                COUNT(*) \
             FROM usage_events WHERE call_category = 'sub_agent' AND occurred_at_ms <= ?1",
            [SUBAGENT_COMPLETED_MS],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        (io, out, cr, cc, n),
        (66_876, 4_631, 1_264_384, 0, 22),
        "subagent.completed.usage 与子代理 wire 逐字段相等（M0 + 本机复证）"
    );
    // completed 之后子代理 wire 继续增长（复用段）⇒ 总 Σ 大于快照，全量逐次入账。
    let total_sub: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE call_category = 'sub_agent'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(total_sub, 150);
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = KimiCodeAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "kimi-code");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["1.5", "1.4"])
    );
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
    assert_eq!(
        json["fields"]["cost"]["availability"]["unavailable"],
        "wire 无费用字段；远端账单/额度页不接入"
    );
    assert_eq!(
        json["detection"]["version_field"],
        "metadata.protocol_version（字符串；本机 13 文件 = 11×\"1.5\" + 2×\"1.4\"（旧会话走 latest_fallback 兼容尝试））"
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
    // 能力声明可落库 roundtrip。
    let (_db, storage) = temp_storage("kimi-code-cap");
    framework::upsert_source_instance(
        &storage,
        &framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "kimi-code@test".to_string(),
            agent: "kimi-code".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "kimi-wire-jsonl".to_string(),
            location_hint: None,
            parser_version: "kimi-wire-15-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        NOW,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'kimi-code@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
    assert_eq!(stored["adapter_id"], "kimi-code");
}
