//! omp detection reads at most four lines to identify a first-line title/session record
//! and locate a session header within that window, then selects the implementation through
//! the super::versions registry.
//!
//! Rules: architecture.md#unknown-version; preserve the four first-line detection outcomes.
//! - Non-JSON/first type other than title/session: reject as unknown format.
//! - Title without a session header in four lines: Pending, possibly an incomplete initial write.
//! - Registered header version: KnownVersion; unregistered/missing: LatestFallback,
//!   trying the latest built-in parser with compatibility metadata.
//! - `changes.jsonl` is a non-usage file-edit audit log only when its filename and
//!   `call`/`change` record shape match; model_usage remains a session entry.

use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;
use crate::adapters::jsonl::{read_jsonl, JsonlLimits, DEFAULT_MAX_LINE_BYTES};

use super::versions;

pub const OMP_FORMAT: &str = "omp-session-jsonl";
pub const OMP_CHANGE_AUDIT_FORMAT: &str = "omp-change-audit-jsonl";

pub(super) fn change_audit_record(path: &Path, row: &serde_json::Value) -> bool {
    if path.file_name().is_none_or(|name| name != "changes.jsonl") {
        return false;
    }
    let Some(fields) = row.as_object() else {
        return false;
    };
    match row["type"].as_str() {
        Some("call") => {
            ["agent", "tool", "start", "end"]
                .iter()
                .all(|k| fields.contains_key(*k))
                && fields.iter().all(|(k, v)| match k.as_str() {
                    "type" | "agent" | "tool" | "label" | "parent" | "script" => v.is_string(),
                    "start" | "end" => v.is_number(),
                    _ => false,
                })
        }
        Some("change") => {
            ["path", "at", "before", "after"]
                .iter()
                .all(|k| fields.contains_key(*k))
                && fields.iter().all(|(k, v)| match k.as_str() {
                    "type" | "path" | "agent" | "tool" => v.is_string(),
                    "at" | "beforeMtimeMs" => v.is_number(),
                    "before" | "after" => {
                        v.is_null() || v.is_string() || v.as_object().is_some_and(|o| o.is_empty())
                    }
                    _ => false,
                })
        }
        _ => false,
    }
}

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
    // Exclude file-change audit logs only when their filename and record shape both match.
    if change_audit_record(path, &first_line) {
        return Ok(DetectOutcome::Supported {
            format: OMP_CHANGE_AUDIT_FORMAT.to_string(),
            format_version: None,
            basis: VersionBasis::KnownVersion,
        });
    }
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
