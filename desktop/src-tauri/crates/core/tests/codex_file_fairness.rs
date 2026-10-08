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
