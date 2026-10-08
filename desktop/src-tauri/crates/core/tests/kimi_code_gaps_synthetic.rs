//! Kimi Code (A12, M4) boundary tests use directories and headers marked synthetic.
//! Cover error steps without usage, unregistered protocol fallback, independently verified
//! 1.4/1.5 selection, echo/completed duplicate prevention,
//! invalid negative values/scopes/second timestamps without guessed conversion, and direct detection.
//! Expected values are calculated manually in each dataset's _expectations.md.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::kimi_code::KimiCodeAdapter;
use llm_usage_core::storage::Storage;

const NOW: i64 = 1_800_000_000_000;

fn diag_count(storage: &Storage, code: &str) -> i64 {
    storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = ?1",
            [code],
            |r| r.get(0),
        )
        .unwrap()
}

fn event_count(storage: &Storage) -> i64 {
    storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap()
}

/// Error/retry wire data without usage.record is a valid shape:
/// complete, zero events and zero diagnostics, without invented zero usage.
#[test]
fn no_usage_records_is_normal_shape() {
    let (_db, storage) = temp_storage("kimi-code-nousage");
    let root = kimi_code_fixture("synthetic-no-usage");
    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 0);
    assert_eq!(reports[0].files[0].diagnostics, 0);
    assert_eq!(event_count(&storage), 0);
    let diags: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
    // No numeric source row is fabricated from absent usage.
    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 0);
}

/// Unregistered protocol "9.9" tries latest_fallback compatibility;
/// valid usage is retained as unverified, without rejecting the version number alone (V30).
#[test]
fn unknown_protocol_version_falls_back_with_compat_flag() {
    let (_db, storage) = temp_storage("kimi-code-unknown");
    let root = kimi_code_fixture("synthetic-unknown-version");
    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(diag_count(&storage, "latest_fallback"), 1);
    let (basis, schema): (String, String) = storage
        .conn()
        .query_row(
            "SELECT parse_basis, schema_version FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(basis, "latest_fallback");
    assert_eq!(schema, "9.9", "原始版本透传 schema_version");

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.uncached_known, Some(100));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.total_tokens_known, Some(550));
}

/// step.end echoes equal usage.record fields; subagent.completed.usage snapshots
/// child wire sums. Neither creates events or adds duplicate usage.
#[test]
fn echo_and_subagent_completed_never_double_count() {
    let (_db, storage) = temp_storage("kimi-code-echo");
    let root = kimi_code_fixture("synthetic-echo-defense");
    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 2, "1 primary + 1 auxiliary");

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
    // Adding echo/completed usage would inflate these four totals.
    assert_eq!(summary.totals.uncached_known, Some(1_000));
    assert_eq!(summary.totals.cache_read_known, Some(400));
    assert_eq!(summary.totals.output_total_known, Some(120));
    assert_eq!(summary.totals.total_tokens_known, Some(1_520));

    let conn = storage.conn();
    let (primary, auxiliary): (i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'auxiliary') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (primary, auxiliary),
        (1, 1),
        "session scope（压缩摘要）= auxiliary"
    );

    // No event may contain completed snapshot values {123,45,678,0}.
    let leaked: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE input_uncached = 123 AND output_total = 45",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0);

    // One turn and its echo have equal sums, producing matched reconciliation.
    assert!(reports[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "matched"));
    assert!(reports[0]
        .reconciliations
        .iter()
        .any(|r| r.series.starts_with("kimi_subagent_completed_snapshot")));
}

/// Diagnose and skip negative values, unknown usageScope and second timestamps.
/// Valid records remain; do not multiply seconds by 1000. All 82 inspected native files used milliseconds.
#[test]
fn bad_shapes_skip_without_guessing() {
    let (_db, storage) = temp_storage("kimi-code-bad");
    let root = kimi_code_fixture("synthetic-bad-shapes");
    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 1, "仅 1 条正常 turn 入账");
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
    assert_eq!(diag_count(&storage, "usage_scope_unknown"), 1);
    assert_eq!(diag_count(&storage, "timestamp_unparseable"), 1);

    let conn = storage.conn();
    let (input_uncached, cache_write): (i64, i64) = conn
        .query_row(
            "SELECT input_uncached, input_cache_write FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    // The only valid synthetic record {200,60,800,10} reports cache write 10;
    // inspected native cache writes were zero, so this test separately checks the positive mapping.
    assert_eq!((input_uncached, cache_write), (200, 10));
    // Second-scale time 1_767_225_600 is not converted into an event.
    let seconds_row: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE occurred_at_ms < 946684800000",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(seconds_row, 0);

    let file_status: String = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_status, "degraded");
}

