//! M8 第二批适配器约定测试：17 个适配器的合成 fixture（尚未用真实样本核验，
//! 2026-09-29 调研锚点；本机盘点均未安装，仅 Zed 有空库）经 探测→扫描→
//! 各步骤的断言。期望值为本文件各用例内的人工核算，不改计算规则。
//!
//! 手工核算摘要（fixture 数值）：
//! - zed：cumulative{in=120,out=34}（cr/cc 缺省=已报告 0）⇒ 聚合
//!   120/34/0/0；request 桶和 161 vs cumulative 154 ⇒ mismatch（覆盖语义）；
//!   非 zed.dev 与 imported（version 1.0.0）线程跳过。
//! - aider：{prompt=10006,completion=81,total=10087,cost=0.0133175} 秒→毫秒
//!   1_755_100_406_000；cost=13_318 micro-USD（Estimated）。
//! - junie：modelUsage[2]（input/output + cache 别名组）⇒ 2 事件；
//!   time=1500 ⇒ interval_start = ts−1500；重复扫描键稳定。
//! - xum：byModel 两模型 ⇒ 2 聚合；键 provider:model 拆分。
//! - droid：tokenUsage{in=10,out=20,cr=30,cc=40,think=5} ⇒ 1 聚合。
//! - amp：ledger 2 行入账（有 timestamp）；1 条无 ledger 对应的 assistant
//!   usage 跳过（不推造时间）⇒ 诊断 + Reconciliation mismatch。
//! - grok：2 行显式 usage（别名组），1 行无 usage 跳过 ⇒ 2 事件。
//! - roo：tokensIn=100（含缓存）+ cr=30+cw=10 ⇒ uncached=60 total=120；
//!   api_req_deleted 不计；condense cost-only 辅助事件。
//! - goose：usage_ledger 3 行（provider_reported cost 入账、estimated 不入、
//!   carried_forward token 入账 cost 不入）⇒ 3 事件 1 cost；旧库 accumulated
//!   ⇒ 1 聚合。
//! - crush：根会话 cost=0.5 ⇒ 1 条 UsageObservation（500_000 micro-USD
//!   Estimated）；子会话行排除。
//! - jcode：openai input=100 cr=30 cw=10 ⇒ uncached=60；anthropic in=60
//!   cr=30 cw=10 ⇒ input_total=100；崩溃窗口同 id 取 journal 版。
//! - gajae：五桶 {1,2,3,4,10} ⇒ 按 pi 的计算规则，totalTokens=10 与四桶和一致；
//!   缺桶行跳过。
//! - commandcode：链上 2 条 assistant usage；孤儿分支 1 条不计。
//! - continue：CLI usage ⇒ 1 聚合（prompt/completion/cached）；
//!   GUI 会话（无 usage）0 产出。
//! - atomcode：两模型累计 ⇒ 2 聚合；rounds 合计 5；计算规则 input=prompt−cached。
//! - kiro：CLI 1 个非零 turn 入账、1 个全零 turn（Auto）跳过；
//!   sqlite request_metadata 1 事件（毫秒时间戳）。
//! - antigravity：protobuf 行 #1+#2 input、#9.#4 时间戳 ⇒ 事件；无时间戳行
//!   跳过。
//! - qoder：探针 fail closed（诊断 usage_fields_unverified，0 事件）。

mod common;

use llm_usage_core::adapters::framework::{
    DetectOutcome, ScanLimits, ScanStatus, ScanTarget, SourceAdapter, StoredScanState,
};
use llm_usage_core::adapters::jsonl::probe_file;
use std::io::Write as _;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "llm-usage-m8-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, content: impl AsRef<str>) {
    let content = content.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn target_for(path: &Path) -> ScanTarget {
    ScanTarget {
        instance_id: format!("m8@{}", path.display()),
        path: path.to_path_buf(),
        file_id: path.display().to_string(),
        file_identity: format!("identity-{}", path.display()),
        probe: probe_file(path).unwrap(),
        generation: 0,
        rescan: false,
    }
}

fn scan(
    adapter: &dyn SourceAdapter,
    path: &Path,
) -> llm_usage_core::adapters::framework::ScanOutcome {
    adapter
        .scan(
            &target_for(path),
            &StoredScanState::default(),
            &ScanLimits::default(),
            NOW,
        )
        .unwrap()
}

// ---- zed ----

