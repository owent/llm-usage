//! V28（M1a）：历史来源身份与存储分区。
//! 主机身份稳定（改名不重复计数、同名不同主机不键冲突）、v3→v4 迁移
//! （legacy_unknown 命名空间、封存保留、认领与分区重算）、交换约定
//! （格式版本/来源注册/字段完整性/修订/快照-增量性质）与合并判定
//! （幂等跳过/权威替换/互斥新增/冲突保留）用脱敏固定样本验证。

mod common;

use common::*;
use llm_usage_core::adapters::framework::{
    upsert_source_instance, SourceInstanceInput, LEGACY_UNKNOWN_HOST,
};
use llm_usage_core::domain::{AttributionStatus, Lifecycle, LocalityBasis, VersionBasis};
use llm_usage_core::exchange::{
    build_export, decide_record_merge, ExchangeKind, ExchangeRecord, ExchangeUsage, ExistingRecord,
    ExportRequest, EXCHANGE_FORMAT_VERSION,
};
use llm_usage_core::identity::content_hash;
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::query::{query_summary, Filters, Granularity, SummaryRequest};
use llm_usage_core::storage::Storage;

fn summary_request(first: &str, last: &str) -> SummaryRequest {
    SummaryRequest {
        timezone: "UTC".to_string(),
        week_start: llm_usage_core::calendar::WeekStart::Monday,
        first_day: llm_usage_core::calendar::parse_date(first).unwrap(),
        last_day: llm_usage_core::calendar::parse_date(last).unwrap(),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: llm_usage_core::calendar::parse_date(last).unwrap(),
        retention_cutoff: None,
    }
}

#[test]
fn local_host_identity_is_stable_across_rename_and_calls() {
    let (_dir, storage) = temp_storage("v28-host");
    let h1 = storage.ensure_local_host("machine-a", 1_000).unwrap();
    let h2 = storage.ensure_local_host("machine-a", 2_000).unwrap();
    assert_eq!(h1, h2, "same machine keeps one host id");
    // 改名：同一主机 ID，新增主机名观察，不产生第二个本机身份。
    let h3 = storage.ensure_local_host("machine-b", 3_000).unwrap();
    assert_eq!(h1, h3, "rename does not change origin_host_id");
    let hosts: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM origin_hosts WHERE is_local = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hosts, 1, "no duplicate counting on rename");
    let names: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM origin_host_names WHERE host_id = ?1",
            [&h1],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(names, 2, "hostname history preserved");
}

#[test]
fn same_hostname_different_hosts_do_not_collide() {
    // 两台同名主机：键用 host_id，不用主机名；同名不发生键冲突（V28）。
    let (_dir, storage) = temp_storage("v28-samename");
    let a = storage.ensure_local_host("same-name", 1_000).unwrap();
    storage
        .register_origin_host("host-external1", Some("same-name"), 1_100)
        .unwrap();
    assert_ne!(a, "host-external1");
    let n: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM origin_host_names WHERE hostname = 'same-name'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 2, "same hostname can belong to different hosts");
    // 两主机的同名实例注册互不覆盖（instance_id 主键不同命名空间）。
    for (instance, host) in [
        ("codex@a", a.as_str()),
        ("codex@host-external1", "host-external1"),
    ] {
        upsert_source_instance(
            &storage,
            &SourceInstanceInput {
                instance_id: instance.to_string(),
                agent: "codex".to_string(),
                host_application: None,
                locality_basis: LocalityBasis::LocalFilesystem,
                attribution_status: AttributionStatus::Verified,
                exclusion_reason: None,
                format: "codex-rollout-jsonl".to_string(),
                location_hint: None,
                parser_version: "codex-rollout-1".to_string(),
                capabilities: serde_json::json!({}),
                health: "ok".to_string(),
                origin_host_id: Some(host.to_string()),
            },
            1_000,
        )
        .unwrap();
    }
    let hosts: Vec<String> = storage
        .conn()
        .prepare("SELECT origin_host_id FROM source_instances ORDER BY instance_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(hosts, vec![a, "host-external1".to_string()]);
}

