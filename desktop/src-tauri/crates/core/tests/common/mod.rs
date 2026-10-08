//! Integration-test helpers for temporary file-backed SQLite databases and synthetic events.
#![allow(dead_code)]

use llm_usage_core::domain::*;
use llm_usage_core::ingest::IngestBatch;
use llm_usage_core::storage::Storage;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "llm-usage-core-test-{}-{}-{n}",
            std::process::id(),
            tag
        ));
        std::fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn db_path(&self) -> PathBuf {
        self.path.join("test.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Open temporary file-backed storage with WAL.
pub fn temp_storage(tag: &str) -> (TempDir, Storage) {
    let dir = TempDir::new(tag);
    let storage = Storage::open(&dir.db_path()).unwrap();
    (dir, storage)
}

/// Parse ISO 8601 into UTC milliseconds.
pub fn ts(s: &str) -> i64 {
    s.parse::<jiff::Timestamp>().unwrap().as_millisecond()
}

/// Synthetic default event: provider prov, model m, final/verified model_call.
pub fn evt(instance: &str, key: &str, occurred_ms: i64) -> EventInput {
    EventInput {
        source_instance_id: instance.to_string(),
        source_record_key: key.to_string(),
        record_kind: RecordKind::ModelCall,
        schema_version: "1".to_string(),
        parser_version: "1".to_string(),
        parse_basis: None,
        origin_call_id: None,
        attempt_id: None,
        session_id: None,
        parent_session_id: None,
        host_application: None,
        agent: "agent-a".to_string(),
        call_category: CallCategory::Primary,
        occurred_at_ms: occurred_ms,
        observed_at_ms: None,
        source_time: None,
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: Some("prov".to_string()),
        model_raw: Some("m".to_string()),
        model_canonical: None,
        model_attribution: ModelAttribution::RequestField,
        usage: TokenUsage::default(),
        quality: TokenQuality::default(),
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }
}

/// Assign synthetic reported input/output/total values to a test event.
pub fn with_tokens(mut e: EventInput, input: i64, output: i64) -> EventInput {
    e.usage.input_total = Some(input);
    e.usage.output_total = Some(output);
    e.usage.total_tokens = Some(input + output);
    e.quality.input_total = FieldQuality::Reported;
    e.quality.output_total = FieldQuality::Reported;
    e.quality.total_tokens = FieldQuality::Reported;
    e
}

pub fn batch(instance: &str, tz: &str, now_ms: i64, events: Vec<EventInput>) -> IngestBatch {
    IngestBatch {
        batch_id: format!("b-{instance}-{now_ms}"),
        instance_id: instance.to_string(),
        timezone: tz.to_string(),
        now_ms,
        events,
        checkpoints: Vec::new(),
        diagnostics: Vec::new(),
        run_id: None,
        retention_cutoff_ms: None,
    }
}

// M2-A Codex adapter helpers.

use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceRunReport,
};
use llm_usage_core::jobs::TriggerKind;

/// Reconstruct rollout JSONL from the M0 redacted test dataset.
/// Preserve selected timestamp/type/payload fields; message bodies remain fixed placeholders.
/// Remove extractor line metadata and serialize each retained object as JSONL.
pub fn reconstruct_codex_jsonl(sanitized_path: &Path) -> Vec<u8> {
    let text = std::fs::read_to_string(sanitized_path).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut out = Vec::new();
    for record in fixture["records"].as_array().unwrap() {
        let mut line = record.clone();
        line.as_object_mut().unwrap().remove("line");
        out.extend_from_slice(serde_json::to_string(&line).unwrap().as_bytes());
        out.push(b'\n');
    }
    out
}

/// Build temporary sessions/2026/09/24/rollout-reconstructed.jsonl layout.
pub fn codex_root_with_file(dir: &TempDir, file_name: &str, contents: &[u8]) -> PathBuf {
    let day_dir = dir
        .path()
        .join("sessions")
        .join("2026")
        .join("09")
        .join("24");
    std::fs::create_dir_all(&day_dir).unwrap();
    std::fs::write(day_dir.join(file_name), contents).unwrap();
    dir.path().to_path_buf()
}

/// Repository path to M0 redacted native 0.155.0-alpha.16.3 samples.
pub fn codex_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codex")
        .join(name)
}