#[test]
fn zed_threads_db_aggregate_and_filters() {
    use llm_usage_core::adapters::zed::ZedAdapter;
    let dir = temp_dir("zed");
    let db = dir.join("threads").join("threads.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT NOT NULL,
             updated_at TEXT NOT NULL, data_type TEXT NOT NULL, data BLOB NOT NULL,
             parent_id TEXT, folder_paths TEXT, folder_paths_order TEXT, created_at TEXT)",
            [],
        )
        .unwrap();
        let thread_json = serde_json::json!({
            "version": "0.3.0",
            "model": {"provider": "zed.dev", "model": "claude-4"},
            "cumulative_token_usage": {"input_tokens": 120, "output_tokens": 34},
            "request_token_usage": {"u1": {"input_tokens": 80, "output_tokens": 20, "cache_read_input_tokens": 7, "cache_creation_input_tokens": 54}},
        })
        .to_string();
        conn.execute(
            "INSERT INTO threads (id, summary, updated_at, data_type, data, created_at)
             VALUES ('t1', 's', '2026-08-05T06:38:00Z', 'json', ?1, '2026-08-01T00:00:00Z')",
            [thread_json.as_bytes()],
        )
        .unwrap();
        // 非 zed.dev provider：跳过。
        let external = serde_json::json!({
            "model": {"provider": "anthropic", "model": "x"},
            "cumulative_token_usage": {"input_tokens": 999},
        })
        .to_string();
        conn.execute(
            "INSERT INTO threads (id, summary, updated_at, data_type, data)
             VALUES ('t2', 's', '2026-08-05T06:38:00Z', 'json', ?1)",
            [external.as_bytes()],
        )
        .unwrap();
        // 分享导入（version 1.0.0）：跳过。
        let imported = serde_json::json!({
            "version": "1.0.0",
            "model": {"provider": "zed.dev", "model": "y"},
            "cumulative_token_usage": {},
        })
        .to_string();
        conn.execute(
            "INSERT INTO threads (id, summary, updated_at, data_type, data)
             VALUES ('t3', 's', '2026-08-05T06:38:00Z', 'json', ?1)",
            [imported.as_bytes()],
        )
        .unwrap();
    }
    let adapter = ZedAdapter::new();
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &db);
    assert_eq!(outcome.aggregates.len(), 1, "仅 zed.dev 线程入账");
    let aggregate = &outcome.aggregates[0];
    assert_eq!(aggregate.usage.input_total, Some(120));
    assert_eq!(aggregate.usage.output_total, Some(34));
    assert_eq!(aggregate.usage.input_cache_read, Some(0), "缺省=已报告 0");
    assert_eq!(
        aggregate.usage.input_cache_write,
        Some(0),
        "cumulative 缺 cc=已报告 0"
    );
    assert_eq!(aggregate.reported_call_count, Some(1));
    assert_eq!(outcome.reconciliations.len(), 1);
    assert_eq!(
        outcome.reconciliations[0].verdict, "mismatch",
        "桶和 161 vs cumulative 154（覆盖语义如实对照）"
    );
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "non_zed_dev_thread_skipped"));
}

// ---- aider ----

