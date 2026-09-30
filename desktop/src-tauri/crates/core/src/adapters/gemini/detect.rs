//! Gemini 探测：整写会话 JSON 的文档级指纹（architecture.md#adapter-layout）。
//!
//! gemini 会话 JSON 无版本字段，格式版本恒为文档级
//! [`super::versions::GEMINI_FORMAT_VERSION`]；不做版本分派、
//! 不存在未知版本回退（区别于 codex 的注册表分派）。
//!
//! 合同（V17 fail closed）：
//! - 文件头 64 KiB（剥 UTF-8 BOM）不以 JSON object 开头 ⇒ 未知格式，
//!   不把任意未知文件交给猜测逻辑；
//! - 缺 sessionId/messages 指纹 ⇒ 未知格式；
//! - 只有 sessionId（可能仍在首次写入中）⇒ Pending，下轮重探；
//! - 指纹成立 ⇒ Supported，文档级格式版本是注册表唯一已收录条目，
//!   选择依据恒为 KnownVersion。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const GEMINI_FORMAT: &str = "gemini-session-json";

/// 探测窗口：文件头 64 KiB 指纹（有界读取，不解析全文件）。
const DETECT_HEAD_BYTES: usize = 64 * 1024;

/// 探测一个会话 JSON 文件。
/// 无版本字段可分派：指纹成立即返回固定文档级格式版本（恒为 KnownVersion）。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // 瞬态不可读（持锁/超时/枚举后被清理）⇒ Pending 下轮重探，不固化失败。
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(super::strip_bom(&head));
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    if !trimmed.starts_with('{') {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "session file does not start with a JSON object".to_string(),
        });
    }
    let has_session_id = trimmed.contains("\"sessionId\"");
    let has_messages = trimmed.contains("\"messages\"");
    if has_session_id && has_messages {
        // 无版本字段：格式版本为文档级 session-doc-1，注册表唯一已收录条目，
        // 选择依据恒为 KnownVersion（不存在"未知版本"状态）。
        Ok(DetectOutcome::Supported {
            format: GEMINI_FORMAT.to_string(),
            format_version: Some(versions::GEMINI_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else if has_session_id {
        // 只有 sessionId：可能仍在首次写入中，下轮重探。
        Ok(DetectOutcome::Pending)
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing sessionId/messages fingerprint".to_string(),
        })
    }
}
