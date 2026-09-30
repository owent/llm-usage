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
        let mut stmt = match conn.prepare("PRAGMA table_info(sessions)") {
            Ok(stmt) => stmt,
            // 瞬态锁（超过 busy_timeout 的持库）⇒ Pending，不误判格式不明。
            Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
            Err(_) => {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "no sessions table; not a Crush database".to_string(),
                });
            }
        };
        let mut rows = match stmt.query([]) {
            Ok(rows) => rows,
            Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
            Err(_) => {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "sessions table probe failed".to_string(),
                });
            }
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
                Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
                Err(_) => break,
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
