//! V29：价格快照、费用估算引擎与汇总的合同测试。
//!
//! 价格样本取自 [价格合同](../../../docs/design/desktop-usage/pricing.md) V29 固定样本
//! （P1–P6，单位换算为"最小货币单位百分之一/百万 token"，即表值 ×100）；
//! 期望金额为人工核算值。E2 说明：原样本 token 量（输入合计 1.3M）按上下文档
//! 阈值语义应命中 P3 长档而非 P2（与 E4/E5 语义一致），测试同时覆盖
//! P2 全维度（token 量缩至阈值内）与原量值命中 P3 两种情形。

mod common;

use common::{batch, evt, temp_storage, ts};
use llm_usage_core::domain::{CostAmount, CostKind, FieldQuality};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::pricing::{parse_snapshot_json, EstimateOptions};
use llm_usage_core::storage::pricing::{CostFilters, CostSummaryRequest};
use llm_usage_core::storage::Storage;

/// P1–P6 固定价格样本 + A4 batch 档对照行（不参与 standard 匹配）。
const SNAPSHOT: &str = r#"{
  "format": "llm-usage-price-snapshot/1",
  "snapshot": {"id": "v29-sample", "source_type": "manual", "source_urls": [],
    "fetched_at": "2026-09-25"},
  "rows": [
    {"price_id": "P1", "provider_id": "zhipuai", "model": "glm-5.3", "region": "cn",
     "channel": "bigmodel", "effective_from": "2026-09-25", "currency": "CNY",
     "input": 80000, "cache_read": 20000, "cache_write_5m": null, "cache_write_1h": null,
     "output": 280000},
    {"price_id": "P2", "provider_id": "openai", "model": "gpt-6-astra", "region": "global",
     "channel": "api", "context_threshold_tokens": 0, "effective_from": "2026-09-25",
     "currency": "USD", "input": 100000, "cache_read": 10000, "cache_write_5m": 125000,
     "cache_write_1h": null, "output": 500000},
    {"price_id": "P3", "provider_id": "openai", "model": "gpt-6-astra", "region": "global",
     "channel": "api", "context_threshold_tokens": 272000, "effective_from": "2026-09-25",
     "currency": "USD", "input": 200000, "cache_read": 20000, "cache_write_5m": 250000,
     "cache_write_1h": null, "output": 750000},
    {"price_id": "P4", "provider_id": "moonshot", "model": "kimi-k3", "region": "global",
     "channel": "api", "effective_from": "2026-09-25", "currency": "USD",
     "input": 30000, "cache_read": 3000, "cache_write_5m": 30000, "cache_write_1h": 60000,
     "output": 150000},
    {"price_id": "P4-batch", "provider_id": "moonshot", "model": "kimi-k3", "region": "global",
     "channel": "api", "service_tier": "batch", "effective_from": "2026-09-25",
     "currency": "USD", "input": 18000, "cache_read": 1800, "cache_write_5m": 18000,
     "cache_write_1h": 36000, "output": 90000},
    {"price_id": "P5", "provider_id": "zhipuai", "model": "glm-5.1", "region": "cn",
     "channel": "bigmodel", "context_threshold_tokens": 0, "effective_from": "2026-09-25",
     "currency": "CNY", "input": 60000, "cache_read": null, "output": 240000},
    {"price_id": "P6", "provider_id": "zhipuai", "model": "glm-5.1", "region": "cn",
     "channel": "bigmodel", "context_threshold_tokens": 32768, "effective_from": "2026-09-25",
     "currency": "CNY", "input": 80000, "cache_read": null, "output": 280000}
  ]
}"#;

fn options() -> EstimateOptions {
    let mut options = EstimateOptions::default();
    options
        .provider_channels
        .insert("zhipuai".into(), ("cn".into(), "bigmodel".into()));
    options
        .provider_channels
        .insert("openai".into(), ("global".into(), "api".into()));
    options
        .provider_channels
        .insert("moonshot".into(), ("global".into(), "api".into()));
    // A6/E3：moonshot 缓存写默认 1h 档；E2'：openai 默认 5m 档（P2 有 5m 写价）。
    options.cache_ttl_minutes.insert("moonshot".into(), 60);
    options.cache_ttl_minutes.insert("openai".into(), 5);
    options
}