/// Run discovery, detection, scanning, and commit so the results are ready for queries.
pub fn run_codex(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_codex_with_limits(storage, root, now_ms, ScanLimits::default())
}

pub fn run_codex_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    let adapter = CodexAdapter::new();
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.to_path_buf()],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits,
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("run-{now_ms}"),
    };
    let reports = run_adapter_scan(storage, &adapter, &ctx, &config).unwrap();
    assert_eq!(reports.len(), 1, "expected exactly one discovered root");
    reports
}

/// UTC daily query helper for independently calculated expectations.
pub fn summary(
    storage: &Storage,
    first_day: &str,
    last_day: &str,
) -> llm_usage_core::query::Summary {
    llm_usage_core::query::query_summary(
        storage,
        &llm_usage_core::query::SummaryRequest {
            timezone: "UTC".to_string(),
            week_start: llm_usage_core::calendar::WeekStart::Monday,
            first_day: llm_usage_core::calendar::parse_date(first_day).unwrap(),
            last_day: llm_usage_core::calendar::parse_date(last_day).unwrap(),
            granularity: llm_usage_core::query::Granularity::Day,
            filters: llm_usage_core::query::Filters::default(),
            today: llm_usage_core::calendar::parse_date(last_day).unwrap(),
            retention_cutoff: None,
        },
    )
    .unwrap()
}

// M2-C Claude Code, Qwen Code, and Gemini CLI helpers.

use llm_usage_core::adapters::claude::ClaudeAdapter;
use llm_usage_core::adapters::framework::SourceAdapter;
use llm_usage_core::adapters::gemini::GeminiAdapter;
use llm_usage_core::adapters::qwen::QwenAdapter;

/// Build projects/<rel>, such as proj/sess-1.jsonl or
/// proj/sess-1/subagents/a.jsonl, and return the temporary configuration root.
pub fn claude_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("projects").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// Build tmp/<rel> Qwen layout, such as proj-1/chats/sess-1.jsonl.
pub fn qwen_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("tmp").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// Build tmp/<rel> Gemini layout, such as hash-1/chats/session-1.json.
pub fn gemini_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("tmp").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// Repository path for synthetic Claude datasets; native datasets have separate validation records.
pub fn claude_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("claude")
        .join(name)
}

pub fn qwen_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("qwen")
        .join(name)
}

pub fn gemini_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("gemini")
        .join(name)
}

fn run_adapter(
    adapter: &dyn SourceAdapter,
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    let ctx = DiscoverContext {
        home_dir: None,
        env: Default::default(),
        manual_roots: vec![root.to_path_buf()],
    };
    let config = RunConfig {
        timezone: "UTC".to_string(),
        now_ms,
        limits,
        trigger: TriggerKind::Manual,
        origin_host_id: None,
        run_id_prefix: format!("run-{now_ms}"),
    };
    let reports = run_adapter_scan(storage, adapter, &ctx, &config).unwrap();
    assert_eq!(reports.len(), 1, "expected exactly one discovered root");
    reports
}

pub fn run_claude(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &ClaudeAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_claude_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&ClaudeAdapter::new(), storage, root, now_ms, limits)
}

pub fn run_qwen(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &QwenAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_qwen_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&QwenAdapter::new(), storage, root, now_ms, limits)
}

pub fn run_gemini(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &GeminiAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

// M2-B/C pi and oh-my-pi helpers.

use llm_usage_core::adapters::omp::OmpAdapter;
use llm_usage_core::adapters::pi::PiAdapter;

/// Reconstruct JSONL from redacted records carrying extractor line metadata.
/// Share Codex reconstruction rules without tying this helper to one Agent.
pub fn reconstruct_jsonl_projection(sanitized_path: &Path) -> Vec<u8> {
    let text = std::fs::read_to_string(sanitized_path).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut out = Vec::new();
    for record in fixture["records"].as_array().unwrap() {
        let mut line = record.clone();
        line.as_object_mut().unwrap().remove("line");
        out.extend_from_slice(serde_json::to_string(&line).unwrap().as_bytes());
        out.push(b'\n');
    }
    out
}

/// Build sessions/<rel>, such as --C--Users-anon--/2026-...jsonl,
/// returning an Agent root containing its sessions child for manual discovery.
pub fn pi_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("sessions").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// omp shares the layout and can use nested subagent paths
/// like --CWD--/<ts>_<parentUUID>/SubAgent.jsonl or deeper named directories.
pub fn omp_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    pi_root_with_file(dir, rel, contents)
}

