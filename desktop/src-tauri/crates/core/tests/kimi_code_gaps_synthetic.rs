//! Kimi Code（A12，M4）缺口场景：合成样本（目录/文件头均标 synthetic）。
//! 覆盖：无 usage 事件（error 步）、未知 protocol_version fallback（含 1.4——
//! Kimi Work 的锚点在本产品注册表未收录）、回声/completed 双计防御、
//! 负值/未知 scope/秒级时间（不猜测换算）、detect 直测。
//! 期望值均为人工核算（见各 fixture 目录 _expectations.md）。

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

/// 无 usage.record 的 wire（error 步 + retry）是正常形状：
/// 0 事件、0 诊断、complete（不补零、不报错）。
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
    // 汇总无该源任何数值行（不伪造零值）。
    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 0);
}

/// 未收录 protocol_version（"9.9"）：latest_fallback 兼容尝试——
/// 数据照常入账并带 unverified 标记，不因版本号未收录直接拒绝（V30）。
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

/// 回声/completed 双计防御：step.end 回声逐字段等于 usage.record、
/// subagent.completed.usage 是子代理 wire Σ 快照——二者都不产生事件。
#[test]
fn echo_and_subagent_completed_never_double_count() {
    let (_db, storage) = temp_storage("kimi-code-echo");
    let root = kimi_code_fixture("synthetic-echo-defense");
    let reports = run_kimi_code(&storage, &root, NOW);
    assert_eq!(reports[0].files[0].events, 2, "1 primary + 1 auxiliary");

    let summary = summary(&storage, "2026-01-01", "2026-01-01");
    assert_eq!(summary.totals.call_count, 2);
    // 若回声/completed 双计，四项都会翻倍以上。
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

    // completed 的 {123,45,678,0} 不得出现在任何事件。
    let leaked: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE input_uncached = 123 AND output_total = 45",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0);

    // 回声对账：记录侧 Σ == 回声侧 Σ（turn 1 条 = 回声 1 条）⇒ matched。
    assert!(reports[0]
        .reconciliations
        .iter()
        .any(|r| r.series == "kimi_wire_step_end_echo" && r.verdict == "matched"));
    assert!(reports[0]
        .reconciliations
        .iter()
        .any(|r| r.series.starts_with("kimi_subagent_completed_snapshot")));
}

/// 坏形状隔离：负值 / 未知 usageScope / 秒级时间各记诊断跳过，
/// 正常记录照常入账；秒值不做 ×1000 猜测（实读 82 文件全部毫秒）。
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
    // 唯一入账记录 {200,60,800,10}：缓存写 10 是 reported（合成缺口场景，
    // 真实本机样本全 0——字段映射经此验证）。
    assert_eq!((input_uncached, cache_write), (200, 10));
    // 秒级 time（1_767_225_600）未被换算入账。
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

/// 同毫秒两条 usage.record：身份加序号区分，不碰撞（防御性；本机未观测）。
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
    // 幂等：重扫不因序号身份产生新事件。
    let reports2 = run_kimi_code(&storage, &root, NOW + 1000);
    let added2: i64 = reports2
        .iter()
        .filter_map(|r| r.outcome.as_ref().map(|o| o.added))
        .sum();
    assert_eq!(added2, 0);
}

/// detect 直测：空文件 Pending；首行非 JSON / 非 metadata 头 ⇒ UnknownFormat。
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

/// 已验证锚点 1.5 ⇒ KnownVersion；1.4（kimi-work 锚点）在本注册表 ⇒ fallback。
#[test]
fn detect_registry_dispatches_by_own_anchor() {
    let adapter = KimiCodeAdapter::new();
    let dir = TempDir::new("kimi-code-dispatch");
    for (version, expected_basis) in [("1.5", "known"), ("1.4", "fallback")] {
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
