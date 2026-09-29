//! Junie CLI 探测：sessions/&lt;id&gt;/events.jsonl 的文档级指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const JUNIE_FORMAT: &str = "junie-events-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

/// 探测一个 events.jsonl：用量事件指纹（LlmResponseMetadataEvent/modelUsage）。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; DETECT_HEAD_BYTES];
    let n = file.read(&mut head)?;
    head.truncate(n);
    if head.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let text = String::from_utf8_lossy(&head);
    let has_usage = text.contains("LlmResponseMetadataEvent") || text.contains("\"modelUsage\"");
    let has_event = text.contains("\"agentEvent\"") || text.contains("\"kind\"");
    if has_usage && has_event {
        Ok(DetectOutcome::Supported {
            format: JUNIE_FORMAT.to_string(),
            format_version: Some(versions::JUNIE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else if has_event {
        Ok(DetectOutcome::UnknownFormat {
            reason: "typed agent event log without usage fingerprint in head window".to_string(),
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Junie agent-event fingerprint".to_string(),
        })
    }
}
