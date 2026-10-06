//! Actual Goose 1.53.0 SQLite schema/rows, checked against real local inference.
mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{
    built_in_adapters,
    framework::{run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter},
    goose::GooseAdapter,
};
use llm_usage_core::jobs::TriggerKind;
use rusqlite::params;

#[test]
fn real_ledger_keeps_missing_cache_and_cost_and_does_not_add_session_totals() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/goose/real-1.53.0/api-usage.json")).unwrap();
    assert_eq!(api["status"], 200);
    assert_eq!(
        (
            api["usage"]["prompt_tokens"].as_i64(),
            api["usage"]["completion_tokens"].as_i64(),
            api["usage"]["total_tokens"].as_i64()
        ),
        (Some(320), Some(2), Some(322))
    );
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/goose/real-1.53.0/sessions-projection.json"
    ))
    .unwrap();
    let dir = TempDir::new("goose-real-carrier");
    let root = dir.path().join("isolated-goose");
    let sessions = root.join("data/sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let file = sessions.join("sessions.db");
    {
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute_batch(source["schemas"]["sessions"].as_str().unwrap())
            .unwrap();
        conn.execute_batch(source["schemas"]["usage_ledger"].as_str().unwrap())
            .unwrap();
        let session = &source["sessions"][0];
        conn.execute("INSERT INTO sessions(id,working_dir,created_at,updated_at,accumulated_input_tokens,accumulated_output_tokens,accumulated_total_tokens,accumulated_cache_read_tokens,accumulated_cache_write_tokens,accumulated_cost) VALUES(?1,'REDACTED',?2,?3,?4,?5,?6,?7,?8,?9)",params![session["id"].as_str().unwrap(),session["created_at"].as_str().unwrap(),session["updated_at"].as_str().unwrap(),session["accumulated_input_tokens"].as_i64(),session["accumulated_output_tokens"].as_i64(),session["accumulated_total_tokens"].as_i64(),session["accumulated_cache_read_tokens"].as_i64(),session["accumulated_cache_write_tokens"].as_i64(),session["accumulated_cost"].as_f64()]).unwrap();
        for row in source["ledger"].as_array().unwrap() {
            conn.execute("INSERT INTO usage_ledger(id,session_id,created_timestamp,model,input_tokens,output_tokens,total_tokens,cache_read_tokens,cache_write_tokens,cost,cost_source,is_compaction) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![row["id"].as_i64().unwrap(),row["session_id"].as_str().unwrap(),row["created_timestamp"].as_i64().unwrap(),row["model"].as_str(),row["input_tokens"].as_i64(),row["output_tokens"].as_i64(),row["total_tokens"].as_i64(),row["cache_read_tokens"].as_i64(),row["cache_write_tokens"].as_i64(),row["cost"].as_f64(),row["cost_source"].as_str(),row["is_compaction"].as_i64()]).unwrap();
        }
    }
    let original = std::fs::read(&file).unwrap();
    let (_database_dir, storage) = temp_storage("goose-real-registry");
    let mut context = DiscoverContext::default();
    context
        .env
        .insert("GOOSE_PATH_ROOT".into(), root.to_string_lossy().into());
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
                    run_id_prefix: format!("goose-real-{pass}-{}", adapter.adapter_id()),
                    origin_host_id: None,
                },
            )
            .unwrap();
            if adapter.adapter_id() == "goose" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none(), "{:?}", reports[0].error);
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
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
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT COUNT(*) FROM source_aggregates", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let unknowns: [Option<i64>;5]=storage.conn().query_row("SELECT input_cache_read,input_cache_write,input_uncached,output_reasoning,cost_amount_minor FROM usage_events",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?])).unwrap();
        assert_eq!(unknowns, [Some(0), None, None, None, None]);
        let totals = summary(&storage, "2026-10-06", "2026-10-06").totals;
        assert_eq!(
            (
                totals.call_count,
                totals.input_total_known,
                totals.output_total_known,
                totals.total_tokens_known
            ),
            (1, Some(320), Some(2), Some(322))
        );
        assert_eq!(
            std::fs::read(&file).unwrap(),
            original,
            "source database is read-only"
        );
    }
    assert!(
        GooseAdapter::new().capability().maintenance["evidence_level"]
            .as_str()
            .unwrap()
            .starts_with("real-local")
    );
}
