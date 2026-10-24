//! V29 tests for price snapshots, cost estimation, and summaries.
//!
//! Fixed P1–P6 samples come from the pricing specification. Rates use hundredths of a
//! minor currency unit per million tokens (displayed major-unit prices multiplied by 10,000).
//! Expected amounts are calculated manually. Original E2 input totals 1.3M and therefore
//! selects the P3 long-context tier. This file tests the scaled 130K-input E2' under P2
//! and P2/P3 threshold boundaries; do not claim a separate original-1.3M test here.

mod common;

use common::{batch, evt, temp_storage, ts};
use llm_usage_core::domain::{CostAmount, CostKind, FieldQuality};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::pricing::{parse_snapshot_json, EstimateOptions, EventEstimate, PricingEvent};
use llm_usage_core::retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy};
use llm_usage_core::storage::pricing::{CostFilters, CostSummaryRequest};
use llm_usage_core::storage::Storage;

/// Fixed P1–P6 samples plus A4's batch comparison row, excluded from standard matching.
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
    // Test options: moonshot cache writes use 1h (A6/E3); openai uses 5m (E2', with P2 write prices).
    options.cache_ttl_minutes.insert("moonshot".into(), 60);
    options.cache_ttl_minutes.insert("openai".into(), 5);
    options
}

/// Build an event with four token components; present values are Reported, occurring at 2026-09-26 12:00 UTC.
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
    // Match quality to values (None means Unknown), as required by ingest validation.
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
    let summary = storage
        .cost_summary(&CostSummaryRequest {
            timezone: "UTC".to_string(),
            first_day: "2026-09-26".to_string(),
            last_day: "2026-09-26".to_string(),
            filters: CostFilters::default(),
            now_ms,
            options: options(),
        })
        .unwrap();
    for mode in [&summary.at_time, &summary.current_sim] {
        for total in &mode.rows {
            let model_rows: Vec<_> = summary
                .models
                .iter()
                .flat_map(|model| {
                    if std::ptr::eq(mode, &summary.at_time) {
                        &model.at_time
                    } else {
                        &model.current_sim
                    }
                })
                .filter(|row| row.currency == total.currency)
                .collect();
            assert_eq!(
                model_rows
                    .iter()
                    .map(|row| row.total_amount_minor)
                    .sum::<i64>(),
                total.total_amount_minor
            );
            assert_eq!(
                model_rows
                    .iter()
                    .map(|row| row.priced_event_count)
                    .sum::<i64>(),
                total.priced_event_count
            );
            assert_eq!(
                model_rows
                    .iter()
                    .map(|row| row.unpriced_event_count)
                    .sum::<i64>(),
                total.unpriced_event_count
            );
        }
    }
    for total in &summary.at_time.rows {
        assert_eq!(
            summary
                .daily
                .iter()
                .filter(|row| row.sums.currency == total.currency)
                .map(|row| row.sums.total_amount_minor)
                .sum::<i64>(),
            total.total_amount_minor
        );
    }
    for total in &summary.current_sim.rows {
        assert_eq!(
            summary
                .daily_current
                .iter()
                .filter(|r| r.sums.currency == total.currency)
                .map(|r| r.sums.total_amount_minor)
                .sum::<i64>(),
            total.total_amount_minor
        );
    }
    summary
}

#[test]
fn current_reference_rates_curve_and_hour_selection_preserve_frozen_costs() {
    let (_dir, storage) = temp_storage("reference-current");
    storage
        .import_price_snapshot(
            &parse_snapshot_json(SNAPSHOT).unwrap(),
            ts("2026-09-25T00:00:00Z"),
        )
        .unwrap();
    let mut a = priced_evt(
        "a",
        "openai",
        "gpt-6-astra",
        Some(100000),
        Some(0),
        Some(0),
        Some(10000),
    );
    let mut b = priced_evt(
        "b",
        "openai",
        "gpt-6-astra",
        Some(300000),
        Some(0),
        Some(0),
        Some(10000),
    );
    a.occurred_at_ms = ts("2026-09-26T08:00:00Z");
    b.occurred_at_ms = ts("2026-09-26T09:00:00Z");
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-26T12:00:00Z"), vec![a, b]),
        None,
    )
    .unwrap();
    storage
        .recompute_cost_day("UTC", "2026-09-26", ts("2026-09-26T12:00:00Z"), &options())
        .unwrap();
    let newer = r#"{"format":"llm-usage-price-snapshot/1","snapshot":{"id":"new-current","source_type":"manual","source_urls":[],"fetched_at":"2026-10-02"},"rows":[
      {"price_id":"new-base","provider_id":"openai","model":"gpt-6-astra","region":"global","channel":"api","currency":"USD","effective_from":"2026-10-02","input":300000,"output":1000000},
      {"price_id":"new-long","provider_id":"openai","model":"gpt-6-astra","region":"global","channel":"api","currency":"USD","effective_from":"2026-10-02","context_threshold_tokens":272000,"input":400000,"output":1000000}] }"#;
    storage
        .import_price_snapshot(
            &parse_snapshot_json(newer).unwrap(),
            ts("2026-10-02T12:00:00Z"),
        )
        .unwrap();
    let request = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-09-26".into(),
        last_day: "2026-09-26".into(),
        filters: CostFilters::default(),
        now_ms: ts("2026-10-02T12:00:00Z"),
        options: options(),
    };
    let full = storage.cost_summary(&request).unwrap();
    assert_eq!(full.at_time.rows[0].total_amount_minor, 825);
    assert_eq!(full.current_sim.rows[0].total_amount_minor, 1700);
    assert_eq!(full.daily_current[0].sums.total_amount_minor, 1700);
    assert_eq!(full.models[0].unit_prices.len(), 2);
    assert_eq!(full.models[0].unit_prices[0].price_id, "new-base");
    assert_eq!(
        full.models[0].unit_prices[1].context_threshold_tokens,
        272000
    );
    let hour = storage
        .cost_summary_selected(&request, Some(("2026-09-26 08:00", "2026-09-26 08:00")))
        .unwrap();
    assert_eq!(hour.current_sim.rows[0].total_amount_minor, 400);
    assert_eq!(hour.models[0].unit_prices.len(), 1);
    assert_eq!(hour.daily_current[0].sums.total_amount_minor, 400);
    assert_eq!(
        hour.at_time.rows[0].total_amount_minor, 825,
        "current-price query leaves frozen daily history intact"
    );
    assert!(storage
        .cost_summary_selected(&request, Some(("2026-09-26 25:00", "2026-09-26 25:00")))
        .is_err());
    assert!(storage
        .cost_summary_selected(&request, Some(("2026-09-26 é:00", "2026-09-26 é:00")))
        .is_err());
}

