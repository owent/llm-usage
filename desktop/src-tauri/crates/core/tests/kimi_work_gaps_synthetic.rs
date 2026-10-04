//! Kimi Work（A13，M4）缺口场景：合成样本（目录/文件头均标 synthetic）。
//! 覆盖：无 usage 事件、未知 protocol_version fallback（含 1.5——Kimi Code 的
//! 锚点在本产品注册表未收录，证明两注册表独立）、回声/micro_compaction 双计
//! 防御、坏形状（负值/未知 scope/秒级时间）、1.4 无 agentId 的目录身份回退、
//! detect 直测。

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

/// "1.5" 在 kimi-work 注册表未收录（锚点只有 1.4）⇒ latest_fallback：
/// 同一 wire 家族的两产品验证态独立（A12/A13）。
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
    // 回声与记录同值：只计记录侧。
    assert_eq!(summary.totals.uncached_known, Some(100));
}

/// 回声/micro_compaction 防双计：step.end 回声与 usage.record 相同只计一次；
/// micro_compaction.apply 控制记录不产事件；session scope 独立计 auxiliary。
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

/// 坏形状隔离：沿用同家族处理规则——负值/未知 scope/秒级时间各记诊断跳过；
/// 唯一正常记录 {200,60,800,10} 入账。
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

/// 1.4 usage.record 无 agentId：代理身份回退到 agents/<id>/ 目录名；
/// 非 main 目录 ⇒ sub_agent（真实 agent-44 fixture 行为的合成最小化）。
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

/// detect 直测：空文件 Pending；非 JSON / 非 metadata ⇒ UnknownFormat；
/// 1.4 ⇒ KnownVersion；1.5（kimi-code 锚点）⇒ LatestFallback。
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

/// 发现边界：无 home 上下文（测试/隔离运行）只用手工根——不探测本机实测
/// 候选（DefaultHome 候选随 home_dir 上下文启用，同 claude/pi/qwen 惯例；
/// 真实候选路径在本机的存在性由 real_verify_kimi_work 覆盖，不在单测断言）。
#[test]
fn discover_uses_observed_candidate_and_manual_roots() {
    let adapter = KimiWorkAdapter::new();
    let dir = TempDir::new("kimi-work-disc");
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
        manual_roots: vec![home_root], // 含 sessions 子目录 ⇒ 按 home 解析
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
    // 同一根再按 sessions 目录本身传入也可解析（幂等语义）。
    let ctx2 = llm_usage_core::adapters::framework::DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.clone()],
    };
    let roots2 = adapter.discover(&ctx2);
    assert!(roots2.iter().any(|r| r.root == root));
}
