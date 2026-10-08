//! SQLite storage: one writer, WAL, synchronous=FULL and bounded busy_timeout.
//! Check schema versions without automatic migrations; see architecture.md#database and the data rules.

pub mod pricing;
pub mod schema;

use crate::error::CoreError;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default bounded busy_timeout avoids indefinite waits behind readers/writers.
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

// SQLite's built-in lower/NOCASE handles only ASCII. Apply matching Unicode
// lowercase rules in SQL filters and Rust grouping, including historical rows.
fn register_functions(conn: &Connection) -> Result<(), rusqlite::Error> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "fold_name",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase())),
    )?;
    conn.create_scalar_function(
        "model_key",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(ctx
                .get::<Option<String>>(0)?
                .map(|s| crate::model_names::model_key(&s)))
        },
    )
}

pub struct OpenOptions {
    pub busy_timeout: Duration,
    /// Latest supported schema; None selects schema::SCHEMA_VERSION.
    pub max_supported_version: Option<u32>,
}

impl Default for OpenOptions {
    fn default() -> Self {
        OpenOptions {
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            max_supported_version: None,
        }
    }
}

/// Single-writer storage handle.
pub struct Storage {
    conn: Connection,
    path: PathBuf,
    pub(crate) summary_cache: std::cell::RefCell<crate::summary_cache::SummaryCache>,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Storage").field("path", &self.path).finish()
    }
}

