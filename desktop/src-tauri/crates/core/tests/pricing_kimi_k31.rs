mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    domain::FieldQuality,
    ingest::commit_batch,
    pricing::{parse_snapshot_json, EstimateOptions, EventEstimate, PriceBook, PricingEvent},
    storage::pricing::CostSummaryRequest,
};

const SNAPSHOT: &str = "seed-2026-10-24-kimi-k3.1";

fn now() -> i64 {
    ts("2026-10-24T12:00:00Z")
}

fn usage(model: &str, provider: Option<&str>) -> PricingEvent {
    PricingEvent {
        model_raw: Some(model.into()),
        provider_id: provider.map(str::to_string),
        occurred_at_ms: now(),
        input_uncached: Some(100_000),
        input_cache_read: Some(200_000),
        input_cache_write: Some(0),
        input_total: Some(300_000),
        output_total: Some(50_000),
        ..Default::default()
    }
}

fn book() -> PriceBook {
    let (_dir, storage) = temp_storage("kimi-k31-book");
    storage.ensure_seed_price_snapshot(now()).unwrap();
    storage.load_price_book().unwrap()
}

#[test]
fn kimi_k31_offline_aliases_select_same_model_rates_without_changing_identity() {
    let book = book();
    for (model, provider, amount) in [
        ("k3.1-pro", None, 74),
        ("k3.1-pro", Some("custom"), 74),
        ("kimi-code/k3.1-pro", None, 74),
        ("kimi-coding/k3.1-pro", None, 74),
        ("moonshotai/kimi-k3.1-pro", None, 74),
        ("custom/k3.1-pro", Some("custom"), 74),
        ("Kimi K3.1 Pro", None, 74),
        ("k3.1-pro-highspeed", None, 144),
        ("kimi-code/k3.1-pro-highspeed", None, 144),
        ("kimi-k3.1-pro-highspeed", Some("custom"), 144),
    ] {
        let event = usage(model, provider);
        let EventEstimate::Priced(a) =
            book.for_model(&event)
                .estimate(&event, &EstimateOptions::default(), now())
        else {
            panic!("expected official reference for {provider:?}/{model}")
        };
        assert_eq!(a.currency, "USD");
        assert_eq!(a.total_amount_minor, amount, "{model}");
        assert_eq!(a.priced_tokens, 350_000);
        assert_eq!(a.priced_tokens, a.known_tokens);
        assert!(a.official_fallback);
        assert!(a.substitute_model.is_none());
        assert!(a.matched_price_ids[0].contains("k3.1-pro"));
        assert_eq!(event.model_raw.as_deref(), Some(model));
        assert_eq!(event.provider_id.as_deref(), provider);
    }
    for model in [
        "k3.1",
        "k3.1-pro-unknown",
        "k3.1-pro-256k",
        "k3.1-pro-highspeed-unknown",
        "unknown/k3.1-pro",
        "kimi-k3.2-pro",
    ] {
        let event = usage(model, Some("custom"));
        assert!(
            matches!(
                book.for_model(&event)
                    .estimate(&event, &EstimateOptions::default(), now()),
                EventEstimate::Unpriced(_)
            ),
            "{model}"
        );
    }
}

#[test]
fn kimi_k31_currencies_cache_ttls_and_partial_usage_use_official_components() {
    let book = book();
    for (model, region, currency, input, read, output, write5, write60) in [
        ("k3.1-pro", "global", "USD", 20, 4, 50, 20, 40),
        ("k3.1-pro", "cn", "CNY", 130, 26, 340, 130, 260),
        ("k3.1-pro-highspeed", "global", "USD", 40, 4, 100, 40, 80),
        ("k3.1-pro-highspeed", "cn", "CNY", 260, 26, 680, 260, 520),
    ] {
        let mut event = usage(model, Some("moonshot"));
        event.input_cache_write = Some(100_000);
        event.input_total = Some(400_000);
        for ttl in [None, Some(5), Some(60)] {
            let mut options = EstimateOptions::default();
            options
                .provider_channels
                .insert("moonshot".into(), (region.into(), "api".into()));
            if let Some(ttl) = ttl {
                options.cache_ttl_minutes.insert("moonshot".into(), ttl);
            }
            let EventEstimate::Priced(a) = book.estimate_at_time(&event, &options) else {
                panic!("{region}/{model}/{ttl:?}")
            };
            let write = ttl.map(|ttl| if ttl == 5 { write5 } else { write60 });
            assert_eq!(a.currency, currency);
            assert_eq!(a.input_amount_minor, Some(input));
            assert_eq!(a.cache_read_amount_minor, Some(read));
            assert_eq!(a.output_amount_minor, Some(output));
            assert_eq!(a.cache_write_amount_minor, write);
            assert_eq!(
                a.total_amount_minor,
                input + read + output + write.unwrap_or(0)
            );
            assert_eq!(
                a.priced_tokens,
                if ttl.is_some() { 450_000 } else { 350_000 }
            );
            assert!(!a.official_fallback);
            assert_eq!(a.ttl_defaulted, ttl.is_some());
        }
    }
    let mut event = usage("k3.1-pro", None);
    event.input_uncached = None;
    event.input_total = None;
    let EventEstimate::Priced(a) = book.estimate_at_time(&event, &EstimateOptions::default())
    else {
        panic!("known cached input and output remain priceable")
    };
    assert_eq!(a.input_amount_minor, None);
    assert_eq!(a.total_amount_minor, 54);
    assert!(a.has_unknown_components);
}

