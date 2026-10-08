//! Codex adapter tests using real sanitized M0 samples from local 0.155.0-alpha.16.3.
//! Read, parse, normalize, commit_batch and query; results must match the manually
//! calculated tests/fixtures/codex/*._expectations.md values and the model/segment
//! calculations below, derived from those samples.

mod common;

use common::*;
use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{SourceAdapter, SourceFileRow};
use rusqlite::OptionalExtension;

// Manual totals: jq sums each sample record and cross-checks _expectations.md.
// multimodel: gpt-6-astra 169 calls, input 23,683,841, cached 22,957,312, output 109,592;
// reasoning 28,759, total 23,793,433; gpt-6-sol 52 calls, input 8,377,585,
// cached 8,098,560, output 33,789, reasoning 16,739, total 8,411,374.
// Final snapshot total 31,724,335; compacted carries 250,108 + 230,364 = 480,472.
// Complete per-call total 32,204,807 = 31,724,335 + 480,472.

fn setup_fixture(tag: &str, fixture: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new(tag);
    let jsonl = reconstruct_codex_jsonl(&codex_fixture(fixture));
    let root = codex_root_with_file(&dir, "rollout-reconstructed.jsonl", &jsonl);
    (dir, root)
}

#[test]
fn zero_source_budget_stops_before_usage_read_and_preserves_resumability() {
    use llm_usage_core::adapters::framework::ScanLimits;
    let (_source, root) = setup_fixture("codex-zero-budget", "rollout-single-call.sanitized.json");
    let (_db, storage) = temp_storage("codex-zero-budget");
    let mut limits = ScanLimits::default();
    limits.jsonl.time_budget = Some(std::time::Duration::ZERO);
    let reports = run_codex_with_limits(&storage, &root, 1_800_000_000_000, limits);
    assert_eq!(
        reports[0].finish,
        llm_usage_core::jobs::RunStatus::Interrupted
    );
    assert!(reports[0].files.is_empty());
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        run_codex(&storage, &root, 1_800_000_001_000)[0]
            .outcome
            .as_ref()
            .unwrap()
            .added,
        1
    );
}

#[test]
fn single_call_full_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("codex-single", "rollout-single-call.sanitized.json");
    let (_db, storage) = temp_storage("codex-single");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 1, "exactly one model call");
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (1, 0, 0));

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(25_558));
    assert_eq!(summary.totals.cache_read_known, Some(4_864));
    assert_eq!(summary.totals.output_total_known, Some(131));
    assert_eq!(summary.totals.total_tokens_known, Some(25_689));

    // auto-review subagent: parent_thread_id sets sub_agent; turn_context supplies the model.
    let (category, model, attribution): (String, String, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, model_raw, model_attribution FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(model, "codex-auto-review");
    assert_eq!(attribution, "provider_mapping");

    // Single-call session: per-call sum equals the final snapshot; reconciliation is matched.
    assert_eq!(report.reconciliations.len(), 1);
    let rec = &report.reconciliations[0];
    assert_eq!(rec.verdict, "matched");
    assert_eq!(rec.detail_sum, 25_689);
    assert_eq!(rec.snapshot_final, Some(25_689));
    assert_eq!(rec.carried_sum, 0);
    assert_eq!(rec.difference, Some(0));

    // Store the snapshot as a source interval aggregate for comparison, without adding it to details.
    let snap_total: Option<i64> = storage
        .conn()
        .query_row("SELECT total_tokens FROM source_aggregates", [], |r| {
            r.get(0)
        })
        .optional()
        .unwrap();
    assert_eq!(snap_total, Some(25_689));
    let _ = dir;
}

