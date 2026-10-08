//! OpenClaw-specific shared helpers under the independent adapter-directory convention:
//! - Read-only database helpers copied from `adapters/kilo/common.rs`; each Agent keeps
//!   its own module with product-specific prefixes and comments.
//! - Read-only schema/ownership checks inspect columns and primary metadata, without transcripts.
//!
//! References and checks: A09, official docs.openclaw.ai, checked 2026-09-24.
//! - Store: one `~/.openclaw/agents/<agentId>/agent/openclaw-agent.sqlite` per Agent,
//!   with session rows and append-only transcripts; legacy `sessions/` and
//!   `sessions/sessions.json` are migration/archive inputs not imported at Gateway startup.
//! - Schema 24, official 2026.9.8 installation and real CLI samples checked; full rules:
//!   docs/design/desktop-usage/openclaw-runtime.md.

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

/// Staging limits follow the kilo/hermes convention.
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

fn backup_to_staging(
    source: &Connection,
    limits: &StagingLimits,
) -> Result<PathBuf, rusqlite::Error> {
    let page_size =
        u64::from(source.query_row("PRAGMA page_size", [], |r| r.get::<_, i64>(0))? as u32);
    let max_pages = (limits.max_bytes / page_size.max(1)).min(i64::MAX as u64) as i64;
    let dest_path = std::env::temp_dir().join(format!(
        "llm-usage-openclaw-staging-{}-{}.db",
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
                    Some("openclaw staging backup timed out".to_string()),
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
                            Some("openclaw staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

pub(crate) fn short_probe(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// Check structure/ownership only; database schema does not identify historical client versions.
pub(crate) fn schema_probe(conn: &Connection, agent: &str) -> Result<Option<i64>, CoreError> {
    use rusqlite::OptionalExtension;
    for (table, required) in [
        (
            "schema_meta",
            &["meta_key", "role", "schema_version", "agent_id"][..],
        ),
        (
            "session_windows",
            &[
                "session_id",
                "session_key",
                "session_entry_provenance",
                "acp_owned",
                "plugin_owner_id",
                "hook_external_content_source",
                "agent_harness_id",
            ][..],
        ),
        (
            "transcript_events",
            &[
                "session_id",
                "seq",
                "event_json",
                "created_at",
                "event_zstd",
                "event_utf8_bytes",
                "navigation_json",
            ][..],
        ),
        ("session_transcript_cold_archives", &["session_id"][..]),
    ] {
        let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns: std::collections::BTreeSet<String> = statement
            .query_map([], |row| row.get(1))?
            .collect::<Result<_, _>>()?;
        if !required.iter().all(|name| columns.contains(*name)) {
            return Ok(None);
        }
    }
    let metadata: Option<(String, i64, Option<String>)> = conn
        .query_row(
            "SELECT role,schema_version,agent_id FROM schema_meta WHERE meta_key='primary'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(metadata
        .filter(|(role, schema, owner)| {
            role == "agent" && *schema == version && owner.as_deref() == Some(agent)
        })
        .map(|(_, schema, _)| schema))
}
