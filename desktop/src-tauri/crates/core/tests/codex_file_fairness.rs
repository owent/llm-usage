//! A large historical rollout must not postpone today's usage behind every read window.
mod common;
use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use llm_usage_core::jobs::RunStatus;
use llm_usage_core::storage::Storage;
use serde_json::json;

fn write_rollout(root: &std::path::Path, day: &str, name: &str, padding_lines: usize) {
    let dir = root.join("sessions/2026/10").join(day);
    std::fs::create_dir_all(&dir).unwrap();
    let meta = json!({"type":"session_meta", "payload":{
        "id":name, "cli_version":"0.162.0-alpha.2"}});
    let call = json!({"type":"token_usage_record", "timestamp":"2026-10-07T02:00:00Z",
        "payload":{"response_id":name,"usage":{"input_tokens":100,
        "cached_input_tokens":50,"cache_write_input_tokens":0,"output_tokens":20,
        "reasoning_output_tokens":5,"total_tokens":120}}});
    let mut text = format!("{meta}\n{call}\n");
    for _ in 0..padding_lines {
        text.push_str("{\"type\":\"response_item\",\"payload\":{}}\n");
    }
    std::fs::write(dir.join(format!("rollout-{name}.jsonl")), text).unwrap();
}

fn limits() -> ScanLimits {
    let mut limits = ScanLimits::default();
    // Deterministic stand-in for the default 32 MiB file read window.
    limits.jsonl.max_lines = Some(3);
    limits
}

