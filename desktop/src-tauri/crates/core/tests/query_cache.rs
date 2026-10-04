mod common;
use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    calendar::{ymd, WeekStart},
    ingest::commit_batch,
    query::{query_summary, query_summary_selected, Filters, Granularity, SummaryRequest},
    storage::Storage,
};

fn request() -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 10, 1),
        last_day: ymd(2026, 10, 2),
        today: ymd(2026, 10, 2),
        granularity: Granularity::Day,
        filters: Filters::default(),
        retention_cutoff: None,
    }
}

#[test]
fn streaming_days_preserve_unknowns_normalized_dimensions_and_source_selection() {
    let (_dir, storage) = temp_storage("query-stream");
    for (source, day, key, input, output, duration) in [
        ("a", "2026-10-01T12:00:00Z", "one", 100, 10, Some(100)),
        ("a", "2026-10-02T12:00:00Z", "two", 0, 0, Some(200)),
        ("b", "2026-10-02T12:00:00Z", "three", 50, 5, Some(300)),
        ("a", "2026-10-02T12:00:01Z", "four", 10, 0, None),
    ] {
        let mut event = evt(source, key, ts(day));
        if key != "two" {
            event = with_tokens(event, input, output);
        } else {
            event.usage.input_total = Some(20);
            event.quality.input_total = llm_usage_core::domain::FieldQuality::Reported;
        }
        event.model_raw = Some(if source == "a" { "Model_X" } else { "model-x" }.into());
        event.agent = if source == "a" { "Example" } else { "example" }.into();
        event.session_id = (key != "four").then(|| "shared".into());
        event.duration_ms = duration;
        commit_batch(&storage, &batch(source, "UTC", ts(day), vec![event]), None).unwrap();
    }
    let q = request();
    for _ in 0..2 {
        let result = query_summary(&storage, &q).unwrap();
        assert_eq!(result.totals.call_count, 4);
        assert_eq!(result.totals.total_tokens_known, Some(175));
        assert_eq!(result.totals.total_unknown_count, 1);
        assert_eq!(result.totals.total_duration_ms, Some(600));
        assert_eq!(result.totals.avg_duration_ms, Some(200));
        assert_eq!(result.active_days, Some(2));
        assert_eq!(result.distinct_sessions, None);
        assert_eq!(result.model_breakdown.len(), 1);
        assert_eq!(result.agent_breakdown.len(), 1);
        assert_eq!(result.periods[0].distinct_sessions, Some(1));
    }
    let selected =
        query_summary_selected(&storage, &q, Some(("2026-10-01", "2026-10-01"))).unwrap();
    assert_eq!(selected.totals.total_tokens_known, Some(110));
    assert_eq!(selected.distinct_sessions, Some(1));
    let past_today = SummaryRequest {
        today: ymd(2026, 9, 30),
        ..q.clone()
    };
    assert!(query_summary(&storage, &past_today)
        .unwrap()
        .periods
        .iter()
        .all(|period| !period.in_progress));
    let retained = SummaryRequest {
        retention_cutoff: Some(ymd(2026, 10, 2)),
        ..q.clone()
    };
    assert!(query_summary(&storage, &retained).unwrap().periods[0].partial_history);
    let original = query_summary(&storage, &q).unwrap();
    assert!(original.periods[1].in_progress);
    assert!(!original.periods[0].partial_history);
    let other = SummaryRequest {
        filters: Filters {
            instances: Some(vec!["b".into()]),
            models: vec!["MODEL_X".into()],
            ..Default::default()
        },
        ..q
    };
    let result = query_summary(&storage, &other).unwrap();
    assert_eq!(result.totals.total_tokens_known, Some(55));
    assert_eq!(result.distinct_sessions, Some(1));
}

