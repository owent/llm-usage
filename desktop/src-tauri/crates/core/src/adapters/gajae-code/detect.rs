//! gajae-code（gjc）探测：会话 JSONL 首行 session 头指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const GJC_FORMAT: &str = "gjc-session-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; DETECT_HEAD_BYTES];
    let n = file.read(&mut head)?;
    head.truncate(n);
    let text = String::from_utf8_lossy(&head);
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(first_line) else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON (not a gjc session)".to_string(),
        });
    };
    let is_session_header = value.get("type").and_then(|v| v.as_str()) == Some("session")
        && value.get("id").is_some()
        && value.get("timestamp").is_some();
    if is_session_header {
        let version = value.get("version").and_then(|v| v.as_i64());
        match version {
            Some(5) => Ok(DetectOutcome::Supported {
                format: GJC_FORMAT.to_string(),
                format_version: Some(versions::GJC_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            }),
            other => Ok(DetectOutcome::Supported {
                format: GJC_FORMAT.to_string(),
                format_version: other.map(|v| v.to_string()),
                basis: VersionBasis::LatestFallback,
            }),
        }
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing gjc session header fingerprint (type=session/id/timestamp)"
                .to_string(),
        })
    }
}
