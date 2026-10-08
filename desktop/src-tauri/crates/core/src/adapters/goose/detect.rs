//! Detect Goose sessions.db by usage_ledger or sessions.accumulated_* table/column shapes.

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::common;
use super::versions;

pub const GOOSE_FORMAT: &str = "goose-sessions-db";

/// Column detection: Ok(None) means absent table, Ok(Some) returns columns, Err is a query error.
/// The caller maps busy to Pending; do not misreport query failures as missing tables.
fn table_columns(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<Option<Vec<String>>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        if let Ok(name) = row.get::<_, String>(1) {
            out.push(name);
        }
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

fn missing(columns: &[String], required: &[&'static str]) -> Vec<&'static str> {
    required
        .iter()
        .filter(|c| !columns.iter().any(|col| col == *c))
        .copied()
        .collect()
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    use std::io::Read;
    let mut header = [0; 16];
    let sqlite = std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .is_ok()
        && &header == b"SQLite format 3\0";
    if !sqlite {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "not a SQLite carrier (header mismatch)".into(),
        });
    }
    let conn = match common::open_readonly(path) {
        Ok(conn) => conn,
        Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable SQLite database".to_string(),
            });
        }
    };
    let ledger = match table_columns(&conn, "usage_ledger") {
        Ok(cols) => cols.unwrap_or_default(),
        // A temporary lock from product writes returns Pending for the next scan.
        Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
        Err(err) => return Err(err.into()),
    };
    if !ledger.is_empty() {
        let missing = missing(&ledger, common::LEDGER_COLUMNS);
        if missing.is_empty() {
            return Ok(DetectOutcome::Supported {
                format: GOOSE_FORMAT.to_string(),
                format_version: Some(versions::GOOSE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            });
        }
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("usage_ledger missing required columns: {missing:?}"),
        });
    }
    let sessions = match table_columns(&conn, "sessions") {
        Ok(cols) => cols.unwrap_or_default(),
        Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
        Err(err) => return Err(err.into()),
    };
    if sessions.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "no usage_ledger/sessions tables; not a Goose sessions database".to_string(),
        });
    }
    let missing = missing(&sessions, common::SESSIONS_FALLBACK_COLUMNS);
    if missing.is_empty() {
        // Before schema 15 there is no per-request table; use sessions.accumulated_* aggregates.
        return Ok(DetectOutcome::Supported {
            format: GOOSE_FORMAT.to_string(),
            format_version: Some(versions::GOOSE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        });
    }
    Ok(DetectOutcome::UnknownFormat {
        reason: format!("sessions table missing fallback columns: {missing:?}"),
    })
}
