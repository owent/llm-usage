//! Hermes Agent 产品特有的公共部分（独立目录合同 architecture.md#adapter-layout）：
//! - usage 映射（`map_hermes`/`HermesUsage`，列级证据为固定源码，见下）；
//! - `billing_base_url` 内存规范化（无凭据 provider 标识或本地摘要，不持久化原文）；
//! - state.db 源库只读合同落地（只读连接 + Online Backup 暂存副本）：
//!   小工具函数复制自 `adapters/kilo/common.rs`（各 Agent 目录保持独立，
//!   不共享模块；kilo 版本为 M3 已验收实现，语义一致仅前缀/注释不同）。
//!
//! 固定源码证据（A24，commit ef70b3661cbfcf57e583008ad91dd04d8ba46070）：
//! - `hermes_state_common.py` SCHEMA_SQL：`session_model_usage` 18 列，
//!   PRIMARY KEY (session_id, model, billing_provider, billing_base_url,
//!   billing_mode, task)；`sessions` 含 id/source/parent_session_id/started_at/
//!   ended_at/end_reason 与累计五列等（本适配器只读时间兜底列）。
//! - `hermes_state_usage.py`：计数器按组合键 ADD 式累计（增量路径），
//!   absolute 路径只覆盖 sessions 总量且不写模型行；`record_auxiliary_usage`
//!   只写 task 键行、不进主会话总量；first_seen 仅插入时写入，last_seen
//!   每次冲突更新推进。
//! - `hermes_constants.py`：get_hermes_home = 上下文覆盖 → HERMES_HOME →
//!   平台默认（Windows %LOCALAPPDATA%/hermes，其他 ~/.hermes）；
//!   命名 profile 为 `<root>/profiles/<name>` 独立 state.db。
//!
//! 时间列（first_seen/last_seen/started_at/ended_at）为 REAL Unix epoch 秒
//!（Python time.time()），本模块统一换算毫秒。字段包含关系（input 与 cache、
//! reasoning 与 output）未随 normalize_usage 完整路径验证（adapters.md A24），
//! 映射不推导互斥/子集，只按列报告。

use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// session_model_usage 五个并行计数列（NOT NULL DEFAULT 0，i64）。
#[derive(Debug, Clone, Copy)]
pub struct HermesUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
}

/// 五列各自独立报告；包含关系未验证 ⇒
/// - input_uncached / total_tokens / source_total 保持 None（不猜互斥口径）；
/// - input_total = input_tokens、output_total = output_tokens 直报，
///   cache/reasoning 并列报告（与 map_genai_usage 的未证包含关系处理同型）。
pub fn map_hermes(raw: &HermesUsage) -> MappedUsage {
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: Some(raw.cache_read_tokens),
        input_cache_write: Some(raw.cache_write_tokens),
        input_total: Some(raw.input_tokens),
        output_total: Some(raw.output_tokens),
        output_reasoning: Some(raw.reasoning_tokens),
        total_tokens: None,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Unknown,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Reported,
        output_total: Q::Reported,
        output_reasoning: Q::Reported,
        total_tokens: Q::Unknown,
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// REAL epoch 秒 → UTC 毫秒。非有限/早于 2000-01-01（秒毫秒误判守卫）→ None。
pub(crate) fn seconds_to_ms(v: f64) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    let ms = v * 1000.0;
    if ms.is_finite() && ms >= crate::domain::MIN_PLAUSIBLE_MS as f64 && ms <= i64::MAX as f64 {
        Some(ms.round() as i64)
    } else {
        None
    }
}

