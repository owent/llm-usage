mod common;
use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    calendar::{ymd, WeekStart},
    ingest::commit_batch,
    model_aliases::{self, import_catalog, parse_catalog, read_state, Book, STATE_KEY},
    query::{chart_series, query_summary, ChartDimension, Filters, Granularity, SummaryRequest},
};

const NOW: i64 = 1_794_960_000_000;
fn catalog(version: &str, name: &str, from: &str, until: Option<&str>) -> String {
    serde_json::json!({"schema_version":1,"rule_version":version,"published_at":"2026-11-16T00:00:00Z",
        "aliases":[{"agent":"qoder","key":"qmodel_latest","display_name":name,"effective_from":from,
        "effective_until":until,"basis":"catalogue","source_urls":[]}]}).to_string()
}

#[test]
fn catalogue_validation_is_bounded_and_does_not_echo_values() {
    let raw = catalog("v1", "Qwen4-Max-Preview", "2026-11-16T00:00:00Z", None);
    parse_catalog(&raw, NOW).unwrap();
    for replacement in ["secret-body-value", "{}", "{\"schema_version\":999}"] {
        let error = parse_catalog(replacement, NOW).unwrap_err().to_string();
        assert!(!error.contains("secret-body-value"));
    }
    for field in ["client_version", "host_application", "prompt"] {
        let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        json["aliases"][0][field] = "not-supported".into();
        assert!(parse_catalog(&json.to_string(), NOW).is_err());
    }
    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let duplicate = json["aliases"][0].clone();
    json["aliases"].as_array_mut().unwrap().push(duplicate);
    assert!(parse_catalog(&json.to_string(), NOW).is_err());
    assert!(parse_catalog(
        &catalog("v1", "\u{0001}", "2026-11-16T00:00:00Z", None),
        NOW
    )
    .is_err());
    assert!(parse_catalog(
        &catalog(
            "v1",
            "x",
            "2026-11-16T00:00:00Z",
            Some("2026-11-15T00:00:00Z")
        ),
        NOW
    )
    .is_err());
    assert!(parse_catalog(&"x".repeat(model_aliases::MAX_CATALOG_BYTES + 1), NOW).is_err());
}

