//! Junie CLI detection: documented sessions/&lt;id&gt;/events.jsonl shape.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const JUNIE_FORMAT: &str = "junie-events-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;
/// Bounded chunk search after identifying a log whose initial window lacks usage.
/// Usage may occur late in large files; bounded reads prevent unbounded scanning.
const DETECT_MAX_SCAN_BYTES: usize = 4 * 1024 * 1024;

fn has_usage_fingerprint(text: &str) -> bool {
    text.contains("LlmResponseMetadataEvent") || text.contains("\"modelUsage\"")
}

fn has_event_fingerprint(text: &str) -> bool {
    text.contains("\"agentEvent\"") || text.contains("\"kind\"")
}

/// Detect events.jsonl by LlmResponseMetadataEvent/modelUsage usage shape.
///
/// For a confirmed typed Junie log without usage in the first 64 KiB,
/// search chunks up to 4 MiB; still absent means Pending, retry next run.
/// Early/no-call sessions must not become permanently uncollectable UnknownFormat.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Transient lock/timeout/deletion after enumeration: Pending, retry rather than permanent failure.
    let mut file = match crate::adapters::run_policy::checked_file(path) {
        Ok(file) => file,
        Err(err) if crate::adapters::framework::is_transient_io(&err) => {
            return Ok(DetectOutcome::Pending);
        }
        Err(err) => return Err(err.into()),
    };
    let mut scanned = String::new();
    let mut head_checked = false;
    let mut total = 0usize;
    let mut chunk = [0u8; DETECT_HEAD_BYTES];
    loop {
        let n = match file.read(&mut chunk) {
            Ok(n) => n,
            Err(err) if crate::adapters::framework::is_transient_io(&err) => {
                return Ok(DetectOutcome::Pending);
            }
            Err(err) => return Err(err.into()),
        };
        if n == 0 {
            if !head_checked && scanned.is_empty() {
                return Ok(DetectOutcome::Pending);
            }
            break;
        }
        total += n;
        scanned.push_str(&String::from_utf8_lossy(&chunk[..n]));
        if !head_checked {
            head_checked = true;
            if !has_event_fingerprint(&scanned) {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "missing Junie agent-event fingerprint".to_string(),
                });
            }
            if has_usage_fingerprint(&scanned) {
                return Ok(DetectOutcome::Supported {
                    format: JUNIE_FORMAT.to_string(),
                    format_version: Some(versions::JUNIE_FORMAT_VERSION.to_string()),
                    basis: VersionBasis::KnownVersion,
                });
            }
        } else if has_usage_fingerprint(&scanned) {
            // Usage after the initial window but within the search limit.
            return Ok(DetectOutcome::Supported {
                format: JUNIE_FORMAT.to_string(),
                format_version: Some(versions::JUNIE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            });
        }
        if total >= DETECT_MAX_SCAN_BYTES {
            break;
        }
    }
    // Typed log without usage in search window may be an early session, not yet identifiable.
    // Pending retries next run, distinct from unknown format; see framework.rs rejection rules.
    Ok(DetectOutcome::Pending)
}
