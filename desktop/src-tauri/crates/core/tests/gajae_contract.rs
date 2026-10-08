//! Native gajae-code v5 data and explicit default-zero rule upgrades.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, gajae_code::GajaeCodeAdapter};
use llm_usage_core::domain::{EventInput, FieldQuality as Q, TimeBasis};
use llm_usage_core::{error::CoreError, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
const FIXTURE: &str = include_str!("fixtures/gajae-code/real-0.18.7/session.jsonl");
const NOW: i64 = 1_800_000_000_000;

fn carrier(dir: &TempDir, legacy_layout: bool) -> (DiscoverContext, PathBuf) {
    let root = dir.path().join("gjc-agent");
    let sessions = root.join("sessions/isolated-scope");
    std::fs::create_dir_all(&sessions).unwrap();
    let path = sessions.join("real.jsonl");
    // The old parser rejected the new non-usage config row. This comparison
    // extract omits that row before the initial scan and retains native usage.
    let text = if legacy_layout {
        FIXTURE
            .lines()
            .filter(|l| !l.contains("configured_model_chain"))
            .map(|l| format!("{l}\n"))
            .collect()
    } else {
        FIXTURE.to_string()
    };
    std::fs::write(&path, text).unwrap();
    let mut context = DiscoverContext::default();
    context
        .env
        .insert("GJC_CODING_AGENT_DIR".into(), root.to_string_lossy().into());
    (context, path)
}
fn config(name: &str) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: name.into(),
        origin_host_id: None,
    }
}
fn check(storage: &Storage) {
    let values: [Option<i64>; 7] = storage.conn().query_row(
        "SELECT input_total,output_total,total_tokens,input_cache_read,input_cache_write,input_uncached,output_reasoning FROM usage_events", [],
        |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?])
    ).unwrap();
    assert_eq!(
        values,
        [Some(412), Some(2), Some(414), None, None, None, None]
    );
    let row: (i64, String, String, Option<i64>) = storage
        .conn()
        .query_row(
            "SELECT COUNT(*),parser_version,time_basis,source_revision FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (1, "gjc-session-2".into(), "source_start".into(), None)
    );
    let totals = summary(storage, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            totals.call_count,
            totals.input_total_known,
            totals.output_total_known,
            totals.total_tokens_known
        ),
        (1, Some(412), Some(2), Some(414))
    );
}
#[test]
fn real_api_cli_and_v5_session_keep_config_rows_and_unknown_cache() {
    let api: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/gajae-code/real-0.18.7/api-usage.json"
    ))
    .unwrap();
    let cli: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/gajae-code/real-0.18.7/cli-usage.json"
    ))
    .unwrap();
    assert_eq!(api["usage"]["prompt_tokens"], cli["usage"]["input"]);
    assert_eq!(api["usage"]["completion_tokens"], cli["usage"]["output"]);
    assert_eq!(api["usage"]["total_tokens"], cli["usage"]["totalTokens"]);
    assert_eq!(cli["usage"]["cacheWrite"], 0); // Native default, without confirmation from API usage.
    assert!(api["usage"]["prompt_tokens_details"]
        .get("cache_write_tokens")
        .is_none());
    let dir = TempDir::new("gjc-real");
    let (ctx, path) = carrier(&dir, false);
    let original = std::fs::read(&path).unwrap();
    let (_db, storage) = temp_storage("gjc-real");
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config(&format!("gjc-real-{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() == "gajae-code" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        check(&storage);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    let metadata: (String, String, String, String) = storage
        .conn()
        .query_row(
            "SELECT provider_id,model_raw,schema_version,cost_kind FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        metadata,
        (
            "local-model".into(),
            "qwen2.5-0.5b-local".into(),
            "gjc-session-doc-1".into(),
            "estimated".into()
        )
    );
}

struct Legacy {
    variant: &'static str,
    events: Mutex<Vec<EventInput>>,
}
impl Legacy {
    fn new(variant: &'static str) -> Self {
        Self {
            variant,
            events: Mutex::new(Vec::new()),
        }
    }
}
impl SourceAdapter for Legacy {
    fn adapter_id(&self) -> &'static str {
        "gajae-code"
    }
    fn agent(&self) -> &'static str {
        "gajae-code"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        GajaeCodeAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        GajaeCodeAdapter::new().instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        GajaeCodeAdapter::new().detect(path)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut result = GajaeCodeAdapter::new().scan(target, stored, limits, now)?;
        let raw: serde_json::Value = std::fs::read_to_string(&target.path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["message"]["role"] == "assistant")
            .unwrap();
        let u = &raw["message"]["usage"];
        let mapped = llm_usage_core::adapters::usage_map::map_pi_family(
            &llm_usage_core::adapters::usage_map::PiFamilyUsage {
                input: u["input"].as_i64().unwrap(),
                output: u["output"].as_i64().unwrap(),
                cache_read: u["cacheRead"].as_i64().unwrap(),
                cache_write: u["cacheWrite"].as_i64().unwrap(),
                total_tokens: u["totalTokens"].as_i64().unwrap(),
                reasoning: u["reasoningTokens"].as_i64(),
            },
        );
        for e in &mut result.events {
            e.parser_version = "gjc-session-1".into();
            e.usage = mapped.usage.clone();
            e.quality = mapped.quality.clone();
            e.time_basis = TimeBasis::SourceCompletion;
            match self.variant {
                "input" => e.usage.input_total = Some(411),
                "quality" => e.quality.output_total = Q::Derived,
                "model" => e.model_raw = Some("different-model".into()),
                "cost" => e.cost.as_mut().unwrap().amount_minor = 1,
                "revision" => e.source_revision = Some(1),
                _ => {}
            }
        }
        *self.events.lock().unwrap() = result.events.clone();
        Ok(result)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = GajaeCodeAdapter::new().capability();
        c.maintenance["parser_version"] = "gjc-session-1".into();
        c
    }
}

