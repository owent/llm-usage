mod common;
use common::*;
use llm_usage_core::adapters::jsonl::DEFAULT_MAX_LINE_BYTES;
use serde_json::json;

fn records(output_size: usize) -> String {
    let header = json!({"type":"session_meta","payload":{"id":"synthetic-session","cli_version":"0.163.0-alpha.11"}});
    let model = json!({"type":"turn_context","payload":{"model":"gpt-6-sol"}});
    let call = json!({"type":"token_usage_record","timestamp":"2026-10-20T00:01:00Z","payload":{"response_id":"synthetic-call","usage":{
        "input_tokens":100,"cached_input_tokens":50,"cache_write_input_tokens":0,
        "output_tokens":20,"reasoning_output_tokens":5,"total_tokens":120}}});
    // Put type after the large payload to require complete structural validation.
    let large = format!("{{\"payload\":{{\"type\":\"custom_tool_call_output\",\"output\":\"{}\"}},\"type\":\"response_item\"}}", "x".repeat(output_size));
    format!("{header}\n{model}\n{large}\n{call}\n")
}

#[test]
fn oversized_tool_output_does_not_block_later_usage_or_repeat_imports() {
    let dir = TempDir::new("codex-large-output");
    let bytes = records(DEFAULT_MAX_LINE_BYTES);
    let root = codex_root_with_file(&dir, "rollout-large.jsonl", bytes.as_bytes());
    let (_db, storage) = temp_storage("codex-large-output");
    for n in 0..3 {
        let report = run_codex(&storage, &root, 1_800_000_000_000 + n);
        assert!(
            report[0]
                .error
                .as_deref()
                .map_or(true, |e| e.starts_with("source_window_budget_exhausted")),
            "{:?}",
            report[0].error
        );
        assert_ne!(report[0].files[0].status, "line_too_long");
    }
    let count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "the call after the oversized output must be imported"
    );
    let totals = summary_of_event_days(&storage).totals;
    assert_eq!(
        (totals.call_count, totals.total_tokens_known),
        (1, Some(120))
    );
    let status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "active_compat");
    let repeated = run_codex(&storage, &root, 1_800_000_000_004);
    assert_eq!(repeated[0].files[0].status, "unchanged");

    storage.conn().execute_batch("UPDATE source_instances SET parser_version='codex-rollout-6'; UPDATE usage_events SET parser_version='codex-rollout-6'; UPDATE source_files SET status='degraded'; UPDATE ingestion_checkpoints SET parse_context=json_set(parse_context,'$.has_record_errors',json('true')); INSERT INTO diagnostics(batch_id,instance_id,code,message,created_ms) SELECT 'old-reader',instance_id,'line_too_long','old reader stopped at a tool output',1 FROM source_instances;").unwrap();
    for n in 5..8 {
        run_codex(&storage, &root, 1_800_000_000_000 + n);
    }
    assert_eq!(summary_of_event_days(&storage).totals, totals);
    let (status, diagnostics, conflicts): (String, i64, i64) = storage.conn().query_row("SELECT (SELECT status FROM source_files),(SELECT COUNT(*) FROM diagnostics WHERE code='line_too_long'),(SELECT COUNT(*) FROM usage_events WHERE conflict=1)", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        (status.as_str(), diagnostics, conflicts),
        ("active_compat", 1, 0)
    );
    assert_eq!(
        std::fs::read(root.join("sessions/2026/09/24/rollout-large.jsonl")).unwrap(),
        bytes.as_bytes()
    );
}

#[test]
fn legacy_reader_also_resumes_after_oversized_non_usage() {
    let dir = TempDir::new("codex-legacy-large-output");
    let mut rows: Vec<serde_json::Value> = records(2048)
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    rows[0]["payload"]["cli_version"] = json!("0.146.0-alpha.3");
    let usage = rows[3]["payload"]["usage"].clone();
    rows[3] = json!({"type":"event_msg","timestamp":"2026-10-20T00:01:00Z","payload":{"type":"token_count","info":{"last_token_usage":usage,"total_token_usage":usage}}});
    let text = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    let root = codex_root_with_file(&dir, "rollout-legacy-large.jsonl", text.as_bytes());
    let (_db, storage) = temp_storage("codex-legacy-large-output");
    let mut limits = llm_usage_core::adapters::framework::ScanLimits::default();
    limits.jsonl.max_line_bytes = 512;
    for n in 0..3 {
        run_codex_with_limits(&storage, &root, 1_800_000_000_000 + n, limits.clone());
    }
    let totals = summary_of_event_days(&storage).totals;
    assert_eq!(
        (totals.call_count, totals.total_tokens_known),
        (1, Some(120))
    );
    let status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "active");
}

