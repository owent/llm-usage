//! Codex 探测与版本分派：有界读取首行，确认 Agent 身份（`type=session_meta`
//! rollout JSONL）后按 [`super::versions`] 注册表选择格式实现。
//!
//! 合同（architecture.md#unknown-version）：
//! - 首行不是 JSON / 不是 session_meta / payload 缺 id 与 cli_version ⇒ 未知格式，
//!   fail closed，不把任意未知文件交给猜测逻辑；
//! - cli_version 已收录 ⇒ KnownVersion；未收录或缺失 ⇒ LatestFallback（带兼容标记）。

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const CODEX_FORMAT: &str = "codex-rollout-jsonl";

/// 探测一个 rollout 文件并按注册表分派。
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
    if line.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record type is not session_meta".to_string(),
        });
    }
    let payload = line
        .get("payload")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    // Agent 身份/输入类型确认：session_meta 须携带会话 id；cli_version 可缺失
    //（缺失 ⇒ 版本未知，默认回退最新内置解析器，不直接拒绝）。
    if payload.get("id").and_then(|v| v.as_str()).is_none()
        && payload.get("session_id").and_then(|v| v.as_str()).is_none()
    {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "session_meta without session id".to_string(),
        });
    }
    let found = payload
        .get("cli_version")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: CODEX_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}
