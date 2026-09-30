//! GitHub Copilot CLI 产品特有的公共部分（独立目录合同）：
//! - usage 映射（自根级 usage_map.rs 下沉，V30：产品映射随适配器目录）；
//! - 源库只读合同（复制自 adapters/goose/common.rs 的 hermes/kilo 同款实现）。
//!
//! 字段证据（M0 m0-agent-fixtures.md 实读结论 + 2026-09-29 本机真实数据复核：
//! ~/.copilot/session-store.db schema_version=8，36/36 行满足）：
//! - `assistant_usage_events`：input_tokens = **未缓存 + cache_read + cache_write**
//!   ⇒ input_uncached 由减法派生；output_tokens 直报；reasoning_tokens 独立列，
//!   与 output 的包含关系未见声明 ⇒ 并列报告不并入（不推导）。
//! - `request_multiplier`（实测恒 27.0）是 premium 付费倍率，非 token：不入账。
//! - `total_nano_aiu` 是计量单位（nano AIU）非 token：不入账。
//! - `created_at` ISO8601 字符串；`duration_ms`/`time_to_first_token_ms`（REAL ms）。
//! - events.jsonl 事件流无逐次 token（M0 结论）：不采集；会话正文列不读。

use crate::adapters::usage_map::{finish, sub_checked, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// assistant_usage_events 的 token 五列（NULL = 未知）。
#[derive(Debug, Clone, Copy, Default)]
pub struct CopilotUsage {
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
}

/// input = 未缓存 + read + write（M0 实读 + 真实数据复核）⇒ uncached 派生。
pub fn map_copilot(raw: &CopilotUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = match (
        raw.input_tokens,
        raw.cached_input_tokens,
        raw.cache_creation_input_tokens,
    ) {
        (Some(total), Some(read), Some(write)) => {
            // 缓存两桶相加溢出（各可达 MAX_TOKEN_VALUE）：派生中止记诊断，
            // 不用 saturating 把失真值带进后续计算。
            match read.checked_add(write) {
                Some(sum) => sub_checked("input_uncached", total, sum, &mut diagnostics),
                None => {
                    diagnostics.push(crate::metrics::Contradiction {
                        code: "negative_derived_field",
                        field: "input_uncached",
                        detail: format!(
                            "cache buckets overflow: {read} + {write}; uncached not derived"
                        ),
                    });
                    None
                }
            }
        }
        _ => None,
    };
    let total = match (raw.input_tokens, raw.output_tokens) {
        (Some(i), Some(o)) => match i.checked_add(o) {
            Some(t) if t <= crate::domain::MAX_TOKEN_VALUE => Some(t),
            _ => {
                diagnostics.push(crate::metrics::Contradiction {
                    code: "token_shape_deviation",
                    field: "total_tokens",
                    detail: format!("derived total {i} + {o} out of range; kept unknown"),
                });
                None
            }
        },
        _ => None,
    };
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: raw.cached_input_tokens,
        input_cache_write: raw.cache_creation_input_tokens,
        input_total: raw.input_tokens,
        output_total: raw.output_tokens,
        // reasoning 与 output 包含关系未证：并列报告，不并入派生总量。
        output_reasoning: raw.reasoning_tokens,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: if uncached.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        input_cache_read: raw
            .cached_input_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_cache_write: raw
            .cache_creation_input_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        input_total: raw.input_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: raw.output_tokens.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_reasoning: raw
            .reasoning_tokens
            .map(|_| Q::Reported)
            .unwrap_or(Q::Unknown),
        total_tokens: if total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        source_total: Q::Unknown,
    };
    finish(usage, quality, diagnostics)
}

// ---- 源库只读访问（复制自 adapters/hermes/common.rs，各目录独立合同）----

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
        "llm-usage-copilot-staging-{}-{}.db",
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
                    Some("copilot staging backup timed out".to_string()),
                ));
            }
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                Ok(_) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("copilot staging copy exceeded the space cap".to_string()),
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

/// 短查询事务探测（kilo 同合同）。
pub(crate) fn short_probe(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// threads 表必需列（固定源码建表 + 迁移列集；created_at 缺列时探测层降级）。
/// assistant_usage_events 必需列（本机 schema_version=8 实测 + M0 1.0.73 取证）。
pub(crate) const USAGE_EVENT_COLUMNS: &[&str] = &[
    "id",
    "session_id",
    "turn_index",
    "model",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "reasoning_tokens",
    "duration_ms",
    "time_to_first_token_ms",
    "created_at",
];

/// schema_version 表（单行 `version INTEGER`，本机实测 8）→ 注册表版本串。
/// 探测与扫描共用：读不到（表缺失/查询失败）返回 None，按 LatestFallback 处理。
pub(crate) fn read_schema_version(conn: &Connection) -> Option<String> {
    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| {
            r.get(0)
        })
        .ok()?;
    Some(format!("assistant-usage-events-v{version}"))
}
