//! Qwen Code 探测与版本分派：有界读取首行，确认 ChatRecord 身份（`type` ∈
//! 固定源码四值枚举且 uuid/sessionId/timestamp 必填齐全）后按
//! [`super::versions`] 注册表选择格式实现。
//!
//! 约定（architecture.md#unknown-version / V17）：
//! - 首行不是 JSON / type 超出固定源码四值 / 身份字段缺失 ⇒ 未知格式，
//!   fail closed，不把任意未知文件交给猜测逻辑；
//! - 格式版本 = 固定源码 commit 锚点（已收录 ⇒ KnownVersion）；
//!   `record.version`（CLI 版本）逐条存 schema_version，不做版本白名单，
//!   不参与探测分派。
//!
//! 本目录化迁移自根级单文件 qwen.rs（M2 目录化迁移，V30），行为约定不变。

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const QWEN_FORMAT: &str = "qwen-chatrecord-jsonl";

/// 探测一个 ChatRecord 文件并按注册表分派。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let limits = super::super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: super::super::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = super::super::jsonl::read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(DetectOutcome::Pending);
    };
    let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        });
    };
    let record_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if !versions::chatrecord_085e98c0::RECORD_TYPES.contains(&record_type) {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("first record type {record_type:?} not in ChatRecord set"),
        });
    }
    // 必填身份字段（固定源码：uuid/sessionId/timestamp 全必填）。
    if line.get("uuid").and_then(|v| v.as_str()).is_none()
        || line.get("sessionId").and_then(|v| v.as_str()).is_none()
        || line.get("timestamp").and_then(|v| v.as_str()).is_none()
    {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record missing required ChatRecord identity fields".to_string(),
        });
    }
    // 格式版本 = 固定源码 commit 锚点（注册表已收录 ⇒ KnownVersion）；
    // 不读 record.version 做白名单（逐条存 schema_version 是扫描层行为）。
    let selection = versions::select(Some(versions::QWEN_FORMAT_VERSION));
    Ok(DetectOutcome::Supported {
        format: QWEN_FORMAT.to_string(),
        format_version: Some(versions::QWEN_FORMAT_VERSION.to_string()),
        basis: selection.basis,
    })
}
