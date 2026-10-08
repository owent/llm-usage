//! Gemini detection: rewritten session JSON fingerprints (architecture.md#adapter-layout).
//!
//! Gemini session JSON has no version field. Use document-level format
//! [`super::versions::GEMINI_FORMAT_VERSION`]; there is no version dispatch
//! or unknown-version fallback as in the Codex registry.
//!
//! Detection rules (V17 rejects unrecognized shapes):
//! - Read the first 64 KiB, strip UTF-8 BOM; a nonobject prefix is UnknownFormat.
//!   Unrecognized files are not parsed by guessing their format.
//! - Missing sessionId/messages fingerprint is UnknownFormat.
//! - sessionId alone may indicate an initial write: Pending, then probe next round.
//! - A matching fingerprint is Supported with the registered document format;
//!   basis is KnownVersion.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const GEMINI_FORMAT: &str = "gemini-session-json";

/// Detection window: first 64 KiB, without parsing the entire file.
const DETECT_HEAD_BYTES: usize = 64 * 1024;

/// Detect one session JSON file.
/// A matching fingerprint selects the fixed document format with KnownVersion.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Transient read failure (lock/timeout/disappearance) is Pending for the next probe.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(super::strip_bom(&head));
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    if !trimmed.starts_with('{') {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "session file does not start with a JSON object".to_string(),
        });
    }
    let has_session_id = trimmed.contains("\"sessionId\"");
    let has_messages = trimmed.contains("\"messages\"");
    if has_session_id && has_messages {
        // Without a native version field, select the registered document format session-doc-1
        // with KnownVersion; this format has no unknown-version state.
        Ok(DetectOutcome::Supported {
            format: GEMINI_FORMAT.to_string(),
            format_version: Some(versions::GEMINI_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else if has_session_id {
        // sessionId alone may indicate an incomplete initial write; probe again next round.
        Ok(DetectOutcome::Pending)
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing sessionId/messages fingerprint".to_string(),
        })
    }
}
