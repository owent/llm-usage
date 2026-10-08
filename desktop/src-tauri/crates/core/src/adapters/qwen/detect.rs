//! Qwen legacy ChatRecord detection reads a bounded first line, requiring a documented type
//! from the fixed-source four-value enumeration and uuid/sessionId/timestamp identity,
//! then selects the implementation through super::versions.
//!
//! Rules: architecture.md#unknown-version / V17.
//! - Non-JSON, undocumented type or missing identity: unknown format;
//!   reject rather than guess an arbitrary file format.
//! - Format ID is the registered fixed-source commit prefix: KnownVersion.
//!   Store each record.version (CLI release) as schema_version without restricting releases;
//!   it does not control detection dispatch.
//!
//! M2/V30 moved root-level qwen.rs into this directory without changing these rules.

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;

use super::versions;

pub const QWEN_FORMAT: &str = "qwen-chatrecord-jsonl";

/// Detect a legacy ChatRecord file and select through its registry.
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
    let record_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if !versions::chatrecord_085e98c0::RECORD_TYPES.contains(&record_type) {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("first record type {record_type:?} not in ChatRecord set"),
        });
    }
    // Fixed source requires uuid/sessionId/timestamp identity fields.
    if line.get("uuid").and_then(|v| v.as_str()).is_none()
        || line.get("sessionId").and_then(|v| v.as_str()).is_none()
        || line.get("timestamp").and_then(|v| v.as_str()).is_none()
    {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first record missing required ChatRecord identity fields".to_string(),
        });
    }
    // The registered fixed-source commit prefix selects KnownVersion.
    // Do not select by record.version; scanning stores it as schema_version per record.
    let selection = versions::select(Some(versions::QWEN_FORMAT_VERSION));
    Ok(DetectOutcome::Supported {
        format: QWEN_FORMAT.to_string(),
        format_version: Some(versions::QWEN_FORMAT_VERSION.to_string()),
        basis: selection.basis,
    })
}
