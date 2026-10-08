//! Legacy DSH (DeepSeek Harness) requirement tests use synthetic data from A08
//! fixed token-meter source; the 2026-09-25 local inventory found no installation.
//! Fixed README replacement rules: finals replace streaming values in the same attempt;
//! retry-started closes replacement and starts another billing attempt. Attempt/step boundaries
//! finalize the last sample. occurred_at uses observation time; this legacy README has no event timestamps.

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
    // msg1 streaming 60 is replaced by msg2 final 75; retry-started finalizes
    // attempt0=(75,12,20,5), then msg3=(30,6,5,5); final step/start closes attempt1.
    // Two billing attempts: uncached=105, output=18, cache read=25, cache write=10;
    // input_total=140, total=158.
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
    // Step one: final 70 replaces streaming 120, yielding attempt(70,8,25,5).
    // Step two: streaming 40/50/60 closes at retry-started as attempt0=60; retry 5 closes attempt1.
    // Three billing attempts: uncached=70+60+5=135, output=8+4+1=13, cache read=25+30+0=55, cache write=5.
    // input_total=135+55+5=195, total=208.
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
    // context/header/offload/user messages do not create calls; pressure/projected estimates
    // do not count as usage. The only usage message (10,2,0,0) produces one call/total 12.
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
