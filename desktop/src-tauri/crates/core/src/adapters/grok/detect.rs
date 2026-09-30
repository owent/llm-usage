//! Grok 探测：sessions/<workspace>/<session>/updates.jsonl 的 JSON-RPC 指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const GROK_FORMAT: &str = "grok-updates-jsonl";

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
    let has_jsonrpc = text.contains("\"jsonrpc\"");
    let has_update = text.contains("\"update\"") || text.contains("\"sessionUpdate\"");
    if has_jsonrpc && has_update {
        Ok(DetectOutcome::Supported {
            format: GROK_FORMAT.to_string(),
            format_version: Some(versions::GROK_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Grok JSON-RPC update fingerprint".to_string(),
        })
    }
}
