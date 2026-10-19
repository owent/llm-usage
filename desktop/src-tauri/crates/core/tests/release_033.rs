//! Reproduce 0.3.2 inherited-carry conflicts and the OpenCode v2 storage change.
mod common;
use common::*;
use llm_usage_core::adapters::{codex::CodexAdapter, framework::*};
use llm_usage_core::{domain::EventInput, storage::Storage, CoreError};
use serde_json::{json, Value};
use std::path::Path;

fn config(lines: Option<u64>) -> RunConfig {
    let mut limits = ScanLimits::default();
    limits.jsonl.max_lines = lines;
    RunConfig {
        timezone: "UTC".into(),
        now_ms: ts("2026-10-19T00:00:00Z"),
        limits,
        trigger: llm_usage_core::jobs::TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: "release-033".into(),
    }
}

fn registry(storage: &Storage, root: &Path, config: &RunConfig) -> Vec<SourceRunReport> {
    static RUN: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let mut config = config.clone();
    config.now_ms += RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    config.run_id_prefix = format!("release-033-{}", config.now_ms);
    let ctx = DiscoverContext {
        manual_roots: vec![root.into()],
        ..Default::default()
    };
    llm_usage_core::adapters::built_in_adapters()
        .iter()
        .flat_map(|a| run_adapter_scan(storage, a.as_ref(), &ctx, &config).unwrap())
        .collect()
}

