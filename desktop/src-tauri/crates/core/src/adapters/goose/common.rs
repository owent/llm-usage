//! Goose 产品特有的公共部分（独立目录约定）：
//! - 源库只读约定（复制自 adapters/zed/common.rs 的 hermes/kilo 同款实现）；
//! - 逐请求 usage_ledger 行映射（官方源码 a701bb1，A26）。
//!
//! 固定源码依据（aaif-goose/goose a701bb1756f0c6a49a7dbc10ac8a90f94dd24bd1）：
//! - usage_ledger（迁移 15，session_manager.rs:1083-1097）：每 provider 响应
//!   一行 INSERT（:913），created_timestamp 为 Unix **秒**；cost_source ∈
//!   {provider_reported, estimated, carried_forward}；is_compaction 标记压缩调用。
//! - Usage 字段语义（goose-provider-types token_usage.rs:93-101）：input_tokens
//!   为全部 prompt tokens，**包含** cache 读/写（子集）⇒ input_uncached 由
//!   减法派生（sub_checked 防负）。
//! - sessions.accumulated_* 是会话累计（非 accumulated 列是最后快照，不用）；
//!   仅在旧库（schema < 15，无 usage_ledger）时按 session 聚合回退。

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// usage_ledger 行的 token 列（NULL 视为未知，不补零）。
#[derive(Debug, Clone, Copy, Default)]
pub struct GooseLedgerUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

/// input_tokens 含 cache 读/写（官方字段语义）⇒ uncached 派生；total 直报对照。
pub fn map_goose_ledger(raw: &GooseLedgerUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = match (
        raw.input_tokens,
        raw.cache_read_tokens,
        raw.cache_write_tokens,
    ) {
        (Some(total), Some(r), Some(w)) => sub_checked(
            "input_uncached",
            total,
            r.saturating_add(w),
            &mut diagnostics,
        ),
        _ => None,
    };
    let derived_total = match (raw.input_tokens, raw.output_tokens) {
        (Some(i), Some(o)) => i.checked_add(o),
        _ => None,
    };
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: raw.cache_read_tokens,
        input_cache_write: raw.cache_write_tokens,
        input_total: raw.input_tokens,
        output_total: raw.output_tokens,
        output_reasoning: None,
        total_tokens: derived_total,
        source_total: raw.total_tokens,
    };
    let quality = TokenQuality {
        input_uncached: if uncached.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        input_cache_read: raw
            .cache_read_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_cache_write: raw
            .cache_write_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_total: raw.input_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: raw.output_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        source_total: raw.total_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        ..Default::default()
    };
    finish(usage, quality, diagnostics)
}

// ---- 源库只读访问（复制自 adapters/hermes/common.rs，各目录独立约定）----

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
        "llm-usage-goose-staging-{}-{}.db",
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
                    Some("goose staging backup timed out".to_string()),
                ));
            }
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                // More：实际拷贝了页，计入空间限制。
                Ok(StepResult::More) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("goose staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// 打开 sessions.db 的只读访问：直接只读短查询 → busy 时暂存副本 → 仍失败上抛。
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

/// 短查询事务探测（与 kilo 遵守同一规则）。
pub(crate) fn short_probe(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// threads 表必需列（固定源码建表 + 迁移列集；created_at 缺列时探测层降级）。
/// usage_ledger 逐列（固定源码迁移 15 建表 SQL）。
pub(crate) const LEDGER_COLUMNS: &[&str] = &[
    "id",
    "session_id",
    "created_timestamp",
    "model",
    "input_tokens",
    "output_tokens",
    "total_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "cost",
    "cost_source",
    "is_compaction",
];
/// sessions 表兜底所需列（旧库无 usage_ledger 时按 accumulated_* 聚合）。
pub(crate) const SESSIONS_FALLBACK_COLUMNS: &[&str] = &[
    "id",
    "accumulated_total_tokens",
    "accumulated_input_tokens",
    "accumulated_output_tokens",
    "created_at",
    "updated_at",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncached_derived_from_input_minus_cache() {
        // input_tokens 含 cache 读/写：uncached = 100 - 30 - 10 = 60。
        let mapped = map_goose_ledger(&GooseLedgerUsage {
            input_tokens: Some(100),
            output_tokens: Some(20),
            total_tokens: Some(120),
            cache_read_tokens: Some(30),
            cache_write_tokens: Some(10),
        });
        assert_eq!(mapped.usage.input_total, Some(100));
        assert_eq!(mapped.usage.input_uncached, Some(60));
        assert_eq!(mapped.usage.total_tokens, Some(120));
        assert_eq!(mapped.usage.source_total, Some(120));
    }

    #[test]
    fn negative_derived_reports_contradiction_not_clamped() {
        // cache 合计超过 input：uncached 置未知 + 矛盾诊断（不改成 0）。
        let mapped = map_goose_ledger(&GooseLedgerUsage {
            input_tokens: Some(10),
            cache_read_tokens: Some(8),
            cache_write_tokens: Some(8),
            ..Default::default()
        });
        assert_eq!(mapped.usage.input_uncached, None);
        assert!(mapped
            .diagnostics
            .iter()
            .any(|d| d.code == "negative_derived_field"));
    }

    #[test]
    fn missing_buckets_stay_unknown() {
        // NULL 列不补零。
        let mapped = map_goose_ledger(&GooseLedgerUsage {
            input_tokens: Some(50),
            ..Default::default()
        });
        assert_eq!(mapped.usage.output_total, None);
        assert_eq!(mapped.usage.total_tokens, None);
        assert_eq!(mapped.usage.input_uncached, None);
    }
}
