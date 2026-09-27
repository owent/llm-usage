//! 分级归档保留（2026-09-26 用户需求）：明细→小时→日→周/月/年逐级保留；
//! 日汇总删除前物化周期；查询按"日层存活走日、更早走物化"合并；
//! 小时图读持久化小时表（明细删除后仍有数据）。

mod common;

use common::*;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{heatmap_cells, query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::retention_tiered::{enforce_tiered_retention, TieredRetentionPolicy};

fn request(first: &str, last: &str, granularity: Granularity) -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".to_string(),
        week_start: llm_usage_core::calendar::WeekStart::Monday,
        first_day: llm_usage_core::calendar::parse_date(first).unwrap(),
        last_day: llm_usage_core::calendar::parse_date(last).unwrap(),
        granularity,
        filters: Filters::default(),
        today: llm_usage_core::calendar::parse_date("2026-09-26").unwrap(),
        retention_cutoff: None,
    }
}

#[test]
fn tiered_retention_prunes_by_layer_and_materializes_history() {
    let (_dir, storage) = temp_storage("tiered");
    // 2025-12-15（其周/月/年均已完成）+ 本周内 09-22/09-25。
    // 注：进行中周/月/年起点受保护（今日 2026-09-26 ⇒ 2026-01-01 起不可删），
    // 故历史数据选在 2025 年才能触发日层删除（保护语义见模块头）。
    for (key, day, tokens) in [
        ("r-old", "2025-12-15T10:00:00Z", 100i64),
        ("r-w22", "2026-09-22T10:00:00Z", 100),
        ("r-w25", "2026-09-25T10:00:00Z", 10),
    ] {
        commit_batch(
            &storage,
            &batch(
                "codex@a",
                "UTC",
                ts(day),
                vec![with_tokens(
                    evt("codex@a", key, ts(day)),
                    tokens,
                    tokens / 10,
                )],
            ),
            None,
        )
        .unwrap();
    }
    // 窗口：明细 1 天、小时 2 天、日 3 天、周/月长期。
    let policy = TieredRetentionPolicy {
        events_days: 1,
        hourly_days: 2,
        daily_days: 3,
        weekly_days: 3650,
        monthly_days: 10_950,
        yearly_days: None,
    };
    let outcome =
        enforce_tiered_retention(&storage, "UTC", ts("2026-09-26T12:00:00Z"), &policy).unwrap();
    // 明细层：全部事件早于明细下限（今天起）⇒ 全删。
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);
    assert_eq!(outcome.deleted_events, 3);
    // 小时层：<09-25 删除 ⇒ 2025-12-15/09-22 删、09-25 留。
    let hourly: Vec<(String, i64)> = storage
        .conn()
        .prepare("SELECT local_day, call_count FROM hourly_usage ORDER BY local_day")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(hourly, vec![("2026-09-25".to_string(), 1)]);
    // 日层：计算下限 09-24；年保护（2026-01-01 起）⇒ 仅 2025-12-15 删。
    let daily: Vec<String> = storage
        .conn()
        .prepare("SELECT DISTINCT local_day FROM daily_usage ORDER BY local_day")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        daily,
        vec!["2026-09-22".to_string(), "2026-09-25".to_string()]
    );
    assert_eq!(outcome.deleted_daily_rows, 1);
    // 物化：2025 的周/月/年各一行（1 调用 110）；本周/本月/本年进行中
    // 不物化（由受保护的日行服务）。
    assert_eq!(outcome.materialized_period_rows, 3, "2025 周+月+年各一行");
    for granularity in ["week", "month", "year"] {
        let row: (i64, Option<i64>) = storage
            .conn()
            .query_row(
                "SELECT call_count, total_known_sum FROM period_usage WHERE granularity = ?1",
                [granularity],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(row, (1, Some(110)), "{granularity} 行来自 2025-12-15");
    }
    // 查询合并：周粒度跨年查询 ⇒ 2025-12-15 周走物化 + 2026-09-21 周走日行。
    let s = query_summary(
        &storage,
        &request("2025-12-01", "2026-09-30", Granularity::Week),
    )
    .unwrap();
    assert_eq!(s.periods.len(), 2);
    assert_eq!(s.totals.call_count, 3, "物化 1 + 日行 2");
    assert_eq!(s.totals.total_tokens_known, Some(110 + 110 + 11));
    let materialized_period = s
        .periods
        .iter()
        .find(|p| p.start_day.to_string().starts_with("2025-"))
        .expect("materialized week row visible");
    assert!(materialized_period.partial_history);
    // 幂等：重复执行不增量。2025 日行已删 ⇒ 无可重物化（物化行原样保留）；
    // 现存 2026 日行属进行中周期 ⇒ materialized=0。
    let again =
        enforce_tiered_retention(&storage, "UTC", ts("2026-09-26T13:00:00Z"), &policy).unwrap();
    assert_eq!(again.materialized_period_rows, 0);
    let kept: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM period_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kept, 3, "物化行保留不丢");
    let s2 = query_summary(
        &storage,
        &request("2025-12-01", "2026-09-30", Granularity::Week),
    )
    .unwrap();
    assert_eq!(s2.totals.call_count, 3, "重复执行不增量");
    assert_eq!(s2.totals.total_tokens_known, Some(231));
    let daily_heatmap = heatmap_cells(
        &storage,
        "UTC",
        llm_usage_core::calendar::parse_date("2025-12-15").unwrap(),
        llm_usage_core::calendar::parse_date("2026-09-26").unwrap(),
        &Filters::default(),
    )
    .unwrap();
    assert!(!daily_heatmap[0].available, "周期归档不能假装还原出每日值");
    assert_eq!(daily_heatmap.iter().map(|c| c.call_count).sum::<i64>(), 2);
    storage
        .conn()
        .execute(
            "DELETE FROM settings WHERE key = 'daily_retention_floor:UTC'",
            [],
        )
        .unwrap();
    let imported_period_only = heatmap_cells(
        &storage,
        "UTC",
        llm_usage_core::calendar::parse_date("2025-12-15").unwrap(),
        llm_usage_core::calendar::parse_date("2025-12-15").unwrap(),
        &Filters {
            agents: vec!["AGENT-A".into()],
            models: vec!["M".into()],
            ..Filters::default()
        },
    )
    .unwrap();
    assert!(
        !imported_period_only[0].available,
        "缺少截止线的周期归档仍不能显示为零调用"
    );
}

