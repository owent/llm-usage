//! 存储与作业杂项：ingest_runs 状态机、归属排除、数据修订号、
//! 接近 i64 上限、大于 JS 安全整数、毫秒/秒误判、null 传播、额度与别名。

mod common;

use common::{batch, evt, temp_storage, ts, with_tokens, TempDir};
use llm_usage_core::calendar::{ymd, WeekStart};
use llm_usage_core::domain::{
    AttributionStatus, FieldQuality, TokenQuality, TokenUsage, MAX_TOKEN_VALUE,
};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::jobs::{
    finish_run, run_status, start_run, RunStart, RunStats, RunStatus, TriggerKind,
};
use llm_usage_core::query::{list_excluded, query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::storage::Storage;

fn day_totals(storage: &Storage) -> llm_usage_core::query::Summary {
    query_summary(
        storage,
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
    .unwrap()
}

#[test]
fn jobs_state_machine_and_restart_interruption() {
    let dir = TempDir::new("jobs");
    let path = dir.db_path();
    let now = ts("2026-09-24T12:00:00Z");
    {
        let storage = Storage::open(&path).unwrap();
        // 启动作业；同源重叠触发合并进既有作业。
        let start = start_run(&storage, "run-1", "inst", TriggerKind::Manual, now).unwrap();
        assert_eq!(start, RunStart::Started("run-1".into()));
        let merged = start_run(&storage, "run-2", "inst", TriggerKind::Interval, now + 1).unwrap();
        assert_eq!(merged, RunStart::Merged("run-1".into()));
        let merged_triggers: String = storage
            .conn()
            .query_row(
                "SELECT merged_triggers FROM ingest_runs WHERE run_id = 'run-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(merged_triggers.contains("interval"));

        // 批次统计与作业进度同事务提交。
        let mut b = batch(
            "inst",
            "UTC",
            now,
            vec![with_tokens(evt("inst", "k1", now - 1000), 10, 0)],
        );
        b.run_id = Some("run-1".into());
        commit_batch(&storage, &b, None).unwrap();
        let added: i64 = storage
            .conn()
            .query_row(
                "SELECT added FROM ingest_runs WHERE run_id = 'run-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(added, 1);

        // 终态后不能再 finish。
        finish_run(
            &storage,
            "run-1",
            RunStatus::Succeeded,
            RunStats::default(),
            None,
            now + 2,
        )
        .unwrap();
        assert_eq!(
            run_status(&storage, "run-1").unwrap(),
            Some(RunStatus::Succeeded)
        );
        assert!(finish_run(
            &storage,
            "run-1",
            RunStatus::Failed,
            RunStats::default(),
            None,
            now + 3
        )
        .is_err());
        // finish 不接受非终态。
        assert!(finish_run(
            &storage,
            "run-x",
            RunStatus::Running,
            RunStats::default(),
            None,
            now
        )
        .is_err());

        // 遗留一个 running 作业，模拟进程退出。
        start_run(&storage, "run-orphan", "inst-b", TriggerKind::Interval, now).unwrap();
    }
    // 重启：running → interrupted，幂等恢复。
    {
        let storage = Storage::open(&path).unwrap();
        assert_eq!(
            run_status(&storage, "run-orphan").unwrap(),
            Some(RunStatus::Interrupted)
        );
        // 已终态的不受影响。
        assert_eq!(
            run_status(&storage, "run-1").unwrap(),
            Some(RunStatus::Succeeded)
        );
    }
}

#[test]
fn data_revision_is_monotonic_and_visible_to_queries() {
    let (_dir, storage) = temp_storage("revision");
    let now = ts("2026-09-24T12:00:00Z");
    assert_eq!(storage.data_revision().unwrap(), 0);
    let o1 = commit_batch(
        &storage,
        &batch(
            "inst",
            "UTC",
            now,
            vec![with_tokens(evt("inst", "k1", now - 1000), 1, 0)],
        ),
        None,
    )
    .unwrap();
    let o2 = commit_batch(
        &storage,
        &batch(
            "inst",
            "UTC",
            now + 1,
            vec![with_tokens(evt("inst", "k2", now - 900), 2, 0)],
        ),
        None,
    )
    .unwrap();
    assert!(o2.data_revision > o1.data_revision);
    assert_eq!(storage.data_revision().unwrap(), o2.data_revision);
    // 查询返回同一修订号视图。
    let summary = day_totals(&storage);
    assert_eq!(summary.data_revision, o2.data_revision);
    assert_eq!(summary.totals.input_total_known, Some(3));
}

#[test]
fn attribution_excluded_events_stay_out_of_totals_and_are_listable() {
    let (_dir, storage) = temp_storage("attrib");
    let base = ts("2026-09-24T10:00:00Z");
    let verified = with_tokens(evt("inst", "local-1", base), 100, 0);
    let mut remote = with_tokens(evt("inst", "remote-1", base), 900, 0);
    remote.attribution_status = AttributionStatus::Excluded;
    remote.exclusion_reason = Some("remote_sync_folder".into());
    let mut pending = with_tokens(evt("inst", "pending-1", base), 500, 0);
    pending.attribution_status = AttributionStatus::Pending;
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 1, vec![verified, remote, pending]),
        None,
    )
    .unwrap();

    let summary = day_totals(&storage);
    // 未确认归属的不进总计。
    assert_eq!(summary.totals.input_total_known, Some(100));
    assert_eq!(summary.excluded_event_count, 2);

    // 按排除原因列出。
    let (start, _) = llm_usage_core::calendar::Calendar::utc()
        .day_range_ms(ymd(2026, 9, 24))
        .unwrap();
    let excluded = list_excluded(&storage, start, start + 86_400_000).unwrap();
    assert_eq!(
        excluded,
        vec![
            ("remote_sync_folder".to_string(), 1),
            ("unverified".to_string(), 1)
        ]
    );
}

#[test]
fn token_limits_near_i64_and_js_safe_integer() {
    let (_dir, storage) = temp_storage("limits");
    let base = ts("2026-09-24T10:00:00Z");
    // 大于 JS 安全整数（2^53+1）精确往返。
    let big = 9_007_199_254_740_993i64;
    let mut e = evt("inst", "big", base);
    e.usage.input_total = Some(big);
    e.quality.input_total = FieldQuality::Reported;
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![e]), None).unwrap();
    let stored: i64 = storage
        .conn()
        .query_row(
            "SELECT input_total FROM usage_events WHERE source_record_key = 'big'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, big);
    assert_eq!(day_totals(&storage).totals.input_total_known, Some(big));

    // 上限边界：MAX_TOKEN_VALUE 接受。
    let mut at_max = evt("inst", "at-max", base);
    at_max.usage.input_total = Some(MAX_TOKEN_VALUE);
    at_max.quality.input_total = FieldQuality::Reported;
    commit_batch(
        &storage,
        &batch("inst", "UTC", base + 2, vec![at_max]),
        None,
    )
    .unwrap();

    // 聚合溢出防护：两条 MAX 记录求和超过 i64 → 报错且整体回滚，不静默回绕。
    let mut m1 = evt("inst", "m1", base);
    m1.usage.input_total = Some(MAX_TOKEN_VALUE);
    m1.quality.input_total = FieldQuality::Reported;
    let mut m2 = evt("inst", "m2", base);
    m2.usage.input_total = Some(MAX_TOKEN_VALUE);
    m2.quality.input_total = FieldQuality::Reported;
    let result = commit_batch(
        &storage,
        &batch("inst", "UTC", base + 3, vec![m1, m2]),
        None,
    );
    assert!(result.is_err());
    // 回滚：m1/m2 未入库。
    let count: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE source_record_key IN ('m1','m2')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn seconds_vs_milliseconds_misjudgment_rejected() {
    let (_dir, storage) = temp_storage("mssec");
    // 秒级时间戳（~2023-11-14）被当作毫秒 → 早于 2000 年，拒绝并记诊断。
    let mut bad = evt("inst", "sec-ts", 1_700_000_000);
    bad.usage.input_total = Some(1);
    bad.quality.input_total = FieldQuality::Reported;
    let out = commit_batch(
        &storage,
        &batch("inst", "UTC", ts("2026-09-24T12:00:00Z"), vec![bad]),
        None,
    )
    .unwrap();
    assert_eq!(out.errors, 1);
    assert_eq!(out.added, 0);
    let diag: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'validation_failed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);
}

#[test]
fn null_fields_propagate_through_storage_and_aggregation() {
    let (_dir, storage) = temp_storage("nulls");
    let base = ts("2026-09-24T10:00:00Z");
    // 全部 token 未知。
    let e = evt("inst", "all-unknown", base);
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![e]), None).unwrap();
    let stored: Option<i64> = storage
        .conn()
        .query_row(
            "SELECT input_total FROM usage_events WHERE source_record_key = 'all-unknown'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, None);
    let summary = day_totals(&storage);
    let sums = &summary.periods[0].sums;
    assert_eq!(sums.input_total_known, None);
    assert_eq!(sums.output_total_known, None);
    assert_eq!(sums.total_tokens_known, None);
    assert_eq!(sums.cache_input_ratio(), None);
    assert_eq!(sums.input_unknown_count, 1);
}

#[test]
fn quota_snapshots_are_not_converted_to_tokens() {
    let (_dir, storage) = temp_storage("quota");
    llm_usage_core::aggregates::insert_quota_snapshot(
        &storage,
        &llm_usage_core::aggregates::QuotaSnapshotInput {
            quota_id: "q-1".into(),
            instance_id: "inst".into(),
            observed_at_ms: ts("2026-09-24T10:00:00Z"),
            kind: "subscription_window".into(),
            quantity_minor: Some(4200),
            unit: "credit".into(),
            window_start_ms: Some(ts("2026-09-01T00:00:00Z")),
            window_end_ms: Some(ts("2026-10-01T00:00:00Z")),
            locality_verified: true,
            detail: Some(serde_json::json!({"plan": "pro"})),
        },
    )
    .unwrap();
    let kind: String = storage
        .conn()
        .query_row(
            "SELECT kind FROM quota_snapshots WHERE quota_id = 'q-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kind, "subscription_window");
    // 额度不进入 token 统计。
    assert_eq!(day_totals(&storage).totals.event_count, 0);
}

#[test]
fn event_aliases_link_cross_source_records() {
    let (_dir, storage) = temp_storage("alias");
    let base = ts("2026-09-24T10:00:00Z");
    let mut e1 = with_tokens(evt("inst", "k1", base), 100, 0);
    e1.origin_call_id = Some("origin-1".into());
    let mut e2 = with_tokens(evt("inst2", "k9", base), 100, 0);
    e2.origin_call_id = Some("origin-1".into());
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![e1]), None).unwrap();
    commit_batch(&storage, &batch("inst2", "UTC", base + 2, vec![e2]), None).unwrap();
    storage
        .conn()
        .execute(
            "INSERT INTO event_aliases (canonical_event_id, member_event_id, link_basis, created_at_ms)
             VALUES ('inst#k1', 'inst2#k9', 'origin_call_id', 1)",
            [],
        )
        .unwrap();
    let linked: String = storage
        .conn()
        .query_row(
            "SELECT member_event_id FROM event_aliases WHERE canonical_event_id = 'inst#k1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(linked, "inst2#k9");
}

#[test]
fn source_instance_stores_locality_and_attribution() {
    let (_dir, storage) = temp_storage("instance");
    let now = ts("2026-09-24T12:00:00Z");
    storage
        .conn()
        .execute(
            "INSERT INTO source_instances (instance_id, agent, host_application, locality_basis,
               attribution_status, exclusion_reason, enabled, format, location_hint, parser_version,
               capabilities, health, created_at_ms, updated_at_ms)
             VALUES ('wsl-1', 'kimi', 'vscode', 'wsl_instance', 'pending', NULL, 1, 'jsonl',
                     'wsl://ubuntu/home/u', '1.0.3', '{\"usage\": true}', 'ok', ?1, ?1)",
            rusqlite::params![now],
        )
        .unwrap();
    let (basis, status): (String, String) = storage
        .conn()
        .query_row("SELECT locality_basis, attribution_status FROM source_instances WHERE instance_id = 'wsl-1'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(basis, "wsl_instance");
    assert_eq!(status, "pending");
}

/// quality_json 持久化逐字段质量（reported/derived/estimated/unknown）。
#[test]
fn field_quality_roundtrip() {
    let (_dir, storage) = temp_storage("quality");
    let base = ts("2026-09-24T10:00:00Z");
    let mut e = evt("inst", "q1", base);
    e.usage.input_total = Some(10);
    e.quality.input_total = FieldQuality::Estimated;
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![e]), None).unwrap();
    let json: String = storage
        .conn()
        .query_row(
            "SELECT quality_json FROM usage_events WHERE source_record_key = 'q1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let quality: TokenQuality = serde_json::from_str(&json).unwrap();
    assert_eq!(quality.input_total, FieldQuality::Estimated);
    assert_eq!(quality.output_total, FieldQuality::Unknown);
    // 估算进入 estimated 质量分区，与 reported 分开。
    let bucket: String = storage
        .conn()
        .query_row(
            "SELECT quality_bucket FROM usage_events WHERE source_record_key = 'q1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bucket, "estimated");
}

/// 批次内矛盾事件：缓存读 > 总输入 → 事件保留原值，诊断记录，不用 max(0,…) 隐藏。
#[test]
fn contradiction_diagnostics_stored_without_clamping() {
    let (_dir, storage) = temp_storage("contradiction");
    let base = ts("2026-09-24T10:00:00Z");
    let mut e = evt("inst", "bad-cache", base);
    e.usage.input_total = Some(100);
    e.usage.input_cache_read = Some(800);
    e.quality.input_total = FieldQuality::Reported;
    e.quality.input_cache_read = FieldQuality::Reported;
    commit_batch(&storage, &batch("inst", "UTC", base + 1, vec![e]), None).unwrap();
    let read: i64 = storage
        .conn()
        .query_row(
            "SELECT input_cache_read FROM usage_events WHERE source_record_key = 'bad-cache'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(read, 800, "矛盾值原样保留，不做 max(0,…) 修正");
    let diag: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'cache_read_exceeds_input_total'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1);
}

/// usage_events 的 usage 值域在查询层再次校验 token 字段质量传播的默认值。
#[test]
fn token_usage_default_is_all_unknown() {
    let usage = TokenUsage::default();
    let quality = TokenQuality::default();
    assert_eq!(llm_usage_core::metrics::input_total(&usage, &quality), None);
    assert_eq!(
        llm_usage_core::metrics::total_tokens(&usage, &quality),
        None
    );
    assert_eq!(llm_usage_core::metrics::cache_input_ratio(&[]), (None, 0));
}

/// 调度合同存储：全局/逐源规则、时区、下次计划时间、desired/applied 状态。
/// （M1 只建结构与存储语义；定时器与并发控制在 M6 接。）
#[test]
fn schedule_tables_roundtrip() {
    let (_dir, storage) = temp_storage("sched");
    let now = ts("2026-09-24T12:00:00Z");
    storage
        .conn()
        .execute(
            "INSERT INTO extraction_schedules (schedule_id, scope, instance_id, rule_kind,
               interval_seconds, time_of_day, weekday, tz, enabled, paused_reason,
               file_trigger_enabled, config_version, next_due_at_ms, created_at_ms, updated_at_ms)
             VALUES ('sched-global', 'global', NULL, 'interval', 60, NULL, NULL,
                     'Asia/Shanghai', 1, NULL, 1, 3, ?1, ?1, ?1)",
            rusqlite::params![now + 60_000],
        )
        .unwrap();
    storage
        .conn()
        .execute(
            "INSERT INTO schedule_state (schedule_id, desired_state, applied_state,
               last_started_ms, last_success_ms, last_duration_ms, last_run_stats,
               error_summary, next_run_ms, updated_ms)
             VALUES ('sched-global', 'enabled', 'applied', ?1, ?1, 820, '{\"added\": 5}', NULL, ?2, ?1)",
            rusqlite::params![now, now + 60_000],
        )
        .unwrap();
    let (tz, rule, desired, applied): (String, String, String, String) = storage
        .conn()
        .query_row(
            "SELECT s.tz, s.rule_kind, st.desired_state, st.applied_state
             FROM extraction_schedules s JOIN schedule_state st ON st.schedule_id = s.schedule_id
             WHERE s.schedule_id = 'sched-global'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (
            tz.as_str(),
            rule.as_str(),
            desired.as_str(),
            applied.as_str()
        ),
        ("Asia/Shanghai", "interval", "enabled", "applied")
    );
}

/// 导入批次身份与状态：planned → committed / rolled_back（旧库导入幂等细节属 M2）。
#[test]
fn import_manifests_roundtrip() {
    let (_dir, storage) = temp_storage("import");
    let now = ts("2026-09-24T12:00:00Z");
    storage
        .conn()
        .execute(
            "INSERT INTO import_manifests (import_id, source_digest, scope, status, stats, rollback_info, created_ms, finished_ms)
             VALUES ('imp-1', 'fnv1a64:abc', 'previous-draft', 'planned', NULL, NULL, ?1, NULL)",
            rusqlite::params![now],
        )
        .unwrap();
    storage
        .conn()
        .execute(
            "UPDATE import_manifests SET status = 'committed', stats = '{\"events\": 42}', finished_ms = ?2 WHERE import_id = ?1",
            rusqlite::params!["imp-1", now + 1000],
        )
        .unwrap();
    let (status, stats): (String, String) = storage
        .conn()
        .query_row(
            "SELECT status, stats FROM import_manifests WHERE import_id = 'imp-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "committed");
    assert!(stats.contains("42"));
}
