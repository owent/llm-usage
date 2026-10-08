//! Hermes Agent helpers; independent layout: architecture.md#adapter-layout.
//! - map_hermes/HermesUsage uses fixed source column definitions below.
//! - Normalize billing_base_url in memory to a credential-free provider ID or local digest.
//! - Read-only state.db access uses direct connections and Online Backup staging:
//!   helpers copied from adapters/kilo/common.rs, keeping independent Agent modules.
//!   Kilo was accepted in M3; shared access semantics retain product-specific prefixes/comments.
//!
//! Fixed A24 source: ef70b3661cbfcf57e583008ad91dd04d8ba46070.
//! - hermes_state_common.py SCHEMA_SQL defines 18 session_model_usage columns, with
//!   PRIMARY KEY (session_id, model, billing_provider, billing_base_url,
//!   billing_mode, task). sessions includes id/source/parent_session_id/started_at/
//!   ended_at/end_reason and five cumulative buckets; read only required fallback-time columns.
//! - hermes_state_usage.py adds counters by compound key on the incremental path.
//!   absolute updates sessions totals without model rows. record_auxiliary_usage writes
//!   task rows rather than main totals; first_seen is insert-only and last_seen advances
//!   on conflict updates.
//! - hermes_constants.py get_hermes_home uses context override, HERMES_HOME, then
//!   platform default: Windows %LOCALAPPDATA%/hermes, otherwise ~/.hermes.
//!   Named profiles have independent state.db at <root>/profiles/<name>.
//!
//! first_seen/last_seen/started_at/ended_at are REAL Unix seconds from Python
//! time.time(); convert to milliseconds. A24 and fixed 0.21.5 normalize_usage establish
//! uncached input_tokens and reasoning as an output subset.
//! Real request/resume checks passed; initialization/missing fields become zero and remain unknown.

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Five native session_model_usage counters: i64, NOT NULL DEFAULT 0.
#[derive(Debug, Clone, Copy)]
pub struct HermesUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
}

/// Positive normalized native buckets are reported; default zeros cannot establish field presence.
/// Derive input/complete totals only with all required buckets known, without default-zero completion.
pub fn map_hermes(raw: &HermesUsage) -> MappedUsage {
    let known = |v| (v != 0).then_some(v);
    let input_uncached = known(raw.input_tokens);
    let input_cache_read = known(raw.cache_read_tokens);
    let input_cache_write = known(raw.cache_write_tokens);
    let output_total = known(raw.output_tokens);
    let output_reasoning = known(raw.reasoning_tokens);
    let input_total = input_uncached
        .zip(input_cache_read)
        .zip(input_cache_write)
        .and_then(|((input, read), write)| input.checked_add(read)?.checked_add(write));
    let total_tokens = input_total
        .zip(output_total)
        .and_then(|(input, output)| input.checked_add(output));
    let usage = TokenUsage {
        input_uncached,
        input_cache_read,
        input_cache_write,
        input_total,
        output_total,
        output_reasoning,
        total_tokens,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Derived,
        output_total: Q::Reported,
        output_reasoning: Q::Reported,
        total_tokens: Q::Derived,
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// REAL epoch seconds to UTC milliseconds; reject nonfinite/pre-2000 values to avoid unit mistakes.
pub(crate) fn seconds_to_ms(v: f64) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    let ms = v * 1000.0;
    if ms.is_finite() && ms >= crate::domain::MIN_PLAUSIBLE_MS as f64 && ms <= i64::MAX as f64 {
        Some(ms.round() as i64)
    } else {
        None
    }
}

/// Normalize billing_base_url only in memory, under the Hermes adapter rules:
/// retain a credential-free provider identifier/local digest, never full URLs or query parameters.
/// URL-like values become scheme/host[:port], stripping userinfo/path/query/fragment.
/// Other values become a local FNV-1a digest via identity::content_hash, without retaining raw text.
pub(crate) fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some((scheme, rest)) = trimmed.split_once("://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        // Strip user:pass@host userinfo by taking the host segment after the final @.
        let host = authority.rsplit('@').next().unwrap_or(authority);
        if !host.is_empty() && !scheme.is_empty() {
            return format!(
                "{}/{}",
                scheme.to_ascii_lowercase(),
                host.to_ascii_lowercase()
            );
        }
    }
    format!(
        "opaque:{}",
        crate::identity::content_hash(&trimmed.to_owned())
    )
}