#[test]
fn changed_creation_fingerprint_is_saved_so_old_cursors_can_finish_after_reopening() {
    let dir = TempDir::new("codex-created-fingerprint");
    let usage = json!({"input_tokens":100,"cached_input_tokens":50,"cache_write_input_tokens":0,
        "output_tokens":20,"reasoning_output_tokens":5,"total_tokens":120});
    let mut contents = format!(
        "{}\n",
        json!({"type":"session_meta","payload":{
        "id":"synthetic-created", "cli_version":"0.162.0-alpha.2"}})
    );
    for index in 0..3 {
        contents.push_str(&format!(
            "{}\n",
            json!({"type":"token_usage_record",
            "timestamp":"2026-10-07T02:00:00Z", "payload":{
                "response_id":format!("synthetic-{index}"), "usage":usage}})
        ));
    }
    let root = codex_root_with_file(&dir, "rollout-created.jsonl", contents.as_bytes());
    let path = root.join("sessions/2026/09/24/rollout-created.jsonl");
    let probe = llm_usage_core::adapters::jsonl::probe_file(&path).unwrap();
    let db = TempDir::new("codex-created-fingerprint-db");
    let now = ts("2026-10-07T04:00:00Z");
    let mut scan_limits = limits();
    scan_limits.jsonl.max_lines = Some(2);
    let generation;
    let fingerprint;
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        run_codex_with_limits(&storage, &root, now, scan_limits.clone());
        assert_eq!(
            summary(&storage, "2026-10-07", "2026-10-07")
                .totals
                .call_count,
            1
        );
        fingerprint = storage
            .conn()
            .query_row::<String, _, _>("SELECT content_hash FROM source_files", [], |r| r.get(0))
            .unwrap();
        // Without birthtime, a stale prefix forces a rescan that must clear the old creation time.
        let old_head = probe.head_hash ^ u64::from(probe.created_ms.is_none());
        let old_created = match probe.created_ms {
            Some(0) => 1,
            Some(created) => created - 1,
            None => 0,
        };
        let old = format!(
            "{old_head}:{}:{}:{old_created}",
            probe.head_len, probe.tail_hash
        );
        storage
            .conn()
            .execute("UPDATE source_files SET content_hash=?1", [old])
            .unwrap();
        generation = storage
            .conn()
            .query_row::<i64, _, _>("SELECT generation FROM source_files", [], |r| r.get(0))
            .unwrap();
        run_codex_with_limits(&storage, &root, now + 1, scan_limits.clone());
        assert_eq!(
            summary(&storage, "2026-10-07", "2026-10-07")
                .totals
                .call_count,
            1
        );
    }
    let storage = Storage::open(&db.db_path()).unwrap();
    let saved = storage
        .conn()
        .query_row::<(String, i64), _, _>(
            "SELECT content_hash, generation FROM source_files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(saved, (fingerprint, generation + 1));
    run_codex_with_limits(&storage, &root, now + 2, scan_limits.clone());
    let totals = summary(&storage, "2026-10-07", "2026-10-07").totals;
    assert_eq!(
        (totals.call_count, totals.total_tokens_known),
        (3, Some(360))
    );
    run_codex_with_limits(&storage, &root, now + 3, scan_limits);
    assert_eq!(summary(&storage, "2026-10-07", "2026-10-07").totals, totals);
    let repeated = storage
        .conn()
        .query_row::<(String, i64), _, _>(
            "SELECT content_hash, generation FROM source_files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(repeated, saved);
    assert_eq!(std::fs::read(path).unwrap(), contents.as_bytes());
}

#[test]
fn today_is_read_before_an_unconsumed_historical_window() {
    let dir = TempDir::new("codex-newest-first");
    write_rollout(dir.path(), "05", "historical", 15);
    write_rollout(dir.path(), "07", "today", 0);
    let (_db, storage) = temp_storage("codex-newest-first");
    let now = ts("2026-10-07T04:00:00Z");
    let reports = run_codex_with_limits(&storage, dir.path(), now, limits());
    assert_eq!(reports[0].finish, RunStatus::Interrupted);
    assert_eq!(reports[0].files.len(), 2);
    assert!(reports[0].files[0].file_id.ends_with("rollout-today.jsonl"));
    assert_eq!(
        summary(&storage, "2026-10-07", "2026-10-07")
            .totals
            .call_count,
        2
    );
    assert_eq!(
        summary(&storage, "2026-10-07", "2026-10-07")
            .totals
            .total_tokens_known,
        Some(240)
    );
}

#[test]
fn partial_file_yields_to_unvisited_files_after_reopening_database() {
    let dir = TempDir::new("codex-file-rotation");
    write_rollout(dir.path(), "06", "older-small", 0);
    write_rollout(dir.path(), "07", "latest-large", 15);
    let db = TempDir::new("codex-file-rotation-db");
    let now = ts("2026-10-07T04:00:00Z");
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        let first = run_codex_with_limits(&storage, dir.path(), now, limits());
        assert_eq!(first[0].finish, RunStatus::Interrupted);
        assert_eq!(first[0].files.len(), 1);
        assert!(first[0].files[0]
            .file_id
            .ends_with("rollout-latest-large.jsonl"));
        assert_eq!(
            summary(&storage, "2026-10-07", "2026-10-07")
                .totals
                .call_count,
            1
        );
    }
    let storage = Storage::open(&db.db_path()).unwrap();
    let next = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
    assert!(next[0].files[0]
        .file_id
        .ends_with("rollout-older-small.jsonl"));
    assert_eq!(next[0].finish, RunStatus::Interrupted);
    let totals = summary(&storage, "2026-10-07", "2026-10-07").totals;
    assert_eq!(
        (totals.call_count, totals.total_tokens_known),
        (2, Some(240))
    );
    for n in 2..10 {
        run_codex_with_limits(&storage, dir.path(), now + n, limits());
    }
    assert_eq!(summary(&storage, "2026-10-07", "2026-10-07").totals, totals);
    assert_eq!(
        run_codex(&storage, dir.path(), now + 11)[0].finish,
        RunStatus::Succeeded
    );
}

#[test]
fn appended_model_switch_is_read_before_unvisited_historical_windows() {
    check_appended_model_switch(0);
}

#[test]
fn incomplete_appended_window_yields_to_historical_files_on_the_next_run() {
    check_appended_model_switch(15);
}

