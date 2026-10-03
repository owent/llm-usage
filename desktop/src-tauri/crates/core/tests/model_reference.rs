mod common;
use common::{temp_storage, ts};
use llm_usage_core::pricing::{EstimateOptions, EventEstimate, PricingEvent, UnpricedReason};

fn event(model: &str) -> PricingEvent {
    PricingEvent {
        model_raw: Some(model.into()),
        occurred_at_ms: ts("2026-10-03T10:00:00Z"),
        input_uncached: Some(1_000_000),
        input_total: Some(2_000_000),
        input_cache_read: Some(1_000_000),
        input_cache_write: Some(0),
        output_total: Some(1_000_000),
        ..Default::default()
    }
}

#[test]
fn verified_hy4_alias_uses_original_cny_rates_and_preserves_unknown_models() {
    let (_dir, s) = temp_storage("hy4-reference");
    let now = ts("2026-10-03T12:00:00Z");
    s.ensure_seed_price_snapshot(now).unwrap();
    let book = s.load_price_book().unwrap();
    for name in [
        "hy4-preview-f",
        "Hy4 preview",
        "HY_4_PREVIEW",
        "tencent/hy4-preview",
    ] {
        let e = event(name);
        let amounts = book
            .for_model(&e)
            .estimate(&e, &EstimateOptions::default(), now);
        let EventEstimate::Priced(a) = amounts else {
            panic!("{name}: {amounts:?}")
        };
        assert_eq!(a.currency, "CNY");
        assert_eq!(a.total_amount_minor, 2430);
        assert!(a.official_fallback);
    }
    assert!(matches!(
        book.estimate(
            &event("hy4-preview-other"),
            &EstimateOptions::default(),
            now
        ),
        EventEstimate::Unpriced(_)
    ));
    assert!(matches!(
        book.estimate_at_time(
            &PricingEvent {
                occurred_at_ms: ts("2026-10-02T12:00:00Z"),
                ..event("hy4-preview-f")
            },
            &EstimateOptions::default()
        ),
        EventEstimate::Unpriced(_)
    ));
}

#[test]
fn spelling_collisions_require_an_exact_id_and_do_not_guess_by_row_order() {
    let (_dir, s) = temp_storage("model-collision");
    let e = event("claude opus 4.8");
    s.ensure_seed_price_snapshot(e.occurred_at_ms).unwrap();
    let mut book = s.load_price_book().unwrap().for_model(&e);
    let row = book
        .rows
        .iter()
        .find(|r| r.model == "claude-opus-4-8")
        .unwrap()
        .clone();
    book.rows = vec![row.clone(), row];
    book.rows[0].model = "claude_opus_4_8".into();
    book.rows[1].model = "claude opus 4 8".into();
    book.rows[1].price_id = "other-price".into();
    assert_eq!(
        book.estimate_at_time(&e, &EstimateOptions::default()),
        EventEstimate::Unpriced(UnpricedReason::ModelAmbiguous)
    );
    let exact = PricingEvent {
        model_raw: Some("claude_opus_4_8".into()),
        ..e
    };
    let EventEstimate::Priced(a) = book.estimate_at_time(&exact, &EstimateOptions::default())
    else {
        panic!("exact id must win")
    };
    assert_eq!(a.matched_price_ids, [book.rows[0].price_id.clone()]);
    book.rows[1].context_threshold_tokens = 200_000;
    let EventEstimate::Priced(a) = book.estimate_at_time(&exact, &EstimateOptions::default())
    else {
        panic!("exact model precedes context tiers")
    };
    assert_eq!(a.matched_price_ids, [book.rows[0].price_id.clone()]);
}

#[test]
fn dated_profile_resolution_survives_shared_price_book_caches_and_model_filters() {
    use common::{batch, evt, with_tokens};
    use llm_usage_core::{
        ingest::commit_batch,
        pricing::parse_snapshot_json,
        storage::pricing::{CostFilters, CostSummaryRequest},
    };
    let (_dir, s) = temp_storage("dated-profile");
    let now = ts("2026-10-03T12:00:00Z");
    // Synthetic rate exercises the dated alias; it is not a published K2.8 price.
    let snapshot=parse_snapshot_json(r#"{"format":"llm-usage-price-snapshot/1",
        "snapshot":{"id":"synthetic-kimi","source_type":"manual","source_urls":[],"fetched_at":"2026-10-03"},
        "rows":[{"price_id":"synthetic","provider_id":"moonshot","model":"kimi-k2.8-preview","region":"global","channel":"api","effective_from":"2026-09-01","currency":"USD","input":10000,"output":20000,"official_vendor":true}]}"#).unwrap();
    s.import_price_snapshot(&snapshot, now).unwrap();
    let mut events = Vec::new();
    for (id, time) in [
        ("before", "2026-09-10T12:00:00Z"),
        ("after", "2026-09-12T12:00:00Z"),
    ] {
        let mut e = with_tokens(evt("source", id, ts(time)), 1000, 1000);
        e.model_raw = Some("kimi_for_coding".into());
        e.provider_id = Some("custom".into());
        events.push(e);
    }
    commit_batch(&s, &batch("source", "UTC", now, events), None).unwrap();
    use llm_usage_core::{
        calendar::{ymd, WeekStart},
        query::{heatmap_cells, query_summary, Filters, Granularity, SummaryRequest},
    };
    let filters = Filters {
        models: vec!["kimi for coding".into()],
        ..Default::default()
    };
    for granularity in [Granularity::Hour, Granularity::Day] {
        let summary = query_summary(
            &s,
            &SummaryRequest {
                timezone: "UTC".into(),
                week_start: WeekStart::Monday,
                first_day: ymd(2026, 9, 10),
                last_day: ymd(2026, 9, 12),
                today: ymd(2026, 10, 3),
                granularity,
                filters: filters.clone(),
                retention_cutoff: None,
            },
        )
        .unwrap();
        assert_eq!(
            summary.totals.call_count, 2,
            "usage and price filters share the spelling key"
        );
    }
    let heatmap = heatmap_cells(&s, "UTC", ymd(2026, 9, 10), ymd(2026, 9, 12), &filters).unwrap();
    assert_eq!(heatmap.iter().map(|d| d.call_count).sum::<i64>(), 2);
    let query = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-09-10".into(),
        last_day: "2026-09-12".into(),
        now_ms: now,
        options: EstimateOptions::default(),
        filters: CostFilters {
            models: vec!["kimi for coding".into()],
            ..Default::default()
        },
    };
    let costs = s.cost_summary(&query).unwrap();
    assert_eq!(
        costs
            .current_sim
            .rows
            .iter()
            .map(|r| r.priced_event_count)
            .sum::<i64>(),
        1
    );
    assert_eq!(
        costs
            .current_sim
            .rows
            .iter()
            .map(|r| r.unpriced_event_count)
            .sum::<i64>(),
        1
    );
    assert_eq!(costs.models.len(), 1);
    assert_eq!(
        costs.models[0].unpriced_reasons.get("no_price_row"),
        Some(&1)
    );
    assert_eq!(
        costs.models[0].reference_models,
        ["kimi-for-coding", "kimi-k2.8-preview"]
    );
    // Past pricing also resolves the profile from its own event date.
    for day in ["2026-09-10", "2026-09-12"] {
        s.recompute_cost_day("UTC", day, now, &query.options)
            .unwrap();
    }
    let costs = s.cost_summary(&query).unwrap();
    assert_eq!(
        costs
            .at_time
            .rows
            .iter()
            .map(|r| r.priced_event_count)
            .sum::<i64>(),
        1
    );
}
