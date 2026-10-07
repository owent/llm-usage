//! Zed 产品特有的公共部分（独立目录约定 architecture.md#adapter-layout）：
//! 本轮 1.22.0 / 76659a55 的 OpenAI-compatible mapper 扣除缓存后保存 input；
//! llm-usage-zhipu 的正桶为报告值，默认零/缺项未知，不派生完整总量。
//! 下列 bd74733 依据与缺省零规则仅描述原有 hosted 映射。
//! - usage 映射（`map_zed`/`ZedUsage`，字段依据为 Zed 官方源码 bd74733，A38）；
//! - threads.db 源库只读访问实现（复制自 `adapters/hermes/common.rs` 的
//!   kilo 同款实现：只读连接 + Online Backup 暂存副本，各 Agent 目录独立）。
//!
//! 固定源码依据（zed-industries/zed bd747337d7be138834e20972b9e203c7b239cc47）：
//! - `crates/agent/src/db.rs:451-483`：threads 表（id/summary/updated_at/
//!   data_type/data + 迁移列 parent_id/folder_paths/folder_paths_order/created_at）；
//!   data_type ∈ {json, zstd}（db.rs:363-385），当前写入固定 zstd（db.rs:536）。
//! - `crates/language_model_core/src/language_model_core.rs:525-535`：
//!   TokenUsage 平铺四 u64：input_tokens/output_tokens/cache_creation_input_tokens/
//!   cache_read_input_tokens，**0 值序列化时整体缺省**（skip_serializing_if），
//!   因此字段缺失 = 已报告 0（官方序列化语义，非猜测补零）。
//! - input 与 cache 两桶的包含关系官方源码未见声明 ⇒ 不推导互斥/子集，
//!   按 hermes 同型并列报告，不派生总量。
//! - 仅 provider=="zed.dev" 的 hosted 调用计入（A38）；request_token_usage
//!   在 turn 内多请求时后写覆盖前写（官方 thread.rs:2893 清零语义），
//!   求和会漏计 ⇒ 线程总量以 cumulative_token_usage 为准，逐桶只作对账。

use crate::adapters::usage_map::{finish_parallel, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};

/// threads.data blob 内 TokenUsage 四桶（缺省=0：官方 skip_serializing_if 语义）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ZedUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
}

impl ZedUsage {
    /// 四桶合计（派生值，checked 算术约定：任一桶可达 MAX_TOKEN_VALUE，
    /// 四桶相加可溢出 i64 ⇒ None 表示溢出，调用方拒绝该线程而非饱和隐藏）。
    pub fn total(&self) -> Option<i64> {
        self.input_tokens
            .checked_add(self.output_tokens)?
            .checked_add(self.cache_read_input_tokens)?
            .checked_add(self.cache_creation_input_tokens)
    }
}

/// 包含关系未验证 ⇒ input_uncached/派生总量保持 None（不猜互斥关系），
/// 四桶并列直报（与 map_hermes 同型）。
pub fn map_zed(raw: &ZedUsage) -> MappedUsage {
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: Some(raw.cache_read_input_tokens),
        input_cache_write: Some(raw.cache_creation_input_tokens),
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: None,
        total_tokens: None,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Unknown,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: Q::Unknown,
        source_total: Q::Unknown,
    };
    finish_parallel(usage, quality, Vec::new())
}

/// Zed 1.22.0 OpenAI chat mapper subtracts cache reads from prompt_tokens.
/// Its serialized default zeroes carry no provider validity bit.
pub fn map_verified_openai(raw: &ZedUsage) -> MappedUsage {
    let positive = |n: i64| (n > 0).then_some(n);
    let usage = TokenUsage {
        input_uncached: positive(raw.input_tokens),
        input_cache_read: positive(raw.cache_read_input_tokens),
        input_cache_write: positive(raw.cache_creation_input_tokens),
        output_total: positive(raw.output_tokens),
        ..Default::default()
    };
    let q = |value: Option<i64>| {
        if value.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        }
    };
    let quality = TokenQuality {
        input_uncached: q(usage.input_uncached),
        input_cache_read: q(usage.input_cache_read),
        input_cache_write: q(usage.input_cache_write),
        output_total: q(usage.output_total),
        ..Default::default()
    };
    finish_parallel(usage, quality, Vec::new())
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
    crate::adapters::run_policy::install_sqlite_control(&conn)?;
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
        "llm-usage-zed-staging-{}-{}.db",
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
                    Some("zed staging backup timed out".to_string()),
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
                            Some("zed staging copy exceeded the space cap staging copy exceeded the space cap".to_string()),
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

/// 打开 threads.db 的只读访问：直接只读短查询 → busy 时暂存副本 → 仍失败上抛。
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
pub(crate) const REQUIRED_THREADS_COLUMNS: &[&str] =
    &["id", "summary", "updated_at", "data_type", "data"];
