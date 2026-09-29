//! Qoder 探测：会话 JSONL/state.json 指纹（识别但不解析——用量字段缺证）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

pub const QODER_FORMAT: &str = "qoder-session-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; DETECT_HEAD_BYTES];
    let n = file.read(&mut head)?;
    head.truncate(n);
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    // 宽指纹：JSON 行会话（jsonl）或 state.json 对象。识别为 Qoder 会话文件
    // 但 LatestFallback（字段未验证，扫描层 fail closed）。
    let looks_jsonl =
        text.starts_with('{') && text.contains("\"sessionId\"") || text.contains("\"session_id\"");
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