// Independent read-only database helpers copied from adapters/kilo/common.rs.

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
        "llm-usage-hermes-staging-{}-{}.db",
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
                    Some("hermes staging backup timed out".to_string()),
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
                            Some("hermes staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// Read state.db directly; busy-like probe failures try staging, other errors propagate.
/// Never write the source or invoke upstream initialization that may migrate/repair it.
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

// Schema fingerprints contain fixed DDL table/column names, rather than source records.

/// Required sessions identity/time-fallback columns; fixed source defines the full table.
pub(crate) const REQUIRED_SESSIONS_COLUMNS: &[&str] = &["id", "started_at", "ended_at"];
/// All 18 session_model_usage columns, using exact SCHEMA_SQL names.
pub(crate) const REQUIRED_SMU_COLUMNS: &[&str] = &[
    "session_id",
    "model",
    "billing_provider",
    "billing_base_url",
    "billing_mode",
    "task",
    "api_call_count",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "reasoning_tokens",
    "estimated_cost_usd",
    "actual_cost_usd",
    "cost_status",
    "cost_source",
    "first_seen",
    "last_seen",
];
/// Expected compound-key columns; v22 migration adds task to the primary key.
pub(crate) const SMU_KEY_COLUMNS: &[&str] = &[
    "session_id",
    "model",
    "billing_provider",
    "billing_base_url",
    "billing_mode",
    "task",
];

pub(crate) struct SchemaProbe {
    /// None indicates missing tables/required columns or an unmatched primary-key shape.
    pub fingerprint: Option<String>,
    /// True when required columns exist but one or more expected key columns are absent from the PK.
    pub legacy_pk: bool,
}

/// Compute schema fingerprint and primary-key classification from actual columns/keys,
/// rather than relying on a version integer.
pub(crate) fn schema_probe(conn: &Connection) -> Result<SchemaProbe, CoreError> {
    let columns_of = |table: &str| -> Result<Vec<(String, i64)>, CoreError> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(CoreError::Sqlite)?;
        let names = stmt
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(5)?)))
            .map_err(CoreError::Sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CoreError::Sqlite)?;
        Ok(names)
    };
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
    if !table_exists("sessions")? || !table_exists("session_model_usage")? {
        return Ok(SchemaProbe {
            fingerprint: None,
            legacy_pk: false,
        });
    }
    let sessions = columns_of("sessions")?;
    let smu = columns_of("session_model_usage")?;
    let has_all = |columns: &[(String, i64)], required: &[&str]| {
        required.iter().all(|r| columns.iter().any(|(c, _)| c == r))
    };
    if !has_all(&sessions, REQUIRED_SESSIONS_COLUMNS) || !has_all(&smu, REQUIRED_SMU_COLUMNS) {
        return Ok(SchemaProbe {
            fingerprint: None,
            legacy_pk: false,
        });
    }
    // All six SMU_KEY_COLUMNS must participate in the v22 primary key.
    let key_in_pk = SMU_KEY_COLUMNS
        .iter()
        .filter(|c| smu.iter().any(|(name, pk)| name == *c && *pk > 0))
        .count();
    if key_in_pk != SMU_KEY_COLUMNS.len() {
        return Ok(SchemaProbe {
            fingerprint: None,
            legacy_pk: true,
        });
    }
    Ok(SchemaProbe {
        fingerprint: Some(format!(
            "sessions({})|session_model_usage({};pk={})",
            REQUIRED_SESSIONS_COLUMNS.join(","),
            REQUIRED_SMU_COLUMNS.join(","),
            SMU_KEY_COLUMNS.join(",")
        )),
        legacy_pk: false,
    })
}