#[test]
fn aider_analytics_events_and_cost() {
    use llm_usage_core::adapters::aider::AiderAdapter;
    let dir = temp_dir("aider");
    let log = dir.join("analytics.jsonl");
    write(
        &log,
        concat!(
            "{\"event\":\"launched\",\"properties\":{\"x\":1},\"user_id\":\"u\",\"time\":1755100400}\n",
            "{\"event\":\"message_send\",\"properties\":{\"main_model\":\"gemini/gemini-2.5-pro\",\"prompt_tokens\":10006,\"completion_tokens\":81,\"total_tokens\":10087,\"cost\":0.0133175,\"total_cost\":0.0133175},\"user_id\":\"u\",\"time\":1755100406}\n",
            "{\"event\":\"message_send_exception\",\"properties\":{\"e\":\"x\"},\"user_id\":\"u\",\"time\":1755100407}\n"
        ),
    );
    let adapter = AiderAdapter::new();
    assert!(matches!(
        adapter.detect(&log).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &log);
    assert_eq!(outcome.events.len(), 1, "仅 message_send 入账");
    let event = &outcome.events[0];
    assert_eq!(event.occurred_at_ms, 1_755_100_406_000, "Unix 秒→毫秒");
    assert_eq!(event.usage.input_total, Some(10006));
    assert_eq!(event.usage.total_tokens, Some(10087));
    assert_eq!(event.model_raw.as_deref(), Some("gemini-2.5-pro"));
    assert_eq!(event.provider_id.as_deref(), Some("gemini"));
    let cost = event.cost.as_ref().unwrap();
    assert_eq!(cost.amount_minor, 13_318, "micro-USD Estimated");
    // 追加一行再扫：增量。
    std::fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(b"{\"event\":\"message_send\",\"properties\":{\"main_model\":\"gpt/x\",\"prompt_tokens\":5,\"completion_tokens\":1,\"total_tokens\":6},\"user_id\":\"u\",\"time\":1755100410}\n")
        .unwrap();
    let stored = StoredScanState {
        cursor: outcome.cursor.clone(),
        parse_context: outcome.parse_context.clone(),
    };
    let target = target_for(&log);
    let outcome2 = adapter
        .scan(&target, &stored, &ScanLimits::default(), NOW)
        .unwrap();
    assert_eq!(outcome2.events.len(), 1, "增量只读新行");
    assert_eq!(outcome2.events[0].usage.input_total, Some(5));
}

// ---- junie ----

#[test]
fn junie_model_usage_events() {
    use llm_usage_core::adapters::junie::JunieAdapter;
    let dir = temp_dir("junie");
    let session = dir.join("session-260901-101010").join("events.jsonl");
    let ts = 1_790_000_000_000i64;
    write(
        &session,
        format!(
            concat!(
                "{{\"kind\":\"x\",\"timestampMs\":{ts},\"event\":{{\"agentEvent\":{{\"kind\":\"AgentStateUpdatedEvent\"}}}}}}\n",
                "{{\"kind\":\"x\",\"timestampMs\":{ts},\"event\":{{\"agentEvent\":{{\"kind\":\"LlmResponseMetadataEvent\",\"modelUsage\":[",
                "{{\"model\":\"gpt-5-codex\",\"provider\":\"openai\",\"inputTokens\":100,\"outputTokens\":20,\"cacheRead\":30,\"cacheCreateTokens\":10,\"reasoningTokens\":5,\"cost\":0.01,\"time\":1500}},",
                "{{\"model\":\"gpt-5-mini\",\"provider\":\"openai\",\"input\":50,\"output\":10}}]}}}}}}\n"
            ),
            ts = ts
        ),
    );
    let adapter = JunieAdapter::new();
    assert!(matches!(
        adapter.detect(&session).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &session);
    assert_eq!(outcome.events.len(), 2, "modelUsage 每元素一事件");
    let first = &outcome.events[0];
    assert_eq!(first.usage.input_total, Some(100));
    assert_eq!(first.usage.input_cache_read, Some(30));
    assert_eq!(first.usage.output_reasoning, Some(5));
    assert_eq!(first.duration_ms, Some(1500));
    assert_eq!(first.interval_start_ms, Some(ts - 1500));
    assert_eq!(first.cost.as_ref().unwrap().amount_minor, 10_000);
    let second = &outcome.events[1];
    assert_eq!(second.usage.input_total, Some(50), "别名组 input");
    assert_eq!(second.usage.input_cache_read, None);
}

// ---- xum ----

#[test]
fn xum_session_usage_by_model() {
    use llm_usage_core::adapters::xum::XumAdapter;
    let dir = temp_dir("xum");
    let file = dir.join("ws-1").join("session-usage.json");
    write(
        &file,
        serde_json::json!({
            "version": 1,
            "byModel": {
                "anthropic:claude-opus-4-6": {
                    "input": {"tokens": 100, "cost_usd": 0.5},
                    "cached": {"tokens": 40},
                    "cacheCreate": {"tokens": 10},
                    "output": {"tokens": 20},
                    "reasoning": {"tokens": 5}
                },
                "openai:gpt-5": {"input": {"tokens": 7}}
            },
            "lastRequest": {"model": "claude-opus-4-6", "timestamp": 1_790_000_000_000i64}
        })
        .to_string(),
    );
    let adapter = XumAdapter::new();
    assert!(matches!(
        adapter.detect(&file).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &file);
    assert_eq!(outcome.aggregates.len(), 2);
    let primary = outcome
        .aggregates
        .iter()
        .find(|a| a.scope_key.contains("anthropic"))
        .unwrap();
    assert_eq!(primary.usage.input_total, Some(100));
    assert_eq!(primary.usage.input_cache_read, Some(40));
    assert_eq!(primary.usage.input_cache_write, Some(10));
    assert_eq!(primary.interval_end_ms, 1_790_000_000_000);
}

// ---- droid ----

#[test]
fn droid_settings_cumulative_aggregate() {
    use llm_usage_core::adapters::droid::DroidAdapter;
    let dir = temp_dir("droid");
    let file = dir.join("6f1c2ab4.settings.json");
    write(
        &file,
        serde_json::json!({
            "model": "claude-sonnet-4-5",
            "providerLock": "anthropic",
            "providerLockTimestamp": "2026-06-01T00:00:00Z",
            "tokenUsage": {"inputTokens": 10, "outputTokens": 20, "cacheReadTokens": 30,
                            "cacheCreationTokens": 40, "thinkingTokens": 5}
        })
        .to_string(),
    );
    let adapter = DroidAdapter::new();
    assert!(matches!(
        adapter.detect(&file).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &file);
    assert_eq!(outcome.aggregates.len(), 1);
    let aggregate = &outcome.aggregates[0];
    assert_eq!(aggregate.scope_key, "droid:6f1c2ab4");
    assert_eq!(aggregate.usage.input_total, Some(10));
    assert_eq!(aggregate.usage.output_reasoning, Some(5));
    assert!(aggregate.scope_key.starts_with("droid:"));
}

// ---- amp ----

#[test]
fn amp_ledger_primary_and_unmatched_message_reported() {
    use llm_usage_core::adapters::amp::AmpAdapter;
    let dir = temp_dir("amp");
    let thread = dir.join("T-abc123.json");
    write(
        &thread,
        serde_json::json!({
            "id": "T-abc123",
            "created": 1_790_000_000_000u64,
            "messages": [
                {"role": "user", "messageId": 1},
                {"role": "assistant", "messageId": 2,
                 "usage": {"model": "claude-4-sonnet", "inputTokens": 100, "outputTokens": 20,
                            "cacheReadInputTokens": 30, "cacheCreationInputTokens": 10, "credits": 1.5}},
                {"role": "assistant", "messageId": 3,
                 "usage": {"model": "claude-4-sonnet", "inputTokens": 999, "outputTokens": 1}}
            ],
            "usageLedger": {"events": [
                {"timestamp": "2026-06-01T00:00:10Z", "model": "claude-4-sonnet",
                 "credits": 1.5, "operationType": "chat",
                 "fromMessageId": 1, "toMessageId": 2,
                 "tokens": {"input": 100, "output": 20, "cacheReadInputTokens": 30,
                             "cacheCreationInputTokens": 10}}
            ]}
        })
        .to_string(),
    );
    let adapter = AmpAdapter::new();
    assert!(matches!(
        adapter.detect(&thread).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &thread);
    // ledger 1 行入账；messageId=2 已对账不计；messageId=3 无 ledger 且无时间戳跳过。
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].usage.input_total, Some(100));
    assert_eq!(
        outcome.events[0].occurred_at_ms, 1_780_272_010_000,
        "2026-06-01T00:00:10Z"
    );
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "message_usage_without_ledger"));
    assert_eq!(outcome.reconciliations.len(), 1);
    assert_eq!(outcome.reconciliations[0].verdict, "mismatch");
}

// ---- grok ----

#[test]
fn grok_explicit_usage_blocks_only() {
    use llm_usage_core::adapters::grok::GrokAdapter;
    let dir = temp_dir("grok");
    let updates = dir.join("ws").join("s1").join("updates.jsonl");
    let line = |event_id: &str, usage: Option<&str>, ts: i64| -> String {
        let usage_json = usage.map(|u| format!(",\"usage\":{u}")).unwrap_or_default();
        let mut out = String::from(
            "{\"jsonrpc\":\"2.0\",\"method\":\"m\",\"params\":{\"_meta\":{\"eventId\":\"",
        );
        out.push_str(event_id);
        out.push_str("\",\"agentTimestampMs\":");
        out.push_str(&ts.to_string());
        out.push_str("},\"update\":{\"sessionUpdate\":\"assistant_message_chunk\"");
        out.push_str(&usage_json);
        out.push_str("}}}\n");
        out
    };
    write(
        &updates,
        format!(
            "{}{}{}",
            line(
                "e1",
                Some(
                    r#"{"inputTokens":100,"outputTokens":20,"cachedReadTokens":30,"reasoningTokens":5}"#
                ),
                1_790_000_000_000
            ),
            line(
                "e2",
                Some(r#"{"input_tokens":50,"output_tokens":10,"cache_creation_input_tokens":4}"#),
                1_790_000_100_000
            ),
            line("e3", None, 1_790_000_200_000),
        ),
    );
    let adapter = GrokAdapter::new();
    assert!(matches!(
        adapter.detect(&updates).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &updates);
    assert_eq!(outcome.events.len(), 2, "只有显式 usage 块入账");
    assert_eq!(outcome.events[0].usage.input_total, Some(100));
    assert_eq!(outcome.events[0].usage.input_cache_read, Some(30));
    assert_eq!(outcome.events[1].usage.input_cache_write, Some(4));
    assert!(
        outcome.events[0].source_record_key.contains("e1"),
        "键含 eventId"
    );
}

// ---- roo ----

#[test]
fn roo_tokens_in_includes_cache_and_deleted_memo_skipped() {
    use llm_usage_core::adapters::roo::RooAdapter;
    let dir = temp_dir("roo");
    let task = dir.join("tasks").join("12345").join("ui_messages.json");
    write(
        &task,
        serde_json::json!([
            {"type": "say", "say": "api_req_started", "ts": 1_790_000_000_000i64,
             "text": "{\"tokensIn\":100,\"tokensOut\":20,\"cacheWrites\":10,\"cacheReads\":30,\"cost\":0.01,\"apiProtocol\":\"anthropic\"}"},
            {"type": "say", "say": "api_req_deleted", "ts": 1_790_000_100_000i64,
             "text": "{\"tokensIn\":50,\"tokensOut\":5}"},
            {"type": "say", "say": "condense_context", "ts": 1_790_000_200_000i64,
             "contextCondense": {"cost": 0.002, "prevContextTokens": 1000, "newContextTokens": 500}}
        ])
        .to_string(),
    );
    let adapter = RooAdapter::new();
    assert!(matches!(
        adapter.detect(&task).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &task);
    // 1 请求 + 1 condense 辅助；deleted 备忘不计。
    assert_eq!(outcome.events.len(), 2);
    let request = &outcome.events[0];
    assert_eq!(request.usage.input_total, Some(100));
    assert_eq!(request.usage.input_uncached, Some(60), "tokensIn 含缓存");
    assert_eq!(request.usage.total_tokens, Some(120));
    assert_eq!(request.cost.as_ref().unwrap().amount_minor, 10_000);
    let condense = &outcome.events[1];
    assert_eq!(
        condense.call_category,
        llm_usage_core::domain::CallCategory::Auxiliary
    );
    assert_eq!(condense.usage.input_total, None, "condense token 未知");
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "deleted_request_memo_skipped"));
}

// ---- goose ----

#[test]
fn goose_usage_ledger_and_old_db_fallback() {
    use llm_usage_core::adapters::goose::GooseAdapter;
    let adapter = GooseAdapter::new();
    // 新库：usage_ledger。
    let dir = temp_dir("goose");
    let db = dir.join("sessions").join("sessions.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE usage_ledger (id INTEGER PRIMARY KEY AUTOINCREMENT,
             session_id TEXT NOT NULL, created_timestamp INTEGER NOT NULL, model TEXT,
             input_tokens INTEGER, output_tokens INTEGER, total_tokens INTEGER,
             cache_read_tokens INTEGER, cache_write_tokens INTEGER, cost REAL,
             cost_source TEXT, is_compaction INTEGER DEFAULT 0)",
            [],
        )
        .unwrap();
        let insert =
            |input: i64, output: i64, cr: i64, cw: i64, cost: Option<f64>, source: &str| {
                conn.execute(
                    "INSERT INTO usage_ledger (session_id, created_timestamp, model,
                 input_tokens, output_tokens, total_tokens, cache_read_tokens,
                 cache_write_tokens, cost, cost_source)
                 VALUES ('20260929_1', 1790000000, 'gpt-5', ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![input, output, input + output, cr, cw, cost, source],
                )
                .unwrap();
            };
        insert(100, 20, 30, 10, Some(0.5), "provider_reported");
        insert(5, 1, 0, 0, Some(0.2), "estimated");
        insert(50, 5, 0, 0, Some(0.1), "carried_forward");
    }
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &db);
    assert_eq!(outcome.events.len(), 3);
    let with_cost = outcome.events.iter().filter(|e| e.cost.is_some()).count();
    assert_eq!(with_cost, 1, "仅 provider_reported cost 入账");
    assert_eq!(
        outcome.events[0].usage.input_uncached,
        Some(60),
        "input 含缓存"
    );

    // 旧库：无 usage_ledger，accumulated_* 兜底。
    let old = dir.join("sessions-old").join("sessions.db");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&old).unwrap();
        conn.execute(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, name TEXT, description TEXT,
             user_set_name BOOLEAN, session_type TEXT, working_dir TEXT,
             created_at TIMESTAMP, updated_at TIMESTAMP, extension_data TEXT,
             total_tokens INTEGER, input_tokens INTEGER, output_tokens INTEGER,
             cache_read_tokens INTEGER, cache_write_tokens INTEGER,
             accumulated_total_tokens INTEGER, accumulated_input_tokens INTEGER,
             accumulated_output_tokens INTEGER, accumulated_cache_read_tokens INTEGER,
             accumulated_cache_write_tokens INTEGER, accumulated_cost REAL,
             schedule_id TEXT, recipe_json TEXT, user_recipe_values_json TEXT,
             provider_name TEXT, model_config_json TEXT, goose_mode TEXT,
             archived_at TIMESTAMP, project_id TEXT, parent_session_id TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, working_dir, created_at, updated_at,
             accumulated_total_tokens, accumulated_input_tokens, accumulated_output_tokens)
             VALUES ('20260101_1', '/w', '2026-01-01 00:00:00', '2026-01-02 00:00:00',
             330, 300, 30)",
            [],
        )
        .unwrap();
    }
    assert!(matches!(
        adapter.detect(&old).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &old);
    assert_eq!(outcome.aggregates.len(), 1);
    assert_eq!(outcome.aggregates[0].usage.input_total, Some(300));
}

