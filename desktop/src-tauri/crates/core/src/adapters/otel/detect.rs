//! Detect OTel span JSONL by record/attribute fields, accepting both spellings.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const OTEL_FORMAT: &str = "otel-spans-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Temporary lock/timeout/removal after enumeration is Pending; retry next run.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    if let Some(detected) = super::versions::qwen_sdk_025::detect_head(&head) {
        return Ok(detected);
    }
    // Recognize gen_ai/agentlens namespaces or span record fields.
    // Avoid whitespace-sensitive matches such as "name":"chat": valid JSON
    // allows whitespace between keys/values; spanId/startTime identify the format.
    let has_genai = text.contains("gen_ai.");
    let has_span_shape = text.contains("\"spanId\"")
        || text.contains("\"span_id\"")
        || text.contains("\"startTime\"");
    // File exporters mix metrics/logs/spans. An initial metrics-only batch
    // still identifies the format; scanning skips non-call records.
    let has_export_shape = text.contains("\"scopeMetrics\"")
        || text.contains("\"resourceMetrics\"")
        || text.contains("\"scopeLogs\"");
    if has_genai || has_span_shape || has_export_shape {
        Ok(DetectOutcome::Supported {
            format: OTEL_FORMAT.to_string(),
            format_version: Some(versions::OTEL_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing OTel span record fingerprint".to_string(),
        })
    }
}
