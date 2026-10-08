//! Synthetic pi adapter cases, marked in directory names and file headers, for V17/V30 rejection and fallback.
//! Cover cases without native samples: four auxiliary usage record types, assistants without usage, inherited fork
//! deduplication, stopReason=error/aborted, cost mapping, version policy (reject known incompatible formats;
//! use latest_fallback for unlisted numeric versions), rejection of unknown formats, and unknown entry diagnostics.
//! Expected values are calculated manually from test data and checked with jq; see each _expectations.md.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::pi::PiAdapter;
use llm_usage_core::domain::VersionBasis;
use rusqlite::OptionalExtension;

fn synthetic_root(name: &str) -> std::path::PathBuf {
    pi_fixture(name)
}

fn diag_count(storage: &llm_usage_core::storage::Storage, code: &str) -> i64 {
    storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = ?1",
            [code],
            |r| r.get(0),
        )
        .unwrap()
}

// Manual calculation (synthetic-auxiliary-carriers, 8 lines and 6 events):
// Primary: syn-a-1 (1000/50/800/100/1050, reasoning 10) + syn-a-2 (no usage; all tokens unknown).
// Auxiliary: syn-u-1 (100/0/90/0/100), syn-c-1 (1000/200/0/0/1200),
// syn-b-1 (800/100/300/0/900), and syn-t-1 (15/5/0/10/20).
// Sum: call_count=6, input_total=2915, cache_read=1190, cache_write=110,
// output=355, total=3270; one usage_shape_deviation (syn-a-2).
#[test]
fn auxiliary_carriers_classified_and_summed() {
    let (_db, storage) = temp_storage("pi-aux");
    let root = synthetic_root("synthetic-auxiliary-carriers");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 8);
    assert_eq!(report.files[0].records_seen, 8);
    assert_eq!(report.files[0].events, 6);
    assert_eq!(report.files[0].diagnostics, 1);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (6, 0, 0));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 6);
    assert_eq!(summary.totals.input_total_known, Some(2_915));
    assert_eq!(summary.totals.cache_read_known, Some(1_190));
    assert_eq!(summary.totals.cache_write_known, Some(110));
    assert_eq!(summary.totals.output_total_known, Some(355));
    assert_eq!(summary.totals.total_tokens_known, Some(3_270));

    // Category counts: pi has no sub_agent entries; 2 primary + 4 auxiliary.
    let mut stmt = storage
        .conn()
        .prepare("SELECT call_category, COUNT(*) FROM usage_events GROUP BY call_category ORDER BY call_category")
        .unwrap();
    let categories: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        categories,
        vec![("auxiliary".to_string(), 4), ("primary".to_string(), 2)]
    );

    // Check ownership and category by record key.
    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT call_category, model_raw, provider_id, model_attribution, input_total
                 FROM usage_events WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .unwrap()
    };
    let a1 = row_for("pi:message:syn-a-1:syn-mc-1:2026-01-05T10:00:02.000Z");
    assert_eq!(
        a1,
        (
            "primary".into(),
            Some("syn-model-a".into()),
            Some("syn-prov".into()),
            "request_field".into(),
            Some(1000)
        )
    );
    // An assistant without usage counts as a call; all tokens remain unknown.
    let a2 = row_for("pi:message:syn-a-2:syn-a-1:2026-01-05T10:00:03.000Z");
    assert_eq!(a2.0, "primary");
    assert_eq!(a2.4, None, "no usage => tokens unknown");
    let u1 = row_for("pi:usage:syn-u-1:syn-a-2:2026-01-05T10:00:04.000Z");
    assert_eq!(
        u1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-prov".into()),
            "request_field".into(),
            Some(100)
        )
    );
    // compaction/branch_summary lack model fields; use the latest preceding model_change, including equal timestamps.
    let c1 = row_for("pi:compaction:syn-c-1:syn-u-1:2026-01-05T10:00:05.000Z");
    assert_eq!(
        c1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-prov".into()),
            "structured_change".into(),
            Some(1000)
        )
    );
    let b1 = row_for("pi:branch_summary:syn-b-1:syn-c-1:2026-01-05T10:00:06.000Z");
    assert_eq!(
        b1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-prov".into()),
            "structured_change".into(),
            Some(800)
        )
    );
    // toolResult usage describes the tool's own consumption: an auxiliary call with an unknown model.
    let t1 = row_for("pi:toolresult:syn-t-1:syn-b-1:2026-01-05T10:00:07.000Z");
    assert_eq!(
        t1,
        ("auxiliary".into(), None, None, "unknown".into(), Some(15))
    );

    // Reasoning is a subset of output; do not add it again. Only syn-a-1 reports 10.
    let reasoning: i64 = storage
        .conn()
        .query_row("SELECT SUM(output_reasoning) FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(reasoning, 10);
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
}

