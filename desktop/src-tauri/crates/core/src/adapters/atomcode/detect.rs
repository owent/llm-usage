//! AtomCode 探测：sessions/&lt;hash&gt;/&lt;id&gt;.meta（SessionMeta）或旧版
//! 单文件 &lt;id&gt;.json（messages+turn_stats）指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::Read;
use std::path::Path;

use super::versions;

pub const ATOMCODE_FORMAT: &str = "atomcode-session-meta-json";

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
    let has_turn_stats = text.contains("\"turn_stats\"");
    let has_id = text.contains("\"id\"");
    if has_turn_stats && has_id {
        Ok(DetectOutcome::Supported {
            format: ATOMCODE_FORMAT.to_string(),
            format_version: Some(versions::ATOMCODE_FORMAT_VERSION.to_string()),
            basis: VersionBasis::KnownVersion,
        })
    } else {
        Ok(DetectOutcome::UnknownFormat {
            reason: "missing AtomCode session meta fingerprint (id/turn_stats)".to_string(),
        })
    }
}