/// 构造带完整四分量 token 的事件（Some 字段标 reported；UTC 2026-09-26 12:00 发生）。
fn priced_evt(
    key: &str,
    provider: &str,
    model: &str,
    uncached: Option<i64>,
    read: Option<i64>,
    write: Option<i64>,
    output: Option<i64>,
) -> llm_usage_core::domain::EventInput {
    let mut e = evt("inst", key, ts("2026-09-26T12:00:00Z"));
    e.provider_id = Some(provider.to_string());
    e.model_raw = Some(model.to_string());
    let total = match (uncached, read, write) {
        (Some(u), Some(r), Some(w)) => Some(u + r + w),
        _ => None,
    };
    e.usage.input_uncached = uncached;
    e.usage.input_cache_read = read;
    e.usage.input_cache_write = write;
    e.usage.input_total = total;
    e.usage.output_total = output;
    e.usage.total_tokens = match (total, output) {
        (Some(t), Some(o)) => Some(t + o),
        _ => None,
    };
    // quality 与取值一致（None ⇒ Unknown），否则 ingest 校验拒绝。
    e.quality.input_uncached = opt_quality(uncached);
    e.quality.input_cache_read = opt_quality(read);
    e.quality.input_cache_write = opt_quality(write);
    e.quality.input_total = opt_quality(total);
    e.quality.output_total = opt_quality(output);
    e.quality.total_tokens = opt_quality(e.usage.total_tokens);
    e
}

fn opt_quality(value: Option<i64>) -> FieldQuality {
    match value {
        Some(_) => FieldQuality::Reported,
        None => FieldQuality::Unknown,
    }
}

fn currency_row<'a>(
    summary: &'a llm_usage_core::storage::pricing::CostSummary,
    mode: fn(
        &llm_usage_core::storage::pricing::CostSummary,
    ) -> &llm_usage_core::storage::pricing::CostModeSummary,
    currency: &str,
) -> &'a llm_usage_core::storage::pricing::CostCurrencyRow {
    mode(summary)
        .rows
        .iter()
        .find(|r| r.currency == currency)
        .unwrap_or_else(|| panic!("missing {currency} row"))
}

fn at_time(
    s: &llm_usage_core::storage::pricing::CostSummary,
) -> &llm_usage_core::storage::pricing::CostModeSummary {
    &s.at_time
}

fn current_sim(
    s: &llm_usage_core::storage::pricing::CostSummary,
) -> &llm_usage_core::storage::pricing::CostModeSummary {
    &s.current_sim
}

fn run_summary(storage: &Storage, now_ms: i64) -> llm_usage_core::storage::pricing::CostSummary {
    storage
        .cost_summary(&CostSummaryRequest {
            timezone: "UTC".to_string(),
            first_day: "2026-09-26".to_string(),
            last_day: "2026-09-26".to_string(),
            filters: CostFilters::default(),
            now_ms,
            options: options(),
        })
        .unwrap()
}