#[test]
fn pause_after_a_transient_failure_does_not_retry_or_advance_the_cursor() {
    use llm_usage_core::adapters::framework::*;
    use llm_usage_core::error::CoreError;
    use std::{
        path::Path,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    struct Pausing<'a> {
        inner: CodexAdapter,
        allowed: &'a AtomicBool,
        attempts: &'a AtomicUsize,
    }
    impl SourceAdapter for Pausing<'_> {
        fn adapter_id(&self) -> &'static str {
            self.inner.adapter_id()
        }
        fn agent(&self) -> &'static str {
            self.inner.agent()
        }
        fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
            self.inner.discover(ctx)
        }
        fn instance_id(&self, root: &DiscoveredRoot) -> String {
            self.inner.instance_id(root)
        }
        fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
            self.inner.detect(path)
        }
        fn capability(&self) -> CapabilityTable {
            self.inner.capability()
        }
        fn scan(
            &self,
            _target: &ScanTarget,
            _stored: &StoredScanState,
            _limits: &ScanLimits,
            _now: i64,
        ) -> Result<ScanOutcome, CoreError> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            self.allowed.store(false, Ordering::SeqCst);
            Err(std::io::Error::from(std::io::ErrorKind::WouldBlock).into())
        }
    }
    let (_source, root) = setup_fixture("codex-pause-retry", "rollout-single-call.sanitized.json");
    let (_db, storage) = temp_storage("codex-pause-retry");
    let allowed = AtomicBool::new(true);
    let attempts = AtomicUsize::new(0);
    let adapter = Pausing {
        inner: CodexAdapter::new(),
        allowed: &allowed,
        attempts: &attempts,
    };
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.clone()],
    };
    let config = RunConfig {
        timezone: "UTC".into(),
        now_ms: 1_800_000_000_000,
        limits: ScanLimits::default(),
        trigger: llm_usage_core::jobs::TriggerKind::Interval,
        origin_host_id: None,
        run_id_prefix: "paused-retry".into(),
    };
    let started = std::time::Instant::now();
    let reports = run_adapter_scan_filtered_controlled(
        &storage,
        &adapter,
        &ctx,
        &config,
        &InstanceFilter::default(),
        &RunControl {
            deadline: None,
            allowed: &|| allowed.load(Ordering::SeqCst),
        },
    )
    .unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "pause must not wait for the five-second retry delay"
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        reports[0].finish,
        llm_usage_core::jobs::RunStatus::Interrupted
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        run_codex(&storage, &root, 1_800_000_001_000)[0]
            .outcome
            .as_ref()
            .unwrap()
            .added,
        1
    );
}

#[test]
fn calls_49_full_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("codex-49", "rollout-49calls.sanitized.json");
    let (_db, storage) = temp_storage("codex-49");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 49);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 49);
    assert_eq!(summary.totals.input_total_known, Some(4_184_537));
    assert_eq!(summary.totals.cache_read_known, Some(4_022_016));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(4_051));
    assert_eq!(summary.totals.total_tokens_known, Some(4_188_588));
    // Cached-input ratio: 4,022,016 / 4,184,537 is about 96.1% for auto-review.
    let ratio = summary.totals.cache_input_ratio().unwrap();
    assert!((ratio.as_f64() - 0.961).abs() < 0.001);
    assert_eq!(summary.totals.ratio_sample_count(), 49);
    // One rollout identifies one session.
    assert_eq!(summary.periods.len(), 1);
    assert_eq!(summary.periods[0].distinct_sessions, Some(1));

    // Reasoning is a subset of output and is not added again; verify it in usage_events.
    let reasoning: i64 = storage
        .conn()
        .query_row("SELECT SUM(output_reasoning) FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(reasoning, 725);

    let rec = &report.reconciliations[0];
    assert_eq!(rec.verdict, "matched");
    assert_eq!(rec.snapshot_final, Some(4_188_588));
    let _ = dir;
}