/// Test import, persistence, backfill, and summaries for E1/E3/E4–E7/E8 and A1–A8.
#[test]
fn v29_contract_amounts_and_anomalies() {
    let (_dir, storage) = temp_storage("v29");
    let snapshot = parse_snapshot_json(SNAPSHOT).unwrap();
    let imported = storage
        .import_price_snapshot(&snapshot, ts("2026-09-25T00:00:00Z"))
        .unwrap();
    assert_eq!(imported.inserted_rows, 7);
    // A10: repeated import of the same snapshot is idempotent.
    let again = storage
        .import_price_snapshot(&snapshot, ts("2026-09-26T00:00:00Z"))
        .unwrap();
    assert!(again.already_present);
    assert_eq!(again.inserted_rows, 0);

    let now = ts("2026-09-27T12:00:00Z");
    let events = vec![
        // E1 mixed input (P1, CNY): 988+100+968=2056 fen; cache writes remain unpriced.
        priced_evt(
            "e1",
            "zhipuai",
            "glm-5.3",
            Some(1_234_567),
            Some(500_000),
            Some(200_000),
            Some(345_678),
        ),
        // E2' has all four components under P2 (130K input <272K): 100+2+13+125=240 US cents.
        priced_evt(
            "e2",
            "openai",
            "gpt-6-astra",
            Some(100_000),
            Some(20_000),
            Some(10_000),
            Some(25_000),
        ),
        // E3 1h TTL (P4; test moonshot default 60 minutes): 120+9+360+225=714 US cents.
        priced_evt(
            "e3",
            "moonshot",
            "kimi-k3",
            Some(400_000),
            Some(300_000),
            Some(600_000),
            Some(150_000),
        ),
        // E4 upper-tier boundary (272,000 total input selects P3): 544+60=604 US cents.
        priced_evt(
            "e4",
            "openai",
            "gpt-6-astra",
            Some(272_000),
            Some(0),
            Some(0),
            Some(8_000),
        ),
        // E5 below the boundary (271,999 selects P2): 272+40=312 US cents.
        priced_evt(
            "e5",
            "openai",
            "gpt-6-astra",
            Some(271_999),
            Some(0),
            Some(0),
            Some(8_000),
        ),
        // E6 GLM threshold (32,768 selects P6): 26+3=29 fen.
        priced_evt(
            "e6",
            "zhipuai",
            "glm-5.1",
            Some(32_768),
            Some(0),
            Some(0),
            Some(1_000),
        ),
        // E7 below the GLM threshold (32,767 selects P5): 20+2=22 fen.
        priced_evt(
            "e7",
            "zhipuai",
            "glm-5.1",
            Some(32_767),
            Some(0),
            Some(0),
            Some(1_000),
        ),
        // A1: a model without pay-as-you-go prices remains unpriced.
        priced_evt(
            "a1",
            "moonshot",
            "kimi-for-coding",
            Some(1_000),
            Some(1_000),
            Some(0),
            Some(1_000),
        ),
        // A2: partially priced P1 usage with known input and unknown output.
        priced_evt(
            "a2",
            "zhipuai",
            "glm-5.3",
            Some(1_000_000),
            Some(0),
            Some(0),
            None,
        ),
        // A3: reasoning is a subset of output, without an additional charge (130K-input P2 tier).
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
        // A5: absent cache-read prices (NULL in P5/P6) leave reads unpriced; price other components normally.
        priced_evt(
            "a5",
            "zhipuai",
            "glm-5.1",
            Some(32_767),
            Some(100_000),
            Some(0),
            Some(1_000),
        ),
        // A6: unknown TTL, no zhipuai default, and no P1 write price leave writes unpriced, as in E1.
        // A7: an event before snapshot effectiveness is unpriced at_time but can use current_sim.
        priced_evt(
            "a7",
            "zhipuai",
            "glm-5.3",
            Some(1_000_000),
            Some(0),
            Some(0),
            Some(1_000),
        ),
        // A8: reject pricing when cache reads+writes exceed known total input.
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
            // Contradictory source fields: total input 60,000 < reads+writes 100,000.
            e.usage.input_total = Some(60_000);
            e
        },
        // Source-recorded amounts in Crush's shape: 1000 US cents Reported +500 US cents source-estimated.
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
    // Move A7 to 2026-09-20, before snapshot effectiveness, and a separate source instance.
    let mut events: Vec<_> = events;
    if let Some(a7) = events.iter_mut().find(|e| e.source_record_key == "a7") {
        a7.occurred_at_ms = ts("2026-09-20T12:00:00Z");
        a7.source_instance_id = "inst2".to_string();
    }
    // Commit batches on two days to check backfill by day.
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

    // Backfill 09-20 and 09-26.
    let outcomes = storage
        .recompute_unsealed_cost_days("UTC", now, &options())
        .unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|o| o.rebuilt));

    let summary = run_summary(&storage, now);

    // E1 (P1/CNY): 2056 fen; check E6/E7/E5 tiers, partial A2, A5, and src2 below.
    let cny = currency_row(&summary, at_time, "CNY");
    // Sum GLM-5.3 uncached input before rounding: 987.6536+800+0.8=1788.4536→1788 fen,
    // rather than 988+800+1. GLM-5.1 input is 72 fen; total CNY amount is 2939 fen.
    // A5 reads of 100,000 make total input 132,767≥32,768, selecting P6; NULL read prices remain unpriced.
    assert_eq!(cny.total_amount_minor, 2939);
    assert_eq!(cny.input_amount_minor, Some(1788 + 72));
    // E1's 200,000 write tokens and A5's 100,000 read tokens are unpriced: known > priced.
    assert!(cny.known_tokens > cny.priced_tokens);
    // Partial cases: E1 unpriced writes, A2 unknown output, A5 unpriced reads.
    assert_eq!(cny.partial_event_count, 3);

    // E2'/E4/E5/E3 + A3 + src1 estimation in USD.
    let usd = currency_row(&summary, at_time, "USD");
    // Total USD amount: 240+604+312+714+1100+6=2976 cents.
    assert_eq!(usd.total_amount_minor, 240 + 604 + 312 + 714 + 1100 + 6);

    // E8: retain separate currencies without exchange-rate conversion.
    assert_eq!(
        summary
            .at_time
            .rows
            .iter()
            .filter(|r| !r.currency.is_empty())
            .count(),
        2
    );

    // Check A1/A8 unpriced counts and reasons.
    let unpriced = currency_row(&summary, at_time, "");
    assert_eq!(unpriced.unpriced_event_count, 2);
    let reasons = &summary.at_time.unpriced_reasons;
    assert_eq!(reasons.get("no_price_row"), Some(&1)); // A1: no_price_row.
    assert_eq!(reasons.get("token_anomaly"), Some(&1)); // A8: token_anomaly.

    // Source amounts are separate from app estimates; Reported and source_estimate share a same-currency display total.
    let src = summary
        .source_amounts
        .iter()
        .find(|r| r.currency == "USD")
        .expect("source USD row");
    assert_eq!(src.total_amount_minor, 1500);

    // A7: the 09-26 range excludes its 09-20 event in both at_time and current_sim.
    // A4: do not use batch prices for standard service; E3 uses P4, verified by the 714-cent assertion.
    // A3: 100 input +1000 output cents, without charging reasoning again.

    // Current simulation has events in the 09-26 range, so detail_limited is false;
    // current rows cover now and give the same amounts as at_time in this snapshot.
    assert!(!summary.current_sim.detail_limited);
    let sim_usd = currency_row(&summary, current_sim, "USD");
    assert_eq!(sim_usd.total_amount_minor, usd.total_amount_minor);
    assert_eq!(sim_usd.unpriced_event_count, 0);

    // A7's 09-20 range has an at_time row with reason no_price_row;
    // current_sim has an amount using an effective current price row.
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
    // Input 1M×80000/1e8=800 + output 1000×280000/1e8 rounds to 3 fen.
    assert_eq!(a7_cny.total_amount_minor, 803);
    let a7_at = currency_row(&a7_summary, at_time, "");
    assert_eq!(a7_at.unpriced_event_count, 1);
    assert_eq!(
        a7_summary.at_time.unpriced_reasons.get("no_price_row"),
        Some(&1)
    );

    // Estimation references include the sample snapshot and a positive data revision.
    assert!(summary.price_basis.contains(&"v29-sample".to_string()));
    assert!(summary.data_revision > 0);

    // Repeated backfill is idempotent.
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