fn opencode_db(dir: &TempDir) -> rusqlite::Connection {
    let root = dir.path().join("opencode");
    std::fs::create_dir_all(&root).unwrap();
    let db = rusqlite::Connection::open(root.join("opencode.db")).unwrap();
    db.execute_batch("CREATE TABLE session_v2(id TEXT PRIMARY KEY,parent_id TEXT,version TEXT,fork_session_id TEXT,fork_boundary TEXT);
        CREATE TABLE session_message(id TEXT PRIMARY KEY,session_id TEXT,type TEXT,seq INTEGER,time_created INTEGER,time_updated INTEGER,data TEXT);
        INSERT INTO session_v2(id,version) VALUES ('s','2.0.32');").unwrap();
    db
}

fn assistant(input: i64) -> Value {
    json!({"time":{"created":ts("2026-10-17T23:59:59Z"),"completed":ts("2026-10-18T00:00:01Z")},
        "model":{"id":"model-a","providerID":"provider-a"},"cost":0,
        "tokens":{"input":input,"output":20,"reasoning":5,"cache":{"read":30,"write":2}}})
}

fn message(db: &rusqlite::Connection, id: &str, session: &str, kind: &str, seq: i64, data: Value) {
    db.execute(
        "INSERT INTO session_message VALUES (?1,?2,?3,?4,?5,?5,?6)",
        rusqlite::params![
            id,
            session,
            kind,
            seq,
            ts("2026-10-18T00:01:00Z"),
            data.to_string()
        ],
    )
    .unwrap();
}

#[test]
fn opencode_v2_registry_recovers_old_unknown_file() {
    let dir = TempDir::new("opencode-v2-registry");
    let db = opencode_db(&dir);
    message(&db, "a", "s", "assistant", 1, assistant(100));
    let mut zeros = assistant(0);
    zeros["tokens"]["reasoning"] = json!(0);
    zeros["tokens"]["cache"]["write"] = json!(0);
    message(&db, "b", "s", "assistant", 2, zeros);
    message(
        &db,
        "c",
        "s",
        "user",
        3,
        json!({"time":{"created":ts("2026-10-18T00:00:00Z")}}),
    );
    let (_tmp, storage) = temp_storage("opencode-v2-registry");
    let cfg = config(None);
    let out = registry(&storage, dir.path(), &cfg);
    assert_eq!(out.len(), 1);
    assert!(out[0].error.is_none(), "{:?}", out[0].error);
    let total = summary(&storage, "2026-10-18", "2026-10-18").totals;
    assert_eq!(total.call_count, 2);
    assert_eq!(total.total_tokens_known, Some(157));
    assert_eq!(total.cache_write_known, Some(2));
    assert_eq!(
        summary(&storage, "2026-10-17", "2026-10-17")
            .totals
            .call_count,
        0
    );
    let zero: (Option<i64>,Option<i64>,Option<i64>)=storage.conn().query_row(
        "SELECT input_uncached,output_reasoning,total_tokens FROM usage_events WHERE source_record_key='opencode:v2:b'",[],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(zero, (None, None, None));
    // Reproduce the registered unsupported 0.3.2 source, then use real registry discovery.
    storage.conn().execute_batch("UPDATE source_instances SET parser_version='opencode-step-finish-parts-2',health='degraded';
        UPDATE source_files SET status='unsupported'; DELETE FROM ingestion_checkpoints;").unwrap();
    let out = registry(&storage, dir.path(), &cfg);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].outcome.as_ref().unwrap().added, 0);
    assert_eq!(summary(&storage, "2026-10-18", "2026-10-18").totals, total);
    assert_eq!(
        registry(&storage, dir.path(), &cfg)[0]
            .outcome
            .as_ref()
            .unwrap()
            .added,
        0
    );
}

#[test]
fn opencode_v2_redacted_native_projection_matches_independent_sums() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/opencode/real-2.0.32-windows/projection.json"
    ))
    .unwrap();
    let dir = TempDir::new("opencode-v2-native");
    let db = opencode_db(&dir);
    db.execute("DELETE FROM session_v2", []).unwrap();
    for s in fixture["sessions"].as_array().unwrap() {
        db.execute(
            "INSERT INTO session_v2(id,version) VALUES (?1,?2)",
            rusqlite::params![s["id"].as_str().unwrap(), s["version"].as_str().unwrap()],
        )
        .unwrap();
    }
    for m in fixture["messages"].as_array().unwrap() {
        db.execute(
            "INSERT INTO session_message VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                m["id"].as_str().unwrap(),
                m["session_id"].as_str().unwrap(),
                m["type"].as_str().unwrap(),
                m["seq"].as_i64().unwrap(),
                m["time_created"].as_i64().unwrap(),
                m["time_updated"].as_i64().unwrap(),
                m["data"].to_string()
            ],
        )
        .unwrap();
    }
    let (_tmp, storage) = temp_storage("opencode-v2-native");
    let mut cfg = config(None);
    cfg.timezone = "Asia/Shanghai".into();
    for pass in 0..2 {
        let result = registry(&storage, dir.path(), &cfg);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].outcome.as_ref().unwrap().added,
            if pass == 0 { 7 } else { 0 }
        );
        let sums: (i64, [Option<i64>; 6]) = storage.conn().query_row(
            "SELECT COUNT(*),SUM(input_uncached),SUM(input_cache_read),SUM(input_cache_write),SUM(input_total),SUM(output_total),SUM(total_tokens) FROM usage_events", [],
            |r|Ok((r.get(0)?,[r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?]))).unwrap();
        assert_eq!(
            sums,
            (7, [Some(78811), Some(93632), None, None, Some(7888), None])
        );
        use llm_usage_core::query::{self, ChartDimension, Filters, Granularity, SummaryRequest};
        let day = llm_usage_core::calendar::parse_date("2026-10-19").unwrap();
        let mut request = SummaryRequest {
            timezone: cfg.timezone.clone(),
            week_start: llm_usage_core::calendar::WeekStart::Monday,
            first_day: day,
            last_day: day,
            granularity: Granularity::Day,
            filters: Filters {
                agents: vec!["opencode".into()],
                ..Default::default()
            },
            today: day,
            retention_cutoff: None,
        };
        for granularity in [
            Granularity::Hour,
            Granularity::Day,
            Granularity::Week,
            Granularity::Month,
        ] {
            request.granularity = granularity;
            let summary = query::query_summary(&storage, &request).unwrap();
            assert_eq!(summary.totals.call_count, 7);
            assert_eq!(summary.totals.input_unknown_count, 7);
            assert_eq!(summary.totals.output_unknown_count, 0);
            assert_eq!(summary.totals.output_total_known, Some(7888));
            assert_eq!(summary.totals.total_tokens_known, None);
            assert_eq!(summary.agent_breakdown[0].agent, "opencode");
            assert_eq!(summary.model_breakdown.len(), 3);
            assert_eq!(
                summary.totals.uncached_known.unwrap()
                    + summary.totals.cache_read_known.unwrap()
                    + summary.totals.output_total_known.unwrap(),
                180_331
            );
            for dimension in [
                ChartDimension::Total,
                ChartDimension::ByAgent,
                ChartDimension::ByModel,
                ChartDimension::ByAgentModel,
            ] {
                let rows = query::chart_series(&storage, &request, &dimension).unwrap();
                assert_eq!(rows.iter().map(|r| r.sums.call_count).sum::<i64>(), 7);
                assert_eq!(
                    rows.iter().map(|r| r.sums.total_unknown_count).sum::<i64>(),
                    7
                );
                assert_eq!(
                    rows.iter()
                        .map(|r| r.sums.uncached_known.unwrap_or(0)
                            + r.sums.cache_read_known.unwrap_or(0)
                            + r.sums.output_total_known.unwrap_or(0))
                        .sum::<i64>(),
                    180_331
                );
            }
        }
        let hourly =
            query::hourly_breakdown(&storage, &cfg.timezone, day, &request.filters).unwrap();
        assert_eq!(hourly.iter().map(|r| r.sums.call_count).sum::<i64>(), 7);
        assert_eq!(
            hourly
                .iter()
                .map(|r| r.sums.uncached_known.unwrap_or(0))
                .sum::<i64>(),
            78_811
        );
        assert_eq!(
            hourly
                .iter()
                .map(|r| r.sums.total_unknown_count)
                .sum::<i64>(),
            7
        );
        let issues: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM diagnostics WHERE code!='latest_fallback'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            issues,
            0,
            "{:?}",
            storage
                .conn()
                .prepare("SELECT code,message FROM diagnostics")
                .unwrap()
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        );
    }
}

