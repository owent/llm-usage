//! Source-derived fixtures only; no Qoder client or native request was executed.
mod common;
use common::*;
use llm_usage_core::{
    adapters::{built_in_adapters, framework::*, qoder::QoderAdapter},
    jobs::{RunStatus, TriggerKind},
    storage::Storage,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;
fn fixture() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/qoder/source-cli-1.1.76/session.jsonl"),
    )
    .unwrap()
}
fn save(root: &Path, data: &str) -> PathBuf {
    let file = root.join("projects/project/session.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, data).unwrap();
    file
}
fn context(root: &Path) -> DiscoverContext {
    DiscoverContext {
        manual_roots: vec![root.into()],
        ..Default::default()
    }
}
fn config(now: i64, lines: Option<u64>) -> RunConfig {
    let mut limits = ScanLimits::default();
    limits.jsonl.max_lines = lines;
    RunConfig {
        now_ms: now,
        timezone: "UTC".into(),
        trigger: TriggerKind::Manual,
        limits,
        origin_host_id: None,
        run_id_prefix: format!("qoder-{now}"),
    }
}
fn run(storage: &Storage, root: &Path, now: i64, lines: Option<u64>) -> Vec<SourceRunReport> {
    run_adapter_scan(
        storage,
        &QoderAdapter::new(),
        &context(root),
        &config(now, lines),
    )
    .unwrap()
}
fn count(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn registry_run(storage: &Storage, root: &Path, now: i64) {
    for adapter in built_in_adapters() {
        let mut cfg = config(now, None);
        cfg.run_id_prefix = format!("registry-{}-{now}", adapter.adapter_id());
        let reports = run_adapter_scan(storage, adapter.as_ref(), &context(root), &cfg).unwrap();
        assert!(
            reports.iter().all(|r| r.finish != RunStatus::Failed),
            "{reports:?}"
        );
    }
}
fn first() -> Value {
    serde_json::from_str(fixture().lines().nth(1).unwrap()).unwrap()
}

#[test]
fn source_fixture_full_registry_deduplicates_and_preserves_unknowns() {
    let dir = TempDir::new("qoder-source");
    let text = fixture();
    let file = save(dir.path(), &text);
    let (_db, storage) = temp_storage("qoder-source");
    for now in [NOW, NOW + 1] {
        registry_run(&storage, dir.path(), now);
    }
    let totals = summary(&storage, "2026-10-22", "2026-10-22").totals;
    assert_eq!(
        (
            totals.call_count,
            totals.input_total_known,
            totals.output_total_known,
            totals.cache_read_known,
            totals.cache_write_known,
            totals.total_tokens_known
        ),
        (4, Some(170), Some(36), Some(30), Some(10), Some(197))
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE agent!='qoder' OR conflict=1"
        ),
        0
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE file_id LIKE '%session.jsonl'"
        ),
        1
    );
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_uncached=80 AND input_total=120 AND total_tokens=140 AND time_basis='observed_at'"),1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total IS NULL AND output_total IS NULL AND total_tokens IS NULL AND quality_bucket='unknown'"),1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total=50 AND input_uncached IS NULL AND input_cache_read IS NULL AND call_category='sub_agent'"),1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE schema_version='1.1.99' AND parse_basis='latest_fallback' AND output_total=9 AND total_tokens IS NULL"),1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE provider_id IS NOT NULL OR cost_amount_minor IS NOT NULL OR output_reasoning IS NOT NULL OR source_total IS NOT NULL"),0);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE schema_version='1.1.76' AND parse_basis='known_version'"),3);
    assert_eq!(std::fs::read_to_string(file).unwrap(), text);
}

