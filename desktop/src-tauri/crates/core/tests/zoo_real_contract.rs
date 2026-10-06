//! Zoo 3.86.0 real native/API coverage and historical doc1 digest migration.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, zoo::ZooAdapter};
use llm_usage_core::domain::{CostAmount, CostKind, EventInput, FieldQuality as Q};
use llm_usage_core::{error::CoreError, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
const DEFAULT: &str = include_str!("fixtures/zoo/real-3.86.0/ui_messages.json");
const NOW: i64 = 1_800_000_000_000;

fn carrier(dir: &TempDir, text: &str) -> (DiscoverContext, PathBuf) {
    let home = dir.path().join("home");
    let file = home.join(".config/Code/User/globalStorage/zoocodeorganization.zoo-code/tasks/anonymous-task/ui_messages.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, text).unwrap();
    let mut context = DiscoverContext::default();
    context.env.insert(
        "APPDATA".into(),
        home.join(".config").to_string_lossy().into(),
    );
    let linux_context = DiscoverContext {
        home_dir: Some(home),
        ..Default::default()
    };
    assert_eq!(
        ZooAdapter::new().discover(&context)[0].files,
        ZooAdapter::new().discover(&linux_context)[0].files
    );
    (context, file)
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
    serde_json::from_str::<serde_json::Value>(text)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["say"] == "api_req_started")
        .map(|r| serde_json::from_str(r["text"].as_str().unwrap()).unwrap())
        .collect()
}
fn count(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn check(
    storage: &Storage,
    calls: i64,
    input: Option<i64>,
    output: Option<i64>,
    total: Option<i64>,
) {
    assert_eq!(count(storage, "SELECT COUNT(*) FROM usage_events"), calls);
    let values: [Option<i64>; 3] = storage
        .conn()
        .query_row(
            "SELECT SUM(input_total),SUM(output_total),SUM(total_tokens) FROM usage_events",
            [],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?]),
        )
        .unwrap();
    assert_eq!(values, [input, output, total]);
    assert_eq!(count(storage,"SELECT COUNT(*) FROM usage_events WHERE input_uncached IS NOT NULL OR input_cache_read IS NOT NULL OR input_cache_write IS NOT NULL OR output_reasoning IS NOT NULL OR source_total IS NOT NULL OR cost_amount_minor IS NOT NULL OR model_raw IS NOT NULL OR provider_id IS NOT NULL OR parser_version!='zoo-ui-messages-doc2'"),0);
    let totals = summary(storage, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            totals.call_count,
            totals.input_total_known,
            totals.output_total_known,
            totals.total_tokens_known
        ),
        (calls, input, output, total)
    );
}

#[test]
fn official_extension_api_native_and_service_match_without_inventing_zero_buckets() {
    let api: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/zoo/real-3.86.0/api-usage.json")).unwrap();
    let ext: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/zoo/real-3.86.0/extension-usage.json"
    ))
    .unwrap();
    let native = raw_usage(DEFAULT);
    assert_eq!((native.len(), api.len()), (1, 1));
    assert_eq!(native[0]["tokensIn"], api[0]["usage"]["prompt_tokens"]);
    assert_eq!(native[0]["tokensOut"], api[0]["usage"]["completion_tokens"]);
    assert_eq!(ext["usage"]["totalTokensIn"], 6118);
    assert_eq!(ext["usage"]["totalTokensOut"], 53);
    assert_eq!(ext["cancelled"], true);
    assert_eq!(ext["tools"], serde_json::json!({}));
    let dir = TempDir::new("zoo-real");
    let (ctx, path) = carrier(&dir, DEFAULT);
    let original = std::fs::read(&path).unwrap();
    let (_db, storage) = temp_storage("zoo-real");
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config(&format!("{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() == "zoo" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        check(&storage, 1, Some(6118), Some(53), Some(6171));
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE quality_bucket='complete'"
        ),
        1
    );
}

