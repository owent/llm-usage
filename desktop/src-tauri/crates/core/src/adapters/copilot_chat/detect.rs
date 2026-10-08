//! Detect VS Code Copilot Chat logs from the first {kind:0, v:{version,...}} record.
//!
//! objectMutationLog.ts allows Initial, kind 0, only first, with the complete initial
//! object; chatSessionOperationLog.ts storageSchema fixes version: 3.
//! Compact rewrites also begin with Initial containing full current state, possibly large.
//! Bound the first-line read to the JSONL reader limit, 8 MiB.

use crate::adapters::framework::DetectOutcome;
use crate::error::CoreError;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use super::versions;

pub const COPILOT_CHAT_FORMAT: &str = "vscode-chat-session-log";

/// First-line size limit, matching the JSONL reader.
const DETECT_MAX_FIRST_LINE: usize = crate::adapters::jsonl::DEFAULT_MAX_LINE_BYTES;

/// Read without newline; return None when too large to identify.
fn read_first_line(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let file = crate::adapters::run_policy::checked_file(path)?;
    let mut reader =
        BufReader::with_capacity(64 * 1024, file.take(DETECT_MAX_FIRST_LINE as u64 + 1));
    let mut buf = Vec::new();
    let n = reader.read_until(b'\n', &mut buf)?;
    if n == 0 || buf.last() != Some(&b'\n') {
        return Ok(None); // Empty file has nothing to identify yet.
    }
    if buf.len() > DETECT_MAX_FIRST_LINE {
        return Ok(None);
    }
    while buf.last() == Some(&b'\n') || buf.last() == Some(&b'\r') {
        buf.pop();
    }
    // Skip UTF-8 BOM.
    if buf.starts_with(b"\xEF\xBB\xBF") {
        buf.drain(0..3);
    }
    Ok(Some(buf))
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let line = match read_first_line(path) {
        Ok(line) => line,
        Err(err) if crate::adapters::framework::is_transient_io(&err) => {
            return Ok(DetectOutcome::Pending);
        }
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable session log".to_string(),
            });
        }
    };
    let Some(line) = line else {
        return Ok(DetectOutcome::Pending);
    };
    let doc: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&line) {
        Ok(doc) => doc,
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "first line is not a JSON mutation-log entry".to_string(),
            });
        }
    };
    // Initial: kind==0 and object v containing session state, such as sessionId/requests.
    if doc.get("kind").and_then(serde_json::Value::as_i64) != Some(0)
        || !doc
            .get("v")
            .map(serde_json::Value::is_object)
            .unwrap_or(false)
    {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first entry is not an Initial mutation-log record".to_string(),
        });
    }
    let v = &doc["v"];
    if !v
        .get("sessionId")
        .map(serde_json::Value::is_string)
        .unwrap_or(false)
        || !v
            .get("requests")
            .map(serde_json::Value::is_array)
            .unwrap_or(false)
    {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "initial record lacks chat session shape (sessionId/requests)".to_string(),
        });
    }
    let found = v
        .get("version")
        .and_then(serde_json::Value::as_i64)
        .map(|n| n.to_string());
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: COPILOT_CHAT_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VersionBasis;

    fn temp_log(name: &str, lines: &[String]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-copilot-chat-detect-{}-{}-{}.jsonl",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(&path, text).unwrap();
        path
    }

    fn header(version: i64) -> String {
        format!(
            r#"{{"kind":0,"v":{{"version":{version},"creationDate":1790781064329,"sessionId":"11111111-2222-3333-4444-555555555555","requests":[],"pendingRequests":[]}}}}"#
        )
    }

    #[test]
    fn version3_header_is_supported_known_version() {
        let path = temp_log(
            "v3",
            &[
                header(3),
                r#"{"kind":1,"k":["customTitle"],"v":"t"}"#.to_string(),
            ],
        );
        match detect(&path).unwrap() {
            DetectOutcome::Supported {
                format,
                format_version,
                basis,
            } => {
                assert_eq!(format, COPILOT_CHAT_FORMAT);
                assert_eq!(format_version.as_deref(), Some("3"));
                assert_eq!(basis, VersionBasis::KnownVersion);
            }
            other => panic!("expected Supported, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn other_version_falls_back_to_latest() {
        let path = temp_log("v4", &[header(4)]);
        match detect(&path).unwrap() {
            DetectOutcome::Supported { basis, .. } => {
                assert_eq!(basis, VersionBasis::LatestFallback);
            }
            other => panic!("expected Supported(latest), got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn plain_jsonl_is_unknown_format() {
        let path = temp_log(
            "plain",
            &[r#"{"type":"assistant.message","data":{}}"#.to_string()],
        );
        assert!(matches!(
            detect(&path).unwrap(),
            DetectOutcome::UnknownFormat { .. }
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_file_is_pending() {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-copilot-chat-detect-empty-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&path, b"").unwrap();
        assert!(matches!(detect(&path).unwrap(), DetectOutcome::Pending));
        let _ = std::fs::remove_file(&path);
    }
}
