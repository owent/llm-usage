//! MiMo Code 探测与版本分派：mimocode.db schema 指纹（真实列，产品互斥：
//! message 须含 `agent_id` 列 ⇒ OpenCode 库不能过本指纹，不从 fork 关系推兼容）+
//! `session.version` 注册表。
//!
//! 约定（architecture.md#unknown-version / adapters.md A14）：
//! - part/session/message 三表或关键列缺失 ⇒ 未知格式 fail closed；
//! - 库尚无会话（session/part 均空）⇒ Pending，下轮重探；
//! - 版本标记取库内数值最大 session.version：已收录 ⇒ KnownVersion；
//!   未收录/缺失 ⇒ LatestFallback（带兼容标记；当前注册表为空：文档或源码依据）。

use crate::adapters::framework::DetectOutcome;
use crate::adapters::mimo_code::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const MIMO_CODE_FORMAT: &str = "mimo-code-sqlite-step-finish-parts";

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::NotADatabase)
    )
}

/// 探测一个 mimocode.db 并按注册表分派。
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
        Ok(None) => Ok(DetectOutcome::UnknownFormat {
            reason: "sqlite file without mimocode part/session/message columns \
                     (message.agent_id is the product fingerprint; opencode-family \
                     siblings are rejected, not assumed compatible)"
                .to_string(),
        }),
        Ok(Some(_)) => {
            // 空 session 且空 part（新装未用）⇒ Pending。
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
                format: MIMO_CODE_FORMAT.to_string(),
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
