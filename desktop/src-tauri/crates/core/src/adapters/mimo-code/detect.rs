//! MiMo Code detection: mimocode.db schema columns distinguish this product.
//! message requires agent_id; OpenCode fails this shape. Shared ancestry does not imply compatibility.
//! Select using the session.version registry.
//!
//! Rules: architecture.md#unknown-version / adapters.md A14.
//! - Missing part/session/message tables or key columns: unknown format, reject.
//! - Empty session and part tables: Pending, detect again next run.
//! - File version uses the numerically greatest session.version; registered means KnownVersion.
//!   Otherwise/missing: marked LatestFallback; registry currently empty, source/documentation only.

use crate::adapters::framework::DetectOutcome;
use crate::adapters::mimo_code::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const MIMO_CODE_FORMAT: &str = "mimo-code-sqlite-step-finish-parts";

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::NotADatabase)
    )
}

/// Detect mimocode.db and select the registered implementation.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let source = match open_source_db(path, short_probe, &StagingLimits::default()) {
        Ok(source) => source,
        Err(CoreError::Sqlite(e)) if is_not_a_database(&e) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "file is not a valid sqlite database".to_string(),
            });
        }
        Err(e) => return Err(e),
    };
    let conn = source.conn();
    match schema_fingerprint(conn) {
        Ok(None) => Ok(DetectOutcome::UnknownFormat {
            reason: "sqlite file without mimocode part/session/message columns \
                     (message.agent_id is the product fingerprint; opencode-family \
                     siblings are rejected, not assumed compatible)"
                .to_string(),
        }),
        Ok(Some(_)) => {
            // Empty session and part tables, such as an unused installation: Pending.
            let row_count: i64 = conn
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM session) + (SELECT COUNT(*) FROM part)",
                    [],
                    |r| r.get(0),
                )
                .map_err(CoreError::Sqlite)?;
            if row_count == 0 {
                return Ok(DetectOutcome::Pending);
            }
            let found = crate::adapters::opencode_family::max_session_version(conn)?;
            let selection = versions::select(found.as_deref());
            Ok(DetectOutcome::Supported {
                format: MIMO_CODE_FORMAT.to_string(),
                format_version: found,
                basis: selection.basis,
            })
        }
        Err(CoreError::Sqlite(e)) if is_not_a_database(&e) => Ok(DetectOutcome::UnknownFormat {
            reason: "file is not a valid sqlite database".to_string(),
        }),
        Err(e) => Err(e),
    }
}
