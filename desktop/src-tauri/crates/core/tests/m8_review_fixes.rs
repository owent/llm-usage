//! M8 提交评审修复的回归测试（2026-09-29）。
//!
//! 每条用例钉住一个评审确认的问题，防止回退：
//! - copilot：未知 schema 走兼容回退，坏 token 桶诊断不补零。
//! - otel：TTFT 三键按文档单位换算（不按数值猜）；token 越界跳过记录；
//!   span status ERROR ⇒ error_status。
//! - roo：同毫秒两条 api_req_started 都入账（去重键含序号）。
//! - jcode：journal 读取截断如实上报（BudgetExhausted/LineTooLong），
//!   不再静默漏计尾部消息；同消息时间戳的更正按会话更新时间修订。
//! - aider：完全相同的两行都入账（键含行号）；同父多个手工目录根不互吞。
//! - goose：token 全 NULL 但 provider cost 有效的行仍入账；schema 指纹
//!   变化后游标重置全量重读；旧库累计会话更新后重读。
//! - crush/kiro/zed：SQLite 行上限后可续扫；Crush 累计成本按修订替换；
//!   Kiro 半写入 SQLite 魔数 Pending。
//! - atomcode：多模型 turn 的 round_count 只计一次（unattributed 单列）；
//!   零 token 但有 rounds 的行保留调用数。
//! - gajae：version≠5 的事件 parse_basis 如实标 LatestFallback。
//! - grok：token 越界行跳过记诊断。
//! - junie：用量指纹在 64 KiB 头之后仍 Supported；类型化日志无指纹 Pending。
//! - zed/crush：单行类型错误跳行记诊断，不中止整轮。
//! - xum：版本取顶层字段；停用来源即使被逐源任务选中也不扫描。

use llm_usage_core::adapters::framework::{
    DetectOutcome, DiscoverContext, ScanLimits, ScanStatus, ScanTarget, SourceAdapter,
    StoredScanState,
};
use llm_usage_core::adapters::jsonl::{probe_file, JsonlLimits};
use llm_usage_core::domain::VersionBasis;
use std::io::Write as _;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "llm-usage-m8fix-{}-{}-{}",
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
        instance_id: format!("m8fix@{}", path.display()),
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

fn scan_with(
    adapter: &dyn SourceAdapter,
    path: &Path,
    stored: &StoredScanState,
    limits: &ScanLimits,
) -> llm_usage_core::adapters::framework::ScanOutcome {
    adapter
        .scan(&target_for(path), stored, limits, NOW)
        .unwrap()
}

// ---- otel ----

#[test]
fn otel_ttft_units_follow_documentation() {
    use llm_usage_core::adapters::otel::OtelAdapter;
    let dir = temp_dir("otel-ttft");
    let path = dir.join("spans.jsonl");
    write(
        &path,
        concat!(
            // copilot_chat.time_to_first_token 是毫秒：7298ms 保持 7298，
            // 不能按"数值小=秒"猜成 7,298,000ms。
            r#"{"name":"chat","spanId":"a1","startTime":[1780000000,0],"attributes":{"gen_ai.usage.input_tokens":10,"copilot_chat.time_to_first_token":7298}}"#,
            "\n",
            // gen_ai.response.time_to_first_chunk 是秒（可含小数）：1.5s ⇒ 1500ms。
            r#"{"name":"chat","spanId":"a2","startTime":[1780000000,0],"attributes":{"gen_ai.usage.input_tokens":10,"gen_ai.response.time_to_first_chunk":1.5}}"#,
            "\n",
            // response.time_to_first_token（agentlens，单位未标）：量级启发
            // 只用于该键——300 按秒折算 300000ms；15000 按毫秒保持。
            r#"{"name":"model_stream","spanId":"a3","startTime":[1780000000,0],"attributes":{"usage.input_tokens":10,"response.time_to_first_token":300}}"#,
            "\n",
            r#"{"name":"model_stream","spanId":"a4","startTime":[1780000000,0],"attributes":{"usage.input_tokens":10,"response.time_to_first_token":15000}}"#,
            "\n",
        ),
    );
    let outcome = scan(&OtelAdapter::new(), &path);
    assert_eq!(outcome.events.len(), 4);
    assert_eq!(outcome.events[0].ttft_ms, Some(7_298));
    assert_eq!(outcome.events[1].ttft_ms, Some(1_500));
    assert_eq!(outcome.events[2].ttft_ms, Some(300_000));
    assert_eq!(outcome.events[3].ttft_ms, Some(15_000));
}

