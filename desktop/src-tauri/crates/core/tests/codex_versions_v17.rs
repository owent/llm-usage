//! Codex per-version regression tests (M2-D, V17/V30).
//! Redacted local samples cover 0.153.0, 0.154.0-alpha.6.1, and 0.154.0-alpha.6.2.
//! Check registry KnownVersion selection and individual usage against independent jq sums,
//! known_version parse basis, matched snapshot comparisons, and repeat-scan deduplication.
//! Expected values: each sample's _expectations.md, checked manually against these tests.
//! Older 0.139-0.151 token_count/last_token_usage formats dispatch to rollout_legacy.
//! Samples 0.139.0/0.142.5/0.146.0-alpha.3 check repeated reports, carried compaction
//! responses, and source counter regressions with visible comparison mismatches.

mod common;

use common::*;
use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::domain::VersionBasis;

struct VersionExpectations {
    fixture: &'static str,
    version: &'static str,
    calls: i64,
    input_total: i64,
    cache_read: i64,
    output_total: i64,
    reasoning: i64,
    total_tokens: i64,
    /// Final token_count snapshot total; matched comparisons equal the individual usage sum.
    snapshot_total: i64,
}

const VERSIONS: &[VersionExpectations] = &[
    VersionExpectations {
        fixture: "rollout-v0.153.0.sanitized.json",
        version: "0.153.0",
        calls: 25,
        input_total: 932_041,
        cache_read: 844_544,
        output_total: 2_326,
        reasoning: 858,
        total_tokens: 934_367,
        snapshot_total: 934_367,
    },
    VersionExpectations {
        fixture: "rollout-v0.154.0-alpha.6.1.sanitized.json",
        version: "0.154.0-alpha.6.1",
        calls: 3,
        input_total: 69_373,
        cache_read: 48_384,
        output_total: 367,
        reasoning: 151,
        total_tokens: 69_740,
        snapshot_total: 69_740,
    },
    VersionExpectations {
        fixture: "rollout-v0.154.0-alpha.6.2.sanitized.json",
        version: "0.154.0-alpha.6.2",
        calls: 5,
        input_total: 115_209,
        cache_read: 97_152,
        output_total: 548,
        reasoning: 10,
        total_tokens: 115_757,
        snapshot_total: 115_757,
    },
];

