//! Kiro 探测：CLI 会话头 JSON 与 kiro-cli SQLite 双载体指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const KIRO_FORMAT: &str = "kiro-cli-turns-json";
pub const KIRO_SQLITE_FORMAT: &str = "kiro-cli-sqlite";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // SQLite 载体：按头 16 字节 magic（"SQLite format 3\0"）。
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; DETECT_HEAD_BYTES];
    let n = file.read(&mut head)?;
    head.truncate(n);
    if head.starts_with(b"SQLite format 3\0") {
        let conn = match rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => conn,
            Err(_) => return Ok(DetectOutcome::Pending),
        };
        let has_table: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='conversations_v2'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|v| v > 0)
            .unwrap_or(false);
        return if has_table {
            Ok(DetectOutcome::Supported {
                format: KIRO_SQLITE_FORMAT.to_string(),
                format_version: Some(versions::KIRO_SQLITE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            })
        } else {
            Ok(DetectOutcome::UnknownFormat {
                reason: "no conversations_v2 table; not a kiro-cli database".to_string(),
            })
        };
    }
    // CLI 会话头 JSON。
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let has_session = text.contains("\"session_id\"");
    let has_turns = text.contains("\"user_turn_metadatas\"");
    if has_session && has_turns {
        Ok(DetectOutcome::Supported {
            format: KIRO_FORMAT.to_string(),
            format_version: Some(versions::KIRO_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Kiro session fingerprint (session_id/user_turn_metadatas)".to_string(),
        })
    }
}