/// E1/E3/E4–E7/E8 + A1–A8 全链路（导入→入库→回填→汇总）。
#[test]
fn v29_contract_amounts_and_anomalies() {
    let (_dir, storage) = temp_storage("v29");
    let snapshot = parse_snapshot_json(SNAPSHOT).unwrap();
    let imported = storage
        .import_price_snapshot(&snapshot, ts("2026-09-25T00:00:00Z"))
        .unwrap();
    assert_eq!(imported.inserted_rows, 7);
    // A10：同快照重复导入幂等跳过。
    let again = storage
        .import_price_snapshot(&snapshot, ts("2026-09-26T00:00:00Z"))
        .unwrap();
    assert!(again.already_present);
    assert_eq!(again.inserted_rows, 0);

    let now = ts("2026-09-27T12:00:00Z");
    let events = vec![
        // E1 混合输入（P1，CNY）：988+100+968 = 2056 分；写分量未计价。
        priced_evt(
            "e1",
            "zhipuai",
            "glm-5.3",
            Some(1_234_567),
            Some(500_000),
            Some(200_000),
            Some(345_678),
        ),
        // E2' 全维度 P2（输入合计 130K < 272K）：100+2+13+125 = 240 美分。
        priced_evt(
            "e2",
            "openai",
            "gpt-6-astra",
            Some(100_000),
            Some(20_000),
            Some(10_000),
            Some(25_000),
        ),
        // E3 TTL 1h（P4，moonshot 默认 60 分钟）：120+9+360+225 = 714 美分。
        priced_evt(
            "e3",
            "moonshot",
            "kimi-k3",
            Some(400_000),
            Some(300_000),
            Some(600_000),
            Some(150_000),
        ),
        // E4 阶梯上界（输入合计 272,000 → P3）：544+60 = 604 美分。
        priced_evt(
            "e4",
            "openai",
            "gpt-6-astra",
            Some(272_000),
            Some(0),
            Some(0),
            Some(8_000),
        ),
        // E5 阶梯下界（271,999 → P2）：272+40 = 312 美分。
        priced_evt(
            "e5",
            "openai",
            "gpt-6-astra",
            Some(271_999),
            Some(0),
            Some(0),
            Some(8_000),
        ),
        // E6 GLM 阶梯（32,768 → P6）：26+3 = 29 分。
        priced_evt(
            "e6",
            "zhipuai",
            "glm-5.1",
            Some(32_768),
            Some(0),
            Some(0),
            Some(1_000),
        ),
        // E7 GLM 阶梯下界（32,767 → P5）：20+2 = 22 分。
        priced_evt(
            "e7",
            "zhipuai",
            "glm-5.1",
            Some(32_767),
            Some(0),
            Some(0),
            Some(1_000),
        ),
        // A1 无按量价模型：未计价。
        priced_evt(
            "a1",
            "moonshot",
            "kimi-for-coding",
            Some(1_000),
            Some(1_000),
            Some(0),
            Some(1_000),
        ),
        // A2 部分可计价：输出未知、输入已知（P1）。
        priced_evt(
            "a2",
            "zhipuai",
            "glm-5.3",
            Some(1_000_000),
            Some(0),
            Some(0),
            None,
        ),
        // A3 推理不重复计价：reasoning ⊂ output（P2 输入 130K 档）。
        {
            let mut e = priced_evt(
                "a3",
                "openai",
                "gpt-6-astra",
                Some(100_000),
                Some(0),
                Some(0),
                Some(200_000),
            );
            e.usage.output_reasoning = Some(50_000);
            e.quality.output_reasoning = FieldQuality::Reported;
            e
        },
        // A5 缓存读价缺失（P5/P6 read NULL）：读分量未计价，其余照计。
        priced_evt(
            "a5",
            "zhipuai",
            "glm-5.1",
            Some(32_767),
            Some(100_000),
            Some(0),
            Some(1_000),
        ),
        // A6 TTL 未知：zhipuai 未设默认档且 P1 无写价 → 写分量未计价（E1 已覆盖）。
        // A7 历史复现：事件早于快照生效起点 → at_time 未计价，current_sim 可计。
        priced_evt(
            "a7",
            "zhipuai",
            "glm-5.3",
            Some(1_000_000),
            Some(0),
            Some(0),
            Some(1_000),
        ),
        // A8 异常 token：缓存读写合计大于已知总输入 → 拒绝计价。
        {
            let mut e = priced_evt(
                "a8",
                "zhipuai",
                "glm-5.3",
                Some(10_000),
                Some(50_000),
                Some(50_000),
                Some(1_000),
            );
            // 源端矛盾：总输入 60,000 < read+write 100,000。
            e.usage.input_total = Some(60_000);
            e
        },
        // 来源记录金额（crush 形态）：reported 1000 美分 + 来源估算 500 美分。
        {
            let mut e = priced_evt(
                "src1",
                "openai",
                "gpt-6-astra",
                Some(1_000),
                Some(0),
                Some(0),
                Some(1_000),
            );
            e.cost = Some(CostAmount {
                amount_minor: 1000,
                currency: "USD".to_string(),
                kind: CostKind::Reported,
                price_version: Some("crush-v1".to_string()),
                billing_scope: None,
            });
            e
        },
        {
            let mut e = priced_evt(
                "src2",
                "zhipuai",
                "glm-5.3",
                Some(1_000),
                Some(0),
                Some(0),
                Some(1_000),
            );
            e.cost = Some(CostAmount {
                amount_minor: 500,
                currency: "USD".to_string(),
                kind: CostKind::Estimated,
                price_version: Some("crush-v1".to_string()),
                billing_scope: None,
            });
            e
        },
    ];
    // A7 事件改到快照生效前（2026-09-20）与独立来源实例。
    let mut events: Vec<_> = events;
    if let Some(a7) = events.iter_mut().find(|e| e.source_record_key == "a7") {
        a7.occurred_at_ms = ts("2026-09-20T12:00:00Z");
        a7.source_instance_id = "inst2".to_string();
    }
    // 提交两个批次（不同日，验证回填按日执行）。
    let (a7, rest): (Vec<_>, Vec<_>) = events
        .into_iter()
        .partition(|e| e.source_record_key == "a7");
    let now_batch = ts("2026-09-26T13:00:00Z");
    commit_batch(&storage, &batch("inst", "UTC", now_batch, rest), None).unwrap();
    commit_batch(
        &storage,
        &batch("inst2", "UTC", ts("2026-09-20T13:00:00Z"), a7),
        None,
    )
    .unwrap();

    // 回填两个日（09-20 与 09-26）。
    let outcomes = storage
        .recompute_unsealed_cost_days("UTC", now, &options())
        .unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.rebuilt));

    let summary = run_summary(&storage, now);

    // E1（P1/CNY）：2056 分；E6/E7/E5 档位、A2 部分计价、A5 与 src2 见下。
    let cny = currency_row(&summary, at_time, "CNY");
    // E1 2056 + E6 29 + E7 22 + A2 800 + A5 29 + src2 估算 4 = 2940 分。
    // A5：read 100,000 使输入合计 132,767 ≥ 32,768 → P6 档（读价 NULL 未计价）。
    assert_eq!(cny.total_amount_minor, 2056 + 29 + 22 + 800 + 29 + 4);
    assert_eq!(cny.input_amount_minor, Some(988 + 26 + 20 + 800 + 26 + 1));
    // E1 的 200,000 写 token 与 A5 的 100,000 读 token 未计价 → known > priced。
    assert!(cny.known_tokens > cny.priced_tokens);
    // 部分：E1（写未计价）、A2（输出未知）、A5（读未计价）。
    assert_eq!(cny.partial_event_count, 3);

    // E2'/E4/E5/E3 + A3 + src1 估算（USD）。
    let usd = currency_row(&summary, at_time, "USD");
    // 240 + 604 + 312 + 714 + 1100 + 6 = 2976 美分。
    assert_eq!(usd.total_amount_minor, 240 + 604 + 312 + 714 + 1100 + 6);

    // E8：多币种分列，无汇率不合并。
    assert_eq!(
        summary
            .at_time
            .rows
            .iter()
            .filter(|r| !r.currency.is_empty())
            .count(),
        2
    );

    // A1/A8 未计价计数与原因。
    let unpriced = currency_row(&summary, at_time, "");
    assert_eq!(unpriced.unpriced_event_count, 2);
    let reasons = &summary.at_time.unpriced_reasons;
    assert_eq!(reasons.get("no_price_row"), Some(&1)); // A1
    assert_eq!(reasons.get("token_anomaly"), Some(&1)); // A8

    // 来源金额分列（reported 与 source_estimate 同币种合并展示）。
    let src = summary
        .source_amounts
        .iter()
        .find(|r| r.currency == "USD")
        .expect("source USD row");
    assert_eq!(src.total_amount_minor, 1500);

    // A7：at_time 汇总（09-26 范围）不含 09-20 事件；current_sim 同理（范围外）。
    // A4：batch 档行不串用——E3 金额按 standard P4 计（已由 714 断言覆盖）。
    // A3：reasoning 不重复计价（A3 事件金额 = 100 输入 + 1000 输出，无推理加成）。

    // 按当前价格模拟：09-26 范围内事件存在 → 非 detail_limited；
    // 金额始终按当前行计算（本快照区间覆盖 now，数值与 at_time 一致）。
    assert!(!summary.current_sim.detail_limited);
    let sim_usd = currency_row(&summary, current_sim, "USD");
    assert_eq!(sim_usd.total_amount_minor, usd.total_amount_minor);
    assert_eq!(sim_usd.unpriced_event_count, 0);

    // A7 单独范围（09-20）：at_time 有该日行（未计价原因 no_price_row），
    // current_sim 有当前价模拟金额（区间有效）。
    let a7_summary = storage
        .cost_summary(&CostSummaryRequest {
            timezone: "UTC".to_string(),
            first_day: "2026-09-20".to_string(),
            last_day: "2026-09-20".to_string(),
            filters: CostFilters::default(),
            now_ms: now,
            options: options(),
        })
        .unwrap();
    let a7_cny = currency_row(&a7_summary, current_sim, "CNY");
    // 1M×80000/1e8 = 800 输入 + 1000×280000/1e8 = 3 输出。
    assert_eq!(a7_cny.total_amount_minor, 803);
    let a7_at = currency_row(&a7_summary, at_time, "");
    assert_eq!(a7_at.unpriced_event_count, 1);
    assert_eq!(
        a7_summary.at_time.unpriced_reasons.get("no_price_row"),
        Some(&1)
    );

    // 估算引用：价格基础含样本快照，数据修订 > 0。
    assert!(summary.price_basis.contains(&"v29-sample".to_string()));
    assert!(summary.data_revision > 0);

    // 幂等：重复回填不增长。
    storage
        .recompute_unsealed_cost_days("UTC", now, &options())
        .unwrap();
    let rerun = run_summary(&storage, now);
    assert_eq!(
        currency_row(&rerun, at_time, "CNY").total_amount_minor,
        cny.total_amount_minor
    );
    assert_eq!(
        currency_row(&rerun, at_time, "USD").total_amount_minor,
        usd.total_amount_minor
    );
}

