//! Codex boundary tests: directories/headers marked synthetic, under V17 rules.
//! Cover gaps in M0 real samples: duplicate final records, subagents, positive cache writes,
//! tool/user messages without usage and absent response_id. Unknown versions try the latest parser
//! with V17/V30 compatibility metadata; unknown formats are rejected.

mod common;

use common::*;
use llm_usage_core::adapters::codex::{map_codex_record, CodexAdapter, CodexRecordUsage};
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::domain::VersionBasis;

fn synthetic_root(name: &str) -> std::path::PathBuf {
    codex_fixture(name)
}

#[test]
fn duplicate_final_counts_once() {
    let (_db, storage) = temp_storage("codex-dup");
    let root = synthetic_root("synthetic-duplicate-final");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    // Three scanned events include a duplicate final; ingestion deduplicates them into two calls.
    assert_eq!(report.files[0].events, 3);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 2);
    assert_eq!(
        outcome.unchanged, 1,
        "identical duplicate final is idempotent"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(3_000));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.output_total_known, Some(150));
    assert_eq!(summary.totals.total_tokens_known, Some(3_150));
    assert_eq!(report.reconciliations[0].verdict, "matched");
}

#[test]
fn subagent_session_category_and_host_mapping() {
    let (_db, storage) = temp_storage("codex-sub");
    let root = synthetic_root("synthetic-subagent");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let (category, host, parent, agent): (String, String, String, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, host_application, parent_session_id, agent FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(
        host, "vscode",
        "codex_vscode originator maps to vscode host"
    );
    assert_eq!(parent, "syn-parent-1");
    assert_eq!(agent, "codex");
}

#[test]
fn cache_write_positive_maps_reported_write_and_derived_uncached() {
    let (_db, storage) = temp_storage("codex-cw");
    let root = synthetic_root("synthetic-cache-write");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let (write, uncached, input, quality): (i64, i64, i64, String) = storage
        .conn()
        .query_row(
            "SELECT input_cache_write, input_uncached, input_total, quality_json FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(write, 100);
    assert_eq!(uncached, 500, "uncached = 1000 - 400 - 100");
    assert_eq!(input, 1000);
    let quality: serde_json::Value = serde_json::from_str(&quality).unwrap();
    assert_eq!(quality["input_cache_write"], "reported");
    assert_eq!(quality["input_uncached"], "derived");

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.cache_write_known, Some(100));
    assert_eq!(summary.totals.total_tokens_known, Some(1_050));
}

#[test]
fn tool_and_user_messages_without_usage_produce_no_calls() {
    let (_db, storage) = temp_storage("codex-nu");
    let root = synthetic_root("synthetic-no-usage-messages");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 9);
    assert_eq!(report.files[0].events, 0, "no usage evidence => no events");
    assert_eq!(report.reconciliations[0].verdict, "no_snapshot");
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "no phantom requests from tool/user messages");
}

#[test]
fn missing_response_id_falls_back_to_session_ordinal_identity() {
    let dir = TempDir::new("codex-norid");
    let file = concat!(
        "{\"timestamp\":\"2026-01-05T15:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-nr\",\"session_id\":\"syn-sess-nr\",\"timestamp\":\"2026-01-05T15:00:00.000Z\",\"cli_version\":\"0.155.0-alpha.16.3\",\"model_provider\":\"openai\"}}\n",
        "{\"timestamp\":\"2026-01-05T15:00:01.000Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"t\",\"model\":\"m\"}}\n",
        "{\"timestamp\":\"2026-01-05T15:00:02.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"thread_id\":\"syn-sess-nr\",\"turn_id\":\"t\",\"session_id\":\"syn-sess-nr\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":0,\"cache_write_input_tokens\":0,\"output_tokens\":5,\"reasoning_output_tokens\":0,\"total_tokens\":15}}}\n",
    );
    let root = codex_root_with_file(&dir, "rollout-norid.jsonl", file.as_bytes());
    let (_db, storage) = temp_storage("codex-norid");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let key: String = storage
        .conn()
        .query_row("SELECT source_record_key FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        key, "seq:syn-sess-nr:3",
        "fallback = session UUID + line number"
    );
    let diag: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'missing_response_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);
}

