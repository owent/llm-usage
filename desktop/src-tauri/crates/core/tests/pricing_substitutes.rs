mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    domain::FieldQuality,
    ingest::commit_batch,
    model_names::reference_model_key_for,
    pricing::{EstimateOptions, EventEstimate, PriceBook, PricingEvent, UnpricedReason},
    storage::pricing::CostSummaryRequest,
};

fn usage(model: &str) -> PricingEvent {
    PricingEvent {
        model_raw: Some(model.into()),
        provider_id: Some("custom".into()),
        occurred_at_ms: ts("2026-10-05T10:00:00Z"),
        input_uncached: Some(1_000_000),
        input_cache_read: Some(1_000_000),
        input_cache_write: Some(0),
        input_total: Some(2_000_000),
        output_total: Some(1_000_000),
        ..Default::default()
    }
}

fn book() -> PriceBook {
    let (_dir, s) = temp_storage("substitute-prices");
    s.ensure_seed_price_snapshot(ts("2026-10-05T12:00:00Z"))
        .unwrap();
    s.load_price_book().unwrap()
}

#[test]
fn explicit_preview_identity_uses_k27_only_for_current_reference() {
    let book = book();
    let options = EstimateOptions::default();
    for model in [
        "k28-agent-preview",
        "custom/k28-agent-preview",
        "kimi-k2.8-preview",
        "kimi-for-coding",
    ] {
        let e = usage(model);
        assert_eq!(
            reference_model_key_for(model, e.provider_id.as_deref(), e.occurred_at_ms),
            "kimi-k2.8-preview"
        );
        let narrowed = book.for_model(&e);
        let EventEstimate::Priced(amount) = narrowed.estimate(&e, &options, e.occurred_at_ms)
        else {
            panic!("{model}")
        };
        assert_eq!(amount.total_amount_minor, 514); // $0.95 input + $0.19 cache + $4 output.
        assert_eq!(amount.substitute_model.as_deref(), Some("kimi-k2.7-code"));
        assert!(amount.official_fallback);
        assert_eq!(
            narrowed.estimate_at_time(&e, &options),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
    }
    for model in [
        "k28-agent-unknown",
        "k29-agent-preview",
        "unknown/k28-agent-preview",
    ] {
        let e = usage(model);
        assert!(matches!(
            book.for_model(&e).estimate(&e, &options, e.occurred_at_ms),
            EventEstimate::Unpriced(_)
        ));
    }
    let mut old = usage("kimi-for-coding");
    old.occurred_at_ms = ts("2026-09-10T10:00:00Z");
    assert!(matches!(
        book.estimate(&old, &options, ts("2026-10-05T12:00:00Z")),
        EventEstimate::Unpriced(_)
    ));
}

#[test]
fn original_model_and_exact_route_prices_win_without_substitution() {
    let mut book = book();
    let e = usage("k28-agent-preview");
    let mut native = book
        .rows
        .iter()
        .find(|r| r.model == "kimi-k2.7-code" && r.currency == "USD")
        .unwrap()
        .clone();
    native.model = "kimi-k2.8-preview".into();
    native.price_id = "native-k28".into();
    native.output_per_mtok_hundredths = Some(100_000);
    book.rows.insert(0, native.clone());
    let options = EstimateOptions::default();
    let EventEstimate::Priced(amount) = book.for_model(&e).estimate(&e, &options, e.occurred_at_ms)
    else {
        panic!()
    };
    assert_eq!(amount.total_amount_minor, 1114);
    assert_eq!(amount.substitute_model, None);
    assert_eq!(amount.matched_price_ids, ["native-k28"]);

    native.provider_id = "custom".into();
    native.model = "k28-agent-preview".into();
    native.price_id = "exact-route".into();
    native.official_vendor = false;
    book.rows.insert(0, native);
    let mut options = options;
    options
        .provider_channels
        .insert("custom".into(), ("global".into(), "api".into()));
    let EventEstimate::Priced(amount) = book.for_model(&e).estimate(&e, &options, e.occurred_at_ms)
    else {
        panic!()
    };
    assert_eq!(amount.matched_price_ids, ["exact-route"]);
    assert!(!amount.official_fallback);
    assert_eq!(amount.substitute_model, None);
}

#[test]
fn substitute_keeps_official_channel_usage_and_missing_component_guards() {
    let source = book();
    let base = source
        .rows
        .iter()
        .find(|r| r.model == "kimi-k2.7-code" && r.currency == "USD")
        .unwrap()
        .clone();
    let e = usage("k28-agent-preview");
    let options = EstimateOptions::default();
    let mut spoof = base.clone();
    spoof.provider_id = "custom".into();
    let untrusted = PriceBook { rows: vec![spoof] };
    assert_eq!(
        untrusted.estimate(&e, &options, e.occurred_at_ms),
        EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
    );

    let mut conflicting = base.clone();
    conflicting.currency = "CNY".into();
    let ambiguous = PriceBook {
        rows: vec![base.clone(), conflicting],
    };
    assert_eq!(
        ambiguous.estimate(&e, &options, e.occurred_at_ms),
        EventEstimate::Unpriced(UnpricedReason::ChannelUnknown)
    );

    let mut native = base.clone();
    native.model = "kimi-k2.8-preview".into();
    native.output_per_mtok_hundredths = None;
    let partial = PriceBook {
        rows: vec![native, base],
    };
    let only_output = PricingEvent {
        input_uncached: None,
        input_total: None,
        input_cache_read: None,
        input_cache_write: None,
        ..e.clone()
    };
    assert_eq!(
        partial.estimate(&only_output, &options, e.occurred_at_ms),
        EventEstimate::Unpriced(UnpricedReason::NoPriceRow),
        "an existing native tariff must not be replaced to fill a missing component"
    );
    let empty = PricingEvent {
        output_total: None,
        ..only_output
    };
    assert_eq!(
        source.estimate(&empty, &options, e.occurred_at_ms),
        EventEstimate::Unpriced(UnpricedReason::NoKnownUsage)
    );
}

#[test]
fn archive_substitute_amount_and_provenance_reach_models_days_and_totals() {
    let (_dir, s) = temp_storage("substitute-archive");
    let now = ts("2026-10-05T12:00:00Z");
    s.ensure_seed_price_snapshot(now).unwrap();
    let mut e = with_tokens(evt("source", "k28", now - 1000), 2_000_000, 1_000_000);
    e.provider_id = Some("custom".into());
    e.model_raw = Some("k28-agent-preview".into());
    e.usage.input_uncached = Some(1_000_000);
    e.usage.input_cache_read = Some(1_000_000);
    e.usage.input_cache_write = Some(0);
    e.quality.input_uncached = FieldQuality::Reported;
    e.quality.input_cache_read = FieldQuality::Reported;
    e.quality.input_cache_write = FieldQuality::Reported;
    commit_batch(&s, &batch("source", "UTC", now, vec![e]), None).unwrap();
    let q = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-10-05".into(),
        last_day: "2026-10-05".into(),
        now_ms: now,
        options: EstimateOptions::default(),
        filters: Default::default(),
    };
    s.recompute_unsealed_cost_days("UTC", now, &q.options)
        .unwrap();
    let before = s.cost_summary(&q).unwrap();
    assert!(before
        .at_time
        .rows
        .iter()
        .all(|r| r.priced_event_count == 0));
    s.conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    s.conn().execute("DELETE FROM usage_events", []).unwrap();
    let summary = s.cost_summary(&q).unwrap();
    assert_eq!(
        summary.current_sim.rows[0].total_amount_minor,
        before.current_sim.rows[0].total_amount_minor
    );
    for row in [
        &summary.current_sim.rows[0],
        &summary.models[0].current_sim[0],
        &summary.daily_current[0].sums,
    ] {
        assert_eq!(row.total_amount_minor, 514);
        assert_eq!(row.substitute_models, ["kimi-k2.7-code"]);
        assert_eq!(row.aggregate_event_count, 1);
    }
    assert_eq!(summary.models[0].model, "k28-agent-preview");
    assert_eq!(summary.models[0].reference_models, ["kimi-k2.8-preview"]);
    assert_eq!(summary.models[0].unit_prices[0].model, "kimi-k2.7-code");
    assert_eq!(
        serde_json::to_value(&summary).unwrap(),
        serde_json::to_value(s.cost_summary(&q).unwrap()).unwrap()
    );
}