/// Without a configured channel or an official rate, leave usage unpriced; do not use unofficial manual rates as references.
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
        summary.at_time.unpriced_reasons.get("no_price_row"),
        Some(&1)
    );
}

#[test]
fn v29_unpriced_reasons_follow_event_dimensions() {
    let (_dir, storage) = temp_storage("v29-unpriced-filters");
    let now = ts("2026-09-27T12:00:00Z");
    let mut first = priced_evt(
        "first",
        "zhipuai",
        "glm-5.3",
        Some(1_000),
        Some(0),
        Some(0),
        Some(100),
    );
    first.source_instance_id = "instance-a".into();
    let mut second = priced_evt(
        "second",
        "moonshot",
        "kimi-k3",
        Some(1_000),
        Some(0),
        Some(0),
        Some(100),
    );
    second.source_instance_id = "instance-b".into();
    second.agent = "agent-b".into();
    commit_batch(
        &storage,
        &batch("instance-a", "UTC", now, vec![first]),
        None,
    )
    .unwrap();
    commit_batch(
        &storage,
        &batch("instance-b", "UTC", now + 1, vec![second]),
        None,
    )
    .unwrap();
    storage
        .recompute_unsealed_cost_days("UTC", now, &EstimateOptions::default())
        .unwrap();

    let summary = |filters| {
        storage
            .cost_summary(&CostSummaryRequest {
                timezone: "UTC".into(),
                first_day: "2026-09-26".into(),
                last_day: "2026-09-26".into(),
                filters,
                now_ms: now,
                options: EstimateOptions::default(),
            })
            .unwrap()
    };
    let all = summary(CostFilters::default());
    assert_eq!(currency_row(&all, at_time, "").unpriced_event_count, 2);
    assert_eq!(all.at_time.unpriced_reasons.get("no_price_row"), Some(&2));

    let only_a = summary(CostFilters {
        agents: vec!["agent-a".into()],
        providers: vec!["zhipuai".into()],
        models: vec!["glm-5.3".into()],
        instances: Some(vec!["instance-a".into()]),
    });
    assert_eq!(currency_row(&only_a, at_time, "").unpriced_event_count, 1);
    assert_eq!(
        only_a.at_time.unpriced_reasons.get("no_price_row"),
        Some(&1)
    );
    let other = summary(CostFilters {
        agents: vec!["agent-b".into()],
        providers: vec!["moonshot".into()],
        models: vec!["kimi-k3".into()],
        instances: Some(vec!["instance-b".into()]),
    });
    assert_eq!(currency_row(&other, at_time, "").unpriced_event_count, 1);
    assert_eq!(other.at_time.unpriced_reasons.get("no_price_row"), Some(&1));
}

