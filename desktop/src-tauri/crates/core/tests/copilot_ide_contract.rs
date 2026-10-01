mod common;

use llm_usage_core::adapters::copilot_chat::CopilotChatAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceAdapter,
};
use llm_usage_core::adapters::vs_copilot::VsCopilotAdapter;
use llm_usage_core::jobs::TriggerKind;
use llm_usage_core::storage::Storage;
use serde_json::{json, Value};
use std::path::Path;

const NOW: i64 = 1_800_000_000_000;
fn turn(id: &str) -> Value {
    json!({"requestId":id,"agent":{"id":"github.copilot.editsAgent"},"timestamp":1_790_000_000_000i64,
        "promptTokens":100,"completionTokens":30,"modelState":{"value":1,"completedAt":1_790_000_000_100i64},
        "result":{"metadata":{"resolvedModel":"model-a","toolCallRounds":[
            {"id":"round-a","modelId":"model-a","timestamp":1_790_000_000_050i64},
            {"id":"round-b","modelId":"model-a","timestamp":1_790_000_000_100i64}]}}})
}
fn write_log(path: &Path, requests: Vec<Value>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "{}\n",
            json!({"kind":0,"v":{"version":3,"sessionId":"session-a","requests":requests}})
        ),
    )
    .unwrap();
}
fn run(storage: &Storage, root: &Path, now: i64) {
    let reports = run_adapter_scan(
        storage,
        &CopilotChatAdapter::new(),
        &DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![root.to_path_buf()],
        },
        &RunConfig {
            timezone: "UTC".into(),
            now_ms: now,
            limits: ScanLimits::default(),
            trigger: TriggerKind::Manual,
            run_id_prefix: format!("test-{now}"),
            origin_host_id: None,
        },
    )
    .unwrap();
    assert!(!reports.is_empty());
    assert!(reports.iter().all(|r| r.error.is_none()));
    assert!(reports
        .iter()
        .filter_map(|r| r.outcome.as_ref())
        .all(|o| o.errors == 0 && o.conflicts == 0));
}
fn totals(storage: &Storage) -> (i64, i64, i64) {
    storage
        .conn()
        .query_row(
            "SELECT SUM(call_count),SUM(input_known_sum),SUM(output_known_sum) FROM daily_usage",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

#[test]
fn copilot_turns_count_observed_rounds_and_replace_authoritative_model_totals() {
    let (dir, storage) = common::temp_storage("copilot-ide-snapshots");
    let file = dir.path().join("chatSessions/s.jsonl");
    let mut request = turn("request-a");
    write_log(&file, vec![request.clone()]);
    run(&storage, &file, NOW);
    assert_eq!(totals(&storage), (2, 100, 30));
    // 同键终态更正可降低计数，不能冲突留旧值，也不能取 MAX。
    request["promptTokens"] = json!(80);
    request["completionTokens"] = json!(20);
    write_log(&file, vec![request.clone()]);
    run(&storage, &file, NOW + 1);
    assert_eq!(totals(&storage), (2, 80, 20));
    request["modelTotals"] = json!([
        {"model":"model-a","inputTokens":1000,"cachedTokens":400,"outputTokens":200},
        {"model":"model-b","inputTokens":2000,"cachedTokens":500,"outputTokens":300}]);
    write_log(&file, vec![request.clone()]);
    run(&storage, &file, NOW + 2);
    assert_eq!(totals(&storage), (2, 3000, 500));
    request["modelTotals"].as_array_mut().unwrap().reverse();
    write_log(&file, vec![request]);
    run(&storage, &file, NOW + 3);
    assert_eq!(totals(&storage), (2, 3000, 500));
    run(&storage, &file, NOW + 4);
    assert_eq!(totals(&storage), (2, 3000, 500));
    assert_eq!(
        storage
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE attribution_status='verified'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        4
    );
}

#[test]
fn copilot_copies_migrations_other_participants_and_single_file_scope() {
    let (dir, storage) = common::temp_storage("copilot-ide-discovery");
    let user = dir.path().join("User");
    let first = user.join("workspaceStorage/a/chatSessions/s.jsonl");
    let copy = user.join("workspaceStorage/b/chatSessions/s.jsonl");
    write_log(&first, vec![turn("request-a")]);
    write_log(&copy, vec![turn("request-a")]);
    let mut other = turn("other-request");
    other["agent"]["id"] = json!("other-extension.agent");
    write_log(&first.parent().unwrap().join("other.jsonl"), vec![other]);
    let adapter = CopilotChatAdapter::new();
    let roots = adapter.discover(&DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![first.clone()],
    });
    assert_eq!(roots[0].files, vec![first.clone()]);
    run(&storage, &user.join("workspaceStorage"), NOW);
    assert_eq!(totals(&storage), (2, 100, 30));
    // 空窗口新载体与工作区来源共享安装命名空间。
    let empty = user.join("globalStorage/emptyWindowChatSessions/s.jsonl");
    write_log(&empty, vec![turn("request-a")]);
    run(&storage, &empty, NOW + 1);
    assert_eq!(totals(&storage), (2, 100, 30));
}

#[test]
fn copilot_partial_tail_does_not_commit_stale_snapshot() {
    let (dir, storage) = common::temp_storage("copilot-ide-tail");
    let file = dir.path().join("chatSessions/s.jsonl");
    write_log(&file, vec![turn("request-a")]);
    run(&storage, &file, NOW);
    use std::io::Write;
    let mut writer = std::fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap();
    writer
        .write_all(b"{\"kind\":1,\"k\":[\"requests\",0,\"completionTokens\"],\"v\":")
        .unwrap();
    run(&storage, &file, NOW + 1);
    assert_eq!(totals(&storage), (2, 100, 30));
    writer.write_all(b"10}\n").unwrap();
    run(&storage, &file, NOW + 2);
    assert_eq!(totals(&storage), (2, 100, 10));
}

#[test]
fn copilot_source_cleanup_preserves_observed_history() {
    let (dir, storage) = common::temp_storage("copilot-ide-cleanup");
    let file = dir.path().join("chatSessions/s.jsonl");
    write_log(&file, vec![turn("a")]);
    run(&storage, &file, NOW);
    write_log(&file, vec![]);
    run(&storage, &file, NOW + 1);
    assert_eq!(
        totals(&storage),
        (2, 100, 30),
        "removing history does not undo consumption"
    );
}

#[test]
fn quota_hard_retention_does_not_restore_old_cached_snapshot() {
    use llm_usage_core::quota_history::{record, QuotaObservation};
    use llm_usage_core::retention::{enforce_retention, RetentionPolicy};
    let (_dir, storage) = common::temp_storage("copilot-quota-retention");
    let q = QuotaObservation {
        observed_at_ms: Some(NOW - 30 * 86_400_000),
        agent: "copilot".into(),
        quota_id: "premium_interactions".into(),
        kind: "rate_limit".into(),
        unit: "milli_requests".into(),
        limit_value: Some(100000),
        used: Some(1000),
        remaining: Some(99000),
        percent_remaining: Some(99.0),
        window_start_ms: None,
        window_end_ms: None,
        locality_verified: false,
        detail: None,
    };
    assert_eq!(
        record(&storage, std::slice::from_ref(&q), "UTC", NOW).unwrap(),
        1
    );
    let out = enforce_retention(
        &storage,
        "UTC",
        NOW,
        &RetentionPolicy {
            detail_days: 7,
            diagnostics_days: 7,
            hard_max_days: Some(7),
        },
    )
    .unwrap();
    assert_eq!(out.deleted_quota_rows, 1);
    assert_eq!(record(&storage, &[q], "UTC", NOW).unwrap(), 0);
}

#[test]
fn vs_copilot_rechecks_service_on_every_batch_and_counts_failed_unknown_usage() {
    let (dir, storage) = common::temp_storage("vs-copilot-ide");
    let file = dir.path().join("traces/s.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let span = json!({"name":"chat model-a","kind":3,"traceId":"trace-a","spanId":"span-a",
        "startTimeUnixNano":"1790000000000000000","endTimeUnixNano":"1790000000100000000",
        "status":{"code":2},"attributes":[]});
    let batch = |service: &str| json!({"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":service}}]},"scopeSpans":[{"spans":[span.clone()]}]}]});
    std::fs::write(
        &file,
        format!("{}\n{}\n", batch("vs-copilot"), batch("unrelated")),
    )
    .unwrap();
    let reports = run_adapter_scan(
        &storage,
        &VsCopilotAdapter::new(),
        &DiscoverContext {
            home_dir: None,
            env: Default::default(),
            manual_roots: vec![file],
        },
        &RunConfig {
            timezone: "UTC".into(),
            now_ms: NOW,
            limits: ScanLimits::default(),
            trigger: TriggerKind::Manual,
            run_id_prefix: "vs-test".into(),
            origin_host_id: None,
        },
    )
    .unwrap();
    assert_eq!(reports[0].outcome.as_ref().unwrap().added, 1);
    let values: (i64, Option<i64>, String) = storage
        .conn()
        .query_row(
            "SELECT COUNT(*),input_total,error_status FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(values, (1, None, "error".into()));
    let day: (i64, Option<i64>, Option<i64>) = storage
        .conn()
        .query_row(
            "SELECT SUM(call_count),SUM(input_known_sum),SUM(output_known_sum) FROM daily_usage",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(day, (1, None, None));
}
