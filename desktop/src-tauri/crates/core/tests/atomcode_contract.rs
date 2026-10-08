//! Native AtomCode cumulative data, default-zero handling and atomic old-database correction.
mod common;
use common::{temp_storage, TempDir};
use llm_usage_core::adapters::{atomcode::AtomCodeAdapter, framework::*};
use llm_usage_core::{domain::FieldQuality as Q, jobs::TriggerKind, storage::Storage};
use std::path::{Path, PathBuf};
const FIXTURE: &str = include_str!("fixtures/atomcode/real-5.2.1/session.meta");
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
fn carrier(dir: &TempDir, value: &serde_json::Value) -> (DiscoverContext, PathBuf) {
    let root = dir.path().join("atomcode");
    std::fs::create_dir_all(root.join("sessions/project")).unwrap();
    let file = root.join("sessions/project/11111111-2222-4333-8444-555555555555.meta");
    std::fs::write(&file, serde_json::to_vec(value).unwrap()).unwrap();
    let mut ctx = DiscoverContext::default();
    ctx.env
        .insert("ATOMCODE_HOME".into(), root.to_string_lossy().into());
    (ctx, file)
}
fn sample() -> serde_json::Value {
    serde_json::from_str(FIXTURE).unwrap()
}

#[test]
fn native_companions_are_excluded_only_with_complete_shape_and_paired_meta() {
    let dir = TempDir::new("atomcode-companions");
    let (ctx, file) = carrier(&dir, &sample());
    let parent = file.parent().unwrap();
    let id = file.file_stem().unwrap().to_str().unwrap();
    let ui = parent.join(format!("{id}.ui.json"));
    let rewind = parent.join(format!("{id}.rewind.json"));
    std::fs::write(&ui, r#"{"v":1,"entries":[]}"#).unwrap();
    std::fs::write(&rewind, r#"{"version":2,"points":[]}"#).unwrap();
    let adapter = AtomCodeAdapter::new();
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].files, vec![file.clone()]);
    let (_db, storage) = temp_storage("atomcode-companions");
    run_adapter_scan(&storage, &adapter, &ctx, &config("paired")).unwrap();
    corrected(&storage);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM diagnostics"), 0);
    std::fs::write(&rewind, r#"{"version":99,"points":[]}"#).unwrap();
    assert!(adapter.discover(&ctx)[0].files.contains(&rewind));
    std::fs::write(&rewind, r#"{"version":2,"points":[]}"#).unwrap();
    std::fs::write(&ui, "{}").unwrap();
    assert!(adapter.discover(&ctx)[0].files.contains(&ui));
    let mut manual = DiscoverContext::default();
    manual.manual_roots.push(parent.to_path_buf());
    assert!(adapter.discover(&manual)[0].files.contains(&ui));
    let (_manual_db, manual_storage) = temp_storage("atomcode-manual-companion");
    run_adapter_scan(&manual_storage, &adapter, &manual, &config("manual-error")).unwrap();
    assert!(count(&manual_storage, "SELECT COUNT(*) FROM diagnostics") > 0);
    std::fs::write(&ui, FIXTURE).unwrap();
    assert!(
        adapter.discover(&ctx)[0].files.contains(&ui),
        "a manually named usage carrier must not be hidden"
    );
}
fn row(storage: &Storage) -> [Option<i64>; 6] {
    storage.conn().query_row("SELECT input_total,output_total,total_tokens,input_cache_read,input_uncached,reported_call_count FROM source_aggregates WHERE scope_key LIKE '%local-model/qwen2.5-0.5b-local'", [], |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?])).unwrap()
}
fn count(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn corrected(storage: &Storage) {
    assert_eq!(
        row(storage),
        [Some(6176), Some(2), Some(6178), None, None, Some(1)]
    );
    assert_eq!(count(storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(count(storage, "SELECT COUNT(*) FROM source_aggregates"), 1);
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
        (Some(6176), Some(2), Some(6178), Some(1))
    );
}

#[test]
fn real_headless_api_cli_meta_and_registry_preserve_interval_and_unknown_cache() {
    let api: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/atomcode/real-5.2.1/api-usage.json")).unwrap();
    let cli: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/atomcode/real-5.2.1/cli-usage.json")).unwrap();
    assert_eq!(api["usage"]["prompt_tokens"], 6176);
    assert_eq!(api["usage"]["completion_tokens"], 2);
    assert_eq!(cli[1]["total_tokens"], 6178);
    assert_eq!(cli[1]["rounds"], 1);
    let dir = TempDir::new("atomcode-real");
    let (ctx, file) = carrier(&dir, &sample());
    let original = std::fs::read(&file).unwrap();
    let (_db, storage) = temp_storage("atomcode-real");
    for pass in 0..2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let reports = run_adapter_scan(
                &storage,
                adapter.as_ref(),
                &ctx,
                &config(&format!("real-{pass}-{}", adapter.adapter_id())),
            )
            .unwrap();
            if adapter.adapter_id() == "atomcode" {
                assert_eq!(reports.len(), 1);
                assert!(reports[0].error.is_none());
            } else {
                assert!(reports.is_empty(), "{}", adapter.adapter_id());
            }
        }
        corrected(&storage);
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
    let interval: (i64,i64,bool,String)=storage.conn().query_row("SELECT interval_start_ms,interval_end_ms,interval_end_inclusive,time_basis FROM source_aggregates",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(
        interval,
        (
            1_791_270_569_650,
            1_791_270_575_895,
            false,
            "uncertain".into()
        )
    );
    let caps = AtomCodeAdapter::new().capability();
    assert_eq!(caps.supported_versions, vec!["atomcode-meta-turns-1"]);
    assert!(caps.maintenance["evidence_level"]
        .as_str()
        .unwrap()
        .starts_with("real-local"));
}

struct Legacy(&'static str);
impl SourceAdapter for Legacy {
    fn adapter_id(&self) -> &'static str {
        "atomcode"
    }
    fn agent(&self) -> &'static str {
        "atomcode"
    }
    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        AtomCodeAdapter::new().discover(ctx)
    }
    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        AtomCodeAdapter::new().instance_id(root)
    }
    fn detect(&self, path: &Path) -> Result<DetectOutcome, llm_usage_core::error::CoreError> {
        AtomCodeAdapter::new().detect(path)
    }
    fn scan(
        &self,
        target: &ScanTarget,
        stored: &StoredScanState,
        limits: &ScanLimits,
        now: i64,
    ) -> Result<ScanOutcome, llm_usage_core::error::CoreError> {
        let mut result = AtomCodeAdapter::new().scan(target, stored, limits, now)?;
        for a in &mut result.aggregates {
            a.usage.input_cache_read = Some(0);
            a.quality.input_cache_read = Q::Reported;
            a.usage.input_uncached = Some(a.usage.input_total.unwrap_or(0));
            a.quality.input_uncached = Q::Reported;
            a.usage.input_total = Some(a.usage.input_total.unwrap_or(0));
            a.quality.input_total = Q::Derived;
            a.usage.output_total = Some(a.usage.output_total.unwrap_or(0));
            a.quality.output_total = Q::Reported;
            a.usage.total_tokens = a
                .usage
                .input_total
                .zip(a.usage.output_total)
                .map(|(i, o)| i + o);
            a.quality.total_tokens = Q::Derived;
            match self.0 {
                "input" => {
                    a.usage.input_total = Some(6000);
                    a.usage.input_uncached = Some(6000);
                    a.usage.total_tokens = Some(6002);
                }
                "quality" => a.quality.output_total = Q::Derived,
                "coverage" => a.coverage = llm_usage_core::aggregates::Coverage::OverlapUnknown,
                "revision" => a.source_revision = a.source_revision.map(|v| v + 1),
                "calls" => a.reported_call_count = Some(2),
                "time" => a.interval_start_ms = a.interval_start_ms.map(|v| v - 1),
                _ => {}
            }
        }
        Ok(result)
    }
    fn capability(&self) -> CapabilityTable {
        let mut caps = AtomCodeAdapter::new().capability();
        caps.maintenance["parser_version"] = "atomcode-meta-turns-1".into();
        caps
    }
}

