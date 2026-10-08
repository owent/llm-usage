//! Parser upgrades change only parsing metadata; test old conflicts, contradictions, order and rollback.
mod common;

use common::*;
use llm_usage_core::domain::{EventInput, FieldQuality, Lifecycle};
use llm_usage_core::identity::{content_hash, event_content_hash};
use llm_usage_core::ingest::{commit_batch, FaultPoint};
use llm_usage_core::storage::Storage;

const DAY: &str = "2026-09-24";

fn seed(storage: &Storage, revision: Option<i64>, legacy_hash: bool) -> EventInput {
    let mut event = with_tokens(evt("inst", "call", ts("2026-09-24T10:00:00Z")), 100, 20);
    event.parser_version = "parser-1".into();
    event.source_revision = revision;
    event.observed_at_ms = Some(event.occurred_at_ms + 1);
    let mut initial = batch("inst", "UTC", event.occurred_at_ms, vec![event.clone()]);
    initial
        .diagnostics
        .push(llm_usage_core::ingest::DiagnosticInput {
            event_id: None,
            code: "existing_audit".into(),
            field: None,
            position: None,
            message: "synthetic existing audit".into(),
        });
    commit_batch(storage, &initial, None).unwrap();
    let hash = if legacy_hash {
        content_hash(&event)
    } else {
        event_content_hash(&event)
    };
    storage
        .conn()
        .execute("UPDATE usage_events SET conflict=1,content_hash=?1", [hash])
        .unwrap();
    storage
        .conn()
        .execute("UPDATE daily_usage SET conflict_count=1", [])
        .unwrap();
    storage
        .conn()
        .execute("UPDATE hourly_usage SET conflict_count=1", [])
        .unwrap();
    event
}

fn upgraded(old: &EventInput) -> EventInput {
    let mut event = old.clone();
    event.parser_version = "parser-2".into();
    event.observed_at_ms = Some(old.occurred_at_ms + 1000);
    event
}

