//! Recognize candidate Qoder files; scanning verifies the CLI envelope and each record version.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};

pub const QODER_FORMAT: &str = "qoder-session-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub(crate) fn native_record(path: &Path) -> bool {
    if path.extension().map_or(true, |e| e != "jsonl") {
        return false;
    }
    let Ok(file) = super::super::run_policy::checked_file(path) else {
        return false;
    };
    // User/tool content may precede the first identifying assistant. Inspect complete
    // lines within the normal 32 MiB window, retaining at most one 8 MiB line.
    let mut reader = BufReader::with_capacity(64 * 1024, file.take(32 * 1024 * 1024));
    let deadline = Instant::now() + Duration::from_millis(200);
    let mut bytes = Vec::new();
    let mut first = true;
    while Instant::now() < deadline {
        bytes.clear();
        if (&mut reader)
            .take(super::super::jsonl::DEFAULT_MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut bytes)
            .is_err()
            || bytes.len() > super::super::jsonl::DEFAULT_MAX_LINE_BYTES
            || bytes.last() != Some(&b'\n')
        {
            return false;
        }
        let line = if first {
            bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)
        } else {
            &bytes
        };
        first = false;
        let Ok(row) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if row["type"] == "assistant"
            && row["entrypoint"] == "cli"
            && row["userType"] == "external"
            && row["isSidechain"].is_boolean()
            && row["message"]["role"] == "assistant"
            && ["sessionId", "uuid", "version", "cwd"]
                .iter()
                .all(|k| row[*k].as_str().is_some_and(|v| !v.trim().is_empty()))
            && row["message"]["id"]
                .as_str()
                .is_some_and(|v| !v.trim().is_empty())
            && (row["modelSource"] == "custom"
                || row["message"]["usage"]["billable"].is_boolean()
                || ["credits", "original_credits"].iter().any(|key| {
                    row["message"]["usage"][*key]
                        .as_f64()
                        .is_some_and(f64::is_finite)
                }))
        {
            return true;
        }
    }
    false
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Temporary lock/timeout/removal after enumeration is Pending; retry detection next run.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    // Recognize JSONL sessions or state.json objects as Qoder session files,
    // and let the scanner validate each native CLI record independently.
    // A session_id substring match also requires an opening brace; otherwise unrelated
    // files containing "session_id" could be incorrectly claimed.
    let looks_jsonl = text.starts_with('{')
        && (text.contains("\"sessionId\"") || text.contains("\"session_id\""));
    let looks_state =
        text.contains("\"modelRequests\"") || text.contains("\"compact_token_usage_json\"");
    if looks_jsonl || looks_state || native_record(path) {
        Ok(DetectOutcome::Supported {
            format: QODER_FORMAT.to_string(),
            format_version: None,
            basis: VersionBasis::LatestFallback,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Qoder session fingerprint".to_string(),
        })
    }
}
