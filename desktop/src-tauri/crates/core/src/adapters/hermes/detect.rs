//! Hermes 探测与版本分派：state.db schema 指纹（真实列 + 主键形状，
//! 不只看 schema_version 整数）+ 注册表。
//!
//! 约定（architecture.md#unknown-version / adapters.md Hermes 本地约定）：
//! - sessions/session_model_usage 两表或关键列缺失 ⇒ 未知格式 fail closed；
//! - 关键列齐全但主键不含 task（pre-v22 形状）⇒ fail closed 待核验
//!   （固定源码 _migrate_v22_session_model_usage 会在上游打开时重建，
//!   本采集器绝不代跑迁移/修复）；
//! - 两表皆空（新装未用）⇒ Pending，下轮重探；
//! - schema_version 已收录（真实 fixture 核验）⇒ KnownVersion；当前注册表
//!   为空（仅固定源码/按文档或源码实现，待真实样本核验），一律 LatestFallback 带兼容标记。

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

/// 探测一个 state.db 并按注册表分派。
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
