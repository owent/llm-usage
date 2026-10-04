//! OpenCode 探测与版本分派：opencode.db schema 指纹（真实列，产品互斥）+
//! `session.version` 注册表。
//!
//! 约定（architecture.md#unknown-version / adapters.md A17）：
//! - part/session/message 三表或关键列缺失 ⇒ 未知格式 fail closed；
//!   仅存新 core 派生视图层（session_message 表）而无 part 表的库同样拒绝，
//!   待核验格式并编写专用实现（A17：新 core 与旧 message 层不能通用解析）；
//! - 库尚无会话（session/part 均空）⇒ Pending，下轮重探；
//! - 版本标记取库内数值最大 session.version：已收录 ⇒ KnownVersion；
//!   未收录/缺失 ⇒ LatestFallback（带兼容标记；当前注册表为空：文档或源码依据）。

use crate::adapters::framework::DetectOutcome;
use crate::adapters::opencode::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const OPENCODE_FORMAT: &str = "opencode-sqlite-step-finish-parts";

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::NotADatabase)
    )
}

/// 探测一个 opencode.db 并按注册表分派。
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
                "opencode core session_message projection without legacy part table; \
                 not fixture-verified, fail closed"
                    .to_string()
            } else {
                "sqlite file without opencode part/session/message tables".to_string()
            };
            Ok(DetectOutcome::UnknownFormat { reason })
        }
        Ok(Some(_)) => {
            // 空 session 且空 part（新装未用）⇒ Pending；版本取最大 session.version。
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
                format: OPENCODE_FORMAT.to_string(),
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
