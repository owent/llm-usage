//! Genuine SDK source plus targeted mutation/transaction boundaries; old UI is separate.
mod common;

use common::{summary, temp_storage, TempDir};
use llm_usage_core::{
    adapters::{built_in_adapters, cline::ClineAdapter, framework::*},
    domain::VersionBasis,
    jobs::{RunStatus, TriggerKind},
    storage::Storage,
};
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

#[test]
fn oversized_carrier_is_visible_and_cannot_advance_a_checkpoint() {
    let source = TempDir::new("cline-sdk-oversized");
    let file = save(source.path(), &native());
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len(llm_usage_core::adapters::cline::versions::sdk_messages_v1::MAX_FILE_BYTES + 1)
        .unwrap();
    assert!(
        matches!(ClineAdapter::new().detect(&file).unwrap(), DetectOutcome::UnknownFormat { reason } if reason.contains("32 MiB"))
    );
    let (_db, storage) = temp_storage("cline-sdk-oversized");
    run(&storage, source.path(), NOW);
    assert_eq!(
        count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
        0
    );
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert!(count(&storage, "SELECT COUNT(*) FROM diagnostics") > 0);
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cline/real-vscode-4.1.22")
}
fn native() -> serde_json::Value {
    serde_json::from_slice(
        &std::fs::read(
            fixture().join("data/sessions/session-redacted/session-redacted.messages.json"),
        )
        .unwrap(),
    )
    .unwrap()
}
fn save(root: &Path, value: &serde_json::Value) -> PathBuf {
    let dir = root.join("data/sessions/session-redacted");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session-redacted.messages.json");
    std::fs::write(&file, serde_json::to_vec(value).unwrap()).unwrap();
    file
}
fn ctx(root: &Path) -> DiscoverContext {
    DiscoverContext {
        manual_roots: vec![root.to_path_buf()],
        ..Default::default()
    }
}
fn config(now: i64) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("cline-sdk-{now}"),
    }
}
fn run(storage: &Storage, root: &Path, now: i64) -> Vec<SourceRunReport> {
    run_adapter_scan(storage, &ClineAdapter::new(), &ctx(root), &config(now)).unwrap()
}
fn count(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn genuine_source_through_entire_registry_matches_api_and_repeats_without_mutation() {
    let (_db, storage) = temp_storage("cline-sdk-registry");
    let file = fixture().join("data/sessions/session-redacted/session-redacted.messages.json");
    let bytes = std::fs::read(&file).unwrap();
    for now in [NOW, NOW + 1] {
        for adapter in built_in_adapters() {
            run_adapter_scan(&storage, adapter.as_ref(), &ctx(&fixture()), &config(now)).unwrap();
        }
    }
    let api: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(fixture().join("api-usage.json")).unwrap()).unwrap();
    let s = summary(&storage, "2026-10-06", "2026-10-06");
    assert_eq!(
        s.totals.input_total_known,
        Some(
            api.iter()
                .map(|a| a["usage"]["prompt_tokens"].as_i64().unwrap())
                .sum()
        )
    );
    assert_eq!(s.totals.output_total_known, Some(48));
    assert_eq!(s.totals.total_tokens_known, Some(8970));
    assert_eq!(s.totals.cache_read_known, Some(5881));
    assert_eq!(s.totals.cache_write_known, None);
    assert_eq!(
        s.totals.call_count, 0,
        "run/retry metrics do not prove call count"
    );
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE record_kind='usage_observation' AND parse_basis='latest_fallback' AND model_raw='qwen3.5-0.8b-local' AND provider_id='lmstudio'"),3);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_uncached IS NULL AND input_cache_write IS NULL AND cost_amount_minor IS NULL"),3);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE instance_id LIKE 'cline@%'"
        ),
        1
    );
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}

