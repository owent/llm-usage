//! Antigravity 探测：conversations/&lt;uuid&gt;.db 的 gen_metadata 表指纹。

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
    let has_table: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='gen_metadata'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v > 0)
        .unwrap_or(false);
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