#[test]
fn otel_out_of_range_token_skips_record_with_diagnostic() {
    use llm_usage_core::adapters::otel::OtelAdapter;
    let dir = temp_dir("otel-range");
    let path = dir.join("spans.jsonl");
    write(
        &path,
        concat!(
            // 负 token：整条记录跳过（不能静默丢桶后入账）。
            r#"{"name":"chat","spanId":"b1","startTime":[1780000000,0],"attributes":{"gen_ai.usage.input_tokens":-5,"gen_ai.usage.output_tokens":3}}"#,
            "\n",
            // 合法记录不受影响。
            r#"{"name":"chat","spanId":"b2","startTime":[1780000000,0],"attributes":{"gen_ai.usage.input_tokens":7}}"#,
            "\n",
        ),
    );
    let outcome = scan(&OtelAdapter::new(), &path);
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].source_record_key, "otel:b2");
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "token_shape_deviation"));
}

#[test]
fn otel_error_span_marks_error_status() {
    use llm_usage_core::adapters::otel::OtelAdapter;
    let dir = temp_dir("otel-status");
    let path = dir.join("spans.jsonl");
    write(
        &path,
        concat!(
            r#"{"name":"chat","spanId":"c1","startTime":[1780000000,0],"status":{"code":"STATUS_CODE_ERROR"},"attributes":{"gen_ai.usage.input_tokens":10}}"#,
            "\n",
            r#"{"name":"chat","spanId":"c2","startTime":[1780000000,0],"status":{"code":2},"attributes":{"gen_ai.usage.input_tokens":10}}"#,
            "\n",
            r#"{"name":"chat","spanId":"c3","startTime":[1780000000,0],"status":{"code":"STATUS_CODE_OK"},"attributes":{"gen_ai.usage.input_tokens":10}}"#,
            "\n",
        ),
    );
    let outcome = scan(&OtelAdapter::new(), &path);
    assert_eq!(outcome.events.len(), 3);
    assert_eq!(outcome.events[0].error_status.as_deref(), Some("error"));
    assert_eq!(outcome.events[1].error_status.as_deref(), Some("error"));
    assert_eq!(outcome.events[2].error_status, None);
}

#[test]
fn copilot_unknown_schema_uses_fallback_and_reports_bad_bucket() {
    use llm_usage_core::adapters::copilot::CopilotAdapter;
    let dir = temp_dir("copilot-schema");
    let db = dir.join("session-store.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_version (version INTEGER);
             INSERT INTO schema_version VALUES (9);
             CREATE TABLE assistant_usage_events (
               id INTEGER PRIMARY KEY, session_id TEXT, turn_index INTEGER,
               model TEXT, input_tokens INTEGER, output_tokens INTEGER,
               cache_read_tokens INTEGER, cache_write_tokens INTEGER,
               reasoning_tokens INTEGER, duration_ms REAL,
               time_to_first_token_ms REAL, created_at TEXT
             );
             INSERT INTO assistant_usage_events VALUES (
               1, 's1', 0, 'model-x', -5, 3, NULL, NULL, NULL, NULL, NULL,
               '2026-09-29T00:00:00Z'
             );",
        )
        .unwrap();
    }
    let adapter = CopilotAdapter::new();
    assert!(matches!(
        adapter.detect(&db).unwrap(),
        DetectOutcome::Supported {
            basis: VersionBasis::LatestFallback,
            ..
        }
    ));
    let outcome = scan(&adapter, &db);
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(
        outcome.events[0].parse_basis,
        Some(VersionBasis::LatestFallback)
    );
    assert_eq!(outcome.events[0].usage.input_total, None);
    assert_eq!(outcome.events[0].usage.output_total, Some(3));
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "token_shape_deviation"));
}

// ---- roo ----

