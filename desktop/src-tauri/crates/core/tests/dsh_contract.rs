//! DSH（DeepSeek Harness）适配器合同测试：合成 fixture（文档级证据，A08
//! 固定源码 token-meter；本机未安装，2026-09-25 盘点 not_found）全链路。
//! 折叠规则（pinned README）：final 样本替换同 attempt 流式值；
//! retry-started 结束替换范围并新开一个计费 attempt；attempt/step 边界
//! 对最后一个样本定稿。occurred_at 用观察时间（README 无逐事件时间）。

mod common;

use common::{summary, temp_storage};
use llm_usage_core::adapters::dsh::DshAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

const NOW: i64 = 1_800_000_000_000;

fn dsh_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dsh")
        .join(name)
}

fn run_dsh(storage: &Storage, root: &std::path::Path, now_ms: i64) {
    let adapter = DshAdapter::new();
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.to_path_buf()],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("run-{now_ms}"),
    };
    let reports = run_adapter_scan(storage, &adapter, &ctx, &config).unwrap();
    assert_eq!(reports.len(), 1);
}

#[test]
fn contract_full_chain_matches_manual_expectations() {
    let (_dir, storage) = temp_storage("dsh-contract");
    let root = dsh_fixture("synthetic-contract");
    run_dsh(&storage, &root, NOW);
    // 消息序列：msg1(60) 流式 → msg2(75) final 替换 → retry-started 定稿
    // attempt0=(75,12,20,5) → msg3(30,6,5,5) → 尾部 step/start 定稿 attempt1。
    // 两个计费 attempt：uncached=105、out=18、cr=25、cw=10
    // ⇒ input_total=140、total=158。
    let s = summary(&storage, "2027-01-15", "2027-01-15");
    assert_eq!(s.totals.call_count, 2);
    assert_eq!(s.totals.input_total_known, Some(140));
    assert_eq!(s.totals.cache_read_known, Some(25));
    assert_eq!(s.totals.cache_write_known, Some(10));
    assert_eq!(s.totals.output_total_known, Some(18));
    assert_eq!(s.totals.total_tokens_known, Some(158));
    run_dsh(&storage, &root, NOW + 1_000);
    let s = summary(&storage, "2027-01-15", "2027-01-15");
    assert_eq!(s.totals.call_count, 2, "重复扫描不增量");
}

#[test]
fn replacement_semantics_fold_streaming_and_retry() {
    let (_dir, storage) = temp_storage("dsh-replace");
    let root = dsh_fixture("synthetic-replacement-cases");
    run_dsh(&storage, &root, NOW);
    // step1：流式 120 被 final 70 替换 ⇒ attempt(70,8,25,5)。
    // step2：流式 40/50/60，retry-started 定稿 attempt0=60；retry 的 5 定稿 attempt1。
    // 三个计费 attempt：uncached=70+60+5=135、out=8+4+1=13、cr=25+30+0=55、cw=5。
    // input_total=135+55+5=195、total=208。
    let s = summary(&storage, "2027-01-15", "2027-01-15");
    assert_eq!(s.totals.call_count, 3, "retry 新开计费 attempt");
    assert_eq!(s.totals.input_total_known, Some(195));
    assert_eq!(s.totals.total_tokens_known, Some(208));
}

#[test]
fn non_usage_events_and_undocumented_type_do_not_invent_usage() {
    let (_dir, storage) = temp_storage("dsh-gaps");
    for name in [
        "synthetic-non-usage-events",
        "synthetic-undocumented-event-type",
    ] {
        let root = dsh_fixture(name);
        run_dsh(&storage, &root, NOW);
    }
    let s = summary(&storage, "2027-01-15", "2027-01-15");
    // 无 usage 词汇（context/header/offload/user 消息）不产生请求；pressure/
    // projected 估算不计账；唯一的 usage 消息 (10,2,0,0) ⇒ 1 调用 total 12。
    assert_eq!(s.totals.call_count, 1);
    assert_eq!(s.totals.total_tokens_known, Some(12));
    assert_eq!(s.totals.attempt_count, 0, "估算/未文档化不产生 attempt");
}

#[test]
fn capability_is_doc_level() {
    let cap = DshAdapter::new().capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "dsh");
    assert!(
        serde_json::to_string(&json["maintenance"])
            .unwrap()
            .contains("合成假设"),
        "能力声明如实标注文档级证据与合成假设"
    );
    assert!(!cap.limitations.is_empty());
}
