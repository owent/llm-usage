mod common;
use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    calendar::{ymd, WeekStart},
    domain::{EventInput, FieldQuality},
    ingest::commit_batch,
    pricing::{EstimateOptions, EventEstimate, PricingEvent},
    query::{query_summary, Filters, Granularity, SummaryRequest},
    storage::pricing::CostSummaryRequest,
};

fn event(
    id: &str,
    model: &str,
    provider: &str,
    input: i64,
    cached: i64,
    output: i64,
) -> EventInput {
    let mut e = with_tokens(
        evt("source", id, ts("2026-10-05T10:00:00Z")),
        input + cached,
        output,
    );
    e.provider_id = Some(provider.into());
    e.model_raw = Some(model.into());
    e.usage.input_uncached = Some(input);
    e.usage.input_cache_read = Some(cached);
    e.usage.input_cache_write = Some(0);
    e.quality.input_uncached = FieldQuality::Reported;
    e.quality.input_cache_read = FieldQuality::Reported;
    e.quality.input_cache_write = FieldQuality::Reported;
    e
}

fn request() -> CostSummaryRequest {
    CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-10-05".into(),
        last_day: "2026-10-05".into(),
        now_ms: ts("2026-10-05T12:00:00Z"),
        options: EstimateOptions::default(),
        filters: Default::default(),
    }
}

#[test]
fn sealed_usage_is_priced_once_and_preserves_six_hundred_million_cached_tokens() {
    let (_dir, s) = temp_storage("price-archive");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let events = (0..600)
        .map(|i| event(&i.to_string(), "k3-256k", "kimi-code", 100, 1_000_000, 10))
        .collect();
    commit_batch(&s, &batch("source", "UTC", q.now_ms, events), None).unwrap();
    let before = s.cost_summary(&q).unwrap();
    assert_eq!(before.current_sim.rows[0].priced_tokens, 600_066_000);
    assert_eq!(before.current_sim.rows[0].total_amount_minor, 18_027);
    // Imported/sealed aggregate can coexist with details; it must replace them.
    s.conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    let sealed = s.cost_summary(&q).unwrap();
    assert_eq!(sealed.current_sim.rows[0].total_amount_minor, 18_027);
    assert_eq!(sealed.current_sim.rows[0].priced_tokens, 600_066_000);
    assert_eq!(sealed.current_sim.rows[0].priced_event_count, 600);
    assert_eq!(sealed.current_sim.rows[0].aggregate_event_count, 600);
    assert!(sealed.current_sim.detail_limited);
    s.conn().execute("DELETE FROM usage_events", []).unwrap();
    let archived = s.cost_summary(&q).unwrap();
    assert_eq!(archived.current_sim.rows[0].total_amount_minor, 18_027);
    assert_eq!(archived.models.len(), 1);
    assert!(!archived.models[0].unit_prices.is_empty());
    assert_eq!(archived.daily_current.len(), 1);
    let mut excluded = q.clone();
    excluded.filters.instances = Some(vec![]);
    assert!(s.cost_summary(&excluded).unwrap().models.is_empty());
}

#[test]
fn tiny_calls_accumulate_before_rounding_in_current_and_persisted_costs() {
    let (_dir, s) = temp_storage("price-precision");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let events = (0..1000)
        .map(|i| event(&i.to_string(), "k3-256k", "kimi-code", 100, 0, 0))
        .collect();
    commit_batch(&s, &batch("source", "UTC", q.now_ms, events), None).unwrap();
    s.recompute_unsealed_cost_days("UTC", q.now_ms, &q.options)
        .unwrap();
    let summary = s.cost_summary(&q).unwrap();
    assert_eq!(summary.current_sim.rows[0].total_amount_minor, 30);
    assert_eq!(summary.at_time.rows[0].total_amount_minor, 30);
    assert_eq!(summary.daily_current[0].sums.total_amount_minor, 30);
}

#[test]
fn archived_request_tiers_are_bounded_instead_of_using_aggregate_input_as_context() {
    let (_dir, s) = temp_storage("price-tiers");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let e = event("one", "gpt-6-sol", "openai", 1_000_000, 0, 1_000_000);
    commit_batch(&s, &batch("source", "UTC", q.now_ms, vec![e]), None).unwrap();
    s.conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    let result = s.cost_summary(&q).unwrap();
    let row = &result.current_sim.rows[0];
    assert_eq!(row.total_amount_minor, 1200);
    assert_eq!(row.upper_amount_minor, Some(1900));
    assert_eq!(result.models[0].unit_prices.len(), 2);
    assert_eq!(row.priced_event_count, 1);
}