#[test]
fn own_and_external_writes_invalidate_results_even_without_a_revision_change() {
    let (_dir, writer) = temp_storage("query-cache-invalidated");
    let at = ts("2026-10-01T12:00:00Z");
    commit_batch(
        &writer,
        &batch(
            "a",
            "UTC",
            at,
            vec![with_tokens(evt("a", "one", at), 100, 10)],
        ),
        None,
    )
    .unwrap();
    let q = request();
    let reader = Storage::open_readonly(writer.path()).unwrap();
    let original = query_summary(&reader, &q).unwrap();
    assert_eq!(original.totals.total_tokens_known, Some(110));
    assert_eq!(
        query_summary(&writer, &q)
            .unwrap()
            .totals
            .total_tokens_known,
        Some(110)
    );
    writer
        .conn()
        .execute("UPDATE daily_usage SET total_known_sum=777", [])
        .unwrap();
    assert_eq!(writer.data_revision().unwrap(), original.data_revision);
    assert_eq!(
        query_summary(&writer, &q)
            .unwrap()
            .totals
            .total_tokens_known,
        Some(777)
    );
    assert_eq!(
        query_summary(&reader, &q)
            .unwrap()
            .totals
            .total_tokens_known,
        Some(777)
    );
    writer
        .conn()
        .execute("UPDATE daily_usage SET sealed=1", [])
        .unwrap();
    assert_eq!(query_summary(&reader, &q).unwrap().distinct_sessions, None);
    assert!(query_summary(&reader, &q).unwrap().periods[0].partial_history);
}

#[test]
fn projections_preserve_cross_day_identity_filters_rollbacks_and_old_database_repair() {
    let (_dir, writer) = temp_storage("query-projections");
    for (source, key, at, session, duration) in [
        ("a", "one", "2026-10-01T12:00:00Z", Some("same"), Some(10)),
        ("a", "two", "2026-10-02T12:00:00Z", Some("same"), Some(20)),
        ("b", "three", "2026-10-02T12:00:00Z", Some("same"), Some(30)),
        ("b", "four", "2026-10-02T12:00:01Z", Some(""), Some(-1)),
    ] {
        let mut e = with_tokens(evt(source, key, ts(at)), 100, 10);
        e.session_id = session.map(String::from);
        e.duration_ms = duration.filter(|d| *d >= 0);
        commit_batch(&writer, &batch(source, "UTC", ts(at), vec![e]), None).unwrap();
    }
    let reader = Storage::open_readonly(writer.path()).unwrap();
    let q = request();
    let accelerated = query_summary(&reader, &q).unwrap();
    assert_eq!(accelerated.distinct_sessions, None);
    assert_eq!(accelerated.totals.total_duration_ms, Some(60));
    assert_eq!(accelerated.periods[0].distinct_sessions, Some(1));
    let own = SummaryRequest {
        filters: Filters {
            instances: Some(vec!["a".into()]),
            ..Default::default()
        },
        ..q.clone()
    };
    assert_eq!(
        query_summary(&reader, &own).unwrap().distinct_sessions,
        Some(1)
    );
    let tx = writer.conn().unchecked_transaction().unwrap();
    tx.execute("UPDATE daily_usage SET total_known_sum=999", [])
        .unwrap();
    tx.rollback().unwrap();
    assert_eq!(
        query_summary(&reader, &q).unwrap().totals,
        accelerated.totals
    );
    writer
        .conn()
        .execute(
            "UPDATE usage_events SET duration_ms=100 WHERE source_record_key='one'",
            [],
        )
        .unwrap();
    assert_eq!(
        query_summary(&reader, &q).unwrap().totals.total_duration_ms,
        Some(150)
    );
    // An older writer may not know these tables. Invalidation still protects reads,
    // and opening the current writer repairs all invalid days without changing revision.
    let revision = writer.data_revision().unwrap();
    drop(writer);
    let writer = Storage::open(reader.path()).unwrap();
    assert_eq!(writer.data_revision().unwrap(), revision);
    let projected = query_summary(&reader, &q).unwrap();
    writer
        .conn()
        .execute("UPDATE query_accel_days SET valid=0", [])
        .unwrap();
    let original = query_summary(&reader, &q).unwrap();
    assert_eq!(projected.totals, original.totals);
    assert_eq!(projected.distinct_sessions, original.distinct_sessions);
    assert_eq!(
        projected
            .periods
            .iter()
            .map(|p| (&p.label, &p.sums, p.distinct_sessions))
            .collect::<Vec<_>>(),
        original
            .periods
            .iter()
            .map(|p| (&p.label, &p.sums, p.distinct_sessions))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        projected
            .model_breakdown
            .iter()
            .map(|m| (&m.model_raw, &m.sums))
            .collect::<Vec<_>>(),
        original
            .model_breakdown
            .iter()
            .map(|m| (&m.model_raw, &m.sums))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        projected
            .agent_breakdown
            .iter()
            .map(|a| (&a.agent, &a.sums))
            .collect::<Vec<_>>(),
        original
            .agent_breakdown
            .iter()
            .map(|a| (&a.agent, &a.sums))
            .collect::<Vec<_>>()
    );
}

