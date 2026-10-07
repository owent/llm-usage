//! Claude Code 探测：有界读取首行，按文档化记录类型集合确认 Agent 身份。
//!
//! 文件首行仅作格式门禁，不认证其他行的客户端版本；扫描按每条 assistant
//! 自带 version 选择规则，队列元数据或安装版本不能认证历史用量。

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
