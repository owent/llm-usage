//! Read-only query timings; print counts and timings, never source identities.
use llm_usage_core::{
    calendar::{ymd, WeekStart},
    pricing::EstimateOptions,
    query::{heatmap_cells, query_summary, Filters, Granularity, SummaryRequest},
    storage::{
        pricing::{CostFilters, CostSummaryRequest},
        Storage,
    },
};
use std::{path::PathBuf, time::Instant};
fn main() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let storage =
        Storage::open_readonly(&repo.join("build/dashboard-repair/real-snapshot.sqlite")).unwrap();
    let request = SummaryRequest {
        timezone: "Asia/Shanghai".into(),
        week_start: WeekStart::Monday,
        first_day: ymd(2026, 9, 3),
        last_day: ymd(2026, 10, 2),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: ymd(2026, 10, 2),
        retention_cutoff: None,
    };
    for pass in 0..3 {
        let start = Instant::now();
        let cells = heatmap_cells(
            &storage,
            &request.timezone,
            request.first_day,
            request.last_day,
            &request.filters,
        )
        .unwrap();
        println!(
            "pass={pass} heatmap_ms={:.3} cells={}",
            start.elapsed().as_secs_f64() * 1000.,
            cells.len()
        );
        let start = Instant::now();
        let summary = query_summary(&storage, &request).unwrap();
        println!(
            "pass={pass} summary_ms={:.3} periods={}",
            start.elapsed().as_secs_f64() * 1000.,
            summary.periods.len()
        );
        let start = Instant::now();
        let costs = storage
            .cost_summary(&CostSummaryRequest {
                timezone: request.timezone.clone(),
                first_day: request.first_day.to_string(),
                last_day: request.last_day.to_string(),
                filters: CostFilters::default(),
                now_ms: jiff::Timestamp::now().as_millisecond(),
                options: EstimateOptions::default(),
            })
            .unwrap();
        println!(
            "pass={pass} costs_ms={:.3} currencies={}",
            start.elapsed().as_secs_f64() * 1000.,
            costs.at_time.rows.len()
        );
    }
}
