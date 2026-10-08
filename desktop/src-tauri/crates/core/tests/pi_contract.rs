//! pi requirement tests use native redacted M2-B/C recovery data from local pi 0.87.1:
//! session-error-zero-usage. Read, parse, normalize, commit_batch and query;
//! compare manually calculated tests/fixtures/pi/session-error-zero-usage._expectations.md.

mod common;

use common::*;
use llm_usage_core::adapters::framework::SourceAdapter;
use llm_usage_core::adapters::pi::PiAdapter;

// Expected values are calculated manually and checked record-by-record with jq:
// Seven records: session/model_change/thinking_level_change/custom/system message/
// custom_message/assistant message; only the seventh assistant produces an event.
// All five usage fields explicitly report zero, including totalTokens; these zeros are known.
// Reasoning is absent and remains unknown; cost.total=0 does not produce a cost value.
// stopReason=error sets error_status="error"; responseId/duration/ttft are absent.

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
    // Explicitly reported zero is known.
    assert_eq!(summary.totals.input_total_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(0));
    assert_eq!(summary.totals.cache_read_known, Some(0));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(0));
    assert_eq!(summary.totals.uncached_known, Some(0));

    // Check category/model/provider/version/error/unknown fields through SQL.
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

    // pi has no cumulative snapshots and produces no reconciliation results.
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
    // All eight field capability entries are present.
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
    // Capability metadata survives storage in source_instances.capabilities.
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
