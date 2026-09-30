//! Copilot CLI 探测：session-store.db 的 assistant_usage_events 表指纹。

use crate::adapters::framework::DetectOutcome;
use crate::error::CoreError;
use std::path::Path;

use super::common;
use super::versions;

pub const COPILOT_FORMAT: &str = "copilot-assistant-usage-events-db";

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
        // 瞬态锁（杀软/产品进程持库）是常态：busy 类错误 Pending 下轮重探，
        // 不固化为 UnknownFormat。
        let mut stmt = match conn.prepare("PRAGMA table_info(assistant_usage_events)") {
            Ok(stmt) => stmt,
            Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
            Err(_) => {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "no assistant_usage_events table; not a Copilot CLI store".to_string(),
                });
            }
        };
        let mut rows = match stmt.query([]) {
            Ok(rows) => rows,
            Err(err) if common::is_busy_like(&err) => return Ok(DetectOutcome::Pending),
            Err(_) => {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "table probe failed".to_string(),
                });
            }
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            if let Ok(name) = row.get::<_, String>(1) {
                out.push(name);
            }
        }
        out
    };
    let missing: Vec<&str> = common::USAGE_EVENT_COLUMNS
        .iter()
        .filter(|c| !columns.iter().any(|col| col == *c))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: format!("assistant_usage_events missing required columns: {missing:?}"),
        });
    }
    // 列集吻合后按 schema_version 表定版本依据（V30，本机实测 version=8）：
    // 已收录版本 KnownVersion；其他/缺失 LatestFallback 兼容尝试，不虚标已验证。
    let found = common::read_schema_version(&conn);
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: COPILOT_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}