#[test]
fn v17_unknown_version_falls_back_with_compat_mark() {
    // Unregistered version with unchanged shape tries the latest built-in parser, persists usage
    // and compatibility metadata; V17/V30 does not reject an unlisted version alone.
    let adapter = CodexAdapter::new();
    let path = synthetic_root("synthetic-unknown-version")
        .join("sessions/2026/01/05/rollout-synthetic-uv.jsonl");
    let outcome = adapter.detect(&path).unwrap();
    assert_eq!(
        outcome,
        DetectOutcome::Supported {
            format: "codex-rollout-jsonl".to_string(),
            format_version: Some("0.999.0-synthetic".to_string()),
            basis: VersionBasis::LatestFallback,
        }
    );

    let (_db, storage) = temp_storage("codex-uv");
    let root = synthetic_root("synthetic-unknown-version");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].events, 1);
    assert_eq!(
        report.files[0].detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: 0.999.0-synthetic)")
    );
    // Valid compatibility data counts one call with input=10, output=5, total=15.
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(10));
    assert_eq!(summary.totals.output_total_known, Some(5));
    assert_eq!(summary.totals.total_tokens_known, Some(15));
    // Persist compatibility in event parse_basis, file active_compat and detection JSON.
    let (basis, schema_version): (String, String) = storage
        .conn()
        .query_row(
            "SELECT parse_basis, schema_version FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(basis, "latest_fallback");
    assert_eq!(schema_version, "0.999.0-synthetic");
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'latest_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diags, 1, "compat attempt visible in diagnostics");
    let (file_status, format_status): (String, String) = storage
        .conn()
        .query_row("SELECT status, format_status FROM source_files", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(file_status, "active_compat");
    let fs: serde_json::Value = serde_json::from_str(&format_status).unwrap();
    assert_eq!(fs["basis"], "latest_fallback");
    assert_eq!(fs["found_version"], "0.999.0-synthetic");
    assert_eq!(fs["compat"], "unverified");
    // Repeated scans add no usage; compatibility metadata preserves deduplication.
    let reports2 = run_codex(&storage, &root, 1_800_000_000_100);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0);
}

#[test]
fn v17_missing_version_but_agent_identified_falls_back() {
    // Version is absent but session_meta.id identifies the Agent's session.
    // V30 selects the latest built-in parser with absent format_version.
    let dir = TempDir::new("codex-mv");
    let root = dir.path().join("root");
    let file = concat!(
        "{\"timestamp\":\"2026-01-06T10:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-mv\",\"session_id\":\"syn-sess-mv\",\"originator\":\"codex_cli\",\"model_provider\":\"openai\"}}\n",
        "{\"timestamp\":\"2026-01-06T10:00:01.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"thread_id\":\"syn-sess-mv\",\"response_id\":\"syn-resp-mv-1\",\"usage\":{\"input_tokens\":7,\"cached_input_tokens\":0,\"cache_write_input_tokens\":0,\"output_tokens\":3,\"reasoning_output_tokens\":0,\"total_tokens\":10}}}\n"
    );
    let sessions = root.join("sessions/2026/01/06");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("rollout-synthetic-mv.jsonl"), file).unwrap();
    let adapter = CodexAdapter::new();
    let outcome = adapter
        .detect(&sessions.join("rollout-synthetic-mv.jsonl"))
        .unwrap();
    assert_eq!(
        outcome,
        DetectOutcome::Supported {
            format: "codex-rollout-jsonl".to_string(),
            format_version: None,
            basis: VersionBasis::LatestFallback,
        }
    );
    let (_db, storage) = temp_storage("codex-mv");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 1);
    let summary = summary(&storage, "2026-01-06", "2026-01-06");
    assert_eq!(summary.totals.total_tokens_known, Some(10));
}

#[test]
fn v17_extra_optional_fields_tolerated_under_fallback() {
    // Additional optional envelope/payload/usage keys do not break required structure;
    // valid records are persisted under the V30 optional-field scenario.
    let dir = TempDir::new("codex-ef");
    let root = dir.path().join("root");
    let file = concat!(
        "{\"timestamp\":\"2026-01-07T10:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-ef\",\"session_id\":\"syn-sess-ef\",\"cli_version\":\"0.998.0-synthetic\",\"originator\":\"codex_cli\",\"model_provider\":\"openai\",\"new_optional_field\":{\"nested\":true}}}\n",
        "{\"timestamp\":\"2026-01-07T10:00:01.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"thread_id\":\"syn-sess-ef\",\"response_id\":\"syn-resp-ef-1\",\"new_payload_field\":\"x\",\"usage\":{\"input_tokens\":20,\"cached_input_tokens\":0,\"cache_write_input_tokens\":0,\"output_tokens\":10,\"reasoning_output_tokens\":0,\"total_tokens\":30,\"new_usage_field\":123}}}\n"
    );
    let sessions = root.join("sessions/2026/01/07");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("rollout-synthetic-ef.jsonl"), file).unwrap();
    let (_db, storage) = temp_storage("codex-ef");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].status, "complete");
    let summary = summary(&storage, "2026-01-07", "2026-01-07");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(30));
}

