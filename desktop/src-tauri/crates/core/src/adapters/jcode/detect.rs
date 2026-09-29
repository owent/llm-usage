//! jcode 探测：session_*.json 快照指纹（id/messages/token_usage）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const JCODE_FORMAT: &str = "jcode-session-json";

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
    let has_messages = text.contains("\"messages\"");
    let has_usage = text.contains("\"token_usage\"");
    let has_id = text.contains("\"id\"");
    if has_messages && has_id {
        let _ = has_usage;
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