fn legacy_carrier() -> String {
    serde_json::to_string(
        &serde_json::from_str::<serde_json::Value>(DEFAULT)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["say"] == "api_req_started")
            .collect::<Vec<_>>(),
    )
    .unwrap()
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
        "zoo"
    }
    fn agent(&self) -> &'static str {
        "zoo-code"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        ZooAdapter::new().discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        ZooAdapter::new().instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        ZooAdapter::new().detect(p)
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut result = ZooAdapter::new().scan(t, s, l, n)?;
        for (e, u) in result
            .events
            .iter_mut()
            .zip(raw_usage(&std::fs::read_to_string(&t.path).unwrap()))
        {
            e.parser_version = "zoo-ui-messages-doc1".into();
            e.usage.input_total = u["tokensIn"].as_i64();
            e.usage.output_total = u["tokensOut"].as_i64();
            e.usage.input_cache_read = u["cacheReads"].as_i64();
            e.usage.input_cache_write = u["cacheWrites"].as_i64();
            e.usage.input_uncached = None;
            e.usage.total_tokens = e
                .usage
                .input_total
                .zip(e.usage.output_total)
                .and_then(|(i, o)| i.checked_add(o));
            e.quality.input_total = e
                .usage
                .input_total
                .map(|_| Q::Reported)
                .unwrap_or(Q::Unknown);
            e.quality.output_total = e
                .usage
                .output_total
                .map(|_| Q::Reported)
                .unwrap_or(Q::Unknown);
            e.quality.input_cache_read = e
                .usage
                .input_cache_read
                .map(|_| Q::Reported)
                .unwrap_or(Q::Unknown);
            e.quality.input_cache_write = e
                .usage
                .input_cache_write
                .map(|_| Q::Reported)
                .unwrap_or(Q::Unknown);
            e.quality.input_uncached = e
                .usage
                .input_uncached
                .map(|_| Q::Derived)
                .unwrap_or(Q::Unknown);
            e.quality.total_tokens = e
                .usage
                .total_tokens
                .map(|_| Q::Derived)
                .unwrap_or(Q::Unknown);
            e.cost = u["cost"].as_f64().map(|c| CostAmount {
                amount_minor: (c * 1_000_000.).round() as i64,
                currency: "USD".into(),
                kind: CostKind::Estimated,
                price_version: None,
                billing_scope: None,
            });
            match self.variant {
                "input" => e.usage.input_total = e.usage.input_total.map(|v| v + 1),
                "quality" => e.quality.output_total = Q::Derived,
                "model" => e.model_raw = Some("different-model".into()),
                "cost" => e.cost.as_mut().unwrap().amount_minor = 1,
                "revision" => e.source_revision = Some(1),
                "time" => e.occurred_at_ms += 1,
                "ownership" => e.host_application = Some("different-host".into()),
                _ => {}
            }
        }
        *self.events.lock().unwrap() = result.events.clone();
        Ok(result)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = ZooAdapter::new().capability();
        c.maintenance["parser_version"] = "zoo-ui-messages-doc1".into();
        c
    }
}

