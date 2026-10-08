//! Bounded read-only export checks, without client invocation or ingestion.
use super::*;
use llm_usage_core::adapters::{
    framework::{ScanLimits, ScanTarget, StoredScanState},
    jsonl::{probe_file, JsonlCursor},
    otel::spans_doc1,
};
use std::collections::BTreeMap;
use std::io::{BufRead, Seek, SeekFrom};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
type CachedEvidence = (
    llm_usage_core::adapters::jsonl::FileProbe,
    Instant,
    String,
    usize,
);
type EvidenceCache = Mutex<BTreeMap<(String, PathBuf), CachedEvidence>>;
static CACHE: OnceLock<EvidenceCache> = OnceLock::new();
fn cache() -> &'static EvidenceCache {
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn expected_agent(id: &str) -> &str {
    if id.starts_with("copilot-vscode") {
        "vscode-copilot-chat"
    } else if id == "qwen" {
        "qwen-code"
    } else {
        id
    }
}

/// A record identifies the client family, not the originating VS Code profile.
/// Distinct client families may share a receiver; indistinguishable profiles cannot
/// each use the same records to confirm their own configuration is active.
pub(super) fn verify_all(mut targets: Vec<Target>) -> Vec<Target> {
    let mut owners = BTreeMap::new();
    for target in &targets {
        let path = Path::new(&target.dto.output_path);
        if let Ok(path) = path.canonicalize() {
            *owners
                .entry((expected_agent(&target.dto.id).to_string(), path))
                .or_insert(0usize) += 1;
        }
    }
    for target in &mut targets {
        verify(target);
        if target.dto.verification == "verified" {
            if let Ok(path) = Path::new(&target.dto.output_path).canonicalize() {
                if owners
                    .get(&(expected_agent(&target.dto.id).to_string(), path))
                    .is_some_and(|n| *n > 1)
                {
                    target.dto.verification = "external".into();
                    target.dto.verified_records = 0;
                }
            }
        }
    }
    targets
}

