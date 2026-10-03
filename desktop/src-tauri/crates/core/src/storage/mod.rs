//! SQLite 持久化：单写者连接、WAL、synchronous=FULL、有界 busy_timeout、
//! 显式版本迁移。数据库合同见 architecture.md#database 与数据合同「数据表」。

pub mod pricing;
pub mod schema;

use crate::error::CoreError;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 默认 busy_timeout：有界，避免长读者/写者死等。
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

// SQLite's built-in lower/NOCASE only handle ASCII. Use the same Unicode
// lowercase mapping for SQL filters and Rust grouping, including old rows.
fn register_functions(conn: &Connection) -> Result<(), rusqlite::Error> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "fold_name",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase())),
    )?;
    conn.create_scalar_function(
        "model_key",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(ctx
                .get::<Option<String>>(0)?
                .map(|s| crate::model_names::model_key(&s)))
        },
    )
}

pub struct OpenOptions {
    pub busy_timeout: Duration,
    /// 本程序支持的最新 schema 版本；None 表示 [`schema::SCHEMA_VERSION`]。
    pub max_supported_version: Option<u32>,
}

impl Default for OpenOptions {
    fn default() -> Self {
        OpenOptions {
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            max_supported_version: None,
        }
    }
}

/// 单写者存储句柄。
pub struct Storage {
    conn: Connection,
    path: PathBuf,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Storage").field("path", &self.path).finish()
    }
}

