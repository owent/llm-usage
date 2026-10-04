//! v6 多用户（users 表/来源归属/按用户过滤查询）与聚合导入与查询验证
//!（导出 → 导入 → 数据不缺失/幂等/修订合并）的回归测试。

mod common;

use common::*;
use llm_usage_core::exchange::build_export;
use llm_usage_core::exchange_import::import_aggregate;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::storage::Storage;

fn aggregate_sample(storage: &Storage) -> llm_usage_core::exchange::ExchangeExport {
    storage.ensure_local_host("sample", 1).unwrap();
    storage.conn().execute("INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,health,created_at_ms,updated_at_ms) VALUES ('sample','codex','local_filesystem','verified',1,'ok',1,1)", []).unwrap();
    let at = ts("2026-09-25T10:00:00Z");
    let mut e = with_tokens(evt("sample", "one", at), 100, 10);
    e.agent = "codex".into();
    commit_batch(storage, &batch("sample", "UTC", at, vec![e]), None).unwrap();
    llm_usage_core::exchange::build_aggregate_export(
        storage,
        &llm_usage_core::exchange::ExportRequest {
            timezone: "UTC".into(),
            from_ms: ts("2026-09-01T00:00:00Z"),
            to_ms: ts("2026-10-01T00:00:00Z"),
            instances: None,
            redact_hostnames: true,
            kind: llm_usage_core::exchange::ExchangeKind::FullSnapshot,
            batch_id: "test".into(),
        },
        at,
    )
    .unwrap()
}

#[test]
fn aggregate_exchange_preserves_completeness_and_rejects_partial_bad_import() {
    let (_a, a) = temp_storage("complete-export");
    let export = aggregate_sample(&a);
    assert!(
        export.records.is_empty(),
        "desktop exchange never exports raw session identities"
    );
    assert_eq!(export.daily_partitions.len(), 1);
    assert_eq!(export.hourly_partitions.len(), 1);
    let (_b, b) = temp_storage("complete-import");
    let mut invalid = export.clone();
    invalid.hourly_partitions[0].hour = 24;
    assert!(import_aggregate(&b, &invalid, 2).is_err());
    let count: i64 = b
        .conn()
        .query_row("SELECT COUNT(*) FROM daily_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 0,
        "late validation failure rolls back earlier inserts"
    );
    import_aggregate(&b, &export, 3).unwrap();
    let before = query_summary(&a, &request_all()).unwrap();
    let after = query_summary(&b, &request_all()).unwrap();
    assert_eq!(
        before.totals, after.totals,
        "all known/unknown counts and ratio samples survive exchange"
    );
    let revision = after.data_revision;
    import_aggregate(&b, &export, 4).unwrap();
    assert_eq!(b.data_revision().unwrap(), revision);
    let repeated = import_aggregate(&a, &export, 5).unwrap();
    assert_eq!(repeated.daily_replaced, 0);
    let sealed: i64 = a
        .conn()
        .query_row("SELECT sealed FROM daily_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sealed, 0, "own snapshot cannot freeze live ingestion");
    let mut conflicting = export.clone();
    conflicting.daily_partitions[0].total_known_sum = Some(999);
    assert!(import_aggregate(&a, &conflicting, 6).is_err());
}

#[test]
fn empty_export_selection_does_not_export_everyone() {
    let (_dir, db) = temp_storage("empty-export");
    aggregate_sample(&db);
    let export = llm_usage_core::exchange::build_aggregate_export(
        &db,
        &llm_usage_core::exchange::ExportRequest {
            timezone: "UTC".into(),
            from_ms: 0,
            to_ms: ts("2026-10-01T00:00:00Z"),
            instances: Some(vec![]),
            redact_hostnames: true,
            kind: llm_usage_core::exchange::ExchangeKind::FullSnapshot,
            batch_id: "empty".into(),
        },
        2,
    )
    .unwrap();
    assert!(export.sources.is_empty());
    assert!(export.records.is_empty());
    assert!(export.daily_partitions.is_empty());
    assert!(export.hourly_partitions.is_empty());
    assert!(export.period_partitions.is_empty());
}

#[test]
fn period_only_history_survives_export_import_without_daily_rows() {
    let (_a, a) = temp_storage("period-export");
    aggregate_sample(&a);
    a.conn().execute("INSERT INTO period_usage(tz_version,granularity,period_key,period_start_day,period_end_day,instance_id,agent,call_category,quality_bucket,event_count,call_count,total_known_sum,conflict_count,active_days,materialized_at_ms,data_revision)
        VALUES ('UTC','month','2025-12','2025-12-01','2025-12-31','sample','codex','primary','exact',9,9,990,1,3,1,1)", []).unwrap();
    let export = llm_usage_core::exchange::build_aggregate_export(
        &a,
        &llm_usage_core::exchange::ExportRequest {
            timezone: "UTC".into(),
            from_ms: ts("2025-12-01T00:00:00Z"),
            to_ms: ts("2026-01-01T00:00:00Z"),
            instances: None,
            redact_hostnames: true,
            kind: llm_usage_core::exchange::ExchangeKind::FullSnapshot,
            batch_id: "period".into(),
        },
        2,
    )
    .unwrap();
    assert!(export.daily_partitions.is_empty());
    assert_eq!(export.period_partitions.len(), 1);
    let (_b, b) = temp_storage("period-import");
    assert_eq!(import_aggregate(&b, &export, 3).unwrap().period_inserted, 1);
    let mut query = request_all();
    query.first_day = llm_usage_core::calendar::parse_date("2025-12-01").unwrap();
    query.last_day = llm_usage_core::calendar::parse_date("2025-12-31").unwrap();
    query.granularity = Granularity::Month;
    let summary = query_summary(&b, &query).unwrap();
    assert_eq!(summary.totals.call_count, 9);
    assert_eq!(summary.totals.total_tokens_known, Some(990));
    assert_eq!(summary.totals.conflict_count, 1);
    assert_eq!(summary.distinct_sessions, None);
    assert_eq!(import_aggregate(&b, &export, 4).unwrap().period_skipped, 1);
}

fn request(instances: Vec<String>) -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".to_string(),
        week_start: llm_usage_core::calendar::WeekStart::Monday,
        first_day: llm_usage_core::calendar::parse_date("2026-09-20").unwrap(),
        last_day: llm_usage_core::calendar::parse_date("2026-09-26").unwrap(),
        granularity: Granularity::Day,
        filters: Filters {
            instances: Some(instances),
            ..Filters::default()
        },
        today: llm_usage_core::calendar::parse_date("2026-09-26").unwrap(),
        retention_cutoff: None,
    }
}