#[test]
fn official_reference_handles_snapshot_ids_custom_routes_and_boundary_without_network() {
    let (_dir, s) = temp_storage("price-aliases");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let book = s.load_price_book().unwrap();
    for (provider, model) in [
        ("other", "gpt-5.5-2026-04-23"),
        ("kimi-code-owent", "k3-256k"),
        ("kimi-for-coding", "k3-256k"),
        ("", "Kimi For Coding - Backup/k3-256k"),
        ("", "kimi-code/k3-256k"),
        ("custom", "custom/k3-256k"),
        ("local-route", "GLM-5.3-Flash"),
        ("local-route", "glm-5.3"),
        ("", "gpt-6-sol"),
    ] {
        let e = PricingEvent {
            provider_id: Some(provider.into()),
            model_raw: Some(model.into()),
            occurred_at_ms: q.now_ms,
            input_total: Some(272_000),
            input_uncached: Some(272_000),
            input_cache_read: Some(0),
            input_cache_write: Some(0),
            output_total: Some(0),
            ..Default::default()
        };
        let EventEstimate::Priced(a) = book.for_model(&e).estimate(&e, &q.options, q.now_ms) else {
            panic!("{provider}/{model}")
        };
        assert!(a.official_fallback);
        if model == "gpt-6-sol" {
            assert_eq!(a.total_amount_minor, 54);
        }
    }
    for model in ["gpt-5.5-unknown", "gpt-5.5-2026-04-24", "unknown/k3-256k"] {
        let e = PricingEvent {
            model_raw: Some(model.into()),
            input_uncached: Some(100),
            occurred_at_ms: q.now_ms,
            ..Default::default()
        };
        assert!(matches!(
            book.for_model(&e).estimate(&e, &q.options, q.now_ms),
            EventEstimate::Unpriced(_)
        ));
    }
}

#[test]
fn model_rows_combine_spelling_and_provider_whitespace_across_agents() {
    let (_dir, s) = temp_storage("price-model-group");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let a = event("a", "GLM_5.3_FLASH", " Route ", 1_000_000, 0, 0);
    let mut b = event("b", "glm-5.3-flash", "route", 1_000_000, 0, 0);
    b.agent = "second-agent".into();
    commit_batch(&s, &batch("source", "UTC", q.now_ms, vec![a, b]), None).unwrap();
    let usage = query_summary(
        &s,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 10, 5),
            last_day: ymd(2026, 10, 5),
            today: ymd(2026, 10, 5),
            granularity: Granularity::Day,
            filters: Filters::default(),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(usage.model_breakdown.len(), 1);
    assert_eq!(usage.model_breakdown[0].sums.call_count, 2);
    let costs = s.cost_summary(&q).unwrap();
    assert_eq!(costs.models.len(), 1);
    assert_eq!(costs.models[0].current_sim[0].total_amount_minor, 30);
}

#[test]
fn archive_substitution_preserves_other_raw_model_partitions_and_hour_selection() {
    let (_dir, s) = temp_storage("archive-partitions");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let a = event("a", "GLM-5.3-Flash", "route", 1_000_000, 0, 0);
    let mut b = event("b", "glm-5.3-flash", "route", 2_000_000, 0, 0);
    b.occurred_at_ms = ts("2026-10-05T11:00:00Z");
    commit_batch(&s, &batch("source", "UTC", q.now_ms, vec![a, b]), None).unwrap();
    s.conn()
        .execute(
            "UPDATE daily_usage SET sealed=1 WHERE model_raw='GLM-5.3-Flash'",
            [],
        )
        .unwrap();
    let result = s.cost_summary(&q).unwrap();
    assert_eq!(result.current_sim.rows[0].total_amount_minor, 45);
    assert_eq!(result.current_sim.rows[0].priced_event_count, 2);
    let hour = s
        .cost_summary_selected(&q, Some(("2026-10-05 11:00", "2026-10-05 11:00")))
        .unwrap();
    assert_eq!(hour.current_sim.rows[0].total_amount_minor, 30);
    assert_eq!(hour.current_sim.rows[0].priced_event_count, 1);
}

#[test]
fn weekly_archive_reference_uses_usage_selection_without_inventing_daily_points() {
    use llm_usage_core::retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy};
    let (_dir, s) = temp_storage("archive-week");
    let mut q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    commit_batch(
        &s,
        &batch(
            "source",
            "UTC",
            q.now_ms,
            vec![event("a", "k3-256k", "kimi-code", 0, 1_000_000, 0)],
        ),
        None,
    )
    .unwrap();
    enforce_tiered_retention(
        &s,
        "UTC",
        ts("2027-01-05T12:00:00Z"),
        &TieredRetentionPolicy {
            events_days: 1,
            hourly_days: 1,
            daily_days: 1,
            ..Default::default()
        },
    )
    .unwrap();
    q.last_day = "2026-10-11".into();
    assert!(s.cost_summary(&q).unwrap().current_sim.rows.is_empty());
    let result = s
        .cost_summary_for_view(&q, None, Granularity::Week, WeekStart::Monday)
        .unwrap();
    assert_eq!(result.current_sim.rows[0].total_amount_minor, 30);
    assert_eq!(result.current_sim.rows[0].priced_event_count, 1);
    assert!(result.current_sim.detail_limited);
    assert!(result.daily_current.is_empty());
    assert!(s
        .cost_summary_for_view(&q, None, Granularity::Week, WeekStart::Sunday)
        .unwrap()
        .current_sim
        .rows
        .is_empty());
}