#[test]
fn per_version_fixtures_dispatch_known_and_match_expectations() {
    for exp in VERSIONS {
        let sanitized = codex_fixture("").join(exp.fixture);
        let contents = reconstruct_codex_jsonl(&sanitized);
        let dir = TempDir::new("codex-ver");
        let root = codex_root_with_file(&dir, "rollout-versioned.jsonl", &contents);

        // Detection and scanning use the same registry to select checked versions.
        let adapter = CodexAdapter::new();
        let file_path = root.join("sessions/2026/09/24/rollout-versioned.jsonl");
        assert_eq!(
            adapter.detect(&file_path).unwrap(),
            DetectOutcome::Supported {
                format: "codex-rollout-jsonl".to_string(),
                format_version: Some(exp.version.to_string()),
                basis: VersionBasis::KnownVersion,
            },
            "fixture {}",
            exp.fixture
        );

        let (_db, storage) = temp_storage("codex-ver");
        let reports = run_codex(&storage, &root, 1_800_000_000_000);
        let report = &reports[0];
        assert_eq!(report.files[0].status, "complete", "{}", exp.fixture);
        assert_eq!(report.files[0].events as i64, exp.calls, "{}", exp.fixture);

        // Compare values with independent jq calculations in _expectations.md.
        let summary = summary(&storage, "2020-01-01", "2100-01-01");
        assert_eq!(summary.totals.call_count, exp.calls, "{}", exp.fixture);
        assert_eq!(
            summary.totals.input_total_known,
            Some(exp.input_total),
            "{}",
            exp.fixture
        );
        assert_eq!(
            summary.totals.cache_read_known,
            Some(exp.cache_read),
            "{}",
            exp.fixture
        );
        assert_eq!(
            summary.totals.output_total_known,
            Some(exp.output_total),
            "{}",
            exp.fixture
        );
        assert_eq!(
            summary.totals.total_tokens_known,
            Some(exp.total_tokens),
            "{}",
            exp.fixture
        );

        // Checked versions carry no fallback marker; preserve the original version string.
        let (basis, schema_version): (Option<String>, String) = storage
            .conn()
            .query_row(
                "SELECT parse_basis, schema_version FROM usage_events LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(basis.as_deref(), Some("known_version"), "{}", exp.fixture);
        assert_eq!(schema_version, exp.version, "{}", exp.fixture);
        // Query reasoning directly from storage for the expected subset sum.
        let reasoning: i64 = storage
            .conn()
            .query_row(
                "SELECT COALESCE(SUM(output_reasoning), 0) FROM usage_events",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reasoning, exp.reasoning, "{}", exp.fixture);
        let file_status: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(file_status, "active", "{}", exp.fixture);

        // Matched reconciliation: individual usage sum equals the final snapshot.
        assert_eq!(
            report.reconciliations[0].verdict, "matched",
            "{}",
            exp.fixture
        );
        assert_eq!(
            report.reconciliations[0].snapshot_final,
            Some(exp.snapshot_total),
            "{}",
            exp.fixture
        );

        // Repeat scanning adds no events.
        let reports2 = run_codex(&storage, &root, 1_800_000_000_100);
        let added2: i64 = reports2
            .iter()
            .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
            .sum();
        assert_eq!(added2, 0, "{}", exp.fixture);
    }
}

#[test]
fn legacy_carrier_fixtures_dispatch_rollout_legacy_and_match_expectations() {
    // Older 0.139-0.151 files lack token_usage_record; individual usage comes from
    // token_count.last_token_usage under the cumulative-total change rules.
    // Expectations use independent JavaScript calculations and manual review.
    struct LegacyExpectations {
        fixture: &'static str,
        version: &'static str,
        calls: i64,
        input_total: i64,
        cache_read: i64,
        output_total: i64,
        reasoning: i64,
        total_tokens: i64,
        snapshot_total: i64,
        carried_sum: i64,
        difference: i64,
        verdict: &'static str,
        /// Snapshot comparison differences produce diagnostics without degrading source_files.status.
        /// Mismatches remain active; actual individual-usage parse errors degrade the file.
        file_status: &'static str,
        call_category: &'static str,
        model: &'static str,
    }
    let cases = [
        LegacyExpectations {
            fixture: "rollout-legacy-v0.139.0.sanitized.json",
            version: "0.139.0",
            calls: 9,
            input_total: 343_705,
            cache_read: 253_824,
            output_total: 1_002,
            reasoning: 347,
            total_tokens: 344_707,
            snapshot_total: 344_707,
            carried_sum: 0,
            difference: 0,
            verdict: "matched",
            file_status: "active",
            call_category: "sub_agent",
            model: "codex-auto-review",
        },
        LegacyExpectations {
            fixture: "rollout-legacy-v0.142.5.sanitized.json",
            version: "0.142.5",
            calls: 1,
            input_total: 27_648,
            cache_read: 7_040,
            output_total: 109,
            reasoning: 91,
            total_tokens: 27_757,
            snapshot_total: 27_757,
            carried_sum: 0,
            difference: 0,
            verdict: "matched",
            file_status: "active",
            call_category: "sub_agent",
            model: "codex-auto-review",
        },
        LegacyExpectations {
            fixture: "rollout-legacy-v0.146.0-alpha.3.sanitized.json",
            version: "0.146.0-alpha.3",
            calls: 86,
            input_total: 11_169_365,
            cache_read: 10_530_048,
            output_total: 42_138,
            reasoning: 20_692,
            total_tokens: 11_211_503,
            snapshot_total: 10_911_604,
            carried_sum: 16_894,
            difference: 299_899,
            verdict: "mismatch",
            // Snapshot mismatch is diagnostic reconciliation only; source health stays active.
            file_status: "active",
            call_category: "primary",
            model: "gpt-5.6-sol",
        },
    ];
    for exp in cases {
        let sanitized = codex_fixture("").join(exp.fixture);
        let contents = reconstruct_codex_jsonl(&sanitized);
        let dir = TempDir::new("codex-legacy-ver");
        let root = codex_root_with_file(&dir, "rollout-legacy-versioned.jsonl", &contents);

        // Registered older versions use KnownVersion in detection and scanning.
        let adapter = CodexAdapter::new();
        let file_path = root.join("sessions/2026/09/24/rollout-legacy-versioned.jsonl");
        assert_eq!(
            adapter.detect(&file_path).unwrap(),
            DetectOutcome::Supported {
                format: "codex-rollout-jsonl".to_string(),
                format_version: Some(exp.version.to_string()),
                basis: VersionBasis::KnownVersion,
            },
            "fixture {}",
            exp.fixture
        );

        let (_db, storage) = temp_storage("codex-legacy-ver");
        let reports = run_codex(&storage, &root, 1_800_000_000_000);
        let report = &reports[0];
        assert_eq!(report.files[0].status, "complete", "{}", exp.fixture);
        // Calls = regular + carried; 0.146.0-alpha.3 includes one carried compaction response.
        assert_eq!(report.files[0].events as i64, exp.calls, "{}", exp.fixture);

        // Compare values with independent JavaScript calculations in _expectations.md.
        let summary = summary(&storage, "2020-01-01", "2100-01-01");
        assert_eq!(summary.totals.call_count, exp.calls, "{}", exp.fixture);
        assert_eq!(
            summary.totals.input_total_known,
            Some(exp.input_total),
            "{}",
            exp.fixture
        );
        assert_eq!(
            summary.totals.cache_read_known,
            Some(exp.cache_read),
            "{}",
            exp.fixture
        );
        assert_eq!(
            summary.totals.output_total_known,
            Some(exp.output_total),
            "{}",
            exp.fixture
        );
        assert_eq!(
            summary.totals.total_tokens_known,
            Some(exp.total_tokens),
            "{}",
            exp.fixture
        );

        // Preserve known_version, rollout_legacy parser, and sequence identity without response_id.
        let (basis, schema_version, parser, record_key): (Option<String>, String, String, String) =
            storage
                .conn()
                .query_row(
                    "SELECT parse_basis, schema_version, parser_version, source_record_key
                 FROM usage_events LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .unwrap();
        assert_eq!(basis.as_deref(), Some("known_version"), "{}", exp.fixture);
        assert_eq!(schema_version, exp.version, "{}", exp.fixture);
        assert_eq!(parser, "codex-rollout-legacy-1", "{}", exp.fixture);
        assert!(
            record_key.starts_with("seq:"),
            "legacy identity is session UUID + line number: {record_key}"
        );
        // Sum reasoning subsets; the carried response contributes a known zero in this sample.
        let reasoning: i64 = storage
            .conn()
            .query_row(
                "SELECT COALESCE(SUM(output_reasoning), 0) FROM usage_events",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reasoning, exp.reasoning, "{}", exp.fixture);
        // Model comes from turn_context; subagent category from parent_thread_id/source.subagent.
        let (category, model): (String, Option<String>) = storage
            .conn()
            .query_row(
                "SELECT call_category, model_raw FROM usage_events LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(category, exp.call_category, "{}", exp.fixture);
        assert_eq!(model.as_deref(), Some(exp.model), "{}", exp.fixture);
        let file_status: String = storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(file_status, exp.file_status, "{}", exp.fixture);

        // Compare regular+carried sums against the final snapshot plus carried usage.
        assert_eq!(
            report.reconciliations[0].verdict, exp.verdict,
            "{}",
            exp.fixture
        );
        assert_eq!(
            report.reconciliations[0].snapshot_final,
            Some(exp.snapshot_total),
            "{}",
            exp.fixture
        );
        assert_eq!(
            report.reconciliations[0].carried_sum, exp.carried_sum,
            "{}",
            exp.fixture
        );
        assert_eq!(
            report.reconciliations[0].difference,
            Some(exp.difference),
            "{}",
            exp.fixture
        );
        if exp.verdict == "mismatch" {
            // Source regressions and carried responses retain visible diagnostics.
            let codes: Vec<String> = storage
                .conn()
                .prepare("SELECT DISTINCT code FROM diagnostics")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            for expected in [
                "reconcile_mismatch",
                "snapshot_regression",
                "source_total_mismatch",
            ] {
                assert!(
                    codes.iter().any(|c| c == expected),
                    "fixture {} missing diagnostic {expected}: {codes:?}",
                    exp.fixture
                );
            }
        }

        // Repeat scanning adds no events.
        let reports2 = run_codex(&storage, &root, 1_800_000_000_100);
        let added2: i64 = reports2
            .iter()
            .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
            .sum();
        assert_eq!(added2, 0, "{}", exp.fixture);
    }
}

#[test]
fn legacy_carrier_versions_stay_fallback_not_pretend_verified() {
    // Unregistered older version 0.141.0 remains a LatestFallback compatibility attempt.
    // rollout_v1 finds no individual usage records in this older shape, so imports zero events.
    // Nonempty snapshots differ from zero details and mark the attempted format incompatible.
    // Preserve diagnostics without fabricating usage or advancing the cursor.
    let file = concat!(
        "{\"timestamp\":\"2026-07-01T10:34:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-legacy\",\"session_id\":\"syn-sess-legacy\",\"cli_version\":\"0.141.0\",\"originator\":\"codex_vscode\",\"model_provider\":\"openai\"}}\n",
        "{\"timestamp\":\"2026-07-01T10:34:01.000Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"item_completed\"}}\n",
        "{\"timestamp\":\"2026-07-01T10:34:02.000Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":26502,\"cached_input_tokens\":9600,\"cache_write_input_tokens\":0,\"output_tokens\":745,\"reasoning_output_tokens\":516,\"total_tokens\":27247},\"last_token_usage\":{\"input_tokens\":26502,\"cached_input_tokens\":9600,\"cache_write_input_tokens\":0,\"output_tokens\":745,\"reasoning_output_tokens\":516,\"total_tokens\":27247},\"model_context_window\":258400}}}\n"
    );
    let dir = TempDir::new("codex-legacy");
    let root = dir.path().join("root");
    let sessions = root.join("sessions/2026/07/01");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("rollout-synthetic-legacy.jsonl"), file).unwrap();

    let adapter = CodexAdapter::new();
    let outcome = adapter
        .detect(&sessions.join("rollout-synthetic-legacy.jsonl"))
        .unwrap();
    assert_eq!(
        outcome,
        DetectOutcome::Supported {
            format: "codex-rollout-jsonl".to_string(),
            format_version: Some("0.141.0".to_string()),
            basis: VersionBasis::LatestFallback,
        }
    );

    let (_db, storage) = temp_storage("codex-legacy");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    // Incompatible attempts commit no events, cursors, or aggregates.
    assert_eq!(report.files[0].status, "incompatible");
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        events, 0,
        "no per-call carrier evidence; no fabricated data"
    );
    let aggregates: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM source_aggregates", [], |r| r.get(0))
        .unwrap();
    assert_eq!(aggregates, 0);
    let checkpoints: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        checkpoints, 0,
        "cursor not advanced; retry allowed next round"
    );
}