#[test]
fn old_probe_cursor_replays_through_registry_and_root_promotion() {
    let dir = TempDir::new("qoder-old-probe");
    let file = save(dir.path(), &(first().to_string() + "\n"));
    let (_db, storage) = temp_storage("qoder-old-probe");
    run(&storage, dir.path(), NOW, None);
    storage.conn().execute_batch("DELETE FROM usage_events; DELETE FROM daily_usage; DELETE FROM period_usage; UPDATE source_instances SET parser_version='qoder-probe-1'; UPDATE source_files SET status='degraded'; UPDATE ingestion_checkpoints SET parse_context=NULL; INSERT INTO diagnostics(batch_id,instance_id,code,message,created_ms) SELECT 'old-probe',instance_id,'usage_fields_unverified','retained probe diagnostic',1 FROM source_instances;").unwrap();
    registry_run(&storage, dir.path(), NOW + 1);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_fields_unverified'"
        ),
        1
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='active'"
        ),
        1
    );
    for root in [
        dir.path().join("projects"),
        file.parent().unwrap().to_path_buf(),
        file,
    ] {
        assert_eq!(
            QoderAdapter::new().discover(&context(&root))[0].files.len(),
            1
        );
    }
}

#[test]
fn bounded_windows_partial_lines_and_errors_retain_valid_output() {
    let dir = TempDir::new("qoder-fields");
    let mut row = first();
    row["message"]["usage"]["input_tokens"] = json!("invalid");
    let file = save(dir.path(), &(row.to_string() + "\n"));
    let (_db, storage) = temp_storage("qoder-fields");
    run(&storage, dir.path(), NOW, Some(1));
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE output_total=20 AND input_total IS NULL AND total_tokens IS NULL"),1);
    let second = first().to_string().replace("\"custom\"", "\"second\"");
    let mut bytes = std::fs::read(&file).unwrap();
    bytes.extend_from_slice(second.as_bytes());
    std::fs::write(&file, &bytes).unwrap();
    run(&storage, dir.path(), NOW + 1, Some(1));
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    bytes.push(b'\n');
    std::fs::write(file, bytes).unwrap();
    run(&storage, dir.path(), NOW + 2, Some(1));
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='degraded'"
        ),
        1
    );
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_shape_deviation'"
        ) > 0
    );
}

#[test]
fn isolated_ownership_formats_and_bad_envelopes_never_import_guesses() {
    for (field, value) in [
        ("entrypoint", json!("remote-control")),
        ("userType", json!("internal")),
        ("forkedFrom", json!({"sessionId":"parent"})),
        ("isSidechain", json!("true")),
        ("timestamp", json!("invalid")),
        ("sessionId", Value::Null),
    ] {
        let dir = TempDir::new("qoder-invalid");
        let mut row = first();
        row[field] = value;
        save(dir.path(), &(row.to_string() + "\n"));
        let (_db, storage) = temp_storage("qoder-invalid");
        run(&storage, dir.path(), NOW, None);
        assert_eq!(
            count(&storage, "SELECT COUNT(*) FROM usage_events"),
            0,
            "{field}"
        );
        assert!(
            count(&storage, "SELECT COUNT(*) FROM diagnostics") > 0,
            "{field}"
        );
    }
    let dir = TempDir::new("qoder-ide-state");
    save(dir.path(),"{\"type\":\"assistant\",\"sessionId\":\"ide\",\"message\":{\"content\":\"synthetic IDE text\"}}\n");
    std::fs::write(
        dir.path().join("projects/project/state.json"),
        "{\"modelRequests\":[]}",
    )
    .unwrap();
    let (_db, storage) = temp_storage("qoder-ide-state");
    run(&storage, dir.path(), NOW, None);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status IN ('incompatible','degraded')"
        ),
        2
    );
}

#[test]
fn checkpoint_failure_rolls_back_then_retry_and_real_conflict_remain_visible() {
    let dir = TempDir::new("qoder-rollback");
    let file = save(dir.path(), &(first().to_string() + "\n"));
    let (_db, storage) = temp_storage("qoder-rollback");
    storage.conn().execute_batch("CREATE TRIGGER fail_qoder_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
    let report = run(&storage, dir.path(), NOW, None);
    assert_eq!(report[0].finish, RunStatus::Failed);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(
        count(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
        0
    );
    storage
        .conn()
        .execute_batch("DROP TRIGGER fail_qoder_checkpoint")
        .unwrap();
    run(&storage, dir.path(), NOW + 1, None);
    run(&storage, dir.path(), NOW + 2, None);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    let mut changed = first();
    changed["message"]["usage"]["output_tokens"] = json!(99);
    std::fs::write(
        file,
        first().to_string() + "\n" + &changed.to_string() + "\n",
    )
    .unwrap();
    run(&storage, dir.path(), NOW + 3, None);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM usage_events WHERE conflict=1"
        ),
        1
    );
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'"
        ) > 0
    );
}

