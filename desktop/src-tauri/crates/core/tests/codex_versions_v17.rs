//! Codex 逐版本 fixture 回归（M2-D，V17/V30）：
//! 0.153.0 / 0.154.0-alpha.6.1 / 0.154.0-alpha.6.2 三个本机历史版本的脱敏
//! 真实样本，验证注册表分派（KnownVersion）、逐次 usage 数值与 jq 独立核算的
//! 期望一致、parse_basis=known_version、快照对账 matched、重复扫描不增量。
//! 期望值来源：各 fixture 同名 `_expectations.md`（人工核算与测试互核）。

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
    /// token_count 最终快照 total（对账 matched 时等于 Σ逐次）。
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

        // 探测分派：已验证版本 → KnownVersion（探测/扫描同一注册表）。
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

        // 数值与 _expectations.md 的 jq 核算一致。
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

        // 已验证版本不带兼容标记；schema_version 保留来源原始版本。
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
        // reasoning 子集合计与期望一致（summary 未暴露该维度，直接查库）。
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

        // 快照对账 matched（Σ逐次 == 最终快照）。
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

        // 重复扫描不增量。
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
    // 0.139–0.151 本机实测无 token_usage_record（仅 token_count 累计快照），
    // 无逐次载体证据：不预登已验证，按未知版本回退（LatestFallback）。
    // 扫描层对无逐次载体的旧形状判不兼容（可见诊断、不伪造数据、游标不推进）。
    let file = concat!(
        "{\"timestamp\":\"2026-07-01T10:34:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"syn-sess-legacy\",\"session_id\":\"syn-sess-legacy\",\"cli_version\":\"0.142.5\",\"originator\":\"codex_vscode\",\"model_provider\":\"openai\"}}\n",
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
            format_version: Some("0.142.5".to_string()),
            basis: VersionBasis::LatestFallback,
        }
    );

    let (_db, storage) = temp_storage("codex-legacy");
    let reports = run_codex(&storage, &root, 1_800_000_000_000);
    let report = &reports[0];
    // 兼容尝试：无逐次载体 ⇒ 0 事件 + 结构诊断 ⇒ incompatible，不提交事件/游标/聚合。
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