#[test]
fn consumed_cursor_complete_old_hash_parallel_policy_and_checkpoint_rollback() {
    for fail_once in [false, true] {
        let dir = TempDir::new("atomcode-old");
        let (ctx, file) = carrier(&dir, &sample());
        let original = std::fs::read(&file).unwrap();
        let (_db, storage) = temp_storage("atomcode-old");
        run_adapter_scan(&storage, &Legacy("defaults"), &ctx, &config("old")).unwrap();
        assert_eq!(
            row(&storage),
            [
                Some(6176),
                Some(2),
                Some(6178),
                Some(0),
                Some(6176),
                Some(1)
            ]
        );
        assert_eq!(
            count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
            1
        );
        let revision: i64 = storage
            .conn()
            .query_row("SELECT source_revision FROM source_aggregates", [], |r| {
                r.get(0)
            })
            .unwrap();
        storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','prior test diagnostic',0 FROM source_instances",[]).unwrap();
        if fail_once {
            storage.conn().execute_batch("CREATE TRIGGER fail_atom_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
            let reports =
                run_adapter_scan(&storage, &AtomCodeAdapter::new(), &ctx, &config("failed"))
                    .unwrap();
            assert_eq!(reports[0].finish, llm_usage_core::jobs::RunStatus::Failed);
            assert_eq!(row(&storage)[3], Some(0));
            assert_eq!(
                count(
                    &storage,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
                ),
                0
            );
            assert_eq!(
                count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
                0
            );
            storage
                .conn()
                .execute_batch("DROP TRIGGER fail_atom_checkpoint")
                .unwrap();
        }
        let adapter = AtomCodeAdapter::new();
        let requests = [ParallelScanRequest {
            adapter: &adapter,
            context: ctx.clone(),
            config: config("upgrade"),
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
        assert!(results
            .into_iter()
            .next()
            .unwrap()
            .unwrap()
            .iter()
            .all(|r| r.error.is_none()));
        let storage = locked.into_inner().unwrap();
        corrected(&storage);
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT source_revision FROM source_aggregates", [], |r| r
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            revision
        );
        run_adapter_scan(&storage, &adapter, &ctx, &config("repeat")).unwrap();
        corrected(&storage);
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
            ),
            1
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_conflict'"
            ),
            0
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='preserved_history'"
            ),
            1
        );
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
}

