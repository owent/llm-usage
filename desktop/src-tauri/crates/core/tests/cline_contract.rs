//! Cline 适配器合同测试：合成 fixture（文档级证据，A03 固定源码；本机未安装，
//! 2026-09-25 盘点 not_found）经读取→解析→commit→查询全链路。
//! 数值为 fixture 人工核算（见各目录数据），口径：四桶互斥
//! （tokensIn/tokensOut/cacheWrites/cacheReads），input_total = in+cw+cr 派生，
//! total = 四桶之和（common.rs 文件头）。

mod common;

use common::{summary, temp_storage};
use llm_usage_core::adapters::cline::ClineAdapter;
use llm_usage_core::adapters::framework::{
    normalize_path, run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use std::path::PathBuf;

const NOW: i64 = 1_800_000_000_000;

fn cline_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("cline")
        .join(name)
}

fn run_cline(storage: &Storage, root: &std::path::Path, now_ms: i64) -> usize {
    let adapter = ClineAdapter::new();
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
    reports.len()
}

#[test]
fn contract_full_chain_matches_manual_expectations() {
    let (_dir, storage) = temp_storage("cline-contract");
    let root = cline_fixture("synthetic-contract");
    run_cline(&storage, &root, NOW);
    // 3 次 api_req_started：(100,20,10,40)、(200,30,50,0)、(50,5,0,25)。
    // input_total = in+cw+cr ⇒ 150+250+75 = 475；total = 四桶和 ⇒ 170+280+80 = 530。
    let s = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(s.totals.call_count, 3);
    assert_eq!(s.totals.input_total_known, Some(475));
    assert_eq!(s.totals.cache_read_known, Some(65));
    assert_eq!(s.totals.cache_write_known, Some(60));
    assert_eq!(s.totals.output_total_known, Some(55));
    assert_eq!(s.totals.total_tokens_known, Some(530));
    // 幂等。
    run_cline(&storage, &root, NOW + 1_000);
    let s = summary(&storage, "2026-09-24", "2026-09-24");
    assert_eq!(s.totals.call_count, 3, "重复扫描不增量");
}

#[test]
fn capability_is_doc_level_with_pending_note() {
    let cap = ClineAdapter::new().capability();
    let json = serde_json::to_value(&cap).unwrap();
    assert_eq!(json["adapter_id"], "cline");
    assert!(
        json["maintenance"]["format_evidence"]
            .as_str()
            .map(|s| s.contains("文档级证据"))
            .unwrap_or(false),
        "能力声明标注文档级证据"
    );
    assert!(!cap.limitations.is_empty());
}

#[test]
fn undocumented_say_and_started_without_finished_visible() {
    // started-without-finished：部分可用（有 usage 的 started 记账，
    // 无对应完成不算失败）；undocumented-say：未文档化 say 不计请求。
    let (_dir, storage) = temp_storage("cline-gaps");
    for name in [
        "synthetic-started-without-finished",
        "synthetic-undocumented-say",
        "synthetic-non-say-type",
    ] {
        let root = cline_fixture(name);
        run_cline(&storage, &root, NOW);
    }
    let s = summary(&storage, "2020-01-01", "2100-01-01");
    assert!(s.totals.call_count >= 1, "可校验部分保留");
}
