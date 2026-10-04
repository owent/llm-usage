//! Junie CLI 探测：sessions/&lt;id&gt;/events.jsonl 的文档级指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const JUNIE_FORMAT: &str = "junie-events-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;
/// 事件日志已确认但头部窗口无用量指纹时，继续分块搜索的上限
/// （用量事件可能位于大文件后段；有界读取防失控）。
const DETECT_MAX_SCAN_BYTES: usize = 4 * 1024 * 1024;

fn has_usage_fingerprint(text: &str) -> bool {
    text.contains("LlmResponseMetadataEvent") || text.contains("\"modelUsage\"")
}

fn has_event_fingerprint(text: &str) -> bool {
    text.contains("\"agentEvent\"") || text.contains("\"kind\"")
}

/// 探测一个 events.jsonl：用量事件指纹（LlmResponseMetadataEvent/modelUsage）。
///
/// 已确认是 Junie 类型化事件日志但头部 64 KiB 窗口内尚无用量指纹时，
/// 分块搜索至 4 MiB；仍无指纹返回 Pending（会话早期/无 LLM 调用，下轮重探），
/// 而不是误报 UnknownFormat 让真实会话永远无法入账。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // 瞬态不可读（持锁/超时/枚举后被清理）⇒ Pending 下轮重探，不固化失败。
    let mut file = match crate::adapters::run_policy::checked_file(path) {
        Ok(file) => file,
        Err(err) if crate::adapters::framework::is_transient_io(&err) => {
            return Ok(DetectOutcome::Pending);
        }
        Err(err) => return Err(err.into()),
    };
    let mut scanned = String::new();
    let mut head_checked = false;
    let mut total = 0usize;
    let mut chunk = [0u8; DETECT_HEAD_BYTES];
    loop {
        let n = match file.read(&mut chunk) {
            Ok(n) => n,
            Err(err) if crate::adapters::framework::is_transient_io(&err) => {
                return Ok(DetectOutcome::Pending);
            }
            Err(err) => return Err(err.into()),
        };
        if n == 0 {
            if !head_checked && scanned.is_empty() {
                return Ok(DetectOutcome::Pending);
            }
            break;
        }
        total += n;
        scanned.push_str(&String::from_utf8_lossy(&chunk[..n]));
        if !head_checked {
            head_checked = true;
            if !has_event_fingerprint(&scanned) {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "missing Junie agent-event fingerprint".to_string(),
                });
            }
            if has_usage_fingerprint(&scanned) {
                return Ok(DetectOutcome::Supported {
                    format: JUNIE_FORMAT.to_string(),
                    format_version: Some(versions::JUNIE_FORMAT_VERSION.to_string()),
                    basis: VersionBasis::KnownVersion,
                });
            }
        } else if has_usage_fingerprint(&scanned) {
            // 用量指纹在头部窗口之后、搜索上限之内。
            return Ok(DetectOutcome::Supported {
                format: JUNIE_FORMAT.to_string(),
                format_version: Some(versions::JUNIE_FORMAT_VERSION.to_string()),
                basis: VersionBasis::KnownVersion,
            });
        }
        if total >= DETECT_MAX_SCAN_BYTES {
            break;
        }
    }
    // 类型化事件日志但搜索窗口内无用量事件：尚未可判定（可能会话早期），
    // Pending 下轮重探；与"未知格式"区分（fail closed 语义见 framework.rs）。
    Ok(DetectOutcome::Pending)
}
