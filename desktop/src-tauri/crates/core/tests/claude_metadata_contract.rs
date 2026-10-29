mod common;
use common::*;
use llm_usage_core::{
    adapters::{built_in_adapters, claude::ClaudeAdapter, framework::*},
    jobs::TriggerKind,
    storage::Storage,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const NOW: i64 = 1_800_000_000_000;
const SAMPLES: [&str; 4] = [
    include_str!("fixtures/claude/native-2.1.312/glm-5.3-flash.jsonl"),
    include_str!("fixtures/claude/native-2.1.312/glm-5.3.jsonl"),
    include_str!("fixtures/claude/native-2.1.312/kimi-for-coding.jsonl"),
    include_str!("fixtures/claude/native-2.1.312/glm-5.3-flash-tool.jsonl"),
];

fn scalar(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn config(now: i64) -> RunConfig {
    RunConfig {
        timezone: "Asia/Shanghai".into(),
        now_ms: now,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("claude-metadata-{now}"),
    }
}

fn parallel(storage: &Mutex<Storage>, root: &std::path::Path, now: i64) {
    let adapters = built_in_adapters();
    let requests = adapters
        .iter()
        .map(|a| ParallelScanRequest {
            adapter: a.as_ref(),
            context: DiscoverContext {
                home_dir: None,
                env: [(
                    "CLAUDE_CONFIG_DIR".into(),
                    root.to_string_lossy().into_owned(),
                )]
                .into(),
                ..Default::default()
            },
            config: RunConfig {
                run_id_prefix: format!("metadata-{now}-{}", a.adapter_id()),
                ..config(now)
            },
            filter: InstanceFilter::default(),
        })
        .collect::<Vec<_>>();
    for result in run_adapter_scans_parallel(storage, &requests, None, Arc::new(|| true), None) {
        for report in result.unwrap() {
            assert!(
                report
                    .files
                    .iter()
                    .all(|f| f.status == "complete" || f.status == "unchanged"),
                "{report:?}"
            );
        }
    }
}

#[test]
fn native_three_models_and_tool_loop_import_once_through_parallel_registry() {
    let dir = TempDir::new("claude-metadata");
    let root = dir.path().to_path_buf();
    for (i, sample) in SAMPLES.iter().enumerate() {
        claude_root_with_file(&dir, &format!("sample/{i}.jsonl"), sample.as_bytes());
    }
    let (_db, storage) = temp_storage("claude-metadata");
    let storage = Mutex::new(storage);
    parallel(&storage, &root, NOW);
    let revision;
    {
        let s = storage.lock().unwrap();
        assert_eq!(scalar(&s, "SELECT COUNT(*) FROM usage_events"), 5);
        assert_eq!(
            scalar(&s, "SELECT COUNT(*) FROM usage_events WHERE schema_version='2.1.312' AND parse_basis='known_version' AND parser_version='claude-transcript-doc2'"),
            5
        );
        assert_eq!(
            scalar(
                &s,
                "SELECT COUNT(*) FROM diagnostics WHERE code='latest_fallback'"
            ),
            0
        );
        assert_eq!(
            scalar(&s, "SELECT SUM(input_uncached) FROM usage_events"),
            7519
        );
        assert_eq!(
            scalar(&s, "SELECT SUM(output_total) FROM usage_events"),
            349
        );
        assert_eq!(
            scalar(&s, "SELECT SUM(input_cache_read) FROM usage_events"),
            2944
        );
        assert_eq!(scalar(&s, "SELECT COUNT(*) FROM usage_events WHERE input_cache_write IS NOT NULL OR input_total IS NOT NULL OR total_tokens IS NOT NULL OR provider_id IS NOT NULL OR cost_amount_minor IS NOT NULL OR conflict<>0"), 0);
        assert_eq!(
            scalar(
                &s,
                "SELECT COUNT(*) FROM source_files WHERE status='active'"
            ),
            4
        );
        assert_eq!(
            scalar(&s, "SELECT COUNT(DISTINCT instance_id) FROM source_files"),
            1
        );
        revision = s.data_revision().unwrap();
    }
    parallel(&storage, &root, NOW + 1);
    assert_eq!(storage.lock().unwrap().data_revision().unwrap(), revision);
}

#[test]
fn metadata_first_line_validation_and_unexpected_usage_remain_fail_closed() {
    let rows: Vec<Value> = SAMPLES[3]
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let mut metadata: Vec<Value> = rows
        .into_iter()
        .filter(|r| {
            matches!(
                r["type"].as_str(),
                Some(
                    "atis-latch"
                        | "tools-used"
                        | "api-request-shape"
                        | "api-request-blob"
                        | "api-request"
                        | "cost-state"
                )
            )
        })
        .collect();
    metadata.extend([
        json!({"type":"file-history-snapshot","messageId":"m","snapshot":{},"isSnapshotUpdate":false}),
        json!({"type":"progress","toolUseID":"t","data":{"type":"tool_heartbeat"}}),
    ]);
    for record in metadata {
        let kind = record["type"].as_str().unwrap();
        let missing_field = match kind {
            "atis-latch" | "tools-used" | "api-request-shape" | "api-request-blob"
            | "api-request" | "cost-state" => "sessionId",
            "file-history-snapshot" => "messageId",
            "progress" => "data",
            _ => unreachable!(),
        };
        let dir = TempDir::new("claude-metadata-first");
        let path = dir.path().join("sample.jsonl");
        std::fs::write(&path, format!("{record}\n")).unwrap();
        assert!(matches!(
            ClaudeAdapter::new().detect(&path).unwrap(),
            DetectOutcome::Supported { .. }
        ));
        for (corrupt, expected_field) in [
            (json!({"type":record["type"]}), missing_field),
            (
                {
                    let mut r = record.clone();
                    r["usage"] = json!({"input_tokens":1});
                    r
                },
                "usage",
            ),
            (
                {
                    let mut r = record.clone();
                    r["message"] = json!({"usage":{"input_tokens":1}});
                    r
                },
                "message.usage",
            ),
        ] {
            std::fs::write(&path, format!("{corrupt}\n")).unwrap();
            assert_eq!(
                ClaudeAdapter::new().detect(&path).unwrap(),
                DetectOutcome::UnknownFormat {
                    reason: format!(
                        "first record type {:?} has invalid metadata field {expected_field}",
                        Some(kind)
                    )
                }
            );
            let source = TempDir::new("claude-metadata-invalid");
            let root = claude_root_with_file(
                &source,
                "sample/session.jsonl",
                format!("{}\n{corrupt}\n", json!({"type":"user"})).as_bytes(),
            );
            let (_db, s) = temp_storage("claude-metadata-invalid");
            run_claude(&s, &root, NOW);
            assert_eq!(scalar(&s, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
            let diagnostic: (String, String) = s
                .conn()
                .query_row(
                    "SELECT field,position FROM diagnostics WHERE code='metadata_shape_deviation'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                diagnostic,
                (expected_field.to_string(), "line 2".to_string())
            );
        }
    }
}

#[test]
fn metadata_wrong_types_and_negative_values_identify_the_deviating_field() {
    let rows: Vec<Value> = SAMPLES[3]
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    for (kind, field, value) in [
        ("tools-used", "tools", json!([1])),
        ("cost-state", "totalCostUSD", json!(-1)),
    ] {
        let mut record = rows.iter().find(|row| row["type"] == kind).unwrap().clone();
        record[field] = value;
        let dir = TempDir::new("claude-metadata-field");
        let path = dir.path().join("sample.jsonl");
        std::fs::write(&path, format!("{record}\n")).unwrap();
        assert_eq!(
            ClaudeAdapter::new().detect(&path).unwrap(),
            DetectOutcome::UnknownFormat {
                reason: format!(
                    "first record type {:?} has invalid metadata field {field}",
                    Some(kind)
                )
            }
        );
        let root = claude_root_with_file(
            &dir,
            "sample/session.jsonl",
            format!("{}\n{record}\n", json!({"type":"user"})).as_bytes(),
        );
        let (_db, s) = temp_storage("claude-metadata-field");
        run_claude(&s, &root, NOW);
        let diagnostic: (String, String) = s
            .conn()
            .query_row(
                "SELECT field,position FROM diagnostics WHERE code='metadata_shape_deviation'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(diagnostic, (field.to_string(), "line 2".to_string()));
        assert_eq!(scalar(&s, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
    }
}

#[test]
fn consumed_unchanged_metadata_policy_replay_preserves_events_history_and_rollback() {
    let dir = TempDir::new("claude-metadata-upgrade");
    let root = claude_root_with_file(&dir, "sample/session.jsonl", SAMPLES[0].as_bytes());
    let (_db, s) = temp_storage("claude-metadata-upgrade");
    run_claude(&s, &root, NOW);
    let revision = s.data_revision().unwrap();
    let observed = scalar(&s, "SELECT observed_at_ms FROM usage_events");
    s.conn().execute_batch("UPDATE ingestion_checkpoints SET parse_context=json_remove(parse_context,'$.metadata_rules_version'); UPDATE source_files SET status='degraded'; INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','old diagnostic',0 FROM source_instances; CREATE TRIGGER reject_metadata_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'checkpoint failure'); END;").unwrap();
    run_claude(&s, &root, NOW + 1);
    assert_eq!(
        scalar(
            &s,
            "SELECT COUNT(*) FROM source_files WHERE status='degraded'"
        ),
        1
    );
    assert_eq!(scalar(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.metadata_rules_version') IS NULL"),1);
    s.conn()
        .execute_batch("DROP TRIGGER reject_metadata_checkpoint")
        .unwrap();
    run_claude(&s, &root, NOW + 2);
    assert_eq!(
        scalar(
            &s,
            "SELECT COUNT(*) FROM source_files WHERE status='active'"
        ),
        1
    );
    assert_eq!(scalar(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.metadata_rules_version')=1"),1);
    assert_eq!(
        scalar(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='preserved_history'"
        ),
        1
    );
    assert_eq!(
        scalar(&s, "SELECT observed_at_ms FROM usage_events"),
        observed
    );
    assert_eq!(
        scalar(&s, "SELECT COUNT(*) FROM usage_events WHERE conflict<>0"),
        0
    );
    assert_eq!(s.data_revision().unwrap(), revision);
    run_claude(&s, &root, NOW + 3);
    assert_eq!(s.data_revision().unwrap(), revision);
}

#[test]
fn bounded_metadata_replay_finishes_once_and_preserves_real_conflicts() {
    let dir = TempDir::new("claude-metadata-bounded");
    let root = claude_root_with_file(&dir, "sample/session.jsonl", SAMPLES[0].as_bytes());
    let (_db, s) = temp_storage("claude-metadata-bounded");
    run_claude(&s, &root, NOW);
    let changed = SAMPLES[0].replace("\"output_tokens\":55", "\"output_tokens\":56");
    std::fs::write(root.join("projects/sample/session.jsonl"), changed).unwrap();
    run_claude(&s, &root, NOW + 1);
    assert_eq!(
        scalar(&s, "SELECT COUNT(*) FROM usage_events WHERE conflict=1"),
        1
    );
    std::fs::write(root.join("projects/sample/session.jsonl"), SAMPLES[0]).unwrap();
    run_claude(&s, &root, NOW + 2);
    s.conn().execute_batch("UPDATE ingestion_checkpoints SET parse_context=json_remove(parse_context,'$.metadata_rules_version')").unwrap();
    let mut limits = ScanLimits::default();
    limits.jsonl.max_lines = Some(4);
    run_claude_with_limits(&s, &root, NOW + 3, limits.clone());
    assert_eq!(scalar(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.metadata_rules_version') IS NULL"),1);
    for round in 4..12 {
        run_claude_with_limits(&s, &root, NOW + round, limits.clone());
    }
    assert_eq!(scalar(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.metadata_rules_version')=1"),1);
    assert_eq!(
        scalar(
            &s,
            "SELECT COUNT(*) FROM usage_events WHERE conflict=1 AND output_total=55"
        ),
        1
    );
    assert!(
        scalar(
            &s,
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'"
        ) > 0
    );
}

#[test]
fn incomplete_metadata_tail_does_not_finish_policy_update() {
    let dir = TempDir::new("claude-metadata-tail");
    let root = claude_root_with_file(
        &dir,
        "sample/session.jsonl",
        SAMPLES[0].trim_end().as_bytes(),
    );
    let (_db, s) = temp_storage("claude-metadata-tail");
    run_claude(&s, &root, NOW);
    assert_eq!(scalar(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.metadata_rules_version') IS NULL AND json_extract(parse_context,'$.native_rules_in_progress')=1"),1);
    assert_eq!(scalar(&s, "SELECT COUNT(*) FROM usage_events"), 1);
    std::fs::write(root.join("projects/sample/session.jsonl"), SAMPLES[0]).unwrap();
    run_claude(&s, &root, NOW + 1);
    assert_eq!(scalar(&s,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.metadata_rules_version')=1 AND json_extract(parse_context,'$.native_rules_in_progress')=0"),1);
    assert_eq!(scalar(&s, "SELECT COUNT(*) FROM usage_events"), 1);
}
