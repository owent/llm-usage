//! One application owner per database. The OS releases the lock after a crash;
//! the lock file stays in place so another process cannot lock a different inode.
use std::fs::File;
use std::path::Path;

pub fn acquire(db_path: &Path) -> std::io::Result<Option<File>> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(db_path.with_extension("lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Do not call Storage::open here: only the lock owner may recover running jobs.
pub fn request_refresh(db_path: &Path, now: i64) -> Result<(), rusqlite::Error> {
    request(db_path, now, "pending_refresh")
}

pub fn request_background_refresh(db_path: &Path, now: i64) -> Result<(), rusqlite::Error> {
    request(db_path, now, "pending_background_refresh")
}

fn request(db_path: &Path, now: i64, key: &str) -> Result<(), rusqlite::Error> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute(
        "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,'1',1,?2)
        ON CONFLICT(key) DO UPDATE SET value='1',updated_at_ms=excluded.updated_at_ms",
        rusqlite::params![key, now],
    )?;
    Ok(())
}

pub fn take_background_refresh_request(
    storage: &llm_usage_core::storage::Storage,
) -> Result<bool, rusqlite::Error> {
    Ok(storage.conn().execute(
        "DELETE FROM settings WHERE key='pending_background_refresh'",
        [],
    )? > 0)
}

pub fn take_refresh_request(
    storage: &llm_usage_core::storage::Storage,
) -> Result<bool, rusqlite::Error> {
    Ok(storage
        .conn()
        .execute("DELETE FROM settings WHERE key='pending_refresh'", [])?
        > 0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn second_owner_is_blocked_and_requests_merge_without_interrupting_jobs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/review-2026-09-27/process-guard");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("{}.sqlite", std::process::id()));
        let guard = super::acquire(&path).unwrap().unwrap();
        let storage = llm_usage_core::storage::Storage::open(&path).unwrap();
        storage.conn().execute("INSERT INTO ingest_runs(run_id,instance_id,trigger_kind,status,started_ms) VALUES ('active','source','manual','running',1)", []).unwrap();
        assert!(super::acquire(&path).unwrap().is_none());
        super::request_refresh(&path, 2).unwrap();
        super::request_refresh(&path, 3).unwrap();
        assert_eq!(
            llm_usage_core::jobs::run_status(&storage, "active").unwrap(),
            Some(llm_usage_core::jobs::RunStatus::Running)
        );
        assert!(super::take_refresh_request(&storage).unwrap());
        assert!(!super::take_refresh_request(&storage).unwrap());
        drop(storage);
        drop(guard);
        assert!(super::acquire(&path).unwrap().is_some());
    }
}
