mod common;
use common::*;
use llm_usage_core::{
    ingest::commit_batch, model_names::*, pricing::*, storage::pricing::CostSummaryRequest,
};

const DATE: &str = "2026-11-16T12:00:00Z";
fn event(model: &str) -> PricingEvent {
    PricingEvent {
        model_raw: Some(model.into()),
        occurred_at_ms: ts(DATE),
        input_uncached: Some(1_000_000),
        input_total: Some(1_000_000),
        output_total: Some(1_000_000),
        ..Default::default()
    }
}
#[test]
fn november_offline_prices_and_future_intervals_preserve_model_identity() {
    let (_dir, storage) = temp_storage("nov-prices");
    storage.ensure_seed_price_snapshot(ts(DATE)).unwrap();
    let book = storage.load_price_book().unwrap();
    for (model, currency, amount) in [
        ("gpt-6.1-terra", "USD", 950),
        ("claude-fable-5-2", "USD", 4800),
        ("gemini-4-carbon", "USD", 1200),
        ("gemini-4-hydrogen", "USD", 900),
        ("qwen4-max", "CNY", 4000),
        ("qwen3.8-max", "CNY", 4800),
        ("qwen3.8-flash", "CNY", 350),
    ] {
        let e = event(model);
        let EventEstimate::Priced(result) =
            book.for_model(&e)
                .estimate(&e, &EstimateOptions::default(), ts(DATE))
        else {
            panic!("missing {model}")
        };
        assert_eq!(
            (result.currency.as_str(), result.total_amount_minor),
            (currency, amount),
            "{model}"
        );
        assert_eq!(e.model_raw.as_deref(), Some(model));
    }
    let e = event("gemini-4-carbon");
    for (date, amount) in [
        ("2026-11-30T23:59:59Z", 1200),
        ("2026-12-01T00:00:00Z", 2400),
    ] {
        let EventEstimate::Priced(r) =
            book.for_model(&e)
                .estimate(&e, &EstimateOptions::default(), ts(date))
        else {
            panic!("dated rate")
        };
        assert_eq!(r.total_amount_minor, amount);
    }
}
#[test]
fn preview_alias_is_known_without_inventing_a_price_or_aliasing_a_routing_key() {
    let models = offline_model_catalog();
    let preview = models.iter().find(|m| m.id == "qwen4-max-preview").unwrap();
    assert!(!preview.pricing_available);
    assert!(!preview.source_urls.is_empty());
    assert_eq!(
        reference_model_key("alibaba/qwen4.0-max-preview"),
        "qwen4-max-preview"
    );
    assert_eq!(reference_model_key("qmodel_latest"), "qmodel-latest");
    let (_dir, storage) = temp_storage("preview-price");
    storage.ensure_seed_price_snapshot(ts(DATE)).unwrap();
    let book = storage.load_price_book().unwrap();
    for model in ["qwen4-max-preview", "qwen4.0-max-preview"] {
        let e = event(model);
        let EventEstimate::Priced(amount) =
            book.for_model(&e)
                .estimate(&e, &EstimateOptions::default(), ts(DATE))
        else {
            panic!("{model}")
        };
        assert_eq!(amount.substitute_model.as_deref(), Some("qwen4-max"));
        assert_eq!(
            (amount.currency.as_str(), amount.total_amount_minor),
            ("CNY", 4000)
        );
        assert_eq!(
            book.estimate_at_time(&e, &EstimateOptions::default()),
            EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
        );
    }
    for model in ["qwen4-max-unknown", "qmodel_latest"] {
        let e = event(model);
        assert!(
            matches!(
                book.for_model(&e)
                    .estimate(&e, &EstimateOptions::default(), ts(DATE)),
                EventEstimate::Unpriced(_)
            ),
            "{model}"
        );
    }
}

