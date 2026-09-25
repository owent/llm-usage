//! Kilo 探测与版本分派：kilo.db schema 指纹（表存在性/关键列）+
//! `session.version` 注册表。kilo 无官方 env 覆盖（A11），发现层不做 env 项。
//!
//! 合同（architecture.md#unknown-version）：
//! - message/session 两表或关键列缺失 ⇒ 未知格式 fail closed；
//!   仅存新 core 数据层（session_message）而无 message 表的库同样拒绝，
//!   待专用实现取证（adapters.md：新版与旧 message 表不保证兼容）；
//! - 库尚无会话（session 空表）⇒ Pending，下轮重探；
//! - 版本标记取库内数值最大 session.version：已收录 ⇒ KnownVersion；
//!   未收录/缺失 ⇒ LatestFallback（带兼容标记，不因版本号未收录直接拒绝）。

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

/// 探测一个 kilo.db 并按注册表分派。
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
        // 指纹不含表：区分“新 core 数据层”与“完全无关的库”，两者都 fail closed。
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
            // 版本标记：库内数值最大 session.version；空 session 表 ⇒ Pending。
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
