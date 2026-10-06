//! OpenClaw 产品特有的公共部分（独立目录约定）：
//! - 源库只读约定小工具复制自 `adapters/kilo/common.rs`（各 Agent 目录保持
//!   独立不共享模块；仅前缀/注释不同）；
//! - 运行时库结构枚举（只读表名集合，不含数据）。
//!
//! 实现依据与核验范围（A09，官方文档 docs.openclaw.ai，2026-09-24 核验）：
//! - store 参考：每 Agent 一个 `~/.openclaw/agents/<agentId>/agent/openclaw-agent.sqlite`
//!   （会话行 + 追加式 transcript 两个持久层）；旧 `sessions/` 目录与
//!   `sessions/sessions.json` 为迁移/归档输入，Gateway 启动不导入。
//! - schema 24/官方 2026.9.8 实装及真实 CLI 样本已核对；完整合同见
//!   docs/design/desktop-usage/openclaw-runtime.md。

use crate::error::CoreError;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 一次只读访问：成功时直接用源库连接；busy/锁时自动切换到暂存副本。
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

/// busy/锁/CANTOPEN 判定（这些错误表示无法一致读取）。
pub(crate) fn is_busy_like(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::DatabaseBusy)
            | Some(rusqlite::ffi::ErrorCode::DatabaseLocked)
            | Some(rusqlite::ffi::ErrorCode::CannotOpen)
    )
}

/// 暂存副本参数（同 kilo/hermes 约定）。
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
                // More：实际拷贝了页，计入空间限制。
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
                // Busy/Locked（#[non_exhaustive] 其余）：无进展重试，仍计入超时时间。
                // 2026-09-30 修复：此前重试也计入页数，与超时出口竞速产生
                // 平台相关的 space cap 误报（CI Linux 页上限先于超时触发）。
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

/// Structural evidence only; never certify a historical row's client version.
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
