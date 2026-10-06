//! Whitelisted real npm rc.2 v4 sample; mutations below are explicitly synthetic.
mod common;
use common::{summary, temp_storage, TempDir};
use llm_usage_core::{
    adapters::{
        dsh::{versions::session_v4, DshAdapter},
        framework::*,
    },
    domain::FieldQuality as Q,
    jobs::TriggerKind,
    storage::Storage,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
struct LegacyV4Discovery;
impl SourceAdapter for LegacyV4Discovery {
    fn adapter_id(&self) -> &'static str {
        "dsh"
    }
    fn agent(&self) -> &'static str {
        "deepseek-harness"
    }
    fn discover(&self, c: &DiscoverContext) -> Vec<DiscoveredRoot> {
        c.manual_roots
            .iter()
            .map(|root| DiscoveredRoot {
                root: root.clone(),
                basis: RootBasis::Manual,
                files: llm_usage_core::adapters::framework::enumerate_files_bounded(
                    root,
                    1,
                    &|p| p.extension().is_some_and(|x| x == "jsonl"),
                ),
            })
            .filter(|r| !r.files.is_empty())
            .collect()
    }
    fn instance_id(&self, r: &DiscoveredRoot) -> String {
        format!("dsh@{}", normalize_path(&r.root))
    }
    fn detect(&self, _: &Path) -> Result<DetectOutcome, llm_usage_core::error::CoreError> {
        Ok(DetectOutcome::UnknownFormat {
            reason: "historical doc1 reader had no native session header".into(),
        })
    }
    fn scan(
        &self,
        _: &ScanTarget,
        _: &StoredScanState,
        _: &ScanLimits,
        _: i64,
    ) -> Result<ScanOutcome, llm_usage_core::error::CoreError> {
        panic!("unrecognized native format must not scan")
    }
    fn capability(&self) -> CapabilityTable {
        let mut c = DshAdapter::new().capability();
        c.maintenance["parser_version"] = json!("dsh-session-log-doc1");
        c
    }
}
const NATIVE: &str = include_str!("fixtures/dsh/real-0.2.0-rc.2/session.v4.jsonl");
const NOW: i64 = 1_800_000_000_000;
fn rows() -> Vec<Value> {
    NATIVE
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn encode(rows: &[Value]) -> String {
    rows.iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect()
}
fn carrier(dir: &TempDir, bytes: &[u8], compressed: bool) -> (DiscoverContext, PathBuf) {
    let home = dir.path().join("home");
    let name = if compressed {
        "session.v4.jsonl.zstd"
    } else {
        "session.v4.jsonl"
    };
    let path = home
        .join(".dsh/sessions/anonymous-project/anonymous-session")
        .join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let data = if compressed {
        zstd::stream::encode_all(bytes, 3).unwrap()
    } else {
        bytes.to_vec()
    };
    std::fs::write(&path, data).unwrap();
    (
        DiscoverContext {
            home_dir: Some(home),
            ..Default::default()
        },
        path,
    )
}
fn config(name: &str) -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: name.into(),
        origin_host_id: None,
    }
}
fn count(s: &Storage, sql: &str) -> i64 {
    s.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}
