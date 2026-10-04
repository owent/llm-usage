//! validation.md「固定数学样本」11 组：期望值写死，不调用实现生成期望。

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::adapters::codex::{map_codex, CodexUsage};
use llm_usage_core::aggregates::{
    observe_cumulative, sum_exclusive_aggregates, upsert_source_aggregate, AggregateScope,
    Coverage, CumulativeOutcome, SourceAggregateInput,
};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::{FieldQuality, TimeBasis, TokenQuality, TokenUsage};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::metrics::{cache_input_ratio, input_total, total_tokens};
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

/// 样本 1：普通输入 100，缓存读 800，缓存写 100，输出 100
/// → 总输入 1000，总 token 1100，缓存输入占比 80%。
#[test]
fn sample1_mutually_exclusive_parts() {
    let usage = TokenUsage {
        input_uncached: Some(100),
        input_cache_read: Some(800),
        input_cache_write: Some(100),
        input_total: None,
        output_total: Some(100),
        ..TokenUsage::default()
    };
    let quality = TokenQuality {
        input_uncached: FieldQuality::Reported,
        input_cache_read: FieldQuality::Reported,
        input_cache_write: FieldQuality::Reported,
        output_total: FieldQuality::Reported,
        ..TokenQuality::default()
    };
    assert_eq!(input_total(&usage, &quality).map(|(v, _)| v), Some(1000));
    assert_eq!(total_tokens(&usage, &quality).map(|(v, _)| v), Some(1100));
    let (ratio, n) = cache_input_ratio(&[(Some(1000), Some(800))]);
    assert_eq!(n, 1);
    let ratio = ratio.unwrap();
    assert_eq!((ratio.numerator, ratio.denominator), (800, 1000));
    assert!((ratio.as_f64() - 0.80).abs() < 1e-12);
}

/// 样本 2：总输入 1000（缓存读 800 已包含），输出 100（推理 40 已包含）
/// → 总 token 1100，不再加缓存或推理；可证明无缓存创建时普通输入为 200。
#[test]
fn sample2_inclusive_input_no_double_count() {
    let mapped = map_codex(&CodexUsage {
        input_tokens: 1000,
        cached_input_tokens: 800,
        output_tokens: 100,
        reasoning_output_tokens: 40,
        total_tokens: 1100,
        declares_no_cache_creation: true,
    });
    assert_eq!(mapped.usage.input_uncached, Some(200));
    assert_eq!(mapped.usage.input_total, Some(1000));
    assert_eq!(mapped.usage.output_total, Some(100));
    assert_eq!(
        total_tokens(&mapped.usage, &mapped.quality).map(|(v, _)| v),
        Some(1100)
    );
    // 绝不是 1940（重复相加缓存/推理）。
    assert_ne!(
        total_tokens(&mapped.usage, &mapped.quality).map(|(v, _)| v),
        Some(1940)
    );
    assert!(mapped.diagnostics.is_empty());
}

/// 样本 3：甲输入 100/缓存 90，乙输入 900/缓存 90 → 合并比例 18%，不是 50%。
#[test]
fn sample3_weighted_ratio_not_average() {
    let (merged, n) = cache_input_ratio(&[(Some(100), Some(90)), (Some(900), Some(90))]);
    assert_eq!(n, 2);
    let merged = merged.unwrap();
    assert_eq!((merged.numerator, merged.denominator), (180, 1000));
    assert!((merged.as_f64() - 0.18).abs() < 1e-12);
    // 分记录：甲 90%，乙 10%；算术平均会是 50% —— 约定禁止。
    let (a, _) = cache_input_ratio(&[(Some(100), Some(90))]);
    let (b, _) = cache_input_ratio(&[(Some(900), Some(90))]);
    assert!((a.unwrap().as_f64() - 0.90).abs() < 1e-12);
    assert!((b.unwrap().as_f64() - 0.10).abs() < 1e-12);
    assert!(((a.unwrap().as_f64() + b.unwrap().as_f64()) / 2.0 - 0.50).abs() < 1e-12);
    assert!((merged.as_f64() - 0.50).abs() > 1e-12);
}

