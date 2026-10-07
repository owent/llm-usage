mod common;
use common::*;
use llm_usage_core::detail_exchange::*;
use llm_usage_core::domain::*;
use llm_usage_core::exchange::{ExchangeKind, ExportRequest};
use llm_usage_core::ingest::commit_batch;
use llm_usage_core::storage::Storage;

fn sample() -> (TempDir, Storage, DetailExchange) {
    let (dir, s) = temp_storage("details-source");
    let now = ts("2026-10-07T10:00:00Z");
    let host = s.ensure_local_host("source-host", now).unwrap();
    s.conn().execute("INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,origin_host_id,created_at_ms,updated_at_ms) VALUES('source','agent-a','local_filesystem','verified',?1,?2,?2)",rusqlite::params![host,now]).unwrap();
    let mut e = with_tokens(evt("source", "one", now), 101, 7);
    e.source_revision = Some(1);
    e.session_id = Some("session".into());
    e.attempt_id = Some("attempt".into());
    e.duration_ms = Some(321);
    e.ttft_ms = Some(100);
    e.cost = Some(CostAmount {
        amount_minor: 123,
        currency: "CNY".into(),
        kind: CostKind::Estimated,
        price_version: Some("v".into()),
        billing_scope: Some("reference".into()),
    });
    commit_batch(
        &s,
        &batch(
            "source",
            "UTC",
            now,
            vec![e, evt("source", "unknown", now + 1)],
        ),
        None,
    )
    .unwrap();
    let export = build_details(
        &s,
        &ExportRequest {
            timezone: "UTC".into(),
            from_ms: 0,
            to_ms: now + 86_400_000,
            instances: None,
            redact_hostnames: true,
            kind: ExchangeKind::FullSnapshot,
            batch_id: "detail-test".into(),
        },
        now,
    )
    .unwrap();
    (dir, s, export)
}