#[test]
fn v29_newer_manual_snapshot_overrides_matching_price_interval() {
    let (_dir, storage) = temp_storage("v29-price-override");
    storage
        .import_price_snapshot(
            &parse_snapshot_json(SNAPSHOT).unwrap(),
            ts("2026-09-25T00:00:00Z"),
        )
        .unwrap();
    let mut newer: serde_json::Value = serde_json::from_str(SNAPSHOT).unwrap();
    newer["snapshot"]["id"] = serde_json::json!("v29-override");
    newer["snapshot"]["fetched_at"] = serde_json::json!("2026-10-01");
    let mut price = newer["rows"][0].clone();
    price["price_id"] = serde_json::json!("P1-override");
    price["effective_from"] = serde_json::json!("2026-10-01");
    price["input"] = serde_json::json!(160000);
    newer["rows"] = serde_json::json!([price]);
    storage
        .import_price_snapshot(
            &parse_snapshot_json(&newer.to_string()).unwrap(),
            ts("2026-10-01T00:00:00Z"),
        )
        .unwrap();

    let book = storage.load_price_book().unwrap();
    let mut event = PricingEvent {
        provider_id: Some("zhipuai".into()),
        model_raw: Some("glm-5.3".into()),
        input_uncached: Some(1_000_000),
        input_cache_read: Some(0),
        input_cache_write: Some(0),
        input_total: Some(1_000_000),
        output_total: Some(0),
        occurred_at_ms: ts("2026-09-27T12:00:00Z"),
        ..Default::default()
    };
    match book.estimate_at_time(&event, &options()) {
        EventEstimate::Priced(amounts) => {
            assert_eq!(amounts.total_amount_minor, 800);
            assert_eq!(amounts.matched_price_ids, ["P1"]);
        }
        other => panic!("expected original price, got {other:?}"),
    }
    event.occurred_at_ms = ts("2026-10-02T12:00:00Z");
    match book.estimate_at_time(&event, &options()) {
        EventEstimate::Priced(amounts) => {
            assert_eq!(amounts.total_amount_minor, 1600);
            assert_eq!(amounts.matched_price_ids, ["P1-override"]);
        }
        other => panic!("expected manual override, got {other:?}"),
    }
}

#[test]
fn v29_same_snapshot_id_rejects_provenance_changes() {
    let (_dir, storage) = temp_storage("v29-provenance");
    let original = parse_snapshot_json(SNAPSHOT).unwrap();
    storage.import_price_snapshot(&original, 1).unwrap();
    assert!(
        storage
            .import_price_snapshot(&original, 2)
            .unwrap()
            .already_present
    );

    let mut changed: serde_json::Value = serde_json::from_str(SNAPSHOT).unwrap();
    changed["snapshot"]["note"] = serde_json::json!("new source note");
    assert!(storage
        .import_price_snapshot(&parse_snapshot_json(&changed.to_string()).unwrap(), 3)
        .is_err());
    changed["snapshot"].as_object_mut().unwrap().remove("note");
    changed["rows"][0]["note"] = serde_json::json!("new row note");
    assert!(storage
        .import_price_snapshot(&parse_snapshot_json(&changed.to_string()).unwrap(), 4)
        .is_err());
    assert_eq!(storage.list_price_snapshots().unwrap()[0].row_count, 7);
}

