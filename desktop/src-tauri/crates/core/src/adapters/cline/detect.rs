//! Cline detection: legacy ui_messages.json array fingerprints, with separate SDK dispatch.
//!
//! Legacy ui_messages.json has no version field; its format uses the document-level
//! [`super::versions::CLINE_FORMAT_VERSION`] (fixed source dcf8c3c). This branch
//! does not dispatch unknown versions; SDK files use sdk_messages_v1::detect.
//!
//! Detection rules (V17 rejects unrecognized shapes):
//! - Read the first 64 KiB, strip UTF-8 BOM; a nonarray prefix is UnknownFormat.
//!   Unrecognized files are not parsed by guessing their format.
//! - Missing say/type fingerprint is UnknownFormat.
//! - Empty content is Pending and is probed again next round.
//! - A matching fingerprint is Supported with the registered document format;
//!   basis is KnownVersion. Scanning checks each message kind separately.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const CLINE_FORMAT: &str = "cline-ui-messages-json";

/// Detection window: first 64 KiB, without parsing the entire file.
const DETECT_HEAD_BYTES: usize = 64 * 1024;

/// Detect an SDK file separately, or a legacy task ui_messages.json file.
/// A legacy fingerprint selects the fixed document format with KnownVersion.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    if versions::sdk_messages_v1::is_sdk_file(path) {
        return versions::sdk_messages_v1::detect(path);
    }
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
    // Fingerprint: say/type markers; getApiMetrics.ts consumes type="say" messages.
    let has_say = trimmed.contains("\"say\"");
    let has_type = trimmed.contains("\"type\"");
    if has_say && has_type {
        Ok(DetectOutcome::Supported {
            format: CLINE_FORMAT.to_string(),
            format_version: Some(versions::CLINE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else if has_type {
        // Typed messages without say in this window do not establish a Cline array.
        Ok(DetectOutcome::UnknownFormat {
            reason: "array carries typed messages but no say fingerprint in head window"
                .to_string(),
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Cline message fingerprint (type/say)".to_string(),
        })
    }
}