#[test]
fn roo_same_millisecond_requests_both_counted() {
    use llm_usage_core::adapters::roo::RooAdapter;
    let dir = temp_dir("roo-samems");
    let path = dir.join("task-1").join("ui_messages.json");
    let doc = serde_json::json!([
        {"type": "say", "say": "api_req_started", "ts": 1_780_000_000_000i64,
         "text": r#"{"tokensIn":100,"tokensOut":20,"cacheReads":30,"cacheWrites":10}"#},
        // 同毫秒第二条请求（retry/并行子任务）：键冲突不能吞并。
        {"type": "say", "say": "api_req_started", "ts": 1_780_000_000_000i64,
         "text": r#"{"tokensIn":50,"tokensOut":5}"#},
    ]);
    write(&path, doc.to_string());
    let outcome = scan(&RooAdapter::new(), &path);
    assert_eq!(outcome.events.len(), 2, "{:?}", outcome.diagnostics);
    assert_ne!(
        outcome.events[0].source_record_key, outcome.events[1].source_record_key,
        "同毫秒事件必须有不同的去重键"
    );
    let inputs: Vec<Option<i64>> = outcome.events.iter().map(|e| e.usage.input_total).collect();
    assert!(inputs.contains(&Some(100)) && inputs.contains(&Some(50)));
}

// ---- jcode ----

fn jcode_fixture(dir: &Path, journal_lines: usize) -> (PathBuf, PathBuf) {
    let snapshot = dir.join("s1.json");
    write(
        &snapshot,
        serde_json::json!({
            "provider_key": "openai", "model": "gpt-x",
            "messages": []
        })
        .to_string(),
    );
    let journal = dir.join("s1.journal.jsonl");
    let mut file = std::fs::File::create(&journal).unwrap();
    for _ in 0..journal_lines {
        writeln!(file, r#"{{"meta":{{}}}}"#).unwrap();
    }
    (snapshot, journal)
}

#[test]
fn jcode_journal_budget_exhaustion_reported() {
    use llm_usage_core::adapters::jcode::JcodeAdapter;
    let dir = temp_dir("jcode-budget");
    // 超过硬编码 100k 行预算：旧实现静默报 Complete，尾部消息永久漏计。
    let (snapshot, journal) = jcode_fixture(&dir, 100_500);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap();
    writeln!(
        file,
        "{}",
        serde_json::json!({
            "meta": {"provider_key": "openai", "model": "gpt-x"},
            "append_messages": [{
                "id": "tail-message", "role": "assistant",
                "timestamp": "2026-09-29T00:00:00Z",
                "token_usage": {"input_tokens": 9, "output_tokens": 2}
            }]
        })
    )
    .unwrap();
    drop(file);
    let adapter = JcodeAdapter::new();
    let first = scan(&adapter, &snapshot);
    assert_eq!(first.status, ScanStatus::BudgetExhausted);
    assert!(first
        .diagnostics
        .iter()
        .any(|d| d.code == "journal_budget_exhausted"));
    assert!(first.events.is_empty());
    let stored = StoredScanState {
        cursor: first.cursor,
        parse_context: first.parse_context,
    };
    let second = scan_with(&adapter, &snapshot, &stored, &ScanLimits::default());
    assert_eq!(second.status, ScanStatus::Complete);
    assert_eq!(second.events.len(), 1, "{:?}", second.diagnostics);
    assert_eq!(second.events[0].source_record_key, "jcode:s1:tail-message");
    assert_eq!(second.events[0].usage.input_total, Some(9));
}

#[test]
fn jcode_same_message_timestamp_uses_session_update_for_revision() {
    use llm_usage_core::adapters::jcode::JcodeAdapter;
    use llm_usage_core::identity::{arbitrate, event_content_hash, Arbitration, ExistingMeta};
    let dir = temp_dir("jcode-revision");
    let (snapshot, journal) = jcode_fixture(&dir, 0);
    let entry = |updated_at: &str, input: i64| {
        serde_json::json!({
            "meta": {"updated_at": updated_at, "provider_key": "openai", "model": "gpt-x"},
            "append_messages": [{
                "id": "same-message", "role": "assistant",
                "timestamp": "2026-09-29T00:00:00Z",
                "token_usage": {"input_tokens": input, "output_tokens": 2}
            }]
        })
        .to_string()
    };
    write(&journal, format!("{}\n", entry("2026-09-29T00:01:00Z", 9)));
    let adapter = JcodeAdapter::new();
    let first = scan(&adapter, &snapshot);
    assert_eq!(first.events.len(), 1);
    let stored = StoredScanState {
        cursor: first.cursor,
        parse_context: first.parse_context,
    };
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap();
    writeln!(file, "{}", entry("2026-09-29T00:02:00Z", 12)).unwrap();
    drop(file);
    let second = scan_with(&adapter, &snapshot, &stored, &ScanLimits::default());
    assert_eq!(second.events.len(), 1);
    let old = &first.events[0];
    let new = &second.events[0];
    assert_eq!(old.occurred_at_ms, new.occurred_at_ms);
    assert_eq!(new.usage.input_total, Some(12));
    assert!(new.source_revision > old.source_revision);
    assert_eq!(
        arbitrate(
            Some(&ExistingMeta {
                lifecycle: old.lifecycle,
                source_revision: old.source_revision,
                content_hash: event_content_hash(old),
            }),
            new,
            &event_content_hash(new),
        ),
        Arbitration::Replace
    );
}

#[test]
fn jcode_journal_overlong_line_held_for_retry() {
    use llm_usage_core::adapters::jcode::JcodeAdapter;
    let dir = temp_dir("jcode-longline");
    let snapshot = dir.join("s1.json");
    write(
        &snapshot,
        serde_json::json!({"provider_key": "openai", "messages": []}).to_string(),
    );
    let journal = dir.join("s1.journal.jsonl");
    write(&journal, format!("{{\"pad\":\"{}\"}}\n", "x".repeat(512)));
    let limits = ScanLimits {
        jsonl: JsonlLimits {
            max_line_bytes: 64,
            ..JsonlLimits::default()
        },
    };
    let outcome = scan_with(
        &JcodeAdapter::new(),
        &snapshot,
        &StoredScanState::default(),
        &limits,
    );
    assert_eq!(outcome.status, ScanStatus::LineTooLong);
}

// ---- aider ----

#[test]
fn aider_identical_lines_both_counted() {
    use llm_usage_core::adapters::aider::AiderAdapter;
    let dir = temp_dir("aider-dup");
    let path = dir.join("analytics.jsonl");
    // 完全相同的重复发送（同秒重发同一 prompt）：内容哈希相同，
    // 键必须靠行号区分，不能折叠。
    let line = r#"{"event":"message_send","time":1755100406,"properties":{"main_model":"openai/gpt-x","prompt_tokens":100,"completion_tokens":10,"total_tokens":110,"cost":0.001}}"#;
    write(&path, format!("{line}\n{line}\n"));
    let outcome = scan(&AiderAdapter::new(), &path);
    assert_eq!(outcome.events.len(), 2, "{:?}", outcome.diagnostics);
    assert_ne!(
        outcome.events[0].source_record_key,
        outcome.events[1].source_record_key
    );
}

#[test]
fn aider_sibling_manual_roots_not_merged() {
    use llm_usage_core::adapters::aider::AiderAdapter;
    let dir = temp_dir("aider-roots");
    let a = dir.join("logs").join("a");
    let b = dir.join("logs").join("b");
    write(&a.join("x.jsonl"), "{}\n");
    write(&b.join("y.jsonl"), "{}\n");
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![a, b],
    };
    let roots = AiderAdapter::new().discover(&ctx);
    assert_eq!(roots.len(), 2, "同父的两个手工目录根不能合并: {roots:?}");
}

#[test]
fn disabled_source_is_not_scanned_even_when_explicitly_included() {
    use llm_usage_core::adapters::aider::AiderAdapter;
    use llm_usage_core::adapters::framework::{
        run_adapter_scan_filtered, InstanceFilter, RunConfig,
    };
    use llm_usage_core::jobs::TriggerKind;
    use llm_usage_core::storage::Storage;
    let dir = temp_dir("disabled-source");
    let source = dir.join("analytics.jsonl");
    write(
        &source,
        "{\"event\":\"message_send\",\"time\":1755100406,\"properties\":{\"main_model\":\"openai/gpt-x\",\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n",
    );
    let storage = Storage::open(&dir.join("usage.sqlite")).unwrap();
    let adapter = AiderAdapter::new();
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![source],
    };
    let root = adapter.discover(&ctx).remove(0);
    let instance_id = adapter.instance_id(&root);
    storage.conn().execute(
        "INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,health,created_at_ms,updated_at_ms)
         VALUES (?1,'aider','local_filesystem','verified',0,'ok',?2,?2)",
        rusqlite::params![instance_id, NOW],
    ).unwrap();
    let config = RunConfig {
        timezone: "UTC".into(),
        now_ms: NOW,
        limits: ScanLimits::default(),
        trigger: TriggerKind::FixedTime,
        run_id_prefix: "disabled-test".into(),
        origin_host_id: None,
    };
    let filter = InstanceFilter {
        include: Some(std::collections::BTreeSet::from([instance_id.clone()])),
        exclude: None,
    };
    let reports = run_adapter_scan_filtered(&storage, &adapter, &ctx, &config, &filter).unwrap();
    assert!(reports.is_empty());
    let files: i64 = storage
        .conn()
        .query_row("SELECT COUNT(*) FROM source_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(files, 0);
    storage
        .conn()
        .execute(
            "UPDATE source_instances SET enabled=1 WHERE instance_id=?1",
            [&instance_id],
        )
        .unwrap();
    let reports = run_adapter_scan_filtered(&storage, &adapter, &ctx, &config, &filter).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].finish,
        llm_usage_core::jobs::RunStatus::Succeeded
    );
}

