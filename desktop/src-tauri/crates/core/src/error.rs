use std::fmt;

/// Core failures remain explicit and diagnosable without silent degradation.
#[derive(Debug)]
pub enum CoreError {
    /// SQLite error.
    Sqlite(rusqlite::Error),
    /// I/O error, including size checks and temporary directories.
    Io(std::io::Error),
    /// Unusable/newer database schema; refuse writable initialization without destructive downgrade.
    SchemaTooNew { found: u32, supported: u32 },
    /// Schema mismatch under rebuild rules; the application requests confirmation to rebuild or exit.
    SchemaMismatch { found: u32, expected: u32 },
    /// Migration failure, retaining the old database.
    MigrationFailed {
        version: u32,
        name: String,
        detail: String,
    },
    /// Invalid record, such as negative/oversized values or implausible timestamps.
    Validation(String),
    /// Token aggregation overflow beyond i64.
    Overflow(&'static str),
    /// Timezone/calendar calculation failure.
    Calendar(String),
    /// Illegal scheduling/job state transition.
    JobState(String),
    /// Invalid query parameters, such as period grouping incompatible with the range.
    Query(String),
    /// Test hook injected a failure.
    FaultInjected(&'static str),
    /// Cooperative collection stop; not a parser or source-health failure.
    Interrupted(&'static str),
    /// JSON serialization failure.
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
