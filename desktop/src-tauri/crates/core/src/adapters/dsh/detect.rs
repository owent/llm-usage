//! DSH detection distinguishes native rc.2 v4 headers from legacy documented JSONL.
//! Native generated/zstd files use bounded v4 detection; reject other explicit formats.
//! Six legacy event kinds use session-log-doc-1, without establishing native product version.
//! Empty files are Pending; bad JSON/unknown first-row kinds retain format diagnostics.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const DSH_FORMAT: &str = "dsh-session-log-jsonl";

/// Detect one saved session log.
/// Dispatch native/documented formats separately; installed versions do not identify old rows.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    if versions::session_v4::is_native_path(path) {
        return versions::session_v4::detect(path);
    }
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
    match line.get("type").and_then(|t| t.as_str()) {
        Some("session") => versions::session_v4::detect(path),
        Some(event_type)
            if versions::session_log_doc1::DOCUMENTED_EVENT_TYPES.contains(&event_type) =>
        {
            Ok(DetectOutcome::Supported {
                format: DSH_FORMAT.to_string(),
                format_version: Some(versions::DSH_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            })
        }
        other => Ok(DetectOutcome::UnknownFormat {
            reason: format!("first event type {other:?} not in documented set"),
        }),
    }
}
