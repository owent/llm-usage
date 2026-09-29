//! OTel 探测：spans JSONL 指纹（span 记录键/属性键，双拼写容错）。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const OTEL_FORMAT: &str = "otel-spans-jsonl";

const DETECT_HEAD_BYTES: usize = 64 * 1024;

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; DETECT_HEAD_BYTES];
    let n = file.read(&mut head)?;
    head.truncate(n);
    let text = String::from_utf8_lossy(&head);
    if text.trim().is_empty() {
        return Ok(DetectOutcome::Pending);
    }
    // 指纹：gen_ai / agentlens 命名空间或 span 记录形状。
    let has_genai = text.contains("gen_ai.");
    let has_span_shape = text.contains("\"spanId\"")
        || text.contains("\"span_id\"")
        || text.contains("\"startTime\"")
        || text.contains("\"name\":\"chat\"");
    if has_genai || has_span_shape {
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