/// 样本 4：输入 100、输出未知 → 已知输入 100；完整总量未知，不产生 total=100。
#[test]
fn sample4_unknown_output_not_a_total() {
    // 纯函数层。
    let usage = TokenUsage {
        input_total: Some(100),
        ..TokenUsage::default()
    };
    let quality = TokenQuality {
        input_total: FieldQuality::Reported,
        ..TokenQuality::default()
    };
    assert_eq!(total_tokens(&usage, &quality), None);

    // 入库层：日汇总 input 已知 100，输出/总量保持未知（NULL），不补零。
    let (_dir, storage) = temp_storage("sample4");
    let mut e = evt("inst", "k1", ts("2026-09-24T12:00:00Z"));
    e.usage.input_total = Some(100);
    e.quality.input_total = FieldQuality::Reported;
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T13:00:00Z"), vec![e]),
        None,
    )
    .unwrap();
    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 24),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.input_total_known, Some(100));
    assert_eq!(sums.output_total_known, None);
    assert_eq!(sums.total_tokens_known, None);
    assert_eq!(sums.input_known_count, 1);
    assert_eq!(sums.output_unknown_count, 1);
}

/// 样本 5：同请求 usage 从 100 改成 80 → 当前值 80，调用数 1；不是 180 或 MAX=100。
#[test]
fn sample5_correction_replaces_old_contribution() {
    let (_dir, storage) = temp_storage("sample5");
    let mut e1 = with_tokens(evt("inst", "req-1", ts("2026-09-24T10:00:00Z")), 100, 0);
    e1.source_revision = Some(1);
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T10:01:00Z"), vec![e1]),
        None,
    )
    .unwrap();

    let mut e2 = with_tokens(evt("inst", "req-1", ts("2026-09-24T10:00:00Z")), 80, 0);
    e2.source_revision = Some(2);
    e2.lifecycle = llm_usage_core::domain::Lifecycle::Corrected;
    let outcome = commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T10:02:00Z"), vec![e2]),
        None,
    )
    .unwrap();
    assert_eq!(outcome.updated, 1);

    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 24),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.input_total_known, Some(80));
    assert_eq!(sums.call_count, 1);
}

/// 样本 6：两个稳定 ID 的请求 usage 都为 100 → 总量 200，调用数 2；内容相同不能去重。
#[test]
fn sample6_identical_content_different_ids_counts_twice() {
    let (_dir, storage) = temp_storage("sample6");
    let ms = ts("2026-09-24T10:00:00Z");
    let e1 = with_tokens(evt("inst", "req-a", ms), 100, 0);
    let e2 = with_tokens(evt("inst", "req-b", ms), 100, 0);
    let outcome = commit_batch(
        &storage,
        &batch("inst", "UTC", ms + 1000, vec![e1, e2]),
        None,
    )
    .unwrap();
    assert_eq!(outcome.added, 2);
    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 24),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(summary.periods[0].sums.input_total_known, Some(200));
    assert_eq!(summary.periods[0].sums.call_count, 2);
}

/// 样本 7：DSH 同 attempt 流式 80→final 100，retry final 40 → 合计 140。
#[test]
fn sample7_dsh_attempt_stream_then_retry() {
    let (_dir, storage) = temp_storage("sample7");
    let base = ts("2026-09-24T10:00:00Z");
    let mut partial = with_tokens(evt("inst", "attempt-1", base), 80, 0);
    partial.lifecycle = llm_usage_core::domain::Lifecycle::Partial;
    partial.attempt_id = Some("attempt-1".into());
    let mut fin = with_tokens(evt("inst", "attempt-1", base), 100, 0);
    fin.attempt_id = Some("attempt-1".into());
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 1000, vec![partial]),
        None,
    )
    .unwrap();
    // final 替换同 attempt 的流式值。
    let out = commit_batch(
        &storage,
        &batch("inst", "UTC", base + 2000, vec![fin]),
        None,
    )
    .unwrap();
    assert_eq!(out.updated, 1);
    // retry 边界产生新尝试。
    let mut retry = with_tokens(evt("inst", "attempt-2", base + 5000), 40, 0);
    retry.attempt_id = Some("attempt-2".into());
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 6000, vec![retry]),
        None,
    )
    .unwrap();

    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 24),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(summary.periods[0].sums.input_total_known, Some(140));
    assert_eq!(summary.periods[0].sums.call_count, 2);
}

