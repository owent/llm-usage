//! Roo Code 探测：任务 ui_messages.json 的文档级指纹（type/say 结构）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const ROO_FORMAT: &str = "roo-ui-messages-json";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes)
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // 瞬态不可读（持锁/超时/枚举后被清理）⇒ Pending 下轮重探，不固化失败。
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(strip_bom(&head));
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    if !trimmed.starts_with('[') {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "task file does not start with a JSON array".to_string(),
        });
    }
    let has_say = trimmed.contains("\"say\"");
    let has_type = trimmed.contains("\"type\"");
    if has_say && has_type {
        Ok(DetectOutcome::Supported {
            format: ROO_FORMAT.to_string(),
            format_version: Some(versions::ROO_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Roo message fingerprint (type/say)".to_string(),
        })
    }
}