#[test]
fn default_environment_precedence_and_all_manual_root_shapes_find_native_file() {
    let dir = TempDir::new("cline-sdk-discover");
    let home_root = dir.path().join(".cline");
    let file = save(&home_root, &native());
    let adapter = ClineAdapter::new();
    let context = DiscoverContext {
        home_dir: Some(dir.path().to_path_buf()),
        ..Default::default()
    };
    assert_eq!(adapter.discover(&context)[0].files, vec![file.clone()]);
    for root in [
        &home_root,
        &home_root.join("data"),
        &home_root.join("data/sessions"),
        file.parent().unwrap(),
        &file,
    ] {
        assert_eq!(adapter.discover(&ctx(root))[0].files, vec![file.clone()]);
    }
    let selected = dir.path().join("selected");
    let selected_file = save(&selected, &native());
    let context = DiscoverContext {
        home_dir: Some(dir.path().to_path_buf()),
        env: std::collections::BTreeMap::from([
            ("CLINE_DIR".into(), home_root.to_string_lossy().into_owned()),
            (
                "CLINE_DATA_DIR".into(),
                home_root.join("data").to_string_lossy().into_owned(),
            ),
            (
                "CLINE_SESSION_DATA_DIR".into(),
                selected
                    .join("data/sessions")
                    .to_string_lossy()
                    .into_owned(),
            ),
        ]),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&context);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].files, vec![selected_file]);
    assert_eq!(
        roots[0].basis,
        RootBasis::EnvOverride("CLINE_SESSION_DATA_DIR".into())
    );
}

#[test]
fn bad_fields_do_not_drop_valid_rows_and_zero_defaults_stay_unknown() {
    let source = TempDir::new("cline-sdk-fields");
    let mut value = native();
    let original = value["messages"][1].clone();
    value["messages"] = serde_json::json!([original]);
    let zero = serde_json::json!({"id":"zero","role":"assistant","ts":1791283929040i64,"metrics":{"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0}});
    let partial = serde_json::json!({"id":"partial","role":"assistant","ts":1791283929041i64,"metrics":{"inputTokens":10}});
    let messages = value["messages"].as_array_mut().unwrap();
    messages.push(zero);
    messages.push(partial);
    for bad in [
        serde_json::json!(-1),
        serde_json::json!("12"),
        serde_json::json!(1.5),
        serde_json::json!(null),
    ] {
        messages.push(serde_json::json!({"id":format!("bad-{}",messages.len()),"role":"assistant","ts":1791283929042i64,"metrics":{"inputTokens":bad,"outputTokens":2}}));
    }
    save(source.path(), &value);
    let (_db, storage) = temp_storage("cline-sdk-fields");
    run(&storage, source.path(), NOW);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_shape_deviation'"
        ),
        4
    );
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total IS NULL AND output_total IS NULL AND total_tokens IS NULL AND json_extract(quality_json,'$.input_total')='unknown'"),1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total=10 AND output_total IS NULL AND total_tokens IS NULL"),1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_instances WHERE health='degraded' AND format='cline'"
        ),
        1
    );
}

#[test]
fn incompatible_envelopes_and_empty_sessions_cannot_certify_support() {
    let source = TempDir::new("cline-sdk-envelope");
    let adapter = ClineAdapter::new();
    for (pointer, replacement) in [
        ("/version", serde_json::json!(2)),
        ("/origin/source", serde_json::json!("cli")),
        ("/origin/mode", serde_json::json!("import")),
        ("/agent", serde_json::json!("subagent")),
        ("/origin/sessionId", serde_json::json!("other")),
    ] {
        let mut value = native();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let file = save(source.path(), &value);
        assert!(matches!(
            adapter.detect(&file).unwrap(),
            DetectOutcome::UnknownFormat { .. }
        ));
    }
    let mut value = native();
    value["messages"] = serde_json::json!([]);
    let file = save(source.path(), &value);
    assert!(matches!(
        adapter.detect(&file).unwrap(),
        DetectOutcome::Pending
    ));
    value = native();
    value["origin"]["version"] = serde_json::json!("4.1.22");
    let file = save(source.path(), &value);
    assert!(matches!(
        adapter.detect(&file).unwrap(),
        DetectOutcome::Supported {
            basis: VersionBasis::LatestFallback,
            ..
        }
    ));
    value["origin"]["version"] = serde_json::json!("999.0.0");
    let file = save(source.path(), &value);
    assert!(matches!(
        adapter.detect(&file).unwrap(),
        DetectOutcome::Supported {
            basis: VersionBasis::LatestFallback,
            ..
        }
    ));
}