fn run(s: &Storage, ctx: &DiscoverContext, name: &str) {
    let reports = run_adapter_scan(s, &DshAdapter::new(), ctx, &config(name)).unwrap();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].error.is_none(), "{:?}", reports[0]);
}
fn direct(path: &Path, stored: &StoredScanState, limits: &ScanLimits) -> ScanOutcome {
    let target = ScanTarget {
        instance_id: "dsh-test".into(),
        path: path.into(),
        file_id: "file".into(),
        file_identity: "identity".into(),
        probe: llm_usage_core::adapters::jsonl::probe_file(path).unwrap(),
        generation: 0,
        rescan: false,
    };
    session_v4::scan(&target, stored, limits, NOW).unwrap()
}
fn native_sums(s: &Storage) {
    let a:Vec<Option<i64>>=s.conn().query_row("SELECT SUM(input_uncached),SUM(input_cache_read),SUM(input_total),SUM(output_total),SUM(total_tokens),SUM(source_total),SUM(input_cache_write),SUM(output_reasoning),SUM(cost_amount_minor) FROM usage_events",[],|r|(0..9).map(|i|r.get(i)).collect()).unwrap();
    assert_eq!(
        a,
        vec![
            Some(5606),
            Some(5568),
            Some(11174),
            Some(105),
            Some(11279),
            None,
            None,
            None,
            None
        ]
    );
    assert_eq!(count(s, "SELECT COUNT(*) FROM usage_events"), 2);
    assert_eq!(count(s,"SELECT COUNT(*) FROM usage_events WHERE model_raw='qwen3.5-0.8b-local' AND provider_id='local-llama' AND schema_version='dsh-session-v4' AND parser_version='dsh-session-v4-1' AND time_basis='observed_at'"),2);
    let daily = summary(s, "2026-10-06", "2026-10-06").totals;
    assert_eq!(
        (
            daily.call_count,
            daily.input_total_known,
            daily.output_total_known,
            daily.total_tokens_known
        ),
        (2, Some(11174), Some(105), Some(11279))
    );
}
#[test]
fn genuine_main_api_pairs_match_plain_and_zstd_full_registry_without_title_or_stream_double_count()
{
    let api: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/dsh/real-0.2.0-rc.2/api-usage.json")).unwrap();
    assert_eq!(api.len(), 3);
    assert_eq!(api[0]["usage"]["total_tokens"], 205);
    let settlements: Vec<_> = rows()
        .into_iter()
        .filter(|r| r["type"] == "assistant/message")
        .collect();
    for (native, raw) in settlements.iter().zip(api.iter().skip(1)) {
        let u = &native["data"]["usage"];
        let a = &raw["usage"];
        assert_eq!(
            u["inputTokens"].as_i64().unwrap() + u["cacheReadTokens"].as_i64().unwrap_or(0),
            a["prompt_tokens"].as_i64().unwrap()
        );
        assert_eq!(u["outputTokens"], a["completion_tokens"]);
        assert_eq!(u["totalTokens"], a["total_tokens"]);
    }
    for compressed in [false, true] {
        let dir = TempDir::new("dsh-v4-real");
        let (ctx, path) = carrier(&dir, NATIVE.as_bytes(), compressed);
        let original = std::fs::read(&path).unwrap();
        let (_db, s) = temp_storage("dsh-real");
        for pass in 0..2 {
            for adapter in llm_usage_core::adapters::built_in_adapters() {
                let reports = run_adapter_scan(
                    &s,
                    adapter.as_ref(),
                    &ctx,
                    &config(&format!("{pass}-{}", adapter.adapter_id())),
                )
                .unwrap();
                if adapter.adapter_id() == "dsh" {
                    assert_eq!(reports.len(), 1);
                } else {
                    assert!(reports.is_empty(), "{}", adapter.adapter_id());
                }
            }
            native_sums(&s);
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
        assert_eq!(count(&s, "SELECT COUNT(*) FROM source_files"), 1);
        assert!(
            count(
                &s,
                "SELECT COUNT(*) FROM diagnostics WHERE code='auxiliary_usage_coverage_unverified'"
            ) >= 1
        );
    }
}
#[test]
fn actual_registry_recovers_old_manual_parent_registration_and_preserves_other_bad_files() {
    for other_bad_file in [false, true] {
        let dir = TempDir::new("dsh-old-root");
        let (_, path) = carrier(&dir, NATIVE.as_bytes(), false);
        let parent = path.parent().unwrap().to_path_buf();
        if other_bad_file {
            std::fs::write(parent.join("manual-bad.jsonl"), "not JSON\n").unwrap();
        }
        let ctx = DiscoverContext {
            manual_roots: vec![parent.clone()],
            ..Default::default()
        };
        let (_db, s) = temp_storage("dsh-old-root");
        run_adapter_scan(&s, &LegacyV4Discovery, &ctx, &config("old")).unwrap();
        let old = format!("dsh@{}", normalize_path(&parent));
        assert_eq!(count(&s, "SELECT COUNT(*) FROM usage_events"), 0);
        s.conn().execute("INSERT INTO diagnostics(instance_id,code,message,created_ms) VALUES(?1,'old_dsh_audit','preserve history',0)",[&old]).unwrap();
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            run_adapter_scan(
                &s,
                adapter.as_ref(),
                &ctx,
                &config(&format!("registry-{}", adapter.adapter_id())),
            )
            .unwrap();
        }
        native_sums(&s);
        assert_eq!(
            count(
                &s,
                "SELECT COUNT(*) FROM diagnostics WHERE code='old_dsh_audit'"
            ),
            1
        );
        let health: String = s
            .conn()
            .query_row(
                "SELECT health FROM source_instances WHERE instance_id=?1",
                [&old],
                |r| r.get(0),
            )
            .unwrap();
        if other_bad_file {
            assert_ne!(health, "not_applicable");
            assert_eq!(
                count(
                    &s,
                    "SELECT COUNT(*) FROM source_files WHERE status='unsupported'"
                ),
                1
            );
        } else {
            assert_eq!(health, "not_applicable");
        }
        assert_eq!(
            count(&s, "SELECT COUNT(*) FROM source_files"),
            if other_bad_file { 2 } else { 1 }
        );
    }
}
#[test]
fn default_env_manual_parent_lift_and_latest_generation_preserve_physical_identity() {
    let dir = TempDir::new("dsh-discovery");
    let (mut ctx, path) = carrier(&dir, NATIVE.as_bytes(), false);
    let adapter = DshAdapter::new();
    let root = adapter.discover(&ctx)[0].clone();
    let id = adapter.instance_id(&root);
    let home = ctx.home_dir.clone().unwrap();
    ctx.env.insert(
        "DSH_HOME".into(),
        home.join(".dsh").to_string_lossy().into(),
    );
    ctx.manual_roots = vec![
        home.clone(),
        home.join(".dsh"),
        root.root.clone(),
        path.parent().unwrap().to_path_buf(),
        path.clone(),
    ];
    let found = adapter.discover(&ctx);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].files, vec![path.clone()]);
    assert_eq!(adapter.instance_id(&found[0]), id);
    let mut tilde = DiscoverContext {
        home_dir: Some(home.clone()),
        ..Default::default()
    };
    tilde.env.insert("DSH_HOME".into(), "~/.dsh".into());
    assert_eq!(adapter.discover(&tilde)[0].files, vec![path.clone()]);
    tilde.env.insert(
        "DSH_HOME".into(),
        home.join("missing").to_string_lossy().into(),
    );
    assert!(adapter.discover(&tilde).is_empty());
    let future = path.with_file_name("session.v5.jsonl");
    std::fs::write(&future, NATIVE).unwrap();
    ctx.manual_roots.clear();
    let files = adapter.discover(&ctx)[0].files.clone();
    assert_eq!(files, vec![future.clone()]);
    assert!(matches!(
        adapter.detect(&future).unwrap(),
        DetectOutcome::UnsupportedVersion { .. }
    ));
    let (_db, s) = temp_storage("dsh-future");
    run(&s, &ctx, "future");
    assert_eq!(count(&s, "SELECT COUNT(*) FROM usage_events"), 0);
}
#[test]
fn whole_snapshot_required_shapes_limits_partial_zstd_and_unknown_types_keep_checkpoint() {
    for variant in [
        "half",
        "blank",
        "seq",
        "header",
        "required",
        "data",
        "ignorable-false",
        "time",
        "model-identity",
        "wrong-role",
        "wrong-source",
        "bad-source-ranges",
        "bad-surface",
    ] {
        let mut r = rows();
        let index = r
            .iter()
            .position(|v| v["type"] == "assistant/message")
            .unwrap();
        match variant {
            "seq" => r[index]["seq"] = json!(999),
            "header" => r[0]["delegationDepth"] = json!("0"),
            "required" => r[index]["type"] = json!("future-required"),
            "data" => r[index]["data"] = json!([]),
            "ignorable-false" => r[index]["ignorable"] = json!(false),
            "time" => r[index]["time"] = json!(1),
            "model-identity" => r[index]["data"]["step"] = json!(99),
            _ => {}
        }
        match variant {
            "wrong-role" => r[index]["data"]["message"]["role"] = json!("user"),
            "wrong-source" => r[index]["data"]["message"]["source"]["kind"] = json!("user"),
            "bad-source-ranges" => r[3]["sourceEventSeqs"] = json!([[0, 999999]]),
            "bad-surface" => {
                r[index]["surfaceOp"] = json!({"op":"replace","startSeq":0,"endSeq":99999})
            }
            _ => {}
        }
        let mut text = encode(&r);
        if variant == "half" {
            text.pop();
        }
        if variant == "blank" {
            text.insert(text.find('\n').unwrap() + 1, '\n');
        }
        let dir = TempDir::new("dsh-invalid");
        let (_, p) = carrier(&dir, text.as_bytes(), false);
        let out = direct(&p, &StoredScanState::default(), &ScanLimits::default());
        assert!(out.events.is_empty(), "{variant}");
        assert!(out.cursor.is_none(), "{variant}");
        assert!(out.parse_context.is_none());
        assert_eq!(out.health, "degraded");
        assert!(!out.diagnostics.is_empty());
    }
    let dir = TempDir::new("dsh-cap");
    let (_, p) = carrier(&dir, NATIVE.as_bytes(), false);
    for limits in [
        ScanLimits {
            jsonl: llm_usage_core::adapters::jsonl::JsonlLimits {
                max_lines: Some(2),
                ..Default::default()
            },
        },
        ScanLimits {
            jsonl: llm_usage_core::adapters::jsonl::JsonlLimits {
                max_line_bytes: 20,
                ..Default::default()
            },
        },
    ] {
        let out = direct(&p, &StoredScanState::default(), &limits);
        assert!(out.events.is_empty());
        assert!(out.cursor.is_none());
    }
    let mut file = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(session_v4::MAX_FILE_BYTES))
        .unwrap();
    file.write_all(&[0]).unwrap();
    drop(file);
    let cap = direct(&p, &StoredScanState::default(), &ScanLimits::default());
    assert!(cap.events.is_empty());
    assert!(cap.cursor.is_none());
    assert_eq!(cap.status, ScanStatus::LineTooLong);
    let (_, z) = carrier(&dir, NATIVE.as_bytes(), true);
    let mut b = std::fs::read(&z).unwrap();
    b.truncate(b.len() - 4);
    std::fs::write(&z, b).unwrap();
    let broken = direct(&z, &StoredScanState::default(), &ScanLimits::default());
    assert!(broken.events.is_empty());
    assert!(broken.cursor.is_none());
    let mut r = rows();
    r[3]["type"] = json!("future-plugin");
    r[3]["ignorable"] = json!(true);
    r[3]["data"] = json!({"opaque":true});
    let (_, p) = carrier(&dir, encode(&r).as_bytes(), false);
    assert_eq!(
        direct(&p, &StoredScanState::default(), &ScanLimits::default())
            .events
            .len(),
        2
    );
}
#[test]
fn zstd_window_and_decoded_expansion_caps_are_enforced_before_json_allocation() {
    use std::io::Write;
    let dir = TempDir::new("dsh-zstd-caps");
    let (_, p) = carrier(&dir, NATIVE.as_bytes(), true);
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 1).unwrap();
    encoder.window_log(27).unwrap();
    encoder.write_all(NATIVE.as_bytes()).unwrap();
    std::fs::write(&p, encoder.finish().unwrap()).unwrap();
    let window = direct(&p, &StoredScanState::default(), &ScanLimits::default());
    assert!(window.events.is_empty());
    assert!(window.cursor.is_none());
    assert!(window
        .diagnostics
        .iter()
        .any(|d| d.code == "dsh_snapshot_unreadable"));
    let expanded = vec![b' '; session_v4::MAX_FILE_BYTES as usize + 1];
    let compressed = zstd::stream::encode_all(expanded.as_slice(), 1).unwrap();
    assert!(compressed.len() < 10000);
    std::fs::write(&p, compressed).unwrap();
    let cap = direct(&p, &StoredScanState::default(), &ScanLimits::default());
    assert!(cap.events.is_empty());
    assert!(cap.cursor.is_none());
    assert!(cap
        .diagnostics
        .iter()
        .any(|d| d.code == "decoded_exceeds_size_cap"));
}
#[test]
fn seed_cut_excludes_inherited_usage_and_ordinary_resume_retains_history() {
    let mut r = rows();
    let marker = r
        .iter()
        .position(|v| v["type"] == "session/end-seed")
        .unwrap();
    r[0]["isSeeded"] = json!(true);
    r[0]["parentSession"] = json!("anonymous-parent");
    r[0]["origin"] = json!("subagent");
    r[marker]["data"]["inherited"] = json!(true);
    let dir = TempDir::new("dsh-seed");
    let (_, p) = carrier(&dir, encode(&r).as_bytes(), false);
    let out = direct(&p, &StoredScanState::default(), &ScanLimits::default());
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].usage.total_tokens, Some(5643));
    assert_eq!(
        out.events[0].call_category,
        llm_usage_core::domain::CallCategory::SubAgent
    );
    r[marker]["data"] = json!({});
    std::fs::write(&p, encode(&r)).unwrap();
    assert!(
        direct(&p, &StoredScanState::default(), &ScanLimits::default())
            .cursor
            .is_none()
    );
    r[0]["isSeeded"] = json!(false);
    r[marker]["data"]["inherited"] = json!(true);
    std::fs::write(&p, encode(&r)).unwrap();
    assert!(
        direct(&p, &StoredScanState::default(), &ScanLimits::default())
            .cursor
            .is_none()
    );
}
#[test]
fn settlement_replaces_downward_last_stream_fallback_and_retry_adds_observed_attempt() {
    let mut r = rows();
    let index = r
        .iter()
        .position(|v| v["type"] == "assistant/message")
        .unwrap();
    let mut replacement = r[index].clone();
    replacement["data"]["usage"] = json!({"inputTokens":10,"outputTokens":2,"totalTokens":12});
    replacement["data"]["stream"] = json!([{ "type":"chunk","chunk":{"type":"usage","usage":{"inputTokens":999,"outputTokens":9,"totalTokens":1008}}}]);
    r.insert(index + 1, replacement);
    let retry = json!({"type":"llm/retry-started","seq":0,"time":1791291216542i64,"data":{"turn":1,"step":1}});
    r.insert(index + 2, retry);
    let attempt = json!({"type":"assistant/attempt","seq":0,"time":1791291216543i64,"data":{"turn":1,"step":1,"stream":[{"type":"chunk","chunk":{"type":"usage","usage":{"inputTokens":20,"outputTokens":3,"totalTokens":23}}},{"type":"chunk","chunk":{"type":"usage","usage":{"inputTokens":5,"outputTokens":1,"totalTokens":6}}}]}});
    r.insert(index + 3, attempt);
    for (seq, row) in r.iter_mut().skip(1).enumerate() {
        row["seq"] = json!(seq);
    }
    let dir = TempDir::new("dsh-settlement");
    let (_, p) = carrier(&dir, encode(&r).as_bytes(), false);
    let out = direct(&p, &StoredScanState::default(), &ScanLimits::default());
    assert_eq!(out.events.len(), 3);
    assert_eq!(
        out.events
            .iter()
            .filter_map(|e| e.usage.input_uncached)
            .sum::<i64>(),
        49
    );
    assert_eq!(
        out.events
            .iter()
            .filter_map(|e| e.usage.output_total)
            .sum::<i64>(),
        44
    );
    let failed = out
        .events
        .iter()
        .find(|e| e.error_status.is_some())
        .unwrap();
    assert!(failed.model_raw.is_none());
    assert_eq!(failed.usage.input_uncached, Some(5));
    assert_eq!(failed.usage.input_total, None);
    assert_eq!(failed.usage.source_total, None);
    let success = out
        .events
        .iter()
        .find(|e| e.usage.input_uncached == Some(10))
        .unwrap();
    assert_eq!(success.usage.total_tokens, Some(12));
    assert_ne!(success.attempt_id, failed.attempt_id);
}
#[test]
fn zero_clamp_foreign_protocol_bad_usage_and_known_neighbors_preserve_unknowns() {
    for variant in ["zero", "bad", "foreign", "mismatched"] {
        let mut r = rows();
        let index = r
            .iter()
            .position(|v| v["type"] == "assistant/message")
            .unwrap();
        match variant {
            "zero" => {
                r[index]["data"]["usage"] =
                    json!({"inputTokens":0,"outputTokens":0,"totalTokens":99,"cacheReadTokens":99})
            }
            "bad" => r[index]["data"]["usage"]["inputTokens"] = json!(-1),
            "foreign" => {
                r[index]["data"]["message"]["source"]["replayState"]["response"]["api"] =
                    json!("anthropic-messages")
            }
            "mismatched" => {
                r[index]["data"]["message"]["source"]["replayState"]["response"]["provider"] =
                    json!("foreign")
            }
            _ => unreachable!(),
        }
        let dir = TempDir::new("dsh-zero");
        let (_, p) = carrier(&dir, encode(&r).as_bytes(), false);
        let out = direct(&p, &StoredScanState::default(), &ScanLimits::default());
        assert_eq!(out.events.len(), 2, "{variant}");
        assert!(out.cursor.is_some());
        let changed = out
            .events
            .iter()
            .find(|e| e.occurred_at_ms == 1791291216541i64)
            .unwrap();
        assert!(changed.usage.input_total.is_none());
        assert!(changed.usage.total_tokens.is_none());
        assert_eq!(changed.quality.input_total, Q::Unknown);
        assert!(changed.usage.source_total.is_none());
        if variant == "bad" {
            assert!(out
                .diagnostics
                .iter()
                .any(|d| d.code == "usage_shape_deviation"));
            assert_eq!(changed.quality.input_uncached, Q::Unknown);
        }
        assert!(out
            .events
            .iter()
            .any(|e| e.usage.total_tokens == Some(5643)));
    }
}
#[test]
fn old_pending_source_recovers_and_checkpoint_fault_rolls_back_then_replay_is_idempotent() {
    let dir = TempDir::new("dsh-rollback");
    let (ctx, path) = carrier(&dir, NATIVE.as_bytes(), true);
    let (_db, s) = temp_storage("dsh-rollback");
    s.conn().execute_batch("CREATE TRIGGER fail_dsh_checkpoint BEFORE INSERT ON ingestion_checkpoints BEGIN SELECT RAISE(ABORT,'injected checkpoint failure'); END;").unwrap();
    let reports = run_adapter_scan(&s, &DshAdapter::new(), &ctx, &config("fault")).unwrap();
    assert!(reports[0].error.is_some());
    assert_eq!(count(&s, "SELECT COUNT(*) FROM usage_events"), 0);
    assert_eq!(count(&s, "SELECT COUNT(*) FROM ingestion_checkpoints"), 0);
    s.conn()
        .execute_batch("DROP TRIGGER fail_dsh_checkpoint")
        .unwrap();
    run(&s, &ctx, "recover");
    native_sums(&s);
    run(&s, &ctx, "repeat");
    native_sums(&s);
    let state = direct(&path, &StoredScanState::default(), &ScanLimits::default());
    let saved = StoredScanState {
        cursor: state.cursor,
        parse_context: state.parse_context,
    };
    let stable = direct(&path, &saved, &ScanLimits::default());
    assert_eq!(
        stable.events[0].source_revision,
        state.events[0].source_revision
    );
    let mut r = rows();
    r.truncate(18);
    std::fs::write(
        &path,
        zstd::stream::encode_all(encode(&r).as_bytes(), 3).unwrap(),
    )
    .unwrap();
    let regressed = direct(&path, &saved, &ScanLimits::default());
    assert!(regressed.events.is_empty());
    assert!(regressed.cursor.is_none());
    assert!(regressed
        .diagnostics
        .iter()
        .any(|d| d.code == "dsh_snapshot_regressed"));
}