// Manual calculation (synthetic-fork-inherited): the source has 2 events (150+300); the fork copies
// source lines L2–L4 verbatim and adds assistant syn-fa-3 (15). There are 3 calls and input_total=350
// (derived input+cacheRead+cacheWrite: 100+240+10), cache_read=40, output=115, total=465.
#[test]
fn fork_inherited_entries_dedup_across_files() {
    let dir = TempDir::new("pi-fork");
    let case = synthetic_root("synthetic-fork-inherited").join("sessions/--C--Users-syn--");
    let source = std::fs::read(case.join("2026-01-05T10-00-00-000Z_syn-sess-src.jsonl")).unwrap();
    let fork = std::fs::read(case.join("2026-01-05T11-00-00-000Z_syn-sess-fork.jsonl")).unwrap();
    // Scan the source alone first, then add the fork; each file is scanned once per round.
    let root = pi_root_with_file(
        &dir,
        "--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-src.jsonl",
        &source,
    );
    let (_db, storage) = temp_storage("pi-fork");
    let first = run_pi(&storage, &root, 1_800_000_000_000);
    assert_eq!(first[0].files.len(), 1);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);

    pi_root_with_file(
        &dir,
        "--C--Users-syn--/2026-01-05T11-00-00-000Z_syn-sess-fork.jsonl",
        &fork,
    );
    let second = run_pi(&storage, &root, 1_800_000_000_000 + 1000);
    let report = &second[0];
    assert_eq!(report.files.len(), 2);
    assert_eq!(report.files[0].status, "unchanged", "源文件无变化短路");
    assert_eq!(report.files[1].status, "complete");
    assert_eq!(
        report.files[1].events, 3,
        "fork 文件产出 2 复制条目 + 1 新条目"
    );
    let outcome = report.outcome.as_ref().unwrap();
    // Known difference (see test _expectations.md): copied entries have the same four-part key, but session_id/
    // parent_session_id come from the fork header. Stored events with that key have different content: conflict
    // handling keeps the first scanned event, rather than the capability's claimed same-key/same-content Keep.
    // Repeated reads do not double-count; ownership remains with the first scanned source session.
    assert_eq!(outcome.added, 1, "仅 fork 自有新条目共入");
    assert_eq!(outcome.conflicts, 2, "复制条目与源会话已存事件冲突");
    assert_eq!(outcome.unchanged, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3, "复制件不双计");
    // Derived input_total=input+cacheRead+cacheWrite: (100+0+0)+(200+40+0)+(10+0+0)=350.
    assert_eq!(summary.totals.input_total_known, Some(350));
    assert_eq!(summary.totals.cache_read_known, Some(40));
    assert_eq!(summary.totals.output_total_known, Some(115));
    assert_eq!(summary.totals.total_tokens_known, Some(465));

    // A new fork entry retains the fork session identity and parentSession.
    let (session, parent): (String, String) = storage
        .conn()
        .query_row(
            "SELECT session_id, parent_session_id FROM usage_events
             WHERE source_record_key = 'pi:message:syn-fa-3:syn-fa-2:2026-01-05T11:00:01.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(session, "syn-sess-fork");
    assert_eq!(parent, "syn-sess-src");

    // Copied entries keep the first scanned source session's ownership and are marked as conflicts.
    let kept: (String, Option<String>, i64) = storage
        .conn()
        .query_row(
            "SELECT session_id, parent_session_id, conflict FROM usage_events
             WHERE source_record_key = 'pi:message:syn-fa-1:syn-fm-1:2026-01-05T10:00:02.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(kept.0, "syn-sess-src");
    assert_eq!(kept.1, None);
    assert_eq!(kept.2, 1);
    assert_eq!(diag_count(&storage, "update_conflict"), 2);
    let _ = dir;
}

// Manual calculation (synthetic-error-aborted): both events are primary; input_total=20,
// cache_read=5, cache_write=0, output=25, total=45.
#[test]
fn error_and_aborted_stop_reasons_map_to_error_status() {
    let (_db, storage) = temp_storage("pi-err");
    let root = synthetic_root("synthetic-error-aborted");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 2);

    let status_for = |key: &str| -> String {
        storage
            .conn()
            .query_row(
                "SELECT error_status FROM usage_events WHERE source_record_key = ?1",
                [key],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(
        status_for("pi:message:syn-e-1:-:2026-01-05T10:00:01.000Z"),
        "error"
    );
    assert_eq!(
        status_for("pi:message:syn-e-2:syn-e-1:2026-01-05T10:00:02.000Z"),
        "aborted"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(20));
    assert_eq!(summary.totals.cache_read_known, Some(5));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(25));
    assert_eq!(summary.totals.total_tokens_known, Some(45));
}

// Manual calculation (synthetic-cost-estimated): syn-c-1 cost.total=0.005 maps to 5000 micro-USD,
// estimated; syn-c-2 cost.total=0 is not mapped. Sum: input_total=105, output=55, total=160.
#[test]
fn cost_total_positive_maps_estimated_zero_stays_unknown() {
    let (_db, storage) = temp_storage("pi-cost");
    let root = synthetic_root("synthetic-cost-estimated");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].events, 2);

    let cost_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT cost_amount_minor, cost_currency, cost_kind FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    assert_eq!(
        cost_for("pi:message:syn-c-1:-:2026-01-05T10:00:01.000Z"),
        (Some(5000), Some("USD".into()), Some("estimated".into()))
    );
    assert_eq!(
        cost_for("pi:message:syn-c-2:syn-c-1:2026-01-05T10:00:02.000Z"),
        (None, None, None),
        "cost.total=0 与无价目不可区分，不映射"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    // Derived input_total=input+cacheRead+cacheWrite: (100+0+0)+(5+0+0)=105.
    assert_eq!(summary.totals.input_total_known, Some(105));
    assert_eq!(summary.totals.total_tokens_known, Some(160));
}

