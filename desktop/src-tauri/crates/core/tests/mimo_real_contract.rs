//! MiMo 0.1.15 API/native projection and full historical policy migration.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, mimo_code::MimoCodeAdapter};
use llm_usage_core::domain::{CostAmount, CostKind, EventInput, FieldQuality as Q};
use llm_usage_core::{error::CoreError, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
const NOW: i64 = 1_800_000_000_000;
const PROJECTION: &str = include_str!("fixtures/mimo-code/real-0.1.15/projection.json");
fn projection() -> serde_json::Value {
    serde_json::from_str(PROJECTION).unwrap()
}
fn carrier(dir: &TempDir, p: &serde_json::Value) -> (DiscoverContext, PathBuf) {
    let home = dir.path().join("mimo-home");
    let file = home.join("data/mimocode.db");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(&file).unwrap();
    for table in ["session", "message", "part"] {
        db.execute_batch(p["schema"][format!("{table}_ddl")].as_str().unwrap())
            .unwrap();
        let list = match table {
            "session" => "sessions",
            "message" => "messages",
            _ => "parts",
        };
        for row in p[list].as_array().unwrap() {
            let obj = row.as_object().unwrap();
            let columns = obj.keys().cloned().collect::<Vec<_>>();
            let values = obj
                .values()
                .map(|v| match v {
                    serde_json::Value::Null => rusqlite::types::Value::Null,
                    serde_json::Value::Number(n) => {
                        rusqlite::types::Value::Integer(n.as_i64().unwrap())
                    }
                    serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                    _ => rusqlite::types::Value::Text(v.to_string()),
                })
                .collect::<Vec<_>>();
            db.execute(
                &format!(
                    "INSERT INTO {table} ({}) VALUES ({})",
                    columns.join(","),
                    vec!["?"; columns.len()].join(",")
                ),
                rusqlite::params_from_iter(values),
            )
            .unwrap();
        }
    }
    drop(db);
    (
        DiscoverContext {
            env: std::collections::BTreeMap::from([(
                "MIMOCODE_HOME".into(),
                home.to_string_lossy().into(),
            )]),
            ..Default::default()
        },
        file,
    )
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
fn count(s: &Storage, sql: &str) -> i64 {
    s.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn check(s: &Storage) {
    assert_eq!(count(s,"SELECT COUNT(*) FROM usage_events WHERE agent='mimo-code' AND parser_version='mimo-code-step-finish-parts-2' AND input_cache_write IS NULL AND output_reasoning IS NULL AND cost_amount_minor IS NULL"),8);
    let v:(i64,i64,i64,i64,i64)=s.conn().query_row("SELECT SUM(input_uncached),SUM(input_cache_read),SUM(input_total),SUM(output_total),SUM(total_tokens) FROM usage_events",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
    assert_eq!(v, (4855, 20733, 25588, 1024, 26612));
    assert_eq!(
        count(
            s,
            "SELECT COUNT(*) FROM usage_events WHERE input_cache_read=0 OR conflict=1"
        ),
        0
    );
    let t = summary(s, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            t.call_count,
            t.input_total_known,
            t.output_total_known,
            t.total_tokens_known
        ),
        (8, Some(25588), Some(1024), Some(26612))
    );
}
#[test]
fn actual_eight_length_calls_match_api_and_repeat_without_message_double_count() {
    let p = projection();
    let api: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "fixtures/mimo-code/real-0.1.15/api-usage.json"
    ))
    .unwrap();
    assert_eq!(api.len(), 8);
    for (row, a) in p["parts"].as_array().unwrap().iter().zip(&api) {
        let t = &row["data"]["tokens"];
        assert_eq!(
            t["input"].as_i64().unwrap()
                + t["cache"]["read"].as_i64().unwrap()
                + t["cache"]["write"].as_i64().unwrap(),
            a["usage"]["prompt_tokens"].as_i64().unwrap()
        );
        assert_eq!(t["output"], a["usage"]["completion_tokens"]);
        assert_eq!(t["total"], a["usage"]["total_tokens"]);
        assert_eq!(row["data"]["reason"], "length");
    }
    let dir = TempDir::new("mimo-real");
    let (ctx, file) = carrier(&dir, &p);
    let original = std::fs::read(&file).unwrap();
    let (_db, s) = temp_storage("mimo-real");
    for pass in 0..2 {
        for a in llm_usage_core::adapters::built_in_adapters() {
            let r = run_adapter_scan(
                &s,
                a.as_ref(),
                &ctx,
                &config(&format!("{pass}-{}", a.adapter_id())),
            )
            .unwrap();
            if a.adapter_id() == "mimo-code" {
                assert_eq!(r.len(), 1);
                assert!(r[0].error.is_none());
            } else {
                assert!(r.is_empty(), "{}", a.adapter_id());
            }
        }
        check(&s);
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
    assert_eq!(count(&s,"SELECT COUNT(*) FROM usage_events WHERE schema_version='0.1.15' AND parse_basis='latest_fallback' AND model_raw='qwen3.5-0.8b-local' AND provider_id='local-llama' AND source_total=total_tokens"),8);
}
struct Legacy {
    variant: &'static str,
    events: Mutex<Vec<EventInput>>,
}
impl Legacy {
    fn new(variant: &'static str) -> Self {
        Self {
            variant,
            events: Mutex::new(vec![]),
        }
    }
}
impl SourceAdapter for Legacy {
    fn adapter_id(&self) -> &'static str {
        "mimo-code"
    }
    fn agent(&self) -> &'static str {
        "mimo-code"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        MimoCodeAdapter::new().discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        MimoCodeAdapter::new().instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        MimoCodeAdapter::new().detect(p)
    }
    fn capability(&self) -> CapabilityTable {
        MimoCodeAdapter::new().capability()
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut out = MimoCodeAdapter::new().scan(t, s, l, n)?;
        for e in &mut out.events {
            e.parser_version = "mimo-code-step-finish-parts-1".into();
            for (v, q) in [
                (&mut e.usage.input_uncached, &mut e.quality.input_uncached),
                (
                    &mut e.usage.input_cache_read,
                    &mut e.quality.input_cache_read,
                ),
                (
                    &mut e.usage.input_cache_write,
                    &mut e.quality.input_cache_write,
                ),
                (
                    &mut e.usage.output_reasoning,
                    &mut e.quality.output_reasoning,
                ),
            ] {
                if v.is_none() {
                    *v = Some(0);
                    *q = Q::Reported;
                }
            }
            e.cost = Some(CostAmount {
                amount_minor: 0,
                currency: "USD".into(),
                kind: CostKind::Estimated,
                price_version: None,
                billing_scope: None,
            });
            match self.variant {
                "input" => e.usage.input_uncached = e.usage.input_uncached.map(|v| v + 1),
                "quality" => e.quality.input_total = Q::Reported,
                "model" => e.model_raw = Some("changed-model".into()),
                "revision" => e.source_revision = None,
                "ownership" => e.host_application = Some("changed-host".into()),
                "time" => e.occurred_at_ms += 1,
                _ => {}
            }
        }
        // Already consumed far past all source rows; a policy change must reset
        // this independently of byte changes or a source_instances registration.
        out.cursor.as_mut().unwrap()["watermark_ms"] = (NOW + 1_000_000).into();
        out.parse_context.as_mut().unwrap()["scan_policy_version"] =
            "mimo-code-step-finish-parts-1".into();
        out.parse_context.as_mut().unwrap()["replay_policy_version"] =
            "mimo-code-step-finish-parts-1".into();
        *self.events.lock().unwrap() = out.events.clone();
        Ok(out)
    }
}
#[test]
fn consumed_old_watermark_full_digests_parallel_upgrade_and_transaction_rollback() {
    for old_hash in [false, true] {
        for rollback in [false, true] {
            let dir = TempDir::new("mimo-old");
            let (ctx, file) = carrier(&dir, &projection());
            let bytes = std::fs::read(&file).unwrap();
            let (_db, s) = temp_storage("mimo-old");
            let old = Legacy::new("default");
            run_adapter_scan(&s, &old, &ctx, &config("old")).unwrap();
            if old_hash {
                for e in old.events.lock().unwrap().iter() {
                    s.conn()
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
            let audit:Vec<(String,Option<i64>,Option<i64>,i64)>=s.conn().prepare("SELECT source_record_key,source_revision,observed_at_ms,created_at_ms FROM usage_events ORDER BY source_record_key").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().map(Result::unwrap).collect();
            s.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','old audit',0 FROM source_instances",[]).unwrap();
            if rollback {
                s.conn().execute_batch("CREATE TRIGGER fail_mimo_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
                let r =
                    run_adapter_scan(&s, &MimoCodeAdapter::new(), &ctx, &config("failed")).unwrap();
                assert_eq!(r[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(count(&s,"SELECT COUNT(*) FROM usage_events WHERE parser_version='mimo-code-step-finish-parts-1' AND input_cache_write=0 AND output_reasoning=0 AND cost_amount_minor=0"),8);
                assert_eq!(
                    count(
                        &s,
                        "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                    ),
                    0
                );
                s.conn()
                    .execute_batch("DROP TRIGGER fail_mimo_checkpoint")
                    .unwrap();
            }
            let adapter = MimoCodeAdapter::new();
            let req = [ParallelScanRequest {
                adapter: &adapter,
                context: ctx.clone(),
                config: config("new"),
                filter: InstanceFilter::default(),
            }];
            let locked = Mutex::new(s);
            let r =
                run_adapter_scans_parallel(&locked, &req, None, std::sync::Arc::new(|| true), None)
                    .into_iter()
                    .next()
                    .unwrap()
                    .unwrap();
            assert!(r.iter().all(|r| r.error.is_none()));
            let s = locked.into_inner().unwrap();
            check(&s);
            run_adapter_scan(&s, &adapter, &ctx, &config("repeat")).unwrap();
            check(&s);
            assert_eq!(
                count(
                    &s,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                ),
                8
            );
            assert_eq!(
                count(
                    &s,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='preserved_history'"
                ),
                1
            );
            let after:Vec<(String,Option<i64>,Option<i64>,i64)>=s.conn().prepare("SELECT source_record_key,source_revision,observed_at_ms,created_at_ms FROM usage_events ORDER BY source_record_key").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().map(Result::unwrap).collect();
            assert_eq!(audit, after);
            assert_eq!(std::fs::read(&file).unwrap(), bytes);
        }
    }
}
#[test]
fn full_old_hash_rejects_real_changes_and_same_batch_conflict_survives_upgrade() {
    for variant in [
        "input",
        "quality",
        "model",
        "revision",
        "ownership",
        "time",
        "same-batch",
    ] {
        let dir = TempDir::new("mimo-conflict");
        let (ctx, _) = carrier(&dir, &projection());
        let (_db, s) = temp_storage("mimo-conflict");
        let old = Legacy::new(variant);
        run_adapter_scan(&s, &old, &ctx, &config("old")).unwrap();
        let prior = old.events.lock().unwrap()[0].clone();
        if variant == "same-batch" {
            let a = MimoCodeAdapter::new();
            let root = a.discover(&ctx).remove(0);
            let path = &root.files[0];
            let t = ScanTarget {
                instance_id: a.instance_id(&root),
                file_id: "native".into(),
                path: path.clone(),
                file_identity: "native".into(),
                probe: llm_usage_core::adapters::jsonl::probe_file(path).unwrap(),
                generation: 0,
                rescan: true,
            };
            let good = a
                .scan(
                    &t,
                    &StoredScanState::default(),
                    &ScanLimits::default(),
                    NOW + 1,
                )
                .unwrap()
                .events
                .remove(0);
            let mut bad = good.clone();
            bad.model_raw = Some("changed-in-batch".into());
            let r = llm_usage_core::ingest::commit_batch(
                &s,
                &common::batch(&prior.source_instance_id, "UTC", NOW + 1, vec![bad, good]),
                None,
            )
            .unwrap();
            assert_eq!((r.updated, r.conflicts), (1, 1));
        } else {
            run_adapter_scan(&s, &MimoCodeAdapter::new(), &ctx, &config("new")).unwrap();
        }
        let (parser, conflict): (String, i64) = s
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
                "mimo-code-step-finish-parts-2"
            } else {
                "mimo-code-step-finish-parts-1"
            },
            "{variant}"
        );
    }
}
#[test]
fn home_and_database_override_priority_manual_aliases_and_memory_are_explicit() {
    let dir = TempDir::new("mimo-paths");
    let (mut ctx, file) = carrier(&dir, &projection());
    let home = file.parent().unwrap().parent().unwrap().to_path_buf();
    let a = MimoCodeAdapter::new();
    let decoy = dir.path().join("xdg/mimocode");
    std::fs::create_dir_all(&decoy).unwrap();
    std::fs::write(decoy.join("mimocode.db"), b"wrong source").unwrap();
    ctx.env.insert(
        "XDG_DATA_HOME".into(),
        dir.path().join("xdg").to_string_lossy().into(),
    );
    ctx.home_dir = Some(dir.path().into());
    assert_eq!(
        a.discover(&ctx)
            .iter()
            .flat_map(|r| r.files.iter())
            .collect::<Vec<_>>(),
        vec![&file]
    );
    ctx.manual_roots = vec![file.clone(), home.clone(), file.parent().unwrap().into()];
    assert_eq!(
        a.discover(&ctx)
            .iter()
            .map(|r| r.files.len())
            .sum::<usize>(),
        1
    );
    ctx.manual_roots.clear();
    ctx.env
        .insert("MIMOCODE_HOME".into(), "relative-invalid".into());
    assert!(a.discover(&ctx).is_empty());
    ctx.env
        .insert("MIMOCODE_HOME".into(), home.to_string_lossy().into());
    ctx.env.insert("MIMOCODE_DB".into(), ":memory:".into());
    assert!(a.discover(&ctx).is_empty());
    let custom = file.parent().unwrap().join("custom.sqlite");
    std::fs::copy(&file, &custom).unwrap();
    ctx.env.insert("MIMOCODE_DB".into(), "custom.sqlite".into());
    assert_eq!(a.discover(&ctx)[0].files, vec![custom.clone()]);
    ctx.env
        .insert("MIMOCODE_DB".into(), custom.to_string_lossy().into());
    assert_eq!(a.discover(&ctx)[0].files, vec![custom]);
}
#[test]
fn default_zeros_optional_total_and_bad_adjacent_part_keep_known_api_neighbors() {
    let mut p = projection();
    for index in 0..3 {
        p["parts"][index]["data"]["tokens"] = serde_json::json!({"input":0,"output":0,"reasoning":0,"cache":{"read":0,"write":0},"total":0});
    }
    p["parts"][1]["data"]["tokens"]
        .as_object_mut()
        .unwrap()
        .remove("total");
    p["parts"][2]["data"]["tokens"]["total"] = serde_json::json!(7);
    p["parts"][3]["data"]["tokens"]["input"] = serde_json::json!("bad-type");
    let dir = TempDir::new("mimo-unknown");
    let (ctx, _) = carrier(&dir, &p);
    let (_db, s) = temp_storage("mimo-unknown");
    run_adapter_scan(&s, &MimoCodeAdapter::new(), &ctx, &config("unknown")).unwrap();
    assert_eq!(count(&s, "SELECT COUNT(*) FROM usage_events"), 8);
    assert_eq!(count(&s,"SELECT COUNT(*) FROM usage_events WHERE input_uncached IS NULL AND input_total IS NULL AND output_total IS NULL AND input_cache_read IS NULL AND input_cache_write IS NULL AND output_reasoning IS NULL AND cost_amount_minor IS NULL"),4);
    assert_eq!(count(&s,"SELECT COUNT(*) FROM usage_events WHERE total_tokens IS NULL AND source_total IS NULL AND quality_bucket='unknown'"),3);
    assert_eq!(
        count(
            &s,
            "SELECT COUNT(*) FROM usage_events WHERE total_tokens=7 AND source_total=7"
        ),
        1
    );
    assert_eq!(count(&s,"SELECT COUNT(*) FROM usage_events WHERE input_total>0 AND output_total=128 AND source_total=total_tokens"),4);
    assert!(
        count(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_shape_deviation'"
        ) >= 1
    );
}
