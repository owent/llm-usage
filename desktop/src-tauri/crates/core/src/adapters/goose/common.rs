//! Goose-specific helpers under the independent adapter-directory convention.
//! - Read-only DB access copied from adapters/zed/common.rs, following hermes/kilo.
//! - Per-request usage_ledger mapping: official a701bb1, A26.
//!
//! Fixed source: aaif-goose/goose a701bb1756f0c6a49a7dbc10ac8a90f94dd24bd1.
//! - usage_ledger migration 15 (session_manager.rs:1083-1097) inserts one row per
//!   provider response (:913); created_timestamp is Unix seconds. cost_source is
//!   provider_reported/estimated/carried_forward; is_compaction marks compaction calls.
//! - Usage (goose-provider-types token_usage.rs:93-101): input_tokens is the entire
//!   prompt including cache-read/write subsets; derive input_uncached by subtraction
//!   with sub_checked rejecting negative results.
//! - sessions.accumulated_* contains session totals; other columns are last snapshots.
//!   Fall back to session aggregates only in older databases without usage_ledger (schema <15).

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// usage_ledger token columns; NULL remains unknown without zero substitution.
#[derive(Debug, Clone, Copy, Default)]
pub struct GooseLedgerUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

/// Derive uncached from cache-inclusive input; retain reported total for comparison.
pub fn map_goose_ledger(raw: &GooseLedgerUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = match (
        raw.input_tokens,
        raw.cache_read_tokens,
        raw.cache_write_tokens,
    ) {
        (Some(total), Some(r), Some(w)) => sub_checked(
            "input_uncached",
            total,
            r.saturating_add(w),
            &mut diagnostics,
        ),
        _ => None,
    };
    let derived_total = match (raw.input_tokens, raw.output_tokens) {
        (Some(i), Some(o)) => i.checked_add(o),
        _ => None,
    };
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: raw.cache_read_tokens,
        input_cache_write: raw.cache_write_tokens,
        input_total: raw.input_tokens,
        output_total: raw.output_tokens,
        output_reasoning: None,
        total_tokens: derived_total,
        source_total: raw.total_tokens,
    };
    let quality = TokenQuality {
        input_uncached: if uncached.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        input_cache_read: raw
            .cache_read_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_cache_write: raw
            .cache_write_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_total: raw.input_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: raw.output_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        source_total: raw.total_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        ..Default::default()
    };
    finish(usage, quality, diagnostics)
}

// Read-only database helpers copied from adapters/hermes/common.rs; modules remain independent.

use crate::error::CoreError;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Read-only source access; busy-like probe failures switch to a staging snapshot.
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
    pub pages_per_step: i32,
    pub max_bytes: u64,
    pub max_time: Duration,
}

impl Default for StagingLimits {
    fn default() -> Self {
        StagingLimits {
            pages_per_step: 512,
            max_bytes: 2 * 1024 * 1024 * 1024,
            max_time: Duration::from_secs(30),
        }
    }
}

/// Open read-only; short busy_timeout bounds probe waits before a staging fallback.
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

/// Online Backup creates a consistent system-temporary snapshot without writing the source.
fn backup_to_staging(
    source: &Connection,
    limits: &StagingLimits,
) -> Result<PathBuf, rusqlite::Error> {
    let page_size =
        u64::from(source.query_row("PRAGMA page_size", [], |r| r.get::<_, i64>(0))? as u32);
    let max_pages = (limits.max_bytes / page_size.max(1)).min(i64::MAX as u64) as i64;
    let dest_path = std::env::temp_dir().join(format!(
        "llm-usage-goose-staging-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let started = Instant::now();
    let attempt = || -> Result<(), rusqlite::Error> {
        let mut dest = Connection::open(&dest_path)?;
        let backup = Backup::new(source, &mut dest)?;
        let mut done_pages: i64 = 0;
        loop {
            if started.elapsed() >= limits.max_time {
                break Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    Some("goose staging backup timed out".to_string()),
                ));
            }
            crate::adapters::run_policy::check_sqlite()?;
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                // More means pages were copied; count those pages toward the size limit.
                Ok(StepResult::More) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("goose staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// Probe read-only sessions.db; busy-like probe failures use staging, later failures propagate.
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

/// Short schema query probes access, following kilo's rule.
pub(crate) fn short_probe(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// Required usage_ledger columns follow the fixed migration-15 schema.
/// Include each column from the fixed usage_ledger creation SQL.
pub(crate) const LEDGER_COLUMNS: &[&str] = &[
    "id",
    "session_id",
    "created_timestamp",
    "model",
    "input_tokens",
    "output_tokens",
    "total_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "cost",
    "cost_source",
    "is_compaction",
];
/// Legacy sessions fallback requires accumulated_* columns when usage_ledger is absent.
pub(crate) const SESSIONS_FALLBACK_COLUMNS: &[&str] = &[
    "id",
    "accumulated_total_tokens",
    "accumulated_input_tokens",
    "accumulated_output_tokens",
    "created_at",
    "updated_at",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncached_derived_from_input_minus_cache() {
        // Input includes cache read/write: uncached = 100 - 30 - 10 = 60.
        let mapped = map_goose_ledger(&GooseLedgerUsage {
            input_tokens: Some(100),
            output_tokens: Some(20),
            total_tokens: Some(120),
            cache_read_tokens: Some(30),
            cache_write_tokens: Some(10),
        });
        assert_eq!(mapped.usage.input_total, Some(100));
        assert_eq!(mapped.usage.input_uncached, Some(60));
        assert_eq!(mapped.usage.total_tokens, Some(120));
        assert_eq!(mapped.usage.source_total, Some(120));
    }

    #[test]
    fn negative_derived_reports_contradiction_not_clamped() {
        // Cache exceeds input: leave uncached unknown with a contradiction diagnostic, without zero replacement.
        let mapped = map_goose_ledger(&GooseLedgerUsage {
            input_tokens: Some(10),
            cache_read_tokens: Some(8),
            cache_write_tokens: Some(8),
            ..Default::default()
        });
        assert_eq!(mapped.usage.input_uncached, None);
        assert!(mapped
            .diagnostics
            .iter()
            .any(|d| d.code == "negative_derived_field"));
    }

    #[test]
    fn missing_buckets_stay_unknown() {
        // NULL columns remain unknown without zero substitution.
        let mapped = map_goose_ledger(&GooseLedgerUsage {
            input_tokens: Some(50),
            ..Default::default()
        });
        assert_eq!(mapped.usage.output_total, None);
        assert_eq!(mapped.usage.total_tokens, None);
        assert_eq!(mapped.usage.input_uncached, None);
    }
}