#[test]
fn archived_hour_preserves_known_coverage_without_deriving_missing_splits() {
    let (_dir, s) = temp_storage("archive-hour");
    let q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    let a = event("a", "k3-256k", "route", 1_000_000, 1_000_000, 0);
    let b = event("b", "glm-5.3", "route", 1_000_000, 0, 0);
    commit_batch(&s, &batch("source", "UTC", q.now_ms, vec![a, b]), None).unwrap();
    s.conn()
        .execute(
            "UPDATE daily_usage SET sealed=1 WHERE model_raw='k3-256k'",
            [],
        )
        .unwrap();
    s.conn()
        .execute("DELETE FROM usage_events WHERE model_raw='k3-256k'", [])
        .unwrap();
    let hour = s
        .cost_summary_selected(&q, Some(("2026-10-05 10:00", "2026-10-05 10:00")))
        .unwrap();
    let row = &hour
        .models
        .iter()
        .find(|m| m.model == "k3-256k")
        .unwrap()
        .current_sim[0];
    assert_eq!(
        row.total_amount_minor, 30,
        "only the saved cache split is priced"
    );
    assert_eq!(row.known_tokens, 2_000_000);
    assert_eq!(row.priced_tokens, 1_000_000);
    assert_eq!(row.partial_event_count, 1);
    assert_eq!(
        row.aggregate_event_count, 1,
        "an active sibling model must not hide this archive"
    );
}

#[test]
fn archive_crossing_a_dynamic_alias_change_is_ambiguous() {
    let (_dir, s) = temp_storage("archive-alias-change");
    let mut q = request();
    s.ensure_seed_price_snapshot(q.now_ms).unwrap();
    s.conn().execute(
        "INSERT INTO period_usage(tz_version, granularity, period_key, period_start_day, period_end_day,
            instance_id, agent, provider_id, model_raw, call_category, quality_bucket,
            event_count, call_count, output_known_sum, total_known_sum, conflict_count,
            active_days, materialized_at_ms, data_revision)
         VALUES('UTC','month','2026-09','2026-09-01','2026-09-30','source','test','kimi-code',
            'kimi-for-coding','interactive','complete',1,1,100,100,0,1,0,1)", []).unwrap();
    q.first_day = "2026-09-01".into();
    q.last_day = "2026-09-30".into();
    let result = s
        .cost_summary_for_view(&q, None, Granularity::Month, WeekStart::Monday)
        .unwrap();
    assert_eq!(
        result.current_sim.unpriced_reasons.get("model_ambiguous"),
        Some(&1)
    );
}

#[test]
fn newer_tariff_does_not_inherit_a_stale_long_context_tier() {
    use llm_usage_core::pricing::parse_snapshot_json;
    let (_dir, s) = temp_storage("coherent-tariff");
    let q = request();
    for (id, day, input, long) in [
        ("old", "2026-10-01", 80000, Some(200000)),
        ("new", "2026-10-05", 10000, None),
    ] {
        let base = serde_json::json!({"price_id":id,"provider_id":"route","model":"m",
            "region":"global","channel":"api","currency":"USD","effective_from":day,"input":input});
        let mut rows = vec![base.clone()];
        if let Some(rate) = long {
            let mut tier = base;
            tier["price_id"] = format!("{id}-long").into();
            tier["input"] = rate.into();
            tier["context_threshold_tokens"] = 272001.into();
            rows.push(tier);
        }
        let snapshot = parse_snapshot_json(
            &serde_json::json!({
                "format":"llm-usage-price-snapshot/1",
                "snapshot":{"id":id,"source_type":"manual","fetched_at":day},"rows":rows
            })
            .to_string(),
        )
        .unwrap();
        s.import_price_snapshot(&snapshot, q.now_ms).unwrap();
    }
    let mut options = q.options;
    options
        .provider_channels
        .insert("route".into(), ("global".into(), "api".into()));
    let event = PricingEvent {
        provider_id: Some("route".into()),
        model_raw: Some("m".into()),
        input_total: Some(300_000),
        input_uncached: Some(300_000),
        occurred_at_ms: q.now_ms,
        ..Default::default()
    };
    let EventEstimate::Priced(result) = s
        .load_price_book()
        .unwrap()
        .estimate(&event, &options, q.now_ms)
    else {
        panic!("new tariff must match");
    };
    assert_eq!(result.total_amount_minor, 30);
    assert_eq!(result.matched_price_ids, ["new"]);
}