/// Repository path to pi native or synthetic datasets.
pub fn pi_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pi")
        .join(name)
}

/// Repository path to omp native or synthetic datasets.
pub fn omp_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("omp")
        .join(name)
}

pub fn run_pi(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &PiAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_pi_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&PiAdapter::new(), storage, root, now_ms, limits)
}

pub fn run_omp(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &OmpAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_omp_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&OmpAdapter::new(), storage, root, now_ms, limits)
}

// M4 ZCode adapter helpers.

use llm_usage_core::adapters::zcode::ZcodeAdapter;

/// Build rollout/<rel>, such as model-io-sess-1.jsonl,
/// returning the CLI root containing rollout for manual discovery.
pub fn zcode_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("rollout").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// ZCode dataset paths: real-* contains redacted native records;
/// synthetic-* contains edge-case data with manually calculated _expectations.md.
pub fn zcode_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("zcode")
        .join(name)
}

pub fn run_zcode(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &ZcodeAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_zcode_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&ZcodeAdapter::new(), storage, root, now_ms, limits)
}

// M3 Kilo Code CLI helpers.

use llm_usage_core::adapters::kilo::KiloAdapter;
use rusqlite::Connection;

/// Repository path to Kilo native or synthetic datasets.
pub fn kilo_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("kilo")
        .join(name)
}

/// Convert JSON to SQLite values, preserving integer/float types and serializing nested values,
/// including irregular source values represented by JSON strings.
fn json_to_sql(value: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as Sql;
    match value {
        serde_json::Value::Null => Sql::Null,
        serde_json::Value::Bool(b) => Sql::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Sql::Integer)
            .or_else(|| n.as_f64().map(Sql::Real))
            .unwrap_or(Sql::Null),
        serde_json::Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

/// Rebuild message/session DDL and redacted records in
/// <dir>/.local/share/kilo/kilo.db using a real SQLite database engine.
/// Return the home-shaped directory for manual discovery.
/// Input can be a parsed redacted dataset or inline synthetic JSON.
pub fn build_kilo_db(dir: &TempDir, projection: &serde_json::Value) -> PathBuf {
    let kilo_home = dir.path().join(".local").join("share").join("kilo");
    std::fs::create_dir_all(&kilo_home).unwrap();
    let db_path = kilo_home.join("kilo.db");
    let conn = Connection::open(&db_path).unwrap();
    // The test rebuilds only message/session, while bundled SQLite enables foreign keys.
    // Disable them here because project references are not rebuilt; do not change the source database.
    conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    conn.execute_batch(projection["schema"]["message_ddl"].as_str().unwrap())
        .unwrap();
    conn.execute_batch(projection["schema"]["session_ddl"].as_str().unwrap())
        .unwrap();
    for session in projection["sessions"].as_array().unwrap() {
        let obj = session.as_object().unwrap();
        let columns: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO session ({}) VALUES ({})",
            columns.join(", "),
            placeholders.join(", ")
        );
        // Redacted session.time_created can be null or an irregular JSON string;
        // use zero solely for a NOT NULL test column that the adapter never reads.
        let values: Vec<rusqlite::types::Value> = obj
            .iter()
            .map(|(k, v)| match (k.as_str(), v) {
                ("time_created", serde_json::Value::Null) => rusqlite::types::Value::Integer(0),
                (_, v) => json_to_sql(v),
            })
            .collect();
        conn.execute(
            &sql,
            rusqlite::params_from_iter(values.iter().map(|v| v as &dyn rusqlite::ToSql)),
        )
        .unwrap();
    }
    for message in projection["messages"].as_array().unwrap() {
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                message["id"].as_str().unwrap(),
                message["session_id"].as_str().unwrap(),
                message["time_created"].as_i64().unwrap(),
                message["time_updated"].as_i64().unwrap(),
                serde_json::to_string(&message["data"]).unwrap(),
            ],
        )
        .unwrap();
    }
    drop(conn);
    dir.path().to_path_buf()
}

/// Rebuild temporary kilo.db from a redacted repository dataset.
pub fn build_kilo_db_from_fixture(dir: &TempDir, sanitized_name: &str) -> PathBuf {
    let text = std::fs::read_to_string(kilo_fixture(sanitized_name)).unwrap();
    let projection: serde_json::Value = serde_json::from_str(&text).unwrap();
    build_kilo_db(dir, &projection)
}

