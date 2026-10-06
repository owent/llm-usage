//! Real jcode 0.91.0 single run, independently checked against model API and CLI.
mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{
    built_in_adapters,
    framework::{run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter},
    jcode::JcodeAdapter,
};
use llm_usage_core::jobs::TriggerKind;

#[test]
fn real_custom_provider_keeps_unknown_buckets_and_snapshot_version_scope() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/jcode/real-0.91.0/api-usage.json")).unwrap();
    let cli: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/jcode/real-0.91.0/cli-usage.json")).unwrap();
    assert_eq!(api["status"], 200);
    assert_eq!(api["usage"]["prompt_tokens"], cli["usage"]["input_tokens"]);
    assert_eq!(
        api["usage"]["completion_tokens"],
        cli["usage"]["output_tokens"]
    );
    assert_eq!(api["usage"]["total_tokens"], 462);
    let fixture = include_str!("fixtures/jcode/real-0.91.0/session.json");
    let snapshot: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        snapshot["env_snapshots"][0]["jcode_version"],
        "v0.91.0 (439a243bb)"
    );
    assert!(snapshot.get("version").is_none());
    let dir = TempDir::new("jcode-real-carrier");
    let sessions = dir.path().join("isolated-jcode/sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let file = sessions.join(format!("{}.json", snapshot["id"].as_str().unwrap()));
    std::fs::write(&file, fixture).unwrap();
    let original = std::fs::read(&file).unwrap();
    let (_database_dir, storage) = temp_storage("jcode-real-registry");
    let mut context = DiscoverContext::default();
    context.env.insert(
        "JCODE_HOME".into(),
        sessions.parent().unwrap().to_string_lossy().into(),
    );
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
                    run_id_prefix: format!("jcode-real-{pass}-{}", adapter.adapter_id()),
                    origin_host_id: None,
                },
            )
            .unwrap();
            if adapter.adapter_id() == "jcode" {
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
        let unknowns: [Option<i64>; 5] = storage.conn().query_row(
            "SELECT input_cache_read,input_cache_write,input_uncached,output_reasoning,total_tokens FROM usage_events", [],
            |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?])
        ).unwrap();
        assert_eq!(unknowns, [Some(0), None, None, None, None]);
        let metadata: (String, String, String) = storage
            .conn()
            .query_row(
                "SELECT provider_id,model_raw,schema_version FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            metadata,
            (
                "local-model".into(),
                "qwen2.5-0.5b-local".into(),
                "jcode-session-1".into()
            )
        );
        let totals = summary(&storage, "2026-10-06", "2026-10-06").totals;
        assert_eq!(
            (
                totals.call_count,
                totals.input_total_known,
                totals.output_total_known,
                totals.total_tokens_known
            ),
            (1, Some(460), Some(2), None)
        );
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
    let capability = JcodeAdapter::new().capability();
    assert_eq!(capability.supported_versions, vec!["jcode-session-1"]);
    assert!(capability.maintenance["evidence_level"]
        .as_str()
        .unwrap()
        .starts_with("real-local"));
}
