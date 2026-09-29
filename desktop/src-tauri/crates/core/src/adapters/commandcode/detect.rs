//! Command Code 探测：v3 树形会话 JSONL 首行 header 指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const COMMANDCODE_FORMAT: &str = "commandcode-tree-v3-jsonl";

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
            reason: "first line is not JSON (not a Command Code session)".to_string(),
        });
    };
    let is_header = value.get("type").and_then(|v| v.as_str()) == Some("session")
        && value.get("version").is_some()
        && value.get("id").is_some();
    if !is_header {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "missing Command Code session header fingerprint".to_string(),
        });
    }
    match value.get("version").and_then(|v| v.as_i64()) {
        Some(3) => Ok(DetectOutcome::Supported {
            format: COMMANDCODE_FORMAT.to_string(),
            format_version: Some(versions::COMMANDCODE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        }),
        // 上游 detectSessionFileVersion：version > 3 拒开；< 3 由产品打开时
        // 自动迁移（.v2.bak）。未迁移文件按不兼容处理（fail closed）。
        other => Ok(DetectOutcome::UnsupportedVersion {
            format: COMMANDCODE_FORMAT.to_string(),
            found: other.map(|v| v.to_string()),
            reason: "session header version is not 3 (legacy files migrate on open by the product)"
                .to_string(),
        }),
    }
}