#[test]
fn v17_fallback_structural_break_marks_incompatible_and_keeps_old() {
    // An unknown version yielding no events plus structure diagnostics is incompatible;
    // retain old results without advancing the cursor or committing events/aggregates.
    // Retry next round, including after parser updates (V30).
    let dir = TempDir::new("codex-ib");
    let root = dir.path().join("root");
    let file = concat!(
        "{\"timestamp\":\"2026-01-08T10:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-ib\",\"session_id\":\"syn-sess-ib\",\"cli_version\":\"0.997.0-synthetic\",\"originator\":\"codex_cli\",\"model_provider\":\"openai\"}}\n",
        "{\"timestamp\":\"2026-01-08T10:00:01.000Z\",\"type\":\"totally_unknown_record\",\"payload\":{\"whatever\":\"structure\"}}\n",
        "{\"timestamp\":\"2026-01-08T10:00:02.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"response_id\":\"syn-resp-ib-1\",\"usage\":{\"input_tokens\":\"not-a-number\"}}}\n"
    );
    let sessions = root.join("sessions/2026/01/08");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("rollout-synthetic-ib.jsonl"), file).unwrap();
    let (_db, storage) = temp_storage("codex-ib");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "incompatible");
    assert_eq!(report.files[0].events, 0);
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "no untrusted events committed");
    // Empty ingestion_checkpoints permits a fresh attempt next round.
    let checkpoints: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(checkpoints, 0);
    let file_status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_status, "incompatible");
    // Show both latest_fallback and structure diagnostics, distinguishing rejection from an empty success.
    for code in [
        "latest_fallback",
        "unknown_record_type",
        "usage_shape_deviation",
    ] {
        let n: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code = ?1",
                [code],
                |r| r.get(0),
            )
            .unwrap();
        assert!(n > 0, "diagnostic {code} expected, found 0");
    }
}

#[test]
fn v17_fallback_partial_usability_keeps_validated_part() {
    // Partly usable data: one valid usage record and one with an invalid required field type.
    // Persist independently validated records with compatibility metadata; report the gap (V30).
    let dir = TempDir::new("codex-pu");
    let root = dir.path().join("root");
    let file = concat!(
        "{\"timestamp\":\"2026-01-09T10:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-pu\",\"session_id\":\"syn-sess-pu\",\"cli_version\":\"0.996.0-synthetic\",\"originator\":\"codex_cli\",\"model_provider\":\"openai\"}}\n",
        "{\"timestamp\":\"2026-01-09T10:00:01.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"response_id\":\"syn-resp-pu-1\",\"usage\":{\"input_tokens\":100,\"cached_input_tokens\":0,\"cache_write_input_tokens\":0,\"output_tokens\":50,\"reasoning_output_tokens\":0,\"total_tokens\":150}}}\n",
        "{\"timestamp\":\"2026-01-09T10:00:02.000Z\",\"type\":\"token_usage_record\",\"payload\":{\"response_id\":\"syn-resp-pu-2\",\"usage\":{\"input_tokens\":null}}}\n"
    );
    let sessions = root.join("sessions/2026/01/09");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("rollout-synthetic-pu.jsonl"), file).unwrap();
    let (_db, storage) = temp_storage("codex-pu");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].events, 1);
    let summary = summary(&storage, "2026-01-09", "2026-01-09");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.total_tokens_known, Some(150));
    let basis: String = storage
        .conn()
        .query_row("SELECT parse_basis FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(basis, "latest_fallback");
    let n: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'usage_shape_deviation'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "broken record visible as coverage gap");
}

#[test]
fn v17_unknown_format_fails_closed_not_success_zero() {
    let adapter = CodexAdapter::new();
    let path = synthetic_root("synthetic-not-codex")
        .join("sessions/2026/01/05/rollout-synthetic-nc.jsonl");
    let outcome = adapter.detect(&path).unwrap();
    assert!(matches!(outcome, DetectOutcome::UnknownFormat { .. }));

    let (_db, storage) = temp_storage("codex-nc");
    let root = synthetic_root("synthetic-not-codex");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "unknown_format");
    let diags: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'unknown_format'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diags, 1);
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("codex-empty");
    let path = dir.path().join("rollout-empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = CodexAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
}

#[test]
fn map_codex_record_full_field_contract() {
    // Synthetic positive cache-write data checks all six mapped fields, as in synthetic-cache-write.
    let mapped = map_codex_record(&CodexRecordUsage {
        input_tokens: 1000,
        cached_input_tokens: 400,
        cache_write_input_tokens: 100,
        output_tokens: 50,
        reasoning_output_tokens: 10,
        total_tokens: 1050,
    });
    assert_eq!(mapped.usage.input_uncached, Some(500));
    assert_eq!(mapped.usage.input_cache_write, Some(100));
    assert_eq!(mapped.usage.total_tokens, Some(1050));
    assert_eq!(mapped.usage.source_total, Some(1050));
    assert!(mapped.diagnostics.is_empty());

    // cached+write exceeding input leaves uncached unknown with a diagnostic; do not truncate values.
    let contradiction = map_codex_record(&CodexRecordUsage {
        input_tokens: 100,
        cached_input_tokens: 90,
        cache_write_input_tokens: 50,
        output_tokens: 10,
        reasoning_output_tokens: 0,
        total_tokens: 110,
    });
    assert_eq!(contradiction.usage.input_uncached, None);
    assert!(contradiction
        .diagnostics
        .iter()
        .any(|d| d.code == "negative_derived_field"));
}