fn request_all() -> SummaryRequest {
    let mut q = request(vec![]);
    q.filters.instances = None;
    q
}

#[test]
fn users_partition_queries_by_source_membership() {
    let (_dir, storage) = temp_storage("users");
    // 两个来源各写一天数据；A 归 default，B 改归 user2。
    // commit_batch 不建 source_instances——采集流程由 upsert 注册，此处直接建。
    for instance in ["codex@a", "codex@b"] {
        storage
            .conn()
            .execute(
                "INSERT INTO source_instances (
                   instance_id, agent, host_application, locality_basis, attribution_status,
                   exclusion_reason, enabled, format, location_hint, parser_version,
                   capabilities, health, created_at_ms, updated_at_ms
                 ) VALUES (?1, 'codex', NULL, 'local_filesystem', 'verified',
                   NULL, 1, 'codex-rollout-jsonl', NULL, 'codex-rollout-1', '{}', 'ok', 1000, 1000)",
                [instance],
            )
            .unwrap();
    }
    for instance in ["codex@a", "codex@b"] {
        commit_batch(
            &storage,
            &batch(
                instance,
                "UTC",
                ts("2026-09-25T10:00:00Z"),
                vec![with_tokens(
                    evt(instance, "rk", ts("2026-09-25T10:00:00Z")),
                    100,
                    10,
                )],
            ),
            None,
        )
        .unwrap();
    }
    let users: Vec<(String, String)> = storage
        .conn()
        .prepare("SELECT user_id, name FROM users")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        users,
        vec![("default".to_string(), "default".to_string())],
        "v6 迁移内置 default"
    );
    storage
        .conn()
        .execute(
            "INSERT INTO users (user_id, name, created_at_ms) VALUES ('u2', 'alice', 1)",
            [],
        )
        .unwrap();
    storage
        .conn()
        .execute(
            "UPDATE source_instances SET user_id = 'u2' WHERE instance_id = 'codex@b'",
            [],
        )
        .unwrap();
    // 按用户的来源集合过滤查询（app 层解析方式）。
    let instances_of = |user: &str| -> Vec<String> {
        storage
            .conn()
            .prepare("SELECT instance_id FROM source_instances WHERE user_id = ?1")
            .unwrap()
            .query_map([user], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let s_default = query_summary(&storage, &request(instances_of("default"))).unwrap();
    assert_eq!(s_default.totals.call_count, 1);
    assert_eq!(s_default.totals.total_tokens_known, Some(110));
    let s_u2 = query_summary(&storage, &request(instances_of("u2"))).unwrap();
    assert_eq!(s_u2.totals.call_count, 1, "用户视图互不串数");
    // 不过滤 = 全部（共享 host 的两个用户合计不双计：各来源一次）。
    let s_all = query_summary(&storage, &request_all()).unwrap();
    assert_eq!(s_all.totals.call_count, 2);
}

#[test]
fn aggregate_export_import_roundtrip_restores_history() {
    // 库 A（导出方）：两来源两天数据 + 小时层。
    let (_dir_a, storage_a) = temp_storage("export-a");
    let host = storage_a.ensure_local_host("machine-a", 1_000).unwrap();
    storage_a
        .conn()
        .execute(
            "INSERT INTO source_instances (
               instance_id, agent, host_application, locality_basis, attribution_status,
               exclusion_reason, enabled, format, location_hint, parser_version,
               capabilities, health, created_at_ms, updated_at_ms
             ) VALUES ('codex@a', 'codex', NULL, 'local_filesystem', 'verified',
               NULL, 1, 'codex-rollout-jsonl', NULL, 'codex-rollout-1', '{}', 'ok', 1000, 1000)",
            [],
        )
        .unwrap();
    for (instance, day) in [("codex@a", "2026-09-24"), ("codex@a", "2026-09-25")] {
        let t = format!("{day}T10:00:00Z");
        commit_batch(
            &storage_a,
            &batch(
                instance,
                "UTC",
                ts(&t),
                vec![with_tokens(
                    {
                        let mut e = evt(instance, day, ts(&t));
                        e.agent = "codex".into();
                        e
                    },
                    100,
                    10,
                )],
            ),
            None,
        )
        .unwrap();
    }
    llm_usage_core::ingest::recompute_days_in_tz(
        &storage_a,
        "UTC",
        ts("2026-09-24T00:00:00Z"),
        ts("2026-09-25T23:00:00Z"),
        2_000,
    )
    .unwrap();
    let export = build_export(
        &storage_a,
        &llm_usage_core::exchange::ExportRequest {
            timezone: "UTC".to_string(),
            from_ms: 0,
            to_ms: 4_102_444_800_000,
            instances: None,
            redact_hostnames: true,
            kind: llm_usage_core::exchange::ExchangeKind::FullSnapshot,
            batch_id: "rt-1".to_string(),
        },
        2_100,
    )
    .unwrap();
    // 聚合导出改造由 app 层清 records + 补全分区；这里直接验证结构字段：
    // 来源注册在、日分区在（build_export 默认含封存行——模拟补全）。
    assert_eq!(export.host.origin_host_id, host);
    assert!(!export.sources.is_empty());

    // 库 B（导入方）：空库导入 JSON 往返包。
    let json = serde_json::to_string(&export).unwrap();
    let parsed: llm_usage_core::exchange::ExchangeExport = serde_json::from_str(&json).unwrap();
    let (_dir_b, storage_b) = temp_storage("import-b");
    let outcome = import_aggregate(&storage_b, &parsed, 3_000).unwrap();
    assert_eq!(outcome.sources_registered, export.sources.len());
    // 重复导入覆盖（同修订也替换——用户约定"覆盖"语义；内容相同 SQL 幂等）。
    let again = import_aggregate(&storage_b, &parsed, 3_100).unwrap();
    assert_eq!(again.daily_inserted, 0, "同键不重复插入");
    assert_eq!(again.daily_replaced, 0);
    assert_eq!(again.daily_skipped, 2);
    // 导入后查询可见（来源聚合重建）。
    let s = query_summary(&storage_b, &request_all()).unwrap();
    assert_eq!(export.daily_partitions.len(), 2);
    assert_eq!(s.totals.call_count, 2);
    assert_eq!(s.totals.total_tokens_known, Some(220));
    assert_eq!(
        s.distinct_sessions, None,
        "aggregate imports do not invent details"
    );
}

