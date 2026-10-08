//! Official Junie failed-task usage and migration comparing complete old event summaries.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, junie::JunieAdapter};
use llm_usage_core::domain::{CostAmount, CostKind, EventInput, FieldQuality as Q};
use llm_usage_core::{error::CoreError, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
const FIXTURE: &str = include_str!("fixtures/junie/real-3419.29/events.jsonl");
const NOW: i64 = 1_800_000_000_000;

fn carrier(dir: &TempDir, text: &str) -> (DiscoverContext, PathBuf) {
    let root = dir.path().join("isolated-junie");
    let session = root.join("sessions/session-261006-080403-anonymous");
    std::fs::create_dir_all(&session).unwrap();
    let path = session.join("events.jsonl");
    std::fs::write(&path, text).unwrap();
    let mut context = DiscoverContext::default();
    context
        .env
        .insert("JUNIE_HOME".into(), root.to_string_lossy().into());
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
fn raw_usage(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter_map(|v| {
            v.pointer("/event/agentEvent/modelUsage")
                .and_then(|x| x.as_array())
                .cloned()
        })
        .flatten()
        .collect()
}
fn check(
    storage: &Storage,
    expected: i64,
    input: Option<i64>,
    output: Option<i64>,
    cache: Option<i64>,
) {
    let count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, expected);
    let values: [Option<i64>; 4] = storage.conn().query_row(
        "SELECT SUM(input_uncached),SUM(output_total),SUM(input_cache_read),SUM(input_total) FROM usage_events", [],
        |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?])).unwrap();
    assert_eq!(values, [input, output, cache, None]);
    let bad: i64 = storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE input_cache_write IS NOT NULL OR total_tokens IS NOT NULL OR source_total IS NOT NULL OR output_reasoning IS NOT NULL OR cost_amount_minor IS NOT NULL OR duration_ms IS NOT NULL OR interval_start_ms IS NOT NULL OR parser_version!='junie-events-doc2'", [], |r| r.get(0)).unwrap();
    assert_eq!(bad, 0);
    let totals = summary(storage, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            totals.call_count,
            totals.input_total_known,
            totals.output_total_known,
            totals.total_tokens_known
        ),
        (expected, None, output, None)
    );
}
#[test]
fn real_failed_task_api_cli_and_all_native_rows_keep_disjoint_positive_buckets() {
    assert_eq!(FIXTURE.lines().count(), 43);
    let native = raw_usage(FIXTURE);
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/junie/real-3419.29/api-usage.json")).unwrap();
    let cli: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/junie/real-3419.29/cli-usage.json")).unwrap();
    assert_eq!(api.as_array().unwrap().len(), 7);
    assert_eq!(cli["errorCode"][0]["calls"], 7);
    for (a, u) in api.as_array().unwrap().iter().zip(&native) {
        assert_eq!(
            u["inputTokens"].as_i64().unwrap(),
            a["usage"]["prompt_tokens"].as_i64().unwrap()
                - a["usage"]["prompt_tokens_details"]["cached_tokens"]
                    .as_i64()
                    .unwrap()
        );
        assert_eq!(
            u["cacheInputTokens"],
            a["usage"]["prompt_tokens_details"]["cached_tokens"]
        );
        assert_eq!(u["outputTokens"], a["usage"]["completion_tokens"]);
        assert!(a["usage"]["prompt_tokens_details"]
            .get("cache_write_tokens")
            .is_none());
    }
    for (key, value) in [
        ("inputTokens", 10972),
        ("cacheInputTokens", 53102),
        ("outputTokens", 98),
    ] {
        assert_eq!(
            native.iter().map(|u| u[key].as_i64().unwrap()).sum::<i64>(),
            value
        );
        assert_eq!(cli["errorCode"][0][key], value);
    }
    assert!(FIXTURE.contains("AgentFailureEvent"));
    let dir = TempDir::new("junie-real");
    let (ctx, path) = carrier(&dir, FIXTURE);
    let original = std::fs::read(&path).unwrap();
    let (_db, storage) = temp_storage("junie-real");
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config(&format!("real-{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() == "junie" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        check(&storage, 7, Some(10972), Some(98), Some(53102));
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    let missing_provider: i64 = storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE provider_id IS NULL AND source_revision IS NULL AND model_raw='qwen2.5-0.5b-local'", [], |r|r.get(0)).unwrap();
    assert_eq!(missing_provider, 7);
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
        "junie"
    }
    fn agent(&self) -> &'static str {
        "junie"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        JunieAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        JunieAdapter::new().instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        JunieAdapter::new().detect(path)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut result = JunieAdapter::new().scan(target, stored, limits, now)?;
        let native = raw_usage(&std::fs::read_to_string(&target.path).unwrap());
        for (e, u) in result.events.iter_mut().zip(native) {
            e.parser_version = "junie-events-doc1".into();
            e.usage = llm_usage_core::domain::TokenUsage {
                input_uncached: None,
                input_total: u["inputTokens"].as_i64(),
                input_cache_read: u["cacheInputTokens"].as_i64(),
                input_cache_write: u["cacheCreateTokens"].as_i64(),
                output_total: u["outputTokens"].as_i64(),
                output_reasoning: u["reasoningTokens"].as_i64(),
                total_tokens: None,
                source_total: None,
            };
            e.quality = llm_usage_core::domain::TokenQuality {
                input_total: if e.usage.input_total.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                input_cache_read: if e.usage.input_cache_read.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                input_cache_write: if e.usage.input_cache_write.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                output_total: if e.usage.output_total.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                output_reasoning: if e.usage.output_reasoning.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                ..Default::default()
            };
            e.cost = u["cost"].as_f64().map(|v| CostAmount {
                amount_minor: (v * 1_000_000.).round() as i64,
                currency: "USD".into(),
                kind: CostKind::Reported,
                price_version: None,
                billing_scope: None,
            });
            e.duration_ms = u["time"].as_i64();
            e.interval_start_ms = e
                .duration_ms
                .and_then(|d| e.occurred_at_ms.checked_sub(d))
                .filter(|v| *v >= 0);
            match self.variant {
                "input" => e.usage.input_total = e.usage.input_total.map(|v| v + 1),
                "quality" => e.quality.output_total = Q::Derived,
                "model" => e.model_raw = Some("different-model".into()),
                "cost" => e.cost.as_mut().unwrap().amount_minor = 1,
                "revision" => e.source_revision = Some(1),
                "time" => e.occurred_at_ms += 1,
                "duration" => {
                    e.duration_ms = Some(1);
                    e.interval_start_ms = Some(e.occurred_at_ms - 1);
                }
                _ => {}
            }
        }
        *self.events.lock().unwrap() = result.events.clone();
        Ok(result)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = JunieAdapter::new().capability();
        c.maintenance["parser_version"] = "junie-events-doc1".into();
        c
    }
}
#[test]
fn consumed_unchanged_cursor_full_and_legacy_hashes_parallel_and_atomic_rollback() {
    for legacy_hash in [false, true] {
        for fail_once in [false, true] {
            let dir = TempDir::new("junie-old");
            let (ctx, path) = carrier(&dir, FIXTURE);
            let original = std::fs::read(&path).unwrap();
            let (_db, storage) = temp_storage("junie-old");
            let old = Legacy::new("default");
            run_adapter_scan(&storage, &old, &ctx, &config("old")).unwrap();
            if legacy_hash {
                for e in old.events.lock().unwrap().iter() {
                    storage
                        .conn()
                        .execute(
                            "UPDATE usage_events SET content_hash=?1 WHERE source_record_key=?2",
                            rusqlite::params![
                                llm_usage_core::identity::content_hash(e),
                                e.source_record_key
                            ],
                        )
                        .unwrap();
                }
            }
            storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','prior audit',0 FROM source_instances", []).unwrap();
            if fail_once {
                storage.conn().execute_batch("CREATE TRIGGER fail_junie_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
                let reports =
                    run_adapter_scan(&storage, &JunieAdapter::new(), &ctx, &config("failed"))
                        .unwrap();
                assert_eq!(reports[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE parser_version='junie-events-doc1' AND input_uncached IS NULL AND input_cache_write=0 AND cost_amount_minor=0 AND duration_ms=0",[],|r|r.get::<_,i64>(0)).unwrap(),7);
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
                    .execute_batch("DROP TRIGGER fail_junie_checkpoint")
                    .unwrap();
            }
            let adapter = JunieAdapter::new();
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
            check(&storage, 7, Some(10972), Some(98), Some(53102));
            run_adapter_scan(&storage, &adapter, &ctx, &config("repeat")).unwrap();
            check(&storage, 7, Some(10972), Some(98), Some(53102));
            for (code, count) in [("parser_policy_updated", 7), ("preserved_history", 1)] {
                assert_eq!(
                    storage
                        .conn()
                        .query_row(
                            "SELECT COUNT(*) FROM diagnostics WHERE code=?1",
                            [code],
                            |r| r.get::<_, i64>(0)
                        )
                        .unwrap(),
                    count
                );
            }
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }
}
#[test]
fn full_digest_protects_other_content_and_same_batch_conflicts() {
    for variant in [
        "input",
        "quality",
        "model",
        "cost",
        "revision",
        "time",
        "duration",
        "same-batch",
    ] {
        let dir = TempDir::new("junie-conflict");
        let (ctx, _path) = carrier(&dir, FIXTURE);
        let (_db, storage) = temp_storage("junie-conflict");
        let old = Legacy::new(variant);
        run_adapter_scan(&storage, &old, &ctx, &config("old")).unwrap();
        let prior = old.events.lock().unwrap()[0].clone();
        if variant == "same-batch" {
            let adapter = JunieAdapter::new();
            let root = adapter.discover(&ctx).remove(0);
            let target = ScanTarget {
                instance_id: adapter.instance_id(&root),
                file_id: "native".into(),
                path: root.files[0].clone(),
                file_identity: "native".into(),
                probe: llm_usage_core::adapters::jsonl::probe_file(&root.files[0]).unwrap(),
                generation: 0,
                rescan: true,
            };
            let correct = adapter
                .scan(
                    &target,
                    &StoredScanState::default(),
                    &ScanLimits::default(),
                    NOW + 1,
                )
                .unwrap()
                .events
                .remove(0);
            let mut bad = correct.clone();
            bad.model_raw = Some("changed-in-batch".into());
            let result = llm_usage_core::ingest::commit_batch(
                &storage,
                &common::batch(
                    &prior.source_instance_id,
                    "UTC",
                    NOW + 1,
                    vec![bad, correct],
                ),
                None,
            )
            .unwrap();
            assert_eq!((result.updated, result.conflicts), (1, 1));
        } else {
            run_adapter_scan(&storage, &JunieAdapter::new(), &ctx, &config("upgrade")).unwrap();
        }
        let (parser, conflict): (String, i64) = storage
            .conn()
            .query_row(
                "SELECT parser_version,conflict FROM usage_events WHERE source_record_key=?1",
                [&prior.source_record_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(conflict, 1);
        assert_eq!(
            parser,
            if variant == "same-batch" {
                "junie-events-doc2"
            } else {
                "junie-events-doc1"
            }
        );
        assert_eq!(
            summary(&storage, "2026-10-06", "2026-10-06")
                .totals
                .conflict_count,
            if variant == "same-batch" { 1 } else { 7 }
        );
    }
}
fn modify_usage(edit: impl Fn(&mut serde_json::Value)) -> String {
    FIXTURE
        .lines()
        .map(|line| {
            let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
            if let Some(array) = v
                .pointer_mut("/event/agentEvent/modelUsage")
                .and_then(|x| x.as_array_mut())
            {
                for u in array {
                    edit(u);
                }
            }
            format!("{v}\n")
        })
        .collect()
}
#[test]
fn all_zero_defaults_missing_fields_and_old_cursor_keep_calls_unknown() {
    for missing in [false, true] {
        let text = modify_usage(|u| {
            for key in [
                "inputTokens",
                "outputTokens",
                "cacheInputTokens",
                "cacheCreateTokens",
            ] {
                if missing {
                    u.as_object_mut().unwrap().remove(key);
                } else {
                    u[key] = 0.into();
                }
            }
        });
        let dir = TempDir::new("junie-zero");
        let (ctx, path) = carrier(&dir, &text);
        let (_db, storage) = temp_storage("junie-zero");
        run_adapter_scan(&storage, &Legacy::new("default"), &ctx, &config("old")).unwrap();
        run_adapter_scan(&storage, &JunieAdapter::new(), &ctx, &config("new")).unwrap();
        check(&storage, 7, None, None, None);
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM usage_events WHERE quality_bucket='unknown'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            7
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }
}
#[test]
fn bad_entry_does_not_block_other_valid_calls_and_positive_cost_is_estimated() {
    let mut changed = false;
    let text = FIXTURE
        .lines()
        .map(|line| {
            let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
            if let Some(array) = v
                .pointer_mut("/event/agentEvent/modelUsage")
                .and_then(|x| x.as_array_mut())
            {
                if !changed {
                    array[0]["inputTokens"] = "bad".into();
                    changed = true;
                }
            }
            format!("{v}\n")
        })
        .collect::<String>();
    let dir = TempDir::new("junie-bad");
    let (ctx, _) = carrier(&dir, &text);
    let (_db, storage) = temp_storage("junie-bad");
    run_adapter_scan(&storage, &JunieAdapter::new(), &ctx, &config("bad")).unwrap();
    check(&storage, 6, Some(10902), Some(96), Some(53102));
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code='token_shape_deviation'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let text = modify_usage(|u| {
        u["cost"] = 0.001.into();
        u["time"] = 1500.into();
    });
    let dir = TempDir::new("junie-fee");
    let (ctx, _) = carrier(&dir, &text);
    let (_db, storage) = temp_storage("junie-fee");
    run_adapter_scan(&storage, &Legacy::new("default"), &ctx, &config("old")).unwrap();
    run_adapter_scan(&storage, &JunieAdapter::new(), &ctx, &config("new")).unwrap();
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE cost_amount_minor=1000 AND cost_kind='estimated' AND duration_ms=1500 AND interval_start_ms=occurred_at_ms-1500",[],|r|r.get::<_,i64>(0)).unwrap(),7);
}