#[test]
fn kimi_k31_exact_prices_win_and_current_prices_do_not_backdate() {
    let mut book = book();
    let options = EstimateOptions::default();
    let mut event = usage("k3.1-pro", Some("custom"));
    event.occurred_at_ms = ts("2026-10-23T23:59:59.999Z");
    assert!(matches!(
        book.estimate_at_time(&event, &options),
        EventEstimate::Unpriced(_)
    ));
    assert!(matches!(
        book.estimate(&event, &options, now()),
        EventEstimate::Priced(_)
    ));
    event.occurred_at_ms += 1;
    assert!(matches!(
        book.estimate_at_time(&event, &options),
        EventEstimate::Priced(_)
    ));

    let mut exact = book
        .rows
        .iter()
        .find(|row| row.model == "kimi-k3.1-pro" && row.currency == "USD")
        .unwrap()
        .clone();
    exact.price_id = "configured-k31".into();
    exact.model = "k3.1-pro".into();
    exact.provider_id = "custom".into();
    exact.input_per_mtok_hundredths = Some(10000);
    book.rows.push(exact);
    let mut options = options;
    options
        .provider_channels
        .insert("custom".into(), ("global".into(), "api".into()));
    let EventEstimate::Priced(a) = book.for_model(&event).estimate_at_time(&event, &options) else {
        panic!("configured exact price must win")
    };
    assert_eq!(a.matched_price_ids, ["configured-k31"]);
    assert_eq!(a.total_amount_minor, 64);
    assert!(!a.official_fallback);
}

#[test]
fn kimi_k31_old_database_loads_new_seed_and_prices_details_and_archives_without_rewriting_history()
{
    let (dir, storage) = temp_storage("kimi-k31-upgrade");
    for json in [
        include_str!("../prices/seed-2026-09-25.json"),
        include_str!("../prices/seed-2026-10-02.json"),
        include_str!("../prices/seed-2026-10-03.json"),
        include_str!("../prices/seed-2026-10-05.json"),
        include_str!("../prices/seed-2026-10-24.json"),
    ] {
        storage
            .import_price_snapshot(&parse_snapshot_json(json).unwrap(), now())
            .unwrap();
    }
    let event_time = ts("2026-10-23T10:00:00Z");
    let mut event = with_tokens(evt("source", "call", event_time), 300_000, 50_000);
    event.model_raw = Some("k3.1-pro".into());
    event.provider_id = Some("custom".into());
    event.usage.input_uncached = Some(100_000);
    event.usage.input_cache_read = Some(200_000);
    event.usage.input_cache_write = Some(0);
    event.quality.input_uncached = FieldQuality::Reported;
    event.quality.input_cache_read = FieldQuality::Reported;
    event.quality.input_cache_write = FieldQuality::Reported;
    commit_batch(&storage, &batch("source", "UTC", now(), vec![event]), None).unwrap();
    let q = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-10-23".into(),
        last_day: "2026-10-23".into(),
        now_ms: now(),
        options: EstimateOptions::default(),
        filters: Default::default(),
    };
    storage
        .recompute_unsealed_cost_days("UTC", now(), &q.options)
        .unwrap();
    let before = storage.cost_summary(&q).unwrap();
    assert_eq!(before.current_sim.unpriced_reasons.values().sum::<i64>(), 1);
    let old_snapshots = serde_json::to_value(storage.list_price_snapshots().unwrap()).unwrap();
    drop(storage);
    let storage = llm_usage_core::storage::Storage::open(&dir.db_path()).unwrap();
    storage.ensure_seed_price_snapshot(now()).unwrap();
    storage.ensure_seed_price_snapshot(now() + 1).unwrap();
    let snapshots = storage.list_price_snapshots().unwrap();
    assert_eq!(
        snapshots
            .iter()
            .filter(|s| s.snapshot_id == SNAPSHOT)
            .count(),
        1
    );
    assert_eq!(
        snapshots
            .iter()
            .find(|s| s.snapshot_id == SNAPSHOT)
            .unwrap()
            .row_count,
        4
    );
    let retained: Vec<_> = snapshots
        .into_iter()
        .filter(|s| s.snapshot_id != SNAPSHOT)
        .collect();
    assert_eq!(serde_json::to_value(retained).unwrap(), old_snapshots);
    let after = storage.cost_summary(&q).unwrap();
    assert_eq!(
        serde_json::to_value(&after.at_time).unwrap(),
        serde_json::to_value(&before.at_time).unwrap()
    );
    assert!(after.current_sim.unpriced_reasons.is_empty());
    assert_eq!(after.current_sim.rows[0].total_amount_minor, 74);
    assert_eq!(after.current_sim.rows[0].fallback_event_count, 1);
    assert_eq!(after.models[0].model, "k3.1-pro");
    assert_eq!(after.models[0].provider, "custom");
    assert_eq!(after.models[0].unit_prices.len(), 1);
    assert_eq!(after.daily_current[0].sums.total_amount_minor, 74);
    let hour = storage
        .cost_summary_selected(&q, Some(("2026-10-23 10:00", "2026-10-23 10:00")))
        .unwrap();
    assert_eq!(hour.current_sim.rows[0].total_amount_minor, 74);
    storage
        .conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    assert_eq!(
        storage.cost_summary(&q).unwrap().current_sim.rows[0].total_amount_minor,
        74
    );
    storage
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    let archive = storage.cost_summary(&q).unwrap();
    assert_eq!(archive.current_sim.rows[0].total_amount_minor, 74);
    assert_eq!(archive.current_sim.rows[0].priced_tokens, 350_000);
    assert_eq!(
        serde_json::to_value(&archive.at_time).unwrap(),
        serde_json::to_value(&before.at_time).unwrap()
    );
}
