//! Qoder 探测：会话 JSONL/state.json 指纹（识别但不解析——用量字段尚未核验）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

pub const QODER_FORMAT: &str = "qoder-session-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // 瞬态不可读（持锁/超时/枚举后被清理）⇒ Pending 下轮重探，不固化失败。
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    // 宽指纹：JSON 行会话（jsonl）或 state.json 对象。识别为 Qoder 会话文件
    // 但 LatestFallback（字段未验证，扫描层 fail closed）。
    // 注意优先级：session_id 子串命中也必须以 '{' 开头为前提，否则任何
    // 含 "session_id" 的异源文件都会被误吞。
    let looks_jsonl = text.starts_with('{')
        && (text.contains("\"sessionId\"") || text.contains("\"session_id\""));
    let looks_state =
        text.contains("\"modelRequests\"") || text.contains("\"compact_token_usage_json\"");
    if looks_jsonl || looks_state {
        Ok(DetectOutcome::Supported {
            format: QODER_FORMAT.to_string(),
            format_version: None,
            basis: VersionBasis::LatestFallback,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Qoder session fingerprint".to_string(),
        })
    }
}