// ---- crush ----

#[test]
fn crush_root_session_cost_only() {
    use llm_usage_core::adapters::crush::CrushAdapter;
    let dir = temp_dir("crush");
    let db = dir.join("proj").join(".crush").join("crush.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT,
             title TEXT NOT NULL, message_count INTEGER NOT NULL DEFAULT 0,
             prompt_tokens INTEGER NOT NULL DEFAULT 0,
             completion_tokens INTEGER NOT NULL DEFAULT 0,
             cost REAL NOT NULL DEFAULT 0.0, updated_at INTEGER NOT NULL,
             created_at INTEGER NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, parent_session_id, title, cost, created_at, updated_at)
             VALUES ('root-1', NULL, 't', 0.5, 1790000000, 1790001000)",
            [],
        )
        .unwrap();
        // 子会话（cost 回卷父行）：排除。
        conn.execute(
            "INSERT INTO sessions (id, parent_session_id, title, cost, created_at, updated_at)
             VALUES ('child-1', 'root-1', 't', 0.2, 1790000500, 1790000900)",
            [],
        )
        .unwrap();
        // 零 cost 根会话：跳过。
        conn.execute(
            "INSERT INTO sessions (id, parent_session_id, title, cost, created_at, updated_at)
             VALUES ('root-2', NULL, 't', 0.0, 1790000000, 1790001000)",
            [],
        )
        .unwrap();
    }
    let adapter = CrushAdapter::new();
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &db);
    assert_eq!(outcome.events.len(), 1, "仅根会话且 cost>0");
    let event = &outcome.events[0];
    assert_eq!(
        event.record_kind,
        llm_usage_core::domain::RecordKind::UsageObservation
    );
    assert_eq!(event.cost.as_ref().unwrap().amount_minor, 500_000);
    assert_eq!(event.usage.input_total, None, "token 快照不采");
}

