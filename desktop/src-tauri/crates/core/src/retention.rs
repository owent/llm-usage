//! Detail cutoff is today's start in the selected timezone minus D-1 days; archive expired days
//! with timezone/field/source versions and block ordinary additions. Size includes DB/WAL/SHM/backups.

use crate::calendar::Calendar;
use crate::error::CoreError;
use crate::ingest;
use crate::storage::Storage;
use jiff::civil::Date;
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeSet;
use std::path::Path;

/// Increment when archive field meanings change; archived rows retain the version used.
pub const SEAL_FIELD_VERSION: &str = "m1-fields-1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Detail retention in local days, including today: 7–3650.
    pub detail_days: u32,
    /// Diagnostic retention in days.
    pub diagnostics_days: u32,
    /// Hard maximum local-day retention for usage/quotas/diagnostics; None adds no extra limit.
    pub hard_max_days: Option<u32>,
}

impl RetentionPolicy {
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(7..=3650).contains(&self.detail_days) {
            return Err(CoreError::Validation(format!(
                "detail retention {} out of range 7..=3650",
                self.detail_days
            )));
        }
        if !(1..=30).contains(&self.diagnostics_days) {
            return Err(CoreError::Validation(format!(
                "diagnostics retention {} out of range 1..=30",
                self.diagnostics_days
            )));
        }
        if let Some(hard) = self.hard_max_days {
            if hard < self.detail_days {
                return Err(CoreError::Validation(
                    "hard max retention must not be shorter than detail retention".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionOutcome {
    /// Keep details on or after this local cutoff day.
    pub cutoff_day: Date,
    pub cutoff_ms: i64,
    pub sealed_days: Vec<String>,
    pub deleted_events: i64,
    pub deleted_diagnostics: i64,
    pub deleted_daily_rows: i64,
    pub deleted_quota_rows: i64,
    pub data_revision: i64,
}

/// Archive expired daily summaries and delete expired details in one transaction.
pub fn enforce_retention(
    storage: &Storage,
    timezone: &str,
    now_ms: i64,
    policy: &RetentionPolicy,
) -> Result<RetentionOutcome, CoreError> {
    policy.validate()?;
    let calendar = Calendar::new(timezone)?;
    let today = calendar.today(now_ms)?;
    let cutoff_day = calendar.retention_cutoff_day(today, policy.detail_days)?;
    let (cutoff_ms, _) = calendar.day_range_ms(cutoff_day)?;

    let conn = storage.conn();
    let tx = conn.unchecked_transaction()?;
    // Persist an increasing cleanup floor; restart/longer retention cannot restore deleted data through ordinary rescans.
    tx.execute(
        "INSERT INTO settings (key, value, schema_version, updated_at_ms)
         VALUES ('detail_retention_floor_ms', ?1, 1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = CAST(MAX(CAST(value AS INTEGER), CAST(excluded.value AS INTEGER)) AS TEXT), updated_at_ms = excluded.updated_at_ms",
        params![cutoff_ms.to_string(), now_ms],
    )?;

    // 1. Find expired local days with details.
    let mut expired_days: BTreeSet<Date> = BTreeSet::new();
    {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT occurred_at_ms FROM usage_events WHERE occurred_at_ms < ?1",
        )?;
        let rows = stmt.query_map(params![cutoff_ms], |r| r.get::<_, i64>(0))?;
        for row in rows {
            expired_days.insert(calendar.local_day_of(row?)?);
        }
    }

    // 2. Recompute each day before archiving; retain timezone/field/source-selection versions.
    // Publish the aggregate generation atomically with the transaction.
    tx.execute(
        "INSERT INTO aggregate_generations (kind, tz_version, status, note, created_at_ms, published_at_ms)
         VALUES ('retention_seal', ?1, 'building', NULL, ?2, NULL)",
        params![calendar.tz_name(), now_ms],
    )?;
    let generation_id = tx.last_insert_rowid();
    let next_revision = Storage::bump_data_revision_tx(&tx, now_ms)?;
    let mut sealed_days = Vec::new();
    for day in &expired_days {
        ingest::recompute_day(&tx, &calendar, *day, next_revision)?;
        let mut stmt = tx.prepare(
            "SELECT DISTINCT source_instance_id FROM usage_events
             WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2",
        )?;
        let (day_start, day_end) = calendar.day_range_ms(*day)?;
        let sources: Vec<String> = stmt
            .query_map(params![day_start, day_end], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        let source_version = serde_json::to_string(&sources)?;
        tx.execute(
            "UPDATE daily_usage SET sealed = 1, sealed_at_ms = ?3, seal_tz = ?4,
             seal_field_version = ?5, seal_source_version = ?6, data_revision = ?7
             WHERE tz_version = ?1 AND local_day = ?2",
            params![
                calendar.tz_name(),
                day.to_string(),
                now_ms,
                calendar.tz_name(),
                SEAL_FIELD_VERSION,
                source_version,
                next_revision
            ],
        )?;
        sealed_days.push(day.to_string());
    }

    // 3. Delete expired details; logical SQLite deletion does not guarantee physical erasure.
    tx.execute(
        "DELETE FROM event_aliases WHERE canonical_event_id IN
         (SELECT event_id FROM usage_events WHERE occurred_at_ms < ?1)
         OR member_event_id IN (SELECT event_id FROM usage_events WHERE occurred_at_ms < ?1)",
        params![cutoff_ms],
    )?;
    let deleted_events = tx.execute(
        "DELETE FROM usage_events WHERE occurred_at_ms < ?1",
        params![cutoff_ms],
    )? as i64;

    // 4. Apply diagnostic retention.
    let mut diag_cutoff = now_ms - i64::from(policy.diagnostics_days) * 86_400_000;
    if let Some(hard_days) = policy.hard_max_days {
        let hard_day = calendar.retention_cutoff_day(today, hard_days)?;
        diag_cutoff = diag_cutoff.max(calendar.day_range_ms(hard_day)?.0);
    }
    let deleted_diagnostics = tx.execute(
        "DELETE FROM diagnostics WHERE created_ms < ?1",
        params![diag_cutoff],
    )? as i64;

    // 5. Apply hard maximum retention to daily summaries and quota snapshots.
    let mut deleted_daily_rows = 0i64;
    let mut deleted_quota_rows = 0i64;
    if let Some(hard_days) = policy.hard_max_days {
        let hard_cutoff_day = calendar.retention_cutoff_day(today, hard_days)?;
        deleted_daily_rows = tx.execute(
            "DELETE FROM daily_usage WHERE tz_version = ?1 AND local_day < ?2",
            params![calendar.tz_name(), hard_cutoff_day.to_string()],
        )? as i64;
        let (hard_cutoff_ms, _) = calendar.day_range_ms(hard_cutoff_day)?;
        tx.execute(
            "INSERT INTO settings (key, value, schema_version, updated_at_ms)
             VALUES ('hard_retention_floor_ms', ?1, 1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = CAST(MAX(CAST(value AS INTEGER), CAST(excluded.value AS INTEGER)) AS TEXT), updated_at_ms = excluded.updated_at_ms",
            params![hard_cutoff_ms.to_string(), now_ms],
        )?;
        deleted_quota_rows = tx.execute(
            "DELETE FROM quota_snapshots WHERE observed_at_ms < ?1",
            params![hard_cutoff_ms],
        )? as i64;
        deleted_quota_rows += tx.execute(
            "DELETE FROM quota_history WHERE observed_at_ms < ?1",
            params![hard_cutoff_ms],
        )? as i64;
        // Native intervals cannot be split proportionally; unknown starts/cross-cutoff aggregates cannot satisfy the hard limit.
        tx.execute(
            "DELETE FROM source_aggregates WHERE interval_start_ms IS NULL OR interval_start_ms < ?1 OR interval_end_ms < ?1",
            params![hard_cutoff_ms],
        )?;
    }

    tx.execute(
        "UPDATE aggregate_generations SET status = 'published', published_at_ms = ?2 WHERE generation_id = ?1",
        params![generation_id, now_ms],
    )?;

    tx.commit()?;
    Ok(RetentionOutcome {
        cutoff_day,
        cutoff_ms,
        sealed_days,
        deleted_events,
        deleted_diagnostics,
        deleted_daily_rows,
        deleted_quota_rows,
        data_revision: next_revision,
    })
}

pub(crate) fn hard_retention_floor(conn: &rusqlite::Connection) -> Result<Option<i64>, CoreError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'hard_retention_floor_ms'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|v| {
            v.parse::<i64>()
                .map_err(|_| CoreError::Validation("invalid hard retention floor".into()))
        })
        .transpose()
}

/// Storage size: main DB, WAL, SHM and backup-directory files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageFootprint {
    pub main_db_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub backup_bytes: u64,
    pub total_bytes: u64,
}

pub fn storage_footprint(
    db_path: &Path,
    backup_dir: Option<&Path>,
) -> Result<StorageFootprint, CoreError> {
    let file_len = |p: &Path| -> u64 { std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) };
    let main = file_len(db_path);
    let wal = file_len(&sidecar(db_path, "-wal"));
    let shm = file_len(&sidecar(db_path, "-shm"));
    let mut backup = 0u64;
    if let Some(dir) = backup_dir {
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_file() {
                    backup += meta.len();
                }
            }
        }
    }
    Ok(StorageFootprint {
        main_db_bytes: main,
        wal_bytes: wal,
        shm_bytes: shm,
        backup_bytes: backup,
        total_bytes: main + wal + shm + backup,
    })
}

fn sidecar(db_path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut s = db_path.as_os_str().to_owned();
    s.push(suffix);
    std::path::PathBuf::from(s)
}
