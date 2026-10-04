use std::fmt;

/// 核心错误类型。所有失败必须显式可诊断，不做静默降级。
#[derive(Debug)]
pub enum CoreError {
    /// SQLite 层错误。
    Sqlite(rusqlite::Error),
    /// IO 错误（容量统计、临时目录等）。
    Io(std::io::Error),
    /// 数据库 user_version 比本程序支持的更新：拒绝打开写入，不破坏性降级。
    SchemaTooNew { found: u32, supported: u32 },
    /// 预发布阶段：版本不匹配（旧库或未知版本），需用户确认删除重建或退出。
    SchemaMismatch { found: u32, expected: u32 },
    /// 迁移执行失败；旧库保持不变。
    MigrationFailed {
        version: u32,
        name: String,
        detail: String,
    },
    /// 记录校验失败（负值、超过上限、时间明显不合理等）。
    Validation(String),
    /// token 聚合溢出（i64 上溢防护）。
    Overflow(&'static str),
    /// 时区/日历计算失败。
    Calendar(String),
    /// 调度/作业状态机非法转移。
    JobState(String),
    /// 查询参数非法（如粒度与区间不匹配）。
    Query(String),
    /// 注入了故障（测试钩子）。
    FaultInjected(&'static str),
    /// Cooperative collection stop; not a parser or source-health failure.
    Interrupted(&'static str),
    /// JSON 序列化失败。
    Json(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Sqlite(e) => write!(f, "sqlite: {e}"),
            CoreError::Io(e) => write!(f, "io: {e}"),
            CoreError::SchemaTooNew { found, supported } => {
                write!(f, "schema version {found} is newer than supported {supported}; refusing to open for write")
            }
            CoreError::SchemaMismatch { found, expected } => {
                write!(f, "database schema version {found} != expected {expected}; rebuild required (pre-release, no incremental migration)")
            }
            CoreError::MigrationFailed {
                version,
                name,
                detail,
            } => {
                write!(f, "migration v{version} ({name}) failed: {detail}")
            }
            CoreError::Validation(msg) => write!(f, "validation: {msg}"),
            CoreError::Overflow(what) => write!(f, "token aggregation overflow: {what}"),
            CoreError::Calendar(msg) => write!(f, "calendar: {msg}"),
            CoreError::JobState(msg) => write!(f, "job state: {msg}"),
            CoreError::Query(msg) => write!(f, "query: {msg}"),
            CoreError::FaultInjected(point) => write!(f, "fault injected at {point}"),
            CoreError::Interrupted(reason) => write!(f, "interrupted: {reason}"),
            CoreError::Json(msg) => write!(f, "json: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Sqlite(e)
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        if let Some(interrupted) = e
            .get_ref()
            .and_then(|error| error.downcast_ref::<crate::adapters::run_policy::ReadInterrupted>())
        {
            return CoreError::Interrupted(interrupted.0);
        }
        CoreError::Io(e)
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Json(e.to_string())
    }
}

impl From<jiff::Error> for CoreError {
    fn from(e: jiff::Error) -> Self {
        CoreError::Calendar(e.to_string())
    }
}