#[test]
fn opencode_v2_pages_health_updates_and_forks() {
    let dir = TempDir::new("opencode-v2-pages");
    let db = opencode_db(&dir);
    message(&db, "a", "s", "assistant", 1, assistant(100));
    let mut invalid = assistant(10);
    invalid["tokens"]["cache"]["read"] = json!("invalid");
    message(&db, "b", "s", "assistant", 2, invalid);
    db.execute(
        "INSERT INTO session_v2 VALUES ('fork',NULL,'future','s',?1)",
        [json!({"type":"after","messageID":"a"}).to_string()],
    )
    .unwrap();
    message(&db, "c", "fork", "assistant", 1, assistant(100));
    message(&db, "d", "fork", "assistant", 3, assistant(200));
    let (_tmp, storage) = temp_storage("opencode-v2-pages");
    let cfg = config(Some(1));
    for _ in 0..4 {
        registry(&storage, dir.path(), &cfg);
    }
    assert_eq!(
        summary(&storage, "2026-10-18", "2026-10-18")
            .totals
            .call_count,
        3
    );
    let status = || {
        storage
            .conn()
            .query_row("SELECT status FROM source_files", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
    };
    assert_eq!(status(), "degraded");
    db.execute(
        "UPDATE session_message SET data=?1,time_updated=time_updated+1 WHERE id='b'",
        [assistant(10).to_string()],
    )
    .unwrap();
    registry(&storage, dir.path(), &cfg);
    assert_eq!(status(), "degraded");
    for _ in 0..3 {
        registry(&storage, dir.path(), &cfg);
    }
    assert_eq!(status(), "active_compat");
    assert_eq!(
        summary(&storage, "2026-10-18", "2026-10-18")
            .totals
            .total_tokens_known,
        Some(481)
    );
    db.execute(
        "UPDATE session_message SET type='future-usage' WHERE id='a'",
        [],
    )
    .unwrap();
    registry(&storage, dir.path(), &cfg);
    assert_eq!(status(), "degraded");
    db.execute("DELETE FROM session_message WHERE id='a'", [])
        .unwrap();
    // Missing fork boundary must remain visible; never count copied fork records as new calls.
    for _ in 0..8 {
        registry(&storage, dir.path(), &cfg);
    }
    assert_eq!(status(), "degraded");
}

struct OldCodex;
impl SourceAdapter for OldCodex {
    fn adapter_id(&self) -> &'static str {
        "codex"
    }
    fn agent(&self) -> &'static str {
        "codex"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        CodexAdapter.discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        CodexAdapter.instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        CodexAdapter.detect(p)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = CodexAdapter.capability();
        c.maintenance["parser_version"] = json!("codex-rollout-5");
        c
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        let mut out = CodexAdapter.scan(t, s, l, n)?;
        // Freeze the complete old carry output: old parser emitted it in the child
        // context, before any model turn, with the compaction timestamp.
        if let Some(c) = out.parse_context.as_mut() {
            if let Some(carries) = c.get("inherited_carries").and_then(Value::as_array) {
                for carry in carries {
                    out.events.insert(
                        0,
                        serde_json::from_value::<EventInput>(carry["prior"].clone()).unwrap(),
                    );
                }
            }
            c.as_object_mut().unwrap().remove("inherited_carries");
        }
        for e in &mut out.events {
            e.parser_version = "codex-rollout-5".into();
        }
        Ok(out)
    }
}

