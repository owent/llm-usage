//! Recheck consumed cursors automatically; distinguish cumulative comparisons from per-call errors.
mod common;
use common::*;
use llm_usage_core::adapters::framework::ScanLimits;
use serde_json::json;

fn records(version: &str, legacy: bool) -> String {
    let usage = json!({"input_tokens":100,"cached_input_tokens":50,"cache_write_input_tokens":0,
        "output_tokens":20,"reasoning_output_tokens":5,"total_tokens":120});
    let mut snapshot = usage.clone();
    snapshot["total_tokens"] = json!(2000);
    snapshot["input_tokens"] = json!(1980);
    let header = json!({"type":"session_meta","payload":{"id":"synthetic-session","cli_version":version,"model_provider":"openai","timestamp":"2026-10-02T00:00:00Z"}});
    let model = json!({"type":"turn_context","payload":{"model":"gpt-6-sol"}});
    let call = if legacy {
        json!({"type":"event_msg","timestamp":"2026-10-02T00:01:00Z","payload":{"type":"token_count","info":{"total_token_usage":snapshot,"last_token_usage":usage}}})
    } else {
        json!({"type":"token_usage_record","timestamp":"2026-10-02T00:01:00Z","payload":{"response_id":"synthetic-call","usage":usage}})
    };
    let last = json!({"type":"event_msg","timestamp":"2026-10-02T00:02:00Z","payload":{"type":"token_count","info":{"total_token_usage":snapshot,"last_token_usage":usage}}});
    format!("{header}\n{model}\n{call}\n{last}\n")
}

fn status(storage: &llm_usage_core::storage::Storage) -> String {
    storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn consumed_old_policy_replays_both_formats_without_duplicate_usage_or_history_loss() {
    for (version, legacy) in [("0.146.0-alpha.3", true), ("0.154.0-alpha.6.2", false)] {
        let dir = TempDir::new("codex-health-replay");
        let contents = records(version, legacy);
        let root = codex_root_with_file(&dir, "rollout-replay.jsonl", contents.as_bytes());
        let (_db, storage) = temp_storage("codex-health-replay");
        let now = ts("2026-10-03T00:00:00Z");
        let first = run_codex(&storage, &root, now);
        assert_eq!(first[0].reconciliations[0].verdict, "mismatch");
        let before = summary(&storage, "2026-10-02", "2026-10-02").totals;
        assert_eq!(
            (before.call_count, before.total_tokens_known),
            (1, Some(120))
        );
        let revision = storage.data_revision().unwrap();
        let history: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingest_runs", [], |r| r.get(0))
            .unwrap();
        storage
            .conn()
            .execute_batch(
                "UPDATE source_instances SET parser_version='codex-rollout-1';
            UPDATE usage_events SET parser_version='codex-rollout-1';
            UPDATE source_files SET status='degraded';",
            )
            .unwrap();
        let replay = run_codex(&storage, &root, now + 1);
        assert!(replay[0].error.is_none());
        assert!(
            replay[0].files[0].lines_read > 0,
            "unchanged bytes must be reevaluated"
        );
        assert_eq!(status(&storage), "active");
        assert_eq!(summary(&storage, "2026-10-02", "2026-10-02").totals, before);
        assert!(storage.data_revision().unwrap() >= revision);
        let after_history: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingest_runs", [], |r| r.get(0))
            .unwrap();
        assert!(after_history >= history);
        let diagnostic: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code='reconcile_mismatch'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            diagnostic > 0,
            "automatic health review retains audit discrepancies"
        );
        let repeated = run_codex(&storage, &root, now + 2);
        assert_eq!(repeated[0].files[0].status, "unchanged");
        assert_eq!(status(&storage), "active");
    }
}

