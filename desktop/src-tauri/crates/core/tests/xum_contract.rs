//! Official Xum default-zero data and a native local-model gateway comparison.
mod common;
use common::{temp_storage, TempDir};
use llm_usage_core::adapters::{framework::*, xum::XumAdapter};
use llm_usage_core::{domain::FieldQuality as Q, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
const REAL: &str = include_str!("fixtures/xum/real-0.30.0/controlled-usage-session-usage.json");
const DEFAULT: &str = include_str!("fixtures/xum/real-0.30.0/default-no-usage-session-usage.json");
const NOW: i64 = 1_800_000_000_000;
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
fn carrier(dir: &TempDir, raw: &str) -> (DiscoverContext, PathBuf) {
    let root = dir.path().join("capture");
    std::fs::create_dir_all(root.join("sessions/anonymous-workspace")).unwrap();
    let file = root.join("sessions/anonymous-workspace/session-usage.json");
    std::fs::write(&file, raw).unwrap();
    let mut ctx = DiscoverContext::default();
    ctx.env
        .insert("XUM_RUN_SESSION_ROOT".into(), root.to_string_lossy().into());
    (ctx, file)
}
fn count(db: &Storage, sql: &str) -> i64 {
    db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn row(db: &Storage) -> [Option<i64>; 8] {
    db.conn().query_row("SELECT input_uncached,input_total,input_cache_read,input_cache_write,output_total,output_reasoning,total_tokens,reported_call_count FROM source_aggregates",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?])).unwrap()
}
fn corrected(db: &Storage, raw: &str) {
    let expected = if raw == REAL {
        [Some(13721), None, None, None, Some(2), None, None, None]
    } else {
        [None; 8]
    };
    assert_eq!(row(db), expected);
    assert_eq!(count(db, "SELECT COUNT(*) FROM source_aggregates"), 1);
    assert_eq!(count(db, "SELECT COUNT(*) FROM usage_events"), 0);
    let source: (String, String) = db
        .conn()
        .query_row(
            "SELECT health,parser_version FROM source_instances",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(source, ("ok".into(), "xum-session-usage-2".into()));
}
fn scan(file: &Path, ctx: &DiscoverContext) -> ScanOutcome {
    let adapter = XumAdapter::new();
    let root = adapter.discover(ctx).into_iter().next().unwrap();
    let target = ScanTarget {
        instance_id: adapter.instance_id(&root),
        file_id: "file".into(),
        file_identity: "identity".into(),
        path: file.to_path_buf(),
        generation: 1,
        rescan: true,
        probe: llm_usage_core::adapters::jsonl::probe_file(file).unwrap(),
    };
    adapter
        .scan(
            &target,
            &StoredScanState::default(),
            &ScanLimits::default(),
            NOW,
        )
        .unwrap()
}

#[test]
fn genuine_api_cli_native_and_registry_preserve_unknown_buckets_and_cumulative_scope() {
    let api: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/xum/real-0.30.0/controlled-usage-api-usage.jsonl"
    ))
    .unwrap();
    let provenance: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/xum/real-0.30.0/provenance.json")).unwrap();
    assert_eq!(api["usage"]["prompt_tokens"], 13721);
    assert_eq!(api["usage"]["completion_tokens"], 2);
    assert_eq!(api["usage"]["total_tokens"], 13723);
    assert!(api["client_stream_options"].is_null());
    assert_eq!(api["upstream_stream_options"]["include_usage"], true);
    assert_eq!(
        provenance["controlled"]["cli"]["usage"]["inputTokens"],
        13721
    );
    assert_eq!(provenance["controlled"]["exit_code"], 0);
    let dir = TempDir::new("xum-real");
    let (ctx, file) = carrier(&dir, REAL);
    let (_db, storage) = temp_storage("xum-real");
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config(&format!("real-{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() == "xum" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        corrected(&storage, REAL);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), REAL);
    }
    let interval: (Option<i64>, i64, i64) = storage
        .conn()
        .query_row(
            "SELECT interval_start_ms,interval_end_ms,source_revision FROM source_aggregates",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(interval, (None, 1_791_276_947_596, 1_791_276_947_596));
    assert!(XumAdapter::new().capability().maintenance["evidence_level"]
        .as_str()
        .unwrap()
        .starts_with("real-local"));
}

#[test]
fn genuine_missing_usage_does_not_certify_five_initialized_zeroes_or_calls() {
    let api: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/xum/real-0.30.0/default-no-usage-api-usage.jsonl"
    ))
    .unwrap();
    assert!(api["usage"].is_null());
    let dir = TempDir::new("xum-default");
    let (ctx, file) = carrier(&dir, DEFAULT);
    let (_db, storage) = temp_storage("xum-default");
    run_adapter_scan(&storage, &XumAdapter::new(), &ctx, &config("default")).unwrap();
    corrected(&storage, DEFAULT);
    let out = scan(&file, &ctx);
    let q = &out.aggregates[0].quality;
    assert!([
        q.input_uncached,
        q.input_total,
        q.input_cache_read,
        q.input_cache_write,
        q.output_total,
        q.output_reasoning,
        q.total_tokens,
        q.source_total
    ]
    .into_iter()
    .all(|q| q == Q::Unknown));
    assert!(out.diagnostics.is_empty());
}