/// Original message DDL from redacted native data, reused for synthetic databases.
pub const KILO_MESSAGE_DDL: &str = "CREATE TABLE `message` ( `id` text PRIMARY KEY, \
`session_id` text NOT NULL, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, \
`data` text NOT NULL, CONSTRAINT `fk_message_session_id_session_id_fk` FOREIGN KEY \
(`session_id`) REFERENCES `session`(`id`) ON DELETE CASCADE )";
pub const KILO_SESSION_DDL: &str = "CREATE TABLE `session` ( `id` text PRIMARY KEY, \
`project_id` text NOT NULL, `parent_id` text, `slug` text NOT NULL, `directory` text NOT NULL, \
`title` text NOT NULL, `version` text NOT NULL, `share_url` text, `summary_additions` integer, \
`summary_deletions` integer, `summary_files` integer, `summary_diffs` text, `revert` text, \
`permission` text, `time_created` integer NOT NULL, `time_updated` integer NOT NULL, \
`time_compacting` integer, `time_archived` integer, `workspace_id` text, `path` text, \
`agent` text, `model` text, `cost` real DEFAULT 0 NOT NULL, `tokens_input` integer DEFAULT 0 \
NOT NULL, `tokens_output` integer DEFAULT 0 NOT NULL, `tokens_reasoning` integer DEFAULT 0 \
NOT NULL, `tokens_cache_read` integer DEFAULT 0 NOT NULL, `tokens_cache_write` integer \
DEFAULT 0 NOT NULL, `metadata` text, CONSTRAINT `fk_session_project_id_project_id_fk` \
FOREIGN KEY (`project_id`) REFERENCES `project`(`id`) ON DELETE CASCADE )";

/// Build synthetic sessions/messages with the same DDL as native test data.
pub fn synthetic_kilo_projection(
    sessions: serde_json::Value,
    messages: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "synthetic": true,
        "schema": {
            "message_ddl": KILO_MESSAGE_DDL,
            "session_ddl": KILO_SESSION_DDL,
        },
        "sessions": sessions,
        "messages": messages,
    })
}

pub fn run_kilo(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &KiloAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

/// Detect one specified kilo.db directly, without discovery.
pub fn kilo_detect(db_path: &Path) -> llm_usage_core::adapters::framework::DetectOutcome {
    use llm_usage_core::adapters::framework::SourceAdapter;
    KiloAdapter::new().detect(db_path).unwrap()
}

// M4 Kimi Code and Kimi Work helpers.

use llm_usage_core::adapters::kimi_code::KimiCodeAdapter;
use llm_usage_core::adapters::kimi_work::KimiWorkAdapter;

/// Build sessions/<wd>/<session>/agents/<agent>/wire.jsonl under a temporary root.
/// Example Code rel: wd_syn/session_syn-1/agents/main/wire.jsonl;
/// Work rel: wd_syn/conv_syn-1/agents/main/wire.jsonl; return the configuration root.
pub fn kimi_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("sessions").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// Reconstruct wire JSONL from M4 redacted records and line metadata.
/// Preserve extracted row order even when original line numbers are not consecutive.
pub fn reconstruct_kimi_wire(sanitized_path: &Path) -> Vec<u8> {
    reconstruct_jsonl_projection(sanitized_path)
}

/// Repository path to Kimi Code native or synthetic datasets.
pub fn kimi_code_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("kimi-code")
        .join(name)
}

/// Repository path to Kimi Work native or synthetic datasets.
pub fn kimi_work_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("kimi-work")
        .join(name)
}

pub fn run_kimi_code(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &KimiCodeAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_kimi_code_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&KimiCodeAdapter::new(), storage, root, now_ms, limits)
}

pub fn run_kimi_work(storage: &Storage, root: &Path, now_ms: i64) -> Vec<SourceRunReport> {
    run_adapter(
        &KimiWorkAdapter::new(),
        storage,
        root,
        now_ms,
        ScanLimits::default(),
    )
}

pub fn run_kimi_work_with_limits(
    storage: &Storage,
    root: &Path,
    now_ms: i64,
    limits: ScanLimits,
) -> Vec<SourceRunReport> {
    run_adapter(&KimiWorkAdapter::new(), storage, root, now_ms, limits)
}