fn codex_files(dir: &TempDir) -> std::path::PathBuf {
    let tokens = json!({"input_tokens":174049,"cached_input_tokens":169728,"cache_write_input_tokens":0,
        "output_tokens":72,"reasoning_output_tokens":14,"total_tokens":174121});
    let record = json!({"thread_id":"owner","response_id":"same-response","usage":tokens});
    let header = |id: &str| json!({"type":"session_meta","payload":{"id":id,"cli_version":"0.163.0","parent_thread_id":"parent","model_provider":"openai"}});
    let child = format!(
        "{}\n{}\n",
        header("child"),
        json!({"timestamp":"2026-10-18T00:00:10Z","type":"compacted","payload":{"latest_token_usage_record":record}})
    );
    let root = codex_root_with_file(dir, "rollout-z-child.jsonl", child.as_bytes());
    let owner = format!(
        "{}\n{}\n{}\n",
        header("owner"),
        json!({"type":"turn_context","payload":{"model":"codex-auto-review"}}),
        json!({"timestamp":"2026-10-17T23:59:59Z","type":"token_usage_record","payload":record})
    );
    codex_root_with_file(dir, "rollout-a-owner.jsonl", owner.as_bytes());
    root
}

#[test]
fn codex_inherited_carry_corrects_complete_old_summary_without_clearing_history() {
    let dir = TempDir::new("codex-carry-upgrade");
    let root = codex_files(&dir);
    let (_tmp, storage) = temp_storage("codex-carry-upgrade");
    let ctx = DiscoverContext {
        manual_roots: vec![root.clone()],
        ..Default::default()
    };
    let cfg = config(None);
    run_adapter_scan(&storage, &OldCodex, &ctx, &cfg).unwrap();
    let old = summary(&storage, "2026-10-01", "2026-10-31").totals;
    assert_eq!((old.call_count, old.conflict_count), (1, 1));
    let first: i64 = storage
        .conn()
        .query_row("SELECT created_at_ms FROM usage_events", [], |r| r.get(0))
        .unwrap();
    let result = registry(&storage, &root, &cfg);
    assert_eq!(result.len(), 1);
    let new = summary(&storage, "2026-10-01", "2026-10-31").totals;
    assert_eq!((new.call_count, new.conflict_count), (1, 0));
    assert_eq!(old.total_tokens_known, new.total_tokens_known);
    assert_eq!(
        summary(&storage, "2026-10-17", "2026-10-17")
            .totals
            .call_count,
        1
    );
    let row: (String, String, i64) = storage
        .conn()
        .query_row(
            "SELECT session_id,model_raw,created_at_ms FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, ("owner".into(), "codex-auto-review".into(), first));
    let history: i64 = storage
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(history > 0);
    registry(&storage, &root, &cfg);
    assert_eq!(summary(&storage, "2026-10-01", "2026-10-31").totals, new);
}

#[test]
fn opencode_v2_compaction_is_an_observation_and_failed_calls_keep_unknown_usage() {
    let dir = TempDir::new("opencode-v2-lifecycle");
    let db = opencode_db(&dir);
    let mut compaction = assistant(100);
    compaction["status"] = json!("completed");
    compaction["time"]
        .as_object_mut()
        .unwrap()
        .remove("completed");
    compaction["model"] = Value::Null;
    message(&db, "compact", "s", "compaction", 1, compaction);
    let mut failed = assistant(10);
    failed.as_object_mut().unwrap().remove("tokens");
    failed["error"] = json!({"type":"failed"});
    message(&db, "failed", "s", "assistant", 2, failed);
    let mut partial = assistant(10);
    partial["time"].as_object_mut().unwrap().remove("completed");
    message(&db, "partial", "s", "assistant", 3, partial);
    let (_tmp, storage) = temp_storage("opencode-v2-lifecycle");
    registry(&storage, dir.path(), &config(None));
    let sums = summary(&storage, "2026-10-17", "2026-10-18").totals;
    assert_eq!(sums.call_count, 1);
    assert_eq!(sums.total_tokens_known, Some(157));
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE record_kind='usage_observation' AND model_raw IS NULL",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM usage_events WHERE error_status='failed' AND total_tokens IS NULL",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    db.execute(
        "UPDATE session_message SET data=?1,time_updated=time_updated+1 WHERE id='partial'",
        [assistant(10).to_string()],
    )
    .unwrap();
    registry(&storage, dir.path(), &config(None));
    assert_eq!(
        summary(&storage, "2026-10-17", "2026-10-18")
            .totals
            .call_count,
        2
    );
}

#[test]
fn codex_correction_requires_full_digest_and_retains_real_conflicts() {
    for changed_usage in [false, true] {
        let dir = TempDir::new("codex-carry-mismatch");
        let root = codex_files(&dir);
        let (_tmp, storage) = temp_storage("codex-carry-mismatch");
        let ctx = DiscoverContext {
            manual_roots: vec![root.clone()],
            ..Default::default()
        };
        let cfg = config(None);
        run_adapter_scan(&storage, &OldCodex, &ctx, &cfg).unwrap();
        if changed_usage {
            // A real contradictory report in the original thread remains a conflict,
            // even when a verified inherited-carry correction occurs in the same batch.
            let owner = CodexAdapter.discover(&ctx)[0]
                .files
                .iter()
                .find(|p| p.to_string_lossy().contains("a-owner"))
                .unwrap()
                .clone();
            let old = std::fs::read_to_string(&owner).unwrap();
            let mut call: Value = serde_json::from_str(old.lines().last().unwrap()).unwrap();
            call["payload"]["usage"]["output_tokens"] = json!(73);
            call["payload"]["usage"]["total_tokens"] = json!(174122);
            use std::io::Write;
            writeln!(
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(owner)
                    .unwrap(),
                "{call}"
            )
            .unwrap();
        } else {
            storage
                .conn()
                .execute(
                    "UPDATE usage_events SET content_hash='unrelated-full-summary'",
                    [],
                )
                .unwrap();
        }
        registry(&storage, &root, &cfg);
        assert_eq!(
            summary(&storage, "2026-10-01", "2026-10-31")
                .totals
                .conflict_count,
            1
        );
    }
}

#[test]
fn codex_correction_survives_separate_windows_and_transaction_rollback() {
    for mode in ["matching", "contradictory", "outside_policy"] {
        let contradictory = mode != "matching";
        let dir = TempDir::new("codex-carry-windows");
        let root = codex_files(&dir);
        let (_tmp, storage) = temp_storage("codex-carry-windows");
        let ctx = DiscoverContext {
            manual_roots: vec![root],
            ..Default::default()
        };
        let cfg = config(None);
        run_adapter_scan(&storage, &OldCodex, &ctx, &cfg).unwrap();
        let discovered = CodexAdapter.discover(&ctx).remove(0);
        let instance = CodexAdapter.instance_id(&discovered);
        let scan = |part: &str| {
            let path = discovered
                .files
                .iter()
                .find(|p| p.to_string_lossy().contains(part))
                .unwrap();
            let target = ScanTarget {
                instance_id: instance.clone(),
                path: path.clone(),
                file_id: normalize_path(path),
                file_identity: part.into(),
                probe: llm_usage_core::adapters::jsonl::probe_file(path).unwrap(),
                generation: 1,
                rescan: true,
            };
            CodexAdapter
                .scan(
                    &target,
                    &StoredScanState::default(),
                    &ScanLimits::default(),
                    cfg.now_ms + 1,
                )
                .unwrap()
        };
        let mut owner = scan("a-owner");
        if mode == "outside_policy" {
            owner.events[0].error_status = Some("unrelated-change".into());
        }
        if mode == "contradictory" {
            let mut conflicting = owner.events.clone();
            conflicting[0].usage.output_total = Some(73);
            conflicting[0].usage.total_tokens = Some(174122);
            llm_usage_core::ingest::commit_batch(
                &storage,
                &batch(&instance, "UTC", cfg.now_ms + 1, conflicting),
                None,
            )
            .unwrap();
        }
        llm_usage_core::ingest::commit_batch(
            &storage,
            &batch(&instance, "UTC", cfg.now_ms + 1, owner.events),
            None,
        )
        .unwrap();
        let child = scan("z-child");
        assert!(child.events.is_empty());
        let mut repair = batch(&instance, "UTC", cfg.now_ms + 2, vec![]);
        repair
            .checkpoints
            .push(llm_usage_core::ingest::CheckpointUpdate {
                scope_key: "child".into(),
                cursor_value: child.cursor,
                parse_context: child.parse_context,
                source_revision: None,
            });
        let failed = llm_usage_core::ingest::commit_batch(
            &storage,
            &repair,
            Some(llm_usage_core::ingest::FaultPoint::AfterAggregates),
        );
        assert!(failed.is_err());
        assert_eq!(
            summary(&storage, "2026-10-01", "2026-10-31")
                .totals
                .conflict_count,
            1
        );
        llm_usage_core::ingest::commit_batch(&storage, &repair, None).unwrap();
        assert_eq!(
            summary(&storage, "2026-10-01", "2026-10-31")
                .totals
                .conflict_count,
            i64::from(contradictory)
        );
        assert_eq!(storage.conn().query_row("SELECT COUNT(*) FROM ingestion_checkpoints WHERE scope_key LIKE 'codex-carry-candidate:%'",[],|r|r.get::<_,i64>(0)).unwrap(),i64::from(mode == "outside_policy"));
        let parser: String = storage
            .conn()
            .query_row("SELECT parser_version FROM usage_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            parser,
            if mode == "outside_policy" {
                "codex-rollout-5"
            } else {
                "codex-rollout-6"
            }
        );
    }
}

struct OldOpenCode;
impl SourceAdapter for OldOpenCode {
    fn adapter_id(&self) -> &'static str {
        "opencode"
    }
    fn agent(&self) -> &'static str {
        "opencode"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        llm_usage_core::adapters::opencode::OpenCodeAdapter.discover(c)
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        llm_usage_core::adapters::opencode::OpenCodeAdapter.instance_id(r)
    }
    fn detect(&self, p: &Path) -> Result<DetectOutcome, CoreError> {
        llm_usage_core::adapters::opencode::OpenCodeAdapter.detect(p)
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = llm_usage_core::adapters::opencode::OpenCodeAdapter.capability();
        c.maintenance["parser_version"] = json!("opencode-dispatch-3");
        c
    }
    fn scan(
        &self,
        t: &ScanTarget,
        s: &StoredScanState,
        l: &ScanLimits,
        n: i64,
    ) -> Result<ScanOutcome, CoreError> {
        use llm_usage_core::domain::FieldQuality as Q;
        let mut out = llm_usage_core::adapters::opencode::OpenCodeAdapter.scan(t, s, l, n)?;
        for e in &mut out.events {
            e.parser_version = "opencode-session-messages-1".into();
            // Frozen parser-1 results for the two independently specified source rows.
            match e.origin_call_id.as_deref() {
                Some("a") => {
                    e.usage.total_tokens = None;
                    e.quality.total_tokens = Q::Unknown;
                }
                Some("b") => {
                    e.usage.output_total = Some(5);
                    e.quality.output_total = Q::Derived;
                }
                _ => {}
            }
        }
        out.parse_context.as_mut().unwrap()["policy"] = json!("opencode-session-messages-1");
        Ok(out)
    }
}