#[test]
fn v29_current_sim_marks_partial_detail_retention_by_filter() {
    let (_dir, storage) = temp_storage("v29-partial-retention");
    storage
        .import_price_snapshot(
            &parse_snapshot_json(SNAPSHOT).unwrap(),
            ts("2026-09-25T00:00:00Z"),
        )
        .unwrap();
    let now = ts("2026-09-27T12:00:00Z");
    let mut old = priced_evt(
        "old",
        "zhipuai",
        "glm-5.3",
        Some(1_000_000),
        Some(0),
        Some(0),
        Some(0),
    );
    old.occurred_at_ms = ts("2026-09-25T12:00:00Z");
    let mut recent = priced_evt(
        "recent",
        "zhipuai",
        "glm-5.3",
        Some(1_000_000),
        Some(0),
        Some(0),
        Some(0),
    );
    recent.agent = "agent-b".into();
    commit_batch(&storage, &batch("inst", "UTC", now, vec![old]), None).unwrap();
    commit_batch(&storage, &batch("inst", "UTC", now + 1, vec![recent]), None).unwrap();
    storage
        .recompute_unsealed_cost_days("UTC", now, &options())
        .unwrap();
    enforce_tiered_retention(
        &storage,
        "UTC",
        now,
        &TieredRetentionPolicy {
            events_days: 2,
            ..TieredRetentionPolicy::default()
        },
    )
    .unwrap();

    let summary = |filters| {
        storage
            .cost_summary(&CostSummaryRequest {
                timezone: "UTC".into(),
                first_day: "2026-09-25".into(),
                last_day: "2026-09-26".into(),
                filters,
                now_ms: now,
                options: options(),
            })
            .unwrap()
    };
    let all = summary(CostFilters::default());
    assert!(all.current_sim.detail_limited);
    assert_eq!(currency_row(&all, current_sim, "CNY").priced_event_count, 2);
    assert_eq!(
        currency_row(&all, current_sim, "CNY").aggregate_event_count,
        1
    );
    assert_eq!(
        currency_row(&all, current_sim, "CNY").total_amount_minor,
        1600
    );
    let recent_only = summary(CostFilters {
        agents: vec!["agent-b".into()],
        ..CostFilters::default()
    });
    assert!(!recent_only.current_sim.detail_limited);
    let old_only = summary(CostFilters {
        agents: vec!["agent-a".into()],
        ..CostFilters::default()
    });
    assert!(old_only.current_sim.detail_limited);
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

/// After collection, select unsealed days with revisions above the pre-refresh revision;
/// later retention revision increases must not hide them. Exclude sealed days.
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

    // A revision above the pre-refresh value selects the day.
    let days = storage
        .cost_backfill_days_since("UTC", revision_after_scan - 1)
        .unwrap();
    assert_eq!(days, vec!["2026-09-26".to_string()]);

    // A refresh with no new writes and a lower bound equal to the current revision selects nothing.
    let days_none = storage
        .cost_backfill_days_since("UTC", revision_after_scan)
        .unwrap();
    assert!(days_none.is_empty());

    // Exclude sealed days even when their revision increases.
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

/// Import and validate all seed snapshots in one idempotent startup sequence.
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
    assert_eq!(snapshots.len(), 6);
    let original = snapshots
        .iter()
        .find(|s| s.snapshot_id == "seed-2026-09-25")
        .unwrap();
    assert_eq!(original.row_count as usize, out1.inserted_rows);
    assert_eq!(
        snapshots
            .iter()
            .find(|s| s.snapshot_id == "seed-2026-10-02-copilot-models")
            .unwrap()
            .row_count,
        2
    );
    assert_eq!(
        snapshots
            .iter()
            .find(|s| s.snapshot_id == "seed-2026-10-03-hy4")
            .unwrap()
            .row_count,
        1
    );
    let october_24 = snapshots
        .iter()
        .find(|s| s.snapshot_id == "seed-2026-10-24")
        .unwrap();
    assert_eq!(october_24.row_count, 25);
    assert_eq!(october_24.source_urls.len(), 7);
    assert!(october_24
        .source_urls
        .contains(&"https://platform.kimi.com/docs/guide/context-caching.md".into()));
    assert!(october_24
        .source_urls
        .contains(&"https://docs.bigmodel.cn/cn/guide/start/pricing.md".into()));
    let book = storage.load_price_book().unwrap();
    assert_eq!(book.rows.len(), 55);
    for retained_id in [
        "openai-gpt-5.5-20261005-long",
        "anthropic-opus-5.5",
        "zhipuai-glm-5.3-flash-cn",
    ] {
        assert!(book.rows.iter().any(|row| row.price_id == retained_id));
    }
}

