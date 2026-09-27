mod common;

use common::*;
use llm_usage_core::calendar::{parse_date, WeekStart};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{
    agent_breakdown, chart_series, event_details, hourly_breakdown, query_summary, ChartDimension,
    EventDetailRequest, Filters, Granularity, SummaryRequest,
};
use llm_usage_core::retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy};

fn request(first: &str, last: &str, granularity: Granularity) -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: parse_date(first).unwrap(),
        last_day: parse_date(last).unwrap(),
        granularity,
        filters: Filters::default(),
        today: parse_date("2026-09-27").unwrap(),
        retention_cutoff: None,
    }
}

#[test]
fn names_group_and_filter_without_case_splitting() {
    let (_dir, db) = temp_storage("case-fold");
    let at = ts("2026-09-27T10:00:00Z");
    let mut a = with_tokens(evt("source", "one", at), 100, 10);
    a.agent = "CodeX".into();
    a.model_raw = Some("GLM-5.3".into());
    let mut b = with_tokens(evt("source", "two", at + 1000), 200, 20);
    b.agent = "codex".into();
    b.model_raw = Some("glm-5.3".into());
    commit_batch(&db, &batch("source", "UTC", at, vec![a, b]), None).unwrap();
    let mut q = request("2026-09-27", "2026-09-27", Granularity::Day);
    assert_eq!(agent_breakdown(&db, &q).unwrap().len(), 1);
    assert_eq!(
        chart_series(&db, &q, &ChartDimension::ByModel)
            .unwrap()
            .len(),
        1
    );
    q.filters.agents = vec!["CODEX".into()];
    q.filters.models = vec!["Glm-5.3".into()];
    assert_eq!(
        query_summary(&db, &q).unwrap().totals.total_tokens_known,
        Some(330)
    );
    assert_eq!(
        hourly_breakdown(&db, "UTC", q.first_day, &q.filters).unwrap()[0].call_count,
        2
    );
    assert_eq!(
        event_details(
            &db,
            &EventDetailRequest {
                timezone: "UTC".into(),
                from_ms: at,
                to_ms: at + 2000,
                offset: 0,
                limit: 20,
                filters: q.filters,
            }
        )
        .unwrap()
        .total_count,
        2
    );
}

#[test]
fn hour_sessions_are_distinct_and_duration_is_sample_weighted() {
    let (_dir, db) = temp_storage("hour-sessions");
    let at = ts("2026-09-27T10:00:00Z");
    let mut events = Vec::new();
    for (i, hour, session, model, duration) in [
        (0, 0, "same", "a", Some(100)),
        (1, 0, "same", "b", Some(300)),
        (2, 0, "other", "a", None),
        (3, 1, "same", "b", Some(900)),
    ] {
        let mut e = with_tokens(
            evt("source", &i.to_string(), at + hour * 3_600_000),
            100,
            10,
        );
        e.session_id = Some(session.into());
        e.model_raw = Some(model.into());
        e.duration_ms = duration;
        events.push(e);
    }
    commit_batch(&db, &batch("source", "UTC", at, events), None).unwrap();
    let q = request("2026-09-27", "2026-09-27", Granularity::Hour);
    let hours = hourly_breakdown(&db, "UTC", q.first_day, &q.filters).unwrap();
    assert_eq!(hours[0].session_count, Some(2));
    assert_eq!(hours[0].avg_duration_ms, Some(200));
    assert_eq!(hours[1].session_count, Some(1));
    let s = query_summary(&db, &q).unwrap();
    assert_eq!(s.periods[0].distinct_sessions, Some(2));
    assert_eq!(s.periods[1].distinct_sessions, Some(1));
    assert_eq!(s.totals.avg_duration_ms, Some(433));
    assert_eq!(s.totals.total_duration_ms, Some(1300));
    assert_eq!(s.totals.total_known_count, 4);
    assert_eq!(s.totals.total_unknown_count, 0);
    let daily = query_summary(&db, &request("2026-09-27", "2026-09-27", Granularity::Day)).unwrap();
    assert_eq!(
        s.totals, daily.totals,
        "hour and day completeness must agree"
    );
}