#[test]
fn import_newer_revision_replaces_and_older_conflicts() {
    let (_dir, storage) = temp_storage("import-rev");
    let make_partition = |revision: i64, total: i64| llm_usage_core::exchange::ExchangeExport {
        format_version: llm_usage_core::exchange::EXCHANGE_FORMAT_VERSION.to_string(),
        kind: llm_usage_core::exchange::ExchangeKind::FullSnapshot,
        batch_id: format!("b-{revision}"),
        exported_at_ms: revision,
        timezone: "UTC".to_string(),
        host: llm_usage_core::exchange::ExchangeHost {
            origin_host_id: "host-x".to_string(),
            hostname_alias: None,
        },
        sources: vec![llm_usage_core::exchange::ExchangeSource {
            source_instance_id: "codex@x".to_string(),
            agent: "codex".to_string(),
            format: "codex-rollout-jsonl".to_string(),
            parser_version: "p".to_string(),
            locality_basis: "local_filesystem".to_string(),
            attribution_status: "verified".to_string(),
            first_seen_ms: 1,
            completeness: serde_json::Value::Null,
            origin_host_id: None,
        }],
        records: vec![],
        daily_partitions: vec![llm_usage_core::exchange::ExchangeDailyPartition {
            statistics: None,
            tz_version: "UTC".to_string(),
            local_day: "2026-09-25".to_string(),
            instance_id: "codex@x".to_string(),
            agent: "codex".to_string(),
            provider_id: String::new(),
            model_raw: String::new(),
            call_category: "primary".to_string(),
            quality_bucket: "exact".to_string(),
            event_count: 1,
            call_count: 1,
            input_known_sum: Some(total),
            cache_read_known_sum: Some(0),
            cache_write_known_sum: Some(0),
            output_known_sum: Some(0),
            total_known_sum: Some(total),
            conflict_count: 0,
            sealed: false,
            data_revision: revision,
        }],
        hourly_partitions: vec![],
        period_partitions: vec![],
    };
    // 初次导入（rev 5）→ 插入。
    let out1 = import_aggregate(&storage, &make_partition(5, 100), 1_000).unwrap();
    assert_eq!(out1.daily_inserted, 1);
    // 更高修订（rev 6, 值 80）→ 替换（当前值 80，V02 更正语义）。
    let out2 = import_aggregate(&storage, &make_partition(6, 80), 1_100).unwrap();
    assert_eq!(out2.daily_replaced, 1);
    let total: Option<i64> = storage
        .conn()
        .query_row("SELECT total_known_sum FROM daily_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, Some(80));
    // 更低修订（rev 4）→ 冲突保留现存。
    let out3 = import_aggregate(&storage, &make_partition(4, 999), 1_200).unwrap();
    assert_eq!(out3.daily_conflicts, 1);
    let total: Option<i64> = storage
        .conn()
        .query_row("SELECT total_known_sum FROM daily_usage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, Some(80), "低修订不覆盖现存");
    let diag: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code = 'import_revision_older'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(diag, 1, "冲突可见");
    // 导入主机登记为外部。
    let is_local: i64 = storage
        .conn()
        .query_row(
            "SELECT is_local FROM origin_hosts WHERE host_id = 'host-x'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(is_local, 0);
}

#[test]
fn old_version_database_rejected() {
    // 预发布约定：任何版本不匹配都拒绝（不做迁移）。
    let dir = TempDir::new("v6-old");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        storage
            .conn()
            .pragma_update(None, "user_version", 5u32)
            .unwrap();
    }
    assert!(Storage::open(&dir.db_path()).is_err());
}
#[test]
fn repeated_import_overwrites_not_duplicates() {
    let (_dir, storage) = temp_storage("import-overwrite");
    let make = |total: i64| llm_usage_core::exchange::ExchangeExport {
        format_version: llm_usage_core::exchange::EXCHANGE_FORMAT_VERSION.to_string(),
        kind: llm_usage_core::exchange::ExchangeKind::FullSnapshot,
        batch_id: "b".to_string(),
        exported_at_ms: 1,
        timezone: "UTC".to_string(),
        host: llm_usage_core::exchange::ExchangeHost {
            origin_host_id: "host-y".to_string(),
            hostname_alias: None,
        },
        sources: vec![llm_usage_core::exchange::ExchangeSource {
            source_instance_id: "codex@y".to_string(),
            agent: "codex".to_string(),
            format: "f".to_string(),
            parser_version: "p".to_string(),
            locality_basis: "local_filesystem".to_string(),
            attribution_status: "verified".to_string(),
            first_seen_ms: 1,
            completeness: serde_json::Value::Null,
            origin_host_id: None,
        }],
        records: vec![],
        daily_partitions: vec![llm_usage_core::exchange::ExchangeDailyPartition {
            statistics: None,
            tz_version: "UTC".to_string(),
            local_day: "2026-09-25".to_string(),
            instance_id: "codex@y".to_string(),
            agent: "codex".to_string(),
            provider_id: String::new(),
            model_raw: String::new(),
            call_category: "primary".to_string(),
            quality_bucket: "exact".to_string(),
            event_count: 1,
            call_count: 1,
            input_known_sum: Some(total),
            cache_read_known_sum: Some(0),
            cache_write_known_sum: Some(0),
            output_known_sum: Some(0),
            total_known_sum: Some(total),
            conflict_count: 0,
            sealed: false,
            data_revision: 10,
        }],
        hourly_partitions: vec![],
        period_partitions: vec![],
    };
    import_aggregate(&storage, &make(100), 1_000).unwrap();
    let out2 = import_aggregate(&storage, &make(200), 1_100).unwrap();
    assert_eq!(out2.daily_inserted, 0, "不重复插入");
    let out3 = import_aggregate(&storage, &make(200), 1_200).unwrap();
    assert_eq!(out3.daily_inserted, 0, "仍不重复插入");
    let (rows, total): (i64, i64) = storage
        .conn()
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_known_sum), 0) FROM daily_usage",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(rows, 1, "单行不冗余");
    assert_eq!(total, 200, "覆盖为最新值");
}
