//! Native Aider 0.86.2 analytics fields checked against independent local-model usage.
mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{
    aider::AiderAdapter,
    built_in_adapters,
    framework::{run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter},
};
use llm_usage_core::jobs::TriggerKind;

#[test]
fn real_local_analytics_matches_model_usage_through_registry_and_repeated_scan() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/aider/real-0.86.2/api-usage.json")).unwrap();
    assert_eq!(api["status"], 200);
    assert_eq!(api["usage"]["prompt_tokens"], 95);
    assert_eq!(api["usage"]["completion_tokens"], 3);
    assert_eq!(api["usage"]["total_tokens"], 98);
    let fixture = include_str!("fixtures/aider/real-0.86.2/analytics.jsonl");
    let rows: Vec<serde_json::Value> = fixture
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows.iter()
            .filter(|row| row["event"] == "message_send")
            .count(),
        1
    );
    assert!(rows.iter().all(|row| row.get("version").is_none()));
    let dir = TempDir::new("aider-real-carrier");
    let file = dir.path().join("analytics.jsonl");
    std::fs::write(&file, fixture).unwrap();
    let (_database_dir, storage) = temp_storage("aider-real-registry");
    let context = DiscoverContext {
        manual_roots: vec![file],
        ..Default::default()
    };
    for pass in 0..2 {
        for adapter in built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &context,
                &RunConfig {
                    timezone: "UTC".into(),
                    now_ms: 1_800_000_000_000 + pass,
                    limits: ScanLimits::default(),
                    trigger: TriggerKind::Manual,
                    run_id_prefix: format!("aider-real-{pass}-{}", adapter.adapter_id()),
                    origin_host_id: None,
                },
            )
            .unwrap();
            if adapter.adapter_id() == "aider" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none(), "{:?}", reports[0].error);
            }
        }
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        let event: serde_json::Value = storage.conn().query_row(
            "SELECT agent,provider_id,model_raw,input_total,output_total,total_tokens,input_cache_read,input_cache_write,output_reasoning,cost_amount_minor,cost_currency,cost_kind FROM usage_events",
            [], |r| Ok(serde_json::json!({
                "agent":r.get::<_,String>(0)?, "provider":r.get::<_,String>(1)?, "model":r.get::<_,String>(2)?,
                "input":r.get::<_,i64>(3)?, "output":r.get::<_,i64>(4)?, "total":r.get::<_,i64>(5)?,
                "cache_read":r.get::<_,Option<i64>>(6)?, "cache_write":r.get::<_,Option<i64>>(7)?, "reasoning":r.get::<_,Option<i64>>(8)?,
                "cost":r.get::<_,i64>(9)?, "currency":r.get::<_,String>(10)?, "cost_kind":r.get::<_,String>(11)?
            }))
        ).unwrap();
        assert_eq!(
            event,
            serde_json::json!({"agent":"aider", "provider":"openai", "model":"qwen2.5-0.5b-local", "input":95, "output":3, "total":98, "cache_read":null, "cache_write":null, "reasoning":null, "cost":0, "currency":"USD", "cost_kind":"estimated"})
        );
        let totals = summary(&storage, "2026-10-06", "2026-10-06").totals;
        assert_eq!(
            (
                totals.call_count,
                totals.input_total_known,
                totals.output_total_known,
                totals.total_tokens_known
            ),
            (1, Some(95), Some(3), Some(98))
        );
    }
    let capability = AiderAdapter::new().capability();
    assert_eq!(capability.supported_versions, vec!["aider-analytics-doc-1"]);
    assert!(capability.maintenance["evidence_level"]
        .as_str()
        .unwrap()
        .starts_with("real-local"));
}
