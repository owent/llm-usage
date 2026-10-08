//! Native Crush cost data: current context snapshots must never become usage totals.
mod common;
use common::{temp_storage, TempDir};
use llm_usage_core::adapters::{built_in_adapters, framework::*};
use llm_usage_core::jobs::TriggerKind;

#[test]
fn real_main_and_title_cost_matches_api_without_adding_context_snapshots() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/crush/real-0.97.1/api-usage.json")).unwrap();
    let cli: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/crush/real-0.97.1/cli-stats.json")).unwrap();
    assert_eq!(api.as_array().unwrap().len(), 2);
    let total: i64 = api
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["usage"]["total_tokens"].as_i64().unwrap())
        .sum();
    assert_eq!(total, 5063);
    assert_eq!(cli["total"]["total_tokens"], 4901);
    let estimated_micro: i64 = api
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row["usage"]["prompt_tokens"].as_i64().unwrap()
                - row["usage"]["prompt_tokens_details"]["cached_tokens"]
                    .as_i64()
                    .unwrap()
                + row["usage"]["completion_tokens"].as_i64().unwrap()
        })
        .sum();
    assert_eq!(estimated_micro, 5059);
    assert_eq!(
        (cli["total"]["total_cost"].as_f64().unwrap() * 1e6).round() as i64,
        estimated_micro
    );

    let dir = TempDir::new("crush-real");
    let data = dir.path().join("project/.crush");
    let registry = dir.path().join("global");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&registry).unwrap();
    let file = data.join("crush.db");
    {
        let db = rusqlite::Connection::open(&file).unwrap();
        db.execute_batch(include_str!("fixtures/crush/real-0.97.1/session.sql"))
            .unwrap();
    }
    std::fs::write(
        registry.join("projects.json"),
        serde_json::to_vec(&serde_json::json!({"projects":[{
            "path":dir.path().join("project"),"data_dir":".crush"
        }]}))
        .unwrap(),
    )
    .unwrap();
    let original = std::fs::read(&file).unwrap();
    let mut ctx = DiscoverContext::default();
    ctx.env.insert(
        "CRUSH_GLOBAL_DATA".into(),
        registry.to_string_lossy().into(),
    );
    let (_db, storage) = temp_storage("crush-real");
    for pass in 0..2 {
        for adapter in built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &RunConfig {
                    timezone: "UTC".into(),
                    now_ms: 1_800_000_000_000 + pass,
                    limits: ScanLimits::default(),
                    trigger: TriggerKind::Manual,
                    run_id_prefix: format!("crush-{pass}-{}", adapter.adapter_id()),
                    origin_host_id: None,
                },
            )
            .unwrap();
            if adapter.adapter_id() == "crush" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        let count: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let observation: (String, i64, String, String, Option<String>) = storage
            .conn()
            .query_row(
                "SELECT record_kind,cost_amount_minor,cost_currency,cost_kind,model_raw FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(
            observation,
            (
                "usage_observation".into(),
                5059,
                "USD".into(),
                "estimated".into(),
                None
            )
        );
        let tokens: [Option<i64>; 8] = storage.conn().query_row("SELECT input_uncached,input_cache_read,input_cache_write,input_total,output_total,output_reasoning,total_tokens,source_total FROM usage_events",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?])).unwrap();
        assert_eq!(tokens, [None; 8]);
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
}