impl Storage {
    /// Open/create the database, configure pragmas and check/create the expected schema;
    /// mark running jobs left by the previous process as interrupted.
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        Self::open_with(path, OpenOptions::default())
    }

    pub fn open_in_memory() -> Result<Self, CoreError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self::setup(conn, PathBuf::from(":memory:"), OpenOptions::default())?;
        Ok(storage)
    }

    pub fn open_with(path: &Path, options: OpenOptions) -> Result<Self, CoreError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        Self::setup(conn, path.to_path_buf(), options)
    }

    /// Read-only connection, alongside one background writer and a few readers.
    /// Do not migrate or persist pragma settings; WAL permits concurrent writer access.
    /// Missing/unreadable databases return errors instead of creating an empty replacement.
    pub fn open_readonly(path: &Path) -> Result<Self, CoreError> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        register_functions(&conn)?;
        conn.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
        Ok(Storage {
            conn,
            path: path.to_path_buf(),
            summary_cache: Default::default(),
        })
    }

    fn setup(conn: Connection, path: PathBuf, options: OpenOptions) -> Result<Self, CoreError> {
        register_functions(&conn)?;
        let supported = options
            .max_supported_version
            .unwrap_or(schema::SCHEMA_VERSION);
        conn.busy_timeout(options.busy_timeout)?;
        let found_version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;

        // Current prerelease policy does not migrate databases version by version.
        // A version mismatch returns an error for application-level rebuild/exit handling.
        // A new database with user_version=0 receives the full schema.
        if found_version == 0 {
            // Distinguish a new database from an unversioned one by checking for the settings table.
            let has_tables: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='settings')",
                [],
                |r| r.get(0),
            )?;
            if !has_tables {
                // Create the schema in one step for a new database.
                conn.execute_batch(schema::FULL_SCHEMA)?;
                conn.pragma_update(None, "user_version", supported)?;
            } else {
                // A settings table without a version identifies an old database requiring rebuilding.
                return Err(CoreError::SchemaTooNew {
                    found: found_version,
                    supported,
                });
            }
        } else if found_version != supported {
            // Prerelease version mismatches require rebuilding rather than automatic migration.
            return Err(CoreError::SchemaMismatch {
                found: found_version,
                expected: supported,
            });
        }

        conn.pragma_update(None, "foreign_keys", "ON")?;
        // journal_mode persists; in-memory databases return memory instead of WAL.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(options.busy_timeout)?;

        // These optional acceleration indexes do not change the schema version.
        // Partial indexes avoid repeated full scans for uncommon coverage gaps.
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_events_unverified_time ON usage_events(occurred_at_ms) WHERE attribution_status!='verified';
            CREATE INDEX IF NOT EXISTS idx_events_carrier_gap_time ON usage_events(occurred_at_ms) WHERE exclusion_reason='copilot_otel_session_authority';
            CREATE INDEX IF NOT EXISTS idx_events_qwen_carrier_gap_time ON usage_events(occurred_at_ms) WHERE exclusion_reason='qwen_sdk_session_authority';
            CREATE INDEX IF NOT EXISTS idx_events_summary_cover ON usage_events(occurred_at_ms,source_instance_id,session_id,duration_ms,record_kind) WHERE attribution_status='verified' AND record_kind IN ('model_call','transport_attempt','usage_observation');")?;

        crate::query_acceleration::install(&conn)?;
        crate::query_acceleration::repair(&conn)?;
        let storage = Storage {
            conn,
            path,
            summary_cache: Default::default(),
        };
        storage.mark_running_jobs_interrupted(crate::jobs::now_ms_fallback())?;
        Ok(storage)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Drop only the optional query cache for explicit uncached measurements.
    pub fn clear_summary_cache(&self) {
        self.summary_cache.borrow_mut().clear();
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current user_version.
    pub fn schema_version(&self) -> Result<u32, CoreError> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Monotonic data revision; queries use one revision for a consistent view.
    pub fn data_revision(&self) -> Result<i64, CoreError> {
        data_revision(&self.conn)
    }

    /// Increment and return the data revision, within a transaction only.
    pub fn bump_data_revision_tx(
        tx: &rusqlite::Transaction<'_>,
        now_ms: i64,
    ) -> Result<i64, CoreError> {
        let next = data_revision(tx)?
            .checked_add(1)
            .ok_or(CoreError::Overflow("data_revision"))?;
        tx.execute(
            "INSERT INTO settings (key, value, schema_version, updated_at_ms)
             VALUES ('data_revision', ?1, 1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            rusqlite::params![next.to_string(), now_ms],
        )?;
        Ok(next)
    }

    /// Recover jobs by marking running as interrupted, without relying on PID files.
    pub fn mark_running_jobs_interrupted(&self, now_ms: i64) -> Result<usize, CoreError> {
        let n = self.conn.execute(
            "UPDATE ingest_runs SET status = 'interrupted', finished_ms = ?1
             WHERE status = 'running'",
            rusqlite::params![now_ms],
        )?;
        Ok(n)
    }

    /// Ensure this source host exists and return its opaque stable ID (data-contract.md#provenance).
    /// - First call persists host-<32hex> in origin_hosts with is_local=1 and
    ///   settings.local_origin_host_id; do not identify hosts by name, IP or hardware fingerprint.
    /// - Later calls return the same ID and record hostname observations; a rename
    ///   creates no new identity or duplicate usage, and names never enter keys.
    /// - Copying a database to another machine does not claim its history automatically.
    ///   Callers supply ownership explicitly; source conflicts require mapping/confirmation (M1a).
    pub fn ensure_local_host(&self, hostname: &str, now_ms: i64) -> Result<String, CoreError> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'local_origin_host_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let host_id = match existing {
            Some(id) => {
                let present: bool = self.conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM origin_hosts WHERE host_id = ?1)",
                    rusqlite::params![id],
                    |r| r.get(0),
                )?;
                if !present {
                    return Err(CoreError::Validation(format!(
                        "local_origin_host_id {id:?} has no origin_hosts row; refusing to guess ownership"
                    )));
                }
                id
            }
            None => {
                let id = format!(
                    "host-{}",
                    self.conn
                        .query_row("SELECT lower(hex(randomblob(16)))", [], |r| {
                            r.get::<_, String>(0)
                        })?
                );
                self.conn.execute(
                    "INSERT INTO origin_hosts (host_id, is_local, note, first_seen_ms, last_seen_ms)
                     VALUES (?1, 1, NULL, ?2, ?2)",
                    rusqlite::params![id, now_ms],
                )?;
                self.conn.execute(
                    "INSERT INTO settings (key, value, schema_version, updated_at_ms)
                     VALUES ('local_origin_host_id', ?1, 1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
                    rusqlite::params![id, now_ms],
                )?;
                id
            }
        };
        self.observe_hostname(&host_id, hostname, now_ms)?;
        Ok(host_id)
    }

    /// Current local host ID, or None before initialization.
    pub fn local_host_id(&self) -> Result<Option<String>, CoreError> {
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'local_origin_host_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id)
    }

    /// Retain historical hostname observations without changing host identity.
    pub fn observe_hostname(
        &self,
        host_id: &str,
        hostname: &str,
        now_ms: i64,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO origin_host_names (host_id, hostname, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(host_id, hostname) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
            rusqlite::params![host_id, hostname, now_ms],
        )?;
        self.conn.execute(
            "UPDATE origin_hosts SET last_seen_ms = ?2 WHERE host_id = ?1",
            rusqlite::params![host_id, now_ms],
        )?;
        Ok(())
    }

    /// Register an imported source host and return host_id; repeated registration keeps the same identity.
    pub fn register_origin_host(
        &self,
        host_id: &str,
        hostname: Option<&str>,
        now_ms: i64,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO origin_hosts (host_id, is_local, note, first_seen_ms, last_seen_ms)
             VALUES (?1, 0, NULL, ?2, ?2)
             ON CONFLICT(host_id) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
            rusqlite::params![host_id, now_ms],
        )?;
        if let Some(name) = hostname {
            self.observe_hostname(host_id, name, now_ms)?;
        }
        Ok(())
    }
}

pub(crate) fn data_revision(conn: &Connection) -> Result<i64, CoreError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'data_revision'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match value {
        Some(v) => v
            .parse::<i64>()
            .map_err(|e| CoreError::Validation(format!("bad data_revision value {v:?}: {e}"))),
        None => Ok(0),
    }
}