#[test]
fn messages_without_usage_and_display_only_metrics_are_excluded() {
    let source = TempDir::new("cline-sdk-display");
    let mut value = native();
    let mut display = value["messages"][1].clone();
    display["id"] = "display".into();
    display["metadata"] = serde_json::json!({"displayOnly":true});
    let mut no_metrics = display.clone();
    no_metrics["id"] = "no-metrics".into();
    no_metrics.as_object_mut().unwrap().remove("metrics");
    no_metrics.as_object_mut().unwrap().remove("metadata");
    value["messages"]
        .as_array_mut()
        .unwrap()
        .extend([display, no_metrics]);
    save(source.path(), &value);
    let (_db, storage) = temp_storage("cline-sdk-display");
    run(&storage, source.path(), NOW);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
}

#[test]
fn duplicate_ids_conflict_instead_of_double_counting_and_removed_history_is_retained() {
    let source = TempDir::new("cline-sdk-rewrite");
    let mut value = native();
    let duplicate = value["messages"][1].clone();
    value["messages"].as_array_mut().unwrap().push(duplicate);
    save(source.path(), &value);
    let (_db, storage) = temp_storage("cline-sdk-rewrite");
    run(&storage, source.path(), NOW);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
    value["messages"].as_array_mut().unwrap().reverse();
    let duplicate = value["messages"][0].clone();
    value["messages"] = serde_json::json!([duplicate]);
    save(source.path(), &value);
    run(&storage, source.path(), NOW + 1);
    assert_eq!(
        summary(&storage, "2026-10-06", "2026-10-06")
            .totals
            .total_tokens_known,
        Some(8970)
    );
    value["messages"][0]["metrics"]["inputTokens"] = serde_json::json!(2916);
    save(source.path(), &value);
    run(&storage, source.path(), NOW + 2);
    assert_eq!(
        summary(&storage, "2026-10-06", "2026-10-06")
            .totals
            .total_tokens_known,
        Some(8970)
    );
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'"
        ) > 0
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE conflict=1"
        ),
        1
    );
}

#[test]
fn contradictory_cache_is_visible_without_clamping_or_corrupting_other_usage() {
    let source = TempDir::new("cline-sdk-cache-conflict");
    let mut value = native();
    value["messages"][1]["metrics"] = serde_json::json!({"inputTokens":10,"outputTokens":2,"cacheReadTokens":8,"cacheWriteTokens":5});
    save(source.path(), &value);
    let (_db, storage) = temp_storage("cline-sdk-cache-conflict");
    run(&storage, source.path(), NOW);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total=10 AND input_cache_read=8 AND input_cache_write=5 AND input_uncached IS NULL AND total_tokens=12"),1);
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='cache_sum_exceeds_input_total'"
        ) > 0
    );
}

#[test]
fn partial_write_does_not_advance_checkpoint_and_restored_native_recovers() {
    let source = TempDir::new("cline-sdk-half-write");
    let file = save(source.path(), &native());
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
    let (_db, storage) = temp_storage("cline-sdk-half-write");
    run(&storage, source.path(), NOW);
    assert_eq!(
        count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
        0
    );
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    std::fs::write(file, bytes).unwrap();
    run(&storage, source.path(), NOW + 1);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
    assert_eq!(
        count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
        1
    );
}

#[test]
fn checkpoint_failure_rolls_back_usage_then_parallel_retry_commits_once() {
    let source = TempDir::new("cline-sdk-rollback");
    let file = save(source.path(), &native());
    let bytes = std::fs::read(&file).unwrap();
    let (_db, storage) = temp_storage("cline-sdk-rollback");
    storage.conn().execute_batch("CREATE TRIGGER fail_cline_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
    let reports = run(&storage, source.path(), NOW);
    assert_eq!(reports[0].finish, RunStatus::Failed);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(
        count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
        0
    );
    storage
        .conn()
        .execute_batch("DROP TRIGGER fail_cline_checkpoint")
        .unwrap();
    let adapter = ClineAdapter::new();
    let requests = [ParallelScanRequest {
        adapter: &adapter,
        context: ctx(source.path()),
        config: config(NOW + 1),
        filter: InstanceFilter::default(),
    }];
    let locked = std::sync::Mutex::new(storage);
    for result in
        run_adapter_scans_parallel(&locked, &requests, None, std::sync::Arc::new(|| true), None)
    {
        result.unwrap();
    }
    let storage = locked.into_inner().unwrap();
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
    run(&storage, source.path(), NOW + 2);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 3);
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}
