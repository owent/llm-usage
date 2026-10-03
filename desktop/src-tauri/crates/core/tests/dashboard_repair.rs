//! Regression evidence for carrier selection, unchanged checkpoints and API references.
mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::adapters::otel::OtelAdapter;
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::{Lifecycle, RecordKind};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::pricing::{
    parse_snapshot_json, EstimateOptions, EventEstimate, PriceBook, PricingEvent, UnpricedReason,
};
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::storage::Storage;
use serde_json::{json, Value};
use std::path::Path;

fn scan(storage: &Storage, file: &Path, now: i64) -> i64 {
    let reports = run_adapter_scan(
        storage,
        &OtelAdapter::new(),
        &DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![file.to_path_buf()],
        },
        &RunConfig {
            timezone: "UTC".into(),
            now_ms: now,
            limits: ScanLimits::default(),
            trigger: TriggerKind::Manual,
            run_id_prefix: format!("repair-{now}"),
            origin_host_id: Some("local-host".into()),
        },
    )
    .unwrap();
    assert!(!reports.is_empty());
    assert!(reports.iter().all(|r| r.error.is_none()));
    reports
        .iter()
        .filter_map(|r| r.outcome.as_ref())
        .map(|o| {
            assert_eq!((o.errors, o.conflicts), (0, 0));
            o.added
        })
        .sum()
}
fn span(id: &str) -> Value {
    json!({"name":"chat claude-opus-4.8","kind":2,"spanId":id,"traceId":"trace",
        "startTime":[1790899200,0],"resource":{"attributes":{"service.name":"copilot-chat"}},
        "attributes":{"copilot_chat.chat_session_id":"session","gen_ai.request.model":"claude-opus-4.8",
            "gen_ai.usage.input_tokens":100,"gen_ai.usage.output_tokens":20,
            "gen_ai.usage.cache_read.input_tokens":50,"gen_ai.usage.cache_creation.input_tokens":10,
            "gen_ai.usage.reasoning_tokens":5}})
}
fn write(file: &Path, values: &[Value]) {
    std::fs::write(
        file,
        values.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
}
fn summary(
    storage: &Storage,
    granularity: Granularity,
    filters: Filters,
) -> llm_usage_core::query::Summary {
    query_summary(
        storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 10, 2),
            last_day: ymd(2026, 10, 2),
            granularity,
            filters,
            today: ymd(2026, 10, 2),
            retention_cutoff: None,
        },
    )
    .unwrap()
}

