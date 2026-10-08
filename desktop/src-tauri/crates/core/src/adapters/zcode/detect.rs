//! ZCode detection: bounded first-line read identifies model-io JSONL type=model_io
//! with required sessionId; then select the format through [`super::versions`].
//!
//! Rules: architecture.md#unknown-version.
//! - Non-JSON/non-model_io/missing sessionId: unknown format,
//!   reject rather than parse arbitrary unidentified files.
//! - Registered request.headers["x-zcode-app-version"]: KnownVersion;
//!   unregistered, such as future 9.9.9, or missing: marked LatestFallback.

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const ZCODE_FORMAT: &str = "zcode-modelio-jsonl";

/// Detect model-io and select the registered implementation.
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
    if line.get("type").and_then(|t| t.as_str()) != Some("model_io") {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record type is not model_io".to_string(),
        });
    }
    // Confirm model_io and sessionId; the version field may be absent.
    // Missing version uses the latest built-in parser, without immediate rejection.
    if line.get("sessionId").and_then(|v| v.as_str()).is_none() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "model_io without session identity (sessionId)".to_string(),
        });
    }
    let found = super::versions::modelio_v1::version_anchor(&line).map(str::to_string);
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: ZCODE_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}
