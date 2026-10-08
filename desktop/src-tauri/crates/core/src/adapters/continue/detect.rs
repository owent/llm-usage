//! Detect Continue sessions/&lt;uuid&gt;.json through sessionId and history.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const CONTINUE_FORMAT: &str = "continue-session-json";

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
    let has_session = text.contains("\"sessionId\"");
    let has_history = text.contains("\"history\"");
    if has_session && has_history {
        Ok(DetectOutcome::Supported {
            format: CONTINUE_FORMAT.to_string(),
            format_version: Some(versions::CONTINUE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Continue session fingerprint (sessionId/history)".to_string(),
        })
    }
}