#[test]
fn missing_session_ids_are_unknown_and_retained_days_remain_countable() {
    let (_dir, db) = temp_storage("missing-session");
    let at = ts("2026-09-20T10:00:00Z");
    let mut e = with_tokens(evt("source", "one", at), 100, 10);
    e.session_id = None;
    commit_batch(&db, &batch("source", "UTC", at, vec![e]), None).unwrap();
    let q = request("2026-09-20", "2026-09-27", Granularity::Day);
    assert_eq!(query_summary(&db, &q).unwrap().distinct_sessions, None);
    let policy = TieredRetentionPolicy::default();
    enforce_tiered_retention(&db, "UTC", ts("2026-09-29T12:00:00Z"), &policy).unwrap();
    let s = query_summary(&db, &q).unwrap();
    assert_eq!(s.active_days, Some(1));
    assert_eq!(s.distinct_sessions, None);
    assert!(s.periods[0].partial_history);
    let rev = s.data_revision;
    let again = enforce_tiered_retention(&db, "UTC", ts("2026-09-29T12:00:01Z"), &policy).unwrap();
    assert_eq!(again.data_revision, rev);
    assert_eq!(
        again.materialized_period_rows, 0,
        "unchanged cleanup does no materialization work"
    );
}

#[test]
fn archived_period_keeps_all_sources_models_and_repeated_cleanup() {
    let (_dir, db) = temp_storage("all-period-dims");
    let at = ts("2025-12-15T10:00:00Z");
    for source in ["a", "b"] {
        let mut events = Vec::new();
        for model in ["m1", "m2"] {
            let mut e = with_tokens(evt(source, model, at), 100, 10);
            e.model_raw = Some(model.into());
            events.push(e);
        }
        commit_batch(&db, &batch(source, "UTC", at, events), None).unwrap();
    }
    let policy = TieredRetentionPolicy {
        events_days: 1,
        hourly_days: 2,
        daily_days: 3,
        weekly_days: 3650,
        monthly_days: 10950,
        yearly_days: None,
    };
    for n in 0..2 {
        enforce_tiered_retention(&db, "UTC", ts("2026-09-27T12:00:00Z") + n, &policy).unwrap();
        let q = request("2025-12-01", "2025-12-31", Granularity::Month);
        let s = query_summary(&db, &q).unwrap();
        assert_eq!(s.totals.total_tokens_known, Some(440));
        assert_eq!(s.model_breakdown.len(), 2);
        assert_eq!(
            agent_breakdown(&db, &q).unwrap()[0].sums.total_tokens_known,
            Some(440)
        );
        assert_eq!(
            chart_series(&db, &q, &ChartDimension::Total).unwrap()[0].total_tokens,
            Some(440)
        );
        // A whole archived month cannot be attributed to one selected day.
        let narrow = request("2025-12-15", "2025-12-15", Granularity::Month);
        assert_eq!(
            query_summary(&db, &narrow)
                .unwrap()
                .totals
                .total_tokens_known,
            None
        );
    }
}

#[test]
fn codex_archive_only_and_active_archive_copies_count_once() {
    let (_dbdir, db) = temp_storage("codex-archives");
    let dir = TempDir::new("codex-archive-source");
    let data = reconstruct_codex_jsonl(&codex_fixture("rollout-single-call.sanitized.json"));
    let archive = dir.path().join("archived_sessions");
    std::fs::create_dir_all(&archive).unwrap();
    std::fs::write(archive.join("rollout-archived.jsonl"), &data).unwrap();
    let reports = run_codex(&db, dir.path(), 1_800_000_000_000);
    assert_eq!(reports.len(), 1, "archive-only source must be discovered");
    let count = || {
        db.conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(count(), 1);
    codex_root_with_file(&dir, "rollout-active.jsonl", &data);
    run_codex(&db, dir.path(), 1_800_000_000_001);
    assert_eq!(
        count(),
        1,
        "active/archive overlap must not duplicate calls"
    );
    run_codex(&db, dir.path(), 1_800_000_000_002);
    assert_eq!(count(), 1);
}

#[test]
fn empty_user_scope_never_returns_other_users_usage() {
    let (_dir, db) = temp_storage("empty-user-scope");
    let at = ts("2026-09-27T10:00:00Z");
    commit_batch(
        &db,
        &batch(
            "source",
            "UTC",
            at,
            vec![with_tokens(evt("source", "one", at), 100, 10)],
        ),
        None,
    )
    .unwrap();
    let mut q = request("2026-09-27", "2026-09-27", Granularity::Day);
    q.filters.instances = Some(vec![]);
    assert_eq!(query_summary(&db, &q).unwrap().totals.event_count, 0);
    assert!(chart_series(&db, &q, &ChartDimension::Total)
        .unwrap()
        .is_empty());
    assert!(hourly_breakdown(&db, "UTC", q.first_day, &q.filters)
        .unwrap()
        .is_empty());
    assert_eq!(
        event_details(
            &db,
            &EventDetailRequest {
                timezone: "UTC".into(),
                from_ms: at,
                to_ms: at + 1000,
                offset: 0,
                limit: 20,
                filters: q.filters,
            }
        )
        .unwrap()
        .total_count,
        0
    );
}
