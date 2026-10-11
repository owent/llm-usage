//! Native per-version checks and consumed-cursor recovery after registry updates.
mod common;
use common::*;
use llm_usage_core::adapters::framework::*;
use llm_usage_core::domain::VersionBasis;
use llm_usage_core::storage::Storage;
use llm_usage_core::CoreError;
use serde_json::Value;
use std::path::Path;

struct BeforeRegistration<'a>(&'a dyn SourceAdapter);

impl SourceAdapter for BeforeRegistration<'_> {
    fn adapter_id(&self) -> &'static str {
        self.0.adapter_id()
    }
    fn agent(&self) -> &'static str {
        self.0.agent()
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        self.0.discover(ctx)
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        self.0.instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        let mut detected = self.0.detect(path)?;
        if let DetectOutcome::Supported { basis, .. } = &mut detected {
            *basis = VersionBasis::LatestFallback;
        }
        Ok(detected)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now_ms: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut outcome = self.0.scan(target, stored, limits, now_ms)?;
        for event in &mut outcome.events {
            event.parse_basis = Some(VersionBasis::LatestFallback);
        }
        outcome.health = "active_compat".into();
        Ok(outcome)
    }
    fn capability(&self) -> CapabilityTable {
        let mut capability = self.0.capability();
        capability.supported_versions.clear();
        capability
    }
}

fn run_registry(
    storage: &Storage,
    ctx: &DiscoverContext,
    config: &RunConfig,
) -> Vec<SourceRunReport> {
    llm_usage_core::adapters::built_in_adapters()
        .iter()
        .flat_map(|adapter| run_adapter_scan(storage, adapter.as_ref(), ctx, config).unwrap())
        .collect()
}

fn verify_native_sample(adapter: &dyn SourceAdapter, root: &Path, exp: &Value) {
    let (_db, storage) = temp_storage("compatibility-release-032");
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.to_path_buf()],
    };
    let mut config = RunConfig {
        timezone: "UTC".into(),
        now_ms: ts("2026-10-11T00:00:00Z"),
        limits: ScanLimits::default(),
        trigger: llm_usage_core::jobs::TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: "compatibility-release-032".into(),
    };
    let old = run_adapter_scan(&storage, &BeforeRegistration(adapter), &ctx, &config).unwrap();
    assert_eq!(old.len(), 1);
    assert!(old[0].error.is_none(), "{:?}", old[0].error);
    let before = summary(&storage, "2020-01-01", "2100-01-01").totals;
    assert_eq!(before.call_count, exp["calls"].as_i64().unwrap());
    for (field, actual) in [
        ("input", before.input_total_known),
        ("cached", before.cache_read_known),
        ("write", before.cache_write_known),
        ("output", before.output_total_known),
        ("total", before.total_tokens_known),
    ] {
        assert_eq!(actual, exp[field].as_i64(), "{}: {field}", exp["version"]);
    }
    let reasoning: i64 = storage
        .conn()
        .query_row("SELECT SUM(output_reasoning) FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(reasoning, exp["reasoning"].as_i64().unwrap());
    let old_state: (i64, i64, i64) = storage.conn().query_row(
        "SELECT COUNT(*),SUM(status='active_compat'),(SELECT COUNT(*) FROM ingestion_checkpoints) FROM source_files",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).unwrap();
    assert_eq!(old_state, (1, 1, 1));
    let first_observations: Vec<(String, i64)> = {
        let mut stmt = storage
            .conn()
            .prepare("SELECT event_id,created_at_ms FROM usage_events ORDER BY event_id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let diagnostics_before: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert!(diagnostics_before > 0);
    config.now_ms += 1;
    config.run_id_prefix = format!("compatibility-release-032-{}", config.now_ms);
    let replay = run_registry(&storage, &ctx, &config);
    assert_eq!(
        replay.len(),
        1,
        "registry must discover exactly the owning adapter"
    );
    assert!(replay[0].error.is_none(), "{:?}", replay[0].error);
    assert!(
        replay[0].files[0].records_seen > 0,
        "consumed unchanged sources must be replayed"
    );
    let outcome = replay[0].outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.conflicts), (0, 0));
    assert_eq!(outcome.updated, before.call_count);
    let state: (String, i64, i64) = storage.conn().query_row(
        "SELECT status,(SELECT COUNT(*) FROM usage_events WHERE parse_basis='known_version'),(SELECT COUNT(*) FROM diagnostics) FROM source_files",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).unwrap();
    assert_eq!(state.0, "active", "{}", exp["version"]);
    assert_eq!(state.1, before.call_count);
    assert!(state.2 >= diagnostics_before);
    assert_eq!(summary(&storage, "2020-01-01", "2100-01-01").totals, before);
    for (id, created) in first_observations {
        let retained: i64 = storage
            .conn()
            .query_row(
                "SELECT created_at_ms FROM usage_events WHERE event_id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(retained, created);
    }
    if let Some(verdict) = exp["reconciliation"].as_str() {
        assert_eq!(replay[0].reconciliations[0].verdict, verdict);
        let missing: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE source_total IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(missing, exp["missing_source_total"].as_i64().unwrap());
    }
    let revision = storage.data_revision().unwrap();
    config.now_ms += 1;
    config.run_id_prefix = format!("compatibility-release-032-{}", config.now_ms);
    let repeat = run_registry(&storage, &ctx, &config);
    assert_eq!(
        repeat[0]
            .outcome
            .as_ref()
            .map(|o| (o.added, o.updated, o.conflicts))
            .unwrap_or_default(),
        (0, 0, 0)
    );
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(summary(&storage, "2020-01-01", "2100-01-01").totals, before);
}

#[test]
fn codex_native_versions_recover_through_the_full_registry_without_usage_changes() {
    let dir = codex_fixture("releases-032");
    let expectations: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expectations.json")).unwrap())
            .unwrap();
    for exp in expectations.as_array().unwrap() {
        let source = TempDir::new("codex-release-032");
        let bytes = reconstruct_codex_jsonl(&dir.join(exp["fixture"].as_str().unwrap()));
        let root = codex_root_with_file(&source, "rollout-native.jsonl", &bytes);
        verify_native_sample(
            &llm_usage_core::adapters::codex::CodexAdapter::new(),
            &root,
            exp,
        );
    }
}

#[test]
fn kilo_native_session_versions_recover_through_parent_discovery_without_usage_changes() {
    let dir = kilo_fixture("releases-032");
    let expectations: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expectations.json")).unwrap())
            .unwrap();
    for exp in expectations.as_array().unwrap() {
        let source = TempDir::new("kilo-release-032");
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(exp["fixture"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let root = build_kilo_db(&source, &fixture);
        verify_native_sample(
            &llm_usage_core::adapters::kilo::KiloAdapter::new(),
            &root,
            exp,
        );
    }
}
