//! Synthetic Claude gap tests; directories/headers are labeled synthetic. Originally
//! created without native samples. Covers subagents, cross-file deduplication and orphaned/superseded
//! variants, fallback identities, no-usage records, unmapped usage keys and V17 rejection
//! of usage on other records/undocumented types, plus direct detection tests.

mod common;

use common::*;
use llm_usage_core::adapters::claude::ClaudeAdapter;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::domain::VersionBasis;

const NOW: i64 = 1_800_000_000_000;

#[test]
fn subagent_category_and_cross_file_dedup() {
    let (_db, storage) = temp_storage("claude-sub");
    let root = claude_fixture("synthetic-subagent");
    let reports = run_claude(&storage, &root, NOW);
    let report = &reports[0];
    assert_eq!(report.files.len(), 2, "main transcript + subagent file");
    for f in &report.files {
        assert_eq!(f.status, "complete");
        assert_eq!(f.lines_read, 3);
        assert_eq!(f.events, 2);
    }
    let outcome = report.outcome.as_ref().unwrap();
    // Discovery sorts path components: sess-1 directory precedes sess-1.jsonl, so subagent first.
    // syn-req-shared first arrives as sub_agent; same-key main copy has another category and conflicts.
    // Retain stored value without double counting; see _expectations.md.
    assert_eq!(outcome.added, 3);
    assert_eq!(outcome.conflicts, 1);
    assert_eq!(outcome.unchanged, 0);

    let rows: Vec<(String, String, Option<String>, i64)> = storage
        .conn()
        .prepare(
            "SELECT source_record_key, call_category, parent_session_id, conflict
             FROM usage_events ORDER BY source_record_key",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "req:syn-req-shared".to_string(),
                "sub_agent".to_string(),
                Some("sess-1".to_string()),
                1
            ),
            (
                "req:syn-req-side".to_string(),
                "sub_agent".to_string(),
                None,
                0
            ),
            (
                "req:syn-req-subonly".to_string(),
                "sub_agent".to_string(),
                Some("sess-1".to_string()),
                0
            ),
        ],
        "sidechain entry => sub_agent; subagents/ path => sub_agent + parent"
    );

    let conflicts: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'update_conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conflicts, 1);

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(
        summary.totals.call_count, 3,
        "shared requestId counted once"
    );
    assert_eq!(summary.totals.input_total_known, Some(235));
    assert_eq!(summary.totals.cache_read_known, Some(30));
    assert_eq!(summary.totals.cache_write_known, Some(5));
    assert_eq!(summary.totals.output_total_known, Some(30));
    assert_eq!(summary.totals.total_tokens_known, Some(265));
    assert_eq!(summary.totals.conflict_count, 1);
}

#[test]
fn orphaned_superseded_variants_dedup_and_discovery() {
    let (_db, storage) = temp_storage("claude-orph");
    let root = claude_fixture("synthetic-orphaned-superseded");
    let reports = run_claude(&storage, &root, NOW);
    let report = &reports[0];
    // Three files: current, .jsonl.superseded- (not ending in .jsonl, explicitly accepted),
    // and .orphaned- variant.
    assert_eq!(
        report.files.len(),
        3,
        "superseded variant must be discovered by the accept rule"
    );
    for f in &report.files {
        assert_eq!(f.status, "complete");
    }
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 3);
    assert_eq!(
        outcome.unchanged, 1,
        "identical re-report in orphaned variant is idempotent"
    );
    assert_eq!(outcome.conflicts, 0);

    let count: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3, "req-a counted once across current + orphaned");

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.input_total_known, Some(75));
    assert_eq!(summary.totals.cache_read_known, Some(5));
    assert_eq!(summary.totals.cache_write_known, Some(10));
    assert_eq!(summary.totals.output_total_known, Some(12));
    assert_eq!(summary.totals.total_tokens_known, Some(87));
}

#[test]
fn fallback_identity_chain_records_diagnostics() {
    let (_db, storage) = temp_storage("claude-fb");
    let root = claude_fixture("synthetic-fallback-identity");
    let reports = run_claude(&storage, &root, NOW);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 3);
    assert_eq!(report.outcome.as_ref().unwrap().added, 3);

    let rows: Vec<(String, Option<String>)> = storage
        .conn()
        .prepare(
            "SELECT source_record_key, origin_call_id FROM usage_events
             ORDER BY source_record_key",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "msg:syn-msg-fb1".to_string(),
                Some("syn-msg-fb1".to_string())
            ),
            ("seq:syn-sess-1:4".to_string(), None),
            ("uuid:syn-uuid-fb2".to_string(), None),
        ],
        "fallback chain: requestId -> message.id -> uuid -> session+line"
    );
    let diags: Vec<(String,)> = storage
        .conn()
        .prepare(
            "SELECT position FROM diagnostics WHERE code = 'missing_request_id'
             ORDER BY position",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?,)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        diags,
        vec![
            ("line 2".to_string(),),
            ("line 3".to_string(),),
            ("line 4".to_string(),)
        ],
        "one diagnostic per fallback entry"
    );

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.input_total_known, Some(7));
    assert_eq!(summary.totals.output_total_known, Some(7));
    assert_eq!(summary.totals.total_tokens_known, Some(14));
}

