//! Synthetic oh-my-pi (omp) cases, marked in directory names and file headers, for
//! V17/V30 version policy. Cover cases without native samples: four auxiliary usage record types,
//! assistants without usage, inherited fork deduplication (the same known difference as pi), nested subagent ownership,
//! stopReason=error/aborted, cost mapping, first-line detection (title/session/Pending),
//! latest_fallback for unknown versions (V30 policy; omp has no known-incompatible-format
//! branch), rejection of unknown formats, and unknown entry diagnostics.
//! Expected values are calculated manually from test data; see each _expectations.md.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::omp::OmpAdapter;
use llm_usage_core::domain::VersionBasis;
use rusqlite::OptionalExtension;

fn synthetic_root(name: &str) -> std::path::PathBuf {
    omp_fixture(name)
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

// Manual calculation (synthetic-auxiliary-carriers, 9 lines and 6 events):
// Primary: syn-a1 (100/50/10/5/165) + syn-a2 (no usage; all tokens unknown).
// Auxiliary: syn-u1 (0/0/0/1000/1000), syn-c1 (200/100/0/0/300),
// syn-b1 (300/150/0/0/450), and syn-t1 (10/5/0/0/15).
// Sum: call_count=6, input_total=1625 (derived), uncached=610, cache_read=10,
// cache_write=1005, output=305, total=1930; one usage_shape_deviation (syn-a2).
#[test]
fn auxiliary_carriers_classified_and_summed() {
    let (_db, storage) = temp_storage("omp-aux");
    let root = synthetic_root("synthetic-auxiliary-carriers");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 9);
    assert_eq!(report.files[0].records_seen, 9);
    assert_eq!(report.files[0].events, 6);
    assert_eq!(report.files[0].diagnostics, 1);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (6, 0, 0));

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 6);
    assert_eq!(summary.totals.input_total_known, Some(1_625));
    assert_eq!(summary.totals.uncached_known, Some(610));
    assert_eq!(summary.totals.cache_read_known, Some(10));
    assert_eq!(summary.totals.cache_write_known, Some(1_005));
    assert_eq!(summary.totals.output_total_known, Some(305));
    assert_eq!(summary.totals.total_tokens_known, Some(1_930));
    // syn-a2 has no usage (quality_bucket=unknown): count the call without counting unknown fields.
    assert_eq!(
        summary.totals.input_unknown_count, 0,
        "syn-a2 无 usage 不计未知字段"
    );

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
    let a1 = row_for("omp:message:syn-a1:syn-mc1:2026-01-05T10:00:05.000Z");
    assert_eq!(
        a1,
        (
            "primary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "request_field".into(),
            Some(115)
        )
    );
    // An assistant without usage counts as a call; all tokens remain unknown.
    let a2 = row_for("omp:message:syn-a2:syn-a1:2026-01-05T10:00:06.000Z");
    assert_eq!(a2.0, "primary");
    assert_eq!(a2.4, None, "no usage => tokens unknown");
    let u1 = row_for("omp:usage:syn-u1:syn-a2:2026-01-05T10:00:07.000Z");
    assert_eq!(
        u1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "request_field".into(),
            Some(1000)
        )
    );
    // compaction/branch_summary lack model fields; use omp's combined model_change structure.
    let c1 = row_for("omp:compaction:syn-c1:syn-u1:2026-01-05T10:00:08.000Z");
    assert_eq!(
        c1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "structured_change".into(),
            Some(200)
        )
    );
    let b1 = row_for("omp:branch_summary:syn-b1:syn-c1:2026-01-05T10:00:09.000Z");
    assert_eq!(
        b1,
        (
            "auxiliary".into(),
            Some("syn-model-a".into()),
            Some("syn-provider".into()),
            "structured_change".into(),
            Some(300)
        )
    );
    // toolResult usage describes the tool's own consumption: an auxiliary call with an unknown model.
    let t1 = row_for("omp:toolresult:syn-t1:syn-b1:2026-01-05T10:00:10.000Z");
    assert_eq!(
        t1,
        ("auxiliary".into(), None, None, "unknown".into(), Some(10))
    );

    // Round omp's fractional milliseconds: syn-a1 duration 200.4→200, ttft 60.6→61;
    // syn-a2 duration 150.5→151; missing ttft remains None.
    let latency = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT duration_ms, ttft_ms FROM usage_events WHERE source_record_key = ?1",
                [key],
                |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?)),
            )
            .unwrap()
    };
    assert_eq!(
        latency("omp:message:syn-a1:syn-mc1:2026-01-05T10:00:05.000Z"),
        (Some(200), Some(61))
    );
    assert_eq!(
        latency("omp:message:syn-a2:syn-a1:2026-01-05T10:00:06.000Z"),
        (Some(151), None)
    );
    assert_eq!(
        latency("omp:usage:syn-u1:syn-a2:2026-01-05T10:00:07.000Z"),
        (None, None),
        "辅助载体无延迟字段"
    );
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
}

