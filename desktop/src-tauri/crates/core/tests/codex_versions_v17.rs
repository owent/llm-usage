//! Codex 逐版本 fixture 回归（M2-D，V17/V30）：
//! 0.153.0 / 0.154.0-alpha.6.1 / 0.154.0-alpha.6.2 三个本机历史版本的脱敏
//! 真实样本，验证注册表分派（KnownVersion）、逐次 usage 数值与 jq 独立核算的
//! 期望一致、parse_basis=known_version、快照对账 matched、重复扫描不增量。
//! 期望值来源：各 fixture 同名 `_expectations.md`（人工核算与测试互核）。
//! M2-D 遗留项（2026-09-26）：0.139–0.151 旧载体（token_count/last_token_usage）
//! 分派 rollout_legacy，三个代表 fixture（0.139.0 / 0.142.5 / 0.146.0-alpha.3）
//! 覆盖重复上报去重、压缩摘要回声（carried）、源端回退（regression+mismatch）。

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
fn legacy_carrier_fixtures_dispatch_rollout_legacy_and_match_expectations() {
    // 0.139–0.151 旧载体：无 token_usage_record，逐次证据 = token_count 的
    // last_token_usage（total 增量法判据）。期望值来自
    // rollout-legacy-v*._expectations.md（build 工具 JS 口径独立核算 + 人工核对）。
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
        /// source_files.status（reconcile_mismatch ⇒ degraded）。
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
            file_status: "degraded",
            call_category: "primary",
            model: "gpt-5.6-sol",
        },
    ];
    for exp in cases {
        let sanitized = codex_fixture("").join(exp.fixture);
        let contents = reconstruct_codex_jsonl(&sanitized);
        let dir = TempDir::new("codex-legacy-ver");
        let root = codex_root_with_file(&dir, "rollout-legacy-versioned.jsonl", &contents);

        // 探测分派：已登记旧版本 → KnownVersion（探测/扫描同一注册表）。
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
        // calls = regular + carried（0.146.0-alpha.3 含 1 条压缩摘要回声）。
        assert_eq!(report.files[0].events as i64, exp.calls, "{}", exp.fixture);

        // 数值与 _expectations.md 的 JS 独立核算一致。
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

        // 分派与身份：known_version + rollout_legacy 解析器 + seq 身份（无 response_id）。
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
        // reasoning 子集合计（carried 回声 reasoning=0，包含无害）。
        let reasoning: i64 = storage
            .conn()
            .query_row(
                "SELECT COALESCE(SUM(output_reasoning), 0) FROM usage_events",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reasoning, exp.reasoning, "{}", exp.fixture);
        // 归属：模型按 turn_context、sub_agent 按 parent_thread_id/source.subagent。
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

        // 快照对账：Σ逐次（regular+carried）== 最终快照 + Σcarried 的判定。
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
            // 源端回退与压缩回声的可见诊断（不伪造数据）。
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
    // 未取证登记的旧系列版本（如 0.141.0，本机无样本）仍按未知版本回退
    // （LatestFallback → rollout_v1）：无逐次载体 ⇒ 0 事件 + 结构诊断
    // （reconcile_mismatch：0 逐次 vs 非空快照）⇒ incompatible，
    // 可见诊断、不伪造数据、游标不推进。
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