/// 样本 8：累计 100→150→150，明确新进程 20 → 区间增量 50、0 与新进程 20；
/// 首次 100 保留原始区间，不硬塞进今天。
#[test]
fn sample8_cumulative_deltas_and_reset() {
    let t1 = ts("2026-09-23T08:00:00Z");
    let t2 = ts("2026-09-24T08:00:00Z");
    let t3 = ts("2026-09-24T09:00:00Z");
    let t4 = ts("2026-09-24T10:00:00Z");

    let (state, out1) = observe_cumulative("series-1", None, 100, t1, false);
    assert_eq!(
        out1,
        CumulativeOutcome::FirstObservation { native_total: 100 }
    );
    let (state, out2) = observe_cumulative("series-1", Some(&state), 150, t2, false);
    assert_eq!(out2, CumulativeOutcome::Delta { amount: 50 });
    let (state, out3) = observe_cumulative("series-1", Some(&state), 150, t3, false);
    assert_eq!(out3, CumulativeOutcome::Delta { amount: 0 });
    // 已确认是新进程：重置为新基线 20（新区间量 20）。
    let (_state, out4) = observe_cumulative("series-1", Some(&state), 20, t4, true);
    assert_eq!(out4, CumulativeOutcome::Reset { new_baseline: 20 });
    // 无法确认发生重置的下降：不按零重新累加。
    let (state_x, _) = observe_cumulative("series-x", None, 100, t1, false);
    let (_s, out_y) = observe_cumulative("series-x", Some(&state_x), 30, t2, false);
    assert_eq!(
        out_y,
        CumulativeOutcome::Regression {
            previous: 100,
            observed: 30
        }
    );

    // 首次 100 保存为源原生区间总量（起点未知），不进入日汇总。
    let (_dir, storage) = temp_storage("sample8");
    let changed = upsert_source_aggregate(
        &storage,
        &SourceAggregateInput {
            instance_id: "inst".into(),
            scope: AggregateScope::ProcessSeries,
            scope_key: "series-1:first".into(),
            interval_start_ms: None,
            interval_end_ms: t1,
            interval_end_inclusive: true,
            usage: TokenUsage {
                total_tokens: Some(100),
                ..TokenUsage::default()
            },
            quality: TokenQuality {
                total_tokens: FieldQuality::Reported,
                ..TokenQuality::default()
            },
            reported_call_count: None,
            coverage: Coverage::OverlapUnknown,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: None,
        },
        t1,
    )
    .unwrap();
    assert!(changed);
    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 23),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    // 没有任何逐次事件：日汇总为空，100 不被塞进今天。
    assert!(summary.periods.is_empty());
    assert_eq!(summary.totals.event_count, 0);
}

/// 样本 9：跨两天同一个 session，各日都活动 → 周/月 DISTINCT session=1；活跃天数=2。
#[test]
fn sample9_distinct_session_across_days() {
    let (_dir, storage) = temp_storage("sample9");
    let mut e1 = with_tokens(evt("inst", "k1", ts("2026-09-23T12:00:00Z")), 10, 5);
    e1.session_id = Some("sess-1".into());
    let mut e2 = with_tokens(evt("inst", "k2", ts("2026-09-24T12:00:00Z")), 20, 5);
    e2.session_id = Some("sess-1".into());
    commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T13:00:00Z"), vec![e1, e2]),
        None,
    )
    .unwrap();

    for granularity in [Granularity::Week, Granularity::Month] {
        let summary = query_summary(
            &storage,
            &SummaryRequest {
                timezone: "UTC".into(),
                week_start: WeekStart::Monday,
                first_day: ymd(2026, 9, 21),
                last_day: ymd(2026, 9, 30),
                granularity,
                filters: Filters::default(),
                today: ymd(2026, 9, 24),
                retention_cutoff: None,
            },
        )
        .unwrap();
        assert_eq!(summary.periods.len(), 1);
        let period = &summary.periods[0];
        assert_eq!(period.distinct_sessions, Some(1));
        assert_eq!(period.active_days, Some(2));
    }
}