// ---- goose ----

fn goose_ledger_db(dir: &Path) -> PathBuf {
    let db = dir.join("sessions").join("sessions.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE usage_ledger (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            created_timestamp INTEGER,
            model TEXT,
            input_tokens INTEGER, output_tokens INTEGER, total_tokens INTEGER,
            cache_read_tokens INTEGER, cache_write_tokens INTEGER,
            cost REAL, cost_source TEXT, is_compaction INTEGER
        );",
    )
    .unwrap();
    db
}

#[test]
fn goose_null_tokens_with_provider_cost_still_recorded() {
    use llm_usage_core::adapters::goose::GooseAdapter;
    let dir = temp_dir("goose-cost");
    let db = goose_ledger_db(&dir);
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        // token 全 NULL 但 provider_reported cost 有效：成本是已报告事实，不能丢。
        conn.execute(
            "INSERT INTO usage_ledger (session_id, created_timestamp, model,
              input_tokens, output_tokens, total_tokens, cache_read_tokens,
              cache_write_tokens, cost, cost_source, is_compaction)
             VALUES ('s1', 1790000000, 'm', NULL, NULL, NULL, NULL, NULL,
                     0.25, 'provider_reported', 0)",
            [],
        )
        .unwrap();
    }
    let outcome = scan(&GooseAdapter::new(), &db);
    assert_eq!(outcome.events.len(), 1, "{:?}", outcome.diagnostics);
    let event = &outcome.events[0];
    assert_eq!(event.usage.input_total, None);
    assert_eq!(event.cost.as_ref().map(|c| c.amount_minor), Some(250_000));
}