impl Storage {
    /// 打开（必要时创建）数据库：设置 pragmas、执行待迁移版本、
    /// 把上次进程遗留的 running 作业标记为 interrupted。
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        Self::open_with(path, OpenOptions::default())
    }

    pub fn open_in_memory() -> Result<Self, CoreError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self::setup(conn, PathBuf::from(":memory:"), OpenOptions::default())?;
        Ok(storage)
    }

    pub fn open_with(path: &Path, options: OpenOptions) -> Result<Self, CoreError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        Self::setup(conn, path.to_path_buf(), options)
    }

    /// 只读连接（architecture.md：一个后台写者、少量只读连接）。
    /// 不执行迁移、不写任何 pragma 持久化设置；WAL 库上可与写者并发。
    /// 库不存在或无读权限时报错（调用方显示空态/错误，不回退到建新库）。
    pub fn open_readonly(path: &Path) -> Result<Self, CoreError> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        register_functions(&conn)?;
        conn.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
        Ok(Storage {
            conn,
            path: path.to_path_buf(),
        })
    }

    fn setup(conn: Connection, path: PathBuf, options: OpenOptions) -> Result<Self, CoreError> {
        register_functions(&conn)?;
        let supported = options
            .max_supported_version
            .unwrap_or(schema::SCHEMA_VERSION);
        conn.busy_timeout(options.busy_timeout)?;
        let found_version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;

        // 预发布阶段合同（2026-09-26 用户决策）：不做逐版本迁移。
        // 版本不匹配 ⇒ 报错让应用层提示"删除重建或退出"。
        // user_version == 0 且文件为空/新建 ⇒ 建全量 schema。
        if found_version == 0 {
            // 可能是新建空文件或旧库；检查是否有表。
            let has_tables: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='settings')",
                [],
                |r| r.get(0),
            )?;
            if !has_tables {
                // 全新库：一步建表。
                conn.execute_batch(schema::FULL_SCHEMA)?;
                conn.pragma_update(None, "user_version", supported)?;
            } else {
                // 有表但无版本号——旧库，要求重建。
                return Err(CoreError::SchemaTooNew {
                    found: found_version,
                    supported,
                });
            }
        } else if found_version != supported {
            // 预发布阶段：任何版本差异都要求重建（不尝试迁移）。
            return Err(CoreError::SchemaMismatch {
                found: found_version,
                expected: supported,
            });
        }

        conn.pragma_update(None, "foreign_keys", "ON")?;
        // journal_mode 是持久化设置；内存库返回 memory，可忽略其结果差异。
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(options.busy_timeout)?;

        let storage = Storage { conn, path };
        storage.mark_running_jobs_interrupted(crate::jobs::now_ms_fallback())?;
        Ok(storage)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 当前 user_version。
    pub fn schema_version(&self) -> Result<u32, CoreError> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// 数据修订号：单调递增，查询按同一修订号返回一致视图。
    pub fn data_revision(&self) -> Result<i64, CoreError> {
        data_revision(&self.conn)
    }

    /// 提升数据修订号并返回新值。仅在事务内调用。
    pub fn bump_data_revision_tx(
        tx: &rusqlite::Transaction<'_>,
        now_ms: i64,
    ) -> Result<i64, CoreError> {
        let next = data_revision(tx)?
            .checked_add(1)
            .ok_or(CoreError::Overflow("data_revision"))?;
        tx.execute(
            "INSERT INTO settings (key, value, schema_version, updated_at_ms)
             VALUES ('data_revision', ?1, 1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            rusqlite::params![next.to_string(), now_ms],
        )?;
        Ok(next)
    }

    /// 进程重启恢复：把 running 作业标记为 interrupted（不依赖 PID 文件）。
    pub fn mark_running_jobs_interrupted(&self, now_ms: i64) -> Result<usize, CoreError> {
        let n = self.conn.execute(
            "UPDATE ingest_runs SET status = 'interrupted', finished_ms = ?1
             WHERE status = 'running'",
            rusqlite::params![now_ms],
        )?;
        Ok(n)
    }

    /// 确保本机来源主机身份存在并返回其不透明稳定 ID（data-contract.md#provenance）。
    /// - 首次调用生成 `host-<32hex>` 持久化于 origin_hosts（is_local=1），
    ///   并把 settings.local_origin_host_id 指向它；不使用主机名/IP/硬件指纹作身份。
    /// - 后续调用返回同一 ID 并记录主机名观察（改名不换 ID、不重复计数；
    ///   主机名只进观察表用于辨认，不参与任何键）。
    /// - 复制数据库到新机器时不自动认领历史：身份与采集由调用方显式传入，
    ///   来源注册冲突走映射/确认流程（M1a 只提供判定，不做静默合并）。
    pub fn ensure_local_host(&self, hostname: &str, now_ms: i64) -> Result<String, CoreError> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'local_origin_host_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let host_id = match existing {
            Some(id) => {
                let present: bool = self.conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM origin_hosts WHERE host_id = ?1)",
                    rusqlite::params![id],
                    |r| r.get(0),
                )?;
                if !present {
                    return Err(CoreError::Validation(format!(
                        "local_origin_host_id {id:?} has no origin_hosts row; refusing to guess ownership"
                    )));
                }
                id
            }
            None => {
                let id = format!(
                    "host-{}",
                    self.conn
                        .query_row("SELECT lower(hex(randomblob(16)))", [], |r| {
                            r.get::<_, String>(0)
                        })?
                );
                self.conn.execute(
                    "INSERT INTO origin_hosts (host_id, is_local, note, first_seen_ms, last_seen_ms)
                     VALUES (?1, 1, NULL, ?2, ?2)",
                    rusqlite::params![id, now_ms],
                )?;
                self.conn.execute(
                    "INSERT INTO settings (key, value, schema_version, updated_at_ms)
                     VALUES ('local_origin_host_id', ?1, 1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
                    rusqlite::params![id, now_ms],
                )?;
                id
            }
        };
        self.observe_hostname(&host_id, hostname, now_ms)?;
        Ok(host_id)
    }

    /// 当前本机主机 ID（未初始化时为 None）。
    pub fn local_host_id(&self) -> Result<Option<String>, CoreError> {
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'local_origin_host_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id)
    }

    /// 记录主机名观察（同一主机的历史名称都保留；改名不换 ID）。
    pub fn observe_hostname(
        &self,
        host_id: &str,
        hostname: &str,
        now_ms: i64,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO origin_host_names (host_id, hostname, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(host_id, hostname) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
            rusqlite::params![host_id, hostname, now_ms],
        )?;
        self.conn.execute(
            "UPDATE origin_hosts SET last_seen_ms = ?2 WHERE host_id = ?1",
            rusqlite::params![host_id, now_ms],
        )?;
        Ok(())
    }

    /// 登记一台外部来源主机（导入用）：返回其 host_id；同一 host_id 重复登记幂等。
    pub fn register_origin_host(
        &self,
        host_id: &str,
        hostname: Option<&str>,
        now_ms: i64,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO origin_hosts (host_id, is_local, note, first_seen_ms, last_seen_ms)
             VALUES (?1, 0, NULL, ?2, ?2)
             ON CONFLICT(host_id) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
            rusqlite::params![host_id, now_ms],
        )?;
        if let Some(name) = hostname {
            self.observe_hostname(host_id, name, now_ms)?;
        }
        Ok(())
    }
}

pub(crate) fn data_revision(conn: &Connection) -> Result<i64, CoreError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'data_revision'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match value {
        Some(v) => v
            .parse::<i64>()
            .map_err(|e| CoreError::Validation(format!("bad data_revision value {v:?}: {e}"))),
        None => Ok(0),
    }
}