#[test]
fn version_certification_updates_full_legacy_basis_with_and_without_parser_changes() {
    use llm_usage_core::domain::VersionBasis;
    for legacy_hash in [false, true] {
        for change_parser in [false, true] {
            let (_dir, storage) = temp_storage("basis-certification");
            let mut old = with_tokens(evt("inst", "call", ts("2026-09-24T10:00:00Z")), 100, 20);
            old.parser_version = "parser-1".into();
            old.parse_basis = Some(VersionBasis::LatestFallback);
            old.source_revision = Some(10);
            commit_batch(
                &storage,
                &batch("inst", "UTC", old.occurred_at_ms, vec![old.clone()]),
                None,
            )
            .unwrap();
            storage
                .conn()
                .execute(
                    "UPDATE usage_events SET content_hash=?1",
                    [if legacy_hash {
                        content_hash(&old)
                    } else {
                        event_content_hash(&old)
                    }],
                )
                .unwrap();
            let mut new = old.clone();
            new.parse_basis = Some(VersionBasis::KnownVersion);
            if change_parser {
                new.parser_version = "parser-2".into();
            }
            let before = summary(&storage, DAY, DAY).totals;
            let result = commit_batch(
                &storage,
                &batch("inst", "UTC", old.occurred_at_ms + 1, vec![new.clone()]),
                None,
            )
            .unwrap();
            assert_eq!((result.updated, result.conflicts), (1, 0));
            assert_eq!(summary(&storage, DAY, DAY).totals, before);
            assert_eq!(
                storage
                    .conn()
                    .query_row("SELECT parse_basis FROM usage_events", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "known_version"
            );
            assert_eq!(
                commit_batch(
                    &storage,
                    &batch("inst", "UTC", old.occurred_at_ms + 2, vec![new.clone()]),
                    None
                )
                .unwrap()
                .updated,
                0
            );
            new.usage.input_total = Some(101);
            assert_eq!(
                commit_batch(
                    &storage,
                    &batch("inst", "UTC", old.occurred_at_ms + 3, vec![new]),
                    None
                )
                .unwrap()
                .conflicts,
                1
            );
        }
    }
}

fn flags(storage: &Storage) -> (i64, i64, i64) {
    (
        storage
            .conn()
            .query_row("SELECT conflict FROM usage_events", [], |r| r.get(0))
            .unwrap(),
        storage
            .conn()
            .query_row("SELECT SUM(conflict_count) FROM daily_usage", [], |r| {
                r.get(0)
            })
            .unwrap(),
        storage
            .conn()
            .query_row("SELECT SUM(conflict_count) FROM hourly_usage", [], |r| {
                r.get(0)
            })
            .unwrap(),
    )
}

#[test]
fn pure_parser_change_repairs_legacy_conflict_without_changing_usage_or_identity() {
    for revision in [None, Some(10)] {
        for legacy_hash in [false, true] {
            let (_dir, storage) = temp_storage("parser-metadata");
            let old = seed(&storage, revision, legacy_hash);
            let before = summary(&storage, DAY, DAY).totals;
            let revision_before = storage.data_revision().unwrap();
            let event = upgraded(&old);
            let result = commit_batch(
                &storage,
                &batch("inst", "UTC", old.occurred_at_ms + 2, vec![event.clone()]),
                None,
            )
            .unwrap();
            assert_eq!((result.added, result.updated, result.conflicts), (0, 1, 0));
            assert_eq!(flags(&storage), (0, 0, 0));
            let after = summary(&storage, DAY, DAY).totals;
            assert_eq!(
                (
                    after.call_count,
                    after.event_count,
                    after.input_total_known,
                    after.output_total_known,
                    after.total_tokens_known
                ),
                (
                    before.call_count,
                    before.event_count,
                    before.input_total_known,
                    before.output_total_known,
                    before.total_tokens_known
                )
            );
            assert!(storage.data_revision().unwrap() > revision_before);
            let (parser,hash,created,observed):(String,String,i64,Option<i64>)=storage.conn().query_row(
                "SELECT parser_version,content_hash,created_at_ms,observed_at_ms FROM usage_events", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
            assert_eq!(parser, "parser-2");
            assert_eq!(hash, event_content_hash(&event));
            assert_eq!(created, old.occurred_at_ms);
            assert_eq!(observed, old.observed_at_ms);
            let history: i64 = storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM diagnostics WHERE code='existing_audit'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(history, 1);
            let repeat = commit_batch(
                &storage,
                &batch("inst", "UTC", old.occurred_at_ms + 3, vec![event]),
                None,
            )
            .unwrap();
            assert_eq!(
                (repeat.updated, repeat.unchanged, repeat.conflicts),
                (0, 1, 0)
            );
            assert_eq!(repeat.data_revision, result.data_revision);
        }
    }
}

#[test]
fn metadata_upgrade_keeps_the_original_day_for_repeated_final_timestamps() {
    let (_dir, storage) = temp_storage("parser-repeat-time");
    let old = seed(&storage, None, false);
    let mut event = upgraded(&old);
    event.occurred_at_ms = ts("2026-09-25T10:00:00Z");
    event.source_time = Some("2026-09-25T10:00:00Z".into());
    let result = commit_batch(
        &storage,
        &batch("inst", "UTC", event.occurred_at_ms, vec![event]),
        None,
    )
    .unwrap();
    assert_eq!((result.updated, result.conflicts), (1, 0));
    let stored_time: i64 = storage
        .conn()
        .query_row("SELECT occurred_at_ms FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stored_time, old.occurred_at_ms);
    assert_eq!(
        summary(&storage, DAY, DAY).totals.total_tokens_known,
        Some(120)
    );
    assert_eq!(
        summary(&storage, "2026-09-25", "2026-09-25")
            .totals
            .event_count,
        0
    );
}

#[test]
fn parser_change_does_not_authorize_other_content_changes() {
    for changed_field in [
        "tokens", "quality", "model", "provider", "schema", "category", "duration", "revision",
    ] {
        let (_dir, storage) = temp_storage("parser-real-conflict");
        let old = seed(&storage, None, false);
        let mut event = upgraded(&old);
        match changed_field {
            "tokens" => {
                event.usage.input_total = Some(101);
                event.usage.total_tokens = Some(121);
            }
            "quality" => event.quality.input_total = FieldQuality::Derived,
            "model" => event.model_raw = Some("other-model".into()),
            "provider" => event.provider_id = Some("other-provider".into()),
            "schema" => event.schema_version = "other-schema".into(),
            "category" => event.call_category = llm_usage_core::domain::CallCategory::SubAgent,
            "duration" => event.duration_ms = Some(100),
            "revision" => event.source_revision = Some(1),
            _ => unreachable!(),
        }
        let result = commit_batch(
            &storage,
            &batch("inst", "UTC", old.occurred_at_ms + 1, vec![event]),
            None,
        )
        .unwrap();
        assert_eq!(
            (result.updated, result.conflicts),
            (0, 1),
            "{changed_field}"
        );
        assert_eq!(flags(&storage), (1, 1, 1), "{changed_field}");
        assert_eq!(
            summary(&storage, DAY, DAY).totals.total_tokens_known,
            Some(120)
        );
    }
}

#[test]
fn metadata_upgrade_does_not_clear_a_real_conflict_in_either_batch_order() {
    for conflict_first in [false, true] {
        let (_dir, storage) = temp_storage("parser-mixed-conflict");
        let old = seed(&storage, Some(10), false);
        let correct = upgraded(&old);
        let mut wrong = correct.clone();
        wrong.usage.input_total = Some(101);
        wrong.usage.total_tokens = Some(121);
        let events = if conflict_first {
            vec![wrong, correct]
        } else {
            vec![correct, wrong]
        };
        let result = commit_batch(
            &storage,
            &batch("inst", "UTC", old.occurred_at_ms + 1, events),
            None,
        )
        .unwrap();
        assert_eq!((result.updated, result.conflicts), (1, 1));
        assert_eq!(flags(&storage), (1, 1, 1));
        let retry = commit_batch(
            &storage,
            &batch("inst", "UTC", old.occurred_at_ms + 2, vec![upgraded(&old)]),
            None,
        )
        .unwrap();
        assert_eq!((retry.updated, retry.conflicts), (0, 0));
        assert_eq!(
            flags(&storage),
            (1, 1, 1),
            "a matching retry cannot clear a real conflict"
        );
    }
}

#[test]
fn source_revision_and_lifecycle_order_still_take_precedence() {
    for lifecycle_order in [false, true] {
        let (_dir, storage) = temp_storage("parser-order");
        let old = seed(
            &storage,
            if lifecycle_order { None } else { Some(10) },
            false,
        );
        let mut event = upgraded(&old);
        if lifecycle_order {
            event.lifecycle = Lifecycle::Partial;
        } else {
            event.source_revision = Some(9);
        }
        let result = commit_batch(
            &storage,
            &batch("inst", "UTC", old.occurred_at_ms + 1, vec![event]),
            None,
        )
        .unwrap();
        assert_eq!(
            (result.updated, result.unchanged, result.conflicts),
            (0, 1, 0)
        );
        assert_eq!(flags(&storage), (1, 1, 1));
        let parser: String = storage
            .conn()
            .query_row("SELECT parser_version FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(parser, "parser-1");
    }
}

#[test]
fn metadata_repair_and_aggregates_roll_back_together() {
    let (_dir, storage) = temp_storage("parser-rollback");
    let old = seed(&storage, None, false);
    let revision = storage.data_revision().unwrap();
    let b = batch("inst", "UTC", old.occurred_at_ms + 1, vec![upgraded(&old)]);
    assert!(commit_batch(&storage, &b, Some(FaultPoint::BeforeCommit)).is_err());
    assert_eq!(flags(&storage), (1, 1, 1));
    assert_eq!(storage.data_revision().unwrap(), revision);
    let parser: String = storage
        .conn()
        .query_row("SELECT parser_version FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(parser, "parser-1");
    let audit: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code='parser_metadata_updated'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audit, 0);
    commit_batch(&storage, &b, None).unwrap();
    assert_eq!(flags(&storage), (0, 0, 0));
}

fn seed_consumed_old_parser(
    storage: &Storage,
    adapter: &dyn llm_usage_core::adapters::framework::SourceAdapter,
    parser: &str,
    source_parser: &str,
    now: i64,
) -> i64 {
    use llm_usage_core::adapters::framework::{ScanLimits, ScanTarget, StoredScanState};
    let files = storage
        .conn()
        .prepare("SELECT instance_id,file_id,file_identity,generation FROM source_files")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut count = 0;
    for (instance, file, identity, generation) in files {
        let path = std::path::PathBuf::from(&file);
        let target = ScanTarget {
            instance_id: instance.clone(),
            probe: llm_usage_core::adapters::jsonl::probe_file(&path).unwrap(),
            path,
            file_id: file,
            file_identity: identity,
            generation,
            rescan: true,
        };
        for mut event in adapter
            .scan(
                &target,
                &StoredScanState::default(),
                &ScanLimits::default(),
                now,
            )
            .unwrap()
            .events
        {
            event.parser_version = parser.into();
            count+=storage.conn().execute("UPDATE usage_events SET parser_version=?1,content_hash=?2,conflict=1 WHERE source_instance_id=?3 AND source_record_key=?4",
                rusqlite::params![parser,event_content_hash(&event),instance,event.source_record_key]).unwrap() as i64;
        }
    }
    storage
        .conn()
        .execute(
            "UPDATE source_instances SET parser_version=?1",
            [source_parser],
        )
        .unwrap();
    storage
        .conn()
        .execute("UPDATE daily_usage SET conflict_count=event_count", [])
        .unwrap();
    storage
        .conn()
        .execute("UPDATE hourly_usage SET conflict_count=event_count", [])
        .unwrap();
    count
}

#[test]
fn consumed_codex_bytes_repair_old_parser_conflicts_and_then_skip_replay() {
    let now = ts("2026-10-03T00:00:00Z");
    for parser in ["codex-rollout-1", "codex-rollout-3"] {
        let dir = TempDir::new("codex-parser-upgrade");
        let data=concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"synthetic\",\"cli_version\":\"0.154.0-alpha.6.2\",\"model_provider\":\"openai\"}}\n",
            "{\"type\":\"turn_context\",\"payload\":{\"model\":\"synthetic-model\"}}\n",
            "{\"type\":\"token_usage_record\",\"timestamp\":\"2026-09-24T10:00:00Z\",\"payload\":{\"response_id\":\"synthetic-call\",\"usage\":{\"input_tokens\":100,\"cached_input_tokens\":50,\"cache_write_input_tokens\":0,\"output_tokens\":20,\"reasoning_output_tokens\":5,\"total_tokens\":120}}}\n");
        let root = codex_root_with_file(&dir, "rollout-synthetic.jsonl", data.as_bytes());
        let (_db, storage) = temp_storage("codex-parser-upgrade");
        run_codex(&storage, &root, now);
        let count = seed_consumed_old_parser(
            &storage,
            &llm_usage_core::adapters::codex::CodexAdapter::new(),
            parser,
            "codex-rollout-3",
            now,
        );
        assert_eq!(count, 1);
        assert_eq!(flags(&storage), (1, 1, 1));
        let history: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingest_runs", [], |r| r.get(0))
            .unwrap();
        let result = run_codex(&storage, &root, now + 1);
        assert!(result[0].files[0].lines_read > 0);
        let outcome = result[0].outcome.as_ref().unwrap();
        assert_eq!(
            (outcome.added, outcome.updated, outcome.conflicts),
            (0, 1, 0)
        );
        assert_eq!(flags(&storage), (0, 0, 0));
        assert_eq!(
            summary(&storage, DAY, DAY).totals.total_tokens_known,
            Some(120)
        );
        let after: i64 = storage
            .conn()
            .query_row("SELECT COUNT(*) FROM ingest_runs", [], |r| r.get(0))
            .unwrap();
        assert!(after > history);
        let revision = storage.data_revision().unwrap();
        let repeated = run_codex(&storage, &root, now + 2);
        assert_eq!(repeated[0].files[0].status, "unchanged");
        assert_eq!(storage.data_revision().unwrap(), revision);
        let file = root.join("sessions/2026/09/24/rollout-synthetic.jsonl");
        assert_eq!(std::fs::read(file).unwrap(), data.as_bytes());
    }
}

#[test]
fn consumed_kilo_watermark_replays_all_old_messages_without_double_counting() {
    check_consumed_kilo_watermark("kilo-message-tokens-1");
}

#[test]
fn kilo_health_policy_upgrade_preserves_parser_three_events_without_conflicts() {
    check_consumed_kilo_watermark("kilo-message-tokens-3");
}

fn check_consumed_kilo_watermark(parser: &str) {
    let dir = TempDir::new("kilo-parser-upgrade");
    let root = build_kilo_db_from_fixture(&dir, "session-7.8.1-k3.sanitized.json");
    let source = root.join(".local/share/kilo/kilo.db");
    let bytes = std::fs::read(&source).unwrap();
    let (_db, storage) = temp_storage("kilo-parser-upgrade");
    let now = ts("2026-10-03T00:00:00Z");
    run_kilo(&storage, &root, now);
    let before = summary(&storage, "2026-01-01", "2027-12-31").totals;
    let count = seed_consumed_old_parser(
        &storage,
        &llm_usage_core::adapters::kilo::KiloAdapter::new(),
        parser,
        parser,
        now,
    );
    assert_eq!(count, 34);
    let result = run_kilo(&storage, &root, now + 1);
    let outcome = result[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.conflicts),
        (0, 34, 0)
    );
    let conflicts: i64 = storage
        .conn()
        .query_row("SELECT SUM(conflict) FROM usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(conflicts, 0);
    let after = summary(&storage, "2026-01-01", "2027-12-31").totals;
    assert_eq!(
        (
            after.call_count,
            after.total_tokens_known,
            after.conflict_count
        ),
        (before.call_count, before.total_tokens_known, 0)
    );
    assert_eq!(
        (after.call_count, after.total_tokens_known),
        (34, Some(3_189_308))
    );
    let revision = storage.data_revision().unwrap();
    let repeated = run_kilo(&storage, &root, now + 2);
    let outcome = repeated[0].outcome.as_ref().unwrap();
    assert_eq!(
        (outcome.added, outcome.updated, outcome.conflicts),
        (0, 0, 0)
    );
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(std::fs::read(source).unwrap(), bytes);
}
