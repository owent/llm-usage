mod common;
use common::*;
use llm_usage_core::{
    adapters::{built_in_adapters, framework::*, zed::ZedAdapter},
    aggregates::recent_zed_thread_usage,
    error::CoreError,
    jobs::TriggerKind,
    storage::Storage,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

const NOW: i64 = 1_800_000_000_000;
fn row() -> Value {
    serde_json::from_str::<Value>(include_str!("fixtures/zed/native-1.26.2-mixed.json")).unwrap()
        ["threads"][0]
        .clone()
}
fn source(dir: &TempDir) -> std::path::PathBuf {
    let p = dir.path().join("threads.db");
    let db = rusqlite::Connection::open(&p).unwrap();
    db.execute("CREATE TABLE threads(id TEXT PRIMARY KEY,summary TEXT NOT NULL,updated_at TEXT NOT NULL,data_type TEXT NOT NULL,data BLOB NOT NULL,created_at TEXT)",[]).unwrap();
    drop(db);
    p
}
fn put(path: &Path, id: &str, doc: &Value) {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute(
        "INSERT OR REPLACE INTO threads VALUES(?1,'redacted',?2,'zstd',?3,?4)",
        rusqlite::params![
            id,
            doc["updated_at"].as_str().unwrap(),
            zstd::stream::encode_all(serde_json::to_vec(doc).unwrap().as_slice(), 3).unwrap(),
            doc["created_at"].as_str().unwrap()
        ],
    )
    .unwrap();
}
fn ctx(path: &Path) -> DiscoverContext {
    DiscoverContext {
        home_dir: None,
        manual_roots: vec![path.into()],
        ..Default::default()
    }
}
fn config(now: i64) -> RunConfig {
    RunConfig {
        timezone: "Asia/Shanghai".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("zed-mixed-{now}"),
    }
}
fn count(s: &Storage, sql: &str) -> i64 {
    s.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn parallel(s: &Mutex<Storage>, path: &Path, now: i64) {
    parallel_checked(s, path, now, false);
}
fn parallel_checked(s: &Mutex<Storage>, path: &Path, now: i64, expected_failure: bool) {
    let adapters = built_in_adapters();
    let requests = adapters
        .iter()
        .map(|a| ParallelScanRequest {
            adapter: a.as_ref(),
            context: llm_usage_core::adapters::routing::context_for_adapter(
                &ctx(path),
                a.adapter_id(),
                &[],
            ),
            config: RunConfig {
                run_id_prefix: format!("zed-mixed-{now}-{}", a.adapter_id()),
                ..config(now)
            },
            filter: InstanceFilter::default(),
        })
        .collect::<Vec<_>>();
    for result in run_adapter_scans_parallel(s, &requests, None, Arc::new(|| true), None) {
        for report in result.unwrap() {
            assert_eq!(report.error.is_some(), expected_failure, "{report:?}");
        }
    }
}

#[test]
fn native_mixed_thread_imports_output_without_inventing_input_or_model_calls() {
    for provider in [
        "deepseek",
        "custom-glm-provider",
        "llm-usage-zhipu",
        "zed.dev",
    ] {
        let dir = TempDir::new("zed-mixed-native");
        let p = source(&dir);
        let mut doc = row();
        doc["model"]["provider"] = provider.into();
        if provider != "deepseek" {
            doc["model"]["model"] = "glm-5.3".into();
        }
        put(&p, "mixed", &doc);
        let (_d, s) = temp_storage("zed-mixed-native");
        let s = Mutex::new(s);
        parallel(&s, &p, NOW);
        let revision;
        {
            let s = s.lock().unwrap();
            assert_eq!(count(&s, "SELECT COUNT(*) FROM usage_events"), 0);
            assert_eq!(count(&s, "SELECT COUNT(*) FROM source_aggregates"), 1);
            assert_eq!(count(&s, "SELECT output_total FROM source_aggregates"), 164);
            assert_eq!(
                count(&s, "SELECT reported_call_count FROM source_aggregates"),
                2
            );
            assert_eq!(count(&s,"SELECT COUNT(*) FROM source_aggregates WHERE input_uncached IS NOT NULL OR input_total IS NOT NULL OR total_tokens IS NOT NULL OR input_cache_read IS NOT NULL OR input_cache_write IS NOT NULL"),0);
            assert_eq!(
                count(
                    &s,
                    "SELECT COUNT(*) FROM source_files WHERE status='active'"
                ),
                1
            );
            assert_eq!(count(&s, "SELECT COUNT(*) FROM source_instances"), 1);
            let instance: String = s
                .conn()
                .query_row("SELECT instance_id FROM source_aggregates", [], |r| {
                    r.get(0)
                })
                .unwrap();
            let totals =
                llm_usage_core::aggregates::sum_exclusive_aggregates(&s, &instance).unwrap();
            assert_eq!(totals.output_total, Some(164));
            assert_eq!(totals.input_total, None);
            assert_eq!(totals.total_tokens, None);
            assert_eq!(
                summary(&s, "2026-10-29", "2026-10-29")
                    .totals
                    .output_total_known,
                None
            );
            revision = s.data_revision().unwrap();
        }
        parallel(&s, &p, NOW + 1);
        assert_eq!(s.lock().unwrap().data_revision().unwrap(), revision);
    }
}

#[test]
fn recent_zed_usage_reads_retained_intervals_without_changing_daily_or_model_stats() {
    let dir = TempDir::new("zed-thread-usage");
    let path = source(&dir);
    let mut doc = row();
    doc["model"] = json!({"provider": "custom-zhipu", "model": "glm-5.4"});
    doc["created_at"] = "2026-10-29T14:00:00Z".into();
    doc["cumulative_token_usage"] = json!({
        "input_tokens":213009,"output_tokens":6498,"cache_read_input_tokens":786240,
    });
    for day in 1..=22 {
        doc["updated_at"] = format!("2026-11-{day:02}T06:35:39Z").into();
        put(&path, &format!("usage-{day:02}"), &doc);
    }
    let other = TempDir::new("zed-thread-other-user");
    let other_path = source(&other);
    doc["updated_at"] = "2026-11-27T06:35:39Z".into();
    put(&other_path, "other-user-thread", &doc);
    let (_app, storage) = temp_storage("zed-thread-usage");
    let storage = Mutex::new(storage);
    parallel(&storage, &path, NOW);
    parallel(&storage, &other_path, NOW + 1);
    let storage = storage.lock().unwrap();
    storage.conn().execute_batch(
        "INSERT INTO users VALUES ('other-user', 'Other', 0);
         UPDATE source_instances SET user_id = 'other-user' WHERE instance_id IN (
           SELECT instance_id FROM source_aggregates WHERE scope_key = 'zed:thread:other-user-thread'
         );
         UPDATE source_aggregates SET coverage = 'overlap_unknown'
           WHERE scope_key = 'zed:thread:usage-22';",
    ).unwrap();

    let revision = storage.data_revision().unwrap();
    let rows = recent_zed_thread_usage(&storage, "default").unwrap();
    assert_eq!(rows.len(), 20);
    assert!(rows.windows(2).all(|pair| pair[0].end_ms > pair[1].end_ms));
    let newest: i64 = storage
        .conn()
        .query_row(
            "SELECT interval_end_ms FROM source_aggregates WHERE scope_key = 'zed:thread:usage-21'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows[0].end_ms, newest);
    assert_eq!(rows[0].start_ms, Some(1_793_282_400_000));
    assert_eq!(rows[0].output_total, Some(6498));
    assert_eq!(rows[0].input_cache_read, Some(786240));
    assert_eq!(rows[0].observed_turns, Some(2));
    assert_eq!(rows[0].input_total, None);
    assert_eq!(rows[0].input_uncached, None);
    assert_eq!(rows[0].input_cache_write, None);
    assert_eq!(rows[0].total_tokens, None);
    assert_eq!(
        recent_zed_thread_usage(&storage, "other-user")
            .unwrap()
            .len(),
        1
    );
    assert!(recent_zed_thread_usage(&storage, "absent")
        .unwrap()
        .is_empty());
    let day = summary(&storage, "2026-11-21", "2026-11-21");
    assert_eq!(day.totals.output_total_known, None);
    assert_eq!(day.totals.call_count, 0);
    assert!(day.model_breakdown.is_empty());
    assert_eq!(storage.data_revision().unwrap(), revision);
}

#[test]
fn recent_zed_usage_excludes_unverified_non_zed_non_session_duplicate_and_empty_scope_rows() {
    use llm_usage_core::aggregates::{
        upsert_source_aggregate, AggregateScope, Coverage, SourceAggregateInput,
    };
    use llm_usage_core::domain::{FieldQuality, TimeBasis, TokenQuality, TokenUsage};

    let (_app, storage) = temp_storage("zed-thread-filters");
    storage
        .conn()
        .execute_batch(
            "INSERT INTO source_instances (instance_id, agent, locality_basis, attribution_status, enabled, format, parser_version, capabilities, health, origin_host_id, user_id, created_at_ms, updated_at_ms) VALUES
             ('zed@verified', 'zed', 'local_filesystem', 'verified', 1, 'zed_threads_db', 'zed-threads-db-2', '{}', 'ok', 'local', 'default', 1, 1),
             ('zed@pending', 'zed', 'local_filesystem', 'pending', 1, 'zed_threads_db', 'zed-threads-db-2', '{}', 'ok', 'local', 'default', 1, 1),
             ('zed@excluded', 'zed', 'local_filesystem', 'excluded', 1, 'zed_threads_db', 'zed-threads-db-2', '{}', 'ok', 'local', 'default', 1, 1),
             ('hermes@verified', 'hermes', 'local_filesystem', 'verified', 1, 'hermes_db', 'hermes-1', '{}', 'ok', 'local', 'default', 1, 1);",
        )
        .unwrap();

    let make = |instance_id: &str,
                scope: AggregateScope,
                scope_key: &str,
                coverage: Coverage,
                end_ms: i64| SourceAggregateInput {
        instance_id: instance_id.into(),
        scope,
        scope_key: scope_key.into(),
        interval_start_ms: Some(1_793_282_400_000),
        interval_end_ms: end_ms,
        interval_end_inclusive: false,
        usage: TokenUsage {
            output_total: Some(100),
            ..Default::default()
        },
        quality: TokenQuality {
            output_total: FieldQuality::Reported,
            ..Default::default()
        },
        reported_call_count: Some(1),
        coverage,
        duplicate_of: (coverage == Coverage::Duplicate).then(|| "zed:thread:valid".into()),
        time_basis: TimeBasis::Uncertain,
        source_revision: Some(end_ms),
    };

    for input in [
        make(
            "zed@verified",
            AggregateScope::Session,
            "zed:thread:valid",
            Coverage::Exclusive,
            1_795_000_000_000,
        ),
        make(
            "zed@pending",
            AggregateScope::Session,
            "zed:thread:pending-row",
            Coverage::Exclusive,
            1_795_100_000_000,
        ),
        make(
            "zed@excluded",
            AggregateScope::Session,
            "zed:thread:excluded-row",
            Coverage::Exclusive,
            1_795_200_000_000,
        ),
        make(
            "hermes@verified",
            AggregateScope::Session,
            "zed:thread:wrong-agent",
            Coverage::Exclusive,
            1_795_300_000_000,
        ),
        make(
            "zed@verified",
            AggregateScope::Day,
            "zed:thread:wrong-scope",
            Coverage::Exclusive,
            1_795_400_000_000,
        ),
        make(
            "zed@verified",
            AggregateScope::Session,
            "zed:thread:",
            Coverage::Exclusive,
            1_795_500_000_000,
        ),
        make(
            "zed@verified",
            AggregateScope::Session,
            "zed:other:1",
            Coverage::Exclusive,
            1_795_600_000_000,
        ),
        make(
            "zed@verified",
            AggregateScope::Session,
            "zed:thread:dup",
            Coverage::Duplicate,
            1_795_700_000_000,
        ),
        make(
            "zed@verified",
            AggregateScope::Session,
            "zed:thread:overlap",
            Coverage::OverlapUnknown,
            1_795_800_000_000,
        ),
    ] {
        upsert_source_aggregate(&storage, &input, NOW).unwrap();
    }

    let revision = storage.data_revision().unwrap();
    let rows = recent_zed_thread_usage(&storage, "default").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].end_ms, 1_795_000_000_000);
    assert_eq!(rows[0].output_total, Some(100));
    assert_eq!(storage.data_revision().unwrap(), revision);
}

#[test]
fn recent_zed_usage_preserves_reported_zero_derived_large_tokens_null_start_and_tie_breaks() {
    use llm_usage_core::aggregates::{
        upsert_source_aggregate, AggregateScope, Coverage, SourceAggregateInput,
    };
    use llm_usage_core::domain::{
        FieldQuality, TimeBasis, TokenQuality, TokenUsage, MAX_TOKEN_VALUE,
    };

    let (_app, storage) = temp_storage("zed-thread-semantics");
    storage
        .conn()
        .execute_batch(
            "INSERT INTO source_instances (instance_id, agent, locality_basis, attribution_status, enabled, format, parser_version, capabilities, health, origin_host_id, user_id, created_at_ms, updated_at_ms) VALUES
             ('zed@root-a', 'zed', 'local_filesystem', 'verified', 1, 'zed_threads_db', 'zed-threads-db-2', '{}', 'ok', 'local', 'default', 1, 1),
             ('zed@root-b', 'zed', 'local_filesystem', 'verified', 0, 'zed_threads_db', 'zed-threads-db-2', '{}', 'degraded', 'local', 'default', 1, 1);",
        )
        .unwrap();

    let tie_end = 1_796_000_000_000;
    let rich = SourceAggregateInput {
        instance_id: "zed@root-a".into(),
        scope: AggregateScope::Session,
        scope_key: "zed:thread:rich".into(),
        interval_start_ms: None,
        interval_end_ms: tie_end + 1_000,
        interval_end_inclusive: false,
        usage: TokenUsage {
            input_total: Some(0),
            input_uncached: Some(42),
            input_cache_read: Some(MAX_TOKEN_VALUE),
            input_cache_write: Some(7),
            output_total: Some(0),
            output_reasoning: None,
            total_tokens: Some(9_007_199_254_740_993),
            source_total: None,
        },
        quality: TokenQuality {
            input_total: FieldQuality::Reported,
            input_uncached: FieldQuality::Derived,
            input_cache_read: FieldQuality::Reported,
            input_cache_write: FieldQuality::Estimated,
            output_total: FieldQuality::Reported,
            output_reasoning: FieldQuality::Unknown,
            total_tokens: FieldQuality::Derived,
            source_total: FieldQuality::Unknown,
        },
        reported_call_count: Some(0),
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::Uncertain,
        source_revision: Some(tie_end + 1_000),
    };
    upsert_source_aggregate(&storage, &rich, NOW).unwrap();

    for (instance, key, turns) in [
        ("zed@root-a", "zed:thread:tie-1", None),
        ("zed@root-b", "zed:thread:tie-2", Some(3)),
    ] {
        let input = SourceAggregateInput {
            instance_id: instance.into(),
            scope: AggregateScope::Session,
            scope_key: key.into(),
            interval_start_ms: Some(tie_end - 5_000),
            interval_end_ms: tie_end,
            interval_end_inclusive: false,
            usage: TokenUsage {
                output_total: Some(12),
                ..Default::default()
            },
            quality: TokenQuality {
                output_total: FieldQuality::Reported,
                ..Default::default()
            },
            reported_call_count: turns,
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(tie_end),
        };
        upsert_source_aggregate(&storage, &input, NOW).unwrap();
    }
    // Force an Unknown quality marker with a non-null SQLite integer to verify quality gating.
    storage
        .conn()
        .execute(
            "UPDATE source_aggregates SET input_uncached = 999 WHERE scope_key = 'zed:thread:tie-1'",
            [],
        )
        .unwrap();

    // Add 20 older rows across both roots so total exceeds 20 and tests cross-instance merging & cap.
    for i in 0..20 {
        let end_ms = tie_end - 10_000 - i * 1_000;
        let input = SourceAggregateInput {
            instance_id: if i % 2 == 0 {
                "zed@root-a".into()
            } else {
                "zed@root-b".into()
            },
            scope: AggregateScope::Session,
            scope_key: format!("zed:thread:older-{i:02}"),
            interval_start_ms: Some(end_ms - 500),
            interval_end_ms: end_ms,
            interval_end_inclusive: false,
            usage: TokenUsage {
                output_total: Some(1 + i),
                ..Default::default()
            },
            quality: TokenQuality {
                output_total: FieldQuality::Reported,
                ..Default::default()
            },
            reported_call_count: Some(1),
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        };
        upsert_source_aggregate(&storage, &input, NOW).unwrap();
    }

    let rows = recent_zed_thread_usage(&storage, "default").unwrap();
    assert_eq!(rows.len(), 20);
    assert_eq!(rows[0].start_ms, None);
    assert_eq!(rows[0].end_ms, tie_end + 1_000);
    assert_eq!(rows[0].input_total, Some(0));
    assert_eq!(rows[0].input_uncached, Some(42));
    assert_eq!(rows[0].input_cache_read, Some(MAX_TOKEN_VALUE));
    assert_eq!(rows[0].input_cache_write, None);
    assert_eq!(rows[0].output_total, Some(0));
    assert_eq!(rows[0].total_tokens, Some(9_007_199_254_740_993));
    assert_eq!(rows[0].observed_turns, Some(0));

    // Equal end_ms rows are ordered deterministically by aggregate_id ascending.
    assert_eq!(rows[1].end_ms, tie_end);
    assert_eq!(rows[2].end_ms, tie_end);
    assert!(rows[1].id < rows[2].id);
    // Disabled/degraded instance rows are still retained and merged with active instances.
    let tie_turns: Vec<Option<i64>> = rows[1..=2].iter().map(|r| r.observed_turns).collect();
    assert!(tie_turns.contains(&None));
    assert!(tie_turns.contains(&Some(3)));
    // Non-null SQLite integer with Unknown quality must remain None.
    assert!(rows[1..=2].iter().all(|r| r.input_uncached.is_none()));
}

struct OldSkip(bool);
impl SourceAdapter for OldSkip {
    fn adapter_id(&self) -> &'static str {
        "zed"
    }
    fn agent(&self) -> &'static str {
        "zed"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        ZedAdapter::new().discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        ZedAdapter::new().instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        ZedAdapter::new().detect(p)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = ZedAdapter::new().capability();
        c.maintenance["parser_version"] = "zed-threads-db-2".into();
        c
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut o = ZedAdapter::new().scan(t, s, l, n)?;
        if self.0 {
            for aggregate in &mut o.aggregates {
                aggregate.usage.input_uncached = Some(47748);
                aggregate.quality.input_uncached = llm_usage_core::domain::FieldQuality::Reported;
            }
        } else {
            o.aggregates.clear();
        }
        o.cursor =
            Some(json!({"generation":t.generation,"offset":t.probe.len,"last_thread_id":"zzzz"}));
        o.parse_context = None;
        o.diagnostics.clear();
        if !self.0 {
            o.diagnostics.push(llm_usage_core::ingest::DiagnosticInput {
                event_id: None,
                code: "non_zed_dev_thread_skipped".into(),
                field: None,
                position: None,
                message: "old provider filter".into(),
            });
        }
        Ok(o)
    }
}

