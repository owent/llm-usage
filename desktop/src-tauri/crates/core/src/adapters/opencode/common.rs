//! OpenCode 产品特有的公共部分（独立目录合同 architecture.md#adapter-layout）：
//! - schema 指纹（产品互斥：session 必须含 tokens_* 五累计列，MiMo 无此列）；
//! - state 库只读合同落地（只读连接 + Online Backup 暂存副本）：
//!   小工具函数复制自 `adapters/kilo/common.rs`（各 Agent 目录保持独立，
//!   不共享模块；kilo 版本为 M3 已验收实现，语义一致仅前缀/注释不同）。
//!
//! 固定源码证据（A17，commit 0027387dc5c59793c12dfc531abc78f825ed6868）：
//! - `packages/core/src/global.ts`：data = xdg-basedir 的 opencode 目录
//!   （$XDG_DATA_HOME 缺省 ~/.local/share/opencode）；
//! - `packages/core/src/database/database.ts`：库文件
//!   `path() = data/opencode.db`（安装通道变体 `opencode-<channel>.db`，
//!   channel 经 [^a-zA-Z0-9._-] 归一）；WAL 模式；
//! - `packages/core/src/session/sql.ts`：`session`（含 parent_id/version/
//!   tokens_input/output/reasoning/cache_read/cache_write 累计五列）、
//!   `message`（id/session_id/…/data JSON）、`part`（id/message_id/session_id/
//!   time_created/time_updated/data JSON）三表逐字列名。

use crate::error::CoreError;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ---- 源库只读访问（复制自 adapters/kilo/common.rs，各目录独立合同）----

/// 一次只读访问：成功时直接用源库连接；busy/锁时自动切换到暂存副本。
/// `guard` 持有暂存副本路径，drop 时清理（即使查询中途失败）。
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

/// busy/锁/CANTOPEN 判定（无法一致读取的证据）。
pub(crate) fn is_busy_like(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::DatabaseBusy)
            | Some(rusqlite::ffi::ErrorCode::DatabaseLocked)
            | Some(rusqlite::ffi::ErrorCode::CannotOpen)
    )
}

/// 暂存副本参数（architecture.md：设置页/时间/空间上限并清理）。
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

/// 打开源库只读连接。busy_timeout 设短：快速失败转暂存副本路径。
pub(crate) fn open_readonly(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.busy_timeout(Duration::from_millis(150))?;
    Ok(conn)
}

/// Online Backup 到系统临时目录的一致暂存副本（从只读连接发起，不写源库）。
fn backup_to_staging(
    source: &Connection,
    limits: &StagingLimits,
) -> Result<PathBuf, rusqlite::Error> {
    let page_size =
        u64::from(source.query_row("PRAGMA page_size", [], |r| r.get::<_, i64>(0))? as u32);
    let max_pages = (limits.max_bytes / page_size.max(1)).min(i64::MAX as u64) as i64;
    let dest_path = std::env::temp_dir().join(format!(
        "llm-usage-opencode-staging-{}-{}.db",
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
                    Some("opencode staging backup timed out".to_string()),
                ));
            }
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                Ok(_) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("opencode staging copy exceeded the space cap".to_string()),
                        ));
                    }
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

/// 打开 opencode.db 的只读访问：直接只读短查询 → busy 时暂存副本 → 仍失败上抛。
/// 绝不写源库（不调用上游可能迁移/初始化的打开路径）。
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

/// 短查询事务探测（kilo 同合同）。
pub(crate) fn short_probe(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

// ---- schema 指纹（固定源码 sql.ts 投影；只含表/列名，可安全持久化）----

/// part 表关键列（逐次 usage 载体；sql.ts PartTable 逐字列名）。
pub(crate) const REQUIRED_PART_COLUMNS: &[&str] = &[
    "id",
    "message_id",
    "session_id",
    "time_created",
    "time_updated",
    "data",
];
/// session 关键列：身份 + 子会话 + 版本 + 累计五列（对账目标；MiMo 无此五列，
/// 指纹因此互斥，不能把 MiMo 库当 OpenCode 解析）。
pub(crate) const REQUIRED_SESSION_COLUMNS: &[&str] = &[
    "id",
    "parent_id",
    "version",
    "tokens_input",
    "tokens_output",
    "tokens_reasoning",
    "tokens_cache_read",
    "tokens_cache_write",
];
/// message 关键列（模型归属 join：message.data.$.modelID/providerID）。
pub(crate) const REQUIRED_MESSAGE_COLUMNS: &[&str] = &["id", "session_id", "data"];

/// 计算 schema 指纹（表缺失/关键列缺失返回 None）。
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
