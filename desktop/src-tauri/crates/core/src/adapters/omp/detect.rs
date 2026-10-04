//! omp 探测与版本分派：有界读取前 4 行，确认 Agent 身份（首行 `type:"title"`
//! 或 `type:"session"`，session 头在前 4 行内定位）后按 [`super::versions`]
//! 注册表选择格式实现。
//!
//! 约定（architecture.md#unknown-version，首行闸口四态语义不变）：
//! - 首行不是 JSON / 首行类型既非 title 也非 session ⇒ 未知格式，fail closed；
//! - 有 title 但前 4 行内无 session 头 ⇒ Pending（可能仍在首次写入中）；
//! - session 头 version 已收录 ⇒ KnownVersion；未收录或缺失 ⇒ LatestFallback
//!   （带兼容标记，先尝试最新内置解析器，不直接拒绝）。

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;
use crate::adapters::jsonl::{read_jsonl, JsonlLimits, DEFAULT_MAX_LINE_BYTES};

use super::versions;

pub const OMP_FORMAT: &str = "omp-session-jsonl";

/// 探测一个 omp 会话文件并按注册表分派。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // omp 文件首行为 title（v=1），session 头在其后；有界读前 4 行定位。
    let limits = JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(4),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(DetectOutcome::Pending);
    };
    let Ok(first_line) =
        crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        });
    };
    let first_type = first_line
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if first_type != "session" && first_type != "title" {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("first record type {first_type:?} is neither title nor session"),
        });
    }
    for raw in &outcome.lines {
        let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&raw.text)
        else {
            continue;
        };
        if line.get("type").and_then(|t| t.as_str()) != Some("session") {
            continue;
        }
        // Agent 身份/输入类型已确认，按注册表分派；未收录/缺失版本回退最新
        // 内置解析器并带兼容标记（V30；omp 旧版格式尚未核验，不直接拒绝）。
        let found = line.get("version").and_then(|v| v.as_i64());
        let selection = versions::select(found);
        return Ok(DetectOutcome::Supported {
            format: OMP_FORMAT.to_string(),
            format_version: found.map(|v| v.to_string()),
            basis: selection.basis,
        });
    }
    // 有 title 但前 4 行内无 session 头：可能仍在首次写入中，下轮重探。
    Ok(DetectOutcome::Pending)
}
