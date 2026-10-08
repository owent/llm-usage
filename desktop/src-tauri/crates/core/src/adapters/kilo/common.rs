//! Kilo Code CLI helpers moved from root usage_map.rs under the V30 directory convention.
//! - map_kilo/KiloUsage maps mutually exclusive usage buckets.
//! - Read-only source SQLite access follows architecture.md#database:
//!   direct connection/short probe, with Online Backup to system temp on busy-like probe failure.
//!   Staging has page/time/size limits and cleanup; errors propagate so existing results survive.
//!   Never checkpoint or alter source journal/schema, or create source-side files.
//!
//! Cross-Agent usage_map.rs retains MappedUsage/finish/sub_checked and contradiction checks.

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::metrics::Contradiction;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// OpenCode-derived Kilo message.data.tokens has five mutually exclusive components:
/// total = input + output + reasoning + cache.read + cache.write, verified in
/// m0-agent-fixtures.md. Add reasoning to canonical output_total as derived output
/// to satisfy normalized total_tokens = input_total + output_total when required buckets are known.
///
/// total is optional: unfinished/failed messages may omit it (two/one records in real
/// 7.4.8/7.4.9 samples; 42 of 13,342 local assistant rows). Missing source_total remains
/// None, without zero-filling; complete known components can still derive total.
#[derive(Debug, Clone, Copy)]
pub struct KiloUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: Option<i64>,
    pub cache_read: i64,
    pub cache_write: i64,
    pub total: Option<i64>,
}

pub fn map_kilo(raw: &KiloUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let input_total = raw
        .input
        .checked_add(raw.cache_read)
        .and_then(|v| v.checked_add(raw.cache_write));
    let output_total = raw
        .reasoning
        .and_then(|reasoning| raw.output.checked_add(reasoning));
    let derived_total = input_total.and_then(|i| output_total.and_then(|o| i.checked_add(o)));
    if let (Some(dt), Some(total)) = (derived_total, raw.total) {
        if dt != total {
            diagnostics.push(Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: format!("kilo total {total} != derived sum {dt}"),
            });
        }
    }
    let usage = TokenUsage {
        input_uncached: Some(raw.input),
        input_cache_read: Some(raw.cache_read),
        input_cache_write: Some(raw.cache_write),
        input_total,
        output_total,
        output_reasoning: raw.reasoning,
        total_tokens: derived_total.or(raw.total),
        source_total: raw.total,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: if output_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_reasoning: if raw.reasoning.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Reported
        },
        source_total: if raw.total.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
    };
    finish(usage, quality, diagnostics)
}

/// Read-only source access; busy-like probe failures switch to a staging snapshot.
/// _staging owns the snapshot guard; drop cleans up even after query failure.
pub struct SourceDb {
    conn: Connection,
    _staging: Option<StagingGuard>,
}

impl SourceDb {
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

struct StagingGuard {
    path: PathBuf,
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
    }
}

/// SQLite busy/locked/CANTOPEN errors prevent a successful consistency probe.
pub(crate) fn is_busy_like(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::DatabaseBusy)
            | Some(rusqlite::ffi::ErrorCode::DatabaseLocked)
            | Some(rusqlite::ffi::ErrorCode::CannotOpen)
    )
}

/// Staging page/time/size limits and cleanup follow architecture.md.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StagingLimits {
    /// Pages copied per backup step, bounding work while holding locks.
    pub pages_per_step: i32,
    /// Snapshot byte limit; abort oversized backups rather than filling the disk.
    pub max_bytes: u64,
    /// Overall backup timeout.
    pub max_time: Duration,
}

impl Default for StagingLimits {
    fn default() -> Self {
        StagingLimits {
            pages_per_step: 512,
            // Default 2 GiB staging limit; do not force-copy larger live databases, preserving prior results.
            max_bytes: 2 * 1024 * 1024 * 1024,
            // Default 30-second staging timeout; exceeding it returns busy rather than completing the snapshot.
            max_time: Duration::from_secs(30),
        }
    }
}

/// Open read-only with short busy_timeout so probe waits are bounded
/// without blocking the collection thread for long periods.
pub(crate) fn open_readonly(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.busy_timeout(Duration::from_millis(150))?;
    crate::adapters::run_policy::install_sqlite_control(&conn)?;
    Ok(conn)
}

