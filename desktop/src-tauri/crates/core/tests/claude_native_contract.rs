//! Native 2.1.197 real calls, repeated content blocks and consumed-cursor repair.
mod common;
use common::{claude_root_with_file, run_claude, run_claude_with_limits, temp_storage, TempDir};
use llm_usage_core::adapters::{
    claude::{map_claude_transcript, ClaudeAdapter, ClaudeTranscriptUsage},
    framework::*,
};
use llm_usage_core::{
    domain::{EventInput, VersionBasis},
    error::CoreError,
    jobs::TriggerKind,
    storage::Storage,
};
use std::path::Path;
use std::sync::Mutex;
const FLASH: &str = include_str!("fixtures/claude/real-2.1.197-zhipu/glm-5.3-flash.jsonl");
const FULL: &str = include_str!("fixtures/claude/real-2.1.197-zhipu/glm-5.3.jsonl");
const NOW: i64 = 1_800_000_000_000;

fn scalar(storage: &Storage, sql: &str) -> i64 {
    storage.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn check(storage: &Storage, calls: i64, input: i64, output: i64) {
    assert_eq!(scalar(storage, "SELECT COUNT(*) FROM usage_events"), calls);
    assert_eq!(
        scalar(storage, "SELECT SUM(input_uncached) FROM usage_events"),
        input
    );
    assert_eq!(
        scalar(storage, "SELECT SUM(output_total) FROM usage_events"),
        output
    );
    assert_eq!(scalar(storage,"SELECT COUNT(*) FROM usage_events WHERE input_total IS NOT NULL OR total_tokens IS NOT NULL OR input_cache_read IS NOT NULL OR input_cache_write IS NOT NULL OR provider_id IS NOT NULL OR cost_amount_minor IS NOT NULL OR conflict<>0"),0);
}
#[test]
fn real_api_cli_native_positive_buckets_and_four_blocks_are_two_calls() {
    let api: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/claude/real-2.1.197-zhipu/api-usage.json"
    ))
    .unwrap();
    let cli: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/claude/real-2.1.197-zhipu/cli-usage.json"
    ))
    .unwrap();
    assert_eq!(api.as_array().unwrap().len(), 2);
    for (i, text) in [FLASH, FULL].into_iter().enumerate() {
        let rows: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|v: &serde_json::Value| v["type"] == "assistant")
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["message"]["id"], rows[1]["message"]["id"]);
        let usage = &api[i]["usage"][1]["usage"];
        assert!(usage.get("cache_creation_input_tokens").is_none());
        assert_eq!(api[i]["status"], 200);
        assert_eq!(cli[i]["report"]["is_error"], false);
        for row in rows {
            for key in ["input_tokens", "output_tokens"] {
                assert_eq!(row["message"]["usage"][key], usage[key]);
                assert_eq!(row["message"]["usage"][key], cli[i]["report"]["usage"][key]);
            }
            assert_eq!(row["message"]["usage"]["cache_creation_input_tokens"], 0);
            assert!(row.get("requestId").is_none());
        }
    }
    let dir = TempDir::new("claude-native-real");
    let root = claude_root_with_file(&dir, "sample/flash.jsonl", FLASH.as_bytes());
    claude_root_with_file(&dir, "sample/full.jsonl", FULL.as_bytes());
    let (_db, storage) = temp_storage("claude-native-real");
    run_claude(&storage, &root, NOW);
    check(&storage, 2, 2677, 56);
    assert_eq!(scalar(&storage,"SELECT COUNT(*) FROM usage_events WHERE schema_version='2.1.197' AND parse_basis='known_version' AND parser_version='claude-transcript-doc2'"),2);
    let revision = storage.data_revision().unwrap();
    run_claude(&storage, &root, NOW + 1);
    assert_eq!(storage.data_revision().unwrap(), revision);
    assert_eq!(
        std::fs::read_to_string(root.join("projects/sample/flash.jsonl")).unwrap(),
        FLASH
    );
}