#[test]
fn partial_response_window_does_not_poison_later_valid_health() {
    let dir = TempDir::new("qoder-partial-window");
    let mut user = first();
    user["type"] = json!("user");
    user["message"]["role"] = json!("user");
    let rows =
        user.to_string() + "\n" + &fixture().lines().take(2).collect::<Vec<_>>().join("\n") + "\n";
    save(dir.path(), &rows);
    let (_db, storage) = temp_storage("qoder-partial-window");
    for n in 0..4 {
        run(&storage, dir.path(), NOW + n, Some(1));
    }
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code!='latest_fallback'"
        ),
        0
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='active'"
        ),
        1
    );
}

#[test]
fn empty_misrouted_owner_recovers_but_statistical_history_blocks_transfer() {
    use llm_usage_core::adapters::junie::JunieAdapter;
    for has_history in [false, true] {
        let dir = TempDir::new("qoder-misrouted");
        let original = save(dir.path(), &(first().to_string() + "\n"));
        let file = original.with_file_name("events.jsonl");
        std::fs::rename(original, &file).unwrap();
        let selected = file.parent().unwrap();
        let junie = JunieAdapter::new();
        let discovered = junie.discover(&context(selected));
        assert_eq!(discovered[0].root, selected.parent().unwrap());
        assert_eq!(discovered[0].files, vec![file.clone()]);
        let owner = junie.instance_id(&discovered[0]);
        let (_db, storage) = temp_storage("qoder-misrouted");
        run(&storage, selected, NOW, None);
        assert_eq!(
            count(&storage, "SELECT COUNT(*) FROM usage_events"),
            1,
            "initial Qoder import"
        );
        if !has_history {
            storage
                .conn()
                .execute_batch(
                    "DELETE FROM usage_events; DELETE FROM daily_usage; DELETE FROM period_usage;",
                )
                .unwrap();
        }
        storage.conn().execute("INSERT INTO source_instances(instance_id,agent,format,location_hint,locality_basis,attribution_status,parser_version,created_at_ms,updated_at_ms) VALUES(?1,'junie','junie',?2,'local_filesystem','verified','old',1,1)",rusqlite::params![owner,normalize_path(&discovered[0].root)]).unwrap();
        for sql in [
            "UPDATE source_files SET instance_id=?1,status='degraded'",
            "UPDATE ingestion_checkpoints SET instance_id=?1",
            "UPDATE usage_events SET source_instance_id=?1",
            "UPDATE daily_usage SET instance_id=?1",
            "UPDATE period_usage SET instance_id=?1",
        ] {
            storage.conn().execute(sql, [&owner]).unwrap();
        }
        let bad = discovered[0].root.join("other/events.jsonl");
        std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
        std::fs::write(&bad, "not JSON\n").unwrap();
        run_adapter_scan(&storage, &junie, &context(selected), &config(NOW + 1, None)).unwrap();
        registry_run(&storage, selected, NOW + 2);
        assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM source_files WHERE instance_id LIKE 'qoder@%'"
            ),
            if has_history { 0 } else { 1 }
        );
        let bad_status: String = storage
            .conn()
            .query_row(
                "SELECT status FROM source_files WHERE file_id=?1",
                [normalize_path(&bad)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bad_status, "unsupported");
        let health: String = storage
            .conn()
            .query_row(
                "SELECT health FROM source_instances WHERE instance_id=?1",
                [owner],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(health, "not_applicable");
        assert!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='unknown_format'"
            ) > 0
        );
    }
}