/// Online Backup creates a consistent system-temp snapshot without writing the source.
/// Copy in steps, aborting at limits; close destination handles before removing partial files.
/// Windows remove_file can otherwise fail because an open handle denies shared deletion.
fn backup_to_staging(
    source: &Connection,
    limits: &StagingLimits,
) -> Result<PathBuf, rusqlite::Error> {
    let page_size =
        u64::from(source.query_row("PRAGMA page_size", [], |r| r.get::<_, i64>(0))? as u32);
    let max_pages = (limits.max_bytes / page_size.max(1)).min(i64::MAX as u64) as i64;
    let dest_path = std::env::temp_dir().join(format!(
        "llm-usage-kilo-staging-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let started = Instant::now();
    // ? exits this closure only; every failure reaches common cleanup after handles close.
    let attempt = || -> Result<(), rusqlite::Error> {
        let mut dest = Connection::open(&dest_path)?;
        let backup = Backup::new(source, &mut dest)?;
        let mut done_pages: i64 = 0;
        loop {
            if started.elapsed() >= limits.max_time {
                break Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    Some("kilo staging backup timed out".to_string()),
                ));
            }
            crate::adapters::run_policy::check_sqlite()?;
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                // StepResult is non-exhaustive; retryable results remain within the configured limits.
                // More means pages were copied; count those pages toward the size limit.
                Ok(StepResult::More) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("kilo staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                // Busy/Locked or other non-exhaustive results retry without progress, within the timeout.
                // 2026-09-30 fix: retries previously added pages and raced the timeout, causing
                // platform-specific false size-limit failures (Linux CI hit the page cap before timeout).
                Ok(_) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => break Err(e),
            }
        }
    };
    // Destination/backup handles have closed with the closure; they no longer prevent removal.
    match attempt() {
        Ok(()) => Ok(dest_path),
        Err(e) => {
            let _ = std::fs::remove_file(&dest_path);
            let _ = std::fs::remove_file(format!("{}-wal", dest_path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", dest_path.display()));
            Err(e)
        }
    }
}

/// Open kilo.db for read-only access:
///
/// 1. Open directly and run the short probe; this function creates no explicit read transaction.
/// 2. On busy-like probe failure, create/read an Online Backup snapshot.
/// 3. Other failures become CoreError; callers retain existing results and report the error.
///
/// Never write the source database.
pub(crate) fn open_source_db<F>(
    path: &Path,
    probe: F,
    limits: &StagingLimits,
) -> Result<SourceDb, CoreError>
where
    F: FnOnce(&Connection) -> Result<(), rusqlite::Error>,
{
    let conn = open_readonly(path).map_err(CoreError::Sqlite)?;
    match probe(&conn) {
        Ok(()) => Ok(SourceDb {
            conn,
            _staging: None,
        }),
        Err(e) if is_busy_like(&e) => {
            // The consistency probe failed; back up through the same read-only source connection.
            let staging_path = backup_to_staging(&conn, limits).map_err(CoreError::Sqlite)?;
            let staged = open_readonly(&staging_path).map_err(CoreError::Sqlite)?;
            Ok(SourceDb {
                conn: staged,
                _staging: Some(StagingGuard { path: staging_path }),
            })
        }
        Err(e) => Err(CoreError::Sqlite(e)),
    }
}

/// kilo.db schema fingerprint covers required tables/columns, without records or conversations.
/// Require five message columns and eight session columns, including five reconciliation token columns.
pub(crate) const REQUIRED_MESSAGE_COLUMNS: &[&str] =
    &["id", "session_id", "time_created", "time_updated", "data"];
pub(crate) const REQUIRED_SESSION_COLUMNS: &[&str] = &[
    "id",
    "parent_id",
    "version",
    "tokens_input",
    "tokens_output",
    "tokens_reasoning",
    "tokens_cache_read",
    "tokens_cache_write",
];

/// Missing tables/columns return None; the persisted fingerprint contains only table/column names.
pub(crate) fn schema_fingerprint(conn: &Connection) -> Result<Option<String>, CoreError> {
    let table_exists = |name: &str| -> Result<bool, CoreError> {
        let found: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
            .map_err(CoreError::Sqlite)?;
        Ok(found.is_some())
    };
    if !table_exists("message")? || !table_exists("session")? {
        return Ok(None);
    }
    let columns_of = |table: &str| -> Result<Vec<String>, CoreError> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(CoreError::Sqlite)?;
        let names = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(CoreError::Sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CoreError::Sqlite)?;
        Ok(names)
    };
    let message_columns = columns_of("message")?;
    let session_columns = columns_of("session")?;
    let has_all = |columns: &[String], required: &[&str]| {
        required.iter().all(|r| columns.iter().any(|c| c == r))
    };
    if !has_all(&message_columns, REQUIRED_MESSAGE_COLUMNS)
        || !has_all(&session_columns, REQUIRED_SESSION_COLUMNS)
    {
        return Ok(None);
    }
    Ok(Some(format!(
        "message({})|session({})",
        REQUIRED_MESSAGE_COLUMNS.join(","),
        REQUIRED_SESSION_COLUMNS.join(",")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_kilo_five_way_exclusive_with_optional_total() {
        let m = map_kilo(&KiloUsage {
            input: 100,
            output: 60,
            reasoning: Some(40),
            cache_read: 800,
            cache_write: 100,
            total: Some(1100),
        });
        assert_eq!(m.usage.input_total, Some(1000));
        assert_eq!(m.usage.output_total, Some(100));
        assert_eq!(m.usage.total_tokens, Some(1100));
        assert_eq!(m.usage.source_total, Some(1100));
        assert!(m.diagnostics.is_empty());

        // Missing native total keeps source_total unknown while known components can still derive total.
        let missing = map_kilo(&KiloUsage {
            input: 10,
            output: 5,
            reasoning: Some(2),
            cache_read: 0,
            cache_write: 0,
            total: None,
        });
        assert_eq!(missing.usage.source_total, None);
        assert_eq!(missing.quality.source_total, Q::Unknown);
        assert_eq!(missing.usage.total_tokens, Some(17));

        // Diagnose disagreement between comparable reported and derived totals.
        let bad = map_kilo(&KiloUsage {
            input: 10,
            output: 5,
            reasoning: Some(0),
            cache_read: 0,
            cache_write: 0,
            total: Some(99),
        });
        assert_eq!(bad.usage.total_tokens, Some(15), "规范化总量按派生口径");
        assert_eq!(bad.usage.source_total, Some(99), "直报值独立保留");
        assert!(bad
            .diagnostics
            .iter()
            .any(|d| d.code == "source_total_mismatch"));
    }

    #[test]
    fn map_kilo_overflow_degrades_to_unknown_not_panic() {
        let m = map_kilo(&KiloUsage {
            input: i64::MAX,
            output: 1,
            reasoning: None,
            cache_read: 1,
            cache_write: 0,
            total: None,
        });
        // Overflowed derived fields stay None; do not panic or truncate numeric values.
        assert_eq!(m.usage.input_total, None);
        assert_eq!(m.usage.total_tokens, None);
    }
}