/// Same-millisecond usage.record entries use ordinal identity suffixes; this synthetic case was not observed locally.
#[test]
fn same_millisecond_records_get_sequence_suffix() {
    let dir = TempDir::new("kimi-code-dup");
    let wire = concat!(
        r#"{"type":"metadata","protocol_version":"1.5","created_at":1767225600000}"#,
        "\n",
        r#"{"type":"usage.record","agentId":"main","model":"syn","usage":{"inputOther":1,"output":2,"inputCacheRead":3,"inputCacheCreation":0},"usageScope":"turn","time":1767225601000}"#,
        "\n",
        r#"{"type":"usage.record","agentId":"main","model":"syn","usage":{"inputOther":10,"output":20,"inputCacheRead":30,"inputCacheCreation":0},"usageScope":"turn","time":1767225601000}"#,
        "\n",
    );
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/session_syn-dup/agents/main/wire.jsonl",
        wire.as_bytes(),
    );
    let (_db, storage) = temp_storage("kimi-code-dup");
    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 2);
    assert_eq!(diag_count(&storage, "usage_time_collision"), 1);
    let keys: Vec<String> = {
        let conn = storage.conn();
        let mut stmt = conn
            .prepare("SELECT source_record_key FROM usage_events ORDER BY source_record_key")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        keys,
        vec![
            "kimi-code:usage:session_syn-dup:main:1767225601000:0".to_string(),
            "kimi-code:usage:session_syn-dup:main:1767225601000:1".to_string(),
        ]
    );
    // Rescanning ordinal identities adds no events.
    let reports2 = run_kimi_code(&storage, &root, NOW + 1000);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0);
}

/// Empty detection is Pending; a non-JSON or nonmetadata first line is UnknownFormat.
#[test]
fn detect_pending_and_unknown_format() {
    let adapter = KimiCodeAdapter::new();
    let dir = TempDir::new("kimi-code-detect");

    let empty = dir.path().join("wire.jsonl");
    std::fs::write(&empty, b"").unwrap();
    assert_eq!(adapter.detect(&empty).unwrap(), DetectOutcome::Pending);

    let nonjson = dir.path().join("wire2.jsonl");
    std::fs::write(&nonjson, b"not a json line\n").unwrap();
    assert!(matches!(
        adapter.detect(&nonjson).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));

    let notmeta = dir.path().join("wire3.jsonl");
    std::fs::write(
        &notmeta,
        b"{\"type\":\"usage.record\",\"usageScope\":\"turn\",\"time\":1767225601000}\n",
    )
    .unwrap();
    assert!(matches!(
        adapter.detect(&notmeta).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

/// Independently verified Kimi Code protocol versions 1.5 and 1.4 both select KnownVersion.
#[test]
fn detect_registry_dispatches_by_own_anchor() {
    let adapter = KimiCodeAdapter::new();
    let dir = TempDir::new("kimi-code-dispatch");
    for (version, expected_basis) in [("1.5", "known"), ("1.4", "known")] {
        let path = dir.path().join(format!("wire-{version}.jsonl"));
        std::fs::write(
            &path,
            format!(
                r#"{{"type":"metadata","protocol_version":"{version}","created_at":1767225600000}}"#
            )
            .as_bytes()
            .iter()
            .copied()
            .chain(std::iter::once(0x0A))
            .collect::<Vec<u8>>(),
        )
        .unwrap();
        match adapter.detect(&path).unwrap() {
            DetectOutcome::Supported {
                basis,
                format_version,
                ..
            } => {
                assert_eq!(format_version.as_deref(), Some(version));
                let is_known = basis == llm_usage_core::domain::VersionBasis::KnownVersion;
                assert_eq!(
                    is_known,
                    expected_basis == "known",
                    "version {version}: basis {basis:?}"
                );
            }
            other => panic!("expected Supported for {version}, got {other:?}"),
        }
    }
}
