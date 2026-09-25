//! 集成测试公共辅助：真实临时 SQLite 文件库与事件构造器。
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

/// 打开真实临时文件库（WAL）。
pub fn temp_storage(tag: &str) -> (TempDir, Storage) {
    let dir = TempDir::new(tag);
    let storage = Storage::open(&dir.db_path()).unwrap();
    (dir, storage)
}

/// ISO 8601 字符串 → UTC 毫秒。
pub fn ts(s: &str) -> i64 {
    s.parse::<jiff::Timestamp>().unwrap().as_millisecond()
}

/// 构造一个默认事件：provider "prov"、model "m"、final、verified、model_call。
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

/// 给事件设置统一 token 值（input_total/output_total/total_tokens reported）。
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

// ---- M2-A：Codex 适配器测试辅助 ----

use llm_usage_core::adapters::codex::CodexAdapter;
use llm_usage_core::adapters::framework::{
    run_adapter_scan, DiscoverContext, RunConfig, ScanLimits, SourceRunReport,
};
use llm_usage_core::jobs::TriggerKind;

/// 把 M0 脱敏 fixture（sanitized projection）还原为 rollout JSONL 字节流。
/// 投影保留了每行的 timestamp/type/payload 白名单结构，正文为常量占位；
/// 去掉提取器附加的 `line` 键后逐行序列化即得原始形状的 JSONL。
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

/// 在临时目录构造 <root>/sessions/2026/09/24/rollout-reconstructed.jsonl 布局。
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

/// M0 真实 fixture（本机 0.155.0-alpha.16.3 脱敏提取）在仓库内的路径。
pub fn codex_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codex")
        .join(name)
}

/// 运行一次完整采集（发现→探测→扫描→commit_batch→查询就绪）。
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

/// UTC 日汇总查询（测试期望均为人工核算值）。
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

// ---- M2-C：Claude Code / Qwen Code / Gemini CLI 适配器测试辅助 ----

use llm_usage_core::adapters::claude::ClaudeAdapter;
use llm_usage_core::adapters::framework::SourceAdapter;
use llm_usage_core::adapters::gemini::GeminiAdapter;
use llm_usage_core::adapters::qwen::QwenAdapter;

/// 在临时目录构造 <root>/projects/<rel> 布局（rel 如 "proj/sess-1.jsonl" 或
/// "proj/sess-1/subagents/a.jsonl"），返回配置根。
pub fn claude_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("projects").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// 在临时目录构造 <root>/tmp/<rel> 布局（rel 如 "proj-1/chats/sess-1.jsonl"），返回配置根。
pub fn qwen_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("tmp").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// 在临时目录构造 <root>/tmp/<rel> 布局（rel 如 "hash-1/chats/session-1.json"），返回配置根。
pub fn gemini_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("tmp").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// 合成 fixture 在仓库内的路径（目录/文件头均标 synthetic；三者本机无真实样本）。
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

// ---- M2-B/C 恢复：pi / oh-my-pi 适配器测试辅助 ----

use llm_usage_core::adapters::omp::OmpAdapter;
use llm_usage_core::adapters::pi::PiAdapter;

/// 通用：把脱敏投影（{records:[{line, ...条目}]}）还原为 JSONL 字节流。
/// 与 reconstruct_codex_jsonl 同逻辑，命名不绑定具体 Agent。
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

/// 在临时目录构造 <root>/sessions/<rel> 布局（rel 如 "--C--Users-anon--/2026-...jsonl"），
/// 返回配置根（手工根语义：含 sessions 子目录按 agent 根解析）。
pub fn pi_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.path().join("sessions").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    dir.path().to_path_buf()
}

/// omp 同 pi 布局；rel 可为深层（子 Agent 文件：
/// "--CWD--/<ts>_<父UUID>/SubAgent.jsonl" 或更深的嵌套子目录）。
pub fn omp_root_with_file(dir: &TempDir, rel: &str, contents: &[u8]) -> PathBuf {
    pi_root_with_file(dir, rel, contents)
}

/// pi 真实/合成 fixture 在仓库内的路径。
pub fn pi_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pi")
        .join(name)
}

/// omp 真实/合成 fixture 在仓库内的路径。
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
