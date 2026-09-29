//! Droid 探测：`<uuid>.settings.json` 的文档级指纹（tokenUsage + model）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const DROID_FORMAT: &str = "droid-settings-json";

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