#[test]
fn goose_schema_fingerprint_change_resets_cursor() {
    use llm_usage_core::adapters::goose::GooseAdapter;
    let dir = temp_dir("goose-fingerprint");
    let db = goose_ledger_db(&dir);
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO usage_ledger (session_id, created_timestamp, model,
              input_tokens, output_tokens, total_tokens, cache_read_tokens,
              cache_write_tokens, cost, cost_source, is_compaction)
             VALUES ('s1', 1790000000, 'm', 100, 20, 120, 30, 10,
                     0.0, 'provider_reported', 0)",
            [],
        )
        .unwrap();
    }
    let adapter = GooseAdapter::new();
    let first = scan(&adapter, &db);
    assert_eq!(first.events.len(), 1);
    let stored = StoredScanState {
        cursor: first.cursor.clone(),
        parse_context: first.parse_context.clone(),
    };
    // schema 变化（列集投影进指纹）⇒ 旧游标作废全量重读（upsert 幂等）。
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("ALTER TABLE usage_ledger ADD COLUMN extra TEXT;")
            .unwrap();
    }
    let second = scan_with(&adapter, &db, &stored, &ScanLimits::default());
    assert_eq!(
        second.events.len(),
        1,
        "指纹变化后必须重读已有行: {:?}",
        second.diagnostics
    );
}

#[test]
fn goose_old_session_cumulative_update_is_rescanned() {
    use llm_usage_core::adapters::goose::GooseAdapter;
    let dir = temp_dir("goose-old-update");
    let db = dir.join("sessions").join("sessions.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, accumulated_total_tokens INTEGER,
                accumulated_input_tokens INTEGER, accumulated_output_tokens INTEGER,
                created_at TEXT, updated_at TEXT
            );
            INSERT INTO sessions VALUES (
                'session-a', 12, 10, 2,
                '2026-01-01 00:00:00', '2026-01-01 00:01:00'
            );",
        )
        .unwrap();
    }
    let adapter = GooseAdapter::new();
    let first = scan(&adapter, &db);
    assert_eq!(first.status, ScanStatus::Complete);
    assert_eq!(first.aggregates.len(), 1);
    assert_eq!(first.aggregates[0].usage.input_total, Some(10));
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE sessions SET accumulated_total_tokens=25,
             accumulated_input_tokens=20, accumulated_output_tokens=5,
             updated_at='2026-01-01 00:02:00' WHERE id='session-a'",
            [],
        )
        .unwrap();
    }
    let stored = StoredScanState {
        cursor: first.cursor,
        parse_context: first.parse_context,
    };
    let second = scan_with(&adapter, &db, &stored, &ScanLimits::default());
    assert_eq!(second.aggregates.len(), 1);
    assert_eq!(second.aggregates[0].usage.input_total, Some(20));
}

// ---- atomcode ----

#[test]
fn atomcode_multi_model_turn_rounds_counted_once() {
    use llm_usage_core::adapters::atomcode::AtomCodeAdapter;
    let dir = temp_dir("atomcode-rounds");
    let path = dir.join("s.meta");
    let doc = serde_json::json!({
        "v": 1, "id": "s1",
        "created_at": 1_790_000_000_000i64, "updated_at": 1_790_000_100_000i64,
        "turn_stats": [
            {"turn_id": "t1", "round_count": 3, "model_usage": [
                {"provider_id": "p", "model_id": "m1",
                 "tokens": {"input": 10, "cached_input": 2, "output": 5}},
                {"provider_id": "p", "model_id": "m2",
                 "tokens": {"input": 7, "cached_input": 0, "output": 3}},
            ]},
            {"turn_id": "t2", "round_count": 5, "model_usage": [
                {"provider_id": "p", "model_id": "m1",
                 "tokens": {"input": 1, "cached_input": 0, "output": 1}},
            ]},
            {"turn_id": "t3", "round_count": 2},
            {"turn_id": "t4", "round_count": 4, "model_usage": [
                {"provider_id": "q", "model_id": "m9"},
            ]},
        ],
    });
    write(&path, doc.to_string());
    let outcome = scan(&AtomCodeAdapter::new(), &path);
    let by_key: std::collections::BTreeMap<_, _> = outcome
        .aggregates
        .iter()
        .map(|a| (a.scope_key.clone(), a))
        .collect();
    // 单模型 turn：rounds 归属该模型。
    assert_eq!(
        by_key["atomcode:session:s1:p/m1"].reported_call_count,
        Some(5)
    );
    // 多模型 turn 的 3 轮 + 无 model_usage turn 的 2 轮：单列不摊派。
    assert_eq!(by_key["atomcode:session:s1:p/m2"].reported_call_count, None);
    assert_eq!(
        by_key["atomcode:session:s1:unattributed"].reported_call_count,
        Some(5)
    );
    // 零 token 但有 rounds 的行保留调用数（token 全未知不补零）。
    let zero = &by_key["atomcode:session:s1:q/m9"];
    assert_eq!(zero.reported_call_count, Some(4));
    assert_eq!(zero.usage.input_total, None);
    // 调用数总计守恒：5 + 5 + 4 = 14 = 3+5+2+4。
    let total: i64 = outcome
        .aggregates
        .iter()
        .filter_map(|a| a.reported_call_count)
        .sum();
    assert_eq!(total, 14);
}