#[test]
fn dated_updates_preserve_old_intervals_conflicts_and_expired_rules() {
    let (_dir, s) = temp_storage("alias-intervals");
    let a = ts("2026-11-16T00:00:00Z");
    let b = ts("2026-11-17T00:00:00Z");
    let c = ts("2026-11-18T00:00:00Z");
    assert_eq!(
        Book::load(&s).label("qmodel_latest", "qoder", None, a, a + 1),
        "Qwen4-Max-Preview"
    );
    import_catalog(
        &s,
        &catalog(
            "v1",
            "New model",
            "2026-11-17T00:00:00Z",
            Some("2026-11-18T00:00:00Z"),
        ),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    let book = Book::load(&s);
    assert_eq!(
        book.label("qmodel_latest", "qoder", None, a, b),
        "Qwen4-Max-Preview"
    );
    assert_eq!(
        book.label("qmodel-latest", "qoder", None, b, c),
        "New model"
    );
    assert_eq!(
        book.label("qmodel-latest", "qoder", None, a, c),
        "qmodel-latest"
    );
    assert_eq!(
        book.label("qmodel-latest", "qoder", None, c, c + 1),
        "qmodel-latest"
    );
    assert_eq!(
        book.label("qmodel_latest", "other", None, b, b + 1),
        "qmodel_latest"
    );
    import_catalog(
        &s,
        &catalog("v2", "Conflict", "2026-11-17T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    assert_eq!(
        Book::load(&s).label("qmodel_latest", "qoder", None, b, b + 1),
        "qmodel_latest"
    );
    import_catalog(
        &s,
        &catalog("v3", "Future model", "2026-11-19T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    assert_eq!(
        Book::load(&s).label("qmodel_latest", "qoder", None, b, b + 1),
        "qmodel_latest"
    );
    assert_eq!(read_state(&s).unwrap().0.candidates.len(), 3);
}

#[test]
fn imports_are_atomic_idempotent_versioned_and_arbitrated_across_writers() {
    let (dir, s) = temp_storage("alias-writers");
    let raw = catalog("v1", "Model A", "2026-11-16T00:00:00Z", None);
    let initial = read_state(&s).unwrap().1;
    let first = import_catalog(&s, &raw, NOW, "online", Some(&initial), true).unwrap();
    let revision = s.data_revision().unwrap();
    assert_eq!(first.generation, 1);
    let repeat = import_catalog(&s, &raw, NOW + 1, "manual", None, false).unwrap();
    assert!(repeat.already_present);
    assert_eq!(repeat.generation, 1);
    assert!(import_catalog(
        &s,
        &catalog("v1", "Wrong", "2026-11-16T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false
    )
    .is_err());
    assert!(import_catalog(&s, &raw, NOW, "online", Some(&initial), true).is_err());
    let (_, token) = read_state(&s).unwrap();
    let other = llm_usage_core::storage::Storage::open(&dir.db_path()).unwrap();
    import_catalog(
        &other,
        &catalog("v2", "Model B", "2026-11-17T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    assert!(import_catalog(&s, &raw, NOW + 2, "online", Some(&token), true).is_err());
    assert_eq!(s.data_revision().unwrap(), revision);
    let before = read_state(&s).unwrap().1;
    s.conn().execute_batch("CREATE TRIGGER reject_alias_update BEFORE UPDATE ON settings WHEN NEW.key='model_alias_catalog_v1' BEGIN SELECT RAISE(ABORT,'synthetic rollback'); END;").unwrap();
    assert!(import_catalog(
        &s,
        &catalog("v3", "Model C", "2026-11-18T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false
    )
    .is_err());
    assert_eq!(read_state(&s).unwrap().1, before);
}

#[test]
fn metadata_only_updates_relabel_cached_usage_without_rewriting_tokens_or_raw_keys() {
    let (_dir, s) = temp_storage("alias-query");
    let at = ts("2026-11-16T10:00:00Z");
    let mut e = with_tokens(evt("source", "call", at), 100, 20);
    e.agent = "qoder".into();
    e.provider_id = None;
    e.model_raw = Some("qmodel_latest".into());
    commit_batch(&s, &batch("source", "UTC", at, vec![e]), None).unwrap();
    let request = SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 11, 16),
        last_day: ymd(2026, 11, 16),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: ymd(2026, 11, 16),
        retention_cutoff: None,
    };
    let before = query_summary(&s, &request).unwrap();
    assert_eq!(
        before.model_breakdown[0].display_model.as_deref(),
        Some("Qwen4-Max-Preview")
    );
    import_catalog(
        &s,
        &catalog("v1", "Renamed model", "2026-11-16T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    let after = query_summary(&s, &request).unwrap();
    assert_eq!(after.data_revision, before.data_revision);
    assert_eq!(after.totals, before.totals);
    assert_eq!(
        after.model_breakdown[0].display_model.as_deref(),
        Some("Renamed model")
    );
    assert_eq!(
        after.model_breakdown[0].model_raw.as_deref(),
        Some("qmodel-latest")
    );
    let chart = chart_series(&s, &request, &ChartDimension::ByAgentModel).unwrap();
    assert_eq!(chart[0].series_name, "qoder/qmodel-latest");
    assert_eq!(chart[0].display_series, "qoder/Renamed model");
    assert_eq!(
        s.conn()
            .query_row("SELECT model_raw FROM usage_events", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "qmodel_latest"
    );
    let value: String = s
        .conn()
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [STATE_KEY],
            |r| r.get(0),
        )
        .unwrap();
    s.conn()
        .execute(
            "UPDATE settings SET value=?1 WHERE key=?2",
            [
                value.replace("Renamed model", "Tampered name"),
                STATE_KEY.to_string(),
            ],
        )
        .unwrap();
    assert!(Book::load(&s).error.is_some());
    assert_eq!(
        query_summary(&s, &request).unwrap().model_breakdown[0]
            .display_model
            .as_deref(),
        Some("qmodel-latest")
    );
}

#[test]
fn provider_rules_require_exact_record_scope_and_do_not_guess_namespaces_or_suffixes() {
    let (_dir, s) = temp_storage("alias-provider");
    let mut value: serde_json::Value = serde_json::from_str(&catalog(
        "provider",
        "Exact provider model",
        "2026-11-16T00:00:00Z",
        None,
    ))
    .unwrap();
    value["aliases"][0]["provider_id"] = "verified-provider".into();
    import_catalog(&s, &value.to_string(), NOW, "manual", None, false).unwrap();
    let at = ts("2026-11-16T10:00:00Z");
    let book = Book::load(&s);
    assert_eq!(
        book.label(
            "qmodel_latest",
            "qoder",
            Some("verified-provider"),
            at,
            at + 1
        ),
        "Exact provider model"
    );
    for provider in [None, Some("other-provider")] {
        assert_eq!(
            book.label("qmodel_latest", "qoder", provider, at, at + 1),
            "Qwen4-Max-Preview"
        );
    }
    for raw in [
        "custom/qmodel_latest",
        "qmodel_latest_old",
        "qmodel_latest:preview",
    ] {
        assert_eq!(
            book.label(raw, "qoder", Some("verified-provider"), at, at + 1),
            raw
        );
    }
    assert_eq!(
        book.label(
            "qmodel_latest",
            "another-agent",
            Some("verified-provider"),
            at,
            at + 1
        ),
        "qmodel_latest"
    );
}

fn request() -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 11, 16),
        last_day: ymd(2026, 11, 16),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: ymd(2026, 11, 16),
        retention_cutoff: None,
    }
}

#[test]
fn accelerated_mixed_agent_queries_keep_all_contributors_and_readonly_cache_invalidation() {
    let (_dir, s) = temp_storage("alias-mixed");
    let at = ts("2026-11-16T10:00:00Z");
    for (source, agent) in [("a", "qoder"), ("b", "other")] {
        let mut e = with_tokens(evt(source, source, at), 100, 20);
        e.agent = agent.into();
        e.model_raw = Some("qmodel_latest".into());
        commit_batch(&s, &batch(source, "UTC", at, vec![e]), None).unwrap();
    }
    assert!(
        s.conn()
            .query_row("SELECT COUNT(*) FROM query_rollup_model", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
            > 0
    );
    let reader = llm_usage_core::storage::Storage::open_readonly(s.path()).unwrap();
    let mut q = request();
    assert_eq!(
        query_summary(&reader, &q).unwrap().model_breakdown[0]
            .display_model
            .as_deref(),
        Some("qmodel-latest")
    );
    q.filters.agents = vec!["qoder".into()];
    let old = query_summary(&reader, &q).unwrap();
    assert_eq!(
        old.model_breakdown[0].display_model.as_deref(),
        Some("Qwen4-Max-Preview")
    );
    import_catalog(
        &s,
        &catalog("rename", "Next model", "2026-11-16T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    let updated = query_summary(&reader, &q).unwrap();
    assert_eq!(updated.data_revision, old.data_revision);
    assert_ne!(updated.model_alias_revision, old.model_alias_revision);
    assert_eq!(
        updated.model_breakdown[0].display_model.as_deref(),
        Some("Next model")
    );
    let chart = chart_series(&reader, &q, &ChartDimension::ByModel).unwrap();
    assert_eq!(chart[0].display_series, "Next model");
    q.filters.agents.clear();
    assert_eq!(
        chart_series(&reader, &q, &ChartDimension::ByModel).unwrap()[0].display_series,
        "qmodel-latest"
    );
    q.filters.instances = Some(vec!["a".into()]);
    assert_eq!(
        query_summary(&reader, &q).unwrap().model_breakdown[0]
            .display_model
            .as_deref(),
        Some("Next model")
    );
}

#[test]
fn archived_and_hourly_labels_use_the_same_usage_partitions_without_repricing() {
    use llm_usage_core::retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy};
    let (_dir, s) = temp_storage("alias-archives");
    let at = ts("2026-11-16T10:00:00Z");
    let mut e = with_tokens(evt("a", "old", at), 100, 20);
    e.agent = "qoder".into();
    e.model_raw = Some("qmodel_latest".into());
    commit_batch(&s, &batch("a", "UTC", at, vec![e]), None).unwrap();
    import_catalog(
        &s,
        &catalog("archived", "Archive model", "2026-11-01T00:00:00Z", None),
        NOW,
        "manual",
        None,
        false,
    )
    .unwrap();
    let mut q = request();
    q.granularity = Granularity::Hour;
    let hourly = chart_series(&s, &q, &ChartDimension::ByModel).unwrap();
    assert_eq!(hourly[0].display_series, "Archive model");
    assert_eq!(hourly[0].total_tokens, Some(120));
    q.granularity = Granularity::Month;
    q.first_day = ymd(2026, 11, 1);
    q.last_day = ymd(2026, 11, 30);
    let before = query_summary(&s, &q).unwrap();
    enforce_tiered_retention(
        &s,
        "UTC",
        ts("2027-02-04T12:00:00Z"),
        &TieredRetentionPolicy {
            events_days: 1,
            hourly_days: 1,
            daily_days: 1,
            weekly_days: 1095,
            monthly_days: 3650,
            yearly_days: None,
        },
    )
    .unwrap();
    assert_eq!(
        s.conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        s.conn()
            .query_row("SELECT COUNT(*) FROM daily_usage", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let after = query_summary(&s, &q).unwrap();
    assert_eq!(
        after.totals.total_tokens_known,
        before.totals.total_tokens_known
    );
    assert_eq!(
        after.model_breakdown[0].display_model.as_deref(),
        Some("Archive model")
    );
    let rows = chart_series(&s, &q, &ChartDimension::ByAgentModel).unwrap();
    assert_eq!(rows[0].display_series, "qoder/Archive model");
    assert_eq!(rows[0].series_name, "qoder/qmodel-latest");
}

#[test]
fn record_native_canonical_model_precedes_catalogue_without_modifying_raw_identity() {
    use llm_usage_core::query::{event_details, EventDetailRequest};
    let (_dir, s) = temp_storage("alias-canonical");
    let at = ts("2026-11-16T10:00:00Z");
    let mut e = with_tokens(evt("a", "canonical", at), 100, 20);
    e.agent = "qoder".into();
    e.model_raw = Some("qmodel_latest".into());
    e.model_canonical = Some("Native known model".into());
    commit_batch(&s, &batch("a", "UTC", at, vec![e]), None).unwrap();
    let page = event_details(
        &s,
        &EventDetailRequest {
            timezone: "UTC".into(),
            from_ms: at,
            to_ms: at + 1,
            filters: Filters::default(),
            offset: 0,
            limit: 50,
        },
    )
    .unwrap();
    assert_eq!(
        page.rows[0].display_model.as_deref(),
        Some("Native known model")
    );
    assert_eq!(page.rows[0].model_raw.as_deref(), Some("qmodel_latest"));
}

#[test]
fn cache_publications_and_settings_changes_cannot_overwrite_newer_state() {
    let (_dir, s) = temp_storage("alias-cache");
    let raw = catalog("first", "First model", "2026-11-16T00:00:00Z", None);
    let etag = read_state(&s).unwrap().1;
    model_aliases::import_online_catalog(&s, &raw, NOW, &etag, 0).unwrap();
    let token = read_state(&s).unwrap().1;
    let mut older: serde_json::Value =
        serde_json::from_str(&catalog("older", "Old model", "2026-11-16T00:00:00Z", None)).unwrap();
    older["published_at"] = "2026-11-15T00:00:00Z".into();
    assert!(
        model_aliases::import_online_catalog(&s, &older.to_string(), NOW + 1, &token, 0).is_err()
    );
    assert_eq!(read_state(&s).unwrap().1, token);
    s.conn()
        .execute("INSERT INTO settings VALUES('app_settings','{}',1,100)", [])
        .unwrap();
    let raw = catalog("second", "Second model", "2026-11-17T00:00:00Z", None);
    assert!(model_aliases::import_online_catalog(&s, &raw, NOW, &token, 0).is_err());
    assert_eq!(read_state(&s).unwrap().1, token);
    model_aliases::import_online_catalog(&s, &raw, NOW, &token, 100).unwrap();
    assert_eq!(
        read_state(&s).unwrap().0.cache.unwrap().rule_version,
        "second"
    );
}

#[test]
fn published_seed_is_a_user_override_and_cannot_claim_an_official_served_model() {
    let raw = include_str!("../../../../../docs/site/public/models/aliases.json");
    let feed = parse_catalog(raw, NOW).unwrap();
    assert_eq!(feed.aliases[0].basis, "user_override");
    assert_eq!(feed.aliases[0].display_name, "Qwen4-Max-Preview");
}
