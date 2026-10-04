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
