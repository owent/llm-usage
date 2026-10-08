//! MiMo Code-specific helpers; independent layout: architecture.md#adapter-layout.
//! - Schema fingerprint requires message.agent_id. The referenced MiMo session has no
//!   tokens_* totals; OpenCode's referenced schema lacks agent_id and cannot match.
//! - Read-only state database access with Online Backup staging snapshots:
//!   helpers copied from `adapters/kilo/common.rs`; each Agent keeps its own module.
//!   The Kilo helper originated in the checked M3 implementation; prefixes/comments differ.
//!
//! Fixed source: A14, commit 456678b6a5afb0eef3fe2754575637218cfb3c84.
//! - packages/shared/src/global.ts resolveMimocodeHome uses MIMOCODE_HOME,
//!   which must be absolute, with <home>/{data,cache,config,state}; otherwise use XDG
//!   $XDG_DATA_HOME/mimocode, defaulting to ~/.local/share/mimocode.
//! - packages/opencode/src/storage/db.ts defines data/mimocode.db,
//!   or channel variant mimocode-<channel>.db; database uses WAL.
//! - packages/opencode/src/session/session.sql.ts: session includes
//!   parent_id/version without tokens_* totals; message includes
//!   agent_id defaulting to main; part completes the three literal table definitions.
//! - packages/opencode/src/session/message-v2.ts StepFinishPart zod schema:
//!   tokens{total?, input, output, reasoning, cache{read, write}}.

use crate::error::CoreError;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// Read-only database helpers copied from adapters/kilo/common.rs; modules remain independent.

/// Read-only source access; busy-like probe failures switch to a staging snapshot.
/// The staging guard owns its path and cleans up on drop, including query failures.
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
        "llm-usage-mimocode-staging-{}-{}.db",
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
                    Some("mimocode staging backup timed out".to_string()),
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
                            Some("mimocode staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// Probe read-only mimocode.db; busy-like probe failures use staging, later failures propagate.
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

// Schema fingerprint uses fixed session.sql.ts table/column names only.

/// Required per-call part columns use literal session.sql.ts PartTable names.
pub(crate) const REQUIRED_PART_COLUMNS: &[&str] = &[
    "id",
    "message_id",
    "session_id",
    "time_created",
    "time_updated",
    "data",
];
/// Required session identity/parent/version; referenced MiMo lacks cumulative totals,
/// so no session-total reconciliation; OpenCode lacks the required message.agent_id.
pub(crate) const REQUIRED_SESSION_COLUMNS: &[&str] = &["id", "parent_id", "version"];
/// MiMo-specific message.agent_id defaults to main and distinguishes the referenced schemas.
/// Assistant data JSON carries modelID/providerID under message-v2.ts zod definitions.
pub(crate) const REQUIRED_MESSAGE_COLUMNS: &[&str] = &["id", "session_id", "agent_id", "data"];

/// Return schema fingerprint, or None for absent tables/required columns.
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
    if !table_exists("part")? || !table_exists("session")? || !table_exists("message")? {
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
    let part_columns = columns_of("part")?;
    let session_columns = columns_of("session")?;
    let message_columns = columns_of("message")?;
    let has_all = |columns: &[String], required: &[&str]| {
        required.iter().all(|r| columns.iter().any(|c| c == r))
    };
    if !has_all(&part_columns, REQUIRED_PART_COLUMNS)
        || !has_all(&session_columns, REQUIRED_SESSION_COLUMNS)
        || !has_all(&message_columns, REQUIRED_MESSAGE_COLUMNS)
    {
        return Ok(None);
    }
    Ok(Some(format!(
        "part({})|session({})|message({})",
        REQUIRED_PART_COLUMNS.join(","),
        REQUIRED_SESSION_COLUMNS.join(","),
        REQUIRED_MESSAGE_COLUMNS.join(",")
    )))
}