/// 样本 10：Hermes 两日累计行 token=1000、api_call_count=3，只有 first_seen/last_seen
/// → 保存来源区间汇总；不产生三条 model_call；不把 1000 全放最后一天；重复扫描不增加。
#[test]
fn sample10_hermes_interval_aggregate() {
    let (_dir, storage) = temp_storage("sample10");
    let first_seen = ts("2026-09-22T10:00:00Z");
    let last_seen = ts("2026-09-23T18:00:00Z");
    let input = SourceAggregateInput {
        instance_id: "hermes".into(),
        scope: AggregateScope::Custom,
        scope_key: "hermes-row-1".into(),
        interval_start_ms: Some(first_seen),
        interval_end_ms: last_seen,
        interval_end_inclusive: true,
        usage: TokenUsage {
            total_tokens: Some(1000),
            ..TokenUsage::default()
        },
        quality: TokenQuality {
            total_tokens: FieldQuality::Reported,
            ..TokenQuality::default()
        },
        reported_call_count: Some(3),
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::Uncertain,
        source_revision: None,
    };
    assert!(upsert_source_aggregate(&storage, &input, last_seen).unwrap());
    // 重复扫描幂等。
    assert!(!upsert_source_aggregate(&storage, &input, last_seen + 1000).unwrap());

    let totals = sum_exclusive_aggregates(&storage, "hermes").unwrap();
    assert_eq!(totals.total_tokens, Some(1000));
    assert_eq!(totals.reported_call_count, Some(3));

    // 不产生逐次 model_call，也不把 1000 放到最后一天的日汇总。
    let summary = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 22),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Day,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(summary.totals.event_count, 0);
    assert_eq!(summary.totals.call_count, 0);
    assert_eq!(summary.totals.total_tokens_known, None);
}

/// 样本 11：Hermes 主模型累计 100，独立 task 辅助累计 20，sessions 主循环也是 100
/// → 已证明覆盖互斥时总量 120，不是 220；session 与模型表不双计。
#[test]
fn sample11_disjoint_coverage_sums_to_120() {
    let (_dir, storage) = temp_storage("sample11");
    let t0 = ts("2026-09-20T00:00:00Z");
    let t1 = ts("2026-09-24T00:00:00Z");
    let mk = |key: &str, total: i64, coverage: Coverage, dup: Option<&str>| SourceAggregateInput {
        instance_id: "hermes".into(),
        scope: AggregateScope::ModelSeries,
        scope_key: key.into(),
        interval_start_ms: Some(t0),
        interval_end_ms: t1,
        interval_end_inclusive: false,
        usage: TokenUsage {
            total_tokens: Some(total),
            ..TokenUsage::default()
        },
        quality: TokenQuality {
            total_tokens: FieldQuality::Reported,
            ..TokenQuality::default()
        },
        reported_call_count: None,
        coverage,
        duplicate_of: dup.map(str::to_string),
        time_basis: TimeBasis::Uncertain,
        source_revision: None,
    };
    upsert_source_aggregate(
        &storage,
        &mk("model-main", 100, Coverage::Exclusive, None),
        t1,
    )
    .unwrap();
    upsert_source_aggregate(&storage, &mk("task-aux", 20, Coverage::Exclusive, None), t1).unwrap();
    // sessions 主循环与 model-main 覆盖相同：对照不叠加。
    upsert_source_aggregate(
        &storage,
        &mk(
            "sessions-main-loop",
            100,
            Coverage::Duplicate,
            Some("model-main"),
        ),
        t1,
    )
    .unwrap();

    let totals = sum_exclusive_aggregates(&storage, "hermes").unwrap();
    assert_eq!(totals.total_tokens, Some(120));
    assert_ne!(totals.total_tokens, Some(220));
    assert_eq!(totals.exclusive_rows, 2);
    assert_eq!(totals.duplicate_rows, 1);
}
