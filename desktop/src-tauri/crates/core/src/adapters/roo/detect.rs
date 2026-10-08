//! Detect Roo Code task ui_messages.json using documented type/say fields.

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
    // Temporary lock, timeout or deletion after enumeration: return Pending and retry next scan.
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
