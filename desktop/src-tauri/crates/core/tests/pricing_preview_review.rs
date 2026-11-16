mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    domain::FieldQuality,
    ingest::commit_batch,
    model_names::reference_model_key_for,
    pricing::{EstimateOptions, EventEstimate, PriceBook, PriceRow, PricingEvent, UnpricedReason},
    storage::pricing::CostSummaryRequest,
};

fn now() -> i64 {
    ts("2026-11-16T12:00:00Z")
}
fn usage(model: &str, provider: &str) -> PricingEvent {
    PricingEvent {
        model_raw: Some(model.into()),
        provider_id: Some(provider.into()),
        occurred_at_ms: ts("2026-10-01T10:00:00Z"),
        input_uncached: Some(100_000),
        input_cache_read: Some(100_000),
        input_cache_write: Some(0),
        input_total: Some(200_000),
        output_total: Some(100_000),
        ..Default::default()
    }
}
fn book() -> PriceBook {
    let (_dir, s) = temp_storage("preview-review-book");
    s.ensure_seed_price_snapshot(now()).unwrap();
    s.load_price_book().unwrap()
}
fn priced(
    book: &PriceBook,
    e: &PricingEvent,
    options: &EstimateOptions,
) -> llm_usage_core::pricing::EventEstimateAmounts {
    let EventEstimate::Priced(a) = book.for_model(e).estimate(e, options, now()) else {
        panic!("{e:?}")
    };
    *a
}
fn row(model: &str) -> PriceRow {
    book()
        .rows
        .into_iter()
        .find(|r| r.model == model && r.context_threshold_tokens == 0)
        .unwrap()
}

#[test]
fn preview_normalization_is_terminal_and_does_not_chain_or_invent_a_model() {
    let book = book();
    let options = EstimateOptions::default();
    for model in [
        "Qwen4_Max_Preview",
        "alibaba/QWEN4-MAX-PREVIEW",
        "qwen4.0-max-preview",
    ] {
        let e = usage(model, "route");
        let a = priced(&book, &e, &options);
        assert_eq!(a.substitute_model.as_deref(), Some("qwen4-max"));
        assert!(a.official_fallback);
        assert_eq!(a.currency, "CNY");
        assert_eq!(e.model_raw.as_deref(), Some(model));
        assert!(matches!(
            book.estimate_at_time(&e, &options),
            EventEstimate::Unpriced(_)
        ));
    }
    for model in [
        "qwen4-max-preview-v2",
        "qwen4-max-preview-preview",
        "unknown/qwen4-max-preview",
        "qwen9-max-preview",
        "qmodel_latest",
    ] {
        assert!(
            matches!(
                book.for_model(&usage(model, "route")).estimate(
                    &usage(model, "route"),
                    &options,
                    now()
                ),
                EventEstimate::Unpriced(_)
            ),
            "{model}"
        );
    }
}

#[test]
fn preview_exact_tariff_keeps_partial_components_and_tier_ambiguity() {
    let base = row("gpt-5.6-luna");
    let e = usage("gpt-5.6-luna-preview", "openai");
    let mut native = base.clone();
    native.model = "gpt-5.6-luna-preview".into();
    native.price_id = "native-preview".into();
    native.output_per_mtok_hundredths = None;
    let mut book = PriceBook {
        rows: vec![native.clone(), base],
    };
    let options = EstimateOptions::default();
    let a = priced(&book, &e, &options);
    assert_eq!(a.substitute_model, None);
    assert_eq!(a.output_amount_minor, None);
    assert_eq!(a.matched_price_ids, ["native-preview"]);
    native.context_threshold_tokens = 272001;
    native.price_id = "native-long".into();
    book.rows.push(native);
    let mut unknown_context = e;
    unknown_context.input_total = None;
    unknown_context.input_uncached = None;
    assert_eq!(
        book.estimate(&unknown_context, &options, now()),
        EventEstimate::Unpriced(UnpricedReason::TierAmbiguous)
    );
}

