//! Selection covers the displayed local hours, with global session deduplication.
mod common;
use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::{
    calendar::{ymd, WeekStart},
    ingest::commit_batch,
    query::{query_summary_selected, Filters, Granularity, SummaryRequest},
};

#[test]
fn bounded_export_probe_keeps_partial_lines_for_the_next_read() {
    use llm_usage_core::adapters::jsonl::{read_jsonl_with_byte_budget, JsonlLimits, StopReason};
    let dir = common::TempDir::new("bounded-export");
    let file = dir.path().join("events.jsonl");
    std::fs::write(&file, b"{\"n\":1}\n{\"n\":2}\n").unwrap();
    let first =
        read_jsonl_with_byte_budget(&file, 0, 1, &JsonlLimits::default(), Some(10)).unwrap();
    assert_eq!(first.lines.len(), 1);
    assert_eq!(first.next_offset, 8);
    assert_eq!(first.pending_bytes, 2);
    assert_eq!(first.stop, StopReason::LineBudget);
    let second = read_jsonl_with_byte_budget(
        &file,
        first.next_offset,
        first.next_line_number,
        &JsonlLimits::default(),
        Some(8),
    )
    .unwrap();
    assert_eq!(second.lines[0].text, "{\"n\":2}");
    assert_eq!(second.next_offset, 16);
}

#[test]
fn selected_hours_preserve_unknown_fields_durations_filters_and_sessions() {
    let (_dir, storage) = temp_storage("selected-hours");
    let mut events = Vec::new();
    for (index, hour) in [8, 9, 10].into_iter().enumerate() {
        let mut event = evt(
            "local",
            &format!("call-{index}"),
            ts(&format!("2026-10-02T{hour:02}:00:00Z")),
        );
        event.session_id = Some(if index < 2 { "shared" } else { "other" }.into());
        event.duration_ms = Some((index as i64 + 1) * 100);
        if index < 2 {
            event = with_tokens(event, 100, 20);
        }
        events.push(event);
    }
    let now = ts("2026-10-02T12:00:00Z");
    commit_batch(&storage, &batch("local", "UTC", now, events), None).unwrap();
    let request = SummaryRequest {
        timezone: "UTC".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 10, 2),
        last_day: ymd(2026, 10, 2),
        granularity: Granularity::Hour,
        filters: Filters::default(),
        today: ymd(2026, 10, 2),
        retention_cutoff: None,
    };
    let selected = query_summary_selected(
        &storage,
        &request,
        Some(("2026-10-02 08:00", "2026-10-02 09:00")),
    )
    .unwrap();
    assert_eq!(selected.totals.call_count, 2);
    assert_eq!(selected.totals.total_tokens_known, Some(240));
    assert_eq!(selected.distinct_sessions, Some(1));
    assert_eq!(selected.totals.avg_duration_ms, Some(150));
    let unknown = query_summary_selected(
        &storage,
        &request,
        Some(("2026-10-02 10:00", "2026-10-02 10:00")),
    )
    .unwrap();
    assert_eq!(unknown.totals.call_count, 1);
    assert_eq!(unknown.totals.total_tokens_known, None);
    let filtered = SummaryRequest {
        filters: Filters {
            agents: vec!["another".into()],
            ..Default::default()
        },
        ..request
    };
    assert_eq!(
        query_summary_selected(
            &storage,
            &filtered,
            Some(("2026-10-02 08:00", "2026-10-02 09:00"))
        )
        .unwrap()
        .totals
        .call_count,
        0
    );
}

#[test]
fn repeated_dst_hour_matches_the_single_displayed_label() {
    let (_dir, storage) = temp_storage("selected-dst");
    let now = ts("2026-11-01T12:00:00Z");
    let events = [
        "2026-11-01T05:30:00Z",
        "2026-11-01T06:30:00Z",
        "2026-11-01T07:30:00Z",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, time)| with_tokens(evt("local", &format!("dst-{i}"), ts(time)), 100, 20))
    .collect();
    commit_batch(
        &storage,
        &batch("local", "America/New_York", now, events),
        None,
    )
    .unwrap();
    let request = SummaryRequest {
        timezone: "America/New_York".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 11, 1),
        last_day: ymd(2026, 11, 1),
        granularity: Granularity::Hour,
        filters: Filters::default(),
        today: ymd(2026, 11, 1),
        retention_cutoff: None,
    };
    let selected = query_summary_selected(
        &storage,
        &request,
        Some(("2026-11-01 01:00", "2026-11-01 01:00")),
    )
    .unwrap();
    assert_eq!(selected.totals.call_count, 2);
    assert_eq!(selected.periods.len(), 1);
    assert_eq!(selected.totals.total_tokens_known, Some(240));
    use llm_usage_core::{
        pricing::{parse_snapshot_json, EstimateOptions},
        storage::pricing::{CostFilters, CostSummaryRequest},
    };
    let prices = r#"{"format":"llm-usage-price-snapshot/1","snapshot":{"id":"dst-prices","source_type":"manual","fetched_at":"2026-10-01"},"rows":[{"price_id":"dst","provider_id":"prov","model":"m","region":"global","channel":"api","currency":"USD","effective_from":"2026-10-01","input":1000000,"output":2000000}]}"#;
    storage
        .import_price_snapshot(&parse_snapshot_json(prices).unwrap(), now)
        .unwrap();
    let mut options = EstimateOptions::default();
    options
        .provider_channels
        .insert("prov".into(), ("global".into(), "api".into()));
    let cost_request = CostSummaryRequest {
        timezone: request.timezone,
        first_day: "2026-11-01".into(),
        last_day: "2026-11-01".into(),
        now_ms: now,
        options,
        filters: CostFilters::default(),
    };
    let cost = storage
        .cost_summary_selected(
            &cost_request,
            Some(("2026-11-01 01:00", "2026-11-01 01:00")),
        )
        .unwrap();
    assert_eq!(
        cost.current_sim.rows[0].priced_event_count, 2,
        "both DST folds match the selected local hour"
    );
    assert_eq!(
        cost.current_sim.rows[0].total_amount_minor, 1,
        "both DST folds accumulate before rounding the selected day/model"
    );
}