#[test]
fn v29_october_24_seed_rates_and_unknown_components() {
    let snapshot = parse_snapshot_json(include_str!("../prices/seed-2026-10-24.json")).unwrap();
    assert_eq!(snapshot.snapshot_id, "seed-2026-10-24");
    assert_eq!(snapshot.source_urls.len(), 7);
    assert_eq!(snapshot.rows.len(), 25);

    // Rates are input/read/write5m/write1h/output/storage, in stored integer units.
    let expected = [
        (
            "openai",
            "gpt-6-astra",
            "openai-gpt-6-astra-20261024-short",
            0,
            [Some(100000), Some(10000), None, None, Some(500000), None],
        ),
        (
            "openai",
            "gpt-6-astra",
            "openai-gpt-6-astra-20261024-long",
            272001,
            [Some(200000), Some(20000), None, None, Some(750000), None],
        ),
        (
            "openai",
            "gpt-6-sol",
            "openai-gpt-6-sol-20261024-short",
            0,
            [Some(20000), Some(2000), None, None, Some(100000), None],
        ),
        (
            "openai",
            "gpt-6-sol",
            "openai-gpt-6-sol-20261024-long",
            272001,
            [Some(40000), Some(4000), None, None, Some(150000), None],
        ),
        (
            "openai",
            "gpt-6.1-sol",
            "openai-gpt-6.1-sol-20261024-short",
            0,
            [Some(20000), Some(1000), None, None, Some(100000), None],
        ),
        (
            "openai",
            "gpt-6.1-sol",
            "openai-gpt-6.1-sol-20261024-long",
            272001,
            [Some(40000), Some(2000), None, None, Some(150000), None],
        ),
        (
            "openai",
            "gpt-6-luna",
            "openai-gpt-6-luna-20261024-short",
            0,
            [Some(1000), Some(100), None, None, Some(5000), None],
        ),
        (
            "openai",
            "gpt-6-luna",
            "openai-gpt-6-luna-20261024-long",
            272001,
            [Some(2000), Some(200), None, None, Some(7500), None],
        ),
        (
            "anthropic",
            "claude-opus-5-5",
            "anthropic-claude-opus-5-5-20261024",
            0,
            [
                Some(40000),
                Some(2000),
                Some(50000),
                Some(80000),
                Some(200000),
                None,
            ],
        ),
        (
            "anthropic",
            "claude-sonnet-5",
            "anthropic-claude-sonnet-5-20261024",
            0,
            [
                Some(20000),
                Some(2000),
                Some(25000),
                Some(40000),
                Some(100000),
                None,
            ],
        ),
        (
            "anthropic",
            "claude-haiku-4-5",
            "anthropic-claude-haiku-4-5-20261024",
            0,
            [
                Some(10000),
                Some(1000),
                Some(12500),
                Some(20000),
                Some(50000),
                None,
            ],
        ),
        (
            "anthropic",
            "claude-haiku-4-5-20251001",
            "anthropic-claude-haiku-4-5-20251001-standard-20261024",
            0,
            [
                Some(10000),
                Some(1000),
                Some(12500),
                Some(20000),
                Some(50000),
                None,
            ],
        ),
        (
            "anthropic",
            "claude-sonnet-5-5",
            "anthropic-claude-sonnet-5-5-20261024",
            0,
            [
                Some(20000),
                Some(1000),
                Some(25000),
                Some(40000),
                Some(100000),
                None,
            ],
        ),
        (
            "anthropic",
            "claude-haiku-5-5",
            "anthropic-claude-haiku-5-5-20261024-short",
            0,
            [
                Some(1000),
                Some(100),
                Some(1250),
                Some(2000),
                Some(5000),
                None,
            ],
        ),
        (
            "anthropic",
            "claude-haiku-5-5",
            "anthropic-claude-haiku-5-5-20261024-long",
            100001,
            [
                Some(5000),
                Some(500),
                Some(6250),
                Some(10000),
                Some(25000),
                None,
            ],
        ),
        (
            "google",
            "gemini-3.8-flash",
            "google-gemini-3.8-flash-2027-full-20261024",
            0,
            [
                Some(15000),
                Some(1500),
                None,
                None,
                Some(75000),
                Some(10000),
            ],
        ),
        (
            "google",
            "gemini-4-argon",
            "google-gemini-4-argon-20261024",
            0,
            [Some(20000), Some(1000), None, None, Some(100000), None],
        ),
        (
            "google",
            "gemini-4-argon",
            "google-gemini-4-argon-20261201",
            0,
            [Some(40000), Some(2000), None, None, Some(200000), None],
        ),
        (
            "moonshot",
            "kimi-k3",
            "moonshot-kimi-k3-cn-20261024",
            0,
            [
                Some(200000),
                Some(20000),
                Some(200000),
                Some(400000),
                Some(1000000),
                None,
            ],
        ),
        (
            "zhipuai",
            "glm-5.3-flash",
            "zhipuai-glm-5.3-flash-cn-20261024",
            0,
            [Some(8000), Some(2300), None, None, Some(28000), Some(0)],
        ),
        (
            "zhipuai",
            "glm-5.1",
            "zhipuai-glm-5.1-cn-0-20261024",
            0,
            [Some(60000), Some(13000), None, None, Some(240000), Some(0)],
        ),
        (
            "zhipuai",
            "glm-5.1",
            "zhipuai-glm-5.1-cn-32k-20261024",
            32768,
            [Some(80000), Some(20000), None, None, Some(280000), Some(0)],
        ),
        (
            "zai",
            "glm-5.3",
            "zai-glm-5.3-20261024",
            0,
            [Some(14000), Some(2600), None, None, Some(44000), Some(0)],
        ),
        (
            "zai",
            "glm-5.3-flash",
            "zai-glm-5.3-flash-20261024",
            0,
            [Some(1500), Some(300), None, None, Some(5000), Some(0)],
        ),
        (
            "zai",
            "glm-5.3-flashx",
            "zai-glm-5.3-flashx-20261024",
            0,
            [Some(3700), Some(750), None, None, Some(12500), Some(0)],
        ),
    ];
    for (provider, model, price_id, threshold, rates) in expected {
        let row = snapshot
            .rows
            .iter()
            .find(|row| row.price_id == price_id)
            .unwrap_or_else(|| panic!("missing {price_id}"));
        let (region, channel, currency) = match provider {
            "openai" | "anthropic" | "google" => ("global", "api", "USD"),
            "moonshot" => ("cn", "api", "CNY"),
            "zhipuai" => ("cn", "bigmodel", "CNY"),
            "zai" => ("global", "zai", "USD"),
            other => panic!("unexpected provider {other}"),
        };
        assert_eq!(row.provider_id, provider, "{price_id}");
        assert_eq!(row.model, model, "{price_id}");
        assert_eq!(row.region, region, "{price_id}");
        assert_eq!(row.channel, channel, "{price_id}");
        assert_eq!(row.currency, currency, "{price_id}");
        assert_eq!(row.service_tier, "standard", "{price_id}");
        assert!(row.official_vendor, "{price_id}");
        assert_eq!(row.context_threshold_tokens, threshold, "{price_id}");
        let (effective_from, effective_to) = match price_id {
            "google-gemini-3.8-flash-2027-full-20261024" => (ts("2027-01-01T00:00:00Z"), None),
            "google-gemini-4-argon-20261024" => {
                (ts("2026-10-24T00:00:00Z"), Some(ts("2026-12-01T00:00:00Z")))
            }
            "google-gemini-4-argon-20261201" => (ts("2026-12-01T00:00:00Z"), None),
            _ => (ts("2026-10-24T00:00:00Z"), None),
        };
        assert_eq!(row.effective_from_ms, effective_from, "{price_id}");
        assert_eq!(row.effective_to_ms, effective_to, "{price_id}");
        let actual_rates = [
            row.input_per_mtok_hundredths,
            row.cache_read_per_mtok_hundredths,
            row.cache_write_5m_per_mtok_hundredths,
            row.cache_write_1h_per_mtok_hundredths,
            row.output_per_mtok_hundredths,
            row.cache_storage_per_mtok_hour_hundredths,
        ];
        assert_eq!(actual_rates, rates, "{price_id}");
    }

    let openai_rows: Vec<_> = snapshot
        .rows
        .iter()
        .filter(|row| row.provider_id == "openai")
        .collect();
    assert_eq!(openai_rows.len(), 8);
    for row in openai_rows {
        assert_eq!(row.cache_write_5m_per_mtok_hundredths, None);
        assert_eq!(row.cache_write_1h_per_mtok_hundredths, None);
        assert_eq!(row.cache_storage_per_mtok_hour_hundredths, None);
        let note = row.note.as_deref().unwrap();
        assert!(note.contains("30m/12h"), "{}", row.price_id);
        assert!(
            note.contains("schema supports only 5m/1h"),
            "{}",
            row.price_id
        );
        assert!(note.contains("unpriced"), "{}", row.price_id);
    }
    for (price_id, short_write_prices) in [
        ("openai-gpt-6-astra-20261024-short", "$12.50/$25.00"),
        ("openai-gpt-6-sol-20261024-short", "$2.50/$5.00"),
        ("openai-gpt-6.1-sol-20261024-short", "$2.50/$5.00"),
        ("openai-gpt-6-luna-20261024-short", "$0.125/$0.25"),
    ] {
        let note = snapshot
            .rows
            .iter()
            .find(|row| row.price_id == price_id)
            .unwrap()
            .note
            .as_deref()
            .unwrap();
        assert!(note.contains(short_write_prices), "{price_id}");
    }
    assert!(!snapshot.rows.iter().any(|row| row.model == "gpt-5.5"));
    assert!(!snapshot
        .rows
        .iter()
        .any(|row| { row.model == "kimi-k2.8-preview" || row.model == "k28-agent-preview" }));
}

