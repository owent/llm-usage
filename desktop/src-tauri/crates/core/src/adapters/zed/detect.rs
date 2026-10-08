//! Detect Zed threads.db by tables/columns from official bd74733 creation/migration SQL.
//!
//! Historical local read-only check, 2026-09-29: %LOCALAPPDATA%/Zed/threads/threads.db
//! had the expected threads columns but zero rows, verifying only the schema.
//!
//! V17 detection rules:
//! - Non-SQLite, absent threads or missing required columns is UnknownFormat.
//! - Matching threads columns, even with no rows, is Supported; empty files remain Pending in the framework.
//! - The format ID zed-threads-db-1 comes from source; it does not identify a product version.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::common;
use super::versions;

pub const ZED_FORMAT: &str = "zed-threads-db";

/// Detect threads.db by its threads table and required columns.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let conn = match common::open_readonly(path) {
        Ok(conn) => conn,
        Err(err) if common::is_busy_like(&err) => {
            // A busy/locked database is Pending for a later check, rather than UnknownFormat.
            return Ok(DetectOutcome::Pending);
        }
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable SQLite database".to_string(),
            });
        }
    };
    let columns: Vec<String> = match conn.prepare("PRAGMA table_info(threads)") {
        Ok(mut stmt) => {
            // A transient product-held lock remains Pending; do not persist it as a format conclusion.
            let mut rows = match stmt.query([]) {
                Ok(rows) => rows,
                Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
                Err(err) => return Err(err.into()),
            };
            let mut out = Vec::new();
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        if let Ok(name) = row.get::<_, String>(1) {
                            out.push(name);
                        }
                    }
                    Ok(None) => break,
                    Err(err) if common::is_busy_like(&err) => {
                        return Ok(DetectOutcome::Pending);
                    }
                    Err(err) => return Err(err.into()),
                }
            }
            out
        }
        Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
        Err(_) => Vec::new(),
    };
    if columns.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "no threads table; not a Zed threads database".to_string(),
        });
    }
    let missing: Vec<&str> = common::REQUIRED_THREADS_COLUMNS
        .iter()
        .filter(|c| !columns.iter().any(|col| col == *c))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("threads table missing required columns: {missing:?}"),
        });
    }
    // created_at is a migration column (official db.rs:460-483); the scanner handles its absence as an older database.
    Ok(DetectOutcome::Supported {
        format: ZED_FORMAT.to_string(),
        format_version: Some(versions::ZED_FORMAT_VERSION.to_string()),
        basis: VersionBasis::KnownVersion,
    })
}
