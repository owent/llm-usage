//! pi 探测与版本分派：有界读取首行，确认 Agent 身份（`type=session` 头）后按
//! [`super::versions`] 注册表选择格式实现。
//!
//! 约定（architecture.md#unknown-version，V30）：
//! - 首行不是 JSON / 不是 session 头 ⇒ 未知格式，fail closed，不把任意未知文件
//!   交给猜测逻辑；
//! - version 已收录（3）⇒ KnownVersion；未收录数值（如 4）⇒ LatestFallback
//!   （带兼容标记）；
//! - 固定源码已确认不兼容的 version（1、2）与缺失 version（v1/v2 时代不写该字段
//!   的 legacy 形态）⇒ UnsupportedVersion，不尝试回退。

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const PI_FORMAT: &str = "pi-session-jsonl";

/// 探测一个 pi session 文件并按注册表分派。
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
    let Ok(line) = serde_json::from_str::<serde_json::Value>(&first.text) else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        });
    };
    if line.get("type").and_then(|t| t.as_str()) != Some("session") {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record type is not session header".to_string(),
        });
    }
    // Agent 身份/输入类型已确认；version 非整数（异常形态）与缺失同按 None 处理，
    // 统一走注册表分派（v1/v2 时代不写 version 字段，legacy 形态已确认不兼容）。
    let found = line.get("version").and_then(|v| v.as_i64());
    match versions::select(found) {
        Ok(selection) => Ok(DetectOutcome::Supported {
            format: PI_FORMAT.to_string(),
            format_version: found.map(|v| v.to_string()),
            basis: selection.basis,
        }),
        Err(reason) => Ok(DetectOutcome::UnsupportedVersion {
            format: PI_FORMAT.to_string(),
            found: found.map(|v| v.to_string()),
            reason: reason.to_string(),
        }),
    }
}