#[test]
fn complete_old_defaults_do_not_override_other_data_quality_scope_or_revision() {
    for variant in ["input", "quality", "coverage", "revision", "calls", "time"] {
        let dir = TempDir::new("atomcode-conflict");
        let (ctx, _file) = carrier(&dir, &sample());
        let (_db, storage) = temp_storage("atomcode-conflict");
        run_adapter_scan(&storage, &Legacy(variant), &ctx, &config("old")).unwrap();
        let before: String = storage
            .conn()
            .query_row("SELECT content_hash FROM source_aggregates", [], |r| {
                r.get(0)
            })
            .unwrap();
        run_adapter_scan(&storage, &AtomCodeAdapter::new(), &ctx, &config("new")).unwrap();
        assert_eq!(
            storage
                .conn()
                .query_row("SELECT content_hash FROM source_aggregates", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            before,
            "{variant}"
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
            ),
            0,
            "{variant}"
        );
    }
}

#[test]
fn all_zero_defaults_remain_unknown_while_reported_round_survives_upgrade() {
    let mut value = sample();
    value["turn_stats"][0]["model_usage"][0]["tokens"] =
        serde_json::json!({"input":0,"output":0,"cached_input":0});
    let dir = TempDir::new("atomcode-zero");
    let (ctx, _file) = carrier(&dir, &value);
    let (_db, storage) = temp_storage("atomcode-zero");
    run_adapter_scan(&storage, &Legacy("defaults"), &ctx, &config("old")).unwrap();
    assert_eq!(
        row(&storage),
        [Some(0), Some(0), Some(0), Some(0), Some(0), Some(1)]
    );
    for pass in 0..2 {
        run_adapter_scan(
            &storage,
            &AtomCodeAdapter::new(),
            &ctx,
            &config(&format!("new-{pass}")),
        )
        .unwrap();
        assert_eq!(row(&storage), [None, None, None, None, None, Some(1)]);
    }
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
        ),
        1
    );
}

#[test]
fn missing_bad_and_overflow_buckets_stay_unknown_without_hiding_valid_models() {
    for variant in [
        "missing_input",
        "missing_cache",
        "missing_output",
        "bad_output",
        "overflow_output",
    ] {
        let mut value = sample();
        let tokens = &mut value["turn_stats"][0]["model_usage"][0]["tokens"];
        match variant {
            "missing_input" => {
                tokens.as_object_mut().unwrap().remove("input");
            }
            "missing_cache" => {
                tokens.as_object_mut().unwrap().remove("cached_input");
            }
            "missing_output" => {
                tokens.as_object_mut().unwrap().remove("output");
            }
            "bad_output" => tokens["output"] = "bad".into(),
            _ => tokens["output"] = llm_usage_core::domain::MAX_TOKEN_VALUE.into(),
        }
        value["turn_stats"][0]["model_usage"].as_array_mut().unwrap().push(serde_json::json!({"provider_id":"local-model","model_id":"valid-other","tokens":{"input":3,"cached_input":1,"output":2}}));
        if variant == "overflow_output" {
            value["detached_model_usage"] = serde_json::json!([{"provider_id":"local-model","model_id":"qwen2.5-0.5b-local","tokens":{"input":1,"cached_input":0,"output":1}}]);
        }
        let dir = TempDir::new("atomcode-partial");
        let (ctx, _file) = carrier(&dir, &value);
        let (_db, storage) = temp_storage("atomcode-partial");
        let reports =
            run_adapter_scan(&storage, &AtomCodeAdapter::new(), &ctx, &config(variant)).unwrap();
        assert!(reports[0].error.is_none(), "{variant}");
        let target = row(&storage);
        assert_eq!(
            target[0],
            if matches!(variant, "missing_input" | "missing_cache") {
                None
            } else if variant == "overflow_output" {
                Some(6177)
            } else {
                Some(6176)
            },
            "{variant}"
        );
        assert_eq!(
            target[1],
            if matches!(variant, "missing_input" | "missing_cache") {
                Some(2)
            } else {
                None
            },
            "{variant}"
        );
        assert_eq!(target[2], None);
        assert_eq!(target[3], None);
        assert_eq!(target[4], None);
        let valid:[Option<i64>;3]=storage.conn().query_row("SELECT input_total,output_total,total_tokens FROM source_aggregates WHERE scope_key LIKE '%valid-other'",[],|r|Ok([r.get(0)?,r.get(1)?,r.get(2)?])).unwrap();
        assert_eq!(valid, [Some(4), Some(2), Some(6)]);
        if matches!(variant, "bad_output" | "overflow_output") {
            assert!(count(&storage,"SELECT COUNT(*) FROM diagnostics WHERE code IN ('token_shape_deviation','token_sum_overflow')")>0);
        }
    }
}