// V30 policy (synthetic-unknown-version): an unlisted numeric version (version=4) uses the latest built-in
// parser and stores compatibility markers. The versionless legacy format is known to be incompatible:
// pinned v1/v2 source omitted version, id, and parentId; reject that format.
#[test]
fn v17_unknown_version_fallback_or_evidenced_reject() {
    let adapter = PiAdapter::new();
    let case = synthetic_root("synthetic-unknown-version");
    let v4 = case.join("sessions/--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-v4.jsonl");
    let legacy =
        case.join("sessions/--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-legacy.jsonl");
    assert_eq!(
        adapter.detect(&v4).unwrap(),
        DetectOutcome::Supported {
            format: "pi-session-jsonl".to_string(),
            format_version: Some("4".to_string()),
            basis: VersionBasis::LatestFallback,
        }
    );
    match adapter.detect(&legacy).unwrap() {
        DetectOutcome::UnsupportedVersion {
            format,
            found,
            reason,
        } => {
            assert_eq!(format, "pi-session-jsonl");
            assert_eq!(
                found, None,
                "缺失 version 按 legacy 形态拒绝（found: None）"
            );
            assert!(
                reason.contains("legacy"),
                "reason 须注明 legacy 依据：{reason}"
            );
        }
        other => panic!("legacy shape should be UnsupportedVersion, got {other:?}"),
    }

    let (_db, storage) = temp_storage("pi-uv");
    let reports = run_pi(&storage, &case, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 2);
    let file_for = |frag: &str| {
        report
            .files
            .iter()
            .find(|f| f.file_id.contains(frag))
            .unwrap_or_else(|| panic!("no report file containing {frag}"))
            .clone()
    };
    // v4: latest_fallback imports one event (syn-v4-1: 1/1/0/0/2).
    let v4_file = file_for("syn-sess-v4");
    assert_eq!(v4_file.status, "complete");
    assert_eq!(v4_file.events, 1);
    assert_eq!(
        v4_file.detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: 4)")
    );
    // Reject the known incompatible legacy format explicitly; do not report a successful empty scan.
    let legacy_file = file_for("syn-sess-legacy");
    assert_eq!(legacy_file.status, "unsupported_version");
    assert_eq!(legacy_file.events, 0);

    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 1, "仅 legacy 被拒绝；v4 兼容入库");
    assert_eq!(diag_count(&storage, "latest_fallback"), 1);
    assert_eq!(
        diag_count(&storage, "unsupported_version"),
        1,
        "显式拒绝逐文件落诊断；不是静默的成功 0 条"
    );
    // File states: v4 active_compat with compatibility markers; legacy unsupported.
    let status_for = |frag: &str| -> String {
        storage
            .conn()
            .query_row(
                "SELECT status FROM source_files WHERE file_id LIKE '%' || ?1 || '%'",
                [frag],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(status_for("syn-sess-v4"), "active_compat");
    assert_eq!(status_for("syn-sess-legacy"), "unsupported");
}

// Manual calculation (synthetic-version-fallback, unlisted version=4): both events are primary:
// syn-vf-1 (10/5/0/0/15), syn-vf-2 (20/10/0/0/30); call_count=2,
// input_total=30, output=15, total=45. All events use parse_basis=latest_fallback;
// schema_version retains the source's declared "4"; the file is active_compat with one latest_fallback diagnostic.
// A repeated scan adds no events.
#[test]
fn unrecorded_version_falls_back_with_compat_marks() {
    let (_db, storage) = temp_storage("pi-vf");
    let root = synthetic_root("synthetic-version-fallback");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 3);
    assert_eq!(report.files[0].records_seen, 3);
    assert_eq!(report.files[0].events, 2);
    assert_eq!(
        report.files[0].detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: 4)")
    );
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (2, 0, 0));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    // Derived input_total=input+cacheRead+cacheWrite: 10+20.
    assert_eq!(summary.totals.input_total_known, Some(30));
    assert_eq!(summary.totals.output_total_known, Some(15));
    assert_eq!(summary.totals.total_tokens_known, Some(45));

    // Persist compatibility markers: event parse_basis=latest_fallback; schema_version retains the source declaration.
    let rows: Vec<(String, String)> = storage
        .conn()
        .prepare("SELECT parse_basis, schema_version FROM usage_events ORDER BY source_record_key")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        rows,
        vec![
            ("latest_fallback".to_string(), "4".to_string()),
            ("latest_fallback".to_string(), "4".to_string()),
        ]
    );
    assert_eq!(diag_count(&storage, "latest_fallback"), 1);

    // File state and detection JSON: active_compat, basis/found_version/compat.
    let (file_status, format_status): (String, String) = storage
        .conn()
        .query_row("SELECT status, format_status FROM source_files", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(file_status, "active_compat");
    let fs: serde_json::Value = serde_json::from_str(&format_status).unwrap();
    assert_eq!(fs["format"], "pi-session-jsonl");
    assert_eq!(fs["found_version"], "4");
    assert_eq!(fs["basis"], "latest_fallback");
    assert_eq!(fs["compat"], "unverified");

    // A repeated scan adds no events; compatibility markers preserve deduplication.
    let reports2 = run_pi(&storage, &root, 1_800_000_000_100);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0);
}

