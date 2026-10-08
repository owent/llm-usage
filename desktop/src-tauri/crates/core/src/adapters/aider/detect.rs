//! Detect the documented --analytics-log JSONL format for Aider.
//!
//! V17: first-line JSON without event/properties/time is unknown format;
//! empty files are Pending. No version field; the documented format selects KnownVersion.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const AIDER_FORMAT: &str = "aider-analytics-jsonl";

/// Inspect at most 64 KiB from the header; check structural fields of possibly long first lines.
const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Temporary lock/timeout/removal after enumeration is Pending; retry next run.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let Ok(value) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(first_line)
    else {
        // A full window without newline may truncate the first line and invalidate partial JSON.
        // Keep Pending and retry a complete line, rather than permanently classifying the format.
        // A short window read the whole file; a failed single-line parse is truly unknown format.
        if head.len() == DETECT_HEAD_BYTES && !text.contains('\n') {
            return Ok(DetectOutcome::Pending);
        }
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not a JSON object (not an aider analytics log)".to_string(),
        });
    };
    let looks_like = value.is_object()
        && value.get("event").is_some_and(|v| v.is_string())
        && value.get("properties").is_some_and(|v| v.is_object())
        && value.get("time").is_some_and(|v| v.is_number());
    if looks_like {
        Ok(DetectOutcome::Supported {
            format: AIDER_FORMAT.to_string(),
            format_version: Some(versions::AIDER_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "JSON line lacks the event/properties/time fingerprint".to_string(),
        })
    }
}
