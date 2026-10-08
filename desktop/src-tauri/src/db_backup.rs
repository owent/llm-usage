//! Consistent database backups and disk-space checks before V15/M1 migration/cleanup/rebuild.
//!
//! Backup rules:
//! - VACUUM INTO creates a single consistent snapshot, including with WAL, without file copying.
//! - Require target free space >= 1.1 * database/WAL/SHM bytes; insufficient space fails,
//!   allowing callers to stop destructive operations.
//! - Keep three matching snapshots in filename order, using timestamped names.
//!   V15 treats backups as application-managed restoration subject to retention rules.
//! - A direct read-only connection backs up old schemas rejected by application setup;
//!   rebuild must finish backup before deleting the original.

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Number of matching backups retained.
const KEEP_BACKUPS: usize = 3;
/// Free-space factor 1.1; this check does not establish an upper bound on snapshot size.
const SPACE_HEADROOM: f64 = 1.1;

/// Current bytes in the database, -wal and -shm files.
pub fn db_files_bytes(db_path: &Path) -> u64 {
    let mut total = 0u64;
    for suffix in ["", "-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if let Ok(meta) = std::fs::metadata(&p) {
            total += meta.len();
        }
    }
    total
}

/// backups/ beside the database, matching backup_before_clear placement.
pub fn backup_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// Pure space test: free >= needed * 1.1 allows backup.
pub fn space_sufficient(free: u64, needed: u64) -> bool {
    if needed == 0 {
        return true;
    }
    (free as f64) >= (needed as f64) * SPACE_HEADROOM
}

/// Insufficient target free space fails before backup; callers must stop destructive work.
fn ensure_space(backup_target_dir: &Path, needed: u64) -> Result<(), String> {
    if needed == 0 {
        return Ok(());
    }
    let free =
        fs4::free_space(backup_target_dir).map_err(|e| format!("free space check failed: {e}"))?;
    if !space_sufficient(free, needed) {
        return Err(format!(
            "insufficient disk space for backup: need ~{:.1} MiB (1.1x database), free {:.1} MiB",
            needed as f64 / 1048576.0,
            free as f64 / 1048576.0
        ));
    }
    Ok(())
}

/// Keep the last KEEP_BACKUPS matching prefix/.sqlite files in lexical filename order.
fn prune_old_backups(dir: &Path, prefix: &str) {
    let mut olds: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with(prefix) && n.ends_with(".sqlite")
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    olds.sort();
    while olds.len() > KEEP_BACKUPS {
        let oldest = olds.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
}

/// Open an old, possibly incompatible schema through a read-only connection.
fn open_legacy_readonly(db_path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("open legacy database for backup: {e}"))
}

/// VACUUM INTO snapshots the caller's current connection consistently.
/// Return its path, or None without backup when the database has no tables.
pub fn consistent_backup(
    conn: &Connection,
    db_path: &Path,
    prefix: &str,
    now_ms: i64,
) -> Result<Option<PathBuf>, String> {
    let has_tables: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("inspect database: {e}"))?;
    if !has_tables {
        return Ok(None);
    }
    let dir = backup_dir(db_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create backup dir: {e}"))?;
    ensure_space(&dir, db_files_bytes(db_path))?;
    let secs = now_ms / 1000;
    let stamp = format!("{}-{:08x}", prefix, u64::try_from(secs.max(0)).unwrap_or(0));
    let mut backup = dir.join(format!("{stamp}.sqlite"));
    let mut suffix = 0u32;
    // Append an ordinal when the same-second name exists, avoiding overwrite.
    while backup.exists() && suffix < 1000 {
        suffix += 1;
        backup = dir.join(format!("{stamp}-{suffix}.sqlite"));
    }
    if backup.exists() {
        return Err("backup failed: no unused snapshot filename".to_string());
    }
    if let Err(e) = conn.execute(
        "VACUUM INTO ?1",
        rusqlite::params![backup.to_string_lossy()],
    ) {
        let _ = std::fs::remove_file(&backup);
        return Err(format!("backup failed: {e}"));
    }
    prune_old_backups(&dir, prefix);
    Ok(Some(backup))
}

/// Back up an incompatible old schema before rebuild deletes the original.
/// A direct read-only connection uses VACUUM INTO; Err must prevent original-file deletion.
pub fn consistent_backup_legacy(
    db_path: &Path,
    prefix: &str,
    now_ms: i64,
) -> Result<Option<PathBuf>, String> {
    if !db_path.is_file() {
        return Ok(None);
    }
    let conn = open_legacy_readonly(db_path)?;
    consistent_backup(&conn, db_path, prefix, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_backup_removes_only_its_partial_destination() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-finalization/backup-cancel")
            .join(format!(
                "{}-{}",
                std::process::id(),
                crate::scanner::now_ms()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("app.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t(x); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO t SELECT x FROM n;").unwrap();
        let dir = backup_dir(&db);
        std::fs::create_dir_all(&dir).unwrap();
        let existing = dir.join("cancel-00000001.sqlite");
        std::fs::write(&existing, b"existing snapshot").unwrap();
        conn.progress_handler(1000, Some(|| true)).unwrap();
        assert!(consistent_backup(&conn, &db, "cancel", 1000).is_err());
        conn.progress_handler(0, None::<fn() -> bool>).unwrap();
        assert_eq!(std::fs::read(&existing).unwrap(), b"existing snapshot");
        assert!(!dir.join("cancel-00000001-1.sqlite").exists());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            10000
        );
    }

    #[test]
    fn exhausted_snapshot_names_preserve_every_existing_file() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/plan-finalization/backup-collision")
            .join(format!(
                "{}-{}",
                std::process::id(),
                crate::scanner::now_ms()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("app.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES(42)")
            .unwrap();
        let dir = backup_dir(&db);
        std::fs::create_dir_all(&dir).unwrap();
        for suffix in 0..=1000 {
            let name = if suffix == 0 {
                "collision-00000001.sqlite".to_string()
            } else {
                format!("collision-00000001-{suffix}.sqlite")
            };
            std::fs::write(dir.join(name), b"existing snapshot").unwrap();
        }
        assert!(consistent_backup(&conn, &db, "collision", 1000)
            .unwrap_err()
            .contains("no unused snapshot filename"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1001);
        assert_eq!(
            std::fs::read(dir.join("collision-00000001-1000.sqlite")).unwrap(),
            b"existing snapshot"
        );
    }

    #[test]
    fn space_decision() {
        assert!(space_sufficient(0, 0));
        assert!(space_sufficient(1_100, 1_000));
        assert!(!space_sufficient(1_099, 1_000));
        assert!(!space_sufficient(0, 1));
    }

    #[test]
    fn backup_roundtrip_and_prune() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-backup-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("app.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (42);")
            .unwrap();
        let backup = consistent_backup(&conn, &db, "test-backup", 1_800_000_000_000)
            .unwrap()
            .expect("backup created");
        // Open the consistent snapshot independently and close it after reading;
        // an open SQLite connection on Windows can prevent prune from deleting the file.
        {
            let bconn =
                Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            let value: i64 = bconn
                .query_row("SELECT x FROM t", [], |r| r.get(0))
                .unwrap();
            assert_eq!(value, 42);
        }
        // Five more timestamped backups leave the last three matching snapshots.
        for i in 1..=5i64 {
            consistent_backup(&conn, &db, "test-backup", 1_800_000_000_000 + i * 5_000).unwrap();
        }
        let remaining = std::fs::read_dir(backup_dir(&db))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("test-backup-") && n.ends_with(".sqlite")
            })
            .count();
        assert_eq!(remaining, KEEP_BACKUPS);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
