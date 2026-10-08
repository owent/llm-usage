//! Kimi Work (A13, M4) boundary tests use directories and headers marked synthetic.
//! Cover absent usage, unknown protocol_version fallback (including Kimi Code 1.5,
//! absent from this product's registry), echo/micro_compaction duplicate prevention,
//! invalid negative values/scopes/second timestamps, 1.4 directory identity without agentId,
//! and direct detection.

mod common;

use common::*;
use llm_usage_core::adapters::framework::{DetectOutcome, SourceAdapter};
use llm_usage_core::adapters::kimi_work::KimiWorkAdapter;
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

#[test]
fn no_usage_records_is_normal_shape() {
    let (_db, storage) = temp_storage("kimi-work-nousage");
    let root = kimi_work_fixture("synthetic-no-usage");
    let reports = run_kimi_work(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].status, "complete");
    assert_eq!(reports[0].files[0].events, 0);
    assert_eq!(reports[0].files[0].diagnostics, 0, "1.4 已知类型静默忽略");
    let diags: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
}

/// kimi-work registers only 1.4, so "1.5" selects latest_fallback.
/// Products sharing the wire family retain independent verification (A12/A13).
#[test]
fn code_anchor_1_5_is_fallback_in_work_registry() {
    let (_db, storage) = temp_storage("kimi-work-unknown");
    let root = kimi_work_fixture("synthetic-unknown-version");
    let reports = run_kimi_work(&storage, &root, NOW);
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
    assert_eq!(schema, "1.5");

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 1);
    assert_eq!(summary.totals.input_total_known, Some(500));
    assert_eq!(summary.totals.total_tokens_known, Some(550));
    // Equal echo and record values count only the usage.record side.
    assert_eq!(summary.totals.uncached_known, Some(100));
}

/// Equal step.end echo and usage.record count once; micro_compaction.apply
/// creates no event, while separate session-scoped usage counts as auxiliary.
#[test]
fn echo_and_micro_compaction_never_double_count() {
    let (_db, storage) = temp_storage("kimi-work-echo");
    let root = kimi_work_fixture("synthetic-echo-defense");
    let reports = run_kimi_work(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 2, "1 primary + 1 auxiliary");

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
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
    assert_eq!((primary, auxiliary), (1, 1));
    assert!(reports[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "matched"));
}

/// Diagnose and skip negative values, unknown scope and second timestamps under shared rules.
/// Only the valid record {200,60,800,10} contributes usage.
#[test]
fn bad_shapes_skip_without_guessing() {
    let (_db, storage) = temp_storage("kimi-work-bad");
    let root = kimi_work_fixture("synthetic-bad-shapes");
    let reports = run_kimi_work(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 1);
    assert_eq!(diag_count(&storage, "usage_shape_deviation"), 1);
    assert_eq!(diag_count(&storage, "usage_scope_unknown"), 1);
    assert_eq!(diag_count(&storage, "timestamp_unparseable"), 1);

    let conn = storage.conn();
    let seconds_row: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE occurred_at_ms < 946684800000",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(seconds_row, 0, "秒值不 ×1000 猜测");

    let (input_uncached, cache_write, total): (i64, i64, i64) = conn
        .query_row(
            "SELECT input_uncached, input_cache_write, total_tokens FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((input_uncached, cache_write, total), (200, 10, 1_070));
}

/// In 1.4, absent agentId uses agents/<id>/ for identity;
/// a nonmain directory is sub_agent, using minimal synthetic data based on real agent-44 behavior.
#[test]
fn agent_identity_falls_back_to_directory_when_agentid_absent() {
    let dir = TempDir::new("kimi-work-noagentid");
    let wire = concat!(
        r#"{"type":"metadata","protocol_version":"1.4","created_at":1767225600000}"#,
        "\n",
        r#"{"type":"usage.record","model":"k3-agent","usage":{"inputOther":10,"output":5,"inputCacheRead":0,"inputCacheCreation":0},"usageScope":"turn","time":1767225601000}"#,
        "\n",
    );
    let root = kimi_root_with_file(
        &dir,
        "wd_syn/conv_syn-ag/agents/agent-7/wire.jsonl",
        wire.as_bytes(),
    );
    let (_db, storage) = temp_storage("kimi-work-noagentid");
    let reports = run_kimi_work(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 1);
    let (category, session): (String, String) = storage
        .conn()
        .query_row(
            "SELECT call_category, session_id FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent", "agents/agent-7 ⇒ sub_agent");
    assert_eq!(session, "conv_syn-ag", "会话身份来自 conv 目录名");
}

/// Direct detection: empty is Pending; non-JSON/nonmetadata is UnknownFormat;
/// 1.4 is KnownVersion; Kimi Code's 1.5 is LatestFallback here.
#[test]
fn detect_dispatches_by_own_anchor() {
    let adapter = KimiWorkAdapter::new();
    let dir = TempDir::new("kimi-work-detect");

    let empty = dir.path().join("wire.jsonl");
    std::fs::write(&empty, b"").unwrap();
    assert_eq!(adapter.detect(&empty).unwrap(), DetectOutcome::Pending);

    let nonjson = dir.path().join("wire2.jsonl");
    std::fs::write(&nonjson, b"not a json line\n").unwrap();
    assert!(matches!(
        adapter.detect(&nonjson).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));

    for (version, expected_known) in [("1.4", true), ("1.5", false), ("9.9", false)] {
        let path = dir.path().join(format!("wire-v{version}.jsonl"));
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
            DetectOutcome::Supported { basis, .. } => {
                assert_eq!(
                    basis == llm_usage_core::domain::VersionBasis::KnownVersion,
                    expected_known,
                    "version {version} basis {basis:?}"
                );
            }
            other => panic!("expected Supported for {version}, got {other:?}"),
        }
    }
}

/// Without a home context, discovery uses only manual roots. DefaultHome candidates
/// require home_dir as in claude/pi/qwen; an isolated home must not access the personal install.
/// Real candidate existence belongs to real_verify_kimi_work, not unit-test assertions.
#[test]
fn discover_uses_observed_candidate_and_manual_roots() {
    let adapter = KimiWorkAdapter::new();
    let dir = TempDir::new("kimi-work-disc");
    let isolated = llm_usage_core::adapters::framework::DiscoverContext {
        home_dir: Some(dir.path().join("isolated-home")),
        ..Default::default()
    };
    assert!(
        adapter.discover(&isolated).is_empty(),
        "isolated home must not discover the fixed personal installation"
    );
    let home_root = kimi_root_with_file(
        &dir,
        "wd_syn/conv_syn-1/agents/main/wire.jsonl",
        concat!(
            r#"{"type":"metadata","protocol_version":"1.4","created_at":1767225600000}"#,
            "\n",
        )
        .as_bytes(),
    );
    assert_eq!(home_root, dir.path().to_path_buf());
    let ctx = llm_usage_core::adapters::framework::DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![home_root], // A sessions child makes this a home-style root.
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(
        roots.len(),
        1,
        "无 home 上下文 ⇒ 仅手工根（隔离，不触本机数据）"
    );
    assert_eq!(
        roots[0].root,
        dir.path().join("sessions"),
        "manual home 根解析到 sessions"
    );
    assert_eq!(
        roots[0].basis,
        llm_usage_core::adapters::framework::RootBasis::Manual
    );
    let root = roots[0].root.clone();
    // Passing the same sessions directory directly also resolves to the same root.
    let ctx2 = llm_usage_core::adapters::framework::DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.clone()],
    };
    let roots2 = adapter.discover(&ctx2);
    assert!(roots2.iter().any(|r| r.root == root));
}