#[test]
fn preview_manual_channel_and_official_currency_guards_remain_independent() {
    let mut base = row("gpt-5.6-luna");
    base.model = "private-model".into();
    base.provider_id = "private".into();
    base.official_vendor = false;
    let e = usage("private-model-preview", "private");
    let mut options = EstimateOptions::default();
    let book = PriceBook {
        rows: vec![base.clone()],
    };
    assert!(matches!(
        book.estimate(&e, &options, now()),
        EventEstimate::Unpriced(_)
    ));
    options
        .provider_channels
        .insert("private".into(), ("global".into(), "api".into()));
    let a = priced(&book, &e, &options);
    assert_eq!(a.substitute_model.as_deref(), Some("private-model"));
    assert!(!a.official_fallback);
    base.provider_id = "another".into();
    base.official_vendor = true;
    assert!(matches!(
        PriceBook { rows: vec![base] }.estimate(&e, &options, now()),
        EventEstimate::Unpriced(_)
    ));

    let base = row("gpt-5.6-luna");
    let mut conflict = base.clone();
    conflict.currency = "CNY".into();
    let book = PriceBook {
        rows: vec![base, conflict],
    };
    assert_eq!(
        book.estimate(
            &usage("gpt-5.6-luna-preview", "route"),
            &EstimateOptions::default(),
            now()
        ),
        EventEstimate::Unpriced(UnpricedReason::ChannelUnknown)
    );
}

#[test]
fn suffixless_kimi_precedes_legacy_substitute_only_when_its_interval_applies() {
    let mut book = book();
    let mut base = row("kimi-k2.7-code");
    base.model = "kimi-k2.8".into();
    base.price_id = "suffixless-k28".into();
    book.rows.insert(0, base);
    let e = usage("k28-agent-preview", "custom");
    let options = EstimateOptions::default();
    assert_eq!(
        priced(&book, &e, &options).substitute_model.as_deref(),
        Some("kimi-k2.8")
    );
    book.rows[0].effective_to_ms = Some(now());
    assert_eq!(
        priced(&book, &e, &options).substitute_model.as_deref(),
        Some("kimi-k2.7-code")
    );
    book.rows[0].effective_to_ms = None;
    book.rows[0].effective_from_ms = now() + 1;
    assert_eq!(
        priced(&book, &e, &options).substitute_model.as_deref(),
        Some("kimi-k2.7-code")
    );
}

#[test]
fn codex_current_api_counterpart_preserves_opaque_identity_and_unknowns() {
    let book = book();
    let options = EstimateOptions::default();
    for model in ["codex-auto-review", "OpenAI/Codex_Auto_Review"] {
        let e = usage(model, " OpenAI ");
        assert_eq!(
            reference_model_key_for(model, e.provider_id.as_deref(), e.occurred_at_ms),
            "codex-auto-review"
        );
        let a = priced(&book, &e, &options);
        assert_eq!(a.substitute_model.as_deref(), Some("gpt-5.6-luna"));
        assert_eq!(a.total_amount_minor, 14);
        assert!(a.official_fallback);
        assert!(matches!(
            book.estimate_at_time(&e, &options),
            EventEstimate::Unpriced(_)
        ));
    }
    for (model, provider) in [
        ("codex-auto-review", ""),
        ("codex-auto-review", "custom"),
        ("custom/codex-auto-review", "openai"),
        ("codex-auto-review-v2", "openai"),
    ] {
        assert!(matches!(
            book.for_model(&usage(model, provider)).estimate(
                &usage(model, provider),
                &options,
                now()
            ),
            EventEstimate::Unpriced(_)
        ));
    }
    let e = PricingEvent {
        model_raw: Some("codex-auto-review".into()),
        provider_id: Some("openai".into()),
        ..Default::default()
    };
    assert_eq!(
        book.estimate(&e, &options, now()),
        EventEstimate::Unpriced(UnpricedReason::NoKnownUsage)
    );
    let mut exact = row("gpt-5.6-luna");
    exact.model = "codex-auto-review".into();
    exact.official_vendor = false;
    let mut options = options;
    options
        .provider_channels
        .insert("openai".into(), ("global".into(), "api".into()));
    assert_eq!(
        priced(
            &PriceBook { rows: vec![exact] },
            &usage("codex-auto-review", "openai"),
            &options
        )
        .substitute_model,
        None
    );
}

