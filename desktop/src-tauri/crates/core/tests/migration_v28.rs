//! V28/M1a tests for historical source identity, partitions, and record exchange.
//! Check stable host IDs across renames and equal names, plus legacy_unknown ownership.
//! Current prerelease storage rejects older schemas; it does not migrate v3 to v4.
//! Check archive identity, ownership claims, partition rebuilding, versioned exchange fields,
//! duplicate skips, newer revision replacement, independent additions, and retained conflicts.

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
    // Renaming a host adds a name observation without creating a second local host ID.
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
    // Equal hostnames remain distinct because keys use host_id (V28).
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
    // Instance registrations under distinct host namespaces do not overwrite each other.
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

    // Verified local file ownership allows claiming the source as local.
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

    // Local scans must not change imported sources already assigned to another host.
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
    // Prerelease storage rejects older schemas; the app offers rebuilding rather than migration.
    let dir = TempDir::new("v28-old-version");
    {
        let storage = Storage::open(&dir.db_path()).unwrap();
        // Write one event so the database is nonempty.
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
        // Set the schema version to an older value.
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
    // Two instances have separate daily partitions; queries sum their contributions.
    // Source registration and partition identity survive detail deletion.
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
    // Deleting details retains source identity in partitions and registrations.
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
    // Exports join source registrations; run_adapter_scan registers instances before ingestion.
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
    // Versioned exchange JSON survives serialization and parsing.
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

    // No existing record: add the independent contribution.
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
    // Same source, key, revision, and content: skip the duplicate.
    assert_eq!(
        decide_record_merge(&base, &hash, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::SkipIdempotent
    );

    // Same revision with different content: retain a conflict regardless of token magnitude.
    let hash_b = content_hash(&"payload-b").to_string();
    assert_eq!(
        decide_record_merge(&base, &hash_b, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::Conflict
    );

    // A higher revision replaces the old contribution.
    let mut revised = base.clone();
    revised.source_revision = Some(6);
    assert_eq!(
        decide_record_merge(&revised, &hash_b, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::ReplaceAfterRevoke
    );

    // Equal revision numbers cannot identify a newer correction; changed content stays conflicting.
    // This matches ingest: ordinary replacement requires a higher revision number.
    assert_eq!(
        decide_record_merge(&base, &hash_b, Lifecycle::Corrected, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::Conflict
    );

    // With neither revision number present, a later corrected lifecycle can replace final.
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

    // A lower revision retains the existing record and reports differing content as a conflict.
    let mut older = base.clone();
    older.source_revision = Some(4);
    assert_eq!(
        decide_record_merge(&older, &hash_b, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::Conflict
    );

    // A different record key adds an independent contribution.
    let mut other_key = base.clone();
    other_key.source_record_key = "resp:2".into();
    assert_eq!(
        decide_record_merge(&other_key, &hash, Lifecycle::Final, Some(&existing)),
        llm_usage_core::exchange::MergeDecision::AddIndependent
    );
}

#[test]
fn tz_partition_repair_makes_history_visible_in_user_timezone() {
    // Regression: scans wrote UTC partitions while users queried Asia/Shanghai.
    // Different tz_version values hid retained events; rebuild partitions from those events.
    let (_dir, storage) = temp_storage("tz-repair");
    // Simulate UTC partitioning: 2026-09-25 18:30 UTC is Shanghai 09-26 02:30.
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
    // Rebuild the event range in the user's timezone.
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
    // Attribute the event to Shanghai date 2026-09-26 (18:30Z = 02:30+08).
    assert_eq!(after.periods[0].label, "2026-09-26");
    // Retain UTC partitions separately under tz_version.
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
    // WAL allows one writer and concurrent read-only connections during an uncommitted
    // transaction; M6 queries no longer wait for a whole scan to release the writer.
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
    // Read-only connections see the committed view while the writer transaction remains open.
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