#[test]
fn opencode_v2_output_recovery_is_independent_of_missing_breakdowns() {
    let dir = TempDir::new("opencode-output-proof");
    let db = opencode_db(&dir);
    let mut no_reasoning = assistant(100);
    no_reasoning["tokens"]["reasoning"] = json!(0);
    message(&db, "a", "s", "assistant", 1, no_reasoning);
    let mut clamped = assistant(100);
    clamped["tokens"]["output"] = json!(0);
    message(&db, "b", "s", "assistant", 2, clamped);
    let mut absent = assistant(0);
    absent["tokens"] = json!({"input":0,"output":0,"reasoning":0,"cache":{"read":0,"write":0}});
    message(&db, "empty", "s", "assistant", 3, absent);
    let (_tmp, storage) = temp_storage("opencode-output-proof");
    registry(&storage, dir.path(), &config(None));
    let fields = |id: &str| -> [Option<i64>; 4] {
        storage.conn().query_row(
        "SELECT input_total,output_total,output_reasoning,total_tokens FROM usage_events WHERE origin_call_id=?1",[id],
        |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?])).unwrap()
    };
    assert_eq!(fields("a"), [Some(132), Some(20), None, Some(152)]);
    assert_eq!(fields("b"), [Some(132), None, Some(5), None]);
    assert_eq!(fields("empty"), [None, None, None, None]);
    assert_eq!(
        summary(&storage, "2026-10-18", "2026-10-18")
            .totals
            .call_count,
        3
    );
}