// Manual calculation (synthetic-fork-inherited): the source has 2 events (150+300); the fork copies
// source lines L3–L5 verbatim and adds assistant syn-fa-3 (15). There are 3 calls and input_total=350
// (derived input+cacheRead+cacheWrite: (100+0+0)+(200+40+0)+(10+0+0)),
// cache_read=40, output=115, total=465. See test _expectations.md for the known difference.
#[test]
fn fork_inherited_entries_dedup_across_files() {
    let dir = TempDir::new("omp-fork");
    let case = synthetic_root("synthetic-fork-inherited").join("sessions/--C--syn--");
    let source = std::fs::read(
        case.join("2026-01-05T10-00-00-060Z_00000000-0000-7000-8000-00000000d060.jsonl"),
    )
    .unwrap();
    let fork = std::fs::read(
        case.join("2026-01-05T11-00-00-061Z_00000000-0000-7000-8000-00000000d061.jsonl"),
    )
    .unwrap();
    // Scan the source alone first, then add the fork; each file is scanned once per round.
    let root = omp_root_with_file(
        &dir,
        "--C--syn--/2026-01-05T10-00-00-060Z_00000000-0000-7000-8000-00000000d060.jsonl",
        &source,
    );
    let (_db, storage) = temp_storage("omp-fork");
    let first = run_omp(&storage, &root, 1_800_000_000_000);
    assert_eq!(first[0].files.len(), 1);
    assert_eq!(first[0].outcome.as_ref().unwrap().added, 2);

    omp_root_with_file(
        &dir,
        "--C--syn--/2026-01-05T11-00-00-061Z_00000000-0000-7000-8000-00000000d061.jsonl",
        &fork,
    );
    let second = run_omp(&storage, &root, 1_800_000_000_000 + 1000);
    let report = &second[0];
    assert_eq!(report.files.len(), 2);
    assert_eq!(report.files[0].status, "unchanged", "源文件无变化短路");
    assert_eq!(report.files[1].status, "complete");
    assert_eq!(
        report.files[1].events, 3,
        "fork 文件产出 2 复制条目 + 1 新条目"
    );
    let outcome = report.outcome.as_ref().unwrap();
    // Known difference, as in pi: copied entries have the same four-part key, but session_id/
    // parent_session_id come from the fork header. Stored events with that key have different content: conflict
    // handling keeps the first scanned event, preventing double-counting and preserving source-session ownership.
    assert_eq!(outcome.added, 1, "仅 fork 自有新条目共入");
    assert_eq!(outcome.conflicts, 2, "复制条目与源会话已存事件冲突");
    assert_eq!(outcome.unchanged, 0);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3, "复制件不双计");
    assert_eq!(summary.totals.input_total_known, Some(350));
    assert_eq!(summary.totals.cache_read_known, Some(40));
    assert_eq!(summary.totals.output_total_known, Some(115));
    assert_eq!(summary.totals.total_tokens_known, Some(465));

    // A new fork entry retains the fork session identity and parentSession.
    let (session, parent): (String, String) = storage
        .conn()
        .query_row(
            "SELECT session_id, parent_session_id FROM usage_events
             WHERE source_record_key = 'omp:message:syn-fa-3:syn-fa-2:2026-01-05T11:00:01.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(session, "syn-omp-fork");
    assert_eq!(parent, "syn-omp-src");

    // Copied entries keep the first scanned source session's ownership and are marked as conflicts.
    let kept: (String, Option<String>, i64) = storage
        .conn()
        .query_row(
            "SELECT session_id, parent_session_id, conflict FROM usage_events
             WHERE source_record_key = 'omp:message:syn-fa-1:syn-fm-1:2026-01-05T10:00:02.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(kept.0, "syn-omp-src");
    assert_eq!(kept.1, None);
    assert_eq!(kept.2, 1);
    assert_eq!(diag_count(&storage, "update_conflict"), 2);
    let _ = dir;
}

// Manual calculation (synthetic-subagent-nested): main-session syn-ma-1 (10/5/0/0/15) is primary;
// two-level Research.jsonl syn-sa-1 (100/50/0/0/150) and three-level nested
// Research/Research.Compactor.jsonl syn-sa-2 (200/60/40/0/300) are sub_agent.
// Both parents are the nearest <ts>_<uuid> ancestor directory d070. Sum: call_count=3,
// input_total=350 (derived), cache_read=40, output=115, total=465, distinct sessions=3.
#[test]
fn nested_subagent_parent_from_directory_shape() {
    let (_db, storage) = temp_storage("omp-nested");
    let root = synthetic_root("synthetic-subagent-nested");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 3, "主会话 + 两层子 Agent + 三层嵌套");
    for file in &report.files {
        assert_eq!(file.status, "complete");
        assert_eq!(file.events, 1);
    }
    assert_eq!(report.outcome.as_ref().unwrap().added, 3);

    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT call_category, session_id, parent_session_id FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    let main = row_for("omp:message:syn-ma-1:-:2026-01-05T10:00:05.000Z");
    assert_eq!(main.0, "primary");
    assert_eq!(main.1, "syn-omp-main");
    assert_eq!(main.2, None, "主会话无父");
    let parent_uuid = "00000000-0000-7000-8000-00000000d070";
    let sub1 = row_for("omp:message:syn-sa-1:-:2026-01-05T10:01:05.000Z");
    assert_eq!(sub1.0, "sub_agent");
    assert_eq!(sub1.1, "syn-omp-sub1", "子 Agent 自有会话头");
    assert_eq!(sub1.2.as_deref(), Some(parent_uuid));
    let sub2 = row_for("omp:message:syn-sa-2:-:2026-01-05T10:02:05.000Z");
    assert_eq!(sub2.0, "sub_agent");
    assert_eq!(sub2.1, "syn-omp-sub2");
    assert_eq!(
        sub2.2.as_deref(),
        Some(parent_uuid),
        "隔代不归名：归最近的 <ts>_<uuid> 祖先目录"
    );

    let distinct_sessions: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(DISTINCT session_id) FROM usage_events",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(distinct_sessions, 3);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 3);
    assert_eq!(summary.totals.input_total_known, Some(350));
    assert_eq!(summary.totals.cache_read_known, Some(40));
    assert_eq!(summary.totals.output_total_known, Some(115));
    assert_eq!(summary.totals.total_tokens_known, Some(465));
}

