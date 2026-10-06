//! Cline 探测：整写 ui_messages.json 数组的文档级指纹。
//!
//! ui_messages.json 无版本字段，格式版本恒为文档级
//! [`super::versions::CLINE_FORMAT_VERSION`]（固定源码 dcf8c3c）；不做版本分派、
//! 不存在未知版本回退（区别于 codex 的注册表分派）。
//!
//! 约定（V17 fail closed）：
//! - 文件头 64 KiB（剥 UTF-8 BOM）不以 JSON 数组开头 ⇒ 未知格式，
//!   不把任意未知文件交给猜测逻辑；
//! - 无 say 消息指纹 ⇒ 未知格式；
//! - 空内容 ⇒ Pending，下轮重探；
//! - 指纹成立 ⇒ Supported，文档级格式版本是注册表唯一已收录条目，
//!   选择依据恒为 KnownVersion。say 种类合法性在扫描层逐条核验。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const CLINE_FORMAT: &str = "cline-ui-messages-json";

/// 探测窗口：文件头 64 KiB 指纹（有界读取，不解析全文件）。
const DETECT_HEAD_BYTES: usize = 64 * 1024;

/// 探测一个任务 ui_messages.json 文件。
/// 无版本字段可分派：指纹成立即返回固定文档级格式版本（恒为 KnownVersion）。
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    if versions::sdk_messages_v1::is_sdk_file(path) {
        return versions::sdk_messages_v1::detect(path);
    }
    // 瞬态不可读（持锁/超时/枚举后被清理）⇒ Pending 下轮重探，不固化失败。
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(super::strip_bom(&head));
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    if !trimmed.starts_with('[') {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "task file does not start with a JSON array".to_string(),
        });
    }
    // 指纹：say 消息结构（getApiMetrics.ts 只消费 type="say" 的消息）。
    let has_say = trimmed.contains("\"say\"");
    let has_type = trimmed.contains("\"type\"");
    if has_say && has_type {
        Ok(DetectOutcome::Supported {
            format: CLINE_FORMAT.to_string(),
            format_version: Some(versions::CLINE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else if has_type {
        // 有消息结构但窗口内无 say 消息：不能确认是 Cline 消息数组。
        Ok(DetectOutcome::UnknownFormat {
            reason: "array carries typed messages but no say fingerprint in head window"
                .to_string(),
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing Cline message fingerprint (type/say)".to_string(),
        })
    }
}
