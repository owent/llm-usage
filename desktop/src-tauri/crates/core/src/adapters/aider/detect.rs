//! Aider 探测：`--analytics-log` JSONL 的文档级指纹。
//!
//! 合同（V17 fail closed）：首行 JSON 不含 event/properties/time 结构 ⇒ 未知格式；
//! 空文件 ⇒ Pending 下轮重探。无版本字段：文档级锚点恒为 KnownVersion。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const AIDER_FORMAT: &str = "aider-analytics-jsonl";

/// 探测窗口：文件头 64 KiB（首行可能很长，指纹只看结构键）。
const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; DETECT_HEAD_BYTES];
    let n = file.read(&mut head)?;
    head.truncate(n);
    let text = String::from_utf8_lossy(&head);
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(first_line) else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line is not a JSON object (not an aider analytics log)".to_string(),
        });
    };
    let looks_like = value.is_object()
        && value.get("event").is_some_and(|v| v.is_string())
        && value.get("properties").is_some_and(|v| v.is_object())
        && value.get("time").is_some_and(|v| v.is_number());
    if looks_like {
        Ok(DetectOutcome::Supported {
            format: AIDER_FORMAT.to_string(),
            format_version: Some(versions::AIDER_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "JSON line lacks the event/properties/time fingerprint".to_string(),
        })
    }
}
