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
        // assistant_usage_events 缺失/列不全：先判定是否为新版 Copilot
        // chronicle 会话库（2026-09-30 本机核验 + 官方 cli-config-dir-reference：
        // 最新 CLI 的 session-store.db 已改为 checkpoint/search 索引，
        // 用量载体外移）。命中则确认为 Copilot 身份但该版本无逐次用量，
        // 记不兼容版本（不回退、不虚报“非 Copilot 库”）。
        if is_chronicle_store(&conn) {
            let found = conn
                .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| {
                    r.get::<_, i64>(0)
                })
                .ok()
                .map(|v| format!("session-store-chronicle-v{v}"));
            return Ok(DetectOutcome::UnsupportedVersion {
                format: COPILOT_FORMAT.to_string(),
                found,
                reason: "session-store.db 为新版 Copilot chronicle 库（sessions/turns/\
                         checkpoints/search_index），已不含 assistant_usage_events：\
                         最新版用量载体外移。session-state/<id>/events.jsonl 事件日志\
                         经同族 copilot-agent 转录核验无逐次 token 字段，本地无已验证\
                         逐次用量——需真实最新 CLI 样本或启用 OTel 载体后再锚定"
                    .to_string(),
            });
        }
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

/// 新版 Copilot session-store.db（checkpoint/search 索引）指纹：
/// sessions/turns/checkpoints/search_index 四表齐备。用于把“最新版布局”与
/// “非 Copilot 库”区分开（前者记不兼容版本，后者才是 UnknownFormat）。
fn is_chronicle_store(conn: &rusqlite::Connection) -> bool {
    let names = table_names(conn);
    ["sessions", "turns", "checkpoints", "search_index"]
        .iter()
        .all(|t| names.contains(*t))
}

fn table_names(conn: &rusqlite::Connection) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    if let Ok(mut stmt) = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'") {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let Ok(name) = row.get::<_, String>(0) {
                    out.insert(name);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str, schema: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-copilot-detect-{}-{}-{}.db",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(schema).unwrap();
        path
    }

    #[test]
    fn new_chronicle_store_is_unsupported_version_not_unknown_format() {
        // 最新 CLI 的 session-store.db：chronicle/search 索引，无 assistant_usage_events。
        let path = temp_db(
            "chronicle",
            "CREATE TABLE schema_version (version INTEGER);
             INSERT INTO schema_version VALUES (3);
             CREATE TABLE sessions (id TEXT);
             CREATE TABLE turns (id INTEGER);
             CREATE TABLE checkpoints (id INTEGER);
             CREATE TABLE search_index (content TEXT);
             CREATE TABLE session_files (id INTEGER);",
        );
        match detect(&path).unwrap() {
            DetectOutcome::UnsupportedVersion { format, found, .. } => {
                assert_eq!(format, COPILOT_FORMAT);
                assert_eq!(found.as_deref(), Some("session-store-chronicle-v3"));
            }
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unrelated_sqlite_is_unknown_format() {
        let path = temp_db("unrelated", "CREATE TABLE foo (a INTEGER, b TEXT);");
        assert!(matches!(
            detect(&path).unwrap(),
            DetectOutcome::UnknownFormat { .. }
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_usage_events_still_supported() {
        let path = temp_db(
            "legacy",
            "CREATE TABLE schema_version (version INTEGER);
             INSERT INTO schema_version VALUES (8);
             CREATE TABLE assistant_usage_events (
               id INTEGER PRIMARY KEY, session_id TEXT, turn_index INTEGER, model TEXT,
               input_tokens INTEGER, output_tokens INTEGER,
               cache_read_tokens INTEGER, cache_write_tokens INTEGER,
               reasoning_tokens INTEGER, duration_ms REAL,
               time_to_first_token_ms REAL, created_at TEXT);",
        );
        assert!(matches!(
            detect(&path).unwrap(),
            DetectOutcome::Supported { .. }
        ));
        let _ = std::fs::remove_file(&path);
    }
}
