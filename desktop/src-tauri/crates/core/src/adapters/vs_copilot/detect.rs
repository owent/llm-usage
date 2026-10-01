//! VS Copilot 遥测探测：首行 OTLP 信封 + service.name=vs-copilot 指纹。

use crate::adapters::framework::DetectOutcome;
use crate::domain::VersionBasis;
use crate::error::CoreError;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub const VS_COPILOT_FORMAT: &str = "vs-copilot-otlp-traces";

/// 探测首行读取上限（遥测批次行可含长 messages 属性，按单行上限有界读取）。
const DETECT_MAX_FIRST_LINE: usize = crate::adapters::jsonl::DEFAULT_MAX_LINE_BYTES;

fn read_first_line(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let file = std::fs::File::open(path)?;
    let mut reader =
        BufReader::with_capacity(64 * 1024, file.take(DETECT_MAX_FIRST_LINE as u64 + 1));
    let mut buf = Vec::new();
    let n = reader.read_until(b'\n', &mut buf)?;
    if n == 0 || buf.last() != Some(&b'\n') {
        return Ok(None);
    }
    if buf.len() > DETECT_MAX_FIRST_LINE {
        return Ok(None);
    }
    while buf.last() == Some(&b'\n') || buf.last() == Some(&b'\r') {
        buf.pop();
    }
    if buf.starts_with(b"\xEF\xBB\xBF") {
        buf.drain(0..3);
    }
    Ok(Some(buf))
}

/// 解析 OTLP 属性数组中的 service.name（本产品=vs-copilot/visualstudio 命名空间）。
pub(crate) fn service_name_of(attrs: &[serde_json::Value]) -> Option<String> {
    for attr in attrs {
        let Some(key) = attr.get("key").and_then(|v| v.as_str()) else {
            continue;
        };
        if key != "service.name" && key != "service_name" {
            continue;
        }
        let value = attr.get("value").and_then(|v| {
            v.as_str()
                .or_else(|| v.get("stringValue").and_then(|s| s.as_str()))
                .map(str::to_string)
        });
        return value;
    }
    None
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    let line = match read_first_line(path) {
        Ok(line) => line,
        Err(err) if crate::adapters::framework::is_transient_io(&err) => {
            return Ok(DetectOutcome::Pending);
        }
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "not a readable telemetry file".to_string(),
            });
        }
    };
    let Some(line) = line else {
        return Ok(DetectOutcome::Pending);
    };
    let doc: serde_json::Value = match serde_json::from_slice(&line) {
        Ok(doc) => doc,
        Err(_) => {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "first line is not JSON".to_string(),
            });
        }
    };
    let Some(batches) = doc.get("resourceSpans").and_then(|v| v.as_array()) else {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "first line lacks OTLP resourceSpans envelope".to_string(),
        });
    };
    let mut service = None;
    for batch in batches {
        let attrs = batch
            .pointer("/resource/attributes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if let Some(name) =
            service_name_of(&attrs).filter(|n| n == "vs-copilot" || n == "visualstudio-copilot")
        {
            service = Some(name);
            break;
        }
    }
    match service.as_deref() {
        Some("vs-copilot") | Some("visualstudio-copilot") => Ok(DetectOutcome::Supported {
            format: VS_COPILOT_FORMAT.to_string(),
            format_version: Some("1".to_string()),
            basis: VersionBasis::KnownVersion,
        }),
        Some(other) => Ok(DetectOutcome::UnknownFormat {
            reason: format!("OTLP telemetry for another service ({other})"),
        }),
        None => Ok(DetectOutcome::UnknownFormat {
            reason: "OTLP envelope without service.name".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log(name: &str, lines: &[String]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-vs-copilot-detect-{}-{}-{}.jsonl",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(&path, text).unwrap();
        path
    }

    fn envelope(service: &str) -> String {
        format!(
            r#"{{"resourceSpans":[{{"resource":{{"attributes":[{{"key":"service.name","value":{{"stringValue":"{service}"}}}}]}},"scopeSpans":[{{"spans":[]}}]}}]}}"#
        )
    }

    #[test]
    fn vs_copilot_envelope_is_supported() {
        let path = temp_log("ok", &[envelope("vs-copilot")]);
        match detect(&path).unwrap() {
            DetectOutcome::Supported {
                format,
                format_version,
                basis,
            } => {
                assert_eq!(format, VS_COPILOT_FORMAT);
                assert_eq!(format_version.as_deref(), Some("1"));
                assert_eq!(basis, VersionBasis::KnownVersion);
            }
            other => panic!("expected Supported, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn other_service_is_unknown_format() {
        let path = temp_log("other", &[envelope("codebuddy-code")]);
        assert!(matches!(
            detect(&path).unwrap(),
            DetectOutcome::UnknownFormat { .. }
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn plain_span_line_is_unknown_format() {
        let path = temp_log(
            "plain",
            &[r#"{"name":"chat gpt-5.3-codex","spanId":"ab"}"#.to_string()],
        );
        assert!(matches!(
            detect(&path).unwrap(),
            DetectOutcome::UnknownFormat { .. }
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_file_is_pending() {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-vs-copilot-detect-empty-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&path, b"").unwrap();
        assert!(matches!(detect(&path).unwrap(), DetectOutcome::Pending));
        let _ = std::fs::remove_file(&path);
    }
}