#[test]
fn v17_unknown_format_fails_closed_not_success_zero() {
    let adapter = PiAdapter::new();
    let path = synthetic_root("synthetic-not-pi")
        .join("sessions/--C--Users-syn--/2026-01-05T10-00-00-000Z_syn-sess-np.jsonl");
    let outcome = adapter.detect(&path).unwrap();
    assert!(
        matches!(outcome, DetectOutcome::UnknownFormat { .. }),
        "首行非 session 头 ⇒ UnknownFormat"
    );

    let (_db, storage) = temp_storage("pi-np");
    let root = synthetic_root("synthetic-not-pi");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "unknown_format");
    assert_eq!(report.files[0].events, 0);
    assert_eq!(diag_count(&storage, "unknown_format"), 1);
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("pi-empty");
    let path = dir.path().join("empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = PiAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
    let _ = dir;
}

// Manual calculation (synthetic-unknown-record-type): one event (syn-m-1: 7/3/0/0/10).
// Unknown type brand_new_thing appears twice; diagnose each type once per file.
#[test]
fn unknown_record_type_ignored_with_single_diagnostic() {
    let (_db, storage) = temp_storage("pi-urt");
    let root = synthetic_root("synthetic-unknown-record-type");
    let reports = run_pi(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 4);
    assert_eq!(report.files[0].events, 1);
    assert_eq!(report.files[0].diagnostics, 1);
    assert_eq!(diag_count(&storage, "unknown_record_type"), 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(7));
    assert_eq!(summary.totals.output_total_known, Some(3));
    assert_eq!(summary.totals.total_tokens_known, Some(10));
    // An unknown entry type does not reject the file; its state remains active.
    let status: Option<String> = storage
        .conn()
        .query_row("SELECT status FROM source_files", [], |r| r.get(0))
        .optional()
        .unwrap();
    assert_eq!(status.as_deref(), Some("active"));
}