// ---- jcode ----

#[test]
fn jcode_snapshot_journal_merge_and_cache_semantics() {
    use llm_usage_core::adapters::jcode::JcodeAdapter;
    let dir = temp_dir("jcode");
    let session = dir.join("session_butterfly_1790000000_621ff7b9");
    let snapshot = session.with_extension("json");
    write(
        &snapshot,
        serde_json::json!({
            "id": "session_butterfly_1790000000_621ff7b9",
            "provider_key": "openai",
            "model": "gpt-5.1",
            "created_at": "2026-06-01T00:00:00Z",
            "messages": [
                {"id": "m0", "role": "user", "timestamp": "2026-06-01T00:00:01Z"},
                {"id": "m1", "role": "assistant", "timestamp": "2026-06-01T00:00:05Z",
                 "token_usage": {"input_tokens": 100, "output_tokens": 20,
                                  "cache_read_input_tokens": 30, "cache_creation_input_tokens": 10,
                                  "prompt_tokens": 999}}
            ]
        })
        .to_string(),
    );
    let journal = dir.join("session_butterfly_1790000000_621ff7b9.journal.jsonl");
    write(
        &journal,
        concat!(
            "{\"meta\":{\"provider_key\":\"anthropic\",\"model\":\"claude-4\"},\"append_messages\":[",
            "{\"id\":\"m1\",\"role\":\"assistant\",\"timestamp\":\"2026-06-01T00:00:05Z\",",
            "\"token_usage\":{\"input_tokens\":60,\"output_tokens\":15,\"cache_read_input_tokens\":30,\"cache_creation_input_tokens\":10}},",
            "{\"id\":\"m2\",\"role\":\"assistant\",\"timestamp\":\"2026-06-01T00:00:09Z\",\"tool_duration_ms\":1200,",
            "\"token_usage\":{\"input_tokens\":50,\"output_tokens\":5}}]}\n"
        ),
    );
    let adapter = JcodeAdapter::new();
    assert!(matches!(
        adapter.detect(&snapshot).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &snapshot);
    // m1 取 journal 版（meta 覆盖为 anthropic：in=60 cr=30 cw=10 ⇒ total=100）；
    // m2 anthropic in=50 无 cache ⇒ total=50。
    assert_eq!(outcome.events.len(), 2);
    let m1 = outcome
        .events
        .iter()
        .find(|e| e.source_record_key.ends_with("m1"))
        .unwrap();
    assert_eq!(m1.usage.input_uncached, Some(60), "anthropic 三列互斥");
    assert_eq!(m1.usage.input_total, Some(100));
    assert_eq!(
        m1.model_raw.as_deref(),
        Some("claude-4"),
        "journal meta 权威"
    );
    let m2 = outcome
        .events
        .iter()
        .find(|e| e.source_record_key.ends_with("m2"))
        .unwrap();
    assert_eq!(
        m2.usage.input_uncached,
        Some(50),
        "anthropic input 即非缓存桶"
    );
    assert_eq!(m2.usage.input_total, None, "cache 桶未知不带和");
    assert_eq!(m2.duration_ms, Some(1200));
}

// ---- gajae-code ----

#[test]
fn gajae_five_bucket_gate_and_pi_mapping() {
    use llm_usage_core::adapters::gajae_code::GajaeCodeAdapter;
    let dir = temp_dir("gjc");
    let session = dir.join("v2-abc").join("2026-09-29T10-00-00-000Z_s1.jsonl");
    write(
        &session,
        concat!(
            "{\"type\":\"session\",\"version\":5,\"id\":\"s1\",\"timestamp\":\"2026-09-29T10:00:00Z\",\"cwd\":\"/w\"}\n",
            "{\"type\":\"message\",\"id\":\"ab12cd34\",\"parentId\":null,\"timestamp\":\"2026-09-29T10:00:05Z\",",
            "\"message\":{\"role\":\"assistant\",\"provider\":\"anthropic\",\"model\":\"claude-4\",",
            "\"timestamp\":1790000005000,\"usage\":{\"input\":1,\"output\":2,\"cacheRead\":3,\"cacheWrite\":4,\"totalTokens\":10,",
            "\"cost\":{\"input\":0.001,\"output\":0.002,\"cacheRead\":0.0003,\"cacheWrite\":0.0004,\"total\":0.0037}}}}\n",
            "{\"type\":\"message\",\"id\":\"ff00ff00\",\"parentId\":null,\"timestamp\":\"2026-09-29T10:00:06Z\",",
            "\"message\":{\"role\":\"assistant\",\"provider\":\"anthropic\",\"model\":\"claude-4\",\"timestamp\":1790000006000,",
            "\"usage\":{\"input\":5,\"output\":1,\"totalTokens\":7}}}\n",
            "{\"type\":\"model_change\",\"id\":\"aa11\",\"parentId\":\"ab12cd34\",\"timestamp\":\"2026-09-29T10:00:07Z\",\"model\":\"anthropic/claude-4\"}\n"
        ),
    );
    let adapter = GajaeCodeAdapter::new();
    assert!(matches!(
        adapter.detect(&session).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &session);
    // 五桶齐全 1 条入账；缺桶行跳过（官方 parser 规则相同）。
    assert_eq!(outcome.events.len(), 1);
    let event = &outcome.events[0];
    assert_eq!(event.usage.input_uncached, Some(1));
    assert_eq!(event.usage.input_total, Some(8), "输入侧和 1+3+4");
    assert_eq!(event.usage.total_tokens, Some(10), "四桶和 8+2");
    assert_eq!(event.usage.source_total, Some(10));
    assert_eq!(event.cost.as_ref().unwrap().amount_minor, 3_700);
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "usage_bucket_missing"));
}

