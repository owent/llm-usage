//! Official Roo VSIX native/API checks and complete old-summary migration.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, roo::RooAdapter};
use llm_usage_core::domain::{CostAmount, CostKind, EventInput, FieldQuality as Q};
use llm_usage_core::{error::CoreError, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
const DEFAULT: &str = include_str!("fixtures/roo/real-3.54.0/default-ui_messages.json");
const BOUNDED: &str = include_str!("fixtures/roo/real-3.54.0/bounded-ui_messages.json");
const NOW: i64 = 1_800_000_000_000;

fn carrier(dir: &TempDir, text: &str) -> (DiscoverContext, PathBuf) {
    let home = dir.path().join("home");
    let file = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/tasks/anonymous-task/ui_messages.json");
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
        RooAdapter::new().discover(&context)[0].files,
        RooAdapter::new().discover(&linux_context)[0].files
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
    assert_eq!(count(storage,"SELECT COUNT(*) FROM usage_events WHERE input_uncached IS NOT NULL OR input_cache_read IS NOT NULL OR input_cache_write IS NOT NULL OR output_reasoning IS NOT NULL OR source_total IS NOT NULL OR cost_amount_minor IS NOT NULL OR model_raw IS NOT NULL OR provider_id IS NOT NULL OR parser_version!='roo-ui-messages-doc2'"),0);
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
fn official_extension_api_native_and_service_keep_cache_and_cancel_coverage_gaps() {
    for (text, api_text, ext_text, calls, input, output) in [
        (
            DEFAULT,
            include_str!("fixtures/roo/real-3.54.0/default-api-usage.json"),
            include_str!("fixtures/roo/real-3.54.0/default-extension-usage.json"),
            2,
            13867,
            4,
        ),
        (
            BOUNDED,
            include_str!("fixtures/roo/real-3.54.0/bounded-api-usage.json"),
            include_str!("fixtures/roo/real-3.54.0/bounded-extension-usage.json"),
            1,
            6772,
            2,
        ),
    ] {
        let native = raw_usage(text);
        let api: Vec<serde_json::Value> = serde_json::from_str(api_text).unwrap();
        let ext: serde_json::Value = serde_json::from_str(ext_text).unwrap();
        assert_eq!(native.len() as i64, calls);
        assert_eq!(ext["usage"]["totalTokensIn"], input);
        assert_eq!(ext["usage"]["totalTokensOut"], output);
        assert_eq!(ext["tools"], serde_json::json!({}));
        for (n, a) in native.iter().zip(&api) {
            assert_eq!(n["tokensIn"], a["usage"]["prompt_tokens"]);
            assert_eq!(n["tokensOut"], a["usage"]["completion_tokens"]);
            assert_eq!(n["cacheReads"], 0);
            assert_eq!(n["cacheWrites"], 0);
            assert_eq!(n["cost"], 0);
        }
        if calls == 2 {
            assert_eq!(api.len(), 3, "cancelled third request has no native row");
            assert_eq!(
                api[1]["usage"]["prompt_tokens_details"]["cached_tokens"], 6773,
                "native zero cannot mean no cache hit"
            );
        } else {
            assert_eq!(api.len(), 1);
            assert_eq!(ext["allowedMaxRequests"], 2);
        }
        let dir = TempDir::new("roo-real");
        let (ctx, path) = carrier(&dir, text);
        let original = std::fs::read(&path).unwrap();
        let (_db, storage) = temp_storage("roo-real");
        for pass in 0..2 {
            for adapter in llm_usage_core::adapters::built_in_adapters() {
                let reports = run_adapter_scan(
                    &storage,
                    adapter.as_ref(),
                    &ctx,
                    &config(&format!("real-{pass}-{}", adapter.adapter_id())),
                )
                .unwrap();
                if adapter.adapter_id() == "roo" {
                    assert_eq!(reports.len(), 1);
                    assert!(reports[0].error.is_none());
                } else {
                    assert!(reports.is_empty(), "{}", adapter.adapter_id());
                }
            }
            check(
                &storage,
                calls,
                Some(input),
                Some(output),
                Some(input + output),
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM usage_events WHERE quality_bucket='complete'"
            ),
            calls
        );
        assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE json_extract(quality_json,'$.input_uncached')='unknown' AND json_extract(quality_json,'$.input_cache_read')='unknown' AND json_extract(quality_json,'$.input_cache_write')='unknown'"),calls);
    }
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
        "roo"
    }
    fn agent(&self) -> &'static str {
        "roo-code"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        RooAdapter::new().discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        RooAdapter::new().instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        RooAdapter::new().detect(p)
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut result = RooAdapter::new().scan(t, s, l, n)?;
        for (e, u) in result
            .events
            .iter_mut()
            .zip(raw_usage(&std::fs::read_to_string(&t.path).unwrap()))
        {
            e.parser_version = "roo-ui-messages-doc1".into();
            e.usage.input_total = u["tokensIn"].as_i64();
            e.usage.output_total = u["tokensOut"].as_i64();
            e.usage.input_cache_read = u["cacheReads"].as_i64();
            e.usage.input_cache_write = u["cacheWrites"].as_i64();
            e.usage.input_uncached = e
                .usage
                .input_total
                .zip(e.usage.input_cache_read)
                .zip(e.usage.input_cache_write)
                .and_then(|((i, r), w)| i.checked_sub(r)?.checked_sub(w))
                .filter(|v| *v >= 0);
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
        let mut c = RooAdapter::new().capability();
        c.maintenance["parser_version"] = "roo-ui-messages-doc1".into();
        c
    }
}

#[test]
fn unchanged_consumed_cursor_full_legacy_digests_parallel_upgrade_and_atomic_rollback() {
    for legacy_hash in [false, true] {
        for fail_once in [false, true] {
            let dir = TempDir::new("roo-old");
            let (ctx, path) = carrier(&dir, DEFAULT);
            let original = std::fs::read(&path).unwrap();
            let (_db, storage) = temp_storage("roo-old");
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
                storage.conn().execute_batch("CREATE TRIGGER fail_roo_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
                let r = run_adapter_scan(&storage, &RooAdapter::new(), &ctx, &config("failed"))
                    .unwrap();
                assert_eq!(r[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE parser_version='roo-ui-messages-doc1' AND input_cache_read=0 AND input_cache_write=0 AND cost_amount_minor=0"),2);
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
                    .execute_batch("DROP TRIGGER fail_roo_checkpoint")
                    .unwrap();
            }
            let adapter = RooAdapter::new();
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
            check(&storage, 2, Some(13867), Some(4), Some(13871));
            run_adapter_scan(&storage, &adapter, &ctx, &config("repeat")).unwrap();
            check(&storage, 2, Some(13867), Some(4), Some(13871));
            assert_eq!(
                count(
                    &storage,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                ),
                2
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
        let dir = TempDir::new("roo-conflict");
        let (ctx, _) = carrier(&dir, DEFAULT);
        let (_db, storage) = temp_storage("roo-conflict");
        let old = Legacy::new(variant);
        run_adapter_scan(&storage, &old, &ctx, &config("old")).unwrap();
        let prior = old.events.lock().unwrap()[0].clone();
        if variant == "same-batch" {
            let adapter = RooAdapter::new();
            let root = adapter.discover(&ctx).remove(0);
            let path = &root.files[0];
            let target = ScanTarget {
                instance_id: adapter.instance_id(&root),
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
            run_adapter_scan(&storage, &RooAdapter::new(), &ctx, &config("new")).unwrap();
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
                "roo-ui-messages-doc2"
            } else {
                "roo-ui-messages-doc1"
            },
            "{variant}"
        );
    }
}

#[test]
fn explicit_default_zero_calls_upgrade_to_unknown_while_pure_placeholders_stay_unobserved() {
    let mut rows: serde_json::Value = serde_json::from_str(BOUNDED).unwrap();
    for r in rows.as_array_mut().unwrap() {
        if r["say"] == "api_req_started" {
            r["text"]=serde_json::json!({"tokensIn":0,"tokensOut":0,"cacheReads":0,"cacheWrites":0,"cost":0,"apiProtocol":"openai"}).to_string().into();
        }
    }
    rows.as_array_mut().unwrap().push(serde_json::json!({"type":"say","say":"api_req_started","ts":1791279400000i64,"text":"{\"apiProtocol\":\"openai\"}"}));
    let text = rows.to_string();
    let dir = TempDir::new("roo-zero");
    let (ctx, path) = carrier(&dir, &text);
    let (_db, storage) = temp_storage("roo-zero");
    run_adapter_scan(&storage, &Legacy::new("default"), &ctx, &config("old")).unwrap();
    run_adapter_scan(&storage, &RooAdapter::new(), &ctx, &config("new")).unwrap();
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
fn malformed_request_does_not_block_valid_calls_and_positive_price_stays_estimated() {
    let mut rows: serde_json::Value = serde_json::from_str(DEFAULT).unwrap();
    let mut first = true;
    for r in rows.as_array_mut().unwrap() {
        if r["say"] == "api_req_started" {
            let mut u: serde_json::Value =
                serde_json::from_str(r["text"].as_str().unwrap()).unwrap();
            if first {
                u["tokensIn"] = "bad".into();
                first = false;
            } else {
                u["cost"] = 0.001.into();
            }
            r["text"] = u.to_string().into();
        }
    }
    let dir = TempDir::new("roo-bad");
    let (ctx, _) = carrier(&dir, &rows.to_string());
    let (_db, storage) = temp_storage("roo-bad");
    run_adapter_scan(&storage, &RooAdapter::new(), &ctx, &config("bad")).unwrap();
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total=7095 AND output_total=2 AND cost_amount_minor=1000 AND cost_kind='estimated' AND input_uncached IS NULL AND input_cache_read IS NULL"),1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_shape_deviation'"
        ),
        1
    );
}
