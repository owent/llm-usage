//! Kilo Code CLI 产品特有的公共部分（从根级 usage_map.rs 下沉，V30 目录约定）：
//! - usage 映射（`map_kilo`/`KiloUsage`，全互斥关系）；
//! - 源 SQLite 只读访问实现（architecture.md#database 末五条）：
//!   只读连接 + 短查询；busy/锁时经 Online Backup API 生成系统临时目录暂存副本，
//!   带页/时间/空间上限并清理；仍无法一致读取时把 busy 上抛，由框架保留旧结果；
//!   绝不写源库（不 checkpoint、不改 journal/schema、不新建源端 sidecar）。
//!
//! 共享的 MappedUsage/finish/sub_checked/矛盾检测仍留在跨 Agent 的 usage_map.rs。

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::metrics::Contradiction;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// kilo（opencode 派生）message.data.tokens：
/// total = input+output+reasoning+cache.read+cache.write 全互斥（与其他源相反，
/// m0-agent-fixtures.md 实读结论）。因此 canonical output_total 须把 reasoning
/// 并入（derived），才能满足统一约定 total_tokens = input_total + output_total。
///
/// `total` 为 `Option`：未完成/出错消息可缺 `total` 字段（7.4.8/7.4.9 真实 fixture
/// 各有 2/1 条；本机实读库 13,342 条 assistant 中 42 条缺失）——缺失时 source_total
/// 保持 None（未知，不补零），派生总量仍由五字段相加得出。
#[derive(Debug, Clone, Copy)]
pub struct KiloUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: Option<i64>,
    pub cache_read: i64,
    pub cache_write: i64,
    pub total: Option<i64>,
}

pub fn map_kilo(raw: &KiloUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let input_total = raw
        .input
        .checked_add(raw.cache_read)
        .and_then(|v| v.checked_add(raw.cache_write));
    let output_total = raw
        .reasoning
        .and_then(|reasoning| raw.output.checked_add(reasoning));
    let derived_total = input_total.and_then(|i| output_total.and_then(|o| i.checked_add(o)));
    if let (Some(dt), Some(total)) = (derived_total, raw.total) {
        if dt != total {
            diagnostics.push(Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: format!("kilo total {total} != derived sum {dt}"),
            });
        }
    }
    let usage = TokenUsage {
        input_uncached: Some(raw.input),
        input_cache_read: Some(raw.cache_read),
        input_cache_write: Some(raw.cache_write),
        input_total,
        output_total,
        output_reasoning: raw.reasoning,
        total_tokens: derived_total.or(raw.total),
        source_total: raw.total,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: if output_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_reasoning: if raw.reasoning.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Reported
        },
        source_total: if raw.total.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
    };
    finish(usage, quality, diagnostics)
}

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

/// busy/锁/CANTOPEN 判定（这些错误表示无法一致读取）。
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
    /// 每步备份页数（限制单步持锁时长）。
    pub pages_per_step: i32,
    /// 暂存副本字节上限（超限放弃，标记 busy 而非占满磁盘）。
    pub max_bytes: u64,
    /// 备份总时限。
    pub max_time: Duration,
}

impl Default for StagingLimits {
    fn default() -> Self {
        StagingLimits {
            pages_per_step: 512,
            // 空间上限 2 GiB：更大的活库不强行暂存（busy/unsupported 保留旧结果）。
            max_bytes: 2 * 1024 * 1024 * 1024,
            // 时间上限 30s：超限放弃暂存并按 busy 上抛（单源每轮 30s 超时时间之内）。
            max_time: Duration::from_secs(30),
        }
    }
}

