//! OpenCode 探测与版本分派：opencode.db schema 指纹（真实列，产品互斥）+
//! `session.version` 注册表。
//!
//! 约定（architecture.md#unknown-version / adapters.md A17）：
//! - part/session/message 三表或关键列缺失 ⇒ 未知格式 fail closed；
//!   仅存新 core 派生视图层（session_message 表）而无 part 表的库同样拒绝，
//!   待核验格式并编写专用实现（A17：新 core 与旧 message 层不能通用解析）；
//! - 库尚无用量部件 ⇒ Pending，下轮重探；空会话不参与版本认证；
//! - 每条 step-finish 按所属 session.version 认证；混合库保留兼容标记。

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
            let Some((found, basis)) = usage_version_summary(conn)? else {
                return Ok(DetectOutcome::Pending);
            };
            Ok(DetectOutcome::Supported {
                format: OPENCODE_FORMAT.to_string(),
                format_version: found,
                basis,
            })
        }
        Err(CoreError::Sqlite(e)) if is_not_a_database(&e) => Ok(DetectOutcome::UnknownFormat {
            reason: "file is not a valid sqlite database".to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// 文件级依据只覆盖现存用量部件；不是库内最高版本或空会话的认证。
pub(crate) fn usage_version_summary(
    conn: &rusqlite::Connection,
) -> Result<Option<(Option<String>, crate::domain::VersionBasis)>, CoreError> {
    use crate::domain::VersionBasis;
    let mut statement = conn.prepare("SELECT DISTINCT CASE WHEN typeof(s.version)='text' THEN s.version END FROM part p LEFT JOIN session s ON s.id=p.session_id WHERE CASE WHEN json_valid(p.data) THEN json_extract(p.data,'$.type')='step-finish' ELSE 1 END")?;
    let mut rows = statement.query([])?;
    let mut versions_seen = std::collections::BTreeSet::new();
    let mut basis = VersionBasis::KnownVersion;
    while let Some(row) = rows.next()? {
        crate::adapters::run_policy::check()?;
        let version: Option<String> = row.get(0)?;
        if versions::select(version.as_deref()).basis == VersionBasis::LatestFallback {
            basis = VersionBasis::LatestFallback;
        }
        versions_seen.insert(version);
    }
    if versions_seen.is_empty() {
        return Ok(None);
    }
    let version = if versions_seen.len() == 1 {
        versions_seen.into_iter().next().flatten()
    } else {
        None
    };
    Ok(Some((version, basis)))
}