#[test]
fn v29_october_24_thresholds_future_rows_and_openai_writes() {
    let (_dir, storage) = temp_storage("v29-october-24-selection");
    storage
        .ensure_seed_price_snapshot(ts("2026-10-24T00:00:00Z"))
        .unwrap();
    let book = storage.load_price_book().unwrap();
    let mut options = options();
    options
        .provider_channels
        .insert("anthropic".into(), ("global".into(), "api".into()));
    options
        .provider_channels
        .insert("google".into(), ("global".into(), "api".into()));
    let event = |provider: &str, model: &str, day: &str, input: i64| PricingEvent {
        provider_id: Some(provider.into()),
        model_raw: Some(model.into()),
        occurred_at_ms: ts(&format!("{day}T12:00:00Z")),
        input_uncached: Some(input),
        input_cache_read: Some(0),
        input_cache_write: Some(0),
        input_total: Some(input),
        output_total: Some(0),
        ..Default::default()
    };
    let estimate = |event: &PricingEvent| match book.estimate_at_time(event, &options) {
        EventEstimate::Priced(amounts) => amounts,
        other => panic!("expected priced event, got {other:?}"),
    };

    let short = estimate(&event("openai", "gpt-6-astra", "2026-10-24", 272000));
    assert_eq!(
        short.matched_price_ids,
        ["openai-gpt-6-astra-20261024-short"]
    );
    assert_eq!(short.total_amount_minor, 272);
    let long = estimate(&event("openai", "gpt-6-astra", "2026-10-24", 272001));
    assert_eq!(long.matched_price_ids, ["openai-gpt-6-astra-20261024-long"]);
    assert_eq!(long.total_amount_minor, 544);

    let short = estimate(&event(
        "anthropic",
        "claude-haiku-5-5",
        "2026-10-24",
        100000,
    ));
    assert_eq!(
        short.matched_price_ids,
        ["anthropic-claude-haiku-5-5-20261024-short"]
    );
    let long = estimate(&event(
        "anthropic",
        "claude-haiku-5-5",
        "2026-10-24",
        100001,
    ));
    assert_eq!(
        long.matched_price_ids,
        ["anthropic-claude-haiku-5-5-20261024-long"]
    );

    let short = estimate(&event("zhipuai", "glm-5.1", "2026-10-24", 32767));
    assert_eq!(short.matched_price_ids, ["zhipuai-glm-5.1-cn-0-20261024"]);
    let long = estimate(&event("zhipuai", "glm-5.1", "2026-10-24", 32768));
    assert_eq!(long.matched_price_ids, ["zhipuai-glm-5.1-cn-32k-20261024"]);

    let before = estimate(&event("google", "gemini-4-argon", "2026-11-30", 1000000));
    assert_eq!(before.matched_price_ids, ["google-gemini-4-argon-20261024"]);
    assert_eq!(before.total_amount_minor, 200);
    let after = estimate(&event("google", "gemini-4-argon", "2026-12-01", 1000000));
    assert_eq!(after.matched_price_ids, ["google-gemini-4-argon-20261201"]);
    assert_eq!(after.total_amount_minor, 400);

    let before = estimate(&event("google", "gemini-3.8-flash", "2026-12-31", 1000000));
    assert_eq!(before.matched_price_ids, ["google-gemini-3.8-flash"]);
    assert_eq!(before.total_amount_minor, 75);
    let after = estimate(&event("google", "gemini-3.8-flash", "2027-01-01", 1000000));
    assert_eq!(
        after.matched_price_ids,
        ["google-gemini-3.8-flash-2027-full-20261024"]
    );
    assert_eq!(after.total_amount_minor, 150);

    let mut write_event = event("openai", "gpt-6.1-sol", "2026-10-24", 100000);
    write_event.input_cache_write = Some(10000);
    write_event.input_total = Some(110000);
    let amounts = estimate(&write_event);
    assert_eq!(
        amounts.matched_price_ids,
        ["openai-gpt-6.1-sol-20261024-short"]
    );
    assert_eq!(amounts.cache_write_amount_minor, None);
    assert_eq!(amounts.total_amount_minor, 20);
    assert!(amounts.priced_tokens < amounts.known_tokens);
    assert!(!amounts.ttl_defaulted);
}