#[test]
fn oversized_usage_holds_the_cursor_and_keeps_the_error_visible() {
    let dir = TempDir::new("codex-large-usage");
    let rows = records(1).lines().map(str::to_string).collect::<Vec<_>>();
    let header = rows[0].replace("0.163.0-alpha.11", "0.154.0-alpha.6.2");
    let prefix = format!("{header}\n{}\n", rows[1]);
    let mut call: serde_json::Value = serde_json::from_str(&rows[3]).unwrap();
    call["payload"]["padding"] = json!("x".repeat(2048));
    let text = format!("{prefix}{call}\n");
    let root = codex_root_with_file(&dir, "rollout-large-usage.jsonl", text.as_bytes());
    let (_db, storage) = temp_storage("codex-large-usage");
    let mut limits = llm_usage_core::adapters::framework::ScanLimits::default();
    limits.jsonl.max_line_bytes = 512;
    for n in 0..2 {
        let report = run_codex_with_limits(&storage, &root, 1_800_000_000_000 + n, limits.clone());
        assert_eq!(report[0].files[0].status, "line_too_long");
        let (cursor,status):(String,String)=storage.conn().query_row("SELECT c.cursor_value,f.status FROM ingestion_checkpoints c JOIN source_files f ON c.instance_id=f.instance_id AND c.scope_key=f.file_identity",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let cursor: serde_json::Value = serde_json::from_str(&cursor).unwrap();
        assert_eq!(cursor["offset"], prefix.len());
        assert_eq!(status, "degraded");
    }
    let count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn completing_a_large_line_recovers_health_but_cannot_erase_real_record_errors() {
    use std::io::Write;
    for bad_json in [false, true] {
        let dir = TempDir::new("codex-large-partial");
        let rows = records(2048)
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let header = rows[0].replace("0.163.0-alpha.11", "0.154.0-alpha.6.2");
        let prefix = format!(
            "{header}\n{}\n{}",
            rows[1],
            if bad_json { "invalid-json\n" } else { "" }
        );
        let root = codex_root_with_file(
            &dir,
            "rollout-partial.jsonl",
            format!("{prefix}{}", rows[2]).as_bytes(),
        );
        let (_db, storage) = temp_storage("codex-large-partial");
        let mut limits = llm_usage_core::adapters::framework::ScanLimits::default();
        limits.jsonl.max_line_bytes = 512;
        let first = run_codex_with_limits(&storage, &root, 1_800_000_000_000, limits.clone());
        assert_eq!(first[0].files[0].status, "line_too_long");
        let path = root.join("sessions/2026/09/24/rollout-partial.jsonl");
        let mut stream = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(stream, "\n{}", rows[3]).unwrap();
        drop(stream);
        for n in 1..4 {
            run_codex_with_limits(&storage, &root, 1_800_000_000_000 + n, limits.clone());
        }
        assert_eq!(summary_of_event_days(&storage).totals.call_count, 1);
        let status: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, if bad_json { "degraded" } else { "active" });
        let history: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code='line_too_long'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(history > 0);
    }
}

#[test]
fn oversized_positional_array_cannot_impersonate_a_native_record() {
    let dir = TempDir::new("codex-array-envelope");
    let rows = records(1).lines().map(str::to_string).collect::<Vec<_>>();
    let header = rows[0].replace("0.163.0-alpha.11", "0.154.0-alpha.6.2");
    let prefix = format!("{header}\n{}\n", rows[1]);
    let text = format!(
        "{prefix}[\"response_item\"{}]\n{}\n",
        " ".repeat(2048),
        rows[3]
    );
    let root = codex_root_with_file(&dir, "rollout-array.jsonl", text.as_bytes());
    let (_db, storage) = temp_storage("codex-array-envelope");
    let mut limits = llm_usage_core::adapters::framework::ScanLimits::default();
    limits.jsonl.max_line_bytes = 512;
    for n in 0..2 {
        let report = run_codex_with_limits(&storage, &root, 1_800_000_000_000 + n, limits.clone());
        assert_eq!(report[0].files[0].status, "line_too_long");
        let cursor: String = storage
            .conn()
            .query_row("SELECT cursor_value FROM ingestion_checkpoints", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&cursor).unwrap()["offset"],
            prefix.len()
        );
    }
    let count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn oversized_skip_and_prior_usage_roll_back_together_before_retry() {
    let dir = TempDir::new("codex-large-rollback");
    let rows = records(2048)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let text = format!("{}\n{}\n{}\n{}\n", rows[0], rows[1], rows[3], rows[2]);
    let root = codex_root_with_file(&dir, "rollout-rollback.jsonl", text.as_bytes());
    let (db, storage) = temp_storage("codex-large-rollback");
    storage.conn().execute_batch("CREATE TRIGGER fail_large_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
    let mut limits = llm_usage_core::adapters::framework::ScanLimits::default();
    limits.jsonl.max_line_bytes = 512;
    let report = run_codex_with_limits(&storage, &root, 1_800_000_000_000, limits.clone());
    assert_eq!(report[0].finish, llm_usage_core::jobs::RunStatus::Failed);
    for table in ["usage_events", "ingestion_checkpoints"] {
        let count: i64 = storage
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    storage
        .conn()
        .execute_batch("DROP TRIGGER fail_large_checkpoint")
        .unwrap();
    drop(storage);
    let storage = llm_usage_core::storage::Storage::open(&db.db_path()).unwrap();
    for n in 1..4 {
        run_codex_with_limits(&storage, &root, 1_800_000_000_000 + n, limits.clone());
    }
    assert_eq!(summary_of_event_days(&storage).totals.call_count, 1);
    let cursor: String = storage
        .conn()
        .query_row("SELECT cursor_value FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cursor).unwrap()["offset"],
        text.len()
    );
    assert_eq!(
        std::fs::read(root.join("sessions/2026/09/24/rollout-rollback.jsonl")).unwrap(),
        text.as_bytes()
    );
}
