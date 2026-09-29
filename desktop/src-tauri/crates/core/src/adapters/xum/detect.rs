//! Xum 探测：session-usage.json 的文档级指纹（byModel + lastRequest/version）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const XUM_FORMAT: &str = "xum-session-usage-json";

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
    let has_by_model = text.contains("\"byModel\"");
    let has_anchor = text.contains("\"lastRequest\"") || text.contains("\"version\"");
    if has_by_model && has_anchor {
        Ok(DetectOutcome::Supported {
            format: XUM_FORMAT.to_string(),
            format_version: Some(versions::XUM_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing byModel/lastRequest fingerprint".to_string(),
        })
    }
}