#[test]
fn disjoint_display_buckets_include_known_reasoning_without_guessing_complete_totals() {
    let mut value: serde_json::Value = serde_json::from_str(REAL).unwrap();
    let bucket = &mut value["byModel"]["local-model:qwen2.5-0.5b-local"];
    bucket["cached"]["tokens"] = 30.into();
    bucket["cacheCreate"]["tokens"] = 10.into();
    bucket["reasoning"]["tokens"] = 5.into();
    bucket["output"]["tokens"] = 20.into();
    let dir = TempDir::new("xum-parts");
    let (ctx, file) = carrier(&dir, &value.to_string());
    let out = scan(&file, &ctx);
    let a = &out.aggregates[0];
    assert_eq!(
        (
            a.usage.input_uncached,
            a.usage.input_cache_read,
            a.usage.input_cache_write
        ),
        (Some(13721), Some(30), Some(10))
    );
    assert_eq!(
        (
            a.usage.output_total,
            a.usage.output_reasoning,
            a.quality.output_total
        ),
        (Some(25), Some(5), Q::Derived)
    );
    assert!(a.usage.input_total.is_none() && a.usage.total_tokens.is_none());
    let (_db, db) = temp_storage("xum-known-reasoning-upgrade");
    run_adapter_scan(&db, &Legacy("original"), &ctx, &config("old-known")).unwrap();
    run_adapter_scan(&db, &XumAdapter::new(), &ctx, &config("new-known")).unwrap();
    assert_eq!(
        row(&db),
        [
            Some(13721),
            None,
            Some(30),
            Some(10),
            Some(25),
            Some(5),
            None,
            None
        ]
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
        ),
        1
    );
    value["byModel"]["bad:model"] = serde_json::json!({"input":{"tokens":"wrong"}});
    value["byModel"]["overflow:model"] = serde_json::json!({"input":{"tokens":1},"output":{"tokens":llm_usage_core::domain::MAX_TOKEN_VALUE},"reasoning":{"tokens":1}});
    std::fs::write(&file, value.to_string()).unwrap();
    let out = scan(&file, &ctx);
    assert_eq!(out.aggregates.len(), 2);
    assert_eq!(out.health, "degraded");
    let overflow = out
        .aggregates
        .iter()
        .find(|a| a.scope_key.ends_with("overflow:model"))
        .unwrap();
    assert_eq!(overflow.usage.output_total, None);
    assert_eq!(overflow.usage.output_reasoning, Some(1));
    assert_eq!(
        out.diagnostics
            .iter()
            .filter(|d| d.code == "token_shape_deviation")
            .count(),
        2
    );
}

