//! SQLite 持久化：单写者连接、WAL、synchronous=FULL、有界 busy_timeout、
//! 显式版本迁移。数据库合同见 architecture.md#database 与数据合同「数据表」。

pub mod schema;

use crate::error::CoreError;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 默认 busy_timeout：有界，避免长读者/写者死等。
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

pub struct OpenOptions {
    pub busy_timeout: Duration,
    /// 本程序支持的最新 schema 版本；None 表示 [`schema::SCHEMA_VERSION`]。
    pub max_supported_version: Option<u32>,
    /// 测试钩子：附加迁移（用于模拟迁移失败/多版本）。
    pub extra_migrations: Vec<schema::Migration>,
}

impl Default for OpenOptions {
    fn default() -> Self {
        OpenOptions {
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            max_supported_version: None,
            extra_migrations: Vec::new(),
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
        conn.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
        Ok(Storage {
            conn,
            path: path.to_path_buf(),
        })
    }

    fn setup(conn: Connection, path: PathBuf, options: OpenOptions) -> Result<Self, CoreError> {
        let supported = options
            .max_supported_version
            .unwrap_or(schema::SCHEMA_VERSION);
        conn.busy_timeout(options.busy_timeout)?;
        let found_version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if found_version > supported {
            return Err(CoreError::SchemaTooNew {
                found: found_version,
                supported,
            });
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // journal_mode 是持久化设置；内存库返回 memory，可忽略其结果差异。
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(options.busy_timeout)?;

        let mut migrations: Vec<&schema::Migration> = schema::MIGRATIONS.iter().collect();
        let extra: Vec<&schema::Migration> = options.extra_migrations.iter().collect();
        migrations.extend(extra);
        migrations.sort_by_key(|m| m.version);

        for migration in migrations {
            if migration.version <= found_version || migration.version > supported {
                continue;
            }
            run_migration(&conn, migration)?;
        }

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

fn run_migration(conn: &Connection, migration: &schema::Migration) -> Result<(), CoreError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| CoreError::MigrationFailed {
            version: migration.version,
            name: migration.name.to_string(),
            detail: e.to_string(),
        })?;
    let result = (|| -> Result<(), CoreError> {
        tx.execute_batch(migration.sql)?;
        if migration.version == 2 && migration.name == "review_identity_and_known_usage" {
            let partitions = {
                let mut stmt = tx.prepare(
                    "SELECT DISTINCT tz_version, local_day FROM daily_usage WHERE sealed = 0",
                )?;
                let rows =
                    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            let has_data: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM daily_usage) OR EXISTS(SELECT 1 FROM usage_events) OR EXISTS(SELECT 1 FROM source_aggregates)", [], |r| r.get(0))?;
            if has_data {
                let revision = Storage::bump_data_revision_tx(&tx, crate::jobs::now_ms_fallback())?;
                tx.execute(
                    "UPDATE daily_usage SET data_revision = ?1 WHERE sealed = 1",
                    [revision],
                )?;
                for (tz, day) in partitions {
                    crate::ingest::recompute_day(
                        &tx,
                        &crate::calendar::Calendar::new(&tz)?,
                        crate::calendar::parse_date(&day)?,
                        revision,
                    )?;
                }
            }
        }
        if migration.version == 4 && migration.name == "origin_host_identity_and_source_partitions"
        {
            // 旧混合日汇总按来源分区重算（事件仍在 ⇒ 推导不是猜测；封存行除外）。
            let partitions = {
                let mut stmt = tx.prepare(
                    "SELECT DISTINCT tz_version, local_day FROM daily_usage WHERE sealed = 0",
                )?;
                let rows =
                    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            if !partitions.is_empty() {
                let revision = Storage::bump_data_revision_tx(&tx, crate::jobs::now_ms_fallback())?;
                tx.execute(
                    "UPDATE daily_usage SET data_revision = ?1 WHERE sealed = 1",
                    [revision],
                )?;
                for (tz, day) in partitions {
                    crate::ingest::recompute_day(
                        &tx,
                        &crate::calendar::Calendar::new(&tz)?,
                        crate::calendar::parse_date(&day)?,
                        revision,
                    )?;
                }
            }
        }
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_ms, notes) VALUES (?1, ?2, ?3, NULL)",
            rusqlite::params![migration.version, migration.name, crate::jobs::now_ms_fallback()],
        )?;
        tx.pragma_update(None, "user_version", migration.version)?;
        Ok(())
    })();
    match result {
        Ok(()) => tx.commit().map_err(|e| CoreError::MigrationFailed {
            version: migration.version,
            name: migration.name.to_string(),
            detail: e.to_string(),
        }),
        Err(e) => {
            // 回滚该版本，保留旧库。
            let _ = tx.rollback();
            Err(CoreError::MigrationFailed {
                version: migration.version,
                name: migration.name.to_string(),
                detail: e.to_string(),
            })
        }
    }
}