#[test]
fn native_file_counts_decorated_client_calls_and_replays_consumed_legacy_cursor() {
    let (dir, storage) = temp_storage("otel-policy-replay");
    let file = dir.path().join("events.jsonl");
    let mut server = span("server");
    server["kind"] = json!(1);
    let mut root = span("summary");
    root["name"] = json!("invoke_agent");
    let mut failed = span("failed");
    failed["attributes"] = json!({"copilot_chat.chat_session_id":"session"});
    failed["status"] = json!({"code":2});
    write(
        &file,
        &[
            json!({"body":"log"}),
            span("call"),
            server,
            root,
            failed,
            json!({"metrics":[]}),
        ],
    );
    let now = ts("2026-10-02T12:00:00Z");
    assert_eq!(scan(&storage, &file, now), 2);
    let sums = summary(&storage, Granularity::Hour, Filters::default()).totals;
    assert_eq!(
        (
            sums.call_count,
            sums.input_total_known,
            sums.output_total_known,
            sums.total_tokens_known
        ),
        (2, Some(100), Some(20), Some(120))
    );
    assert_eq!(sums.uncached_known, Some(40));
    assert_eq!(
        sums.total_unknown_count, 0,
        "a failed call without usage is not an unknown token observation"
    );
    // Old parser consumed the identical file but dropped decorated spans.
    storage.conn().execute_batch("DELETE FROM usage_events; DELETE FROM daily_usage; DELETE FROM hourly_usage;
        UPDATE ingestion_checkpoints SET parse_context=json_remove(parse_context,'$.policy_version');").unwrap();
    assert_eq!(scan(&storage, &file, now + 1), 2);
    let revision = storage.data_revision().unwrap();
    assert_eq!(scan(&storage, &file, now + 2), 0);
    assert_eq!(storage.data_revision().unwrap(), revision);
}

fn native(storage: &Storage, revision: i64) {
    let mut e = with_tokens(evt("native", "turn", ts("2026-10-02T08:00:00Z")), 300, 40);
    e.agent = "vscode-copilot-chat".into();
    e.parser_version = "vscode-copilot-chat-session-log-2".into();
    e.session_id = Some("session".into());
    e.model_raw = Some("claude-opus-4-8".into());
    e.record_kind = RecordKind::UsageObservation;
    e.lifecycle = Lifecycle::Corrected;
    e.source_revision = Some(revision);
    commit_batch(
        storage,
        &batch("native", "UTC", ts("2026-10-02T12:00:00Z"), vec![e]),
        None,
    )
    .unwrap();
}
fn owner(storage: &Storage, host: &str, user: &str) {
    storage
        .conn()
        .execute(
            "INSERT OR IGNORE INTO users(user_id,name,created_at_ms) VALUES(?1,?1,0)",
            [user],
        )
        .unwrap();
    storage.conn().execute("INSERT INTO source_instances(instance_id,agent,format,location_hint,locality_basis,attribution_status,enabled,user_id,health,origin_host_id,created_at_ms,updated_at_ms)
        VALUES('native','vscode-copilot-chat','test','test','local_filesystem','verified',1,?1,'ok',?2,0,0)",rusqlite::params![user,host]).unwrap();
}
#[test]
fn carrier_selection_preserves_records_revisions_and_native_rescan() {
    for telemetry_first in [false, true] {
        let (dir, storage) = temp_storage("carrier-order");
        let file = dir.path().join("events.jsonl");
        write(&file, &[span("call")]);
        owner(&storage, "local-host", "default");
        if !telemetry_first {
            native(&storage, 1);
        }
        scan(&storage, &file, ts("2026-10-02T12:00:00Z"));
        native(&storage, if telemetry_first { 1 } else { 2 });
        let result = summary(&storage, Granularity::Day, Filters::default());
        assert_eq!(
            (result.totals.call_count, result.totals.total_tokens_known),
            (1, Some(120))
        );
        assert!(result.periods[0].partial_history);
        let count: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        // Coverage is limited to the selected source/model, not unrelated users.
        let unrelated = summary(
            &storage,
            Granularity::Day,
            Filters {
                agents: vec!["codex".into()],
                ..Default::default()
            },
        );
        assert!(unrelated.periods.iter().all(|p| !p.partial_history));
    }
}
#[test]
fn carrier_selection_does_not_claim_another_host_or_user_or_rejected_event() {
    for (host, user) in [
        ("other-host", "default"),
        ("local-host", "other-user"),
        ("legacy_unknown", "default"),
    ] {
        let (dir, storage) = temp_storage("carrier-owner");
        owner(&storage, host, user);
        native(&storage, 1);
        let file = dir.path().join("events.jsonl");
        write(&file, &[span("call")]);
        scan(&storage, &file, ts("2026-10-02T12:00:00Z"));
        let verified:i64=storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE source_instance_id='native' AND attribution_status='verified'",[],|r|r.get(0)).unwrap();
        assert_eq!(verified, 1);
    }
    let (_dir, storage) = temp_storage("rejected-carrier");
    owner(&storage, "local-host", "default");
    native(&storage, 1);
    let mut bad = evt("native", "bad", ts("2026-10-02T08:01:00Z"));
    bad.agent = "vscode-copilot-chat".into();
    bad.parser_version = "otel-spans-file-2".into();
    bad.session_id = Some("session".into());
    bad.usage.input_total = Some(-1);
    assert_eq!(
        commit_batch(
            &storage,
            &batch("native", "UTC", ts("2026-10-02T12:00:00Z"), vec![bad]),
            None
        )
        .unwrap()
        .errors,
        1
    );
    assert_eq!(
        summary(&storage, Granularity::Day, Filters::default())
            .totals
            .input_total_known,
        Some(300)
    );
}
#[test]
fn sealed_native_partition_is_not_augmented_with_overlapping_telemetry() {
    let (dir, storage) = temp_storage("sealed-carrier");
    owner(&storage, "local-host", "default");
    native(&storage, 1);
    storage
        .conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    let file = dir.path().join("events.jsonl");
    write(&file, &[span("call")]);
    scan(&storage, &file, ts("2026-10-02T12:00:00Z"));
    assert_eq!(
        summary(&storage, Granularity::Day, Filters::default())
            .totals
            .total_tokens_known,
        Some(340)
    );
}
#[test]
fn claude_spellings_merge_calls_and_tokens_in_daily_and_hourly_queries() {
    let (_dir, storage) = temp_storage("claude-model-key");
    let mut a = with_tokens(evt("source", "one", ts("2026-10-02T08:00:00Z")), 100, 20);
    a.model_raw = Some("claude-opus-4.8".into());
    let mut b = with_tokens(evt("source", "two", ts("2026-10-02T08:01:00Z")), 200, 30);
    b.model_raw = Some("claude-opus-4-8".into());
    commit_batch(
        &storage,
        &batch("source", "UTC", ts("2026-10-02T12:00:00Z"), vec![a, b]),
        None,
    )
    .unwrap();
    for grain in [Granularity::Day, Granularity::Hour] {
        let result = summary(
            &storage,
            grain,
            Filters {
                models: vec!["claude-opus-4.8".into()],
                ..Default::default()
            },
        );
        assert_eq!(result.model_breakdown.len(), 1);
        assert_eq!(result.totals.call_count, 2);
        assert_eq!(result.totals.input_total_known, Some(300));
        assert_eq!(result.totals.input_known_count, 2);
    }
}

#[test]
fn vs_total_upgrade_replays_unchanged_eof_without_duplicate_calls_or_conflicts() {
    use llm_usage_core::adapters::vs_copilot::VsCopilotAdapter;
    use llm_usage_core::domain::FieldQuality;
    let (dir, storage) = temp_storage("vs-total-repair");
    let file = dir.path().join("events.jsonl");
    let now = ts("2026-10-02T12:00:00Z");
    let attributes = vec![
        json!({"key":"gen_ai.usage.input_tokens","value":{"intValue":"100"}}),
        json!({"key":"gen_ai.usage.output_tokens","value":{"intValue":"20"}}),
    ];
    let mut row = json!({"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"vs-copilot"}}]},"scopeSpans":[{"spans":[{"traceId":"trace","spanId":"call","name":"chat gpt-test","kind":3,"endTimeUnixNano":(now*1_000_000).to_string(),"attributes":attributes}]}]}]});
    row["resourceSpans"][0]["scopeSpans"][0]["spans"].as_array_mut().unwrap().push(json!({"traceId":"trace","spanId":"partial","name":"chat gpt-test","kind":3,"endTimeUnixNano":(now*1_000_000).to_string(),"attributes":[{"key":"gen_ai.usage.input_tokens","value":{"intValue":"100"}}]}));
    write(&file, &[row]);
    let scan_vs = |time| {
        run_adapter_scan(
            &storage,
            &VsCopilotAdapter::new(),
            &DiscoverContext {
                home_dir: None,
                env: Default::default(),
                manual_roots: vec![file.clone()],
            },
            &RunConfig {
                timezone: "UTC".into(),
                now_ms: time,
                limits: ScanLimits::default(),
                trigger: TriggerKind::Manual,
                run_id_prefix: format!("vs-{time}"),
                origin_host_id: Some("local-host".into()),
            },
        )
        .unwrap()
    };
    let initial = scan_vs(now);
    let instance = &initial[0].instance_id;
    // Exact legacy payload retained behind a fully consumed, unchanged checkpoint.
    let mut legacy = evt(instance, "vs-copilot:span:trace:call", now);
    legacy.agent = "vs-copilot".into();
    legacy.schema_version =
        llm_usage_core::adapters::vs_copilot::versions::VS_COPILOT_FORMAT_VERSION.into();
    legacy.parser_version = "vs-copilot-otlp-traces-2".into();
    legacy.parse_basis = Some(llm_usage_core::domain::VersionBasis::KnownVersion);
    legacy.origin_call_id = Some("otel-span:trace:call".into());
    legacy.host_application = Some("Visual Studio".into());
    legacy.source_time = Some(now.to_string());
    legacy.provider_id = Some("github-copilot".into());
    legacy.model_raw = Some("gpt-test".into());
    legacy.model_attribution = llm_usage_core::domain::ModelAttribution::RequestField;
    legacy.usage.input_total = Some(100);
    legacy.usage.output_total = Some(20);
    legacy.quality.input_total = FieldQuality::Reported;
    legacy.quality.output_total = FieldQuality::Reported;
    // Test setup restores the old exact payload using SQL, preserving all records/checkpoints.
    for (id, output) in [("call", Some(20)), ("partial", None)] {
        let mut old = legacy.clone();
        old.source_record_key = format!("vs-copilot:span:trace:{id}");
        old.origin_call_id = Some(format!("otel-span:trace:{id}"));
        old.usage.output_total = output;
        old.quality.output_total = if output.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        };
        let hash = llm_usage_core::identity::event_content_hash(&old);
        storage.conn().execute("UPDATE usage_events SET parser_version=?1,total_tokens=NULL,quality_json=?2,content_hash=?3 WHERE source_instance_id=?4 AND source_record_key=?5",
            rusqlite::params![old.parser_version,serde_json::to_string(&old.quality).unwrap(),hash,instance,old.source_record_key]).unwrap();
    }
    storage.conn().execute("UPDATE ingestion_checkpoints SET parse_context=json_remove(parse_context,'$.policy_version') WHERE instance_id=?1",[instance]).unwrap();
    let before = storage.data_revision().unwrap();
    let repaired = scan_vs(now + 1);
    let result = repaired[0].outcome.as_ref().unwrap();
    assert_eq!(
        (
            result.added,
            result.updated,
            result.conflicts,
            result.errors
        ),
        (0, 2, 0, 0)
    );
    assert!(storage.data_revision().unwrap() > before);
    let row: (i64, Option<i64>) = storage
        .conn()
        .query_row(
            "SELECT COUNT(*),SUM(total_tokens) FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, (2, Some(120)));
    let revision = storage.data_revision().unwrap();
    assert_eq!(scan_vs(now + 2)[0].files[0].status, "unchanged");
    assert_eq!(storage.data_revision().unwrap(), revision);
    // A changed source value must remain a conflict even behind an old policy marker.
    legacy.usage.input_total = Some(101);
    let hash = llm_usage_core::identity::event_content_hash(&legacy);
    storage.conn().execute("UPDATE usage_events SET parser_version=?1,input_total=101,total_tokens=NULL,quality_json=?2,content_hash=?3 WHERE source_instance_id=?4 AND source_record_key=?5",
        rusqlite::params![legacy.parser_version,serde_json::to_string(&legacy.quality).unwrap(),hash,instance,legacy.source_record_key]).unwrap();
    storage.conn().execute("UPDATE ingestion_checkpoints SET parse_context=json_remove(parse_context,'$.policy_version') WHERE instance_id=?1",[instance]).unwrap();
    let rejected = scan_vs(now + 3);
    assert_eq!(rejected[0].outcome.as_ref().unwrap().conflicts, 1);
    assert_eq!(rejected[0].outcome.as_ref().unwrap().updated, 0);
}
fn price_book() -> PriceBook {
    let rows=[("openai","gpt-example"),("anthropic","claude-example-4-8"),("google","gemini-example"),
        ("moonshotai","kimi-example"),("zai","glm-example"),("deepseek","deepseek-example")].iter().enumerate().map(|(i,(p,m))|
            json!({"price_id":format!("p{i}"),"provider_id":p,"model":m,"region":"global","channel":"api","effective_from":"2026-10-01","currency":"USD","input":100000,"output":200000,"official_vendor":true})).collect::<Vec<_>>();
    let file = json!({"format":"llm-usage-price-snapshot/1","snapshot":{"id":"official-test","source_type":"manual","fetched_at":"2026-10-01"},"rows":rows});
    PriceBook {
        rows: parse_snapshot_json(&file.to_string()).unwrap().rows,
    }
}
#[test]
fn official_model_references_work_without_provider_or_channel_and_reject_ambiguity() {
    let book = price_book();
    for row in &book.rows {
        for provider in [None, Some("relay".into())] {
            let e = PricingEvent {
                provider_id: provider,
                model_raw: Some(row.model.clone()),
                model_canonical: None,
                occurred_at_ms: ts("2026-10-02T08:00:00Z"),
                input_uncached: Some(1000),
                input_total: Some(1000),
                input_cache_read: Some(0),
                input_cache_write: Some(0),
                output_total: Some(1000),
            };
            let EventEstimate::Priced(amount) =
                book.estimate_at_time(&e, &EstimateOptions::default())
            else {
                panic!("official model was not priced")
            };
            assert!(amount.official_fallback);
            assert_eq!(amount.total_amount_minor, 3);
            let mut missing = e.clone();
            missing.model_raw = Some(format!("{}-missing-release", row.model));
            assert!(matches!(
                book.estimate_at_time(&missing, &EstimateOptions::default()),
                EventEstimate::Unpriced(_)
            ));
        }
    }
    let e = PricingEvent {
        provider_id: None,
        model_raw: Some("claude-example-4.8".into()),
        model_canonical: None,
        occurred_at_ms: ts("2026-10-02T08:00:00Z"),
        input_uncached: Some(1000),
        input_total: Some(1000),
        input_cache_read: Some(0),
        input_cache_write: Some(0),
        output_total: Some(1000),
    };
    let mut ambiguous = book.clone();
    let mut cn = book.rows[1].clone();
    cn.price_id = "cn".into();
    cn.currency = "CNY".into();
    ambiguous.rows.push(cn);
    assert_eq!(
        ambiguous.estimate_at_time(&e, &EstimateOptions::default()),
        EventEstimate::Unpriced(UnpricedReason::ChannelUnknown)
    );
    let mut spoof = book.rows[1].clone();
    spoof.provider_id = "unverified-relay".into();
    assert!(matches!(
        PriceBook { rows: vec![spoof] }.estimate_at_time(&e, &EstimateOptions::default()),
        EventEstimate::Unpriced(_)
    ));
}

#[test]
fn retained_observations_are_priced_once_and_future_catalog_updates_do_not_rewrite_them() {
    use llm_usage_core::storage::pricing::{CostFilters, CostSummaryRequest};
    let (_dir, storage) = temp_storage("cost-policy-repair");
    let now = ts("2026-10-02T12:00:00Z");
    let mut e = with_tokens(evt("source", "observation", now - 1000), 1000, 1000);
    e.model_raw = Some("claude-example-4.8".into());
    e.provider_id = None;
    e.record_kind = RecordKind::UsageObservation;
    commit_batch(&storage, &batch("source", "UTC", now, vec![e]), None).unwrap();
    let import = |id: &str, output: i64| {
        parse_snapshot_json(&json!({"format":"llm-usage-price-snapshot/1","snapshot":{"id":id,"source_type":"manual","fetched_at":"2026-10-01"},
          "rows":[{"price_id":id,"provider_id":"anthropic","model":"claude-example-4-8","region":"global","channel":"api","effective_from":"2026-10-01","currency":"USD","output":output,"official_vendor":true}]}).to_string()).unwrap()
    };
    storage
        .import_price_snapshot(&import("first", 200000), now)
        .unwrap();
    storage
        .ensure_cost_matching_policy("UTC", now, &EstimateOptions::default())
        .unwrap();
    let query = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-10-02".into(),
        last_day: "2026-10-02".into(),
        now_ms: now,
        options: EstimateOptions::default(),
        filters: CostFilters {
            models: vec!["claude-example-4-8".into()],
            ..Default::default()
        },
    };
    let first = storage.cost_summary(&query).unwrap();
    assert_eq!(first.at_time.rows[0].total_amount_minor, 2);
    assert_eq!(first.at_time.rows[0].partial_event_count, 1);
    assert_eq!(first.at_time.rows[0].fallback_event_count, 1);
    storage
        .import_price_snapshot(&import("newer", 500000), now + 1)
        .unwrap();
    storage
        .ensure_cost_matching_policy("UTC", now + 2, &EstimateOptions::default())
        .unwrap();
    let later = storage.cost_summary(&query).unwrap();
    assert_eq!(
        later.at_time.rows[0].total_amount_minor, 2,
        "background catalog update preserves estimates"
    );
    assert_eq!(
        later.current_sim.rows[0].total_amount_minor, 5,
        "current price simulation uses latest catalog"
    );
}

#[test]
fn pricing_distinguishes_unknown_tokens_zero_usage_and_native_currency_units() {
    let (_dir, storage) = temp_storage("pricing-token-coverage");
    let now = ts("2026-10-03T12:00:00Z");
    storage.ensure_seed_price_snapshot(now).unwrap();
    let book = storage.load_price_book().unwrap();
    let unknown = PricingEvent {
        model_raw: Some("k3-256k".into()),
        provider_id: Some("custom".into()),
        occurred_at_ms: now,
        ..Default::default()
    };
    assert_eq!(
        book.estimate_at_time(&unknown, &EstimateOptions::default()),
        EventEstimate::Unpriced(UnpricedReason::NoKnownUsage)
    );
    let mut zero = unknown.clone();
    zero.input_total = Some(0);
    zero.input_uncached = Some(0);
    zero.input_cache_read = Some(0);
    zero.input_cache_write = Some(0);
    zero.output_total = Some(0);
    let EventEstimate::Priced(amounts) = book.estimate_at_time(&zero, &EstimateOptions::default())
    else {
        panic!("known zero usage should be priced")
    };
    assert_eq!(amounts.total_amount_minor, 0);
    assert!(!amounts.has_unknown_components);
    assert!(!amounts.ttl_defaulted);
    let mut event = zero.clone();
    event.input_total = Some(1_000_000);
    event.input_uncached = Some(1_000_000);
    event.output_total = Some(1_000_000);
    let EventEstimate::Priced(usd) = book.estimate_at_time(&event, &EstimateOptions::default())
    else {
        panic!("USD reference missing")
    };
    assert_eq!(
        (usd.currency.as_str(), usd.total_amount_minor),
        ("USD", 1800)
    ); // $3 input + $15 output
    let mut options = EstimateOptions::default();
    options
        .provider_channels
        .insert("custom".into(), ("cn".into(), "api".into()));
    let EventEstimate::Priced(cny) = book.estimate_at_time(&event, &options) else {
        panic!("CNY reference missing")
    };
    assert_eq!(
        (cny.currency.as_str(), cny.total_amount_minor),
        ("CNY", 12000)
    ); // ¥20 input + ¥100 output
    event.input_uncached = None;
    event.input_cache_read = None;
    event.input_cache_write = None;
    let EventEstimate::Priced(partial) = book.estimate_at_time(&event, &EstimateOptions::default())
    else {
        panic!("known output should be priced")
    };
    assert!(partial.has_unknown_components);
    assert_eq!(partial.input_amount_minor, None);
    assert_eq!(partial.total_amount_minor, 1500); // only known output; never guess the input split
}

#[test]
fn old_cost_policy_is_repaired_for_bare_k3_once_without_touching_sealed_days() {
    use llm_usage_core::storage::pricing::{CostFilters, CostSummaryRequest};
    let (_dir, storage) = temp_storage("bare-k3-policy");
    let now = ts("2026-10-03T12:00:00Z");
    storage.ensure_seed_price_snapshot(now).unwrap();
    let mut e = with_tokens(evt("source", "k3-call", now - 1000), 1000, 1000);
    e.model_raw = Some("k3-256k".into());
    e.provider_id = Some("kimi-code-custom".into());
    commit_batch(&storage, &batch("source", "UTC", now, vec![e]), None).unwrap();
    storage
        .recompute_cost_day("UTC", "2026-10-03", now, &EstimateOptions::default())
        .unwrap();
    storage.conn().execute_batch("DELETE FROM daily_cost_usage WHERE currency='';
        UPDATE daily_cost_usage SET currency='',priced_event_count=0,unpriced_event_count=1,total_amount_minor=0,unpriced_reasons='{\"no_price_row\":1}';
        INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES('cost_matching_policy:UTC','official-reference-2',1,0);").unwrap();
    let query = CostSummaryRequest {
        timezone: "UTC".into(),
        first_day: "2026-10-03".into(),
        last_day: "2026-10-03".into(),
        now_ms: now,
        options: EstimateOptions::default(),
        filters: CostFilters::default(),
    };
    assert_eq!(
        storage.cost_summary(&query).unwrap().at_time.rows[0].unpriced_event_count,
        1
    );
    storage
        .ensure_cost_matching_policy("UTC", now, &query.options)
        .unwrap();
    let repaired = storage.cost_summary(&query).unwrap();
    assert_eq!(repaired.at_time.rows[0].currency, "USD");
    assert_eq!(repaired.at_time.rows[0].priced_event_count, 1);
    let marker: String = storage
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key='cost_matching_policy:UTC'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(marker, "official-reference-3");
    storage
        .conn()
        .execute_batch("UPDATE daily_cost_usage SET sealed=1; UPDATE daily_usage SET sealed=1;")
        .unwrap();
    storage
        .conn()
        .execute(
            "UPDATE settings SET value='official-reference-2' WHERE key='cost_matching_policy:UTC'",
            [],
        )
        .unwrap();
    storage
        .ensure_cost_matching_policy("UTC", now + 1, &query.options)
        .unwrap();
    let sealed = storage.cost_summary(&query).unwrap();
    assert_eq!(
        sealed.at_time.rows[0].total_amount_minor,
        repaired.at_time.rows[0].total_amount_minor
    );
    assert_eq!(sealed.at_time.rows[0].priced_event_count, 1);
}

#[test]
fn trace_span_identity_replays_legacy_records_without_duplicate_contributions() {
    let (dir, storage) = temp_storage("otel-legacy-identity");
    let file = dir.path().join("events.jsonl");
    write(&file, &[span("call")]);
    let now = ts("2026-10-02T12:00:00Z");
    scan(&storage, &file, now);
    let instance: String = storage
        .conn()
        .query_row("SELECT source_instance_id FROM usage_events", [], |r| {
            r.get(0)
        })
        .unwrap();
    let old_event_id = llm_usage_core::identity::event_id(&instance, "otel:call");
    storage.conn().execute("UPDATE usage_events SET event_id=?1,source_record_key='otel:call',origin_call_id='otel-span:call',parser_version='otel-spans-doc1'", [&old_event_id]).unwrap();
    storage.conn().execute_batch("
        UPDATE ingestion_checkpoints SET parse_context=json_remove(parse_context,'$.policy_version');").unwrap();
    scan(&storage, &file, now + 1);
    assert_eq!(
        summary(&storage, Granularity::Day, Filters::default())
            .totals
            .call_count,
        1
    );
    let old:i64=storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE exclusion_reason='otel_trace_identity_upgrade'",[],|r|r.get(0)).unwrap();
    assert_eq!(old, 1);
}
#[test]
fn repeated_file_exports_use_trace_and_span_identity_and_discovery_dedups_paths() {
    let (dir, storage) = temp_storage("otel-copy-identity");
    let first = dir.path().join("a.jsonl");
    let second = dir.path().join("b.jsonl");
    write(&first, &[span("shared-span")]);
    let mut other = span("shared-span");
    other["traceId"] = json!("another-trace");
    write(&second, &[span("shared-span"), other]);
    scan(&storage, &first, ts("2026-10-02T12:00:00Z"));
    scan(&storage, &second, ts("2026-10-02T12:00:01Z"));
    assert_eq!(
        summary(&storage, Granularity::Day, Filters::default())
            .totals
            .call_count,
        2,
        "same span ID across different traces is a different call"
    );
    let roots = OtelAdapter::new().discover(&DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![first.clone(), dir.path().to_path_buf()],
    });
    assert_eq!(
        roots.iter().map(|r| r.files.len()).sum::<usize>(),
        2,
        "parent/file overlap does not duplicate discovery"
    );
}
#[test]
fn fractional_token_attributes_are_rejected_instead_of_truncated() {
    let (dir, storage) = temp_storage("otel-fraction");
    let file = dir.path().join("events.jsonl");
    let mut bad = span("bad");
    bad["attributes"]["gen_ai.usage.input_tokens"] = json!({"doubleValue":1.5});
    write(&file, &[span("good"), bad]);
    scan(&storage, &file, ts("2026-10-02T12:00:00Z"));
    assert_eq!(
        summary(&storage, Granularity::Day, Filters::default())
            .totals
            .input_total_known,
        Some(100)
    );
    assert_eq!(
        summary(&storage, Granularity::Day, Filters::default())
            .totals
            .call_count,
        1
    );
}
#[test]
fn kimi_profile_alias_and_official_global_named_channels_have_reference_prices() {
    let (_dir, storage) = temp_storage("official-seed-alias");
    let now = ts("2026-10-02T12:00:00Z");
    storage.ensure_seed_price_snapshot(now).unwrap();
    let mut book = storage.load_price_book().unwrap();
    for model in [
        "kimi-code/k3-256k",
        "kimi-code/k3",
        "glm-5.3",
        "glm-5.3-flash",
        "claude-opus-4.8",
        "gpt-4o-mini-2024-07-18",
    ] {
        let e = PricingEvent {
            provider_id: None,
            model_raw: Some(model.into()),
            model_canonical: None,
            occurred_at_ms: now,
            input_uncached: Some(1000),
            input_total: Some(1000),
            input_cache_read: Some(0),
            input_cache_write: Some(0),
            output_total: Some(1000),
        };
        assert!(
            matches!(book.estimate_at_time(&e,&EstimateOptions::default()),EventEstimate::Priced(a) if a.official_fallback && a.currency=="USD"),
            "{model}"
        );
    }
    assert_eq!(
        llm_usage_core::model_names::reference_model_key("kimi-code/kimi-for-coding"),
        "kimi-code/kimi-for-coding"
    );
    // 真实场景（issue 5）：Kilo Code / oh-my-pi 以裸 model_raw `k3-256k` + 用户自定义
    // provider（如 kimi-code-owent）上报；别名仍应命中官方 moonshot kimi-k3 参考价。
    for provider in [
        None,
        Some("kimi-code-owent".to_string()),
        Some("custom-relay".to_string()),
    ] {
        for model in ["k3-256k", "k3"] {
            let e = PricingEvent {
                provider_id: provider.clone(),
                model_raw: Some(model.into()),
                model_canonical: None,
                occurred_at_ms: now,
                input_uncached: Some(1000),
                input_total: Some(1000),
                input_cache_read: Some(0),
                input_cache_write: Some(0),
                output_total: Some(1000),
            };
            assert!(
                matches!(book.estimate_at_time(&e, &EstimateOptions::default()), EventEstimate::Priced(a) if a.official_fallback && a.currency == "USD"),
                "bare {model} / {provider:?}"
            );
        }
    }
    assert_eq!(
        llm_usage_core::model_names::reference_model_key("k3-256k"),
        "kimi-k3"
    );
    assert_eq!(
        llm_usage_core::model_names::reference_model_key("k3"),
        "kimi-k3"
    );
    let mut configured = book
        .rows
        .iter()
        .find(|r| r.model == "kimi-k3")
        .unwrap()
        .clone();
    configured.price_id = "configured-profile".into();
    configured.provider_id = "kimi-code".into();
    configured.model = "kimi-code/k3-256k".into();
    configured.region = "configured".into();
    configured.channel = "configured".into();
    book.rows.push(configured);
    let mut options = EstimateOptions::default();
    options.provider_channels.insert(
        "kimi-code".into(),
        ("configured".into(), "configured".into()),
    );
    let event = PricingEvent {
        provider_id: Some("kimi-code".into()),
        model_raw: Some("kimi-code/k3-256k".into()),
        model_canonical: None,
        occurred_at_ms: now,
        input_uncached: Some(1000),
        input_total: Some(1000),
        input_cache_read: Some(0),
        input_cache_write: Some(0),
        output_total: Some(1000),
    };
    assert!(
        matches!(book.estimate_at_time(&event, &options), EventEstimate::Priced(a)
        if !a.official_fallback && a.matched_price_ids == ["configured-profile"])
    );
}
