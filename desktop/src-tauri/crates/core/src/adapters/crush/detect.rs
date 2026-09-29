//! Crush 探测：项目库 crush.db 的 sessions 表指纹（cost-only 载体）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::common;
use super::versions;

pub const CRUSH_FORMAT: &str = "crush-sessions-db";

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
    let columns: Vec<String> = {
        let Ok(mut stmt) = conn.prepare("PRAGMA table_info(sessions)") else {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "no sessions table; not a Crush database".to_string(),
            });
        };
        let Ok(mut rows) = stmt.query([]) else {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "sessions table probe failed".to_string(),
            });
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            if let Ok(name) = row.get::<_, String>(1) {
                out.push(name);
            }
        }
        out
    };
    let missing: Vec<&str> = common::SESSIONS_COLUMNS
        .iter()
        .filter(|c| !columns.iter().any(|col| col == *c))
        .copied()
        .collect();
    if missing.is_empty() {
        Ok(DetectOutcome::Supported {
            format: CRUSH_FORMAT.to_string(),
            format_version: Some(versions::CRUSH_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: format!("sessions table missing required columns: {missing:?}"),
        })
    }
}
