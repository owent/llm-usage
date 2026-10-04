//! DSH 探测：持久会话日志 JSONL 首行事件指纹。
//!
//! 持久日志无版本字段，格式版本恒为文档级
//! [`super::versions::DSH_FORMAT_VERSION`]（固定 token-meter README 46a7f68）；
//! 不做版本分派、不存在未知版本回退。
//!
//! 约定（V17 fail closed）：
//! - 首行不是 JSON ⇒ 未知格式；
//! - 首行 type 缺失或不在 README 枚举的六事件集合 ⇒ 未知格式；
//! - 空文件 ⇒ Pending，下轮重探；
//! - 指纹成立 ⇒ Supported（恒为 KnownVersion）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const DSH_FORMAT: &str = "dsh-session-log-jsonl";

/// 探测一个持久会话日志文件。
/// 无版本字段可分派：指纹成立即返回固定文档级格式版本（恒为 KnownVersion）。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let limits = super::super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: super::super::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = super::super::jsonl::read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(DetectOutcome::Pending);
    };
    let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not JSON".to_string(),
        });
    };
    match line.get("type").and_then(|t| t.as_str()) {
        Some(event_type)
            if versions::session_log_doc1::DOCUMENTED_EVENT_TYPES.contains(&event_type) =>
        {
            Ok(DetectOutcome::Supported {
                format: DSH_FORMAT.to_string(),
                format_version: Some(versions::DSH_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            })
        }
        other => Ok(DetectOutcome::UnknownFormat {
            reason: format!("first event type {other:?} not in documented set"),
        }),
    }
}
