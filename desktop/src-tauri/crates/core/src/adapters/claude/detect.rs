//! Detect Claude Code from a bounded first line and documented record types.
//!
//! First line checks format only, not client versions of other rows. Each assistant
//! uses its own version; queue metadata or installed versions do not verify historical usage.

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;

use super::versions::CLAUDE_FORMAT_VERSION;

pub const CLAUDE_FORMAT: &str = "claude-transcript-jsonl";

/// Detect one transcript; missing version allows fallback, using a documented format identifier.
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
    match line.get("type").and_then(|t| t.as_str()) {
        Some("user") | Some("assistant") | Some("system") => Ok(DetectOutcome::Supported {
            format: CLAUDE_FORMAT.to_string(),
            format_version: Some(CLAUDE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        }),
        // Native Claude Code 2.1.197 sample, WSL, 2026-09-30: queue/attachment/last-prompt
        // metadata may appear first; these records contain no usage and detection accepts them.
        // Line parsing still selects permitted fields through transcript_doc1.
        Some("queue-operation") | Some("attachment") | Some("last-prompt") => {
            Ok(DetectOutcome::Supported {
                format: CLAUDE_FORMAT.to_string(),
                format_version: Some(CLAUDE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            })
        }
        other => Ok(DetectOutcome::UnknownFormat {
            reason: format!("first record type {other:?} not in documented set"),
        }),
    }
}