#[test]
fn legacy_instances_claimed_only_by_verified_local_scan() {
    let (_dir, storage) = temp_storage("v28-claim");
    upsert_source_instance(
        &storage,
        &SourceInstanceInput {
            instance_id: "codex@old".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: LocalityBasis::LocalFilesystem,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({}),
            health: "ok".to_string(),
            origin_host_id: None,
        },
        1_000,
    )
    .unwrap();
    let host: String = storage
        .conn()
        .query_row(
            "SELECT origin_host_id FROM source_instances WHERE instance_id = 'codex@old'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        host, LEGACY_UNKNOWN_HOST,
        "no host evidence → legacy namespace"
    );

    // 本机核验采集认领（文件就在本机 + locality 已核验 = 可证明映射）。
    let local = storage.ensure_local_host("machine-a", 2_000).unwrap();
    upsert_source_instance(
        &storage,
        &SourceInstanceInput {
            instance_id: "codex@old".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: LocalityBasis::LocalFilesystem,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({}),
            health: "ok".to_string(),
            origin_host_id: Some(local.clone()),
        },
        3_000,
    )
    .unwrap();
    let host: String = storage
        .conn()
        .query_row(
            "SELECT origin_host_id FROM source_instances WHERE instance_id = 'codex@old'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(host, local, "verified local scan claims legacy instance");

    // 已属于其他主机的来源不被覆盖（导入来源不因本机扫描改归属）。
    upsert_source_instance(
        &storage,
        &SourceInstanceInput {
            instance_id: "codex@external".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: LocalityBasis::LocalFilesystem,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({}),
            health: "ok".to_string(),
            origin_host_id: Some("host-other".to_string()),
        },
        4_000,
    )
    .unwrap();
    upsert_source_instance(
        &storage,
        &SourceInstanceInput {
            instance_id: "codex@external".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: LocalityBasis::LocalFilesystem,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({}),
            health: "ok".to_string(),
            origin_host_id: Some(local),
        },
        5_000,
    )
    .unwrap();
    let host: String = storage
        .conn()
        .query_row(
            "SELECT origin_host_id FROM source_instances WHERE instance_id = 'codex@external'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        host, "host-other",
        "established ownership is never overwritten"
    );
}

#[test]
fn old_version_database_rejected_not_migrated() {
    // 预发布约定：旧版本库直接拒绝打开（应用层提示重建），不做迁移。
    let dir = TempDir::new("v28-old-version");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        // 手动写一个事件让库非空。
        commit_batch(
            &storage,
            &batch(
                "codex@a",
                "UTC",
                ts("2026-09-20T10:00:00Z"),
                vec![with_tokens(
                    evt("codex@a", "r1", ts("2026-09-20T10:00:00Z")),
                    100,
                    10,
                )],
            ),
            None,
        )
        .unwrap();
        // 强制设置旧版本号。
        storage
            .conn()
            .pragma_update(None, "user_version", 3u32)
            .unwrap();
    }
    let result = Storage::open(&dir.db_path());
    assert!(
        result.is_err(),
        "旧版本库应被拒绝打开（提示重建），不做迁移"
    );
}