// ---- commandcode ----

#[test]
fn commandcode_effective_path_and_orphan_exclusion() {
    use llm_usage_core::adapters::commandcode::CommandCodeAdapter;
    let dir = temp_dir("cmd");
    let session = dir.join("proj-slug").join("sess-1.jsonl");
    write(
        &session,
        concat!(
            "{\"type\":\"session\",\"version\":3,\"id\":\"sess-1\",\"timestamp\":\"2026-09-29T00:00:00Z\",\"cwd\":\"/w\"}\n",
            "{\"type\":\"message\",\"id\":\"aaaa0001\",\"parentId\":null,\"timestamp\":\"2026-09-29T00:00:01Z\",",
            "\"message\":{\"role\":\"user\",\"content\":[]}}\n",
            "{\"type\":\"message\",\"id\":\"aaaa0002\",\"parentId\":\"aaaa0001\",\"timestamp\":\"2026-09-29T00:00:05Z\",",
            "\"message\":{\"role\":\"assistant\",\"content\":[]},\"usage\":{\"inputTokens\":100,\"outputTokens\":20,\"cacheReadTokens\":30,\"cacheWriteTokens\":10,\"costUsd\":0.01},\"model\":\"anthropic/claude-4\"}\n",
            "{\"type\":\"message\",\"id\":\"bbbb0001\",\"parentId\":\"aaaa0001\",\"timestamp\":\"2026-09-29T00:01:00Z\",",
            "\"message\":{\"role\":\"assistant\",\"content\":[]},\"usage\":{\"inputTokens\":999,\"outputTokens\":9,\"cacheReadTokens\":0,\"cacheWriteTokens\":0}}\n",
            "{\"type\":\"message\",\"id\":\"cccc0001\",\"parentId\":\"aaaa0002\",\"timestamp\":\"2026-09-29T00:02:00Z\",",
            "\"message\":{\"role\":\"assistant\",\"content\":[]},\"usage\":{\"inputTokens\":50,\"outputTokens\":5,\"cacheReadTokens\":0,\"cacheWriteTokens\":0}}\n"
        ),
    );
    let adapter = CommandCodeAdapter::new();
    assert!(matches!(
        adapter.detect(&session).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &session);
    // head=cccc0001 → aaaa0002 → aaaa0001：bbbb0001 是 rewind 孤儿不计。
    assert_eq!(outcome.events.len(), 2);
    let keys: Vec<&str> = outcome
        .events
        .iter()
        .map(|e| e.source_record_key.as_str())
        .collect();
    assert!(keys.iter().any(|k| k.contains("cccc0001")));
    assert!(!keys.iter().any(|k| k.contains("bbbb0001")), "孤儿分支不计");
    let first = outcome
        .events
        .iter()
        .find(|e| e.source_record_key.contains("aaaa0002"))
        .unwrap();
    assert_eq!(first.usage.input_uncached, Some(60), "inputTokens 含缓存");
}

// ---- continue ----

#[test]
fn continue_cli_usage_aggregate_and_gui_empty() {
    use llm_usage_core::adapters::continuedev::ContinueAdapter;
    let adapter = ContinueAdapter::new();
    let dir = temp_dir("continue");
    let cli_session = dir.join("0b6c3a2e-1.json");
    write(
        &cli_session,
        serde_json::json!({
            "sessionId": "0b6c3a2e-1",
            "title": "t",
            "history": [],
            "usage": {"promptTokens": 300, "completionTokens": 40,
                       "promptTokensDetails": {"cachedTokens": 100},
                       "totalCost": 0.5}
        })
        .to_string(),
    );
    assert!(matches!(
        adapter.detect(&cli_session).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &cli_session);
    assert_eq!(outcome.aggregates.len(), 1);
    let aggregate = &outcome.aggregates[0];
    assert_eq!(aggregate.usage.input_total, Some(300));
    assert_eq!(aggregate.usage.input_cache_read, Some(100));
    assert_eq!(aggregate.usage.output_total, Some(40));

    // GUI 会话（无 usage 字段）：0 产出、正常完成。
    let gui_session = dir.join("0b6c3a2e-2.json");
    write(
        &gui_session,
        serde_json::json!({"sessionId": "0b6c3a2e-2", "title": "t", "history": []}).to_string(),
    );
    let outcome = scan(&adapter, &gui_session);
    assert_eq!(outcome.status, ScanStatus::Complete);
    assert!(outcome.aggregates.is_empty());
}

// ---- atomcode ----

#[test]
fn atomcode_meta_turns_by_model() {
    use llm_usage_core::adapters::atomcode::AtomCodeAdapter;
    let dir = temp_dir("atomcode");
    let meta = dir.join("hash1").join("sess-1.meta");
    write(
        &meta,
        serde_json::json!({
            "v": 1,
            "id": "sess-1",
            "working_dir": "/w",
            "created_at": 1_790_000_000_000i64,
            "updated_at": 1_790_000_900_000i64,
            "turn_count": 2,
            "turn_stats": [
                {"turn_id": "t1", "round_count": 3, "duration_ms": 5000, "total_tokens": 130,
                 "model_usage": [
                    {"provider_id": "anthropic", "model_id": "claude-4",
                     "tokens": {"input": 60, "cached_input": 40, "output": 20}},
                    {"provider_id": "openai", "model_id": "gpt-5",
                     "tokens": {"input": 10, "cached_input": 0, "output": 5}}
                 ]},
                {"turn_id": "t2", "round_count": 2, "duration_ms": 3000, "total_tokens": 50,
                 "model_usage": [
                    {"provider_id": "anthropic", "model_id": "claude-4",
                     "tokens": {"input": 30, "cached_input": 10, "output": 10}}
                 ]}
            ],
            "detached_model_usage": [
                {"provider_id": "openai", "model_id": "gpt-5",
                 "tokens": {"input": 4, "cached_input": 1, "output": 2}}
            ]
        })
        .to_string(),
    );
    let adapter = AtomCodeAdapter::new();
    assert!(matches!(
        adapter.detect(&meta).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &meta);
    // 两模型各一聚合 + 多模型 turn 的 round_count 单列 unattributed
    // （t1 的 3 轮调用无法确认所属模型，不能对 claude/gpt 各计一次虚增合计）。
    assert_eq!(outcome.aggregates.len(), 3, "两模型各一聚合 + unattributed");
    let claude = outcome
        .aggregates
        .iter()
        .find(|a| a.scope_key.contains("claude-4"))
        .unwrap();
    // 60+30=90 未缓存 + 40+10=50 缓存 ⇒ input_total=140；
    // 仅单模型 turn t2 的 2 轮归属 claude。
    assert_eq!(claude.usage.input_uncached, Some(90));
    assert_eq!(claude.usage.input_cache_read, Some(50));
    assert_eq!(claude.usage.input_total, Some(140));
    assert_eq!(claude.reported_call_count, Some(2));
    let gpt = outcome
        .aggregates
        .iter()
        .find(|a| a.scope_key.contains("gpt-5"))
        .unwrap();
    assert_eq!(gpt.usage.input_uncached, Some(14), "turn 10 + detached 4");
    assert_eq!(gpt.usage.input_total, Some(15));
    assert_eq!(gpt.reported_call_count, None, "多模型 turn 的轮次不摊派");
    let unattributed = outcome
        .aggregates
        .iter()
        .find(|a| a.scope_key.ends_with(":unattributed"))
        .unwrap();
    assert_eq!(unattributed.reported_call_count, Some(3), "t1 的 3 轮单列");
    // 调用数总计守恒：2 + 3 = 5 = 3(t1) + 2(t2)。
    let total_calls: i64 = outcome
        .aggregates
        .iter()
        .filter_map(|a| a.reported_call_count)
        .sum();
    assert_eq!(total_calls, 5);
}

// ---- kiro ----

#[test]
fn kiro_cli_turns_skip_auto_zero_and_sqlite_requests() {
    use llm_usage_core::adapters::kiro::KiroAdapter;
    let adapter = KiroAdapter::new();
    let dir = temp_dir("kiro");
    let cli = dir.join("cli").join("session-1.json");
    write(
        &cli,
        serde_json::json!({
            "session_id": "session-1",
            "cwd": "/w",
            "session_state": {
                "rts_model_state": {"model_info": {"model_id": "claude-sonnet-4", "context_window_tokens": 200000}},
                "conversation_metadata": {"user_turn_metadatas": [
                    {"input_token_count": 100, "output_token_count": 20,
                     "cache_read_input_token_count": 30, "cache_write_input_token_count": 10,
                     "end_timestamp": 1790000000, "total_request_count": 2},
                    {"input_token_count": 0, "output_token_count": 0,
                     "end_timestamp": 1790000100}
                ]}
            }
        })
        .to_string(),
    );
    assert!(matches!(
        adapter.detect(&cli).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &cli);
    // 全零 turn（Auto agent 默认）跳过；1 个真实 turn 入账。
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(
        outcome.events[0].occurred_at_ms, 1_790_000_000_000,
        "秒→毫秒"
    );
    assert_eq!(outcome.events[0].usage.input_total, Some(100));
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "zero_count_turns_skipped"));

    // kiro-cli SQLite：request_metadata 逐请求。
    let db = dir.join("kiro-cli").join("data.sqlite3");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE conversations_v2 (key TEXT, conversation_id TEXT, value TEXT)",
            [],
        )
        .unwrap();
        let value = serde_json::json!({
            "conversation_id": "conv-1",
            "history": [
                {"user": {}, "assistant": {}, "request_metadata": {
                    "request_start_timestamp_ms": 1790000010000i64,
                    "stream_end_timestamp_ms": 1790000012000i64,
                    "input_tokens": 80, "output_tokens": 8,
                    "cache_read_input_tokens": 20,
                    "cache_write_input_tokens": 5,
                    "reasoning_tokens": 3,
                    "request_count": 1}}
            ]
        })
        .to_string();
        conn.execute(
            "INSERT INTO conversations_v2 (key, conversation_id, value) VALUES ('/w', 'conv-1', ?1)",
            [&value],
        )
        .unwrap();
    }
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &db);
    assert_eq!(outcome.events.len(), 1);
    let event = &outcome.events[0];
    assert_eq!(event.usage.input_total, Some(80));
    assert_eq!(event.usage.output_reasoning, Some(3));
    assert_eq!(event.occurred_at_ms, 1_790_000_010_000);
}