#[test]
fn multimodel_compaction_pipeline_matches_expectations() {
    let (dir, root) = setup_fixture("codex-mm", "rollout-multimodel-full.sanitized.json");
    let (_db, storage) = temp_storage("codex-mm");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(
        report.files[0].events, 221,
        "token_usage_record 计数即调用数"
    );

    let summary = summary(&storage, "2026-09-23", "2026-09-24");
    assert_eq!(summary.totals.call_count, 221);
    assert_eq!(summary.totals.input_total_known, Some(32_061_426));
    assert_eq!(summary.totals.cache_read_known, Some(31_055_872));
    assert_eq!(summary.totals.output_total_known, Some(143_381));
    assert_eq!(summary.totals.total_tokens_known, Some(32_204_807));

    // turn_context positions assign the two model segments; manual totals are above.
    let breakdown: std::collections::BTreeMap<String, (i64, i64, i64)> = summary
        .model_breakdown
        .iter()
        .map(|row| {
            (
                row.model_raw.clone().unwrap_or_default(),
                (
                    row.sums.call_count,
                    row.sums.input_total_known.unwrap_or(0),
                    row.sums.total_tokens_known.unwrap_or(0),
                ),
            )
        })
        .collect();
    assert_eq!(breakdown["gpt-6-astra"], (169, 23_683_841, 23_793_433));
    assert_eq!(breakdown["gpt-6-sol"], (52, 8_377_585, 8_411_374));
    assert_eq!(
        breakdown.len(),
        2,
        "no unknown-model row: turn_context covers all calls"
    );

    // Compaction resets snapshots: per-call sum = final snapshot + carried records (480,472).
    let rec = &report.reconciliations[0];
    assert_eq!(rec.verdict, "matched");
    assert_eq!(rec.detail_sum, 32_204_807);
    assert_eq!(rec.snapshot_final, Some(31_724_335));
    assert_eq!(rec.carried_sum, 480_472);
    assert_eq!(rec.difference, Some(0));

    // The snapshot aggregate keeps the final comparison value, separate from per-call totals.
    let snap_total: i64 = storage
        .conn()
        .query_row("SELECT total_tokens FROM source_aggregates", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(snap_total, 31_724_335);

    // No parent_thread_id identifies a primary session.
    let categories: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE call_category = 'primary'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(categories, 221);
    let _ = dir;
}

#[test]
fn capability_table_is_structured_and_complete() {
    let adapter = CodexAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "codex");
    // supported_versions comes from the version registry: M2-D per-version samples plus
    // 0.139-0.151 older formats checked and registered on 2026-09-26.
    assert_eq!(
        json["supported_versions"],
        serde_json::json!([
            "0.162.0-alpha.2",
            "0.155.0-alpha.16",
            "0.153.4",
            "0.151.0-alpha.7.2",
            "0.150.0-alpha.12.2",
            "0.150.0-alpha.8",
            "0.149.0-alpha.4",
            "0.148.0-alpha.21",
            "0.155.0-alpha.16.3",
            "0.154.0-alpha.6.2",
            "0.154.0-alpha.6.1",
            "0.153.0",
            "0.151.0-alpha.7.1",
            "0.149.0-alpha.4.1",
            "0.148.0-alpha.9",
            "0.147.0-alpha.6.5",
            "0.146.0-alpha.9.2",
            "0.146.0-alpha.3.1",
            "0.146.0-alpha.3",
            "0.145.0-alpha.27",
            "0.145.0-alpha.18",
            "0.144.5",
            "0.144.2",
            "0.144.0-alpha.4",
            "0.142.5",
            "0.142.4",
            "0.142.3",
            "0.142.2",
            "0.142.0",
            "0.142.0-alpha.6",
            "0.142.0-alpha.1",
            "0.140.0-alpha.2",
            "0.139.0"
        ])
    );
    // All eight field-capability entries must exist.
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
    assert_eq!(
        json["fields"]["cost"]["availability"],
        serde_json::json!({"unavailable": "本地无费用字段；远端账单/账号不接入"})
    );
    assert_eq!(json["fields"]["tokens"]["availability"], "available");
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
    // Persist structured capabilities in source_instances.capabilities.
    let (_db, storage) = temp_storage("codex-cap");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "codex@test".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: json.clone(),
            health: "ok".to_string(),
        },
        1_800_000_000_000,
    )
    .unwrap();
    let stored: String = storage
        .conn()
        .query_row(
            "SELECT capabilities FROM source_instances WHERE instance_id = 'codex@test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["detection"]["fail_closed"], true);
}

#[test]
fn source_file_row_composite_roundtrip() {
    let (_db, storage) = temp_storage("codex-filerow");
    llm_usage_core::adapters::framework::upsert_source_instance(
        &storage,
        &llm_usage_core::adapters::framework::SourceInstanceInput {
            origin_host_id: None,
            instance_id: "codex@test".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: llm_usage_core::domain::LocalityBasis::LocalFilesystem,
            attribution_status: llm_usage_core::domain::AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({}),
            health: "ok".to_string(),
        },
        1,
    )
    .unwrap();
    let row = SourceFileRow {
        file_id: "p".to_string(),
        file_identity: "id-1".to_string(),
        generation: 3,
        len: 1234,
        mtime_ms: 55,
        created_ms: Some(77),
        head_hash: 0xABCD,
        head_len: 1234,
        tail_hash: 0xEF01,
        status: "active".to_string(),
        format_status: None,
    };
    llm_usage_core::adapters::framework::upsert_source_file(&storage, "codex@test", &row, 1)
        .unwrap();
    let loaded = llm_usage_core::adapters::framework::load_source_file(&storage, "codex@test", "p")
        .unwrap()
        .unwrap();
    assert_eq!(loaded, row);
    // Lookup by file identity returns the existing row and its stored path.
    let by_identity = llm_usage_core::adapters::framework::find_source_file_by_identity(
        &storage,
        "codex@test",
        "id-1",
    )
    .unwrap()
    .unwrap();
    assert_eq!(by_identity.file_id, "p");
}
