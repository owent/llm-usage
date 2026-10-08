//! Detect pi and select its version: read the first line within limits, verify the
//! type=session header, then select an implementation through [super::versions].
//!
//! Rules: architecture.md#unknown-version, V30.
//! - A non-JSON/non-session first line is UnknownFormat; reject it rather than
//!   guessing how to parse arbitrary files.
//! - Registered version 3 uses KnownVersion; unregistered integers such as 4
//!   use LatestFallback with compatibility metadata.
//! - Source-verified incompatible versions 1/2 and missing versions from the
//!   legacy v1/v2 format use UnsupportedVersion without fallback.

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const PI_FORMAT: &str = "pi-session-jsonl";

/// Detect a pi session file and select through the registry.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let limits = super::super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: super::super::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = super::super::jsonl::read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(DetectOutcome::Pending);
    };
    let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        });
    };
    if line.get("type").and_then(|t| t.as_str()) != Some("session") {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record type is not session header".to_string(),
        });
    }
    // Agent/input type is verified. Noninteger and absent versions both become None;
    // registry dispatch rejects the verified incompatible legacy format that lacks version.
    let found = line.get("version").and_then(|v| v.as_i64());
    match versions::select(found) {
        Ok(selection) => Ok(DetectOutcome::Supported {
            format: PI_FORMAT.to_string(),
            format_version: found.map(|v| v.to_string()),
            basis: selection.basis,
        }),
        Err(reason) => Ok(DetectOutcome::UnsupportedVersion {
            format: PI_FORMAT.to_string(),
            found: found.map(|v| v.to_string()),
            reason: reason.to_string(),
        }),
    }
}