#[test]
fn cross_source_query_sums_while_partitions_keep_identity() {
    // 两个来源实例各写一天数据：日分区按来源落盘，查询跨来源求和；
    // 删除明细后分区与来源注册仍保留来源身份。
    let (_dir, storage) = temp_storage("v28-partition");
    let host = storage.ensure_local_host("machine-a", 1_000).unwrap();
    for (instance, key) in [("codex@a", "ra"), ("codex@b", "rb")] {
        commit_batch(
            &storage,
            &batch(
                instance,
                "UTC",
                ts("2026-09-21T10:00:00Z"),
                vec![with_tokens(
                    evt(instance, key, ts("2026-09-21T10:00:00Z")),
                    100,
                    10,
                )],
            ),
            None,
        )
        .unwrap();
    }
    let partitions: Vec<(String, i64)> = storage
        .conn()
        .prepare(
            "SELECT instance_id, total_known_sum FROM daily_usage
             WHERE local_day = '2026-09-21' ORDER BY instance_id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        partitions,
        vec![("codex@a".to_string(), 110), ("codex@b".to_string(), 110),],
        "each source keeps its own partition contribution"
    );
    let s = query_summary(&storage, &summary_request("2026-09-21", "2026-09-21")).unwrap();
    assert_eq!(s.totals.call_count, 2, "cross-source sum in query");
    assert_eq!(s.totals.total_tokens_known, Some(220));
    // 删除明细：分区与来源注册保留来源身份（不随清理丢失）。
    storage
        .conn()
        .execute("DELETE FROM usage_events", [])
        .unwrap();
    let kept: Vec<String> = storage
        .conn()
        .prepare("SELECT DISTINCT instance_id FROM daily_usage")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(kept.len(), 2, "partition identity survives detail cleanup");
    let _ = host;
}

#[test]
fn export_contains_contract_fields_and_round_trips() {
    let (_dir, storage) = temp_storage("v28-export");
    let host = storage.ensure_local_host("machine-a", 1_000).unwrap();
    // 导出按来源注册表关联记录；采集流程会先注册实例（run_adapter_scan）。
    upsert_source_instance(
        &storage,
        &SourceInstanceInput {
            instance_id: "codex@a".to_string(),
            agent: "codex".to_string(),
            host_application: None,
            locality_basis: LocalityBasis::LocalFilesystem,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            format: "codex-rollout-jsonl".to_string(),
            location_hint: None,
            parser_version: "codex-rollout-1".to_string(),
            capabilities: serde_json::json!({"fields": {}}),
            health: "ok".to_string(),
            origin_host_id: Some(host.clone()),
        },
        1_000,
    )
    .unwrap();
    let mut e = with_tokens(evt("codex@a", "r1", ts("2026-09-22T10:00:00Z")), 100, 10);
    e.parse_basis = Some(VersionBasis::LatestFallback);
    commit_batch(
        &storage,
        &batch("codex@a", "UTC", ts("2026-09-22T10:00:00Z"), vec![e]),
        None,
    )
    .unwrap();
    let request = ExportRequest {
        timezone: "UTC".to_string(),
        from_ms: 0,
        to_ms: 4_102_444_800_000,
        instances: None,
        redact_hostnames: true,
        kind: ExchangeKind::FullSnapshot,
        batch_id: "batch-1".to_string(),
    };
    let export = build_export(&storage, &request, 2_000).unwrap();
    assert_eq!(export.format_version, EXCHANGE_FORMAT_VERSION);
    assert_eq!(export.host.origin_host_id, host);
    assert_eq!(
        export.host.hostname_alias, None,
        "hostname redacted on request"
    );
    assert_eq!(export.batch_id, "batch-1");
    assert_eq!(export.records.len(), 1);
    let record = &export.records[0];
    assert_eq!(record.source_instance_id, "codex@a");
    assert_eq!(record.parse_basis.as_deref(), Some("latest_fallback"));
    assert_eq!(record.usage.total_tokens, Some(110));
    assert_eq!(export.sources.len(), 1);
    assert_eq!(export.sources[0].source_instance_id, "codex@a");
    // JSON 往返稳定（版本化格式可被导入端解析）。
    let json = serde_json::to_string(&export).unwrap();
    let back: llm_usage_core::exchange::ExchangeExport = serde_json::from_str(&json).unwrap();
    assert_eq!(back, export);
}