#[test]
fn nonempty_details_roundtrip_preserves_whole_events_unknown_quality_and_revision() {
    let (_dir, source, p) = sample();
    assert_eq!(p.details.len(), 2);
    assert!(p.archive.daily_partitions.is_empty());
    assert!(p.archive.hourly_partitions.is_empty());
    let (_target, destination) = temp_storage("details-target");
    destination
        .ensure_local_host("destination", p.archive.exported_at_ms)
        .unwrap();
    let parsed: DetailExchange = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    let out = import_details(&destination, &parsed, p.archive.exported_at_ms).unwrap();
    assert_eq!(out.details_added, 2);
    let request = ExportRequest {
        timezone: "UTC".into(),
        from_ms: 0,
        to_ms: i64::MAX,
        instances: None,
        redact_hostnames: true,
        kind: ExchangeKind::FullSnapshot,
        batch_id: "roundtrip".into(),
    };
    let reexport = build_details(&destination, &request, p.archive.exported_at_ms).unwrap();
    for (a, b) in p.details.iter().zip(&reexport.details) {
        assert_eq!(
            serde_json::to_value(&a.event).unwrap(),
            serde_json::to_value(&b.event).unwrap()
        );
    }
    let sums = |s: &Storage| {
        s.conn().query_row("SELECT SUM(call_count),SUM(total_known_sum),SUM(total_unknown_count) FROM daily_usage",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).unwrap()
    };
    assert_eq!(sums(&source), sums(&destination));
    assert_eq!(sums(&destination).0, 2);
    assert_eq!(sums(&destination).1, 108);
    let unknown: Option<i64> = destination
        .conn()
        .query_row(
            "SELECT total_tokens FROM usage_events WHERE source_record_key='unknown'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unknown, None);
    let revision = destination.data_revision().unwrap();
    let replay = import_details(&destination, &parsed, p.archive.exported_at_ms + 1).unwrap();
    assert_eq!(replay.details_unchanged, 2);
    assert_eq!(destination.data_revision().unwrap(), revision);
    let enabled: i64 = destination
        .conn()
        .query_row("SELECT enabled FROM source_instances", [], |r| r.get(0))
        .unwrap();
    assert_eq!(enabled, 0);
}

#[test]
fn authority_conflict_history_and_cross_host_rejection() {
    let (_dir, _, mut p) = sample();
    let (_target, s) = temp_storage("details-conflict");
    let now = p.archive.exported_at_ms;
    import_details(&s, &p, now).unwrap();
    p.details[0].event.usage.output_total = Some(9);
    p.details[0].event.usage.total_tokens = Some(110);
    assert_eq!(
        import_details(&s, &p, now + 1).unwrap().details_conflicts,
        1
    );
    let conflict: i64 = s
        .conn()
        .query_row(
            "SELECT conflict FROM usage_events WHERE source_record_key='one'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conflict, 1);
    p.details[0].event.source_revision = Some(2);
    assert_eq!(import_details(&s, &p, now + 2).unwrap().details_updated, 1);
    let history: i64 = s
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(history > 0);
    let revision = s.data_revision().unwrap();
    p.archive.sources[0].origin_host_id = Some("different-host".into());
    assert!(import_details(&s, &p, now + 3).is_err());
    assert_eq!(s.data_revision().unwrap(), revision);
}

#[test]
fn invalid_late_source_rolls_back_entire_package_and_retention_floor_is_respected() {
    let (_dir, _, mut p) = sample();
    let (_target, s) = temp_storage("details-rollback");
    let now = p.archive.exported_at_ms;
    let mut declared = p.archive.sources[0].clone();
    declared.source_instance_id = "undeclared-agent-source".into();
    declared.attribution_status = "unverified".into();
    p.archive.sources.push(declared);
    assert!(import_details(&s, &p, now).is_err());
    for table in [
        "source_instances",
        "usage_events",
        "origin_hosts",
        "daily_usage",
    ] {
        let count: i64 = s
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
    p.archive.sources.pop();
    s.conn().execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES('detail_retention_floor_ms',?1,1,?2)",rusqlite::params![(now+1000).to_string(),now]).unwrap();
    assert_eq!(import_details(&s, &p, now).unwrap().details_skipped, 2);
    let count: i64 = s
        .conn()
        .query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn cumulative_archive_roundtrip_and_partial_days_cannot_destroy_history() {
    use llm_usage_core::aggregates::{
        upsert_source_aggregate, AggregateScope, Coverage, SourceAggregateInput,
    };
    let (_dir, source, p) = sample();
    let now = p.archive.exported_at_ms;
    let old = now - 86_400_000;
    commit_batch(
        &source,
        &batch(
            "source",
            "UTC",
            now,
            vec![with_tokens(evt("source", "old", old), 10, 2)],
        ),
        None,
    )
    .unwrap();
    source
        .conn()
        .execute(
            "UPDATE daily_usage SET sealed=1 WHERE local_day='2026-10-06'",
            [],
        )
        .unwrap();
    source
        .conn()
        .execute("DELETE FROM usage_events WHERE source_record_key='old'", [])
        .unwrap();
    upsert_source_aggregate(
        &source,
        &SourceAggregateInput {
            instance_id: "source".into(),
            scope: AggregateScope::Session,
            scope_key: "reconciliation".into(),
            interval_start_ms: Some(old),
            interval_end_ms: now,
            interval_end_inclusive: true,
            usage: TokenUsage {
                input_total: Some(999),
                ..Default::default()
            },
            quality: TokenQuality {
                input_total: FieldQuality::Reported,
                ..Default::default()
            },
            reported_call_count: None,
            coverage: Coverage::OverlapUnknown,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(2),
        },
        now,
    )
    .unwrap();
    let mut request = ExportRequest {
        timezone: "UTC".into(),
        from_ms: 0,
        to_ms: i64::MAX,
        instances: None,
        redact_hostnames: true,
        kind: ExchangeKind::FullSnapshot,
        batch_id: "archive".into(),
    };
    let package = build_details(&source, &request, now).unwrap();
    assert_eq!(package.details.len(), 2);
    assert_eq!(package.cumulative.len(), 1);
    assert_eq!(package.archive.daily_partitions.len(), 1);
    let (_dest, s) = temp_storage("detail-archive");
    assert_eq!(
        import_details(&s, &package, now)
            .unwrap()
            .cumulative_changed,
        1
    );
    let old_total: i64 = s
        .conn()
        .query_row(
            "SELECT total_known_sum FROM daily_usage WHERE local_day='2026-10-06'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(old_total, 12);
    assert_eq!(
        import_details(&s, &package, now + 1)
            .unwrap()
            .cumulative_changed,
        0
    );
    request.from_ms = now + 1;
    assert!(build_details(&source, &request, now).is_err());
    let (_aggregate, aggregate_target) = temp_storage("detail-aggregate-target");
    let aggregate = llm_usage_core::exchange::build_aggregate_export(
        &source,
        &ExportRequest {
            from_ms: 0,
            ..request
        },
        now,
    )
    .unwrap();
    llm_usage_core::exchange_import::import_aggregate(&aggregate_target, &aggregate, now).unwrap();
    let revision = aggregate_target.data_revision().unwrap();
    assert!(import_details(&aggregate_target, &package, now + 2).is_err());
    assert_eq!(aggregate_target.data_revision().unwrap(), revision);
}

#[test]
fn enabled_pricing_rebuilds_source_cost_in_the_merge_transaction() {
    let (_dir, _, p) = sample();
    let (_target, s) = temp_storage("detail-cost");
    import_details_with_pricing(&s, &p, p.archive.exported_at_ms, Some(&Default::default()))
        .unwrap();
    let cost:i64=s.conn().query_row("SELECT total_amount_minor FROM daily_cost_usage WHERE kind='source_estimate' AND currency='CNY'",[],|r|r.get(0)).unwrap();
    assert_eq!(cost, 123);
    let revision = s.data_revision().unwrap();
    import_details_with_pricing(
        &s,
        &p,
        p.archive.exported_at_ms + 1,
        Some(&Default::default()),
    )
    .unwrap();
    assert_eq!(s.data_revision().unwrap(), revision);
}