/// 渠道不明不套价（未配置供应商渠道 → 全部未计价，不写 0）。
#[test]
fn v29_unconfigured_channel_leaves_events_unpriced() {
    let (_dir, storage) = temp_storage("v29chan");
    let snapshot = parse_snapshot_json(SNAPSHOT).unwrap();
    storage
        .import_price_snapshot(&snapshot, ts("2026-09-25T00:00:00Z"))
        .unwrap();
    let now = ts("2026-09-27T12:00:00Z");
    commit_batch(
        &storage,
        &batch(
            "inst",
            "UTC",
            now,
            vec![priced_evt(
                "e1",
                "zhipuai",
                "glm-5.3",
                Some(1_000_000),
                Some(0),
                Some(0),
                Some(1_000),
            )],
        ),
        None,
    )
    .unwrap();
    storage
        .recompute_unsealed_cost_days("UTC", now, &EstimateOptions::default())
        .unwrap();
    let summary = run_summary_channel_none(&storage, now);
    assert!(summary
        .at_time
        .rows
        .iter()
        .all(|r| r.currency.is_empty() || r.total_amount_minor == 0));
    assert_eq!(
        summary
            .at_time
            .rows
            .iter()
            .map(|r| r.unpriced_event_count)
            .sum::<i64>(),
        1
    );
    assert_eq!(
        summary.at_time.unpriced_reasons.get("channel_unknown"),
        Some(&1)
    );
}