#[test]
fn unchanged_old_cursor_full_digests_parallel_and_checkpoint_rollback() {
    for legacy_hash in [false, true] {
        for fail_once in [false, true] {
            let dir = TempDir::new("gjc-old");
            let (ctx, path) = carrier(&dir, true);
            let original = std::fs::read(&path).unwrap();
            let (_db, storage) = temp_storage("gjc-old");
            let old_adapter = Legacy::new("zero");
            run_adapter_scan(&storage, &old_adapter, &ctx, &config("old")).unwrap();
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            if legacy_hash {
                let old = old_adapter.events.lock().unwrap()[0].clone();
                storage
                    .conn()
                    .execute(
                        "UPDATE usage_events SET content_hash=?1",
                        [llm_usage_core::identity::content_hash(&old)],
                    )
                    .unwrap();
            }
            storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','prior audit',0 FROM source_instances", []).unwrap();
            if fail_once {
                storage.conn().execute_batch("CREATE TRIGGER fail_gjc_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
                let reports =
                    run_adapter_scan(&storage, &GajaeCodeAdapter::new(), &ctx, &config("failed"))
                        .unwrap();
                assert_eq!(reports[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(
                    storage
                        .conn()
                        .query_row("SELECT input_cache_write FROM usage_events", [], |r| r
                            .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
                assert_eq!(
                    storage
                        .conn()
                        .query_row(
                            "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'",
                            [],
                            |r| r.get::<_, i64>(0)
                        )
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
                storage
                    .conn()
                    .execute_batch("DROP TRIGGER fail_gjc_checkpoint")
                    .unwrap();
            }
            let adapter = GajaeCodeAdapter::new();
            let requests = [ParallelScanRequest {
                adapter: &adapter,
                context: ctx.clone(),
                config: config("upgraded"),
                filter: InstanceFilter::default(),
            }];
            let locked = Mutex::new(storage);
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
            run_adapter_scan(&storage, &adapter, &ctx, &config("repeat")).unwrap();
            check(&storage);
            for code in ["parser_policy_updated", "preserved_history"] {
                assert_eq!(
                    storage
                        .conn()
                        .query_row(
                            "SELECT COUNT(*) FROM diagnostics WHERE code=?1",
                            [code],
                            |r| r.get::<_, i64>(0)
                        )
                        .unwrap(),
                    1
                );
            }
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }
}

#[test]
fn full_digest_rejects_other_content_and_preserves_real_same_batch_conflict() {
    for variant in [
        "input",
        "quality",
        "model",
        "cost",
        "revision",
        "same-batch",
    ] {
        let dir = TempDir::new("gjc-conflict");
        let (ctx, _path) = carrier(&dir, true);
        let (_db, storage) = temp_storage("gjc-conflict");
        let legacy = Legacy::new(variant);
        run_adapter_scan(&storage, &legacy, &ctx, &config("old")).unwrap();
        let old = legacy.events.lock().unwrap()[0].clone();
        let before: String = storage
            .conn()
            .query_row("SELECT content_hash FROM usage_events", [], |r| r.get(0))
            .unwrap();
        if variant == "same-batch" {
            let mut correct = old.clone();
            correct.parser_version = "gjc-session-2".into();
            correct.usage.input_cache_read = None;
            correct.quality.input_cache_read = Q::Unknown;
            correct.usage.input_cache_write = None;
            correct.quality.input_cache_write = Q::Unknown;
            correct.usage.input_uncached = None;
            correct.quality.input_uncached = Q::Unknown;
            correct.time_basis = TimeBasis::SourceStart;
            let mut bad = correct.clone();
            bad.model_raw = Some("changed-in-batch".into());
            let batch = common::batch(&old.source_instance_id, "UTC", NOW + 1, vec![bad, correct]);
            let result = llm_usage_core::ingest::commit_batch(&storage, &batch, None).unwrap();
            assert_eq!((result.updated, result.conflicts), (1, 1));
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT conflict FROM usage_events", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                summary(&storage, "2026-10-06", "2026-10-06")
                    .totals
                    .conflict_count,
                1
            );
            check(&storage);
        } else {
            run_adapter_scan(&storage, &GajaeCodeAdapter::new(), &ctx, &config("upgrade")).unwrap();
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT content_hash FROM usage_events", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                before
            );
            assert_eq!(
                storage
                    .conn()
                    .query_row(
                        "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                0
            );
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT conflict FROM usage_events", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                1
            );
        }
    }
}

#[test]
fn unknown_zero_fallback_counts_call_and_unknown_entry_still_holds_cursor() {
    for unknown_entry in [false, true] {
        let dir = TempDir::new("gjc-unknown");
        let (ctx, path) = carrier(&dir, false);
        let mut rows: Vec<serde_json::Value> = FIXTURE
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let usage = &mut rows.last_mut().unwrap()["message"]["usage"];
        for key in ["input", "output", "cacheRead", "cacheWrite", "totalTokens"] {
            usage[key] = 0.into();
        }
        if unknown_entry {
            rows.push(serde_json::json!({"type":"unverified_future_usage","id":"00000010"}));
        }
        std::fs::write(
            &path,
            rows.into_iter()
                .map(|v| format!("{v}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let (_db, storage) = temp_storage("gjc-unknown");
        run_adapter_scan(&storage, &GajaeCodeAdapter::new(), &ctx, &config("unknown")).unwrap();
        let count: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, i64::from(!unknown_entry));
        if !unknown_entry {
            let quality: String = storage
                .conn()
                .query_row("SELECT quality_bucket FROM usage_events", [], |r| r.get(0))
                .unwrap();
            assert_eq!(quality, "unknown");
            let totals = summary(&storage, "2026-10-06", "2026-10-06").totals;
            assert_eq!(
                (
                    totals.call_count,
                    totals.input_total_known,
                    totals.output_total_known,
                    totals.total_tokens_known
                ),
                (1, None, None, None)
            );
        } else {
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }
}

#[test]
fn consumed_old_all_zero_defaults_upgrade_to_unknown_without_changing_call_count() {
    let dir = TempDir::new("gjc-old-zero");
    let (ctx, path) = carrier(&dir, true);
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if v["message"]["role"] == "assistant" {
                for key in ["input", "output", "cacheRead", "cacheWrite", "totalTokens"] {
                    v["message"]["usage"][key] = 0.into();
                }
            }
            v
        })
        .collect();
    std::fs::write(
        &path,
        rows.into_iter()
            .map(|v| format!("{v}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let (_db, storage) = temp_storage("gjc-old-zero");
    run_adapter_scan(&storage, &Legacy::new("zero"), &ctx, &config("old-zero")).unwrap();
    assert_eq!(
        storage
            .conn()
            .query_row("SELECT total_tokens FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    run_adapter_scan(
        &storage,
        &GajaeCodeAdapter::new(),
        &ctx,
        &config("new-zero"),
    )
    .unwrap();
    let totals = summary(&storage, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            totals.call_count,
            totals.input_total_known,
            totals.output_total_known,
            totals.total_tokens_known
        ),
        (1, None, None, None)
    );
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}
