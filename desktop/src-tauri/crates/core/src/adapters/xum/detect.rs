//! Detect documented session-usage.json fields: byModel and lastRequest/version.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const XUM_FORMAT: &str = "xum-session-usage-json";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Temporary lock/timeout/removal after enumeration is Pending; retry detection next run.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let has_by_model = text.contains("\"byModel\"");
    let has_anchor = text.contains("\"lastRequest\"") || text.contains("\"version\"");
    if has_by_model && has_anchor {
        // Classify integer version honestly (V30): reference parser tokscale mux.rs covers
        // documented version=1 or absent; unseen integer values use LatestFallback
        // compatibility attempts, without claiming KnownVersion.
        let basis = match crate::adapters::run_policy::json_from_str::<serde_json::Value>(&text) {
            Ok(doc) if doc.is_object() => {
                if !doc.get("byModel").is_some_and(|v| v.is_object())
                    || (doc.get("lastRequest").is_none() && doc.get("version").is_none())
                {
                    return Ok(DetectOutcome::UnknownFormat {
                        reason: "Xum session usage lacks top-level byModel/anchor".to_string(),
                    });
                }
                match doc.get("version") {
                    None => VersionBasis::KnownVersion,
                    Some(v) if v.as_u64() == Some(1) => VersionBasis::KnownVersion,
                    _ => VersionBasis::LatestFallback,
                }
            }
            Ok(_) => {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "Xum session usage must be a JSON object".to_string(),
                });
            }
            // A truncated prefix or incomplete JSON parse does not verify the version.
            Err(_) => VersionBasis::LatestFallback,
        };
        Ok(DetectOutcome::Supported {
            format: XUM_FORMAT.to_string(),
            format_version: Some(versions::XUM_FORMAT_VERSION.to_string()),
            basis,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing byModel/lastRequest fingerprint".to_string(),
        })
    }
}
