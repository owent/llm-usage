//! GitHub Copilot CLI helpers under the independent adapter-directory convention.
//! - Usage mapping moved from root usage_map.rs; V30 keeps product rules with the adapter.
//! - Read-only database helpers copied from adapters/goose/common.rs, following hermes/kilo.
//!
//! Field references: M0 m0-agent-fixtures.md and real local checks on 2026-09-29:
//! ~/.copilot/session-store.db schema_version=8; all 36 rows matched these rules.
//! - assistant_usage_events input_tokens = uncached + cache_read + cache_write.
//!   Derive input_uncached by subtraction; output_tokens is reported. reasoning_tokens is
//!   separate, with unverified output inclusion: retain it without adding it to output/totals.
//! - request_multiplier, observed as 27.0, is a premium multiplier rather than tokens.
//! - total_nano_aiu measures nano AIU, rather than tokens; exclude it from token statistics.
//! - created_at is ISO8601; duration_ms/time_to_first_token_ms are REAL milliseconds.
//! - M0 events.jsonl has no per-call tokens and is excluded; do not read conversation columns.

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Five assistant_usage_events token fields; NULL remains unknown.
#[derive(Debug, Clone, Copy, Default)]
pub struct CopilotUsage {
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
}

/// Verified input = uncached + cache read + cache write; derive uncached by subtraction.
pub fn map_copilot(raw: &CopilotUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = match (
        raw.input_tokens,
        raw.cached_input_tokens,
        raw.cache_creation_input_tokens,
    ) {
        (Some(total), Some(read), Some(write)) => {
            // Both cache buckets may reach MAX_TOKEN_VALUE; overflow stops derivation and is diagnosed.
            // Do not pass distorted saturating sums into later calculations.
            match read.checked_add(write) {
                Some(sum) => sub_checked("input_uncached", total, sum, &mut diagnostics),
                None => {
                    diagnostics.push(crate::metrics::Contradiction {
                        code: "negative_derived_field",
                        field: "input_uncached",
                        detail: format!(
                            "cache buckets overflow: {read} + {write}; uncached not derived"
                        ),
                    });
                    None
                }
            }
        }
        _ => None,
    };
    let total = match (raw.input_tokens, raw.output_tokens) {
        (Some(i), Some(o)) => match i.checked_add(o) {
            Some(t) if t <= crate::domain::MAX_TOKEN_VALUE => Some(t),
            _ => {
                diagnostics.push(crate::metrics::Contradiction {
                    code: "token_shape_deviation",
                    field: "total_tokens",
                    detail: format!("derived total {i} + {o} out of range; kept unknown"),
                });
                None
            }
        },
        _ => None,
    };
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: raw.cached_input_tokens,
        input_cache_write: raw.cache_creation_input_tokens,
        input_total: raw.input_tokens,
        output_total: raw.output_tokens,
        // Reasoning/output inclusion is unverified; report separately without adding reasoning to totals.
        output_reasoning: raw.reasoning_tokens,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: if uncached.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        input_cache_read: raw
            .cached_input_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_cache_write: raw
            .cache_creation_input_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_total: raw.input_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: raw.output_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_reasoning: raw
            .reasoning_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        total_tokens: if total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        source_total: Q::Unknown,
    };
    finish(usage, quality, diagnostics)
}

// Independent read-only database helpers copied from adapters/hermes/common.rs.

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

/// Online Backup creates a consistent system-temp snapshot without writing the source.
fn backup_to_staging(
    source: &Connection,
    limits: &StagingLimits,
) -> Result<PathBuf, rusqlite::Error> {
    let page_size =
        u64::from(source.query_row("PRAGMA page_size", [], |r| r.get::<_, i64>(0))? as u32);
    let max_pages = (limits.max_bytes / page_size.max(1)).min(i64::MAX as u64) as i64;
    let dest_path = std::env::temp_dir().join(format!(
        "llm-usage-copilot-staging-{}-{}.db",
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
                    Some("copilot staging backup timed out".to_string()),
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
                            Some("copilot staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// Read the source database directly; busy-like probe failures try staging, other errors propagate.
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

/// Short sqlite_master count probe, following kilo; it creates no explicit transaction.
pub(crate) fn short_probe(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// Required usage-event columns; thread metadata is not this table's schema.
/// assistant_usage_events columns verified against local schema 8 and M0 1.0.73.
pub(crate) const USAGE_EVENT_COLUMNS: &[&str] = &[
    "id",
    "session_id",
    "turn_index",
    "model",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "reasoning_tokens",
    "duration_ms",
    "time_to_first_token_ms",
    "created_at",
];

/// Read schema_version.version into a registry format string; local sample reports 8.
/// Shared by detection/scanning: missing table or failed query returns None for LatestFallback.
pub(crate) fn read_schema_version(conn: &Connection) -> Option<String> {
    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| {
            r.get(0)
        })
        .ok()?;
    Some(format!("assistant-usage-events-v{version}"))
}

#[cfg(test)]
mod control_tests {
    use super::*;
    #[test]
    fn backup_checks_pause_between_pages_and_removes_partial_copy() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("CREATE TABLE chunks(value BLOB); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<16) INSERT INTO chunks SELECT zeroblob(8192) FROM n").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let gate = calls.clone();
        let _scope = crate::adapters::run_policy::enter(
            None,
            Some(Arc::new(move || gate.fetch_add(1, Ordering::SeqCst) == 0)),
        );
        let limits = StagingLimits {
            pages_per_step: 1,
            ..Default::default()
        };
        let error = backup_to_staging(&source, &limits).unwrap_err();
        assert!(
            matches!(error, rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::OperationInterrupted)
        );
        assert!(calls.load(Ordering::SeqCst) >= 2);
        assert_eq!(
            source
                .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            16
        );
        let prefix = format!("llm-usage-copilot-staging-{}-", std::process::id());
        assert!(!std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with(&prefix)));
    }
}
