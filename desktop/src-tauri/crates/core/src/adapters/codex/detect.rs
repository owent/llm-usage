//! Bounded Codex first-line detection: identify session_meta rollout JSONL,
//! then select implementation through the super::versions registry.
//!
//! Rules: architecture.md#unknown-version.
//! - Non-JSON, non-session_meta or missing both payload.id/session_id is unknown format;
//!   reject rather than guess how to parse an arbitrary file.
//! - Registered cli_version selects KnownVersion; absent/unregistered uses marked LatestFallback.

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const CODEX_FORMAT: &str = "codex-rollout-jsonl";

/// Detect one rollout and select its registered implementation.
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
    if line.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record type is not session_meta".to_string(),
        });
    }
    let payload = line
        .get("payload")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    // session_meta must have a session ID; cli_version may be absent.
    // Missing version uses the latest built-in parser with unknown-version status, not immediate rejection.
    if payload.get("id").and_then(|v| v.as_str()).is_none()
        && payload.get("session_id").and_then(|v| v.as_str()).is_none()
    {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "session_meta without session id".to_string(),
        });
    }
    let found = payload
        .get("cli_version")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: CODEX_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}