#[test]
fn default_and_environment_roots_find_the_same_source_derived_carrier() {
    let home = TempDir::new("qoder-home");
    let file = save(&home.path().join(".qoder"), &(first().to_string() + "\n"));
    let adapter = QoderAdapter::new();
    let ctx = DiscoverContext {
        home_dir: Some(home.path().into()),
        ..Default::default()
    };
    assert_eq!(adapter.discover(&ctx)[0].files, vec![file]);
    let custom = home.path().join("custom-config");
    let file = save(&custom, &(first().to_string() + "\n"));
    let ctx = DiscoverContext {
        env: std::collections::BTreeMap::from([(
            "QODER_CONFIG_DIR".into(),
            custom.to_string_lossy().into_owned(),
        )]),
        ..Default::default()
    };
    assert_eq!(adapter.discover(&ctx)[0].files, vec![file]);
}

#[test]
fn contradictory_cache_and_derived_total_cap_preserve_other_known_fields() {
    let dir = TempDir::new("qoder-contradiction");
    let mut row = first();
    row["message"]["usage"] = json!({"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":8,"cache_creation_input_tokens":7});
    let mut huge = first();
    huge["message"]["id"] = json!("huge");
    huge["message"]["usage"] =
        json!({"input_tokens":llm_usage_core::domain::MAX_TOKEN_VALUE,"output_tokens":5});
    save(
        dir.path(),
        &(row.to_string() + "\n" + &huge.to_string() + "\n"),
    );
    let (_db, storage) = temp_storage("qoder-contradiction");
    run(&storage, dir.path(), NOW, None);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total=10 AND input_uncached IS NULL AND output_total=5 AND total_tokens=15"),1);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE input_total>10 AND output_total=5 AND total_tokens IS NULL"),1);
    assert!(count(&storage,"SELECT COUNT(*) FROM diagnostics WHERE code IN ('cache_sum_exceeds_input_total','derived_total_out_of_range')")>=2);
}

#[test]
fn official_response_without_model_source_is_identified_by_native_credits_not_priced() {
    let dir = TempDir::new("qoder-official");
    let mut row = first();
    row.as_object_mut().unwrap().remove("modelSource");
    row["message"]["usage"] =
        json!({"input_tokens":0,"output_tokens":0,"credits":0.5,"billable":true});
    save(dir.path(), &(row.to_string() + "\n"));
    let (_db, storage) = temp_storage("qoder-official");
    registry_run(&storage, dir.path(), NOW);
    assert_eq!(count(&storage,"SELECT COUNT(*) FROM usage_events WHERE agent='qoder' AND input_total IS NULL AND output_total IS NULL AND provider_id IS NULL AND cost_amount_minor IS NULL"),1);
}

#[test]
fn indistinguishable_chat_only_files_keep_the_probe_and_malformed_forks_are_diagnosed() {
    let dir = TempDir::new("qoder-ambiguous");
    let mut row = first();
    row.as_object_mut().unwrap().remove("modelSource");
    save(dir.path(), &(row.to_string() + "\n"));
    let (_db, storage) = temp_storage("qoder-ambiguous");
    run(&storage, dir.path(), NOW, None);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_fields_unverified'"
        ) > 0
    );
    let dir = TempDir::new("qoder-fork-owner");
    let mut row = first();
    row["forkedFrom"] = json!({"sessionId":"source-session","messageUuid":"wrong-uuid"});
    save(dir.path(), &(row.to_string() + "\n"));
    let (_db, storage) = temp_storage("qoder-fork-owner");
    run(&storage, dir.path(), NOW, None);
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='ownership_unverified'"
        ) > 0
    );
}

#[test]
fn long_initial_user_and_metadata_prefix_route_through_the_full_registry() {
    for bom in [false, true] {
        let dir = TempDir::new("qoder-long-prefix");
        let mut user = first();
        user["type"] = json!("user");
        user["message"]["role"] = json!("user");
        user["message"]["content"] = json!("x".repeat(96 * 1024));
        user.as_object_mut().unwrap().remove("modelSource");
        let mut text = if bom { "\u{feff}" } else { "" }.to_string();
        text.push_str(&(user.to_string() + "\n"));
        for _ in 0..40 {
            text.push_str(
                "{\"type\":\"token-stats\",\"sessionId\":\"fixture\",\"promptTokenCount\":4000}\n",
            );
        }
        text.push_str(&(first().to_string() + "\n"));
        let file = save(dir.path(), &text);
        let (_db, storage) = temp_storage("qoder-long-prefix");
        registry_run(&storage, dir.path(), NOW);
        registry_run(&storage, dir.path(), NOW + 1);
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM usage_events WHERE agent='qoder' AND input_total=120"
            ),
            1
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM usage_events WHERE agent!='qoder' OR conflict=1"
            ),
            0
        );
        assert_eq!(std::fs::read_to_string(file).unwrap(), text);
    }
}

