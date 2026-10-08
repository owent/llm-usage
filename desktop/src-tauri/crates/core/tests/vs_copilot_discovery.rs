mod common;

use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::adapters::vs_copilot::VsCopilotAdapter;
use llm_usage_core::jobs::TriggerKind;
use serde_json::json;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_791_288_000_000;

fn trace(temp: &Path, id: &str) -> PathBuf {
    let dir = temp.join("VSGitHubCopilotLogs/traces");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{id}_VSGitHubCopilot_traces.jsonl"));
    let batch = json!({"resourceSpans":[{
        "resource":{"attributes":[
            {"key":"service.name","value":{"stringValue":"vs-copilot"}},
            {"key":"service.version","value":{"stringValue":"18.10.1203+60f4a0a576"}}
        ]},
        "scopeSpans":[{"spans":[{
            "name":"chat model-a","kind":3,"traceId":id,"spanId":"call",
            "startTimeUnixNano":((NOW-100)*1_000_000).to_string(),
            "endTimeUnixNano":(NOW*1_000_000).to_string(),
            "attributes":[
                {"key":"gen_ai.usage.input_tokens","value":{"intValue":"100"}},
                {"key":"gen_ai.usage.output_tokens","value":{"intValue":"10"}}
            ]
        }]}]
    }]});
    std::fs::write(&path, format!("{batch}\n")).unwrap();
    path
}

fn config() -> RunConfig {
    RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: TriggerKind::Manual,
        run_id_prefix: "vs-directory-recovery".into(),
        origin_host_id: None,
    }
}

#[test]
fn tmp_and_temp_are_both_discovered_and_old_cursors_stay_idempotent() {
    let (dir, storage) = common::temp_storage("vs-temp-recovery");
    let temp = dir.path().join("temp");
    let tmp = dir.path().join("tmp");
    trace(&temp, "old");
    let old_context = DiscoverContext {
        env: [("TEMP".into(), temp.to_string_lossy().into_owned())].into(),
        ..Default::default()
    };
    run_adapter_scan(&storage, &VsCopilotAdapter::new(), &old_context, &config()).unwrap();
    let old_cursor: String = storage
        .conn()
        .query_row("SELECT cursor_value FROM ingestion_checkpoints", [], |r| {
            r.get(0)
        })
        .unwrap();
    trace(&tmp, "new");
    let mut ctx = old_context;
    ctx.env
        .insert("TMP".into(), tmp.to_string_lossy().into_owned());
    for round in 1..=2 {
        for adapter in llm_usage_core::adapters::built_in_adapters() {
            let mut config = config();
            config.run_id_prefix =
                format!("vs-directory-recovery-{round}-{}", adapter.adapter_id());
            run_adapter_scan(&storage, adapter.as_ref(), &ctx, &config).unwrap();
        }
        let totals: (i64, i64, i64) = storage
            .conn()
            .query_row(
                "SELECT COUNT(*), SUM(input_total), SUM(output_total) FROM usage_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(totals, (2, 200, 20));
        let cursor: String = storage
            .conn()
            .query_row(
                "SELECT c.cursor_value FROM ingestion_checkpoints c JOIN source_files f
                ON c.instance_id=f.instance_id AND c.scope_key=f.file_identity
                WHERE f.file_id LIKE '%old_VSGitHubCopilot_traces.jsonl'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cursor, old_cursor);
        let owners: i64 = storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM source_files WHERE instance_id NOT LIKE 'vs-copilot@%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(owners, 0);
    }
}

#[test]
fn profile_temp_survives_a_launcher_temp_override_and_duplicate_candidates() {
    let (dir, _) = common::temp_storage("vs-profile-temp");
    let local = dir.path().join("local");
    let file = trace(&local.join("Temp"), "profile");
    let mut ctx = DiscoverContext {
        env: [
            (
                "TEMP".into(),
                dir.path().join("launcher").to_string_lossy().into_owned(),
            ),
            ("TMP".into(), " ".into()),
            ("LOCALAPPDATA".into(), local.to_string_lossy().into_owned()),
        ]
        .into(),
        ..Default::default()
    };
    assert_eq!(
        VsCopilotAdapter::new().discover(&ctx)[0].files,
        vec![file.clone()]
    );
    ctx.env.insert(
        "TMP".into(),
        local.join("Temp").to_string_lossy().into_owned(),
    );
    ctx.manual_roots = vec![file.clone(), file.parent().unwrap().to_path_buf()];
    let roots = VsCopilotAdapter::new().discover(&ctx);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].files, vec![file]);
}

#[test]
fn a_manual_temp_parent_uses_the_same_identity_as_the_traces_directory() {
    let (dir, _) = common::temp_storage("vs-parent-root");
    let file = trace(dir.path(), "manual");
    let ctx = |root: PathBuf| DiscoverContext {
        manual_roots: vec![root],
        ..Default::default()
    };
    let adapter = VsCopilotAdapter::new();
    let parent = adapter.discover(&ctx(dir.path().to_path_buf()));
    let direct = adapter.discover(&ctx(file.parent().unwrap().to_path_buf()));
    assert_eq!(parent.len(), 1);
    assert_eq!(parent[0].files, vec![file]);
    assert_eq!(
        adapter.instance_id(&parent[0]),
        adapter.instance_id(&direct[0])
    );
}

#[test]
fn manual_only_context_never_reads_the_process_temp() {
    assert!(VsCopilotAdapter::new()
        .discover(&DiscoverContext::default())
        .is_empty());
}
