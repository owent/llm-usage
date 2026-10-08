//! Detect the gen_metadata table in Antigravity conversations/&lt;uuid&gt;.db.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const ANTIGRAVITY_FORMAT: &str = "antigravity-gen-metadata-db";

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let conn = match rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable SQLite database".to_string(),
            });
        }
    };
    // A new zero-byte database has no detectable content; return Pending and retry later.
    let is_empty = std::fs::metadata(path)
        .map(|m| m.len() == 0)
        .unwrap_or(false);
    if is_empty {
        return Ok(DetectOutcome::Pending);
    }
    // Host writes can cause transient busy errors; return Pending rather than UnknownFormat.
    if conn
        .busy_timeout(std::time::Duration::from_millis(150))
        .is_err()
    {
        return Ok(DetectOutcome::Pending);
    }
    let has_table: bool = match conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='gen_metadata'",
        [],
        |r| r.get::<_, i64>(0),
    ) {
        Ok(v) => v > 0,
        Err(_) => return Ok(DetectOutcome::Pending),
    };
    if has_table {
        Ok(DetectOutcome::Supported {
            format: ANTIGRAVITY_FORMAT.to_string(),
            format_version: Some(versions::ANTIGRAVITY_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "no gen_metadata table; not an Antigravity conversations db".to_string(),
        })
    }
}
