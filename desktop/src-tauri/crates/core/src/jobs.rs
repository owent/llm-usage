//! 采集作业合同：ingest_runs 状态机（running/interrupted/succeeded/failed/cancelled）。
//! 同源最多一个运行作业；重叠触发合并进既有作业（M6 才接调度并发控制，这里
//! 只提供结构与存储语义）。进程重启由 Storage::open 把 running 标记为 interrupted。

use crate::error::CoreError;
use crate::storage::Storage;
use rusqlite::params;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Interrupted,
    Succeeded,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Interrupted => "interrupted",
            RunStatus::Succeeded => "succeeded",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "running" => Ok(RunStatus::Running),
            "interrupted" => Ok(RunStatus::Interrupted),
            "succeeded" => Ok(RunStatus::Succeeded),
            "failed" => Ok(RunStatus::Failed),
            "cancelled" => Ok(RunStatus::Cancelled),
            other => Err(CoreError::JobState(format!("unknown run status: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Manual,
    Interval,
    FixedTime,
    FileWatch,
    Startup,
    Resume,
}

impl TriggerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TriggerKind::Manual => "manual",
            TriggerKind::Interval => "interval",
            TriggerKind::FixedTime => "fixed_time",
            TriggerKind::FileWatch => "file_watch",
            TriggerKind::Startup => "startup",
            TriggerKind::Resume => "resume",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStart {
    /// 新建作业。
    Started(String),
    /// 同源已有运行作业：触发被合并，返回既有作业 ID。
    Merged(String),
}

/// 启动同源采集作业；已有 running 作业时合并触发原因并返回 Merged。
pub fn start_run(
    storage: &Storage,
    run_id: &str,
    instance_id: &str,
    trigger: TriggerKind,
    now_ms: i64,
) -> Result<RunStart, CoreError> {
    let existing: Option<String> = storage
        .conn()
        .query_row(
            "SELECT run_id FROM ingest_runs WHERE instance_id = ?1 AND status = 'running' ORDER BY started_ms LIMIT 1",
            params![instance_id],
            |r| r.get(0),
        )
        .ok();
    if let Some(existing_id) = existing {
        let merged: String = storage.conn().query_row(
            "SELECT merged_triggers FROM ingest_runs WHERE run_id = ?1",
            params![existing_id],
            |r| r.get(0),
        )?;
        let mut triggers: Vec<String> = serde_json::from_str(&merged).unwrap_or_default();
        triggers.push(trigger.as_str().to_string());
        storage.conn().execute(
            "UPDATE ingest_runs SET merged_triggers = ?1 WHERE run_id = ?2",
            params![serde_json::to_string(&triggers)?, existing_id],
        )?;
        return Ok(RunStart::Merged(existing_id));
    }
    storage.conn().execute(
        "INSERT INTO ingest_runs (run_id, instance_id, trigger_kind, status, started_ms)
         VALUES (?1, ?2, ?3, 'running', ?4)",
        params![run_id, instance_id, trigger.as_str(), now_ms],
    )?;
    Ok(RunStart::Started(run_id.to_string()))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunStats {
    pub added: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub skipped: i64,
    pub errors: i64,
}

/// 结束作业为终态。running → succeeded/failed/cancelled；其他转移非法。
pub fn finish_run(
    storage: &Storage,
    run_id: &str,
    status: RunStatus,
    stats: RunStats,
    error_summary: Option<&str>,
    now_ms: i64,
) -> Result<(), CoreError> {
    match status {
        RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled => {}
        other => {
            return Err(CoreError::JobState(format!(
                "finish_run requires a terminal status, got {}",
                other.as_str()
            )))
        }
    }
    let current: String = storage
        .conn()
        .query_row(
            "SELECT status FROM ingest_runs WHERE run_id = ?1",
            params![run_id],
            |r| r.get(0),
        )
        .map_err(|_| CoreError::JobState(format!("unknown run {run_id}")))?;
    if current != "running" {
        return Err(CoreError::JobState(format!(
            "run {run_id} is {current}, cannot finish as {}",
            status.as_str()
        )));
    }
    storage.conn().execute(
        "UPDATE ingest_runs SET status = ?1, finished_ms = ?2, added = ?3, updated = ?4,
         unchanged = ?5, skipped = ?6, errors = ?7, error_summary = ?8
         WHERE run_id = ?9",
        params![
            status.as_str(),
            now_ms,
            stats.added,
            stats.updated,
            stats.unchanged,
            stats.skipped,
            stats.errors,
            error_summary,
            run_id
        ],
    )?;
    Ok(())
}

pub fn run_status(storage: &Storage, run_id: &str) -> Result<Option<RunStatus>, CoreError> {
    let row: Option<String> = storage
        .conn()
        .query_row(
            "SELECT status FROM ingest_runs WHERE run_id = ?1",
            params![run_id],
            |r| r.get(0),
        )
        .ok();
    row.map(|s| RunStatus::parse(&s)).transpose()
}

/// 批次内更新作业计数（与事件/游标/聚合同事务提交进度）。
pub(crate) fn merge_run_stats_tx(
    tx: &rusqlite::Transaction<'_>,
    run_id: &str,
    stats: RunStats,
    data_revision: i64,
) -> Result<(), CoreError> {
    tx.execute(
        "UPDATE ingest_runs SET added = added + ?1, updated = updated + ?2,
         unchanged = unchanged + ?3, skipped = skipped + ?4, errors = errors + ?5,
         data_revision = ?6
         WHERE run_id = ?7 AND status = 'running'",
        params![
            stats.added,
            stats.updated,
            stats.unchanged,
            stats.skipped,
            stats.errors,
            data_revision,
            run_id
        ],
    )?;
    Ok(())
}

/// 无法获得单调时钟时的兜底时间戳（打开/迁移路径）。
pub(crate) fn now_ms_fallback() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