fn run_summary_channel_none(
    storage: &Storage,
    now_ms: i64,
) -> llm_usage_core::storage::pricing::CostSummary {
    storage
        .cost_summary(&CostSummaryRequest {
            timezone: "UTC".to_string(),
            first_day: "2026-09-26".to_string(),
            last_day: "2026-09-26".to_string(),
            filters: CostFilters::default(),
            now_ms,
            options: EstimateOptions::default(),
        })
        .unwrap()
}

/// 采集后回填选日语义：只选"修订号大于刷新前值"的未封存日——
/// 分级保留在扫描后再 bump 修订号不得使选日落空，封存日不入选。
#[test]
fn v29_backfill_day_selection_uses_revision_floor() {
    let (_dir, storage) = temp_storage("v29backfill");
    let snapshot = parse_snapshot_json(SNAPSHOT).unwrap();
    storage
        .import_price_snapshot(&snapshot, ts("2026-09-25T00:00:00Z"))
        .unwrap();
    let now = ts("2026-09-27T12:00:00Z");
    let events = vec![priced_evt(
        "b1",
        "zhipuai",
        "glm-5.3",
        Some(1_000_000),
        Some(0),
        Some(0),
        Some(1_000),
    )];
    commit_batch(&storage, &batch("inst", "UTC", now, events), None).unwrap();
    let revision_after_scan = storage.data_revision().unwrap();

    // 刷新前修订号 < 扫描修订号 → 该日入选。
    let days = storage
        .cost_backfill_days_since("UTC", revision_after_scan - 1)
        .unwrap();
    assert_eq!(days, vec!["2026-09-26".to_string()]);

    // 无新写入的刷新（下限 = 当前修订号）→ 空集，不重复回填。
    let days_none = storage
        .cost_backfill_days_since("UTC", revision_after_scan)
        .unwrap();
    assert!(days_none.is_empty());

    // 封存日不入选：封存后即使修订号更大也跳过。
    storage
        .conn()
        .execute(
            "UPDATE daily_usage SET sealed = 1, data_revision = ?1",
            [revision_after_scan + 5],
        )
        .unwrap();
    let days_sealed = storage
        .cost_backfill_days_since("UTC", revision_after_scan - 1)
        .unwrap();
    assert!(days_sealed.is_empty());
}

