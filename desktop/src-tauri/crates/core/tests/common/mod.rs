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
