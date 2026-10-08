//! Zoo Code detection: document fingerprints for rewritten ui_messages.json arrays.
//!
//! ui_messages.json has no version field. Fixed source f780647 taskMessages.ts
//! parses the whole file as a JSON array. Use document-level format
//! [`super::versions::ZOO_FORMAT_VERSION`], without unknown-version dispatch.
//!
//! Detection rules (V17 rejects unrecognized shapes):
//! - The first 64 KiB, after UTF-8 BOM removal, must start with a JSON array.
//! - Missing say/type fingerprint is UnknownFormat.
//! - Empty content is Pending and is probed again next round.
//! - A matching fingerprint is Supported with the registered document format;
//!   basis is KnownVersion. Scanning checks the complete documented ask/say set,
//!   rejecting undocumented kinds and skipping known nonusage messages.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const ZOO_FORMAT: &str = "zoo-ui-messages-json";

/// Detection window: first 64 KiB, without parsing the entire file.
const DETECT_HEAD_BYTES: usize = 64 * 1024;

/// Detect one task ui_messages.json file.
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
    if !trimmed.starts_with('[') {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "task file does not start with a JSON array".to_string(),
        });
    }
    // Fingerprint: say/type markers; consolidateTokenUsage/consolidateApiRequests
    // consume type="say" messages.
    let has_say = trimmed.contains("\"say\"");
    let has_type = trimmed.contains("\"type\"");
    if has_say && has_type {
        Ok(DetectOutcome::Supported {
            format: ZOO_FORMAT.to_string(),
            format_version: Some(versions::ZOO_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else if has_type {
        Ok(DetectOutcome::UnknownFormat {
            reason: "array carries typed messages but no say fingerprint in head window"
                .to_string(),
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Zoo message fingerprint (type/say)".to_string(),
        })
    }
}
