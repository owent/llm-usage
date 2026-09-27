//! Kilo Code CLI 适配器合同测试：真实脱敏 fixture（session-7.4.9-family /
//! session-7.4.8-edges，前一会话实读提取 + _expectations.md 人工核算）经
//! 读取→解析→标准化→commit_batch→查询 全链路。数值全部对照
//! tests/fixtures/kilo/*._expectations.md 的独立核算，不改口径。

mod common;

use common::*;

// 手工核算汇总（对照 _expectations.md）：
// - 7.4.9-family：5 会话（1 主 + 4 子）58 行消息（51 assistant + 7 user）。
//   逐次五字段合计：input=121774 output=8876 reasoning=45302
//   cache_read=2137472 cache_write=0；derived_total=2313424。
//   canonical 口径：input_total = input+cr+cw = 2259246；
//   output_total = output+reasoning = 54178；total_tokens = 2313424。
//   逐会话对账 detail_sum：1984963/39997/167085/61589/59790，全部 matched；
//   missing_total_msgs=1（未完成消息，无 tokens.total）。
// - 7.4.8-edges：1 会话 16 行消息（13 assistant + 3 user）；模型 k2p7×2 +
//   glm-5.2×11；两条 error 消息 tokens 全零且缺 total（missing_total_msgs=2）；
//   五字段合计 input=33638 output=1535 reasoning=4403 cache_read=272896 cw=0
//   derived_total=312472；canonical input_total=306534 output_total=5938。

const NOW: i64 = 1_800_000_000_000;

#[test]
fn archived_sessions_before_and_after_collection_keep_the_same_usage() {
    let dir = TempDir::new("kilo-archived");
    let root = build_kilo_db_from_fixture(&dir, "session-7.4.9-family.sanitized.json");
    let path = root.join(".local/share/kilo/kilo.db");
    let source = rusqlite::Connection::open(path).unwrap();
    source
        .execute("UPDATE session SET time_archived=?1", [NOW])
        .unwrap();
    let (_db, storage) = temp_storage("kilo-archived");
    run_kilo(&storage, &root, NOW);
    let before = summary(&storage, "2026-01-01", "2026-12-31").totals;
    assert_eq!(before.call_count, 51);
    assert_eq!(before.total_tokens_known, Some(2_313_424));
    source
        .execute("UPDATE session SET time_archived=NULL", [])
        .unwrap();
    run_kilo(&storage, &root, NOW + 1);
    source
        .execute("UPDATE session SET time_archived=?1", [NOW + 2])
        .unwrap();
    run_kilo(&storage, &root, NOW + 2);
    assert_eq!(summary(&storage, "2026-01-01", "2026-12-31").totals, before);
}