#[test]
fn merge_decisions_follow_contract_table() {
    let base = ExchangeRecord {
        source_instance_id: "codex@a".into(),
        source_record_key: "resp:1".into(),
        record_kind: "model_call".into(),
        schema_version: "0.155.0-alpha.16.3".into(),
        parser_version: "codex-rollout-1".into(),
        parse_basis: None,
        occurred_at_ms: 1_000,
        source_time: None,
        time_basis: "source_completion".into(),
        agent: "codex".into(),
        call_category: "primary".into(),
        lifecycle: "final".into(),
        provider_id: None,
        model_raw: None,
        origin_call_id: None,
        session_id: None,
        parent_session_id: None,
        usage: ExchangeUsage {
            total_tokens: Some(110),
            ..ExchangeUsage::default()
        },
        quality_bucket: "exact".into(),
        source_revision: Some(5),
        conflict: false,
        parse_note: None,
    };
    let hash = content_hash(&"payload-a").to_string();

    // 无现存 → 新增独立贡献。
    assert_eq!(
        decide_record_merge(&base, &hash, Lifecycle::Final, None),
        llm_usage_core::exchange::MergeDecision::AddIndependent
    );

    let existing = ExistingRecord {
        source_instance_id: "codex@a".into(),
        source_record_key: "resp:1".into(),
        lifecycle: Lifecycle::Final,
        source_revision: Some(5),
        content_hash: hash.clone(),
    };
    // 同来源同键、同修订、内容一致 → 幂等跳过。
    assert_eq!(
        decide_record_merge(&base, &hash, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::SkipIdempotent
    );

    // 同修订不同内容 → 冲突（不按 token 大小裁决）。
    let hash_b = content_hash(&"payload-b").to_string();
    assert_eq!(
        decide_record_merge(&base, &hash_b, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::Conflict
    );

    // 更权威修订（修订号更高）→ 撤销旧贡献后替换。
    let mut revised = base.clone();
    revised.source_revision = Some(6);
    assert_eq!(
        decide_record_merge(&revised, &hash_b, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::ReplaceAfterRevoke
    );

    // 修订同级更正但修订号相同：无法确认哪条修订更新，内容不同 → 冲突可见
    //（与 ingest 仲裁一致：修订号相等只比内容；更正要替换须携带更高修订号）。
    assert_eq!(
        decide_record_merge(&base, &hash_b, Lifecycle::Corrected, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::Conflict
    );

    // 无修订号记录（两侧）：corrected 生命周期是权威顺序 → 替换。
    let mut no_rev = base.clone();
    no_rev.source_revision = None;
    let existing_no_rev = ExistingRecord {
        source_instance_id: "codex@a".into(),
        source_record_key: "resp:1".into(),
        lifecycle: Lifecycle::Final,
        source_revision: None,
        content_hash: hash.clone(),
    };
    assert_eq!(
        decide_record_merge(&no_rev, &hash, Lifecycle::Final, Some(&existing_no_rev)),
        llm_usage_core::exchange::MergeDecision::SkipIdempotent
    );
    assert_eq!(
        decide_record_merge(
            &no_rev,
            &hash_b,
            Lifecycle::Corrected,
            Some(&existing_no_rev)
        ),
        llm_usage_core::exchange::MergeDecision::ReplaceAfterRevoke
    );

    // 更低修订 → 保留现存（Keep + 内容不同）→ 冲突可见。
    let mut older = base.clone();
    older.source_revision = Some(4);
    assert_eq!(
        decide_record_merge(&older, &hash_b, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::Conflict
    );

    // 不同记录键 → 互斥来源新增。
    let mut other_key = base.clone();
    other_key.source_record_key = "resp:2".into();
    assert_eq!(
        decide_record_merge(&other_key, &hash, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::AddIndependent
    );
}

#[test]
fn tz_partition_repair_makes_history_visible_in_user_timezone() {
    // 回归（2026-09-26 缺陷）：扫描以 UTC 写日分区而用户按 Asia/Shanghai 查询
    // ⇒ tz_version 不匹配导致 UI 永远为空。事件仍在 ⇒ 重算是推导非猜测。
    let (_dir, storage) = temp_storage("tz-repair");
    // 模拟旧行为：UTC 日界提交（2026-09-25 18:30 UTC = 上海 09-26 02:30）。
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-25T18:30:00Z"),
            vec![with_tokens(
                evt("codex@a", "r-tz", ts("2026-09-25T18:30:00Z")),
                100,
                10,
            )],
        ),
        None,
    )
    .unwrap();
    let sh = llm_usage_core::query::SummaryRequest {
        timezone: "Asia/Shanghai".to_string(),
        week_start: llm_usage_core::calendar::WeekStart::Monday,
        first_day: llm_usage_core::calendar::parse_date("2026-09-20").unwrap(),
        last_day: llm_usage_core::calendar::parse_date("2026-09-30").unwrap(),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: llm_usage_core::calendar::parse_date("2026-09-30").unwrap(),
        retention_cutoff: None,
    };
    let before = query_summary(&storage, &sh).unwrap();
    assert_eq!(
        before.totals.call_count, 0,
        "UTC 分区在上海时区下不可见（缺陷复现）"
    );
    // 修复：在用户时区重算事件覆盖范围。
    llm_usage_core::ingest::recompute_days_in_tz(
        &storage,
        "Asia/Shanghai",
        ts("2026-09-25T18:30:00Z"),
        ts("2026-09-25T18:30:00Z"),
        1_800_000_100_000,
    )
    .unwrap();
    let after = query_summary(&storage, &sh).unwrap();
    assert_eq!(after.totals.call_count, 1);
    assert_eq!(after.totals.total_tokens_known, Some(110));
    // 事件归属上海日 2026-09-26（18:30Z = 02:30+08）。
    assert_eq!(after.periods[0].label, "2026-09-26");
    // UTC 分区保留（多时区并存按 tz_version 区分，不互相污染）。
    let utc_rows: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM daily_usage WHERE tz_version = 'UTC'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(utc_rows > 0);
}

#[test]
fn readonly_queries_do_not_block_behind_writer_transaction() {
    // WAL 约定：一个后台写者 + 只读连接并发。写事务未提交期间，
    // open_readonly 的查询照常进行（M6 修复：UI 查询不再被长扫描阻塞）。
    let (_dir, storage) = temp_storage("wal-concurrent");
    commit_batch(
        &storage,
        &batch(
            "codex@a",
            "UTC",
            ts("2026-09-25T10:00:00Z"),
            vec![with_tokens(
                evt("codex@a", "r-wal", ts("2026-09-25T10:00:00Z")),
                50,
                5,
            )],
        ),
        None,
    )
    .unwrap();
    let conn = storage.conn();
    let held = conn.unchecked_transaction().unwrap();
    held.execute_batch("INSERT INTO settings (key, value, schema_version, updated_at_ms) VALUES ('hold', '1', 1, 1)").unwrap();
    // 写事务未提交：只读连接读到的仍是提交前视图，且不被阻塞。
    let reader = Storage::open_readonly(storage.path()).unwrap();
    let req = llm_usage_core::query::SummaryRequest {
        timezone: "UTC".to_string(),
        week_start: llm_usage_core::calendar::WeekStart::Monday,
        first_day: llm_usage_core::calendar::parse_date("2026-09-25").unwrap(),
        last_day: llm_usage_core::calendar::parse_date("2026-09-25").unwrap(),
        granularity: Granularity::Day,
        filters: Filters::default(),
        today: llm_usage_core::calendar::parse_date("2026-09-25").unwrap(),
        retention_cutoff: None,
    };
    let s = query_summary(&reader, &req).unwrap();
    assert_eq!(
        s.totals.call_count, 1,
        "reader sees committed snapshot during writer transaction"
    );
    let value: Option<String> = reader
        .conn()
        .query_row("SELECT value FROM settings WHERE key = 'hold'", [], |r| {
            r.get(0)
        })
        .ok();
    assert_eq!(value, None, "未提交写对只读连接不可见");
    held.commit().unwrap();
}
