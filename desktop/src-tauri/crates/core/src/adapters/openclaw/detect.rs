//! Official schema 24 and native 2026.9.8 CLI evidence; legacy inputs stay closed.
use crate::adapters::framework::DetectOutcome;
use crate::adapters::openclaw::common::{open_source_db, schema_probe, short_probe, StagingLimits};
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::{Path, PathBuf};

pub const OPENCLAW_FORMAT: &str = "openclaw-agent-sqlite";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputKind {
    RuntimeStore,
    LegacyTranscript,
    LegacySessionRows,
    Other,
}

pub(crate) fn agent_dir(path: &Path) -> Option<PathBuf> {
    let layer = path.parent()?;
    if !matches!(layer.file_name()?.to_str()?, "agent" | "sessions") {
        return None;
    }
    let agent = layer.parent()?;
    if agent.file_name()?.to_str()?.is_empty() || agent.parent()?.file_name()?.to_str()? != "agents"
    {
        return None;
    }
    Some(agent.to_path_buf())
}

pub(crate) fn classify(path: &Path) -> InputKind {
    if agent_dir(path).is_none() {
        return InputKind::Other;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    match path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        Some("agent") if name == "openclaw-agent.sqlite" => InputKind::RuntimeStore,
        Some("sessions") if name == "sessions.json" => InputKind::LegacySessionRows,
        Some("sessions") if name.ends_with(".jsonl") => InputKind::LegacyTranscript,
        _ => InputKind::Other,
    }
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let unknown = |reason: &str| DetectOutcome::UnknownFormat {
        reason: reason.into(),
    };
    match classify(path) {
        InputKind::RuntimeStore => {
            let source = match open_source_db(path, short_probe, &StagingLimits::default()) {
                Ok(source) => source,
                Err(CoreError::Sqlite(error)) if error.sqlite_error_code() == Some(rusqlite::ffi::ErrorCode::NotADatabase) => return Ok(unknown("OpenClaw runtime path is not a SQLite database; fail closed")),
                Err(error) => return Err(error),
            };
            let snapshot = source.conn().unchecked_transaction()?;
            let owner = agent_dir(path).unwrap();
            let owner = owner.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            match schema_probe(&snapshot, owner)? {
                Some(24) => Ok(DetectOutcome::Supported { format: OPENCLAW_FORMAT.into(), format_version: None, basis: VersionBasis::LatestFallback }),
                Some(version) => Ok(DetectOutcome::UnsupportedVersion { format: OPENCLAW_FORMAT.into(), found: Some(version.to_string()), reason: "Only native schema 24 has been verified; fail closed".into() }),
                None => Ok(unknown("OpenClaw runtime schema/agent ownership does not match verified schema 24; fail closed")),
            }
        }
        InputKind::LegacyTranscript | InputKind::LegacySessionRows => Ok(unknown("OpenClaw legacy sessions are migration inputs; fail closed, not an additional usage source")),
        InputKind::Other => Ok(unknown("File does not match agents/<agentId>/agent/openclaw-agent.sqlite or legacy sessions inputs")),
    }
}