#[test]
fn metadata_only_first_window_reaches_usage_without_spurious_errors() {
    let dir = TempDir::new("qoder-metadata-window");
    let rows = format!(
        "{{\"type\":\"token-stats\",\"sessionId\":\"fixture\",\"promptTokenCount\":4000}}\n{}\n",
        first()
    );
    save(dir.path(), &rows);
    let (_db, storage) = temp_storage("qoder-metadata-window");
    for n in 0..4 {
        run(&storage, dir.path(), NOW + n, Some(1));
    }
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='active'"
        ),
        1
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_fields_unverified'"
        ),
        0
    );
}

#[test]
fn corrupt_first_window_keeps_its_diagnostic_and_allows_later_valid_calls() {
    let dir = TempDir::new("qoder-bad-first-window");
    save(dir.path(), &format!("{{broken JSON\n{}\n", first()));
    let (_db, storage) = temp_storage("qoder-bad-first-window");
    for n in 0..4 {
        run(&storage, dir.path(), NOW + n, Some(1));
    }
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='degraded'"
        ),
        1
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='invalid_json'"
        ),
        1
    );
}

#[test]
fn unchanged_old_reader_health_replays_without_replacing_history_or_conflicts() {
    for conflicting in [false, true] {
        let dir = TempDir::new("qoder-reader-upgrade");
        let file = save(dir.path(), &(first().to_string() + "\n"));
        let (db, storage) = temp_storage("qoder-reader-upgrade");
        run(&storage, dir.path(), NOW, None);
        storage.conn().execute_batch("UPDATE source_instances SET parser_version='qoder-transcript-1'; UPDATE usage_events SET parser_version='qoder-transcript-1'; UPDATE source_files SET status='degraded'; UPDATE ingestion_checkpoints SET parse_context=json_set(parse_context,'$.parser_version','qoder-transcript-1','$.has_errors',json('true')); INSERT INTO diagnostics(batch_id,instance_id,code,message,created_ms) SELECT 'old-reader',instance_id,'usage_fields_unverified','old metadata-only window warning',1 FROM source_instances;").unwrap();
        if conflicting {
            storage
                .conn()
                .execute_batch("UPDATE usage_events SET conflict=1;")
                .unwrap();
        }
        let original: (String, i64) = storage
            .conn()
            .query_row(
                "SELECT event_id,observed_at_ms FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        drop(storage);
        let storage = Storage::open(&db.db_path()).unwrap();
        registry_run(&storage, dir.path(), NOW + 1);
        registry_run(&storage, dir.path(), NOW + 2);
        let current: (String, i64, bool) = storage
            .conn()
            .query_row(
                "SELECT event_id,observed_at_ms,conflict FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((current.0, current.1), original);
        assert_eq!(current.2, conflicting);
        assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 1);
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM source_files WHERE status='active'"
            ),
            1
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='usage_fields_unverified'"
            ),
            1
        );
        assert_eq!(
            std::fs::read_to_string(file).unwrap(),
            first().to_string() + "\n"
        );
    }
}

#[test]
fn explicitly_selected_transcript_names_do_not_hide_records_or_corruption() {
    for name in ["subagents-session.jsonl", "segments-session.jsonl"] {
        let dir = TempDir::new("qoder-manual-name");
        let file = dir.path().join(name);
        std::fs::write(&file, first().to_string() + "\ninvalid JSON\n").unwrap();
        let (_db, storage) = temp_storage("qoder-manual-name");
        registry_run(&storage, &file, NOW);
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM usage_events WHERE agent='qoder'"
            ),
            1
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM source_files WHERE status='degraded'"
            ),
            1
        );
        assert_eq!(
            count(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='invalid_json'"
            ),
            1
        );
    }
}
