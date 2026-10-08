//! Hermes detection: state.db tables/columns/primary-key shape,
//! not schema_version alone; select through the registry.
//!
//! Rules: architecture.md#unknown-version / adapters.md local Hermes layout.
//! - Missing sessions/session_model_usage tables or key columns: unknown format, reject.
//! - Complete columns but primary key lacks task, pre-v22: reject pending verification.
//!   Upstream _migrate_v22_session_model_usage rebuilds it when opening;
//!   this collector never runs source migrations/repairs.
//! - Both tables empty, such as unused installations: Pending, detect again next run.
//! - Native 0.21.5 samples exist, but database migration versions do not identify every row.
//!   Registry stays empty; all versions use marked LatestFallback.

use crate::adapters::framework::DetectOutcome;
use crate::adapters::hermes::common::{
    db_schema_version, open_source_db, schema_probe, short_probe, StagingLimits,
};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const HERMES_FORMAT: &str = "hermes-state-db-session-model-usage";

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::NotADatabase)
    )
}

/// Detect state.db and select the registered implementation.
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
    match schema_probe(conn) {
        Ok(probe) => match probe.fingerprint {
            None => Ok(DetectOutcome::UnknownFormat {
                reason: if probe.legacy_pk {
                    "session_model_usage primary key lacks task (pre-v22 shape); \
                     fail closed pending evidence"
                        .to_string()
                } else {
                    "sqlite file without hermes sessions/session_model_usage columns".to_string()
                },
            }),
            Some(_) => {
                let row_count: i64 = conn
                    .query_row(
                        "SELECT (SELECT COUNT(*) FROM sessions) \
                           + (SELECT COUNT(*) FROM session_model_usage)",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(CoreError::Sqlite)?;
                if row_count == 0 {
                    return Ok(DetectOutcome::Pending);
                }
                let found = db_schema_version(conn)?;
                let selection = versions::select(found.map(|v| v.to_string()).as_deref());
                Ok(DetectOutcome::Supported {
                    format: HERMES_FORMAT.to_string(),
                    format_version: found.map(|v| v.to_string()),
                    basis: selection.basis,
                })
            }
        },
        Err(CoreError::Sqlite(e)) if is_not_a_database(&e) => Ok(DetectOutcome::UnknownFormat {
            reason: "file is not a valid sqlite database".to_string(),
        }),
        Err(e) => Err(e),
    }
}