// ---- gajae ----

fn gajae_session(version: i64) -> String {
    format!(
        concat!(
            "{{\"type\":\"session\",\"version\":{version},\"id\":\"s1\",\"timestamp\":\"2026-09-01T00:00:00Z\"}}",
            "\n",
            "{{\"type\":\"message\",\"id\":\"abcd1234\",\"timestamp\":\"2026-09-01T00:00:01Z\",\"message\":{{\"role\":\"assistant\",\"provider\":\"p\",\"model\":\"m\",\"usage\":{{\"input\":1,\"output\":2,\"cacheRead\":3,\"cacheWrite\":4,\"totalTokens\":10}},\"timestamp\":1790000000000}}}}",
            "\n",
        ),
        version = version
    )
}

#[test]
fn gajae_fallback_version_marks_parse_basis_honestly() {
    use llm_usage_core::adapters::gajae_code::GajaeCodeAdapter;
    let dir = temp_dir("gajae-basis");
    // version=5（已验证）⇒ KnownVersion。
    let v5 = dir.join("v5.jsonl");
    write(&v5, gajae_session(5));
    let outcome = scan(&GajaeCodeAdapter::new(), &v5);
    assert_eq!(outcome.events.len(), 1, "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.events[0].parse_basis,
        Some(VersionBasis::KnownVersion)
    );
    // version=6（未收录）⇒ LatestFallback（不能虚标 KnownVersion）。
    let v6 = dir.join("v6.jsonl");
    write(&v6, gajae_session(6));
    let outcome = scan(&GajaeCodeAdapter::new(), &v6);
    assert_eq!(outcome.events.len(), 1, "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.events[0].parse_basis,
        Some(VersionBasis::LatestFallback)
    );
}

// ---- grok ----

#[test]
fn grok_out_of_range_token_line_skipped() {
    use llm_usage_core::adapters::grok::GrokAdapter;
    let dir = temp_dir("grok-range");
    let path = dir.join("sess-1").join("updates.jsonl");
    write(
        &path,
        concat!(
            r#"{"params":{"update":{"usage":{"inputTokens":-1,"outputTokens":3}},"timestamp":1790000000000}}"#,
            "\n",
            r#"{"params":{"update":{"usage":{"inputTokens":9,"outputTokens":3}},"timestamp":1790000000001}}"#,
            "\n",
        ),
    );
    let outcome = scan(&GrokAdapter::new(), &path);
    assert_eq!(outcome.events.len(), 1, "{:?}", outcome.diagnostics);
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "token_shape_deviation"));
}

#[test]
fn xum_version_comes_from_top_level_document() {
    use llm_usage_core::adapters::xum::XumAdapter;
    let dir = temp_dir("xum-version");
    let path = dir.join("session-usage.json");
    write(
        &path,
        r#"{"byModel":{"m":{"version":1}},"lastRequest":{},"version":2}"#,
    );
    let outcome = XumAdapter::new().detect(&path).unwrap();
    assert!(matches!(
        outcome,
        DetectOutcome::Supported {
            basis: VersionBasis::LatestFallback,
            ..
        }
    ));
    write(
        &path,
        r#"{"wrapper":{"byModel":{}},"lastRequest":{},"version":1}"#,
    );
    assert!(matches!(
        XumAdapter::new().detect(&path).unwrap(),
        DetectOutcome::UnknownFormat { .. }
    ));
}

// ---- junie ----