#[test]
fn november_23_catalogue_keeps_glm_plan_unpriced_and_applies_official_api_tiers() {
    let glm = offline_model_catalog()
        .iter()
        .find(|m| m.id == "glm-5.4")
        .unwrap();
    assert!(!glm.pricing_available);
    let (_dir, storage) = temp_storage("nov-23-prices");
    let now = ts("2026-11-23T12:00:00Z");
    storage.ensure_seed_price_snapshot(now).unwrap();
    storage.ensure_seed_price_snapshot(now + 1).unwrap();
    let book = storage.load_price_book().unwrap();
    let mut glm_event = event("glm-5.4");
    glm_event.occurred_at_ms = now;
    assert_eq!(
        book.estimate_at_time(&glm_event, &EstimateOptions::default()),
        EventEstimate::Unpriced(UnpricedReason::NoPriceRow)
    );
    for (model, tokens, amount) in [
        ("gemini-4-neon", 1_000_000, 1800),
        ("gpt-6.2-astra", 272_000, 4218),
        ("gpt-6.2-astra", 272_001, 6435),
        ("gpt-6.2-sol", 1_000_000, 1900),
    ] {
        let mut e = event(model);
        e.occurred_at_ms = now;
        e.input_uncached = Some(tokens);
        e.input_total = Some(tokens);
        let EventEstimate::Priced(result) = book.estimate_at_time(&e, &EstimateOptions::default())
        else {
            panic!("missing {model}");
        };
        assert_eq!(result.total_amount_minor, amount, "{model}");
    }
    let mut hy4 = event("hy4-flash");
    hy4.provider_id = Some("tencent".into());
    let mut options = EstimateOptions::default();
    options
        .provider_channels
        .insert("tencent".into(), ("cn-guangzhou".into(), "api".into()));
    for (date, amount) in [("2026-11-29T12:00:00Z", 125), ("2026-12-01T00:00:00Z", 250)] {
        hy4.occurred_at_ms = ts(date);
        let EventEstimate::Priced(result) = book.estimate_at_time(&hy4, &options) else {
            panic!("missing Guangzhou rate on {date}");
        };
        assert_eq!(
            (result.currency.as_str(), result.total_amount_minor),
            ("CNY", amount)
        );
    }
    hy4.occurred_at_ms = ts("2026-11-30T08:00:00Z");
    assert!(matches!(
        book.estimate_at_time(&hy4, &options),
        EventEstimate::Unpriced(_)
    ));
}

#[test]
fn old_database_imports_november_snapshot_once_without_rewriting_estimates() {
    let (dir, storage) = temp_storage("nov-upgrade");
    let old = parse_snapshot_json(include_str!("../prices/seed-2026-10-24.json")).unwrap();
    storage.import_price_snapshot(&old, ts(DATE)).unwrap();
    let mut e = with_tokens(evt("native", "terra", ts(DATE)), 1_000_000, 1_000_000);
    e.model_raw = Some("gpt-6.1-terra".into());
    e.usage.input_uncached = Some(1_000_000);
    e.quality.input_uncached = llm_usage_core::domain::FieldQuality::Reported;
    commit_batch(&storage, &batch("native", "UTC", ts(DATE), vec![e]), None).unwrap();
    let q = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-11-16".into(),
        last_day: "2026-11-16".into(),
        now_ms: ts(DATE),
        options: EstimateOptions::default(),
        filters: Default::default(),
    };
    storage
        .recompute_unsealed_cost_days("UTC", ts(DATE), &q.options)
        .unwrap();
    let before = serde_json::to_value(storage.cost_summary(&q).unwrap().at_time).unwrap();
    drop(storage);
    let storage = llm_usage_core::storage::Storage::open(&dir.db_path()).unwrap();
    for now in [ts(DATE), ts(DATE) + 1] {
        storage.ensure_seed_price_snapshot(now).unwrap();
    }
    assert_eq!(
        storage
            .list_price_snapshots()
            .unwrap()
            .iter()
            .filter(|s| s.snapshot_id == "seed-2026-11-16")
            .count(),
        1
    );
    let after = storage.cost_summary(&q).unwrap();
    assert_eq!(serde_json::to_value(after.at_time).unwrap(), before);
    assert_eq!(after.current_sim.rows[0].total_amount_minor, 950);
}