#[test]
fn bad_records_remain_degraded_after_budgeted_reads_and_good_appends() {
    for (version, legacy) in [("0.146.0-alpha.3", true), ("0.154.0-alpha.6.2", false)] {
        let dir = TempDir::new("codex-health-batches");
        let text = records(version, legacy).replacen('\n', "\ninvalid-json\n", 1);
        let root = codex_root_with_file(&dir, "rollout-errors.jsonl", text.as_bytes());
        let (_db, storage) = temp_storage("codex-health-batches");
        let now = ts("2026-10-03T00:00:00Z");
        let mut limits = ScanLimits::default();
        limits.jsonl.max_lines = Some(2);
        run_codex_with_limits(&storage, &root, now, limits);
        assert_eq!(status(&storage), "degraded");
        run_codex(&storage, &root, now + 1);
        assert_eq!(
            status(&storage),
            "degraded",
            "later valid records do not erase earlier read errors"
        );
        assert_eq!(
            summary(&storage, "2026-10-02", "2026-10-02")
                .totals
                .call_count,
            1
        );
    }
}

#[test]
fn invalid_snapshot_does_not_degrade_independent_per_call_records() {
    let dir = TempDir::new("codex-snapshot-shape");
    let mut lines = records("0.154.0-alpha.6.2", false)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut last: serde_json::Value = serde_json::from_str(&lines[3]).unwrap();
    last["payload"]["info"]["total_token_usage"] = json!({"total_tokens":2000});
    lines[3] = last.to_string();
    let root = codex_root_with_file(
        &dir,
        "rollout-snapshot.jsonl",
        (lines.join("\n") + "\n").as_bytes(),
    );
    let (_db, storage) = temp_storage("codex-snapshot-shape");
    run_codex(&storage, &root, ts("2026-10-03T00:00:00Z"));
    assert_eq!(status(&storage), "active");
    assert_eq!(
        summary(&storage, "2026-10-02", "2026-10-02")
            .totals
            .total_tokens_known,
        Some(120)
    );
}

#[test]
fn optional_token_info_is_not_a_broken_usage_record_or_a_call() {
    for (version, legacy) in [("0.144.0-alpha.4", true), ("0.154.0-alpha.6.2", false)] {
        let dir = TempDir::new("codex-rate-only");
        let rate_only = json!({"type":"event_msg","timestamp":"2026-10-02T00:00:00Z",
            "payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":10}}}});
        let absent_info =
            json!({"type":"event_msg","payload":{"type":"token_count","rate_limits":null}});
        let text = records(version, legacy).replacen('\n', &format!("\n{rate_only}\n"), 1)
            + &format!("{absent_info}\n");
        let root = codex_root_with_file(&dir, "rollout-rate-only.jsonl", text.as_bytes());
        let (_db, storage) = temp_storage("codex-rate-only");
        run_codex(&storage, &root, ts("2026-10-03T00:00:00Z"));
        let sums = summary(&storage, "2026-10-02", "2026-10-02").totals;
        assert_eq!(status(&storage), "active");
        assert_eq!(sums.call_count, 1);
        assert_eq!(sums.total_tokens_known, Some(120));
    }
}

#[test]
fn malformed_legacy_usage_remains_degraded_when_a_call_cannot_be_identified() {
    let dir = TempDir::new("codex-legacy-malformed");
    let mut lines = records("0.144.0-alpha.4", true)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut call: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();
    call["payload"]["info"]["total_token_usage"] = json!({"total_tokens":2000});
    lines[2] = call.to_string();
    lines.truncate(3);
    let root = codex_root_with_file(
        &dir,
        "rollout-malformed.jsonl",
        (lines.join("\n") + "\n").as_bytes(),
    );
    let (_db, storage) = temp_storage("codex-legacy-malformed");
    run_codex(&storage, &root, ts("2026-10-03T00:00:00Z"));
    assert_eq!(status(&storage), "degraded");
    assert_eq!(
        summary(&storage, "2026-10-02", "2026-10-02")
            .totals
            .call_count,
        0
    );
}