#[test]
fn hourly_table_survives_detail_prune() {
    let (_dir, storage) = temp_storage("hourly-survive");
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-25T10:00:00Z"),
            vec![with_tokens(
                evt("codex@a", "r-h", ts("2026-09-25T10:00:00Z")),
                30,
                3,
            )],
        ),
        None,
    )
    .unwrap();
    let policy = TieredRetentionPolicy {
        events_days: 1,
        hourly_days: 30,
        ..TieredRetentionPolicy::default()
    };
    enforce_tiered_retention(&storage, "UTC", ts("2026-09-26T12:00:00Z"), &policy).unwrap();
    let events: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0, "明细已删");
    let buckets = llm_usage_core::query::hourly_breakdown(
        &storage,
        "UTC",
        llm_usage_core::calendar::parse_date("2026-09-25").unwrap(),
        &Filters::default(),
    )
    .unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].hour, 10);
    assert_eq!(buckets[0].call_count, 1);
    assert_eq!(buckets[0].total_tokens_known, Some(33));
}

#[test]
fn tiered_policy_validates_ordering() {
    // 明细比小时层长是合法冗余（不丢数据）。
    let redundant = TieredRetentionPolicy {
        events_days: 10,
        hourly_days: 5,
        ..TieredRetentionPolicy::default()
    };
    assert!(redundant.validate().is_ok());
    // 粗层短于细层非法。
    let bad = TieredRetentionPolicy {
        hourly_days: 90,
        daily_days: 30,
        ..TieredRetentionPolicy::default()
    };
    assert!(enforce_tiered_retention(&storage_none(), "UTC", 0, &bad).is_err());
}

fn storage_none() -> llm_usage_core::storage::Storage {
    // 校验在打开库之前失败即可；用内存库占位。
    llm_usage_core::storage::Storage::open_in_memory().unwrap()
}

#[test]
fn debug_hourly_rows() {
    let (_dir, storage) = temp_storage("dbg-hourly");
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-25T10:00:00Z"),
            vec![with_tokens(
                evt("codex@a", "r-dbg", ts("2026-09-25T10:00:00Z")),
                30,
                3,
            )],
        ),
        None,
    )
    .unwrap();
    let rows: Vec<(String, i64, i64, Option<i64>)> = storage
        .conn()
        .prepare("SELECT local_day, hour, call_count, total_known_sum FROM hourly_usage")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    println!("hourly rows: {rows:?}");
}
