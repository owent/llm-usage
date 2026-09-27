mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::parse_date;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{heatmap_cells, Filters};

#[test]
fn heatmap_keeps_dates_separate_and_reads_retained_daily_totals() {
    let (_dir, storage) = temp_storage("heatmap-daily");
    let events = [
        ("a", "2026-09-14T23:30:00Z", 110),
        ("b", "2026-09-21T23:30:00Z", 220),
    ]
    .into_iter()
    .map(|(key, time, total)| {
        let mut event = with_tokens(evt("codex@a", key, ts(time)), total - 10, 10);
        event.agent = "Codex".into();
        event.model_raw = Some("GPT-5".into());
        event
    })
    .collect();
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "Asia/Shanghai",
            ts("2026-09-23T00:00:00Z"),
            events,
        ),
        None,
    )
    .unwrap();
    // This is the ordinary state after detail retention: daily totals survive.
    storage
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    let filters = Filters {
        agents: vec!["codex".into()],
        models: vec!["gpt-5".into()],
        ..Filters::default()
    };
    let cells = heatmap_cells(
        &storage,
        "Asia/Shanghai",
        parse_date("2026-09-15").unwrap(),
        parse_date("2026-09-22").unwrap(),
        &filters,
    )
    .unwrap();
    assert_eq!(cells.len(), 8);
    assert_eq!(
        (
            &cells[0].day,
            cells[0].weekday,
            cells[0].call_count,
            cells[0].total_tokens_known
        ),
        (&"2026-09-15".to_string(), 2, 1, Some(110))
    );
    assert_eq!(
        (
            &cells[7].day,
            cells[7].weekday,
            cells[7].call_count,
            cells[7].total_tokens_known
        ),
        (&"2026-09-22".to_string(), 2, 1, Some(220))
    );
    assert!(cells[1..7]
        .iter()
        .all(|c| c.available && c.call_count == 0 && c.total_tokens_known.is_none()));
    let other_user = Filters {
        instances: Some(vec![]),
        ..filters
    };
    assert!(heatmap_cells(
        &storage,
        "Asia/Shanghai",
        parse_date("2026-09-15").unwrap(),
        parse_date("2026-09-22").unwrap(),
        &other_user,
    )
    .unwrap()
    .iter()
    .all(|c| c.call_count == 0));
    storage
        .conn()
        .execute(
            "INSERT INTO settings(key, value, schema_version, updated_at_ms) VALUES (?1, ?2, 1, 0)",
            ["daily_retention_floor:Asia/Shanghai", "2026-09-20"],
        )
        .unwrap();
    let with_floor = heatmap_cells(
        &storage,
        "Asia/Shanghai",
        parse_date("2026-09-15").unwrap(),
        parse_date("2026-09-22").unwrap(),
        &Filters::default(),
    )
    .unwrap();
    assert!(with_floor[0].available && with_floor[0].partial);
    assert!(!with_floor[1].available && !with_floor[1].partial);
    assert!(with_floor[7].available && !with_floor[7].partial);
}

#[test]
fn heatmap_marks_pruned_daily_history_unavailable_instead_of_zero() {
    let (_dir, storage) = temp_storage("heatmap-coverage");
    storage
        .conn()
        .execute(
            "INSERT INTO settings(key, value, schema_version, updated_at_ms) VALUES (?1, ?2, 1, 0)",
            ["daily_retention_floor:UTC", "2026-09-20"],
        )
        .unwrap();
    let cells = heatmap_cells(
        &storage,
        "UTC",
        parse_date("2026-09-19").unwrap(),
        parse_date("2026-09-21").unwrap(),
        &Filters::default(),
    )
    .unwrap();
    assert_eq!(
        cells.iter().map(|c| c.available).collect::<Vec<_>>(),
        vec![false, true, true]
    );
}
