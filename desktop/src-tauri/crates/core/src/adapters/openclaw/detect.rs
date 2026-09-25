//! OpenClaw 探测与版本分派（文档级证据，A09）。
//!
//! 输入三形（官方 store 参考给出的磁盘位置，`~/.openclaw/agents/<agentId>/...`）：
//! 1. 运行时库 `agent/openclaw-agent.sqlite`：Agent 身份由文档路径形状确认；
//!    官方文档未给出任何表名/列名 ⇒ **fail closed（待真实样本）**，
//!    不读表、不猜字段、不产生零值；
//! 2. 旧归档 `sessions/*.jsonl`（legacy/archive transcript artifacts）：
//!    文档明确为迁移/离线维护输入（Gateway 启动不导入，须经
//!    `openclaw doctor --fix` 迁移）；条目级 schema 未在文档给出 ⇒
//!    按迁移输入降级处理，fail closed 并标注待证；
//! 3. 旧会话行 `sessions/sessions.json`（legacy row migration input）：
//!    同为迁移输入，非 usage 详单 ⇒ fail closed。
//!
//! 非 OpenClaw 形状的文件直接 UnknownFormat（防止手工根误投）。

use crate::adapters::framework::DetectOutcome;
use crate::adapters::openclaw::common::{
    open_source_db, short_probe, user_table_count, StagingLimits,
};
use crate::error::CoreError;
use std::path::Path;

pub const OPENCLAW_FORMAT: &str = "openclaw-agent-sqlite";

fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ffi::ErrorCode::NotADatabase)
    )
}

/// 路径形状分类（官方文档磁盘位置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputKind {
    /// agents/<agentId>/agent/openclaw-agent.sqlite（运行时库）。
    RuntimeStore,
    /// agents/<agentId>/sessions/ 下旧 JSONL 归档。
    LegacyTranscript,
    /// agents/<agentId>/sessions/sessions.json（旧会话行迁移输入）。
    LegacySessionRows,
    /// 非 OpenClaw 文档形状。
    Other,
}

pub(crate) fn classify(path: &Path) -> InputKind {
    fn os_to_str(c: &std::ffi::OsStr) -> Option<&str> {
        c.to_str()
    }
    let components: Vec<&std::ffi::OsStr> = path.components().map(|c| c.as_os_str()).collect();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    // components = [..., "agents", "<agentId>", "agent", "openclaw-agent.sqlite"]
    // 或 [..., "agents", "<agentId>", "sessions", ...]。
    let find_seq = |seq: &[&str]| -> bool {
        'outer: for start in 0..components.len().saturating_sub(seq.len() - 1) {
            for (offset, want) in seq.iter().enumerate() {
                match components.get(start + offset).and_then(|c| os_to_str(c)) {
                    Some(actual) if actual == *want => continue,
                    _ => continue 'outer,
                }
            }
            return true;
        }
        false
    };
    if name == "openclaw-agent.sqlite" && find_seq(&["agents", "agent"]) {
        return InputKind::RuntimeStore;
    }
    if find_seq(&["agents", "sessions"]) {
        if name == "sessions.json" {
            return InputKind::LegacySessionRows;
        }
        if name.ends_with(".jsonl") {
            return InputKind::LegacyTranscript;
        }
    }
    InputKind::Other
}

/// 探测一个候选文件。当前所有输入均 fail closed（见模块头），
/// 返回值携带分类原因，供诊断与能力页展示。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    match classify(path) {
        InputKind::RuntimeStore => {
            let source = match open_source_db(path, short_probe, &StagingLimits::default()) {
                Ok(source) => source,
                Err(CoreError::Sqlite(e)) if is_not_a_database(&e) => {
                    return Ok(DetectOutcome::UnknownFormat {
                        reason: "openclaw runtime store path shape but not a valid sqlite \
                                     database"
                            .to_string(),
                    });
                }
                Err(e) => return Err(e),
            };
            let tables = user_table_count(source.conn())?;
            Ok(DetectOutcome::UnknownFormat {
                reason: format!(
                    "openclaw per-agent runtime store (documented shape \
                     agents/<agentId>/agent/openclaw-agent.sqlite; {tables} user tables); \
                     official docs (A09) do not name any table/column; fail closed \
                     pending a real sample"
                ),
            })
        }
        InputKind::LegacyTranscript => Ok(DetectOutcome::UnknownFormat {
            reason: "openclaw legacy transcript archive (migration/offline-maintenance input \
                     per docs; gateway does not import at startup); entry-level schema not \
                     documented; fail closed pending a real sample"
                .to_string(),
        }),
        InputKind::LegacySessionRows => Ok(DetectOutcome::UnknownFormat {
            reason: "openclaw legacy session-row migration input (sessions.json); migrate via \
                     openclaw doctor --fix; not a usage transcript; fail closed"
                .to_string(),
        }),
        InputKind::Other => Ok(DetectOutcome::UnknownFormat {
            reason: "file does not match a documented openclaw input shape \
                     (agents/<agentId>/agent/openclaw-agent.sqlite or sessions/ legacy inputs)"
                .to_string(),
        }),
    }
}