/// Read maximum schema_version (fixed source SCHEMA_VERSION=30); SQL/type errors propagate.
pub(crate) fn db_schema_version(conn: &Connection) -> Result<Option<i64>, CoreError> {
    let found: Option<i64> = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(CoreError::Sqlite)?;
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_hermes_uses_verified_disjoint_input_buckets_and_reasoning_subset() {
        let m = map_hermes(&HermesUsage {
            input_tokens: 100,
            output_tokens: 40,
            cache_read_tokens: 50,
            cache_write_tokens: 10,
            reasoning_tokens: 5,
        });
        assert_eq!(m.usage.input_total, Some(160));
        assert_eq!(m.usage.input_cache_read, Some(50));
        assert_eq!(m.usage.input_cache_write, Some(10));
        assert_eq!(m.usage.input_uncached, Some(100));
        assert_eq!(m.usage.output_total, Some(40));
        assert_eq!(m.usage.output_reasoning, Some(5));
        assert_eq!(
            m.usage.total_tokens,
            Some(200),
            "reasoning 为输出子集，不重加"
        );
        assert_eq!(m.quality.input_uncached, Q::Reported);
        assert_eq!(m.quality.input_total, Q::Derived);
        assert_eq!(m.quality.total_tokens, Q::Derived);
        assert!(m.diagnostics.is_empty());
    }

    #[test]
    fn default_zero_does_not_complete_totals_and_overflow_is_unknown() {
        let zero = HermesUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
        };
        let mapped = map_hermes(&zero);
        assert_eq!(mapped.usage, TokenUsage::default());
        assert!([
            mapped.quality.input_uncached,
            mapped.quality.input_cache_read,
            mapped.quality.input_cache_write,
            mapped.quality.input_total,
            mapped.quality.output_total,
            mapped.quality.output_reasoning,
            mapped.quality.total_tokens,
            mapped.quality.source_total,
        ]
        .into_iter()
        .all(|q| q == Q::Unknown));
        let mapped = map_hermes(&HermesUsage {
            input_tokens: i64::MAX,
            cache_read_tokens: 1,
            cache_write_tokens: 1,
            output_tokens: 1,
            reasoning_tokens: 0,
        });
        assert_eq!(mapped.usage.input_uncached, Some(i64::MAX));
        assert_eq!(mapped.usage.input_total, None);
        assert_eq!(mapped.usage.total_tokens, None);
        assert_eq!(mapped.quality.input_total, Q::Unknown);
        assert_eq!(mapped.quality.total_tokens, Q::Unknown);
    }

    #[test]
    fn seconds_to_ms_rejects_implausible() {
        assert_eq!(seconds_to_ms(1_790_157_600.0), Some(1_790_157_600_000));
        assert_eq!(seconds_to_ms(0.0), None, "epoch 0 是秒基，早于 2000 拒绝");
        assert_eq!(seconds_to_ms(1_790_157_600.5), Some(1_790_157_600_500));
        assert_eq!(seconds_to_ms(f64::NAN), None);
    }

    #[test]
    fn base_url_normalization_never_keeps_full_url() {
        assert_eq!(
            normalize_base_url("https://api.syn.example/v1#frag"),
            "https/api.syn.example"
        );
        assert_eq!(
            normalize_base_url("https://user:pass@Host.example:8443/x?k=v"),
            "https/host.example:8443",
            "userinfo/query/path/fragment 全部去除"
        );
        assert_eq!(normalize_base_url(""), "");
        let opaque = normalize_base_url("not a url at all");
        assert!(opaque.starts_with("opaque:fnv1a64:"), "非 URL 只留摘要");
        assert!(!opaque.contains("not a url"));
    }
}
