//! Kimi Code pipeline tests (A12/M4) with redacted session-main/subagent-agent-0 samples.
//! Extracted 2026-09-25 from desktop 1.0.3, wire protocol_version=1.5.
//! Read, parse, normalize, commit, and query; expected amounts were calculated manually.
//! References: matching _expectations.md files under tests/fixtures/kimi-code.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::kimi_code::KimiCodeAdapter;

// Manual expectations independently checked with jq:
// Main: 369 primary turn records + 3 auxiliary compaction/session records = 372.
// Main buckets: inputOther 1,506,693; cacheRead 47,066,368; output 257,373; creation 0.
// agent-0: 150 subagent turns; inputOther 198,401; cacheRead 18,428,416; output 72,798.
// Total 522 events: uncached 1,705,094; total input 67,199,878; total tokens 67,530,049.
// subagent.completed at 1790269234273 reports {66876,4631,1264384,0},
// a snapshot of subagent wire sums up to that time; do not create another usage event.
// The 368 step.end echoes omit interrupted usage; count usage records instead of echoes.

const NOW: i64 = 1_800_000_000_000;
/// Observed main-sample subagent.completed time, bounding subagent reconciliation.
const SUBAGENT_COMPLETED_MS: i64 = 1_790_269_234_273;

#[test]
fn contract_full_pipeline_matches_expectations() {
    let (_db, storage) = temp_storage("kimi-code-contract");
    // One session: sessions/<wd>/<session>/agents/{main,agent-0}/wire.jsonl.
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
    // Lexical order visits agents/agent-0 before agents/main.
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

    // Check all event sums over 2026-09-24/25 UTC.
    let summary = summary(&storage, "2026-09-24", "2026-09-25");
    assert_eq!(summary.totals.call_count, 522);
    assert_eq!(summary.totals.uncached_known, Some(1_705_094));
    assert_eq!(summary.totals.input_total_known, Some(67_199_878));
    assert_eq!(summary.totals.cache_read_known, Some(65_494_784));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(330_171));
    assert_eq!(summary.totals.total_tokens_known, Some(67_530_049));

    let conn = storage.conn();
    // Main: 369 primary + 3 auxiliary events; agent-0: 150 sub_agent events.
    let (primary, auxiliary, sub_agent): (i64, i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'auxiliary'), \
             SUM(call_category = 'sub_agent') FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((primary, auxiliary, sub_agent), (369, 3, 150));

    // Both files belong to kimi-code under the same root instance.
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

    // Session identity comes from the path; agents/<id> identifies subagent ownership.
    let sub_agent_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE call_category = 'sub_agent' \
             AND session_id = 'session_syn-real'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sub_agent_rows, 150);

    // Key: kimi-code:usage:<session>:<agent>:<time_ms>:<same-ms sequence>; schema 1.5.
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

    // usage.record has no uuid/messageId; do not invent origin_call_id.
    let origins: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE origin_call_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(origins, 0);

    // Preserve observed model strings, including combined alias/model identifiers.
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

    // Valid redacted native sample shapes produce zero diagnostics.
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);

    // Reconciliation: agent-0 records equal echoes (matched); main records exceed echoes
    // because one of 369 turns was interrupted and three session-scope records have no echo.
    // completed is no_detail_in_file because its details reside in the subagent wire.
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

    // Scanning again adds no events.
    let reports2 = run_kimi_code(&storage, &root, NOW + 1000);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0, "重复扫描不增量（V12）");
}

/// Check the M0 result: completed.usage equals the subagent wire sum through its timestamp.
/// Main completed {66876,4631,1264384,0} matches the first 22 agent-0 wire records.
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
    // Manual completed.usage: inputOther=66876, output=4631,
    // cacheRead=1264384, creation=0; compare agent-0 events at or before completion.
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
    // Later subagent wire records exceed this snapshot; retain all individual usage records.
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
    // Capability declarations survive storage round trips.
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
