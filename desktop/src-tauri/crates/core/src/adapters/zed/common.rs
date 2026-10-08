//! Zed-specific helpers; independent layout: architecture.md#adapter-layout.
//! Verified 1.22.0 / 76659a55 OpenAI-compatible mapping stores input after removing cache reads.
//! For llm-usage-zhipu, positive buckets are reported; default zero/absence is unknown, no total derived.
//! The bd74733 references and omitted-zero rules below describe only the historical hosted mapper.
//! - map_zed/ZedUsage field references: official Zed bd74733, A38.
//! - Read-only threads.db helpers copied from adapters/hermes/common.rs, matching kilo:
//!   read-only connections/Online Backup snapshots in independent Agent modules.
//!
//! Fixed source: zed-industries/zed bd747337d7be138834e20972b9e203c7b239cc47.
//! - crates/agent/src/db.rs:451-483: threads id/summary/updated_at/data_type/data plus
//!   migrated parent_id/folder_paths/folder_paths_order/created_at.
//!   data_type is json or zstd (db.rs:363-385); the referenced writer uses zstd (db.rs:536).
//! - crates/language_model_core/src/language_model_core.rs:525-535 defines
//!   four flat u64 TokenUsage fields: input_tokens/output_tokens/cache_creation_input_tokens/
//!   cache_read_input_tokens. Zero fields are omitted by skip_serializing_if,
//!   so the historical mapper treats absence as reported zero under that serialization rule.
//! - Input/cache inclusion is not declared in that source; do not infer exclusive/subset buckets.
//!   The historical mapper reports them separately without deriving totals.
//! - That hosted path accepts provider=="zed.dev" (A38). Within a turn, request_token_usage
//!   overwrites earlier requests (thread.rs:2893 reset), so summing it misses usage.
//!   Use cumulative_token_usage for thread totals; request buckets reconcile only.

use crate::adapters::usage_map::{finish_parallel, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// Historical hosted TokenUsage in threads.data; omitted fields decode as zero.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZedUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
}

impl ZedUsage {
    /// Checked four-bucket sum; each may reach MAX_TOKEN_VALUE.
    /// Overflow returns None so the caller can reject the thread rather than saturate.
    pub fn total(&self) -> Option<i64> {
        self.input_tokens
            .checked_add(self.output_tokens)?
            .checked_add(self.cache_read_input_tokens)?
            .checked_add(self.cache_creation_input_tokens)
    }
}

/// Historical hosted input/cache inclusion is unverified: uncached/derived totals remain None.
/// Report all four buckets separately; the current Hermes mapper has independent verified rules.
pub fn map_zed(raw: &ZedUsage) -> MappedUsage {
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: Some(raw.cache_read_input_tokens),
        input_cache_write: Some(raw.cache_creation_input_tokens),
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: None,
        total_tokens: None,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Unknown,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: Q::Unknown,
        source_total: Q::Unknown,
    };
    finish_parallel(usage, quality, Vec::new())
}

/// Zed 1.22.0 OpenAI chat mapping subtracts cache reads from prompt_tokens.
/// Serialized default zeros lack a provider validity marker.
pub fn map_verified_openai(raw: &ZedUsage) -> MappedUsage {
    let positive = |n: i64| (n > 0).then_some(n);
    let usage = TokenUsage {
        input_uncached: positive(raw.input_tokens),
        input_cache_read: positive(raw.cache_read_input_tokens),
        input_cache_write: positive(raw.cache_creation_input_tokens),
        output_total: positive(raw.output_tokens),
        ..Default::default()
    };
    let q = |value: Option<i64>| {
        if value.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        }
    };
    let quality = TokenQuality {
        input_uncached: q(usage.input_uncached),
        input_cache_read: q(usage.input_cache_read),
        input_cache_write: q(usage.input_cache_write),
        output_total: q(usage.output_total),
        ..Default::default()
    };
    finish_parallel(usage, quality, Vec::new())
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
        "llm-usage-zed-staging-{}-{}.db",
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
                    Some("zed staging backup timed out".to_string()),
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
                            Some("zed staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// Read threads.db directly; busy-like probe failures try staging, other errors propagate.
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

/// Required threads columns from fixed creation/migrations; detection separately handles absent created_at.
pub(crate) const REQUIRED_THREADS_COLUMNS: &[&str] =
    &["id", "summary", "updated_at", "data_type", "data"];
