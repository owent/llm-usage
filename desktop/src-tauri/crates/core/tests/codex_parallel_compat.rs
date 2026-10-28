//! Parallel collection must retain Codex's validated, non-counting inherited records.
mod common;
use common::*;
use llm_usage_core::{
    adapters::{built_in_adapters, framework::*},
    jobs::{RunStatus, TriggerKind},
    storage::Storage,
};
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

fn sources(dir: &TempDir, malformed: bool) -> std::path::PathBuf {
    let root = dir.path().join(".codex");
    let day_dir = root.join("sessions/2026/10/28");
    std::fs::create_dir_all(&day_dir).unwrap();
    let usage = json!({"input_tokens":100,"cached_input_tokens":80,"cache_write_input_tokens":0,
        "output_tokens":20,"reasoning_output_tokens":5,"total_tokens":120});
    for (index, version) in ["0.164.0-alpha.11.1", "0.165.0-alpha.1.2"]
        .iter()
        .enumerate()
    {
        let rows = [
            json!({"type":"session_meta","payload":{"id":format!("child-{index}"),"cli_version":version}}),
            json!({"timestamp":"2026-10-28T00:00:00Z","type":"compacted","payload":{
                "latest_token_usage_record":{"thread_id":"owner","session_id":"owner","response_id":"native-call","usage":usage}}}),
            json!({"type":"world_state","payload":{"full":true,"state":{}}}),
            json!({"type":"turn_context","payload":{"model":"synthetic-model"}}),
            json!({"timestamp":"2026-10-28T00:00:00Z","type":"event_msg","payload":{
                "type":"token_count","info":{"total_token_usage":usage}}}),
            json!({"type":"event_msg","payload":{"type":"thread_settings_applied"}}),
            json!({"type":"event_msg","payload":{"type":"task_started"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete"}}),
        ];
        let text = rows.iter().map(|v| format!("{v}\n")).collect::<String>();
        std::fs::write(day_dir.join(format!("rollout-child-{index}.jsonl")), text).unwrap();
    }
    if malformed {
        let text = format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":"bad","cli_version":"0.165.0-alpha.1.2"}}),
            json!({"timestamp":"2026-10-28T00:00:00Z","type":"compacted","payload":{
                "latest_token_usage_record":{"thread_id":"owner","response_id":"bad","usage":{"input_tokens":100}}}})
        );
        std::fs::write(day_dir.join("rollout-bad.jsonl"), text).unwrap();
    }
    root
}

fn config(now: i64) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("parallel-compat-{now}"),
    }
}

fn parallel(storage: &Mutex<Storage>, root: &Path, now: i64) -> Vec<SourceRunReport> {
    let adapters = built_in_adapters();
    let requests = adapters
        .iter()
        .map(|adapter| ParallelScanRequest {
            adapter: adapter.as_ref(),
            context: DiscoverContext {
                home_dir: Some(root.parent().unwrap().into()),
                ..Default::default()
            },
            config: RunConfig {
                run_id_prefix: format!("parallel-compat-{now}-{}", adapter.adapter_id()),
                ..config(now)
            },
            filter: InstanceFilter::default(),
        })
        .collect::<Vec<_>>();
    run_adapter_scans_parallel(storage, &requests, None, Arc::new(|| true), None)
        .into_iter()
        .flat_map(|result| result.unwrap())
        .collect()
}

fn count(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn parallel_registry_accepts_inherited_only_files_without_counting_tokens() {
    let dir = TempDir::new("codex-parallel-inherited");
    let root = sources(&dir, true);
    let (_db, storage) = temp_storage("codex-parallel-inherited");
    let storage = Mutex::new(storage);
    let reports = parallel(&storage, &root, 1000);
    assert!(
        reports.iter().all(|r| r.finish == RunStatus::Succeeded),
        "reports: {reports:?}"
    );
    assert_eq!(
        reports
            .iter()
            .filter(|r| r.instance_id.starts_with("codex@"))
            .count(),
        1
    );
    let storage = storage.lock().unwrap();
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='active_compat'"
        ),
        2,
        "reports: {reports:?}"
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM source_files WHERE status='incompatible'"
        ),
        1
    );
    assert_eq!(count(&storage, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(
        summary(&storage, "2026-10-28", "2026-10-28")
            .totals
            .total_tokens_known,
        None
    );
    assert_eq!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='inherited_compaction_usage'"
        ),
        2
    );
    assert!(
        count(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='usage_shape_deviation'"
        ) > 0
    );
}

fn recover_unchanged_incompatible_files(fail_commit: bool) {
    let dir = TempDir::new("codex-parallel-old-state");
    let root = sources(&dir, false);
    let db = TempDir::new("codex-parallel-old-state-db");
    let history;
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        let ctx = DiscoverContext {
            manual_roots: vec![root.clone()],
            ..Default::default()
        };
        run_adapter_scan(
            &storage,
            &llm_usage_core::adapters::codex::CodexAdapter,
            &ctx,
            &config(1000),
        )
        .unwrap();
        // The old parallel wrapper rejected these files without persisting their cursors.
        storage.conn().execute_batch("UPDATE source_files SET status='incompatible'; DELETE FROM ingestion_checkpoints;
            INSERT INTO diagnostics(code,message,created_ms) VALUES ('prior_diagnostic','retained',1);").unwrap();
        history = count(&storage, "SELECT COUNT(*) FROM diagnostics");
    }
    let storage = Mutex::new(Storage::open(&db.db_path()).unwrap());
    if fail_commit {
        storage.lock().unwrap().conn().execute_batch("CREATE TRIGGER reject_compat BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'synthetic compatibility failure'); END;").unwrap();
        let reports = parallel(&storage, &root, 2000);
        assert_eq!(reports[0].finish, RunStatus::Failed);
        let s = storage.lock().unwrap();
        assert_eq!(count(&s,"SELECT COUNT(*) FROM source_files WHERE status='incompatible' AND last_seen_ms=1000"), 2);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM diagnostics"), history);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
        s.conn()
            .execute_batch("DROP TRIGGER reject_compat")
            .unwrap();
    }
    let reports = parallel(&storage, &root, 3000);
    assert_eq!(reports[0].finish, RunStatus::Succeeded);
    {
        let s = storage.lock().unwrap();
        assert_eq!(
            count(
                &s,
                "SELECT COUNT(*) FROM source_files WHERE status='incompatible'"
            ),
            0
        );
        assert_eq!(count(&s,"SELECT COUNT(*) FROM source_files WHERE status='active_compat' AND first_seen_ms=1000"), 2);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM usage_events"), 0);
        assert_eq!(
            count(
                &s,
                "SELECT COUNT(*) FROM diagnostics WHERE code='prior_diagnostic'"
            ),
            1
        );
        assert!(count(&s, "SELECT COUNT(*) FROM diagnostics") >= history);
        assert_eq!(count(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(cursor_value,'$.offset')>0"), 2);
    }
    let repeated = parallel(&storage, &root, 4000);
    assert!(repeated[0].files.iter().all(|f| f.status == "unchanged"));
    assert_eq!(
        count(
            &storage.lock().unwrap(),
            "SELECT COUNT(*) FROM usage_events"
        ),
        0
    );
}

#[test]
fn parallel_registry_recovers_unchanged_incompatible_files_after_reopen() {
    recover_unchanged_incompatible_files(false);
}

#[test]
fn parallel_compatibility_recovery_rolls_back_and_retains_diagnostic_history() {
    recover_unchanged_incompatible_files(true);
}
