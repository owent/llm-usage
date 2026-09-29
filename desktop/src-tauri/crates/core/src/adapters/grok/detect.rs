//! Grok 探测：sessions/<workspace>/<session>/updates.jsonl 的 JSON-RPC 指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const GROK_FORMAT: &str = "grok-updates-jsonl";

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