#[test]
fn junie_usage_fingerprint_beyond_head_window_supported() {
    use llm_usage_core::adapters::junie::JunieAdapter;
    let dir = temp_dir("junie-window");
    let path = dir.join("sessions").join("s1").join("events.jsonl");
    // 64 KiB 头内只有事件指纹，用量事件在大文件后段：
    // 旧实现误报 UnknownFormat，该会话永远无法入账。
    let mut content = String::new();
    while content.len() < 80 * 1024 {
        content.push_str(r#"{"kind":"AgentStateUpdated","pad":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#);
        content.push('\n');
    }
    content.push_str(r#"{"kind":"LlmResponseMetadataEvent","modelUsage":[]}"#);
    content.push('\n');
    write(&path, &content);
    let outcome = JunieAdapter::new().detect(&path).unwrap();
    assert!(
        matches!(outcome, DetectOutcome::Supported { .. }),
        "用量指纹在头部窗口之后必须仍判 Supported: {outcome:?}"
    );
}

#[test]
fn junie_typed_log_without_usage_is_pending_not_unknown_format() {
    use llm_usage_core::adapters::junie::JunieAdapter;
    let dir = temp_dir("junie-pending");
    let path = dir.join("events.jsonl");
    write(
        &path,
        "{\"kind\":\"AgentStateUpdated\"}\n{\"kind\":\"x\"}\n",
    );
    let outcome = JunieAdapter::new().detect(&path).unwrap();
    assert!(
        matches!(outcome, DetectOutcome::Pending),
        "类型化事件日志无用量指纹应 Pending 重探: {outcome:?}"
    );
    // 非 junie 文件仍 fail closed。
    let other = dir.join("other.jsonl");
    write(&other, "{\"hello\":\"world\"}\n");
    let outcome = JunieAdapter::new().detect(&other).unwrap();
    assert!(matches!(outcome, DetectOutcome::UnknownFormat { .. }));
}

// ---- zed / crush 行级容错 ----

#[test]
fn zed_row_type_error_skips_row_not_scan() {
    use llm_usage_core::adapters::zed::ZedAdapter;
    let dir = temp_dir("zed-rowerr");
    let db = dir.join("threads").join("threads.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, summary TEXT,
             updated_at TEXT, data_type TEXT, data BLOB, created_at TEXT)",
            [],
        )
        .unwrap();
        // data 列存 INTEGER：String 读取类型错误——单行跳过不中止整轮。
        conn.execute(
            "INSERT INTO threads (id, updated_at, data_type, data) VALUES ('bad', '2026-08-05T06:38:00Z', 'json', 42)",
            [],
        )
        .unwrap();
        let good = serde_json::json!({
            "model": {"provider": "zed.dev", "model": "claude-4"},
            "cumulative_token_usage": {"input_tokens": 10, "output_tokens": 3},
        })
        .to_string();
        conn.execute(
            "INSERT INTO threads (id, updated_at, data_type, data) VALUES ('good', '2026-08-05T06:38:00Z', 'json', ?1)",
            [good.as_bytes()],
        )
        .unwrap();
    }
    let outcome = scan(&ZedAdapter::new(), &db);
    assert_eq!(outcome.aggregates.len(), 1, "{:?}", outcome.diagnostics);
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "row_read_failed"
            || d.code == "blob_decode_failed"
            || d.code == "blob_json_unparseable"));
}

#[test]
fn crush_row_type_error_skips_row_not_scan() {
    use llm_usage_core::adapters::crush::CrushAdapter;
    let dir = temp_dir("crush-rowerr");
    let db = dir.join("crush.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT,
             title TEXT, cost REAL, created_at INTEGER, updated_at INTEGER)",
            [],
        )
        .unwrap();
        // cost 存非数值文本（REAL 列亲和无法转换即原样存 TEXT）：
        // f64 读取类型错误——单行跳过不中止整轮。
        conn.execute(
            "INSERT INTO sessions (id, parent_session_id, cost, created_at, updated_at)
             VALUES ('bad', NULL, 'not-a-number', 1790000000, 1790000001)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, parent_session_id, cost, created_at, updated_at)
             VALUES ('good', NULL, 0.5, 1790000000, 1790000001)",
            [],
        )
        .unwrap();
    }
    let outcome = scan(&CrushAdapter::new(), &db);
    assert_eq!(outcome.events.len(), 1, "{:?}", outcome.diagnostics);
    assert!(outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "row_read_failed"));
}

#[test]
fn crush_cumulative_cost_update_replaces_prior_observation() {
    use llm_usage_core::adapters::crush::CrushAdapter;
    use llm_usage_core::identity::{arbitrate, event_content_hash, Arbitration, ExistingMeta};
    let dir = temp_dir("crush-update");
    let db = dir.join("crush.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT,
             title TEXT, cost REAL, created_at INTEGER, updated_at INTEGER);
             INSERT INTO sessions VALUES ('root', NULL, 't', 0.5, 1790000000, 1790000001);",
        )
        .unwrap();
    }
    let adapter = CrushAdapter::new();
    let first = scan(&adapter, &db).events.remove(0);
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE sessions SET cost=0.75, updated_at=1790000002 WHERE id='root'",
            [],
        )
        .unwrap();
    }
    let second = scan(&adapter, &db).events.remove(0);
    assert_eq!(first.source_record_key, second.source_record_key);
    assert_eq!(first.source_revision, Some(1_790_000_001_000));
    assert_eq!(second.source_revision, Some(1_790_000_002_000));
    assert_eq!(second.cost.as_ref().map(|c| c.amount_minor), Some(750_000));
    let old = ExistingMeta {
        lifecycle: first.lifecycle,
        source_revision: first.source_revision,
        content_hash: event_content_hash(&first),
    };
    assert_eq!(
        arbitrate(Some(&old), &second, &event_content_hash(&second)),
        Arbitration::Replace
    );
}

