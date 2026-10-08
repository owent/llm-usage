//! Detect Copilot CLI through session-store.db assistant_usage_events columns.

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
        // Transient antivirus/product locks cause Pending and retry on a later scan;
        // do not persist them as UnknownFormat.
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
        // If assistant_usage_events/required columns are absent, check the newer Copilot
        // chronicle store verified locally on 2026-09-30 and in cli-config-dir-reference.
        // The checked session-store.db is a checkpoint/search index;
        // per-call usage moved elsewhere. Matching tables identify Copilot but lack verified per-call usage.
        // Return UnsupportedVersion without fallback, rather than misidentifying it as unrelated SQLite.
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
    // Matching columns use schema_version for V30 selection; native checked version=8.
    // Registered versions use KnownVersion; absent/other versions try LatestFallback without claiming verified support.
    let found = common::read_schema_version(&conn);
    let selection = versions::select(found.as_deref());
    Ok(DetectOutcome::Supported {
        format: COPILOT_FORMAT.to_string(),
        format_version: found,
        basis: selection.basis,
    })
}

/// Newer Copilot checkpoint/search schema requires all four tables:
/// sessions/turns/checkpoints/search_index. It distinguishes chronicle
/// (UnsupportedVersion) from unrelated databases (UnknownFormat).
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
        // Checked newer CLI layout: chronicle/search index, without assistant_usage_events.
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