// Manual calculation (synthetic-error-aborted): both events are primary; syn-e-1 error,
// syn-e-2 aborted (no duration/ttft). Sum: input_total=25, cache_read=5,
// cache_write=0, output=25, total=50.
#[test]
fn error_and_aborted_stop_reasons_map_to_error_status() {
    let (_db, storage) = temp_storage("omp-err");
    let root = synthetic_root("synthetic-error-aborted");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 2);

    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT error_status, duration_ms, ttft_ms FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    let e1 = row_for("omp:message:syn-e-1:-:2026-01-05T10:00:01.000Z");
    assert_eq!(e1.0, "error");
    assert_eq!(e1.1, Some(100), "duration 100.4→100");
    assert_eq!(e1.2, None, "ttft 缺字段保持 unknown");
    let e2 = row_for("omp:message:syn-e-2:syn-e-1:2026-01-05T10:00:02.000Z");
    assert_eq!(e2.0, "aborted");
    assert_eq!(e2.1, None, "aborted 条目无 duration（真实同形状）");
    assert_eq!(e2.2, None);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(25));
    assert_eq!(summary.totals.cache_read_known, Some(5));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(25));
    assert_eq!(summary.totals.total_tokens_known, Some(50));
}

// Manual calculation (synthetic-cost-estimated): syn-c1 cost.total=0.058278 maps to 58278 micro-USD,
// estimated; syn-c2 cost.total=0 is not mapped. Sum: input_total=105, output=55, total=160.
#[test]
fn cost_total_positive_maps_estimated_zero_stays_unknown() {
    let (_db, storage) = temp_storage("omp-cost");
    let root = synthetic_root("synthetic-cost-estimated");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
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
        cost_for("omp:message:syn-c1:-:2026-01-05T10:00:01.000Z"),
        (Some(58278), Some("USD".into()), Some("estimated".into()))
    );
    assert_eq!(
        cost_for("omp:message:syn-c2:syn-c1:2026-01-05T10:00:02.000Z"),
        (None, None, None),
        "cost.total=0 与无价目不可区分，不映射"
    );

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(105));
    assert_eq!(summary.totals.total_tokens_known, Some(160));
}