fn check_appended_model_switch(padding_lines: usize) {
    use std::io::Write;

    let dir = TempDir::new("codex-appended-model");
    write_rollout(dir.path(), "06", "active", 0);
    let db = TempDir::new("codex-appended-model-db");
    let now = ts("2026-10-07T04:00:00Z");
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        assert_eq!(
            run_codex(&storage, dir.path(), now)[0].finish,
            RunStatus::Succeeded
        );
    }
    // Older, never visited windows reproduce a database still catching up with history.
    for day in ["01", "02", "03"] {
        write_rollout(dir.path(), day, day, 15);
    }
    let path = dir.path().join("sessions/2026/10/06/rollout-active.jsonl");
    let model = json!({"type":"turn_context","payload":{"model":"gpt-6-astra"}});
    let call = json!({"type":"token_usage_record","timestamp":"2026-10-07T03:00:00Z",
        "payload":{"response_id":"astra-call","usage":{"input_tokens":100,
        "cached_input_tokens":50,"cache_write_input_tokens":0,"output_tokens":20,
        "reasoning_output_tokens":5,"total_tokens":120}}});
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(file, "{model}\n{call}").unwrap();
    for _ in 0..padding_lines {
        writeln!(file, "{{\"type\":\"response_item\",\"payload\":{{}}}}").unwrap();
    }
    drop(file);
    let storage = Storage::open(&db.db_path()).unwrap();
    let reports = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
    assert!(reports[0].files[0]
        .file_id
        .ends_with("rollout-active.jsonl"));
    let astra: (i64, i64) = storage
        .conn()
        .query_row(
            "SELECT COUNT(*),SUM(total_tokens) FROM usage_events WHERE model_raw='gpt-6-astra'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(astra, (1, 120));
    assert_eq!(reports[0].finish, RunStatus::Interrupted);
    // An unfinished bounded window yields even after further appends.
    if padding_lines > 0 {
        append_text(&path, "{\"type\":\"response_item\",\"payload\":{}}\n");
    }
    let next = run_codex_with_limits(&storage, dir.path(), now + 2, limits());
    assert!(!next[0].files[0].file_id.ends_with("rollout-active.jsonl"));
    assert_eq!(
        storage
            .conn()
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM usage_events WHERE model_raw='gpt-6-astra'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn parallel_desktop_entry_preserves_codex_file_rotation() {
    use llm_usage_core::adapters::codex::CodexAdapter;
    use llm_usage_core::adapters::framework::{
        run_adapter_scans_parallel, DiscoverContext, InstanceFilter, ParallelScanRequest, RunConfig,
    };
    use llm_usage_core::jobs::TriggerKind;
    use std::sync::{Arc, Mutex};

    let dir = TempDir::new("codex-parallel-rotation");
    write_rollout(dir.path(), "06", "older-small", 0);
    write_rollout(dir.path(), "07", "latest-large", 15);
    let (_db, storage) = temp_storage("codex-parallel-rotation");
    let storage = Mutex::new(storage);
    let adapter = CodexAdapter::new();
    let now = ts("2026-10-07T04:00:00Z");
    for n in 0..2 {
        let request = ParallelScanRequest {
            adapter: &adapter,
            context: DiscoverContext {
                manual_roots: vec![dir.path().to_path_buf()],
                ..Default::default()
            },
            config: RunConfig {
                timezone: "UTC".into(),
                now_ms: now + n,
                limits: limits(),
                trigger: TriggerKind::Manual,
                origin_host_id: None,
                run_id_prefix: format!("parallel-rotation-{n}"),
            },
            filter: InstanceFilter::default(),
        };
        let results =
            run_adapter_scans_parallel(&storage, &[request], None, Arc::new(|| true), None);
        let report = &results[0].as_ref().unwrap()[0];
        assert_eq!(report.finish, RunStatus::Interrupted);
        let expected_file = if n == 0 {
            "latest-large"
        } else {
            "older-small"
        };
        assert!(report.files[0]
            .file_id
            .ends_with(&format!("rollout-{expected_file}.jsonl")));
    }
    assert_eq!(
        summary(&storage.lock().unwrap(), "2026-10-07", "2026-10-07")
            .totals
            .call_count,
        2
    );
}

fn append_text(path: &std::path::Path, text: &str) {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

fn astra_record() -> String {
    format!(
        "{}\n{}\n",
        json!({"type":"turn_context","payload":{"model":"gpt-6-astra"}}),
        json!({"type":"token_usage_record","timestamp":"2026-10-07T03:00:00Z",
        "payload":{"response_id":"new-astra","usage":{"input_tokens":100,
        "cached_input_tokens":50,"cache_write_input_tokens":0,"output_tokens":20,
        "reasoning_output_tokens":5,"total_tokens":120}}})
    )
}

#[test]
#[cfg(any(windows, unix))]
fn appended_usage_uses_the_original_path_when_its_identity_is_not_a_filesystem_path() {
    let dir = TempDir::new("codex-native-path-priority");
    #[cfg(windows)]
    let root = std::fs::canonicalize(dir.path())
        .unwrap()
        .join("verbatim-trailing-dot.");
    #[cfg(unix)]
    let root = dir.path().join(r"literal\backslash");
    write_rollout(&root, "06", "active", 0);
    let path = root.join("sessions/2026/10/06/rollout-active.jsonl");
    let identity = llm_usage_core::adapters::framework::normalize_path(&path);
    assert!(std::fs::metadata(&path).is_ok());
    assert!(std::fs::metadata(&identity).is_err());
    let db = TempDir::new("codex-native-path-priority-db");
    let now = ts("2026-10-07T04:00:00Z");
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        assert_eq!(
            run_codex(&storage, &root, now)[0].finish,
            RunStatus::Succeeded
        );
        assert_eq!(
            storage
                .conn()
                .query_row::<String, _, _>("SELECT file_id FROM source_files", [], |row| row.get(0))
                .unwrap(),
            identity
        );
    }
    write_rollout(&root, "02", "history", 15);
    write_rollout(&root, "01", "older-history", 15);
    append_text(&path, &astra_record());
    let storage = Storage::open(&db.db_path()).unwrap();
    let reports = run_codex_with_limits(&storage, &root, now + 1, limits());
    assert_eq!(reports[0].files[0].file_id, identity);
    assert_eq!(
        storage
            .conn()
            .query_row::<(i64, i64), _, _>(
                "SELECT COUNT(*),SUM(total_tokens) FROM usage_events WHERE model_raw='gpt-6-astra'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?))
            )
            .unwrap(),
        (1, 120)
    );
    let repeated = run_codex_with_limits(&storage, &root, now + 2, limits());
    assert!(repeated[0].files[0]
        .file_id
        .ends_with("rollout-older-history.jsonl"));
    assert_eq!(
        storage
            .conn()
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM usage_events WHERE model_raw='gpt-6-astra'",
                [],
                |row| row.get(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn completed_partial_line_is_prioritized_after_database_reopen() {
    let dir = TempDir::new("codex-partial-tail-priority");
    write_rollout(dir.path(), "06", "active", 0);
    let path = dir.path().join("sessions/2026/10/06/rollout-active.jsonl");
    let appended = astra_record();
    append_text(&path, &appended[..appended.len() - 1]);
    let db = TempDir::new("codex-partial-tail-priority-db");
    let now = ts("2026-10-07T04:00:00Z");
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        assert_eq!(
            run_codex(&storage, dir.path(), now)[0].finish,
            RunStatus::Succeeded
        );
        assert_eq!(
            summary(&storage, "2026-10-07", "2026-10-07")
                .totals
                .call_count,
            1
        );
    }
    write_rollout(dir.path(), "01", "history", 15);
    append_text(&path, "\n");
    let storage = Storage::open(&db.db_path()).unwrap();
    let report = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
    assert!(report[0].files[0].file_id.ends_with("rollout-active.jsonl"));
    assert_eq!(
        storage
            .conn()
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM usage_events WHERE model_raw='gpt-6-astra'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        std::fs::read_to_string(path)
            .unwrap()
            .matches("new-astra")
            .count(),
        1
    );
}

#[test]
fn missing_saved_mtime_does_not_promote_an_unchanged_file() {
    let dir = TempDir::new("codex-unknown-mtime");
    write_rollout(dir.path(), "06", "active", 0);
    let (_db, storage) = temp_storage("codex-unknown-mtime");
    let now = ts("2026-10-07T04:00:00Z");
    run_codex(&storage, dir.path(), now);
    storage
        .conn()
        .execute("UPDATE source_files SET mtime_ms=NULL", [])
        .unwrap();
    write_rollout(dir.path(), "01", "history", 15);
    let report = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
    assert!(report[0].files[0]
        .file_id
        .ends_with("rollout-history.jsonl"));
    assert_eq!(
        summary(&storage, "2026-10-07", "2026-10-07")
            .totals
            .call_count,
        2
    );
}

#[test]
fn same_size_rewrite_with_changed_mtime_is_prioritized() {
    let dir = TempDir::new("codex-mtime-rewrite-priority");
    write_rollout(dir.path(), "06", "active", 0);
    let path = dir.path().join("sessions/2026/10/06/rollout-active.jsonl");
    let (_db, storage) = temp_storage("codex-mtime-rewrite-priority");
    let now = ts("2026-10-07T04:00:00Z");
    run_codex(&storage, dir.path(), now);
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let original = std::fs::read_to_string(&path).unwrap();
    let changed = original.replace("\"response_id\":\"active\"", "\"response_id\":\"second\"");
    assert_ne!(changed, original);
    assert_eq!(changed.len(), original.len());
    std::fs::write(&path, &changed).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new().set_modified(mtime + std::time::Duration::from_secs(10)),
        )
        .unwrap();
    write_rollout(dir.path(), "01", "history", 15);
    let report = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
    assert!(report[0].files[0].file_id.ends_with("rollout-active.jsonl"));
    assert_eq!(storage.conn().query_row::<i64,_,_>("SELECT COUNT(*) FROM usage_events WHERE source_record_key IN ('resp:active','resp:second')",[],|r|r.get(0)).unwrap(),2);
}

#[test]
fn old_cursor_without_eof_marker_still_prioritizes_growth_with_unknown_mtime() {
    let dir = TempDir::new("codex-old-cursor-priority");
    write_rollout(dir.path(), "06", "active", 0);
    let (_db, storage) = temp_storage("codex-old-cursor-priority");
    let now = ts("2026-10-07T04:00:00Z");
    run_codex(&storage, dir.path(), now);
    storage.conn().execute_batch("UPDATE ingestion_checkpoints SET cursor_value=json_remove(cursor_value,'$.file_read_complete'); UPDATE source_files SET mtime_ms=NULL;").unwrap();
    append_text(
        &dir.path().join("sessions/2026/10/06/rollout-active.jsonl"),
        &astra_record(),
    );
    write_rollout(dir.path(), "01", "history", 15);
    let report = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
    assert!(report[0].files[0].file_id.ends_with("rollout-active.jsonl"));
    assert_eq!(
        storage
            .conn()
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM usage_events WHERE model_raw='gpt-6-astra'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn failed_priority_batch_preserves_visits_and_retries_after_reopen() {
    let dir = TempDir::new("codex-priority-rollback");
    write_rollout(dir.path(), "06", "active", 0);
    let path = dir.path().join("sessions/2026/10/06/rollout-active.jsonl");
    let db = TempDir::new("codex-priority-rollback-db");
    let now = ts("2026-10-07T04:00:00Z");
    {
        let storage = Storage::open(&db.db_path()).unwrap();
        run_codex(&storage, dir.path(), now);
        let prior: (String,i64,String)=storage.conn().query_row("SELECT f.content_hash,f.last_seen_ms,c.cursor_value FROM source_files f JOIN ingestion_checkpoints c ON c.instance_id=f.instance_id AND c.scope_key=f.file_identity",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        let revision = storage.data_revision().unwrap();
        append_text(&path, &astra_record());
        write_rollout(dir.path(), "01", "history", 15);
        storage.conn().execute_batch("CREATE TRIGGER reject_priority BEFORE INSERT ON usage_events BEGIN SELECT RAISE(ABORT,'synthetic priority failure'); END").unwrap();
        let report = run_codex_with_limits(&storage, dir.path(), now + 1, limits());
        assert_eq!(report[0].finish, RunStatus::Failed);
        let after: (String,i64,String)=storage.conn().query_row("SELECT f.content_hash,f.last_seen_ms,c.cursor_value FROM source_files f JOIN ingestion_checkpoints c ON c.instance_id=f.instance_id AND c.scope_key=f.file_identity",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(prior, after);
        assert_eq!(storage.data_revision().unwrap(), revision);
        storage
            .conn()
            .execute_batch("DROP TRIGGER reject_priority")
            .unwrap();
    }
    let storage = Storage::open(&db.db_path()).unwrap();
    let report = run_codex_with_limits(&storage, dir.path(), now + 2, limits());
    assert!(report[0].files[0].file_id.ends_with("rollout-active.jsonl"));
    assert_eq!(
        storage
            .conn()
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM usage_events WHERE model_raw='gpt-6-astra'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn parallel_entry_prioritizes_appends_and_model_summary_matches() {
    use llm_usage_core::adapters::codex::CodexAdapter;
    use llm_usage_core::adapters::framework::{
        run_adapter_scans_parallel, DiscoverContext, InstanceFilter, ParallelScanRequest, RunConfig,
    };
    use llm_usage_core::jobs::TriggerKind;
    use std::sync::{Arc, Mutex};
    let dir = TempDir::new("codex-parallel-append");
    write_rollout(dir.path(), "06", "active", 0);
    let (_db, storage) = temp_storage("codex-parallel-append");
    let now = ts("2026-10-07T04:00:00Z");
    run_codex(&storage, dir.path(), now);
    append_text(
        &dir.path().join("sessions/2026/10/06/rollout-active.jsonl"),
        &astra_record(),
    );
    write_rollout(dir.path(), "01", "history", 15);
    let storage = Mutex::new(storage);
    let request = ParallelScanRequest {
        adapter: &CodexAdapter::new(),
        context: DiscoverContext {
            manual_roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        },
        config: RunConfig {
            timezone: "UTC".into(),
            now_ms: now + 1,
            limits: limits(),
            trigger: TriggerKind::Manual,
            origin_host_id: None,
            run_id_prefix: "parallel-appended".into(),
        },
        filter: InstanceFilter::default(),
    };
    let reports = run_adapter_scans_parallel(&storage, &[request], None, Arc::new(|| true), None);
    assert!(reports[0].as_ref().unwrap()[0].files[0]
        .file_id
        .ends_with("rollout-active.jsonl"));
    let day = llm_usage_core::calendar::parse_date("2026-10-07").unwrap();
    let result = llm_usage_core::query::query_summary(
        &storage.lock().unwrap(),
        &llm_usage_core::query::SummaryRequest {
            timezone: "UTC".into(),
            week_start: llm_usage_core::calendar::WeekStart::Monday,
            first_day: day,
            last_day: day,
            today: day,
            retention_cutoff: None,
            granularity: llm_usage_core::query::Granularity::Day,
            filters: llm_usage_core::query::Filters {
                models: vec!["gpt-6-astra".into()],
                ..Default::default()
            },
        },
    )
    .unwrap();
    assert_eq!(
        (result.totals.call_count, result.totals.total_tokens_known),
        (1, Some(120))
    );
}
