//! jcode 探测：session_*.json 快照指纹（id/messages/token_usage）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const JCODE_FORMAT: &str = "jcode-session-json";

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
    let has_messages = text.contains("\"messages\"");
    let has_id = text.contains("\"id\"");
    if has_messages && has_id {
        Ok(DetectOutcome::Supported {
            format: JCODE_FORMAT.to_string(),
            format_version: Some(versions::JCODE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing jcode session fingerprint (id/messages)".to_string(),
        })
    }
}
