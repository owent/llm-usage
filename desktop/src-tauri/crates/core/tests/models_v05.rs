//! V05：模型切换、同名不同 provider 不合并、unknown 进总计、别名不篡改原始记录。

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::ModelAttribution;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};

fn summarize(storage: &llm_usage_core::storage::Storage) -> llm_usage_core::query::Summary {
    query_summary(
        storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 20),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Week,
            filters: Filters::default(),
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap()
}

/// 模型切换：按不晚于调用的结构化证据归属；历史调用不被后来的会话当前值改写。
#[test]
fn v05_model_switch_keeps_historical_attribution() {
    let (_dir, storage) = temp_storage("v05switch");
    let base = ts("2026-09-22T10:00:00Z");
    let mut e1 = with_tokens(evt("inst", "k1", base), 100, 0);
    e1.model_raw = Some("model-old".into());
    e1.model_attribution = ModelAttribution::StructuredChange;
    let mut e2 = with_tokens(evt("inst", "k2", base + 3600_000), 200, 0);
    e2.model_raw = Some("model-new".into());
    e2.model_attribution = ModelAttribution::StructuredChange;
    commit_batch(&storage, &batch("inst", "UTC", base + 7200_000, vec![e1, e2]), None).unwrap();

    let summary = summarize(&storage);
    assert_eq!(summary.model_breakdown.len(), 2);
    let old = summary
        .model_breakdown
        .iter()
        .find(|r| r.model_raw.as_deref() == Some("model-old"))
        .unwrap();
    assert_eq!(old.sums.input_total_known, Some(100));
    let new = summary
        .model_breakdown
        .iter()
        .find(|r| r.model_raw.as_deref() == Some("model-new"))
        .unwrap();
    assert_eq!(new.sums.input_total_known, Some(200));

    // 别名变更不改写原始记录：加版本化别名后，usage_events 的 model_raw 保持原值。
    storage
        .conn()
        .execute(
            "INSERT INTO model_aliases (provider_id, model_raw, rule_version, alias, family, updated_at_ms)
             VALUES ('prov', 'model-old', 'rules-2', 'Old Model', 'family-x', 1)",
            [],
        )
        .unwrap();
    let raw: String = storage
        .conn()
        .query_row("SELECT model_raw FROM usage_events WHERE source_record_key = 'k1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, "model-old");
}

#[test]
fn v05_same_name_different_providers_not_merged() {
    let (_dir, storage) = temp_storage("v05prov");
    let base = ts("2026-09-22T10:00:00Z");
    let mut e1 = with_tokens(evt("inst", "k1", base), 100, 0);
    e1.provider_id = Some("provider-a".into());
    e1.model_raw = Some("same-name".into());
    let mut e2 = with_tokens(evt("inst", "k2", base), 300, 0);
    e2.provider_id = Some("provider-b".into());
    e2.model_raw = Some("same-name".into());
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![e1, e2]), None).unwrap();

    let summary = summarize(&storage);
    assert_eq!(summary.model_breakdown.len(), 2);
    // 总计包含两者。
    assert_eq!(summary.totals.input_total_known, Some(400));
}

#[test]
fn v05_unknown_model_in_totals_with_own_row() {
    let (_dir, storage) = temp_storage("v05unknown");
    let base = ts("2026-09-22T10:00:00Z");
    let known = with_tokens(evt("inst", "k1", base), 100, 0);
    let mut unknown = with_tokens(evt("inst", "k2", base), 50, 0);
    unknown.provider_id = None;
    unknown.model_raw = None;
    unknown.model_attribution = ModelAttribution::Unknown;
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![known, unknown]), None).unwrap();

    let summary = summarize(&storage);
    // unknown 独立行存在且总计包含其已知 token。
    let unknown_row = summary.model_breakdown.iter().find(|r| r.model_raw.is_none()).unwrap();
    assert_eq!(unknown_row.sums.input_total_known, Some(50));
    assert_eq!(summary.totals.input_total_known, Some(150));

    // 筛选某个模型时 unknown 不消失于总计口径之外——筛选只影响展示行。
    let filtered = query_summary(
        &storage,
        &SummaryRequest {
            timezone: "UTC".into(),
            week_start: WeekStart::Monday,
            first_day: ymd(2026, 9, 20),
            last_day: ymd(2026, 9, 24),
            granularity: Granularity::Week,
            filters: Filters { models: vec!["m".to_string()], ..Filters::default() },
            today: ymd(2026, 9, 24),
            retention_cutoff: None,
        },
    )
    .unwrap();
    assert_eq!(filtered.totals.input_total_known, Some(100));
}