struct Legacy {
    variant: &'static str,
    events: Mutex<Vec<EventInput>>,
}
impl SourceAdapter for Legacy {
    fn adapter_id(&self) -> &'static str {
        "claude"
    }
    fn agent(&self) -> &'static str {
        "claude-code"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        ClaudeAdapter::new().discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        ClaudeAdapter::new().instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        ClaudeAdapter::new().detect(p)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = ClaudeAdapter::new().capability();
        c.maintenance["parser_version"] = "claude-transcript-doc1".into();
        c.supported_versions = vec!["transcript-doc-1".into()];
        c
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut result = ClaudeAdapter::new().scan(t, s, l, n)?;
        for e in &mut result.events {
            let mapped = map_claude_transcript(&ClaudeTranscriptUsage {
                input_tokens: e.usage.input_uncached.unwrap_or(0),
                output_tokens: e.usage.output_total.unwrap_or(0),
                cache_read_input_tokens: e.usage.input_cache_read.unwrap_or(0),
                cache_creation_input_tokens: e.usage.input_cache_write.unwrap_or(0),
            });
            e.usage = mapped.usage;
            e.quality = mapped.quality;
            e.provider_id = Some("anthropic".into());
            e.schema_version = "transcript-doc-1".into();
            e.parser_version = "claude-transcript-doc1".into();
            if self.variant == "input" {
                e.usage.input_uncached = Some(1340);
                e.usage.input_total = Some(1340);
                e.usage.total_tokens = Some(1373);
            }
            if self.variant == "model" {
                e.model_raw = Some("different".into());
            }
            if self.variant == "quality" {
                e.quality.output_total = llm_usage_core::domain::FieldQuality::Derived;
            }
            if self.variant == "revision" {
                e.source_revision = Some(1);
            }
        }
        result
            .parse_context
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("native_rules_version");
        *self.events.lock().unwrap() = result.events.clone();
        Ok(result)
    }
}
fn config(prefix: &str) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: prefix.into(),
    }
}
#[test]
fn full_old_digest_unchanged_bytes_legacy_hash_and_checkpoint_rollback() {
    for legacy_hash in [false, true] {
        for fail in [false, true] {
            let dir = TempDir::new("claude-native-upgrade");
            let root = claude_root_with_file(&dir, "sample/flash.jsonl", FLASH.as_bytes());
            let ctx = DiscoverContext {
                home_dir: None,
                env: Default::default(),
                manual_roots: vec![root.clone()],
            };
            let (_db, storage) = temp_storage("claude-native-upgrade");
            let old = Legacy {
                variant: "default",
                events: Mutex::new(vec![]),
            };
            run_adapter_scan(&storage, &old, &ctx, &config("old")).unwrap();
            assert_eq!(
                scalar(&storage, "SELECT COUNT(*) FROM ingestion_checkpoints"),
                1
            );
            let observed = scalar(&storage, "SELECT observed_at_ms FROM usage_events");
            if legacy_hash {
                let e = old.events.lock().unwrap()[0].clone();
                storage
                    .conn()
                    .execute(
                        "UPDATE usage_events SET content_hash=?1",
                        [llm_usage_core::identity::content_hash(&e)],
                    )
                    .unwrap();
            }
            storage.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) SELECT instance_id,'preserved_history','old diagnostic',0 FROM source_instances",[]).unwrap();
            if fail {
                storage.conn().execute_batch("CREATE TRIGGER fail_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
                let reports =
                    run_adapter_scan(&storage, &ClaudeAdapter::new(), &ctx, &config("fail"))
                        .unwrap();
                assert_eq!(reports[0].finish, llm_usage_core::jobs::RunStatus::Failed);
                assert_eq!(scalar(&storage,"SELECT COUNT(*) FROM usage_events WHERE parser_version='claude-transcript-doc1' AND input_cache_write=0"),1);
                assert_eq!(
                    scalar(
                        &storage,
                        "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                    ),
                    0
                );
                storage
                    .conn()
                    .execute_batch("DROP TRIGGER fail_checkpoint")
                    .unwrap();
            }
            run_adapter_scan(&storage, &ClaudeAdapter::new(), &ctx, &config("upgrade")).unwrap();
            check(&storage, 1, 1339, 33);
            assert_eq!(
                scalar(&storage, "SELECT observed_at_ms FROM usage_events"),
                observed
            );
            assert_eq!(
                scalar(
                    &storage,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='preserved_history'"
                ),
                1
            );
            assert_eq!(
                scalar(
                    &storage,
                    "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
                ),
                1
            );
            let revision = storage.data_revision().unwrap();
            run_claude(&storage, &root, NOW + 1);
            assert_eq!(storage.data_revision().unwrap(), revision);
            assert_eq!(
                std::fs::read_to_string(root.join("projects/sample/flash.jsonl")).unwrap(),
                FLASH
            );
        }
    }
}
#[test]
fn full_digest_rejects_unrelated_counter_quality_model_and_revision_changes() {
    for variant in ["input", "quality", "model", "revision"] {
        let dir = TempDir::new("claude-native-conflict");
        let root = claude_root_with_file(&dir, "sample/flash.jsonl", FLASH.as_bytes());
        let ctx = DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![root],
        };
        let (_db, storage) = temp_storage("claude-native-conflict");
        run_adapter_scan(
            &storage,
            &Legacy {
                variant,
                events: Mutex::new(vec![]),
            },
            &ctx,
            &config("old"),
        )
        .unwrap();
        run_adapter_scan(&storage, &ClaudeAdapter::new(), &ctx, &config("new")).unwrap();
        assert_eq!(
            scalar(
                &storage,
                "SELECT COUNT(*) FROM diagnostics WHERE code='parser_policy_updated'"
            ),
            0
        );
        assert_eq!(
            scalar(
                &storage,
                "SELECT COUNT(*) FROM usage_events WHERE parser_version='claude-transcript-doc1'"
            ),
            1
        );
    }
}

