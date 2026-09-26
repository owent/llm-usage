//! 诊断探针：复现 app 层 summary 命令的构建与查询步骤，定位查询失败原因。
use llm_usage_core::calendar::{parse_date, Calendar, WeekStart};
use llm_usage_core::query::{
    agent_breakdown, heatmap_cells, hourly_breakdown, query_summary, Filters, Granularity,
    SummaryRequest,
};
use llm_usage_core::storage::Storage;

fn main() {
    let db = std::env::args().nth(1).expect("db path");
    // 可选第三参 repair：<tz> 时先在用户时区重算（app init 修复路径的等价入口）。
    if let Some(mode) = std::env::args().nth(3) {
        if mode == "repair" {
            let storage = Storage::open(std::path::Path::new(&db)).unwrap();
            let (min_ms, max_ms): (i64, i64) = storage
                .conn()
                .query_row(
                    "SELECT MIN(occurred_at_ms), MAX(occurred_at_ms) FROM usage_events",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            let rev = llm_usage_core::ingest::recompute_days_in_tz(
                &storage,
                &std::env::args().nth(2).unwrap(),
                min_ms,
                max_ms,
                1_800_000_200_000,
            )
            .unwrap();
            println!("repaired, revision={rev}");
        }
    }
    let tz = std::env::args().nth(2).unwrap_or_else(|| {
        std::env::var("TZ")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| match jiff::tz::TimeZone::system().iana_name() {
                Some(n) => n.to_string(),
                None => "UTC".to_string(),
            })
    });
    println!("timezone = {tz:?}");
    match Calendar::new(&tz) {
        Ok(cal) => println!(
            "calendar ok, today={:?}",
            cal.today(1_800_000_000_000_i64).map(|d| d.to_string())
        ),
        Err(e) => {
            println!("calendar ERR: {e}");
            return;
        }
    }
    let storage = Storage::open(std::path::Path::new(&db)).unwrap();
    let request = SummaryRequest {
        timezone: tz.clone(),
        week_start: WeekStart::Monday,
        first_day: parse_date("2026-08-28").unwrap(),
        last_day: parse_date("2026-09-26").unwrap(),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: parse_date("2026-09-26").unwrap(),
        retention_cutoff: None,
    };
    match query_summary(&storage, &request) {
        Ok(s) => println!(
            "summary ok: periods={} calls={} total={:?} models={} agents={}",
            s.periods.len(),
            s.totals.call_count,
            s.totals.total_tokens_known,
            s.model_breakdown.len(),
            agent_breakdown(&storage, &request).unwrap().len()
        ),
        Err(e) => println!("summary ERR: {e}"),
    }
    match hourly_breakdown(
        &storage,
        &tz,
        parse_date("2026-09-26").unwrap(),
        &Filters::default(),
    ) {
        Ok(h) => println!("hourly ok: {} buckets", h.len()),
        Err(e) => println!("hourly ERR: {e}"),
    }
    match heatmap_cells(
        &storage,
        &tz,
        parse_date("2026-08-28").unwrap(),
        parse_date("2026-09-26").unwrap(),
        &Filters::default(),
    ) {
        Ok(c) => println!("heatmap ok: {} cells", c.len()),
        Err(e) => println!("heatmap ERR: {e}"),
    }
}