// Manual calculation (synthetic-detect-supported): title (v=1) first, then session v3;
// 5 lines and 1 event (syn-a1: 100/10/0/0/110, duration 100.4→100, ttft 50.4→50).
#[test]
fn detect_supported_title_first_and_full_scan() {
    let adapter = OmpAdapter::new();
    let case = synthetic_root("synthetic-detect-supported");
    let path = case.join(
        "sessions/--C--syn--/2026-01-05T10-00-00-010Z_00000000-0000-7000-8000-00000000d010.jsonl",
    );
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: Some("3".to_string()),
            basis: VersionBasis::KnownVersion,
        }
    );

    let (_db, storage) = temp_storage("omp-detect");
    let reports = run_omp(&storage, &case, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].lines_read, 5);
    assert_eq!(report.files[0].records_seen, 5);
    assert_eq!(report.files[0].events, 1);
    assert_eq!(report.outcome.as_ref().unwrap().added, 1);

    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(
        summary.totals.input_total_known,
        Some(100),
        "派生口径 input+cacheRead+cacheWrite=100+0+0"
    );
    assert_eq!(summary.totals.cache_read_known, Some(0));
    assert_eq!(summary.totals.output_total_known, Some(10));
    assert_eq!(summary.totals.total_tokens_known, Some(110));

    let latency: (i64, i64) = storage
        .conn()
        .query_row(
            "SELECT duration_ms, ttft_ms FROM usage_events
             WHERE source_record_key = 'omp:message:syn-a1:syn-u1:2026-01-05T10:00:05.000Z'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(latency, (100, 50));
}

#[test]
fn detect_session_header_first_also_supported() {
    // omp detection also accepts a session header first, as pi does; retain that documented limit.
    let dir = TempDir::new("omp-detect-pi-shape");
    let path = dir.path().join("pi-shape.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"session\",\"version\":3,\"id\":\"syn-omp-sh\",\"timestamp\":\"2026-01-05T10:00:00.000Z\"}\n",
    )
    .unwrap();
    let adapter = OmpAdapter::new();
    assert_eq!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: Some("3".to_string()),
            basis: VersionBasis::KnownVersion,
        }
    );
    let _ = dir;
}

#[test]
fn detect_pending_when_only_title_without_session() {
    // A title without a session header in the first 4 lines may be an incomplete write; retry detection next round.
    let dir = TempDir::new("omp-detect-title");
    let path = dir.path().join("title-only.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"title\",\"v\":1,\"title\":\"syn-only\",\"updatedAt\":\"2026-01-05T10:00:00.000Z\"}\n",
    )
    .unwrap();
    let adapter = OmpAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
    let _ = dir;
}

#[test]
fn detect_pending_on_empty_file() {
    let dir = TempDir::new("omp-empty");
    let path = dir.path().join("empty.jsonl");
    std::fs::write(&path, b"").unwrap();
    let adapter = OmpAdapter::new();
    assert_eq!(adapter.detect(&path).unwrap(), DetectOutcome::Pending);
    let _ = dir;
}

