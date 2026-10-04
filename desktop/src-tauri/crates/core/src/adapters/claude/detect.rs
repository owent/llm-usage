//! Claude Code 探测：有界读取首行，按文档化记录类型集合确认 Agent 身份。
//!
//! claude transcript 无 CLI 版本字段可读：格式锚点是文档级格式版本
//! transcript-doc-1（按 A01 文档定义），detect 成功即 KnownVersion，
//! 不存在"未知版本"状态；未文档化记录类型 ⇒ 未知格式，fail closed，
//! 不把任意未知文件交给猜测逻辑（V17）。

use crate::error::CoreError;
use std::path::Path;

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;

use super::versions::CLAUDE_FORMAT_VERSION;

pub const CLAUDE_FORMAT: &str = "claude-transcript-jsonl";

/// 探测一个 transcript 文件；无版本字段可回退，格式版本固定为文档级锚点。
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
        Some("user") | Some("assistant") | Some("system") => Ok(DetectOutcome::Supported {
            format: CLAUDE_FORMAT.to_string(),
            format_version: Some(CLAUDE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        }),
        // 2026-09-30 真实样本核验结果（Claude Code 2.1.197，WSL）：排队/附件/last-prompt
        // 元数据记录可出现在文件首行；它们不是用量载体，探测放行，
        // 逐行解析阶段仍按白名单处理（transcript_doc1）。
        Some("queue-operation") | Some("attachment") | Some("last-prompt") => {
            Ok(DetectOutcome::Supported {
                format: CLAUDE_FORMAT.to_string(),
                format_version: Some(CLAUDE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            })
        }
        other => Ok(DetectOutcome::UnknownFormat {
            reason: format!("first record type {other:?} not in documented set"),
        }),
    }
}
