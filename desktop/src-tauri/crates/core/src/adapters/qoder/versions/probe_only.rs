//! Qoder CLI 会话载体探针实现（`probe_only`，qoder-pending-evidence）。
//!
//! 格式核验结论（2026-09-29 实现前格式检查：官方文档 + npm @qoder-ai/qodercli 1.1.64
//! 解包 + 第三方脚本）：
//! - 路径已证：`~/.qoder/projects/<processed-project-path-name>/<session-id>.jsonl`
//!   （对话日志）与 `<session-id>/state.json`（会话状态）；`QODER_CONFIG_DIR`
//!   重定向根。
//! - **用量落盘字段仍待真实样本核验**：bundle 混淆代码含 state.json 的
//!   `modelRequests[]`/`compact_token_usage_json` schema 字符串与 OTel→存储列
//!   映射（input_tokens/cache_read_tokens 等），但**未证实实际持久化**；
//!   `/usage` 计量是云端 Credits（docs.qoder.com/cli/usage.md）。
//! - 按"禁止猜测"约定：本实现只发现与识别会话文件，**不解析任何用量字段**
//!   （fail closed，诊断说明待核验项）；取得本机 fixture 证实 state.json/
//!   会话 jsonl 实际字段后再实现解析。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

pub const QODER_PARSER_VERSION: &str = "qoder-probe-1";

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ProbeCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // fail closed：不解析、不入账；诊断说明尚未核验的字段及样本获取方式。
    // 游标取文件长度：内容未变化时走 framework 的 unchanged 短路，
    // 不每轮重复推同一条诊断（文件增长由 generation 裁决仍会重探）。
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(ProbeCursor {
            generation: target.generation,
            offset: target.probe.len,
        })?),
        parse_context: None,
        events: Vec::new(),
        aggregates: Vec::new(),
        diagnostics: vec![DiagnosticInput {
            event_id: None,
            code: "usage_fields_unverified".to_string(),
            field: None,
            position: Some(crate::adapters::framework::normalize_path(&target.path)),
            message: "Qoder usage persistence is unverified (bundle schema hints only; \
billing is cloud Credits); parsing disabled pending a local fixture of \
<session-id>.jsonl and <session-id>/state.json"
                .to_string(),
        }],
        lines_read: 0,
        records_seen: 0,
        reconciliations: Vec::new(),
        health: "degraded".to_string(),
    })
}