// Manual calculation (synthetic-unsupported-version retains its historical directory name; behavior changed in V30):
// both an unlisted numeric version (version=4) and a versionless legacy format use
// LatestFallback with session_v3. Older omp files have not been checked or identified as incompatible;
// do not reject them directly. pi has a separate known-incompatible branch. Each file has one event
// (100/10/0/0/110): call_count=2, input_total=200 (derived), output=20,
// total=220; events use parse_basis=latest_fallback; files are active_compat;
// detection records one latest_fallback diagnostic per file.
#[test]
fn v17_unknown_version_falls_back_with_compat_mark() {
    let adapter = OmpAdapter::new();
    let case = synthetic_root("synthetic-unsupported-version");
    let v4 = case.join(
        "sessions/--C--syn--/2026-01-05T10-00-00-020Z_00000000-0000-7000-8000-00000000d020.jsonl",
    );
    let legacy = case.join(
        "sessions/--C--syn--/2026-01-05T10-00-00-021Z_00000000-0000-7000-8000-00000000d021.jsonl",
    );
    // Both unlisted numeric versions and missing versions use the latest parser with compatibility markers.
    assert_eq!(
        adapter.detect(&v4).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: Some("4".to_string()),
            basis: VersionBasis::LatestFallback,
        }
    );
    assert_eq!(
        adapter.detect(&legacy).unwrap(),
        DetectOutcome::Supported {
            format: "omp-session-jsonl".to_string(),
            format_version: None,
            basis: VersionBasis::LatestFallback,
        }
    );

    let (_db, storage) = temp_storage("omp-uv");
    let reports = run_omp(&storage, &case, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files.len(), 2);
    for file in &report.files {
        assert_eq!(file.status, "complete");
        assert_eq!(file.events, 1, "兼容尝试成功，事件正常产出");
    }
    assert_eq!(
        report.files[0].detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: 4)")
    );
    assert_eq!(
        report.files[1].detail.as_deref(),
        Some("latest_fallback: version compatibility unverified (found: missing)")
    );
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.conflicts, outcome.errors),
        (2, 0, 0)
    );

    // Successfully parsed compatible data contributes to statistics.
    let summary = summary(&storage, "2026-01-05", "2026-01-05");
    assert_eq!(summary.totals.call_count, 2);
    assert_eq!(summary.totals.input_total_known, Some(200));
    assert_eq!(summary.totals.output_total_known, Some(20));
    assert_eq!(summary.totals.total_tokens_known, Some(220));

    // Persist compatibility markers in event parse_basis, file active_compat, and detection JSON.
    let row_for = |key: &str| {
        storage
            .conn()
            .query_row(
                "SELECT parse_basis, schema_version, session_id FROM usage_events
                 WHERE source_record_key = ?1",
                [key],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap()
    };
    assert_eq!(
        row_for("omp:message:syn-m1:-:2026-01-05T10:00:05.000Z"),
        (
            Some("latest_fallback".into()),
            "4".into(),
            Some("syn-omp-v4".into())
        )
    );
    // A versionless legacy format has schema_version=unknown and retains the fallback parse basis.
    assert_eq!(
        row_for("omp:message:syn-l1:-:2026-01-05T10:00:05.000Z"),
        (
            Some("latest_fallback".into()),
            "unknown".into(),
            Some("syn-omp-legacy".into())
        )
    );
    assert_eq!(
        diag_count(&storage, "latest_fallback"),
        2,
        "兼容尝试逐文件可见，不是静默处理"
    );
    let statuses: Vec<(String, String)> = {
        let mut stmt = storage
            .conn()
            .prepare("SELECT status, format_status FROM source_files ORDER BY file_id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(statuses.len(), 2);
    for (status, format_status) in &statuses {
        assert_eq!(status, "active_compat");
        let fs: serde_json::Value = serde_json::from_str(format_status).unwrap();
        assert_eq!(fs["basis"], "latest_fallback");
        assert_eq!(fs["compat"], "unverified");
    }
    // file_id order: d020 (version=4), then d021 (missing version).
    let fs0: serde_json::Value = serde_json::from_str(&statuses[0].1).unwrap();
    let fs1: serde_json::Value = serde_json::from_str(&statuses[1].1).unwrap();
    assert_eq!(fs0["found_version"], "4");
    assert_eq!(fs1["found_version"], serde_json::Value::Null);

    // A repeated scan adds no events; compatibility markers preserve deduplication.
    let second = run_omp(&storage, &case, 1_800_000_000_000 + 1000);
    let added2: i64 = second
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0);
}

#[test]
fn v17_unknown_format_fails_closed_not_success_zero() {
    let adapter = OmpAdapter::new();
    let path = synthetic_root("synthetic-not-omp").join(
        "sessions/--C--syn--/2026-01-05T10-00-00-030Z_00000000-0000-7000-8000-00000000d030.jsonl",
    );
    let outcome = adapter.detect(&path).unwrap();
    assert!(
        matches!(outcome, DetectOutcome::UnknownFormat { .. }),
        "首行非 title/session ⇒ UnknownFormat（即使第 2 行是合法 session 头）"
    );

    let (_db, storage) = temp_storage("omp-nf");
    let root = synthetic_root("synthetic-not-omp");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "unknown_format");
    assert_eq!(report.files[0].events, 0);
    assert_eq!(diag_count(&storage, "unknown_format"), 1);
}

// Manual calculation (synthetic-unknown-record-type): one event (syn-m1: 7/3/0/0/10).
// Unknown type brand_new_thing appears twice; diagnose each type once per file.
#[test]
fn unknown_record_type_ignored_with_single_diagnostic() {
    let (_db, storage) = temp_storage("omp-urt");
    let root = synthetic_root("synthetic-unknown-record-type");
    let reports = run_omp(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 5);
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
