//! Detect documented Droid `<uuid>.settings.json` fields: tokenUsage and model.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const DROID_FORMAT: &str = "droid-settings-json";

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
    let has_usage = text.contains("\"tokenUsage\"");
    let has_model = text.contains("\"model\"") || text.contains("\"providerLock\"");
    if has_usage && has_model {
        Ok(DetectOutcome::Supported {
            format: DROID_FORMAT.to_string(),
            format_version: Some(versions::DROID_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing tokenUsage/model fingerprint".to_string(),
        })
    }
}