#[test]
fn unchanged_consumed_cursor_full_legacy_digests_parallel_upgrade_and_atomic_rollback() {
    for legacy_hash in [false, true] {
        for fail_once in [false, true] {
            let dir = TempDir::new("zoo-old");
            let (ctx, path) = carrier(&dir, &legacy_carrier());
            let original = std::fs::read(&path).unwrap();
            let (_db, storage) = temp_storage("zoo-old");
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
            let before:Vec<(String,Option<i64>,Option<i64>,i64)>=storage.conn().prepare("SELECT source_record_key,source_revision,observed_at_ms,created_at_ms FROM usage_events ORDER BY source_record_key").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().map(Result::unwrap).collect();
            storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','prior audit',0 FROM source_instances",[]).unwrap();
            if fail_once {
                storage.conn().execute_batch("CREATE TRIGGER fail_zoo_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
                let r = run_adapter_scan(&storage, &ZooAdapter::new(), &ctx, &config("failed"))
                    .unwrap();
                assert_eq!(r[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE parser_version='zoo-ui-messages-doc1' AND input_cache_read=0 AND input_cache_write=0 AND cost_amount_minor=0"),1);
                assert_eq!(
                    count(
                        &storage,
                        "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                    ),
                    0
                );
                assert_eq!(
                    count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
                    0
                );
                storage
                    .conn()
                    .execute_batch("DROP TRIGGER fail_zoo_checkpoint")
                    .unwrap();
            }
            let adapter = ZooAdapter::new();
            let requests = [ParallelScanRequest {
                adapter: &adapter,
                context: ctx.clone(),
                config: config("upgraded"),
                filter: InstanceFilter::default(),
            }];
            let locked = Mutex::new(storage);
            let reports = run_adapter_scans_parallel(
                &locked,
                &requests,
                None,
                std::sync::Arc::new(|| true),
                None,
            )
            .into_iter()
            .next()
            .unwrap()
            .unwrap();
            let storage = locked.into_inner().unwrap();
            assert!(reports.iter().all(|r| r.error.is_none()));
            check(&storage, 1, Some(6118), Some(53), Some(6171));
            run_adapter_scan(&storage, &adapter, &ctx, &config("repeat")).unwrap();
            check(&storage, 1, Some(6118), Some(53), Some(6171));
            assert_eq!(
                count(
                    &storage,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                ),
                1
            );
            assert_eq!(
                count(
                    &storage,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='preserved_history'"
                ),
                1
            );
            let after:Vec<(String,Option<i64>,Option<i64>,i64)>=storage.conn().prepare("SELECT source_record_key,source_revision,observed_at_ms,created_at_ms FROM usage_events ORDER BY source_record_key").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().map(Result::unwrap).collect();
            assert_eq!(before, after);
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }
}

#[test]
fn full_digest_rejects_changed_values_quality_identity_revision_and_same_batch_conflicts_survive() {
    for variant in [
        "input",
        "quality",
        "model",
        "cost",
        "revision",
        "time",
        "ownership",
        "same-batch",
    ] {
        let dir = TempDir::new("zoo-conflict");
        let (ctx, _) = carrier(&dir, &legacy_carrier());
        let (_db, storage) = temp_storage("zoo-conflict");
        let old = Legacy::new(variant);
        run_adapter_scan(&storage, &old, &ctx, &config("old")).unwrap();
        let prior = old.events.lock().unwrap()[0].clone();
        if variant == "same-batch" {
            let adapter = ZooAdapter::new();
            let zoot = adapter.discover(&ctx).remove(0);
            let path = &zoot.files[0];
            let target = ScanTarget {
                instance_id: adapter.instance_id(&zoot),
                file_id: "native".into(),
                path: path.clone(),
                file_identity: "native".into(),
                probe: llm_usage_core::adapters::jsonl::probe_file(path).unwrap(),
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
            let r = llm_usage_core::ingest::commit_batch(
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
            assert_eq!((r.updated, r.conflicts), (1, 1));
        } else {
            run_adapter_scan(&storage, &ZooAdapter::new(), &ctx, &config("new")).unwrap();
        }
        let (parser, conflict): (String, i64) = storage
            .conn()
            .query_row(
                "SELECT parser_version,conflict FROM usage_events WHERE source_record_key=?1",
                [&prior.source_record_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(conflict, 1, "{variant}");
        assert_eq!(
            parser,
            if variant == "same-batch" {
                "zoo-ui-messages-doc2"
            } else {
                "zoo-ui-messages-doc1"
            },
            "{variant}"
        );
    }
}

#[test]
fn explicit_default_zero_calls_upgrade_to_unknown_while_pure_placeholders_stay_unobserved() {
    let mut rows: serde_json::Value = serde_json::from_str(DEFAULT).unwrap();
    for r in rows.as_array_mut().unwrap() {
        if r["say"] == "api_req_started" {
            r["text"]=serde_json::json!({"tokensIn":0,"tokensOut":0,"cacheReads":0,"cacheWrites":0,"cost":0,"apiProtocol":"openai"}).to_string().into();
        }
    }
    rows.as_array_mut().unwrap().push(serde_json::json!({"type":"say","say":"api_req_started","ts":1791279400000i64,"text":"{\"apiProtocol\":\"openai\"}"}));
    let text = rows.to_string();
    let dir = TempDir::new("zoo-zero");
    let (ctx, path) = carrier(&dir, &text);
    let (_db, storage) = temp_storage("zoo-zero");
    run_adapter_scan(&storage, &Legacy::new("default"), &ctx, &config("old")).unwrap();
    run_adapter_scan(&storage, &ZooAdapter::new(), &ctx, &config("new")).unwrap();
    check(&storage, 1, None, None, None);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE quality_bucket='unknown'"
        ),
        1
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}

#[test]
fn malformed_request_isolated_and_unknown_ask_still_holds_whole_file() {
    let mut rows: serde_json::Value = serde_json::from_str(DEFAULT).unwrap();
    let mut bad = rows[1].clone();
    bad["ts"] = 1791290965700i64.into();
    bad["text"] = serde_json::json!({"tokensIn":"bad","tokensOut":1})
        .to_string()
        .into();
    rows.as_array_mut().unwrap().insert(0, bad);
    let dir = TempDir::new("zoo-bad");
    let (ctx, path) = carrier(&dir, &rows.to_string());
    let (_db, storage) = temp_storage("zoo-bad");
    run_adapter_scan(&storage, &ZooAdapter::new(), &ctx, &config("bad")).unwrap();
    check(&storage, 1, Some(6118), Some(53), Some(6171));
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_shape_deviation'"
        ),
        1
    );
    rows.as_array_mut()
        .unwrap()
        .push(serde_json::json!({"type":"ask","ask":"unknown_future_ask","ts":1791290973140i64}));
    std::fs::write(&path, rows.to_string()).unwrap();
    let r = run_adapter_scan(&storage, &ZooAdapter::new(), &ctx, &config("unknown")).unwrap();
    assert_eq!(r[0].files[0].status, "pending");
    assert_eq!(r[0].files[0].events, 0);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='undocumented_ask_kind'"
        ),
        1
    );
}