// ---- antigravity ----

#[test]
fn antigravity_protobuf_rows_and_timestamp_gate() {
    use llm_usage_core::adapters::antigravity::AntigravityAdapter;
    let dir = temp_dir("agy");
    let db = dir.join("conv-1.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB)",
            [],
        )
        .unwrap();
        // protobuf：chatModel{#19 model bytes, #9{#4 Timestamp{#1 seconds}}} +
        // usage{#1 1132, #2 100, #5 40, #9 20, #10 5, #11 "resp-1"}。
        let mut row1 = protobuf_field_bytes(1, &{
            let mut m = protobuf_field_bytes(19, b"gemini-2.6-pro");
            m.extend_from_slice(&protobuf_field_bytes(
                9,
                &protobuf_field_bytes(4, &protobuf_field_varint(1, 1_790_000_000)),
            ));
            m
        });
        row1.extend_from_slice(&protobuf_field_bytes(4, &{
            let mut u = protobuf_field_varint(1, 1132);
            u.extend_from_slice(&protobuf_field_varint(2, 100));
            u.extend_from_slice(&protobuf_field_varint(5, 40));
            u.extend_from_slice(&protobuf_field_varint(9, 20));
            u.extend_from_slice(&protobuf_field_varint(10, 5));
            u.extend_from_slice(&protobuf_field_bytes(11, b"resp-1"));
            u
        }));
        // 行 2：无 #9.#4 时间戳（1.1.18+ 布局）⇒ 跳过。
        let row2 = protobuf_field_bytes(4, &{
            let mut u = protobuf_field_varint(2, 999);
            u.extend_from_slice(&protobuf_field_varint(9, 99));
            u
        });
        conn.execute(
            "INSERT INTO gen_metadata (idx, data) VALUES (1, ?1), (2, ?2)",
            rusqlite::params![row1, row2],
        )
        .unwrap();
    }
    let adapter = AntigravityAdapter::new();
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported { .. }
    ));
    let outcome = scan(&adapter, &db);
    assert_eq!(outcome.events.len(), 1, "无时间戳行跳过");
    let event = &outcome.events[0];
    assert_eq!(
        event.usage.input_total,
        Some(1232),
        "input=#1+#2 固定提示计入"
    );
    assert_eq!(event.usage.input_cache_read, Some(40));
    assert_eq!(event.usage.output_reasoning, Some(5));
    assert_eq!(event.occurred_at_ms, 1_790_000_000_000);
    assert_eq!(event.model_raw.as_deref(), Some("gemini-2.6-pro"));
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "rows_without_verifiable_timestamp"));
}

