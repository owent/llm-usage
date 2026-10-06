//! Actual Continue CLI cumulative carrier; initialized cache zeroes are not API evidence.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{continuedev::ContinueAdapter, framework::*};
use llm_usage_core::{domain::FieldQuality, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
const FIXTURE: &str = include_str!("fixtures/continue/real-1.5.47/session.json");
const NOW: i64 = 1_800_000_000_000;

fn carrier(dir: &TempDir) -> (DiscoverContext, PathBuf) {
    let root = dir.path().join("continue");
    std::fs::create_dir_all(root.join("sessions")).unwrap();
    let file = root.join("sessions/11111111-2222-4333-8444-555555555555.json");
    std::fs::write(&file, FIXTURE).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_791_266_584_000),
        ))
        .unwrap();
    std::fs::write(root.join("sessions/sessions.json"), "[]").unwrap();
    let mut ctx = DiscoverContext::default();
    ctx.env
        .insert("CONTINUE_GLOBAL_DIR".into(), root.to_string_lossy().into());
    (ctx, file)
}
fn config(id: &str) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: id.into(),
        origin_host_id: None,
    }
}
fn check(storage: &Storage) {
    let row: [Option<i64>;7] = storage.conn().query_row("SELECT input_total,output_total,total_tokens,input_cache_read,input_cache_write,reported_call_count,input_uncached FROM source_aggregates",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?])).unwrap();
    assert_eq!(row, [Some(1471), Some(2), None, None, None, None, None]);
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let totals = summary(storage, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            totals.input_total_known,
            totals.output_total_known,
            totals.total_tokens_known,
            totals.call_count
        ),
        (None, None, None, 0)
    );
    let instance: String = storage
        .conn()
        .query_row("SELECT instance_id FROM source_aggregates", [], |r| {
            r.get(0)
        })
        .unwrap();
    let totals = llm_usage_core::aggregates::sum_exclusive_aggregates(storage, &instance).unwrap();
    assert_eq!(
        (
            totals.input_total,
            totals.output_total,
            totals.total_tokens,
            totals.reported_call_count
        ),
        (Some(1471), Some(2), None, None)
    );
}
#[test]
fn real_streaming_session_preserves_cumulative_scope_and_unknown_defaults() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/continue/real-1.5.47/api-usage.json")).unwrap();
    assert_eq!(api["usage"]["prompt_tokens"], 1471);
    assert_eq!(api["usage"]["completion_tokens"], 2);
    assert_eq!(api["usage"]["total_tokens"], 1473);
    let dir = TempDir::new("continue-real");
    let (ctx, file) = carrier(&dir);
    let (_db, storage) = temp_storage("continue-real");
    let original = std::fs::read(&file).unwrap();
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config(&format!("real-{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() == "continue" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        check(&storage);
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
    assert!(
        ContinueAdapter::new().capability().maintenance["evidence_level"]
            .as_str()
            .unwrap()
            .starts_with("real-local")
    );
}

struct Legacy(&'static str);
impl SourceAdapter for Legacy {
    fn adapter_id(&self) -> &'static str {
        "continue"
    }
    fn agent(&self) -> &'static str {
        "continue"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        ContinueAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        ContinueAdapter::new().instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, llm_usage_core::error::CoreError> {
        ContinueAdapter::new().detect(path)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<ScanOutcome, llm_usage_core::error::CoreError> {
        let mut result = ContinueAdapter::new().scan(target, stored, limits, now)?;
        for a in &mut result.aggregates {
            a.usage.input_cache_read = Some(0);
            a.quality.input_cache_read = FieldQuality::Reported;
            a.usage.input_cache_write = Some(0);
            a.quality.input_cache_write = FieldQuality::Reported;
            match self.0 {
                "input" => a.usage.input_total = Some(1400),
                "quality" => a.quality.output_total = FieldQuality::Derived,
                "coverage" => a.coverage = llm_usage_core::aggregates::Coverage::OverlapUnknown,
                "revision" => a.source_revision = a.source_revision.map(|v| v + 1),
                _ => {}
            }
        }
        Ok(result)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = ContinueAdapter::new().capability();
        c.maintenance["parser_version"] = "continue-session-usage-1".into();
        c
    }
}
#[test]
fn unchanged_legacy_cursor_reassesses_exact_cache_policy_and_rolls_back_with_checkpoint() {
    for fail_once in [false, true] {
        let dir = TempDir::new("continue-old");
        let (ctx, file) = carrier(&dir);
        let (_db, storage) = temp_storage("continue-old");
        run_adapter_scan(&storage, &Legacy("zero"), &ctx, &config("old")).unwrap();
        let source_revision: i64 = storage
            .conn()
            .query_row("SELECT source_revision FROM source_aggregates", [], |r| {
                r.get(0)
            })
            .unwrap();
        storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','test prior diagnostic',0 FROM source_instances",[]).unwrap();
        if fail_once {
            storage.conn().execute_batch("CREATE TRIGGER fail_continue_update BEFORE UPDATE ON source_aggregates BEGIN SELECT RAISE(ABORT,'injected aggregate failure'); END;").unwrap();
            let reports =
                run_adapter_scan(&storage, &ContinueAdapter::new(), &ctx, &config("failed"))
                    .unwrap();
            assert_eq!(reports[0].finish, llm_usage_core::jobs::RunStatus::Failed);
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT input_cache_write FROM source_aggregates", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            storage
                .conn()
                .execute_batch("DROP TRIGGER fail_continue_update")
                .unwrap();
        }
        let adapter = ContinueAdapter::new();
        let requests = [ParallelScanRequest {
            adapter: &adapter,
            context: ctx.clone(),
            config: config("upgraded"),
            filter: InstanceFilter::default(),
        }];
        let locked = std::sync::Mutex::new(storage);
        let results = run_adapter_scans_parallel(
            &locked,
            &requests,
            None,
            std::sync::Arc::new(|| true),
            None,
        );
        let reports = results.into_iter().next().unwrap().unwrap();
        let storage = locked.into_inner().unwrap();
        assert!(reports.iter().all(|r| r.error.is_none()));
        check(&storage);
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT source_revision FROM source_aggregates", [], |r| r
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            source_revision
        );
        run_adapter_scan(&storage, &adapter, &ctx, &config("repeat")).unwrap();
        check(&storage);
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='preserved_history'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(std::fs::read(&file).unwrap(), FIXTURE.as_bytes());
    }
}

#[test]
fn cache_correction_does_not_replace_other_token_quality_coverage_or_newer_revision() {
    for variant in ["input", "quality", "coverage", "revision"] {
        let dir = TempDir::new("continue-conflict");
        let (ctx, _file) = carrier(&dir);
        let (_db, storage) = temp_storage("continue-conflict");
        run_adapter_scan(&storage, &Legacy(variant), &ctx, &config("old")).unwrap();
        let original: String = storage
            .conn()
            .query_row("SELECT content_hash FROM source_aggregates", [], |r| {
                r.get(0)
            })
            .unwrap();
        run_adapter_scan(&storage, &ContinueAdapter::new(), &ctx, &config("new")).unwrap();
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT content_hash FROM source_aggregates", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            original
        );
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        let expected = i64::from(variant != "revision");
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_conflict'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            expected
        );
    }
}
