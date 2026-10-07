mod common;
use common::*;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits,
};
use llm_usage_core::adapters::zed::ZedAdapter;
use llm_usage_core::domain::FieldQuality;
use llm_usage_core::jobs::TriggerKind;

#[test]
fn native_openai_threads_preserve_cache_semantics_unknown_zero_and_idempotent_rescan() {
    let source = TempDir::new("zed-native");
    let db = source.path().join("threads.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute("CREATE TABLE threads(id TEXT PRIMARY KEY,summary TEXT NOT NULL,updated_at TEXT NOT NULL,data_type TEXT NOT NULL,data BLOB NOT NULL,created_at TEXT)",[]).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/zed/native-1.22.0.json")).unwrap();
    for row in fixture["threads"].as_array().unwrap() {
        let compressed =
            zstd::stream::encode_all(serde_json::to_vec(row).unwrap().as_slice(), 3).unwrap();
        conn.execute(
            "INSERT INTO threads VALUES(?1,'synthetic',?2,'zstd',?3,'2026-10-07T03:38:00Z')",
            rusqlite::params![
                row["id"].as_str().unwrap(),
                row["updated_at"].as_str().unwrap(),
                compressed
            ],
        )
        .unwrap();
    }
    let unknown = serde_json::json!({"version":"0.3.0","model":{"provider":"llm-usage-zhipu"},"cumulative_token_usage":{}});
    conn.execute(
        "INSERT INTO threads VALUES('empty','empty','2026-10-07T03:40:00Z','json',?1,NULL)",
        [serde_json::to_vec(&unknown).unwrap()],
    )
    .unwrap();
    drop(conn);
    let (_dir, s) = temp_storage("zed-native-read");
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![db.clone()],
    };
    let cfg = RunConfig {
        timezone: "UTC".into(),
        now_ms: ts("2026-10-07T04:00:00Z"),
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: "zed-native".into(),
    };
    let bytes = std::fs::read(&db).unwrap();
    run_adapter_scan(&s, &ZedAdapter::new(), &ctx, &cfg).unwrap();
    let values:(i64,i64,i64,i64)=s.conn().query_row("SELECT COUNT(*),SUM(input_uncached),SUM(output_total),SUM(input_cache_read) FROM source_aggregates",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(values, (2, 21696, 61, 10816));
    let (zero,total,quality):(Option<i64>,Option<i64>,String)=s.conn().query_row("SELECT input_cache_write,total_tokens,quality_json FROM source_aggregates WHERE scope_key='zed:thread:flash-thread'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!((zero, total), (None, None));
    let quality: llm_usage_core::domain::TokenQuality = serde_json::from_str(&quality).unwrap();
    assert_eq!(quality.input_uncached, FieldQuality::Reported);
    assert_eq!(quality.input_cache_write, FieldQuality::Unknown);
    let revision = s.data_revision().unwrap();
    run_adapter_scan(&s, &ZedAdapter::new(), &ctx, &cfg).unwrap();
    assert_eq!(s.data_revision().unwrap(), revision);
    assert_eq!(std::fs::read(&db).unwrap(), bytes);
}
