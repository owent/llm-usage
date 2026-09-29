//! Amp 探测：threads/T-*.json 的文档级指纹（messages + usageLedger/usage）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const AMP_FORMAT: &str = "amp-threads-json";

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
    let has_usage = text.contains("\"usageLedger\"") || text.contains("\"usage\"");
    let has_thread_shape = text.contains("\"created\"");
    if has_messages && has_usage && has_thread_shape {
        Ok(DetectOutcome::Supported {
            format: AMP_FORMAT.to_string(),
            format_version: Some(versions::AMP_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Amp thread fingerprint (messages/usage/created)".to_string(),
        })
    }
}