#[test]
fn crush_row_cap_continues_to_later_session() {
    use llm_usage_core::adapters::crush::CrushAdapter;
    let dir = temp_dir("crush-page");
    let db = dir.join("crush.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT,
             title TEXT, cost REAL, created_at INTEGER, updated_at INTEGER);
             BEGIN;",
        )
        .unwrap();
        {
            let mut insert = conn
                .prepare("INSERT INTO sessions VALUES (?1, NULL, 't', 0.0, 1790000000, 1790000001)")
                .unwrap();
            for i in 0..=50_000 {
                insert.execute([format!("s{i:05}")]).unwrap();
            }
        }
        conn.execute_batch(
            "INSERT INTO sessions VALUES ('s50001', NULL, 't', 0.5, 1790000000, 1790000001);
             COMMIT;",
        )
        .unwrap();
    }
    let adapter = CrushAdapter::new();
    let first = scan(&adapter, &db);
    assert_eq!(first.status, ScanStatus::BudgetExhausted);
    assert!(first.events.is_empty());
    let stored = StoredScanState {
        cursor: first.cursor,
        parse_context: first.parse_context,
    };
    let second = scan_with(&adapter, &db, &stored, &ScanLimits::default());
    assert_eq!(second.status, ScanStatus::Complete);
    assert_eq!(second.events.len(), 1);
    assert_eq!(second.events[0].source_record_key, "crush:session:s50001");
}

#[test]
fn zed_row_cap_continues_to_later_thread() {
    use llm_usage_core::adapters::zed::ZedAdapter;
    let dir = temp_dir("zed-page");
    let db = dir.join("threads").join("threads.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, updated_at TEXT, data_type TEXT, data BLOB);
             BEGIN;",
        )
        .unwrap();
        {
            let mut insert = conn
                .prepare("INSERT INTO threads VALUES (?1, '2026-08-05T06:38:00Z', 'json', ?2)")
                .unwrap();
            let other = br#"{"model":{"provider":"external"}}"#;
            for i in 0..=50_000 {
                insert
                    .execute(rusqlite::params![format!("s{i:05}"), other])
                    .unwrap();
            }
            let tail =
                br#"{"model":{"provider":"zed.dev"},"cumulative_token_usage":{"input_tokens":7}}"#;
            insert.execute(rusqlite::params!["s50001", tail]).unwrap();
        }
        conn.execute_batch("COMMIT;").unwrap();
    }
    let adapter = ZedAdapter::new();
    let first = scan(&adapter, &db);
    assert_eq!(first.status, ScanStatus::BudgetExhausted);
    assert!(first.aggregates.is_empty());
    let stored = StoredScanState {
        cursor: first.cursor,
        parse_context: first.parse_context,
    };
    let second = scan_with(&adapter, &db, &stored, &ScanLimits::default());
    assert_eq!(second.status, ScanStatus::Complete);
    assert_eq!(second.aggregates.len(), 1);
    assert_eq!(second.aggregates[0].usage.input_total, Some(7));
}

#[test]
fn kiro_row_cap_continues_to_later_conversation() {
    use llm_usage_core::adapters::kiro::KiroAdapter;
    let dir = temp_dir("kiro-page");
    let db = dir.join("data.sqlite3");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations_v2 (key TEXT, conversation_id TEXT, value TEXT);
             BEGIN;",
        )
        .unwrap();
        {
            let mut insert = conn
                .prepare("INSERT INTO conversations_v2 VALUES (?1, ?1, ?2)")
                .unwrap();
            for i in 0..=50_000 {
                insert
                    .execute(rusqlite::params![format!("s{i:05}"), "{}"])
                    .unwrap();
            }
            let tail = serde_json::json!({
                "conversation_id": "s50001",
                "history": [{"user": {}, "assistant": {}, "request_metadata": {
                    "request_start_timestamp_ms": 1_790_000_010_000i64,
                    "input_tokens": 8, "output_tokens": 2,
                    "request_count": 1
                }}]
            })
            .to_string();
            insert.execute(rusqlite::params!["s50001", tail]).unwrap();
        }
        conn.execute_batch("COMMIT;").unwrap();
    }
    let adapter = KiroAdapter::new();
    let first = scan(&adapter, &db);
    assert_eq!(first.status, ScanStatus::BudgetExhausted);
    assert!(first.events.is_empty());
    let stored = StoredScanState {
        cursor: first.cursor,
        parse_context: first.parse_context,
    };
    let second = scan_with(&adapter, &db, &stored, &ScanLimits::default());
    assert_eq!(second.status, ScanStatus::Complete);
    assert_eq!(second.events.len(), 1, "{:?}", second.diagnostics);
    assert_eq!(second.events[0].usage.input_total, Some(8));
}

#[test]
fn kiro_partial_sqlite_header_remains_pending() {
    use llm_usage_core::adapters::kiro::KiroAdapter;
    let dir = temp_dir("kiro-magic");
    let path = dir.join("data.sqlite3");
    write(&path, "SQLite format");
    let adapter = KiroAdapter::new();
    assert!(matches!(
        adapter.detect(&path).unwrap(),
        DetectOutcome::Pending
    ));
    assert_eq!(scan(&adapter, &path).status, ScanStatus::Pending);
}