#[test]
fn assistant_without_usage_produces_no_event_and_diag_once() {
    let (_db, storage) = temp_storage("claude-nu");
    let root = claude_fixture("synthetic-assistant-without-usage");
    let reports = run_claude(&storage, &root, NOW);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 4);
    assert_eq!(report.files[0].events, 1, "no usage evidence => no event");
    assert_eq!(report.outcome.as_ref().unwrap().added, 1);

    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 1);
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'assistant_without_usage'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        diags, 1,
        "two no-usage entries but one diagnostic per file (parse context)"
    );

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(10));
    assert_eq!(summary.totals.output_total_known, Some(5));
    assert_eq!(summary.totals.total_tokens_known, Some(15));
}

#[test]
fn unmapped_usage_keys_kept_and_diag_once() {
    let (_db, storage) = temp_storage("claude-umk");
    let root = claude_fixture("synthetic-unmapped-usage-keys");
    let reports = run_claude(&storage, &root, NOW);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 2);
    assert_eq!(report.outcome.as_ref().unwrap().added, 2);

    // Extra keys do not change recording of the four mapped fields.
    let row: (i64, i64, i64, i64, i64) = storage
        .conn()
        .query_row(
            "SELECT input_uncached, input_cache_read, input_cache_write, input_total, total_tokens
             FROM usage_events WHERE source_record_key = 'req:syn-req-k1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(row, (10, 2, 1, 13, 16));
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unmapped_usage_keys'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diags, 1, "extra keys on two entries, one diagnostic");

    let summary = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(33));
    assert_eq!(summary.totals.cache_read_known, Some(2));
    assert_eq!(summary.totals.cache_write_known, Some(1));
    assert_eq!(summary.totals.output_total_known, Some(7));
    assert_eq!(summary.totals.total_tokens_known, Some(40));
}

#[test]
fn usage_on_user_record_fails_closed_and_cursor_held() {
    let (_db, storage) = temp_storage("claude-uou");
    let root = claude_fixture("synthetic-usage-on-user");

    for round in 1..=2 {
        let reports = run_claude(&storage, &root, NOW + round * 1000);
        let report = &reports[0];
        assert_eq!(
            report.files[0].status, "pending",
            "fail closed => file held pending, round {round}"
        );
        assert_eq!(report.files[0].lines_read, 3);
        assert_eq!(report.files[0].records_seen, 3);
        assert_eq!(
            report.files[0].events, 0,
            "events cleared on fail closed (would-be event on line 2)"
        );
        let outcome = report.outcome.as_ref().unwrap();
        assert_eq!((outcome.added, outcome.errors), (0, 0));

        let events: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(events, 0);
        let checkpoints: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(checkpoints, 0, "cursor never advances, round {round}");
        let diags: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code = 'usage_on_unexpected_record_type'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            diags, round,
            "deterministic re-refusal records the diagnostic again"
        );
        let file_status: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(file_status, "degraded");
    }
}

#[test]
fn undocumented_record_type_fails_closed() {
    let adapter = ClaudeAdapter::new();
    let root = claude_fixture("synthetic-undocumented-type");
    let path = root.join("projects/proj/sess-1.jsonl");
    // Detection checks only the user first line: Supported; scanner rejects later invalid records.
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Supported {
            format: "claude-transcript-jsonl".to_string(),
            format_version: Some("transcript-doc-1".to_string()),
            basis: VersionBasis::KnownVersion,
        }
    );

    let (_db, storage) = temp_storage("claude-udt");
    let reports = run_claude(&storage, &root, NOW);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "pending");
    assert_eq!(report.files[0].lines_read, 3);
    assert_eq!(report.files[0].events, 0);

    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
    let checkpoints: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(checkpoints, 0, "cursor held for controlled retry");
    let diag: (String, String) = storage
        .conn()
        .query_row(
            "SELECT field, position FROM diagnostics WHERE code = 'undocumented_record_type'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(diag, ("type".to_string(), "line 3".to_string()));
}

#[test]
fn detect_empty_file_pending() {
    let dir = TempDir::new("claude-empty");
    let path = dir.path().join("empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = ClaudeAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn detect_undocumented_first_line_unknown_format() {
    let dir = TempDir::new("claude-detect-unk");
    let path = dir.path().join("bad.jsonl");
    std::fs::write(
        &path,
        b"{\"type\":\"file-history-snapshot\",\"timestamp\":\"2026-09-24T10:00:00.000Z\"}\n",
    )
    .unwrap();
    let adapter = ClaudeAdapter::new();
    let outcome = adapter.detect(&path).unwrap();
    assert!(
        matches!(outcome, DetectOutcome::UnknownFormat { .. }),
        "first record type outside documented set => UnknownFormat: {outcome:?}"
    );
}

#[test]
fn detect_non_json_first_line_unknown_format() {
    let dir = TempDir::new("claude-detect-nj");
    let path = dir.path().join("bad.jsonl");
    std::fs::write(&path, b"this is not json\n").unwrap();
    let adapter = ClaudeAdapter::new();
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        }
    );
}

#[test]
fn detect_documented_first_line_supported() {
    let adapter = ClaudeAdapter::new();
    let path = claude_fixture("synthetic-contract").join("projects/proj/sess-1.jsonl");
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Supported {
            format: "claude-transcript-jsonl".to_string(),
            format_version: Some("transcript-doc-1".to_string()),
            basis: VersionBasis::KnownVersion,
        }
    );
}
