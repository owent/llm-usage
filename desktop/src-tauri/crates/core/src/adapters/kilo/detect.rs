//! Kilo detection: kilo.db tables/key columns and
//! session.version registry. No official environment override, A11.
//!
//! Rules: architecture.md#unknown-version.
//! - Missing message/session tables or key columns: unknown format, reject.
//!   Also reject new-core session_message databases without message;
//!   verify and implement separately; compatibility with old message layout is unestablished.
//! - Empty session table: Pending, detect again next run.
//! - File marker uses greatest numeric session.version; registered means KnownVersion.
//!   Missing/unregistered versions use marked LatestFallback; scanners retain each message's session version.

use crate::adapters::framework::DetectOutcome;
use crate::adapters::kilo::common::{open_source_db, schema_fingerprint, StagingLimits};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const KILO_FORMAT: &str = "kilo-sqlite-message-tokens";

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::NotADatabase)
    )
}

fn probe_schema(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// Detect kilo.db and select the registered implementation.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let source = match open_source_db(path, probe_schema, &StagingLimits::default()) {
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
        // Distinguish new-core layout from unrelated databases; reject both unverified shapes.
        Ok(None) => {
            let has_session_message: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' \
                     AND name='session_message')",
                    [],
                    |r| r.get(0),
                )
                .map_err(CoreError::Sqlite)?;
            let reason = if has_session_message {
                "kilo core session_message data layer without legacy message table; \
                 not fixture-verified, fail closed"
                    .to_string()
            } else {
                "sqlite file without kilo message/session tables".to_string()
            };
            Ok(DetectOutcome::UnknownFormat { reason })
        }
        Err(CoreError::Sqlite(e)) if is_not_a_database(&e) => Ok(DetectOutcome::UnknownFormat {
            reason: "file is not a valid sqlite database".to_string(),
        }),
        Err(e) => Err(e),
        Ok(Some(_)) => {
            // File marker is greatest numeric session.version; no sessions means Pending.
            let mut stmt = conn
                .prepare("SELECT version FROM session")
                .map_err(CoreError::Sqlite)?;
            let versions = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(CoreError::Sqlite)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CoreError::Sqlite)?;
            let Some(found) = versions
                .into_iter()
                .reduce(|a, b| versions::version_max(&a, &b).to_string())
            else {
                return Ok(DetectOutcome::Pending);
            };
            let selection = versions::select(Some(&found));
            Ok(DetectOutcome::Supported {
                format: KILO_FORMAT.to_string(),
                format_version: Some(found),
                basis: selection.basis,
            })
        }
    }
}