fn protobuf_varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    out
}

fn protobuf_field_varint(field: u64, v: u64) -> Vec<u8> {
    let mut out = protobuf_varint(field << 3);
    out.extend(protobuf_varint(v));
    out
}

fn protobuf_field_bytes(field: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = protobuf_varint((field << 3) | 2);
    out.extend(protobuf_varint(payload.len() as u64));
    out.extend_from_slice(payload);
    out
}

// ---- qoder ----

#[test]
fn qoder_probe_fail_closed() {
    use llm_usage_core::adapters::qoder::QoderAdapter;
    let dir = temp_dir("qoder");
    let session = dir.join("proj").join("sess-1.jsonl");
    write(&session, "{\"sessionId\":\"sess-1\"}\n");
    let adapter = QoderAdapter::new();
    let outcome = scan(&adapter, &session);
    assert!(outcome.events.is_empty(), "探针不解析");
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "usage_fields_unverified"));
}

// ---- 结构约定：V30 注册表形状（M8 全目录）----

#[test]
fn m8_adapter_directories_and_registries() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("adapters");
    for agent in [
        "zed",
        "aider",
        "junie",
        "xum",
        "droid",
        "amp",
        "grok",
        "roo",
        "goose",
        "crush",
        "jcode",
        "gajae-code",
        "commandcode",
        "continue",
        "atomcode",
        "kiro",
        "antigravity",
        "qoder",
    ] {
        let dir = root.join(agent);
        assert!(dir.is_dir(), "{agent} 目录缺失");
        assert!(dir.join("mod.rs").is_file(), "{agent}/mod.rs 缺失");
        assert!(dir.join("detect.rs").is_file(), "{agent}/detect.rs 缺失");
        assert!(
            dir.join("versions").join("mod.rs").is_file(),
            "{agent}/versions/mod.rs 缺失"
        );
        let registry = std::fs::read_to_string(dir.join("versions").join("mod.rs")).unwrap();
        assert!(
            registry.contains("VERIFIED_VERSION_IMPLS"),
            "{agent} 注册表缺 VERIFIED_VERSION_IMPLS"
        );
        assert!(
            registry.contains("LATEST_IMPL_ID"),
            "{agent} 注册表缺 LATEST_IMPL_ID"
        );
        assert!(registry.contains("fn select"), "{agent} 注册表缺 select()");
    }
}
