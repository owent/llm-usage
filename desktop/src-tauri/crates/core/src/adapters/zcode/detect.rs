//! ZCode 探测与版本分派：有界读取首行，确认 Agent 身份（`type=model_io` 的
//! model-io JSONL、sessionId 必填）后按 [`super::versions`] 注册表选择格式实现。
//!
//! 合同（architecture.md#unknown-version）：
//! - 首行不是 JSON / 不是 model_io / 缺 sessionId 身份字段 ⇒ 未知格式，
//!   fail closed，不把任意未知文件交给猜测逻辑；
//! - 版本锚点 `request.headers["x-zcode-app-version"]` 已收录 ⇒ KnownVersion；
//!   未收录（如未来 9.9.9）或缺失 ⇒ LatestFallback（带兼容标记）。

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const ZCODE_FORMAT: &str = "zcode-modelio-jsonl";

/// 探测一个 model-io 文件并按注册表分派。
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
    if line.get("type").and_then(|t| t.as_str()) != Some("model_io") {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record type is not model_io".to_string(),
        });
    }
    // Agent 身份/输入类型确认：model_io 须携带 sessionId；版本锚点可缺失
    //（缺失 ⇒ 版本未知，默认回退最新内置解析器，不直接拒绝）。
    if line.get("sessionId").and_then(|v| v.as_str()).is_none() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "model_io without session identity (sessionId)".to_string(),
        });
    }
    let found = super::versions::modelio_v1::version_anchor(&line).map(str::to_string);
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: ZCODE_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}