#[test]
fn contract_family_fixture_full_pipeline_matches_expectations() {
    let dir = TempDir::new("kilo-family");
    let root = build_kilo_db_from_fixture(&dir, "session-7.4.9-family.sanitized.json");
    let (_db, storage) = temp_storage("kilo-family");
    let reports = run_kilo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files.len(), 1, "一个 kilo.db 一个文件目标");
    assert_eq!(report.files[0].status, "complete");
    assert_eq!(report.files[0].records_seen, 58, "全部消息行（含 user）");
    assert_eq!(report.files[0].events, 51, "只有 assistant 记 model_call");
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!((outcome.added, outcome.updated, outcome.errors), (51, 0, 0));

    // 白名单数值核对（UTC 汇总，2026-07/08 期间）。
    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 51);
    assert_eq!(summary.totals.input_total_known, Some(2_259_246));
    assert_eq!(summary.totals.output_total_known, Some(54_178));
    assert_eq!(summary.totals.cache_read_known, Some(2_137_472));
    assert_eq!(summary.totals.cache_write_known, Some(0));
    assert_eq!(summary.totals.total_tokens_known, Some(2_313_424));

    let conn = storage.conn();
    // 分类：主会话 37 primary + 子会话（parent_id 非空）14 sub_agent。
    let (primary, sub): (i64, i64) = conn
        .query_row(
            "SELECT SUM(call_category = 'primary'), SUM(call_category = 'sub_agent') \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((primary, sub), (37, 14));

    // 版本分派：数值最大 session.version=7.4.9 已收录 ⇒ known_version。
    let bases: std::collections::BTreeSet<String> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT parse_basis FROM usage_events")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        bases,
        std::iter::once("known_version".to_string()).collect(),
        "7.4.9 注册在 VERIFIED_VERSION_IMPLS"
    );

    // 事件键与白名单字段（kilo:msg:{message.id} 稳定身份）。
    type RowRow = (
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
    );
    let row: RowRow = conn
        .query_row(
            "SELECT input_total, input_cache_read, input_uncached, output_total, \
             output_reasoning, total_tokens, provider_id, model_raw, session_id, \
             model_attribution, time_basis \
             FROM usage_events WHERE source_record_key = 'kilo:msg:anon-8'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                ))
            },
        )
        .unwrap();
    // fixture anon-8：input=20040 output=51 reasoning=477 cache 全 0 total=20568。
    assert_eq!(row.0, Some(20_040), "input_total = input + cr + cw");
    assert_eq!(row.1, Some(0));
    assert_eq!(row.2, Some(20_040), "input_uncached = 互斥口径的 input 桶");
    assert_eq!(row.3, Some(528), "output_total = output + reasoning");
    assert_eq!(row.4, Some(477));
    assert_eq!(row.5, Some(20_568));
    assert_eq!(row.6.as_deref(), Some("zhipuai-coding-plan"));
    assert_eq!(row.7.as_deref(), Some("glm-5.2"));
    assert_eq!(row.8, "anon-1");
    assert_eq!(row.9, "request_field");
    assert_eq!(row.10, "source_completion");

    // 子会话消息的 parent_session_id 与 sub_agent 分类
    //（anon-47 是子会话 anon-3 的 assistant 消息）。
    let (category, parent): (String, Option<String>) = conn
        .query_row(
            "SELECT call_category, parent_session_id FROM usage_events \
             WHERE source_record_key = 'kilo:msg:anon-47'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(category, "sub_agent");
    assert_eq!(parent.as_deref(), Some("anon-1"));

    // 缺 total 的未完成消息：source_total NULL、partial 生命周期、source_start 时间。
    let (source_total, lifecycle, time_basis): (Option<i64>, String, String) = conn
        .query_row(
            "SELECT source_total, lifecycle, time_basis FROM usage_events \
             WHERE source_record_key = 'kilo:msg:anon-13'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(source_total, None, "missing_total_msgs=1：缺 total 不补零");
    assert_eq!(lifecycle, "partial", "无 finish/error 的未完成消息");
    assert_eq!(time_basis, "source_start", "无 completed 退 created");
    let partial_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE lifecycle = 'partial'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(partial_count, 1);

    // 会话累计快照对账：5 个会话全部 matched，明细合计逐会话对照期望。
    let mut details: Vec<i64> = report
        .reconciliations
        .iter()
        .map(|r| r.detail_sum)
        .collect();
    details.sort_unstable();
    assert_eq!(
        details,
        vec![39_997, 59_790, 61_589, 167_085, 1_984_963],
        "逐会话 detail_sum 与 _expectations.md 一致"
    );
    assert!(report
        .reconciliations
        .iter()
        .all(|r| r.verdict == "matched" && r.carried_sum == 0));

    // 事件时间取 data.time.completed（毫秒 epoch）。
    let occurred: i64 = conn
        .query_row(
            "SELECT occurred_at_ms FROM usage_events \
             WHERE source_record_key = 'kilo:msg:anon-8'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(occurred, 1_784_190_965_520);

    // 无诊断：真实 fixture 全部行成功解析映射。
    let diags: i64 = conn
        .query_row("SELECT COUNT(*) FROM diagnostics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(diags, 0);
    assert_eq!(report.files[0].diagnostics, 0);
    let _ = dir;
}

#[test]
fn contract_edges_fixture_error_messages_and_model_switch() {
    let dir = TempDir::new("kilo-edges");
    let root = build_kilo_db_from_fixture(&dir, "session-7.4.8-edges.sanitized.json");
    let (_db, storage) = temp_storage("kilo-edges");
    let reports = run_kilo(&storage, &root, NOW);

    let report = &reports[0];
    assert_eq!(report.files[0].records_seen, 16);
    assert_eq!(report.files[0].events, 13);
    let outcome = report.outcome.as_ref().unwrap();
    assert_eq!(outcome.added, 13);

    let summary = summary(&storage, "2026-01-01", "2026-12-31");
    assert_eq!(summary.totals.call_count, 13);
    assert_eq!(summary.totals.input_total_known, Some(306_534));
    assert_eq!(summary.totals.output_total_known, Some(5_938));
    assert_eq!(summary.totals.cache_read_known, Some(272_896));
    assert_eq!(summary.totals.total_tokens_known, Some(312_472));

    let conn = storage.conn();
    // 双模型切换按消息归属（k2p7×2 + glm-5.2×11）。
    let models: std::collections::BTreeMap<String, i64> = {
        let mut stmt = conn
            .prepare("SELECT model_raw, COUNT(*) FROM usage_events GROUP BY model_raw")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(
        models,
        std::collections::BTreeMap::from([("glm-5.2".to_string(), 11), ("k2p7".to_string(), 2),])
    );

    // error 消息：error_status=error、tokens 全零照常记账、缺 total（2 条）。
    let (errors, missing_total): (i64, i64) = conn
        .query_row(
            "SELECT SUM(error_status = 'error'), SUM(source_total IS NULL) \
             FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(errors, 2, "两条 error 消息（fixture anon-4/anon-6）");
    assert_eq!(missing_total, 2, "missing_total_msgs=2");
    let zero_tokens: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE total_tokens = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(zero_tokens, 2, "error 消息 tokens 全零是已知量");

    // 版本 7.4.8 已收录；单会话对账 matched。
    assert_eq!(report.reconciliations.len(), 1);
    assert_eq!(report.reconciliations[0].detail_sum, 312_472);
    assert_eq!(report.reconciliations[0].verdict, "matched");
    let _ = dir;
}

#[test]
fn capability_table_is_structured_and_complete() {
    use llm_usage_core::adapters::framework::SourceAdapter;
    let adapter = llm_usage_core::adapters::kilo::KiloAdapter::new();
    let cap = adapter.capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "kilo");
    assert_eq!(
        json["supported_versions"],
        serde_json::json!(["7.4.8", "7.4.9"])
    );
    assert_eq!(json["discovery"]["env_override"], serde_json::Value::Null);
    for key in [
        "tokens",
        "cache_read",
        "cache_write",
        "per_request_calls",
        "model",
        "time",
        "cost",
        "latency",
    ] {
        assert!(
            json["fields"].get(key).is_some(),
            "capability fields missing {key}"
        );
    }
    assert_eq!(json["fields"]["tokens"]["availability"], "available");
    assert!(
        json["fields"]["cost"]["availability"]
            .get("unavailable")
            .is_some(),
        "cost 列存模型 JSON，不可用"
    );
    for section in [
        "discovery",
        "detection",
        "lifecycle",
        "incremental",
        "dedup",
        "integrity",
        "maintenance",
        "scheduling",
    ] {
        assert!(json.get(section).is_some(), "capability missing {section}");
    }
    assert!(!cap.limitations.is_empty());
}

#[test]
fn discover_finds_kilo_db_from_default_home_shape() {
    use llm_usage_core::adapters::framework::{DiscoverContext, SourceAdapter};
    let dir = TempDir::new("kilo-discover");
    build_kilo_db_from_fixture(&dir, "session-7.4.8-edges.sanitized.json");
    let adapter = llm_usage_core::adapters::kilo::KiloAdapter::new();

    // 默认根：home_dir 推导 <home>/.local/share/kilo（Unix 布局，Windows 同形）。
    let ctx = DiscoverContext {
        home_dir: Some(dir.path().to_path_buf()),
        env: Default::default(),
        manual_roots: vec![],
    };
    let roots = adapter.discover(&ctx);
    assert_eq!(roots.len(), 1);
    let instance_id = adapter.instance_id(&roots[0]);
    assert!(
        instance_id.starts_with("kilo@") && instance_id.contains(".local/share/kilo"),
        "实例身份来自发现的 kilo home：{instance_id}"
    );

    // 手工根三形态：kilo home 本身 / 其父目录 / 用户 home。
    for manual in [
        dir.path().join(".local").join("share").join("kilo"),
        dir.path().join(".local").join("share"),
        dir.path().to_path_buf(),
    ] {
        let ctx = DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![manual.clone()],
        };
        let roots = adapter.discover(&ctx);
        assert_eq!(roots.len(), 1, "手工根 {manual:?} 应恰好发现一个 kilo.db");
        assert!(roots[0].files[0]
            .file_name()
            .is_some_and(|n| n == "kilo.db"));
        assert_eq!(
            roots[0].files[0].parent(),
            Some(
                dir.path()
                    .join(".local")
                    .join("share")
                    .join("kilo")
                    .as_path()
            )
        );
    }

    // 空目录不产出根。
    let ctx = DiscoverContext {
        home_dir: Some(std::env::temp_dir()),
        env: Default::default(),
        manual_roots: vec![],
    };
    assert!(adapter.discover(&ctx).is_empty());
    let _ = dir;
}