#[test]
fn opencode_v2_output_policy_replays_old_cursors_with_full_hash_and_rollback() {
    use llm_usage_core::adapters::opencode::OpenCodeAdapter;
    for mode in ["normal", "mismatch", "real_conflict", "same_batch_conflict"] {
        let dir = TempDir::new("opencode-output-upgrade");
        let db = opencode_db(&dir);
        let mut a = assistant(100);
        a["tokens"]["reasoning"] = json!(0);
        message(&db, "a", "s", "assistant", 1, a);
        let mut b = assistant(100);
        b["tokens"]["output"] = json!(0);
        message(&db, "b", "s", "assistant", 2, b);
        message(&db, "c", "s", "assistant", 3, assistant(100));
        let (_tmp, storage) = temp_storage("opencode-output-upgrade");
        let ctx = DiscoverContext {
            manual_roots: vec![dir.path().into()],
            ..Default::default()
        };
        let cfg = config(None);
        run_adapter_scan(&storage, &OldOpenCode, &ctx, &cfg).unwrap();
        let before = summary(&storage, "2026-10-18", "2026-10-18").totals;
        assert_eq!(before.output_total_known, Some(50));
        assert_eq!(before.total_tokens_known, Some(157));
        let first: (i64, i64) = storage
            .conn()
            .query_row(
                "SELECT created_at_ms,observed_at_ms FROM usage_events WHERE origin_call_id='a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let root = OpenCodeAdapter.discover(&ctx).remove(0);
        let instance = OpenCodeAdapter.instance_id(&root);
        let path = &root.files[0];
        let target = ScanTarget {
            instance_id: instance.clone(),
            path: path.clone(),
            file_id: normalize_path(path),
            file_identity: "output-proof".into(),
            probe: llm_usage_core::adapters::jsonl::probe_file(path).unwrap(),
            generation: 1,
            rescan: true,
        };
        let new = OpenCodeAdapter
            .scan(
                &target,
                &StoredScanState::default(),
                &cfg.limits,
                cfg.now_ms + 1,
            )
            .unwrap();
        let mut repair = batch(&instance, "UTC", cfg.now_ms + 1, new.events.clone());
        repair
            .checkpoints
            .push(llm_usage_core::ingest::CheckpointUpdate {
                scope_key: "output-proof".into(),
                cursor_value: new.cursor,
                parse_context: new.parse_context,
                source_revision: None,
            });
        assert!(llm_usage_core::ingest::commit_batch(
            &storage,
            &repair,
            Some(llm_usage_core::ingest::FaultPoint::AfterAggregates)
        )
        .is_err());
        assert_eq!(summary(&storage, "2026-10-18", "2026-10-18").totals, before);
        assert_eq!(
            storage
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM ingestion_checkpoints WHERE scope_key='output-proof'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        if mode == "mismatch" {
            // Same revision with an unrelated complete prior summary must not be corrected.
            storage.conn().execute("UPDATE usage_events SET content_hash='different-complete-summary' WHERE origin_call_id='a'",[]).unwrap();
        }
        if mode == "real_conflict" || mode == "same_batch_conflict" {
            let mut wrong = new
                .events
                .iter()
                .find(|e| e.origin_call_id.as_deref() == Some("c"))
                .unwrap()
                .clone();
            wrong.model_raw = Some("different-model".into());
            if mode == "real_conflict" {
                llm_usage_core::ingest::commit_batch(
                    &storage,
                    &batch(&instance, "UTC", cfg.now_ms + 1, vec![wrong]),
                    None,
                )
                .unwrap();
            } else {
                repair.events.insert(0, wrong);
                llm_usage_core::ingest::commit_batch(&storage, &repair, None).unwrap();
            }
        }
        for _ in 0..2 {
            registry(&storage, dir.path(), &cfg);
        }
        let after = summary(&storage, "2026-10-18", "2026-10-18").totals;
        assert_eq!(after.call_count, 3);
        assert_eq!(after.output_total_known, Some(45));
        assert_eq!(
            after.total_tokens_known,
            Some(if mode == "mismatch" { 157 } else { 309 })
        );
        assert_eq!(after.conflict_count, i64::from(mode != "normal"));
        assert_eq!(storage.conn().query_row("SELECT created_at_ms,observed_at_ms FROM usage_events WHERE origin_call_id='a'",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).unwrap(),first);
        if mode == "real_conflict" || mode == "same_batch_conflict" {
            assert!(
                storage
                    .conn()
                    .query_row(
                        "SELECT COUNT(*) FROM diagnostics WHERE code='update_conflict'",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap()
                    > 0
            );
        }
    }
}