pub(super) fn verify(target: &mut Target) {
    if target.dto.verification == "external" {
        return;
    }
    target.dto.verified_records = 0;
    target.dto.verification = "waiting".into();
    let path = Path::new(&target.dto.output_path);
    if !path.is_absolute() || safe_path(path).is_err() {
        target.dto.verification = "unavailable".into();
        return;
    }
    let probe = match probe_file(path) {
        Ok(probe) => probe,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                target.dto.verification = "unavailable".into();
            }
            return;
        }
    };
    if probe.len == 0 {
        return;
    }
    let cache_key = (target.dto.id.clone(), path.to_path_buf());
    if let Some((cached, checked, status, count)) = cache().lock().unwrap().get(&cache_key) {
        if *cached == probe && checked.elapsed() < Duration::from_secs(300) {
            target.dto.verification = status.clone();
            target.dto.verified_records = *count;
            return;
        }
    }
    // Read the newest 2 MiB from a complete-line boundary, within time/row limits.
    let mut offset = probe.len.saturating_sub(2 * 1024 * 1024);
    if target.dto.id == "qwen" {
        offset = 0;
    }
    if offset > 0 {
        let Ok(mut file) = std::fs::File::open(path) else {
            return;
        };
        if file.seek(SeekFrom::Start(offset)).is_err() {
            return;
        }
        let mut reader = std::io::BufReader::new(file.take(256 * 1024));
        let mut first = Vec::new();
        let Ok(n) = reader.read_until(b'\n', &mut first) else {
            return;
        };
        if first.last() != Some(&b'\n') {
            target.dto.verification = "unrecognized".into();
            return;
        }
        offset += n as u64;
    }
    let scan_target = ScanTarget {
        instance_id: "telemetry-check".into(),
        path: path.to_path_buf(),
        file_id: "check".into(),
        file_identity: "check".into(),
        probe,
        generation: 0,
        rescan: false,
    };
    let limits = ScanLimits {
        jsonl: llm_usage_core::adapters::jsonl::JsonlLimits {
            max_lines: Some(2000),
            max_line_bytes: 256 * 1024,
            time_budget: Some(Duration::from_millis(200)),
            ..Default::default()
        },
    };
    let stored = StoredScanState {
        cursor: Some(
            serde_json::to_value(JsonlCursor {
                generation: 0,
                offset,
                line_number: 1,
            })
            .unwrap(),
        ),
        parse_context: Some(json!({"policy_version":3})),
    };
    let expected = expected_agent(&target.dto.id);
    if expected == "qwen-code" {
        if let Ok(outcome) =
            llm_usage_core::adapters::otel::versions::qwen_sdk_025::scan_with_byte_budget(
                &scan_target,
                &StoredScanState::default(),
                &limits,
                jiff::Timestamp::now().as_millisecond(),
                2 * 1024 * 1024,
            )
        {
            target.dto.verified_records = outcome
                .events
                .iter()
                .filter(|e| {
                    e.agent == expected
                        && e.parse_basis == Some(llm_usage_core::domain::VersionBasis::KnownVersion)
                        && (e.usage.input_total.is_some() || e.usage.output_total.is_some())
                })
                .count();
        }
    }
    if expected != "qwen-code" {
        if let Ok(outcome) = spans_doc1::scan_with_byte_budget(
            &scan_target,
            &stored,
            &limits,
            jiff::Timestamp::now().as_millisecond(),
            Some(2 * 1024 * 1024),
        ) {
            target.dto.verified_records = outcome
                .events
                .iter()
                .filter(|e| e.agent == expected)
                .count();
        }
    }
    if target.dto.verified_records == 0 && offset > 0 {
        let stored = StoredScanState {
            cursor: None,
            parse_context: Some(json!({"policy_version":3})),
        };
        if let Ok(outcome) = spans_doc1::scan_with_byte_budget(
            &scan_target,
            &stored,
            &limits,
            jiff::Timestamp::now().as_millisecond(),
            Some(8 * 1024 * 1024),
        ) {
            target.dto.verified_records = outcome
                .events
                .iter()
                .filter(|e| e.agent == expected)
                .count();
        }
    }
    // Receiver has already extracted permitted fields from supplemental logs. Check
    // exact client event names, timestamps and nonnegative integer usage.
    if target.dto.verified_records == 0 && matches!(expected, "claude" | "codex") {
        if let Ok(read) = llm_usage_core::adapters::jsonl::read_jsonl_with_byte_budget(
            path,
            offset,
            1,
            &limits.jsonl,
            Some(2 * 1024 * 1024),
        ) {
            for line in read.lines {
                let Ok(value) = serde_json::from_str::<Value>(&line.text) else {
                    continue;
                };
                let name = value["name"].as_str().unwrap_or_default();
                let matches = if expected == "claude" {
                    matches!(name, "claude_code.api_request" | "claude_code.api_error")
                } else {
                    matches!(
                        name,
                        "codex.api_request" | "codex.sse_event" | "codex.websocket_event"
                    )
                };
                let timestamp = value["time_unix_nano"]
                    .as_str()
                    .and_then(|s| s.parse::<i64>().ok())
                    .or_else(|| value["time_unix_nano"].as_i64());
                let usage = value["attributes"].as_object().is_some_and(|a| {
                    a.iter().any(|(key, v)| {
                        matches!(
                            key.as_str(),
                            "input_tokens"
                                | "output_tokens"
                                | "gen_ai.usage.input_tokens"
                                | "gen_ai.usage.output_tokens"
                        ) && v.as_i64().is_some_and(|n| {
                            (0..=llm_usage_core::domain::MAX_TOKEN_VALUE).contains(&n)
                        })
                    })
                });
                if matches
                    && timestamp
                        .is_some_and(|t| t / 1_000_000 >= llm_usage_core::domain::MIN_PLAUSIBLE_MS)
                    && usage
                {
                    target.dto.verified_records += 1;
                }
            }
        }
    }
    target.dto.verification = if target.dto.verified_records > 0 {
        "verified"
    } else {
        "unrecognized"
    }
    .into();
    let mut cache = cache().lock().unwrap();
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(
        cache_key,
        (
            scan_target.probe,
            Instant::now(),
            target.dto.verification.clone(),
            target.dto.verified_records,
        ),
    );
}
