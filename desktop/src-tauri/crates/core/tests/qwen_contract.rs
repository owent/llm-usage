//! Qwen Code field tests: synthetic boundaries and redacted native 0.25.0 local-model records.
//! Exercise reading, parsing, normalization, commit_batch and queries; each dataset states its origin.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{self, SourceAdapter};
use llm_usage_core::adapters::qwen::QwenAdapter;

// Manual calculations match the test data's _expectations.md.
// syn-a-1 (primary): prompt 1000, candidates 50, cached 400, thoughts 10, toolUse 5, total 1050.
// syn-a-2 (isSidechain -> sub_agent): prompt 2000, candidates 100, cached 0, total 2100.
// syn-a-3 (agentId -> sub_agent): prompt 500, candidates 25, total 525.
// Totals: call_count=3, input_total=1000+2000+500=3500, output_total=50+100+25=175.
// cache_read=400+0=400; a3 is unknown, never filled with zero. total_tokens=1050+2100+525=3675.
// thoughts/toolUsePrompt enter no bucket. goal_state tokensUsed=4,000,000,000 is a
// cross-turn cumulative value and is excluded; chat_compression/session_model create no events.

const NOW: i64 = 1_800_000_000_000;

#[test]
fn real_025_local_main_usage_matches_native_records_without_inventing_background_calls() {
    for (fixture, input, total) in [
        ("real-0.25.0-local-default", 10_226i64, 10_228i64),
        ("real-0.25.0-local-controlled", 8_903, 8_905),
    ] {
        let (_db, storage) = temp_storage(fixture);
        let root = qwen_fixture(fixture);
        let reports = run_qwen(&storage, &root, NOW);
        let outcome = reports[0].outcome.as_ref().unwrap();
        assert_eq!((outcome.added, outcome.updated, outcome.errors), (1, 0, 0));
        let totals = summary(&storage, "2026-10-05", "2026-10-05").totals;
        assert_eq!(totals.call_count, 1);
        assert_eq!(totals.input_total_known, Some(input));
        assert_eq!(totals.output_total_known, Some(2));
        assert_eq!(totals.cache_read_known, Some(0));
        assert_eq!(totals.total_tokens_known, Some(total));
        assert_eq!(totals.cache_write_known, None);
        let identity: (String, String, Option<String>, Option<String>) = storage
            .conn()
            .query_row(
                "SELECT schema_version, model_raw, model_canonical, provider_id FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            identity,
            ("0.25.0".into(), "qwen2.5-0.5b-local".into(), None, None)
        );
        // Default CLI stats also count a background memory call; these ChatRecords cannot invent that event.
        let revision = storage.data_revision().unwrap();
        let repeated = run_qwen(&storage, &root, NOW + 1000);
        assert_eq!(repeated[0].files[0].status, "unchanged");
        assert!(repeated[0].outcome.is_none());
        assert_eq!(storage.data_revision().unwrap(), revision);
        assert_eq!(
            summary(&storage, "2026-10-05", "2026-10-05")
                .totals
                .total_tokens_known,
            Some(total)
        );
    }
}

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
    // Categories: one primary and two sub_agent records through isSidechain/agentId.
    let (primary, sub): (i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'sub_agent') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((primary, sub), (1, 2));

    // Event key = qwen:{uuid}; schema_version retains each record.version; no provider field exists.
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

    // thoughts=10/toolUsePrompt=5 enter no bucket; reasoning/uncached/source_total are NULL.
    let nulls: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT output_reasoning, input_uncached, source_total, input_cache_write \
             FROM usage_events WHERE source_record_key = 'qwen:syn-a-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(nulls, (None, None, None, None));

    // Exclude goal_state cumulative 4,000,000,000 and compression/control records from all totals.
    // total_tokens_known=3675 already checks this; MAX(total_tokens) also excludes the large value.
    let max_total: Option<i64> = conn
        .query_row("SELECT MAX(total_tokens) FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(max_total, Some(2_100));

    // Check categories and identities of both sub_agent records.
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

    // No diagnostics.
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
    // All eight field capabilities are present.
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
    // Round-trip capabilities through source_instances.capabilities.
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
