//! Goose 探测：sessions.db 的表/列指纹（usage_ledger 或 sessions.accumulated_*）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::common;
use super::versions;

pub const GOOSE_FORMAT: &str = "goose-sessions-db";

fn table_columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({table})")) else {
        return Vec::new();
    };
    let Ok(mut rows) = stmt.query([]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        if let Ok(name) = row.get::<_, String>(1) {
            out.push(name);
        }
    }
    out
}

fn missing(columns: &[String], required: &[&'static str]) -> Vec<&'static str> {
    required
        .iter()
        .filter(|c| !columns.iter().any(|col| col == *c))
        .copied()
        .collect()
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let conn = match common::open_readonly(path) {
        Ok(conn) => conn,
        Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable SQLite database".to_string(),
            });
        }
    };
    let ledger = table_columns(&conn, "usage_ledger");
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
    let sessions = table_columns(&conn, "sessions");
    if sessions.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "no usage_ledger/sessions tables; not a Goose sessions database".to_string(),
        });
    }
    let missing = missing(&sessions, common::SESSIONS_FALLBACK_COLUMNS);
    if missing.is_empty() {
        // 旧库（schema < 15）：无逐请求表，按 sessions.accumulated_* 聚合兜底。
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