/// billing_base_url 内存规范化（adapters.md Hermes 合同：只在内存规范化成
/// 无凭据的 provider 标识或本机摘要，不持久化完整 URL/查询参数）：
/// URL 形 → `scheme/host[:port]`（去 userinfo/path/query/fragment）；
/// 非 URL 形 → 不可逆本地摘要（FNV-1a，identity::content_hash），不落原文。
pub(crate) fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some((scheme, rest)) = trimmed.split_once("://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        // 去 userinfo（user:pass@host）：只取最后一个 @ 之后的 host 段。
        let host = authority.rsplit('@').next().unwrap_or(authority);
        if !host.is_empty() && !scheme.is_empty() {
            return format!(
                "{}/{}",
                scheme.to_ascii_lowercase(),
                host.to_ascii_lowercase()
            );
        }
    }
    format!(
        "opaque:{}",
        crate::identity::content_hash(&trimmed.to_owned())
    )
}

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
            max_time: Duration::from_secs(2),
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
        "llm-usage-hermes-staging-{}-{}.db",
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
                    Some("hermes staging backup timed out".to_string()),
                ));
            }
            match backup.step(limits.pages_per_step) {
                Ok(StepResult::Done) => break Ok(()),
                Ok(_) => {
                    done_pages += i64::from(limits.pages_per_step);
                    if done_pages > max_pages {
                        break Err(rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_TOOBIG),
                            Some("hermes staging copy exceeded the space cap".to_string()),
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

/// 打开 state.db 的只读访问：直接只读短查询 → busy 时暂存副本 → 仍失败上抛。
/// 绝不写源库（不调用上游可能迁移/修复的初始化 API）。
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

// ---- schema 指纹（固定源码 DDL 投影；只含表/列名，可安全持久化）----

/// sessions 只读列（本适配器仅用身份与时间兜底列；完整列集见固定源码）。
pub(crate) const REQUIRED_SESSIONS_COLUMNS: &[&str] = &["id", "started_at", "ended_at"];
/// session_model_usage 全部 18 列（固定源码 SCHEMA_SQL 逐字列名）。
pub(crate) const REQUIRED_SMU_COLUMNS: &[&str] = &[
    "session_id",
    "model",
    "billing_provider",
    "billing_base_url",
    "billing_mode",
    "task",
    "api_call_count",
    "input_tokens",
    "output_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "reasoning_tokens",
    "estimated_cost_usd",
    "actual_cost_usd",
    "cost_status",
    "cost_source",
    "first_seen",
    "last_seen",
];
/// 组合键列（v22 起 task 参与主键；固定源码 _migrate_v22_session_model_usage）。
pub(crate) const SMU_KEY_COLUMNS: &[&str] = &[
    "session_id",
    "model",
    "billing_provider",
    "billing_base_url",
    "billing_mode",
    "task",
];

pub(crate) struct SchemaProbe {
    /// None = 表/关键列缺失或主键形状不符（探测层 fail closed 的依据分类）。
    pub fingerprint: Option<String>,
    /// True = 两表关键列齐全但主键不含 task（pre-v22 形状）。
    pub legacy_pk: bool,
}

/// 计算 schema 指纹 + 主键形状分类（adapters.md：必须探测真实列与主键，
/// 不只看版本整数）。
pub(crate) fn schema_probe(conn: &Connection) -> Result<SchemaProbe, CoreError> {
    let columns_of = |table: &str| -> Result<Vec<(String, i64)>, CoreError> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(CoreError::Sqlite)?;
        let names = stmt
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(5)?)))
            .map_err(CoreError::Sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CoreError::Sqlite)?;
        Ok(names)
    };
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
    if !table_exists("sessions")? || !table_exists("session_model_usage")? {
        return Ok(SchemaProbe {
            fingerprint: None,
            legacy_pk: false,
        });
    }
    let sessions = columns_of("sessions")?;
    let smu = columns_of("session_model_usage")?;
    let has_all = |columns: &[(String, i64)], required: &[&str]| {
        required.iter().all(|r| columns.iter().any(|(c, _)| c == r))
    };
    if !has_all(&sessions, REQUIRED_SESSIONS_COLUMNS) || !has_all(&smu, REQUIRED_SMU_COLUMNS) {
        return Ok(SchemaProbe {
            fingerprint: None,
            legacy_pk: false,
        });
    }
    // 主键形状：SMU_KEY_COLUMNS 六列均须参与主键（v22 重建后形状）。
    let key_in_pk = SMU_KEY_COLUMNS
        .iter()
        .filter(|c| smu.iter().any(|(name, pk)| name == *c && *pk > 0))
        .count();
    if key_in_pk != SMU_KEY_COLUMNS.len() {
        return Ok(SchemaProbe {
            fingerprint: None,
            legacy_pk: true,
        });
    }
    Ok(SchemaProbe {
        fingerprint: Some(format!(
            "sessions({})|session_model_usage({};pk={})",
            REQUIRED_SESSIONS_COLUMNS.join(","),
            REQUIRED_SMU_COLUMNS.join(","),
            SMU_KEY_COLUMNS.join(",")
        )),
        legacy_pk: false,
    })
}

/// 库内 schema_version（固定源码 SCHEMA_VERSION=30；缺表/空 → None）。
pub(crate) fn db_schema_version(conn: &Connection) -> Result<Option<i64>, CoreError> {
    let found: Option<i64> = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(CoreError::Sqlite)?;
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_hermes_reports_columns_without_inclusion_claims() {
        let m = map_hermes(&HermesUsage {
            input_tokens: 100,
            output_tokens: 40,
            cache_read_tokens: 50,
            cache_write_tokens: 10,
            reasoning_tokens: 5,
        });
        assert_eq!(m.usage.input_total, Some(100));
        assert_eq!(m.usage.input_cache_read, Some(50));
        assert_eq!(m.usage.input_cache_write, Some(10));
        assert_eq!(m.usage.input_uncached, None, "包含关系未验证不推导");
        assert_eq!(m.usage.output_total, Some(40));
        assert_eq!(m.usage.output_reasoning, Some(5));
        assert_eq!(m.usage.total_tokens, None, "无 total 列，不伪造派生总量");
        assert_eq!(m.quality.input_total, Q::Reported);
        assert_eq!(m.quality.total_tokens, Q::Unknown);
    }

    #[test]
    fn seconds_to_ms_rejects_implausible() {
        assert_eq!(seconds_to_ms(1_790_157_600.0), Some(1_790_157_600_000));
        assert_eq!(seconds_to_ms(0.0), None, "epoch 0 是秒基，早于 2000 拒绝");
        assert_eq!(seconds_to_ms(1_790_157_600.5), Some(1_790_157_600_500));
        assert_eq!(seconds_to_ms(f64::NAN), None);
    }

    #[test]
    fn base_url_normalization_never_keeps_full_url() {
        assert_eq!(
            normalize_base_url("https://api.syn.example/v1#frag"),
            "https/api.syn.example"
        );
        assert_eq!(
            normalize_base_url("https://user:pass@Host.example:8443/x?k=v"),
            "https/host.example:8443",
            "userinfo/query/path/fragment 全部去除"
        );
        assert_eq!(normalize_base_url(""), "");
        let opaque = normalize_base_url("not a url at all");
        assert!(opaque.starts_with("opaque:fnv1a64:"), "非 URL 只留摘要");
        assert!(!opaque.contains("not a url"));
    }
}