#[test]
fn consumed_old_cursor_replays_through_parallel_registry_and_write_rollback() {
    let dir = TempDir::new("zed-mixed-old");
    let p = source(&dir);
    put(&p, "mixed", &row());
    let bytes = std::fs::read(&p).unwrap();
    let (_d, s) = temp_storage("zed-mixed-old");
    run_adapter_scan(&s, &OldSkip(false), &ctx(&p), &config(NOW)).unwrap();
    assert_eq!(count(&s, "SELECT COUNT(*) FROM source_aggregates"), 0);
    let checkpoint: (String, Option<String>) = s
        .conn()
        .query_row(
            "SELECT cursor_value,parse_context FROM ingestion_checkpoints",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    s.conn().execute_batch("CREATE TRIGGER reject_zed_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'checkpoint failure'); END;").unwrap();
    let s = Mutex::new(s);
    parallel_checked(&s, &p, NOW + 1, true);
    {
        let s = s.lock().unwrap();
        assert_eq!(count(&s, "SELECT COUNT(*) FROM source_aggregates"), 0);
        let after: (String, Option<String>) = s
            .conn()
            .query_row(
                "SELECT cursor_value,parse_context FROM ingestion_checkpoints",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(after, checkpoint);
        s.conn()
            .execute_batch("DROP TRIGGER reject_zed_checkpoint; CREATE TRIGGER reject_zed_aggregate BEFORE INSERT ON source_aggregates BEGIN SELECT RAISE(ABORT,'aggregate failure'); END;")
            .unwrap();
    }
    parallel_checked(&s, &p, NOW + 2, true);
    {
        let s = s.lock().unwrap();
        assert_eq!(count(&s, "SELECT COUNT(*) FROM source_aggregates"), 0);
        let after: (String, Option<String>) = s
            .conn()
            .query_row(
                "SELECT cursor_value,parse_context FROM ingestion_checkpoints",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(after, checkpoint);
        assert_eq!(
            count(
                &s,
                "SELECT COUNT(*) FROM diagnostics WHERE code='non_zed_dev_thread_skipped'"
            ),
            1
        );
        s.conn()
            .execute_batch("DROP TRIGGER reject_zed_aggregate")
            .unwrap();
    }
    parallel(&s, &p, NOW + 3);
    let s = s.lock().unwrap();
    assert_eq!(count(&s, "SELECT output_total FROM source_aggregates"), 164);
    assert_eq!(
        count(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='non_zed_dev_thread_skipped'"
        ),
        1
    );
    assert_eq!(count(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.version')=3"),1);
    assert_eq!(std::fs::read(p).unwrap(), bytes);
}

#[test]
fn native_policy_keeps_existing_input_and_real_conflicts_at_the_same_revision() {
    let dir = TempDir::new("zed-mixed-old-input");
    let p = source(&dir);
    let mut doc = row();
    doc["model"]["provider"] = "llm-usage-zhipu".into();
    doc["model"]["model"] = "glm-5.3".into();
    put(&p, "mixed", &doc);
    let (_d, s) = temp_storage("zed-mixed-old-input");
    run_adapter_scan(&s, &OldSkip(true), &ctx(&p), &config(NOW)).unwrap();
    let before: (i64, String) = s
        .conn()
        .query_row(
            "SELECT input_uncached,content_hash FROM source_aggregates",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let s = Mutex::new(s);
    parallel(&s, &p, NOW + 1);
    parallel(&s, &p, NOW + 2);
    let s = s.lock().unwrap();
    let after: (i64, String) = s
        .conn()
        .query_row(
            "SELECT input_uncached,content_hash FROM source_aggregates",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(before, after);
    assert!(
        count(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_conflict'"
        ) > 0
    );
    assert_eq!(
        count(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='aggregate_parser_policy_upgrade'"
        ),
        0
    );
    assert_eq!(count(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.version')=3"),1);
}

#[test]
fn invalid_rows_keep_known_output_and_policy_updates_wait_for_a_valid_snapshot() {
    let dir = TempDir::new("zed-mixed-invalid");
    let p = source(&dir);
    put(&p, "good", &row());
    let mut bad = row();
    bad["cumulative_token_usage"]["output_tokens"] = (-1).into();
    put(&p, "bad", &bad);
    let (_d, s) = temp_storage("zed-mixed-invalid");
    run_adapter_scan(&s, &ZedAdapter::new(), &ctx(&p), &config(NOW)).unwrap();
    assert_eq!(
        count(&s, "SELECT SUM(output_total) FROM source_aggregates"),
        164
    );
    assert_eq!(
        count(
            &s,
            "SELECT COUNT(*) FROM source_files WHERE status='degraded'"
        ),
        1
    );
    assert_eq!(count(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.version') IS NULL"),1);
    bad["cumulative_token_usage"] = json!({});
    bad["request_token_usage"] = json!({});
    put(&p, "bad", &bad);
    run_adapter_scan(&s, &ZedAdapter::new(), &ctx(&p), &config(NOW + 1)).unwrap();
    assert_eq!(count(&s, "SELECT COUNT(*) FROM source_aggregates"), 1);
    assert_eq!(
        count(
            &s,
            "SELECT COUNT(*) FROM source_files WHERE status='active'"
        ),
        1
    );
    assert_eq!(count(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.version')=3"),1);
    assert!(
        count(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='token_usage_shape_deviation'"
        ) > 0
    );
}

#[test]
fn continued_policy_page_does_not_restart_or_hide_earlier_errors() {
    let dir = TempDir::new("zed-mixed-page");
    let p = source(&dir);
    put(&p, "a", &row());
    put(&p, "b", &row());
    let t = ScanTarget {
        instance_id: "zed-page".into(),
        path: p.clone(),
        file_id: "file".into(),
        file_identity: "identity".into(),
        probe: llm_usage_core::adapters::jsonl::probe_file(&p).unwrap(),
        generation: 0,
        rescan: false,
    };
    for invalid in [false, true] {
        let state = StoredScanState {
            cursor: Some(json!({"generation":0,"offset":0,"last_thread_id":"a"})),
            parse_context: Some(json!({"in_progress":true,"invalid":invalid})),
        };
        let o = ZedAdapter::new()
            .scan(&t, &state, &ScanLimits::default(), NOW)
            .unwrap();
        assert_eq!(o.records_seen, 1);
        assert_eq!(o.aggregates.len(), 1);
        assert_eq!(
            o.parse_context
                .as_ref()
                .unwrap()
                .get("version")
                .and_then(Value::as_u64),
            (!invalid).then_some(3)
        );
        assert_eq!(o.health, if invalid { "degraded" } else { "active" });
    }
}

#[test]
fn native_overflowing_ambiguous_totals_preserve_reported_output_and_cache() {
    for cumulative_overflow in [false, true] {
        let dir = TempDir::new("zed-mixed-overflow");
        let p = source(&dir);
        let mut doc = row();
        doc["cumulative_token_usage"]["cache_read_input_tokens"] = 128.into();
        if cumulative_overflow {
            doc["cumulative_token_usage"]["input_tokens"] =
                llm_usage_core::domain::MAX_TOKEN_VALUE.into();
            doc["cumulative_token_usage"]["cache_read_input_tokens"] =
                llm_usage_core::domain::MAX_TOKEN_VALUE.into();
        } else {
            for bucket in doc["request_token_usage"]
                .as_object_mut()
                .unwrap()
                .values_mut()
            {
                bucket["input_tokens"] = llm_usage_core::domain::MAX_TOKEN_VALUE.into();
                bucket["cache_read_input_tokens"] = llm_usage_core::domain::MAX_TOKEN_VALUE.into();
            }
        }
        put(&p, "mixed", &doc);
        let t = ScanTarget {
            instance_id: "zed-mixed-overflow".into(),
            path: p.clone(),
            file_id: "file".into(),
            file_identity: "identity".into(),
            probe: llm_usage_core::adapters::jsonl::probe_file(&p).unwrap(),
            generation: 0,
            rescan: false,
        };
        let outcome = ZedAdapter::new()
            .scan(&t, &StoredScanState::default(), &ScanLimits::default(), NOW)
            .unwrap();
        assert_eq!(outcome.status, ScanStatus::Complete);
        assert_eq!(outcome.health, "active");
        assert_eq!(outcome.aggregates.len(), 1);
        let usage = &outcome.aggregates[0].usage;
        assert_eq!(usage.output_total, Some(164));
        assert_eq!(
            usage.input_cache_read,
            Some(if cumulative_overflow {
                llm_usage_core::domain::MAX_TOKEN_VALUE
            } else {
                128
            })
        );
        assert_eq!(usage.input_uncached, None);
        assert_eq!(usage.input_total, None);
        assert_eq!(usage.total_tokens, None);
        assert_eq!(outcome.reconciliations.len(), 1);
        assert_eq!(outcome.reconciliations[0].verdict, "matched");
        assert_eq!(
            outcome
                .parse_context
                .as_ref()
                .unwrap()
                .get("version")
                .and_then(Value::as_u64),
            Some(3)
        );
    }
}

#[test]
fn mixed_input_and_cache_are_not_added_in_output_reconciliation() {
    let dir = TempDir::new("zed-mixed-cache");
    let p = source(&dir);
    let mut doc = row();
    doc["cumulative_token_usage"]["cache_read_input_tokens"] = 128.into();
    put(&p, "mixed", &doc);
    let t = ScanTarget {
        instance_id: "zed-mixed-cache".into(),
        path: p.clone(),
        file_id: "file".into(),
        file_identity: "identity".into(),
        probe: llm_usage_core::adapters::jsonl::probe_file(&p).unwrap(),
        generation: 0,
        rescan: false,
    };
    let outcome = ZedAdapter::new()
        .scan(&t, &StoredScanState::default(), &ScanLimits::default(), NOW)
        .unwrap();
    assert_eq!(outcome.aggregates[0].usage.input_cache_read, Some(128));
    assert_eq!(outcome.aggregates[0].usage.input_total, None);
    assert_eq!(outcome.reconciliations.len(), 1);
    let r = &outcome.reconciliations[0];
    assert_eq!(r.series, "request_output_tokens_vs_cumulative");
    assert_eq!(r.detail_sum, 164);
    assert_eq!(r.snapshot_final, Some(164));
    assert_eq!(r.difference, Some(0));
    for bucket in doc["request_token_usage"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        bucket["output_tokens"] = 0.into();
    }
    doc["cumulative_token_usage"]["output_tokens"] = 0.into();
    put(&p, "mixed", &doc);
    let outcome = ZedAdapter::new()
        .scan(&t, &StoredScanState::default(), &ScanLimits::default(), NOW)
        .unwrap();
    assert_eq!(outcome.aggregates[0].usage.output_total, None);
    assert!(outcome.reconciliations.is_empty());
}

#[test]
fn native_output_reconciliation_rejects_incomplete_comparisons_without_losing_usage() {
    let dir = TempDir::new("zed-mixed-output-reconciliation");
    let p = source(&dir);
    let t = ScanTarget {
        instance_id: "zed-mixed-output-reconciliation".into(),
        path: p.clone(),
        file_id: "file".into(),
        file_identity: "identity".into(),
        probe: llm_usage_core::adapters::jsonl::probe_file(&p).unwrap(),
        generation: 0,
        rescan: false,
    };
    for (case, expected) in [
        ("mismatch", Some((101, "mismatch"))),
        ("zero_bucket", None),
        ("missing_bucket", None),
        ("output_overflow", None),
    ] {
        let mut doc = row();
        let mut buckets = doc["request_token_usage"]
            .as_object_mut()
            .unwrap()
            .values_mut();
        let first = buckets.next().unwrap();
        let second = buckets.next().unwrap();
        match case {
            "mismatch" => {
                first["output_tokens"] = 100.into();
                second["output_tokens"] = 1.into();
            }
            "zero_bucket" => {
                first["output_tokens"] = 1.into();
                second["output_tokens"] = 0.into();
            }
            "missing_bucket" => {
                first["output_tokens"] = 1.into();
                second.as_object_mut().unwrap().remove("output_tokens");
            }
            "output_overflow" => {
                first["output_tokens"] = llm_usage_core::domain::MAX_TOKEN_VALUE.into();
                second["output_tokens"] = llm_usage_core::domain::MAX_TOKEN_VALUE.into();
            }
            _ => unreachable!(),
        }
        put(&p, "mixed", &doc);
        let outcome = ZedAdapter::new()
            .scan(&t, &StoredScanState::default(), &ScanLimits::default(), NOW)
            .unwrap();
        assert_eq!(outcome.aggregates.len(), 1, "{case}");
        assert_eq!(
            outcome.aggregates[0].usage.output_total,
            Some(164),
            "{case}"
        );
        assert_eq!(outcome.aggregates[0].reported_call_count, Some(2), "{case}");
        match expected {
            Some((sum, verdict)) => {
                assert_eq!(outcome.reconciliations.len(), 1, "{case}");
                let comparison = &outcome.reconciliations[0];
                assert_eq!(comparison.detail_sum, sum, "{case}");
                assert_eq!(comparison.snapshot_final, Some(164), "{case}");
                assert_eq!(comparison.difference, Some(sum - 164), "{case}");
                assert_eq!(comparison.verdict, verdict, "{case}");
            }
            None => assert!(outcome.reconciliations.is_empty(), "{case}"),
        }
    }
}
