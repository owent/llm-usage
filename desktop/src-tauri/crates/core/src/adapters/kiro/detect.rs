//! Detect Kiro CLI session-header JSON or kiro-cli SQLite.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const KIRO_FORMAT: &str = "kiro-cli-turns-json";
pub const KIRO_SQLITE_FORMAT: &str = "kiro-cli-sqlite";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // Identify SQLite by its first 16 bytes: "SQLite format 3\0".
    // Temporary lock, timeout or deletion after enumeration: Pending, retry next scan.
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    if head.starts_with(b"SQLite format 3\0") {
        let conn = match rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => conn,
            Err(_) => return Ok(DetectOutcome::Pending),
        };
        // busy during kiro-cli writes is temporary: Pending, rather than UnknownFormat.
        if conn
            .busy_timeout(std::time::Duration::from_millis(150))
            .is_err()
        {
            return Ok(DetectOutcome::Pending);
        }
        let has_table: bool = match conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='conversations_v2'",
            [],
            |r| r.get::<_, i64>(0),
        ) {
            Ok(v) => v > 0,
            Err(_) => return Ok(DetectOutcome::Pending),
        };
        return if has_table {
            Ok(DetectOutcome::Supported {
                format: KIRO_SQLITE_FORMAT.to_string(),
                format_version: Some(versions::KIRO_SQLITE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            })
        } else {
            Ok(DetectOutcome::UnknownFormat {
                reason: "no conversations_v2 table; not a kiro-cli database".to_string(),
            })
        };
    }
    if !head.is_empty() && b"SQLite format 3\0".starts_with(&head) {
        return Ok(DetectOutcome::Pending);
    }
    // CLI session-header JSON.
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let has_session = text.contains("\"session_id\"");
    let has_turns = text.contains("\"user_turn_metadatas\"");
    if has_session && has_turns {
        Ok(DetectOutcome::Supported {
            format: KIRO_FORMAT.to_string(),
            format_version: Some(versions::KIRO_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Kiro session fingerprint (session_id/user_turn_metadatas)".to_string(),
        })
    }
}
