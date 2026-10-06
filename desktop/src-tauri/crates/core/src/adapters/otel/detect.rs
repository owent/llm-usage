//! OTel 探测：spans JSONL 指纹（span 记录键/属性键，双拼写容错）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const OTEL_FORMAT: &str = "otel-spans-jsonl";

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
    if let Some(detected) = super::versions::qwen_sdk_025::detect_head(&head) {
        return Ok(detected);
    }
    // 指纹：gen_ai / agentlens 命名空间或 span 记录形状。
    // （"name":"chat" 之类对空白敏感的窄指纹不采用：合法 JSON 序列化
    //   允许键值间空白，spanId/startTime 已足够判定形状。）
    let has_genai = text.contains("gen_ai.");
    let has_span_shape = text.contains("\"spanId\"")
        || text.contains("\"span_id\"")
        || text.contains("\"startTime\"");
    // File exporters mix metrics/logs and spans. An initial metrics-only batch
    // still identifies this carrier; the parser skips non-call records.
    let has_export_shape = text.contains("\"scopeMetrics\"")
        || text.contains("\"resourceMetrics\"")
        || text.contains("\"scopeLogs\"");
    if has_genai || has_span_shape || has_export_shape {
        Ok(DetectOutcome::Supported {
            format: OTEL_FORMAT.to_string(),
            format_version: Some(versions::OTEL_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing OTel span record fingerprint".to_string(),
        })
    }
}
