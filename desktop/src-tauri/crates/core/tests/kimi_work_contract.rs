//! Kimi Work (A13, M4) tests use real sanitized conv-main and agent-44-subagent samples.
//! Extracted 2026-09-25 from the local embedded kimi-code home, wire protocol_version=1.4.
//! Read, parse, normalize, commit_batch and query against manual totals in
//! tests/fixtures/kimi-work/*.sanitized.json and their corresponding _expectations.md files.
//! Assertions distinguish this sample from Kimi Code 1.5: conv-* layout,
//! no usage.record agentId, bare model ids and registered format 1.4.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::kimi_work::KimiWorkAdapter;

// Manual totals cross-checked against _expectations.md and an independent jq round trip.
// conv-main: 38 turn/primary records; io 47,574, cr 1,310,720, out 18,658.
// agent-44: 7 turn/sub_agent records plus 1 session/auxiliary record;
// io 204,485+29,663, cr 374,784+183,040, out 23,343+12,440.
// 46 events: input_uncached 281,722, input_total 2,150,266, total 2,204,707.

const NOW: i64 = 1_800_000_000_000;

#[test]
fn contract_full_pipeline_matches_expectations() {
    let (_db, storage) = temp_storage("kimi-work-contract");
    // Two real sessions in distinct wd/conv directories under one instance root.
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

    // Compare all events over a date range containing 2026-07-18 and 2026-09-13.
    let summary = summary(&storage, "2026-07-18", "2026-09-13");
    assert_eq!(summary.totals.call_count, 46);
    assert_eq!(summary.totals.uncached_known, Some(281_722));
    assert_eq!(summary.totals.input_total_known, Some(2_150_266));
    assert_eq!(summary.totals.cache_read_known, Some(1_868_544));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(54_441));
    assert_eq!(summary.totals.total_tokens_known, Some(2_204_707));

    let conn = storage.conn();
    // Categories: 38 primary, 1 auxiliary agent-44 compaction summary and 7 sub_agent.
    let (primary, auxiliary, sub_agent): (i64, i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'auxiliary'), \
             SUM(call_category = 'sub_agent') FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((primary, auxiliary, sub_agent), (38, 1, 7));

    // Agent is kimi-work, separately identified from kimi-code (A12/A13).
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

    // Verify the event-key namespace and registered schema 1.4.
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

    // Keep the observed bare model ids from 1.4.
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

    // Session identity uses the conv directory; 1.4 has no agentId field.
    let sub_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE session_id = 'conv_syn-sub'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sub_rows, 8, "agent-44 的 8 条（7 turn + 1 session）");

    // Valid real sample shapes produce zero diagnostics.
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);

    // Reconcile turn-scope records against echoes; session scope has no echo. Both files match:
    // conv-main 38 turns == 38 echoes; agent-44 7 turns == 7 echoes, excluding its session record.
    let matched = reports[0]
        .reconciliations
        .iter()
        .filter(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "matched")
        .count();
    assert_eq!(matched, 2, "两文件的回声对账均 matched");

    // Repeated scanning adds no duplicate events.
    let reports2 = run_kimi_work(&storage, &root, NOW + 1000);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0, "重复扫描不增量（V12）");
}

/// A manual-root Kimi Work scan returns its own source-instance namespace and Agent value.
/// Verify kimi-work identity and event keys independently of the kimi-code product (A12/A13).
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
    // Candidate roots come from local observations and manual input, with no environment override.
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
