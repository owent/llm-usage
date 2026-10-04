//! Xum 探测：session-usage.json 的文档级指纹（byModel + lastRequest/version）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const XUM_FORMAT: &str = "xum-session-usage-json";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    // 瞬态不可读（持锁/超时/枚举后被清理）⇒ Pending 下轮重探，不固化失败。
    let Some(head) = crate::adapters::framework::read_detect_head(path, DETECT_HEAD_BYTES)? else {
        return Ok(DetectOutcome::Pending);
    };
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    let has_by_model = text.contains("\"byModel\"");
    let has_anchor = text.contains("\"lastRequest\"") || text.contains("\"version\"");
    if has_by_model && has_anchor {
        // version 整数字段如实判定（V30）：参考解析器（tokscale mux.rs）覆盖
        // version=1/缺失的文档形态；其他整数值是未见形态 ⇒ LatestFallback
        // 兼容尝试，不能虚标 KnownVersion。
        let basis = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(doc) if doc.is_object() => {
                if !doc.get("byModel").is_some_and(|v| v.is_object())
                    || (doc.get("lastRequest").is_none() && doc.get("version").is_none())
                {
                    return Ok(DetectOutcome::UnknownFormat {
                        reason: "Xum session usage lacks top-level byModel/anchor".to_string(),
                    });
                }
                match doc.get("version") {
                    None => VersionBasis::KnownVersion,
                    Some(v) if v.as_u64() == Some(1) => VersionBasis::KnownVersion,
                    _ => VersionBasis::LatestFallback,
                }
            }
            Ok(_) => {
                return Ok(DetectOutcome::UnknownFormat {
                    reason: "Xum session usage must be a JSON object".to_string(),
                });
            }
            // 头部窗口截断或 JSON 无法完整解析时不声明版本已验证。
            Err(_) => VersionBasis::LatestFallback,
        };
        Ok(DetectOutcome::Supported {
            format: XUM_FORMAT.to_string(),
            format_version: Some(versions::XUM_FORMAT_VERSION.to_string()),
            basis,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing byModel/lastRequest fingerprint".to_string(),
        })
    }
}