/// 打开源库只读连接。busy_timeout 设短：快速失败转暂存副本路径，
/// 不长时间阻塞采集线程。
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
/// 逐步备份并在页/时间/空间任一超限时中止；中止/失败时先关闭目标连接再删文件
/// （Windows 下打开句柄中的 remove_file 会因共享冲突静默失败留下残留）。
fn backup_to_staging(
    source: &Connection,
    limits: &StagingLimits,
) -> Result<PathBuf, rusqlite::Error> {
    let page_size =
        u64::from(source.query_row("PRAGMA page_size", [], |r| r.get::<_, i64>(0))? as u32);
    let max_pages = (limits.max_bytes / page_size.max(1)).min(i64::MAX as u64) as i64;
    let dest_path = std::env::temp_dir().join(format!(
        "llm-usage-kilo-staging-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let started = Instant::now();
    // 闭包内 `?` 只退出闭包：任何失败路径都统一落入下方清理（先关句柄再删文件）。
    let attempt = || -> Result<(), rusqlite::Error> {
        let mut dest = Connection::open(&dest_path)?;
        let backup = Backup::new(source, &mut dest)?;
        let mut done_pages: i64 = 0;
        loop {
            if started.elapsed() >= limits.max_time {
                break Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    Some("kilo staging backup timed out".to_string()),
                ));
            }
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                // Busy/Locked/More（StepResult 标记 #[non_exhaustive]）按可重试推进。
                // More：实际拷贝了页，计入空间限制。
                Ok(StepResult::More) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("kilo staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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
    // 此处 dest/backup 均已随闭包结束关闭，删除不再受句柄阻塞。
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

/// 打开 kilo.db 的只读访问：
///
/// 1. 直接只读连接 + 短查询事务（`probe` 成功即视为本步一致可读）；
/// 2. busy/锁时经 Online Backup 生成暂存副本再读；
/// 3. 仍失败 → CoreError（调用方记 busy/unsupported，保留旧结果）。
///
/// 绝不写源库。
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
            // 短读失败：用同一只读连接发起到暂存副本的一致备份。
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

/// kilo.db schema 指纹：表与关键列的存在情况（不含数据、不含正文）。
/// message 五列 + session 九列（usage 对账需要 tokens_* 五列）。
pub(crate) const REQUIRED_MESSAGE_COLUMNS: &[&str] =
    &["id", "session_id", "time_created", "time_updated", "data"];
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

/// 计算 schema 指纹（表缺失返回 None）。指纹串只含表/列名，可安全持久化。
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
    if !table_exists("message")? || !table_exists("session")? {
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
    let message_columns = columns_of("message")?;
    let session_columns = columns_of("session")?;
    let has_all = |columns: &[String], required: &[&str]| {
        required.iter().all(|r| columns.iter().any(|c| c == r))
    };
    if !has_all(&message_columns, REQUIRED_MESSAGE_COLUMNS)
        || !has_all(&session_columns, REQUIRED_SESSION_COLUMNS)
    {
        return Ok(None);
    }
    Ok(Some(format!(
        "message({})|session({})",
        REQUIRED_MESSAGE_COLUMNS.join(","),
        REQUIRED_SESSION_COLUMNS.join(",")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_kilo_five_way_exclusive_with_optional_total() {
        let m = map_kilo(&KiloUsage {
            input: 100,
            output: 60,
            reasoning: Some(40),
            cache_read: 800,
            cache_write: 100,
            total: Some(1100),
        });
        assert_eq!(m.usage.input_total, Some(1000));
        assert_eq!(m.usage.output_total, Some(100));
        assert_eq!(m.usage.total_tokens, Some(1100));
        assert_eq!(m.usage.source_total, Some(1100));
        assert!(m.diagnostics.is_empty());

        // 缺 total：source_total None，派生总量仍在。
        let missing = map_kilo(&KiloUsage {
            input: 10,
            output: 5,
            reasoning: Some(2),
            cache_read: 0,
            cache_write: 0,
            total: None,
        });
        assert_eq!(missing.usage.source_total, None);
        assert_eq!(missing.quality.source_total, Q::Unknown);
        assert_eq!(missing.usage.total_tokens, Some(17));

        // 直报 total 与派生值不一致时记诊断（包含关系成立才有可比性）。
        let bad = map_kilo(&KiloUsage {
            input: 10,
            output: 5,
            reasoning: Some(0),
            cache_read: 0,
            cache_write: 0,
            total: Some(99),
        });
        assert_eq!(bad.usage.total_tokens, Some(15), "规范化总量按派生口径");
        assert_eq!(bad.usage.source_total, Some(99), "直报值独立保留");
        assert!(bad
            .diagnostics
            .iter()
            .any(|d| d.code == "source_total_mismatch"));
    }

    #[test]
    fn map_kilo_overflow_degrades_to_unknown_not_panic() {
        let m = map_kilo(&KiloUsage {
            input: i64::MAX,
            output: 1,
            reasoning: None,
            cache_read: 1,
            cache_write: 0,
            total: None,
        });
        // 溢出的派生字段保持 None（未知），不 panic、不截断数值。
        assert_eq!(m.usage.input_total, None);
        assert_eq!(m.usage.total_tokens, None);
    }
}