/// 种子快照导入 + 校验（schema 校验/区间重叠/币种枚举在 pricing 单测覆盖）。
#[test]
fn v29_seed_snapshot_imports_idempotently() {
    let (_dir, storage) = temp_storage("v29seed");
    let out1 = storage
        .ensure_seed_price_snapshot(ts("2026-09-30T00:00:00Z"))
        .unwrap();
    assert!(!out1.already_present);
    assert!(out1.inserted_rows > 0);
    let out2 = storage
        .ensure_seed_price_snapshot(ts("2026-09-30T01:00:00Z"))
        .unwrap();
    assert!(out2.already_present);
    assert_eq!(out2.inserted_rows, 0);
    let snapshots = storage.list_price_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].snapshot_id, "seed-2026-09-25");
    assert_eq!(snapshots[0].row_count as usize, out1.inserted_rows);
}

/// 维度筛选语义：provider/model 筛选只命中所选值（空值事件不算入任何具体
/// 供应商筛选）；"unknown" 筛选值命中空 provider 事件（与 query::Filters 一致）。
#[test]
fn v29_dimension_filters_do_not_include_empty_values() {
    let (_dir, storage) = temp_storage("v29filter");
    let snapshot = parse_snapshot_json(SNAPSHOT).unwrap();
    storage
        .import_price_snapshot(&snapshot, ts("2026-09-25T00:00:00Z"))
        .unwrap();
    let now = ts("2026-09-27T12:00:00Z");
    // 三个事件：openai 有价、无 provider（不套价）、zhipuai 有价。
    let events = vec![
        priced_evt(
            "f1",
            "openai",
            "gpt-6-astra",
            Some(1_000_000),
            Some(0),
            Some(0),
            Some(1_000_000),
        ),
        {
            let mut e = priced_evt(
                "f2",
                "",
                "gpt-6-astra",
                Some(1_000_000),
                Some(0),
                Some(0),
                Some(1_000_000),
            );
            e.provider_id = None;
            e
        },
        priced_evt(
            "f3",
            "zhipuai",
            "glm-5.3",
            Some(1_000_000),
            Some(0),
            Some(0),
            Some(1_000_000),
        ),
    ];
    commit_batch(&storage, &batch("inst", "UTC", now, events), None).unwrap();
    storage
        .recompute_unsealed_cost_days("UTC", now, &options())
        .unwrap();

    let summary_with = |filters: llm_usage_core::storage::pricing::CostFilters| {
        storage
            .cost_summary(&CostSummaryRequest {
                timezone: "UTC".to_string(),
                first_day: "2026-09-26".to_string(),
                last_day: "2026-09-26".to_string(),
                filters,
                now_ms: now,
                options: options(),
            })
            .unwrap()
    };

    // 只筛 openai：不含无 provider 事件。输入合计 1M ≥ 272K → P3 长档：
    // in 1M×200000/1e8 = 2000 + out 1M×750000/1e8 = 7500 → 9500 美分。
    let openai_only = summary_with(llm_usage_core::storage::pricing::CostFilters {
        providers: vec!["openai".to_string()],
        ..Default::default()
    });
    let usd = openai_only
        .at_time
        .rows
        .iter()
        .find(|r| r.currency == "USD")
        .expect("USD row");
    assert_eq!(usd.total_amount_minor, 9500);
    assert_eq!(usd.priced_event_count, 1);

    // "unknown" 筛选：只命中无 provider 事件（未计价，channel_unknown）。
    let unknown_only = summary_with(llm_usage_core::storage::pricing::CostFilters {
        providers: vec!["unknown".to_string()],
        ..Default::default()
    });
    assert_eq!(
        unknown_only
            .at_time
            .rows
            .iter()
            .map(|r| r.unpriced_event_count)
            .sum::<i64>(),
        1
    );
    assert_eq!(
        unknown_only.at_time.unpriced_reasons.get("no_provider"),
        Some(&1)
    );

    // 模型筛选同理：只筛 glm-5.3（P1：in 800 + out 2800 = 3600 分，不含 gpt 事件）。
    let glm_only = summary_with(llm_usage_core::storage::pricing::CostFilters {
        models: vec!["glm-5.3".to_string()],
        ..Default::default()
    });
    let cny = glm_only
        .at_time
        .rows
        .iter()
        .find(|r| r.currency == "CNY")
        .expect("CNY row");
    assert_eq!(cny.total_amount_minor, 3600);
}