struct Legacy(&'static str);
impl SourceAdapter for Legacy {
    fn adapter_id(&self) -> &'static str {
        "xum"
    }
    fn agent(&self) -> &'static str {
        "xum"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        XumAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        XumAdapter::new().instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, llm_usage_core::error::CoreError> {
        XumAdapter::new().detect(path)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = XumAdapter::new().capability();
        c.maintenance["parser_version"] = "xum-session-usage-1".into();
        c
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<ScanOutcome, llm_usage_core::error::CoreError> {
        let mut out = XumAdapter::new().scan(target, stored, limits, now)?;
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&target.path)?).unwrap();
        for a in &mut out.aggregates {
            let entry = &raw["byModel"][a.scope_key.splitn(3, ':').nth(2).unwrap()];
            let get = |key: &str| entry[key]["tokens"].as_i64();
            a.usage = llm_usage_core::domain::TokenUsage {
                input_total: get("input"),
                input_cache_read: get("cached"),
                input_cache_write: get("cacheCreate"),
                output_total: get("output"),
                output_reasoning: get("reasoning"),
                ..Default::default()
            };
            a.quality = llm_usage_core::domain::TokenQuality {
                input_total: if a.usage.input_total.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                input_cache_read: if a.usage.input_cache_read.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                input_cache_write: if a.usage.input_cache_write.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                output_total: if a.usage.output_total.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                output_reasoning: if a.usage.output_reasoning.is_some() {
                    Q::Reported
                } else {
                    Q::Unknown
                },
                ..Default::default()
            };
            match self.0 {
                "input" => a.usage.input_total = Some(13000),
                "output" => a.usage.output_total = Some(3),
                "cache" => a.usage.input_cache_read = Some(1),
                "quality" => a.quality.input_total = Q::Derived,
                "revision" => a.source_revision = a.source_revision.map(|n| n + 1),
                "coverage" => a.coverage = llm_usage_core::aggregates::Coverage::OverlapUnknown,
                "interval" => a.interval_end_ms -= 1,
                "calls" => a.reported_call_count = Some(1),
                "total" => {
                    a.usage.total_tokens = Some(13723);
                    a.quality.total_tokens = Q::Reported;
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

#[test]
fn unchanged_legacy_cursor_repairs_only_exact_policy_with_parallel_atomic_rollback_and_history() {
    for raw in [REAL, DEFAULT] {
        for fail_once in [false, true] {
            let dir = TempDir::new("xum-old");
            let (ctx, file) = carrier(&dir, raw);
            let (_db, db) = temp_storage("xum-old");
            run_adapter_scan(&db, &Legacy("original"), &ctx, &config("old")).unwrap();
            let original = row(&db);
            let revision: i64 = db
                .conn()
                .query_row("SELECT source_revision FROM source_aggregates", [], |r| {
                    r.get(0)
                })
                .unwrap();
            db.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'aggregate_conflict','preserved prior conflict',0 FROM source_instances",[]).unwrap();
            if fail_once {
                db.conn().execute_batch("CREATE TRIGGER fail_xum BEFORE UPDATE ON source_aggregates BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
                let reports =
                    run_adapter_scan(&db, &XumAdapter::new(), &ctx, &config("failed")).unwrap();
                assert_eq!(reports[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(row(&db), original);
                assert_eq!(count(&db, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
                assert_eq!(count(&db,"SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"),0);
                db.conn().execute_batch("DROP TRIGGER fail_xum;").unwrap();
            }
            let adapter = XumAdapter::new();
            let requests = [ParallelScanRequest {
                adapter: &adapter,
                context: ctx.clone(),
                config: config("upgrade"),
                filter: InstanceFilter::default(),
            }];
            let locked = std::sync::Mutex::new(db);
            let results = run_adapter_scans_parallel(
                &locked,
                &requests,
                None,
                std::sync::Arc::new(|| true),
                None,
            );
            assert!(results
                .into_iter()
                .next()
                .unwrap()
                .unwrap()
                .iter()
                .all(|r| r.error.is_none()));
            let db = locked.into_inner().unwrap();
            corrected(&db, raw);
            assert_eq!(
                db.conn()
                    .query_row("SELECT source_revision FROM source_aggregates", [], |r| r
                        .get::<_, i64>(
                        0
                    ))
                    .unwrap(),
                revision
            );
            run_adapter_scan(&db, &adapter, &ctx, &config("repeat")).unwrap();
            corrected(&db, raw);
            assert_eq!(
                count(
                    &db,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
                ),
                1
            );
            assert_eq!(
                count(
                    &db,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_conflict'"
                ),
                1
            );
            assert_eq!(std::fs::read_to_string(&file).unwrap(), raw);
        }
    }
}

#[test]
fn display_correction_never_overwrites_other_positive_quality_revision_or_coverage_changes() {
    for variant in [
        "input", "output", "cache", "quality", "revision", "coverage", "interval", "calls", "total",
    ] {
        let dir = TempDir::new("xum-conflict");
        let (ctx, _file) = carrier(&dir, REAL);
        let (_db, db) = temp_storage("xum-conflict");
        run_adapter_scan(&db, &Legacy(variant), &ctx, &config("old")).unwrap();
        let old: String = db
            .conn()
            .query_row("SELECT content_hash FROM source_aggregates", [], |r| {
                r.get(0)
            })
            .unwrap();
        run_adapter_scan(&db, &XumAdapter::new(), &ctx, &config("new")).unwrap();
        assert_eq!(
            db.conn()
                .query_row("SELECT content_hash FROM source_aggregates", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            old,
            "{variant}"
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
            ),
            0,
            "{variant}"
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_conflict'"
            ),
            i64::from(variant != "revision"),
            "{variant}"
        );
    }
}

#[test]
fn documented_roots_use_canonical_precedence_and_do_not_certify_unknown_schema_versions() {
    let dir = TempDir::new("xum-env");
    let (ctx, file) = carrier(&dir, REAL);
    let value = ctx.env["XUM_RUN_SESSION_ROOT"].clone();
    for key in [
        "XUM_ROOT",
        "MUX_ROOT",
        "XUM_RUN_SESSION_ROOT",
        "MUX_RUN_SESSION_ROOT",
    ] {
        let mut ctx = DiscoverContext::default();
        ctx.env.insert(key.into(), value.clone());
        assert_eq!(
            XumAdapter::new().discover(&ctx)[0].files,
            vec![file.clone()]
        );
    }
    let mut ctx = DiscoverContext::default();
    ctx.env.insert(
        "XUM_ROOT".into(),
        dir.path().join("missing").to_string_lossy().into(),
    );
    ctx.env.insert("MUX_ROOT".into(), value);
    assert!(XumAdapter::new().discover(&ctx).is_empty());
    let mut raw: serde_json::Value = serde_json::from_str(REAL).unwrap();
    raw["version"] = 2.into();
    std::fs::write(&file, raw.to_string()).unwrap();
    assert!(matches!(
        XumAdapter::new().detect(&file).unwrap(),
        DetectOutcome::Supported {
            basis: llm_usage_core::domain::VersionBasis::LatestFallback,
            ..
        }
    ));
}