/// Provider/model filters match selected values; missing providers do not match any named
/// provider. The "unknown" filter selects missing providers, as query::Filters does.
#[test]
fn v29_dimension_filters_do_not_include_empty_values() {
    let (_dir, storage) = temp_storage("v29filter");
    let snapshot = parse_snapshot_json(SNAPSHOT).unwrap();
    storage
        .import_price_snapshot(&snapshot, ts("2026-09-25T00:00:00Z"))
        .unwrap();
    let now = ts("2026-09-27T12:00:00Z");
    // Three events: priced openai, missing provider, and priced zhipuai.
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

    // Filter openai only, excluding missing-provider events. Total input 1M≥272K selects P3:
    // input 1M×200000/1e8=2000 + output 1M×750000/1e8=7500 gives 9500 US cents.
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

    // The "unknown" filter selects only missing-provider events, unpriced with channel_unknown.
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
        unknown_only.at_time.unpriced_reasons.get("no_price_row"),
        Some(&1)
    );

    // Model filtering selects glm-5.3 only: P1 input 800 + output 2800=3600 fen, excluding GPT events.
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

/// Test official-provider fallback when the event provider has no exact price row:
/// use the official pay-as-you-go rate and retain fallback_event_count in daily costs and summaries.
/// Unconfigured channels may use an explicitly official same-model reference; actual channel remains unknown.
#[test]
fn v29_official_provider_fallback_end_to_end() {
    let (_dir, storage) = temp_storage("v29fallback");
    // Synthetic community snapshot: official vendorA m-one; official_vendor defaults true, with $10/$50 USD rates.
    let community = parse_snapshot_json(
        r#"{"format":"llm-usage-price-snapshot/1",
            "snapshot":{"id":"models-dev-2026-09-25-test","source_type":"community",
              "source_urls":["https://models.dev/api.json"],"fetched_at":"2026-09-25"},
            "rows":[{"price_id":"md:m-one","provider_id":"vendorA","model":"m-one",
              "region":"global","channel":"api","effective_from":"2026-09-25","currency":"USD",
              "input":100000,"cache_read":10000,"cache_write_5m":null,"output":500000}]}"#,
    )
    .unwrap();
    storage
        .import_price_snapshot(&community, ts("2026-09-25T00:00:00Z"))
        .unwrap();

    let now = ts("2026-09-27T12:00:00Z");
    let events = vec![
        // relay-x lacks an exact row; vendorA's official reference gives 1M×$10 +1M×$50=$60, or 6000 US cents.
        priced_evt(
            "fb1",
            "relay-x",
            "m-one",
            Some(1_000_000),
            Some(0),
            Some(0),
            Some(1_000_000),
        ),
        // An unconfigured channel may also use this model's explicitly official reference price.
        priced_evt(
            "fb2",
            "relay-unconfigured",
            "m-one",
            Some(1_000_000),
            Some(0),
            Some(0),
            Some(1_000_000),
        ),
    ];
    commit_batch(&storage, &batch("inst", "UTC", now, events), None).unwrap();

    let mut options = EstimateOptions::default();
    options
        .provider_channels
        .insert("relay-x".into(), ("global".into(), "api".into()));
    storage
        .recompute_unsealed_cost_days("UTC", now, &options)
        .unwrap();

    let summary = storage
        .cost_summary(&CostSummaryRequest {
            timezone: "UTC".to_string(),
            first_day: "2026-09-26".to_string(),
            last_day: "2026-09-26".to_string(),
            filters: CostFilters::default(),
            now_ms: now,
            options: options.clone(),
        })
        .unwrap();
    let usd = currency_row(&summary, at_time, "USD");
    assert_eq!(usd.total_amount_minor, 12000);
    assert_eq!(usd.priced_event_count, 2);
    assert_eq!(usd.fallback_event_count, 2);
    assert!(summary
        .price_basis
        .contains(&"models-dev-2026-09-25-test".to_string()));
    assert!(summary.at_time.unpriced_reasons.is_empty());
    // Current simulation also uses and counts official-provider fallback.
    let sim_usd = currency_row(&summary, current_sim, "USD");
    assert_eq!(sim_usd.fallback_event_count, 2);
}