#[test]
fn same_batch_actual_conflict_survives_later_verified_policy_correction() {
    let dir = TempDir::new("claude-native-batch-conflict");
    let root = claude_root_with_file(&dir, "sample/flash.jsonl", FLASH.as_bytes());
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root],
    };
    let (_db, storage) = temp_storage("claude-native-batch-conflict");
    run_adapter_scan(
        &storage,
        &Legacy {
            variant: "default",
            events: Mutex::new(vec![]),
        },
        &ctx,
        &config("old"),
    )
    .unwrap();
    let adapter = ClaudeAdapter::new();
    let discovered = adapter.discover(&ctx).remove(0);
    let target = ScanTarget {
        instance_id: adapter.instance_id(&discovered),
        path: discovered.files[0].clone(),
        file_id: "native".into(),
        file_identity: "native".into(),
        probe: llm_usage_core::adapters::jsonl::probe_file(&discovered.files[0]).unwrap(),
        generation: 0,
        rescan: true,
    };
    let correct = adapter
        .scan(
            &target,
            &StoredScanState::default(),
            &ScanLimits::default(),
            NOW + 1,
        )
        .unwrap()
        .events
        .remove(0);
    let mut changed = correct.clone();
    changed.usage.input_uncached = Some(1340);
    let batch = llm_usage_core::ingest::IngestBatch {
        batch_id: "actual-conflict".into(),
        instance_id: target.instance_id,
        timezone: "UTC".into(),
        now_ms: NOW + 1,
        events: vec![changed, correct],
        checkpoints: vec![],
        diagnostics: vec![],
        run_id: None,
        retention_cutoff_ms: None,
    };
    let outcome = llm_usage_core::ingest::commit_batch(&storage, &batch, None).unwrap();
    assert_eq!((outcome.conflicts, outcome.updated), (1, 1));
    assert_eq!(scalar(&storage, "SELECT COUNT(*) FROM usage_events WHERE conflict=1 AND input_uncached=1339 AND parser_version='claude-transcript-doc2'"), 1);
    assert_eq!(
        scalar(
            &storage,
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'"
        ),
        1
    );
}
#[test]
fn mixed_record_versions_do_not_use_file_or_installed_version_for_history() {
    let mut rows = Vec::new();
    for (i, version) in [Some("2.1.197"), Some("9.9.9"), None]
        .into_iter()
        .enumerate()
    {
        let mut row: serde_json::Value = FLASH
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .find(|v: &serde_json::Value| v["type"] == "assistant")
            .unwrap();
        if let Some(v) = version {
            row["version"] = v.into();
        } else {
            row.as_object_mut().unwrap().remove("version");
        }
        row["message"]["id"] = format!("mixed-{i}").into();
        rows.push(row.to_string());
    }
    let dir = TempDir::new("claude-native-mixed");
    let root = claude_root_with_file(
        &dir,
        "sample/mixed.jsonl",
        (rows.join("\n") + "\n").as_bytes(),
    );
    let (_db, storage) = temp_storage("claude-native-mixed");
    run_claude(&storage, &root, NOW);
    assert_eq!(scalar(&storage,"SELECT COUNT(*) FROM usage_events WHERE schema_version='2.1.197' AND parse_basis='known_version' AND total_tokens IS NULL"),1);
    assert_eq!(scalar(&storage,"SELECT COUNT(*) FROM usage_events WHERE schema_version='9.9.9' AND parse_basis='latest_fallback' AND total_tokens IS NULL"),1);
    assert_eq!(scalar(&storage,"SELECT COUNT(*) FROM usage_events WHERE schema_version='transcript-doc-1' AND total_tokens=1372"),1);
    assert_eq!(VersionBasis::KnownVersion.as_str(), "known_version");
}
#[test]
fn budgeted_policy_replay_resumes_and_bad_snapshot_never_marks_rule_complete() {
    let dir = TempDir::new("claude-native-budget");
    let root = claude_root_with_file(&dir, "sample/flash.jsonl", FLASH.as_bytes());
    let (_db, storage) = temp_storage("claude-native-budget");
    let mut limits = ScanLimits::default();
    limits.jsonl.max_lines = Some(3);
    for i in 0..4 {
        run_claude_with_limits(&storage, &root, NOW + i, limits.clone());
    }
    check(&storage, 1, 1339, 33);
    let revision = storage.data_revision().unwrap();
    run_claude(&storage, &root, NOW + 5);
    assert_eq!(storage.data_revision().unwrap(), revision);
    let bad_dir = TempDir::new("claude-native-bad");
    let bad_root = claude_root_with_file(
        &bad_dir,
        "sample/bad.jsonl",
        (FLASH.to_owned() + "invalid-json\n").as_bytes(),
    );
    let (_db, bad) = temp_storage("claude-native-bad");
    run_claude(&bad, &bad_root, NOW);
    assert_eq!(scalar(&bad,"SELECT COUNT(*) FROM ingestion_checkpoints WHERE json_extract(parse_context,'$.native_rules_version')=1"),0);
}
