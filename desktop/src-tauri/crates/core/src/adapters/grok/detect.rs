//! Detect Grok sessions/<workspace>/<session>/updates.jsonl through JSON-RPC records.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const GROK_FORMAT: &str = "grok-updates-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // A transient lock, timeout or removed file returns Pending for another detection attempt.
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