#[test]
fn projection_invalidation_discards_retained_identities_transactionally() {
    let (_dir, writer) = temp_storage("query-projection-retention");
    let at = ts("2026-10-01T12:00:00Z");
    let mut e = with_tokens(evt("a", "one", at), 10, 5);
    e.session_id = Some("must-be-deleted".into());
    commit_batch(&writer, &batch("a", "UTC", at, vec![e]), None).unwrap();
    let count = || {
        writer
            .conn()
            .query_row("SELECT COUNT(*) FROM query_session_days", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(count(), 1);
    let tx = writer.conn().unchecked_transaction().unwrap();
    tx.execute("DELETE FROM usage_events", []).unwrap();
    assert_eq!(count(), 0);
    tx.rollback().unwrap();
    assert_eq!(count(), 1);
    writer
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    assert_eq!(count(), 0);
    for table in [
        "query_rollup_day",
        "query_rollup_agent",
        "query_rollup_model",
        "query_rollup_provider_day",
        "query_rollup_provider_agent",
        "query_provider_session_days",
    ] {
        assert_eq!(
            writer
                .conn()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn optional_projection_overflow_does_not_reject_individually_valid_events() {
    let (_dir, writer) = temp_storage("query-projection-overflow");
    let at = ts("2026-10-01T12:00:00Z");
    for model in ["a", "b", "c"] {
        let mut e = with_tokens(evt("source", model, at), i64::MAX / 2, 0);
        e.model_raw = Some(model.into());
        commit_batch(&writer, &batch("source", "UTC", at, vec![e]), None).unwrap();
    }
    assert_eq!(
        writer
            .conn()
            .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        writer
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM query_accel_days WHERE valid=1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let own = SummaryRequest {
        last_day: ymd(2026, 10, 1),
        filters: Filters {
            models: vec!["a".into()],
            ..Default::default()
        },
        ..request()
    };
    assert_eq!(
        query_summary(&writer, &own)
            .unwrap()
            .totals
            .input_total_known,
        Some(i64::MAX / 2)
    );
    drop(writer);
    // Writer repair also tolerates the optional overflow.
    Storage::open(&_dir.db_path()).unwrap();
}

#[test]
fn provider_projections_merge_sessions_and_preserve_unknowns_and_layout_repair() {
    let (dir, writer) = temp_storage("query-provider-projections");
    for (key, source, provider, day, duration) in [
        ("one", "a", Some("OpenAI"), "2026-10-01T12:00:00Z", 10),
        ("two", "a", Some("Anthropic"), "2026-10-01T13:00:00Z", 20),
        ("three", "a", Some("openai"), "2026-10-02T12:00:00Z", 30),
        ("four", "b", None, "2026-10-02T12:00:00Z", 40),
    ] {
        let mut e = with_tokens(evt(source, key, ts(day)), 10, 5);
        e.provider_id = provider.map(str::to_owned);
        e.session_id = Some("same".into());
        e.duration_ms = Some(duration);
        commit_batch(&writer, &batch(source, "UTC", ts(day), vec![e]), None).unwrap();
    }
    let q = SummaryRequest {
        filters: Filters {
            providers: vec!["OPENAI".into(), "anthropic".into()],
            ..Filters::default()
        },
        ..request()
    };
    let projected = query_summary(&writer, &q).unwrap();
    assert_eq!(projected.totals.call_count, 3);
    assert_eq!(projected.totals.total_tokens_known, Some(45));
    assert_eq!(projected.totals.total_duration_ms, Some(60));
    assert_eq!(projected.distinct_sessions, Some(1));
    assert_eq!(projected.periods[0].distinct_sessions, Some(1));
    assert_eq!(projected.model_breakdown.len(), 2);
    let single = SummaryRequest {
        filters: Filters {
            providers: vec!["OPENAI".into()],
            ..Filters::default()
        },
        ..request()
    };
    let single_result = query_summary(&writer, &single).unwrap();
    assert_eq!(single_result.totals.call_count, 2);
    assert_eq!(single_result.distinct_sessions, Some(1));
    assert_eq!(single_result.totals.total_duration_ms, Some(40));
    let unknown = SummaryRequest {
        filters: Filters {
            providers: vec!["unknown".into()],
            ..Filters::default()
        },
        ..request()
    };
    let unknown_result = query_summary(&writer, &unknown).unwrap();
    assert_eq!(unknown_result.totals.call_count, 1);
    assert_eq!(unknown_result.totals.total_duration_ms, Some(40));
    assert_eq!(unknown_result.distinct_sessions, Some(1));
    let empty = SummaryRequest {
        filters: Filters {
            providers: vec!["openai".into()],
            instances: Some(vec![]),
            ..Filters::default()
        },
        ..request()
    };
    assert_eq!(query_summary(&writer, &empty).unwrap().totals.call_count, 0);
    writer
        .conn()
        .execute("DELETE FROM query_accel_meta", [])
        .unwrap();
    let original = query_summary(&writer, &q).unwrap();
    assert_eq!(projected.totals, original.totals);
    assert_eq!(projected.distinct_sessions, original.distinct_sessions);
    assert_eq!(
        query_summary(&writer, &unknown).unwrap().totals,
        unknown_result.totals
    );
    let revision = writer.data_revision().unwrap();
    drop(writer);
    let writer = Storage::open(&dir.db_path()).unwrap();
    assert_eq!(writer.data_revision().unwrap(), revision);
    assert_eq!(query_summary(&writer, &q).unwrap().totals, projected.totals);
    assert_eq!(
        query_summary(&writer, &q).unwrap().distinct_sessions,
        Some(1)
    );
    writer
        .conn()
        .execute(
            "UPDATE usage_events SET provider_id='changed' WHERE source_record_key='one'",
            [],
        )
        .unwrap();
    assert_eq!(
        query_summary(&writer, &q).unwrap().totals.total_duration_ms,
        Some(50)
    );
}

#[test]
fn projection_days_preserve_dst_gap_fold_and_invalidate_moved_events() {
    let (_dir, mut writer) = temp_storage("query-projection-dst");
    for (key, at) in [
        ("gap-before", "2026-03-08T06:59:59Z"),
        ("gap-after", "2026-03-08T07:00:00Z"),
        ("fold-first", "2026-11-01T05:30:00Z"),
        ("fold-second", "2026-11-01T06:30:00Z"),
    ] {
        let mut e = with_tokens(evt("a", key, ts(at)), 10, 5);
        e.duration_ms = Some(10);
        e.session_id = Some("same".into());
        commit_batch(
            &writer,
            &batch("a", "America/New_York", ts(at), vec![e]),
            None,
        )
        .unwrap();
    }
    for day in [ymd(2026, 3, 8), ymd(2026, 11, 1)] {
        let q = SummaryRequest {
            timezone: "America/New_York".into(),
            first_day: day,
            last_day: day,
            today: day,
            ..request()
        };
        let projected = query_summary(&writer, &q).unwrap();
        assert_eq!(projected.totals.call_count, 2);
        assert_eq!(projected.totals.total_duration_ms, Some(20));
        assert_eq!(projected.distinct_sessions, Some(1));
        let tx = writer.conn().unchecked_transaction().unwrap();
        tx.execute("UPDATE query_accel_days SET valid=0", [])
            .unwrap();
        tx.commit().unwrap();
        let original = query_summary(&writer, &q).unwrap();
        assert_eq!(projected.totals, original.totals);
        writer = Storage::open(&_dir.db_path()).unwrap();
    }
    writer
        .conn()
        .execute(
            "UPDATE usage_events SET occurred_at_ms=?1 WHERE source_record_key='gap-before'",
            [ts("2026-11-01T06:00:00Z")],
        )
        .unwrap();
    assert_eq!(
        writer
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM query_accel_days WHERE valid=0",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2,
        "old and new calendar days must both invalidate"
    );
}
