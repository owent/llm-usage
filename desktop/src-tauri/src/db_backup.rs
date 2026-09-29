//! 数据库一致备份 + 备份前空间检查（V15/M1 余项：迁移/清理/重建前备份合同）。
//!
//! 合同要点：
//! - 一致备份用 `VACUUM INTO` 单文件快照（WAL 下也是一致点；不是文件复制）；
//! - 备份前检查目标盘剩余空间 ≥ 库文件（含 -wal）× 1.1：不足则失败，
//!   调用方中止破坏性操作（无无声数据丢失）；
//! - 备份只保留最近 3 份（文件名含时间戳，字典序即时间序）；
//!   备份是应用管理的恢复路径，不绕过最长保留的清理语义（V15）；
//! - 旧 schema 库（我们的 setup 拒绝打开）用裸连接只读打开做备份——
//!   重建路径先备份再删除。

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// 备份保留份数。
const KEEP_BACKUPS: usize = 3;
/// 空间余量系数（VACUUM INTO 产物可能略小于源库，取 1.1 倍上界）。
const SPACE_HEADROOM: f64 = 1.1;

/// 库文件（含 -wal/-shm）当前占用的字节数。
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

/// 备份目录（库文件同目录 backups/，与 backup_before_clear 同一布局）。
pub fn backup_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// 空间判定（纯函数，供测试）：free ≥ needed×1.1 才允许备份。
pub fn space_sufficient(free: u64, needed: u64) -> bool {
    if needed == 0 {
        return true;
    }
    (free as f64) >= (needed as f64) * SPACE_HEADROOM
}

/// 备份前空间检查：目标盘剩余空间不足时报错（调用方中止破坏性操作）。
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

/// 只保留最近 KEEP_BACKUPS 份（前缀过滤 + 字典序）。
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

/// 打开（可能 schema 不兼容的）旧库做只读备份连接。
fn open_legacy_readonly(db_path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("open legacy database for backup: {e}"))
}

/// 一致备份当前可写库（调用方持有写连接；VACUUM INTO 单文件快照）。
/// 返回备份文件路径；库无任何表（全新/空）时返回 None 不备份。
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
    // 同秒重复备份：追加序号避免覆盖。
    while backup.exists() && suffix < 1000 {
        suffix += 1;
        backup = dir.join(format!("{stamp}-{suffix}.sqlite"));
    }
    conn.execute(
        "VACUUM INTO ?1",
        rusqlite::params![backup.to_string_lossy()],
    )
    .map_err(|e| format!("backup failed: {e}"))?;
    prune_old_backups(&dir, prefix);
    Ok(Some(backup))
}

/// 一致备份一个我们不兼容的旧库（重建路径：先备份再删除）。
/// 用只读裸连接 + VACUUM INTO；失败返回 Err（调用方不得删除原库）。
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
        // 备份是一致快照：可独立打开并读出数据（读毕关闭——
        // Windows 下未关闭的 SQLite 连接持有文件锁，会阻止 prune 删除）。
        {
            let bconn =
                Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            let value: i64 = bconn
                .query_row("SELECT x FROM t", [], |r| r.get(0))
                .unwrap();
            assert_eq!(value, 42);
        }
        // 重复备份 5 份（不同时间戳）⇒ 只留最近 3 份。
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