#[test]
fn luna_context_boundary_and_cache_uncertainty_use_versioned_rates() {
    let book = book();
    let options = EstimateOptions::default();
    for (input, rate, output_rate, cache) in [
        (272000, 2000, 12000, Some(200)),
        (272001, 4000, 18000, None),
    ] {
        let mut e = usage("codex-auto-review", "openai");
        e.input_total = Some(input);
        e.input_uncached = Some(input - 100000);
        let a = priced(&book, &e, &options);
        let r = book
            .rows
            .iter()
            .find(|r| a.matched_price_ids.contains(&r.price_id))
            .unwrap();
        assert_eq!(r.input_per_mtok_hundredths, Some(rate));
        assert_eq!(r.output_per_mtok_hundredths, Some(output_rate));
        assert_eq!(r.cache_read_per_mtok_hundredths, cache);
        assert_eq!(a.cache_read_amount_minor.is_some(), cache.is_some());
    }
    let mut e = usage("codex-auto-review", "openai");
    e.input_cache_write = Some(10000);
    e.input_total = Some(210000);
    let mut options = options;
    options.cache_ttl_minutes.insert("openai".into(), 5);
    let a = priced(&book, &e, &options);
    assert_eq!(a.cache_write_amount_minor, None);
}

#[test]
fn old_database_provider_cache_archives_and_saved_costs_preserve_provenance() {
    let (dir, s) = temp_storage("preview-review-upgrade");
    let mut events = Vec::new();
    for (id, model, provider) in [
        ("a", "codex-auto-review", "custom"),
        ("b", "codex-auto-review", "openai"),
        ("c", "qwen4-max-preview", "alibaba"),
    ] {
        let mut e = with_tokens(evt("native", id, now() - 1000), 200000, 100000);
        e.model_raw = Some(model.into());
        e.provider_id = Some(provider.into());
        e.usage.input_uncached = Some(100000);
        e.quality.input_uncached = FieldQuality::Reported;
        e.usage.input_cache_read = Some(100000);
        e.quality.input_cache_read = FieldQuality::Reported;
        e.usage.input_cache_write = Some(0);
        e.quality.input_cache_write = FieldQuality::Reported;
        events.push(e);
    }
    commit_batch(&s, &batch("native", "UTC", now(), events), None).unwrap();
    let q = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-11-16".into(),
        last_day: "2026-11-16".into(),
        now_ms: now(),
        options: EstimateOptions::default(),
        filters: Default::default(),
    };
    s.recompute_unsealed_cost_days("UTC", now(), &q.options)
        .unwrap();
    let historical = serde_json::to_value(s.cost_summary(&q).unwrap().at_time).unwrap();
    drop(s);
    let s = llm_usage_core::storage::Storage::open(&dir.db_path()).unwrap();
    for _ in 0..2 {
        s.ensure_seed_price_snapshot(now()).unwrap();
    }
    assert_eq!(
        s.list_price_snapshots()
            .unwrap()
            .iter()
            .filter(|p| p.snapshot_id == "seed-2026-11-16-gpt-5.6-luna")
            .count(),
        1
    );
    let before = s.cost_summary(&q).unwrap();
    assert_eq!(serde_json::to_value(&before.at_time).unwrap(), historical);
    let codex = before
        .models
        .iter()
        .find(|m| m.provider == "openai")
        .unwrap();
    assert_eq!(codex.model, "codex-auto-review");
    assert_eq!(codex.reference_models, ["codex-auto-review"]);
    assert_eq!(codex.current_sim[0].substitute_models, ["gpt-5.6-luna"]);
    assert_eq!(codex.unit_prices[0].model, "gpt-5.6-luna");
    assert!(before
        .models
        .iter()
        .find(|m| m.provider == "custom")
        .unwrap()
        .current_sim
        .iter()
        .all(|r| r.priced_event_count == 0 && r.substitute_models.is_empty()));
    s.conn()
        .execute_batch(
            "BEGIN; UPDATE daily_usage SET sealed=1; DELETE FROM usage_events; ROLLBACK;",
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(s.cost_summary(&q).unwrap()).unwrap()
    );
    s.conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    let coexisting = s.cost_summary(&q).unwrap();
    s.conn().execute("DELETE FROM usage_events", []).unwrap();
    let archived = s.cost_summary(&q).unwrap();
    assert_eq!(
        serde_json::to_value(&coexisting).unwrap(),
        serde_json::to_value(&archived).unwrap()
    );
    for currency in ["USD", "CNY"] {
        let row = archived
            .current_sim
            .rows
            .iter()
            .find(|r| r.currency == currency)
            .unwrap();
        assert_eq!(row.priced_event_count, 1);
        assert_eq!(row.aggregate_event_count, 1);
        assert_eq!(row.substitute_models.len(), 1);
        assert_eq!(
            archived
                .daily_current
                .iter()
                .find(|d| d.sums.currency == currency)
                .unwrap()
                .sums
                .substitute_models,
            row.substitute_models
        );
    }
    assert_eq!(
        serde_json::to_value(&archived).unwrap(),
        serde_json::to_value(s.cost_summary(&q).unwrap()).unwrap()
    );
}

