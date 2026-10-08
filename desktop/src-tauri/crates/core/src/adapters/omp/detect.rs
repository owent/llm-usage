//! omp detection reads at most four lines to identify a first-line title/session record
//! and locate a session header within that window, then selects the implementation through
//! the super::versions registry.
//!
//! Rules: architecture.md#unknown-version; preserve the four first-line detection outcomes.
//! - Non-JSON/first type other than title/session: reject as unknown format.
//! - Title without a session header in four lines: Pending, possibly an incomplete initial write.
//! - Registered header version: KnownVersion; unregistered/missing: LatestFallback,
//!   trying the latest built-in parser with compatibility metadata.

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;
use crate::adapters::jsonl::{read_jsonl, JsonlLimits, DEFAULT_MAX_LINE_BYTES};

use super::versions;

pub const OMP_FORMAT: &str = "omp-session-jsonl";

/// Detect an omp session file and select through its version registry.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Native omp files begin with title v=1 before session; locate the header within four lines.
    let limits = JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(4),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(DetectOutcome::Pending);
    };
    let Ok(first_line) =
        crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        });
    };
    let first_type = first_line
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if first_type != "session" && first_type != "title" {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("first record type {first_type:?} is neither title nor session"),
        });
    }
    for raw in &outcome.lines {
        let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&raw.text)
        else {
            continue;
        };
        if line.get("type").and_then(|t| t.as_str()) != Some("session") {
            continue;
        }
        // With known product identity/input format, select through the registry; missing/unregistered
        // versions try the latest parser with compatibility metadata. Older omp formats remain unverified (V30).
        let found = line.get("version").and_then(|v| v.as_i64());
        let selection = versions::select(found);
        return Ok(DetectOutcome::Supported {
            format: OMP_FORMAT.to_string(),
            format_version: found.map(|v| v.to_string()),
            basis: selection.basis,
        });
    }
    // Title without a session header in four lines may still be writing; retry next scan.
    Ok(DetectOutcome::Pending)
}