#[test]
fn preview_and_review_provenance_survives_hour_week_and_month_retention() {
    use llm_usage_core::{
        calendar::WeekStart,
        query::Granularity,
        retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy},
    };
    for (granularity, weekly_days, first, last) in [
        (Granularity::Week, 1095, "2026-11-16", "2026-11-22"),
        (Granularity::Month, 1, "2026-11-01", "2026-11-30"),
    ] {
        let (_dir, s) = temp_storage("preview-review-periods");
        s.ensure_seed_price_snapshot(now()).unwrap();
        let events = [
            ("a", "codex-auto-review", "openai"),
            ("b", "qwen4-max-preview", "alibaba"),
        ]
        .into_iter()
        .map(|(id, model, provider)| {
            let mut e = with_tokens(evt("native", id, now() - 1000), 0, 100000);
            e.model_raw = Some(model.into());
            e.provider_id = Some(provider.into());
            e.usage.input_uncached = Some(0);
            e.quality.input_uncached = FieldQuality::Reported;
            e.usage.input_cache_read = Some(0);
            e.quality.input_cache_read = FieldQuality::Reported;
            e.usage.input_cache_write = Some(0);
            e.quality.input_cache_write = FieldQuality::Reported;
            e
        })
        .collect();
        commit_batch(&s, &batch("native", "UTC", now(), events), None).unwrap();
        let q = CostSummaryRequest {
            timezone: "UTC".into(),
            first_day: first.into(),
            last_day: last.into(),
            now_ms: now(),
            options: EstimateOptions::default(),
            filters: Default::default(),
        };
        let hour = s
            .cost_summary_selected(&q, Some(("2026-11-16 11:00", "2026-11-16 11:00")))
            .unwrap();
        assert_eq!(hour.current_sim.rows.len(), 2);
        assert!(s
            .cost_summary_selected(&q, Some(("2026-11-16 10:00", "2026-11-16 10:00")))
            .unwrap()
            .current_sim
            .rows
            .is_empty());
        enforce_tiered_retention(
            &s,
            "UTC",
            ts("2027-01-05T12:00:00Z"),
            &TieredRetentionPolicy {
                events_days: 1,
                hourly_days: 1,
                daily_days: 1,
                weekly_days,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.cost_summary(&q).unwrap().current_sim.rows.is_empty());
        let result = s
            .cost_summary_for_view(&q, None, granularity, WeekStart::Monday)
            .unwrap();
        assert!(result.daily_current.is_empty());
        for (currency, substitute) in [("USD", "gpt-5.6-luna"), ("CNY", "qwen4-max")] {
            let row = result
                .current_sim
                .rows
                .iter()
                .find(|r| r.currency == currency)
                .unwrap();
            assert_eq!(row.priced_event_count, 1);
            assert_eq!(row.aggregate_event_count, 1);
            assert_eq!(row.substitute_models, [substitute]);
            assert!(result
                .models
                .iter()
                .any(|m| m.unit_prices.iter().any(|r| r.model == substitute)));
        }
    }
}
