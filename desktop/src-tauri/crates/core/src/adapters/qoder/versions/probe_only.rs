//! Fallback probe for unverified state.json and other Qoder persistence formats.
//! CLI transcript fields have an independent source-derived reader; Credits are not tokens.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, path::Path};

pub const QODER_PARSER_VERSION: &str = "qoder-probe-2";
pub const SEALED_STATE_FORMAT: &str = "qoder-sealed-session-state";

// Session state is a complete JSON document. Only an empty plaintext map plus
// well-formed encrypted items establishes that this file has no readable usage.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionState {
    session_id: String,
    #[serde(rename = "revision")]
    _revision: u64,
    created_at: String,
    updated_at: String,
    data: BTreeMap<String, serde::de::IgnoredAny>,
    items: BTreeMap<String, SealedItem>,
    #[serde(default, rename = "workspaceDirectories")]
    _workspace_directories: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SealedItem {
    c: String,
    u: String,
    n: String,
    p: String,
    t: String,
}

fn base64_field(value: &str) -> bool {
    let body = value.trim_end_matches('=');
    !body.is_empty()
        && value.len().is_multiple_of(4)
        && value.len() - body.len() <= 2
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/'))
}

pub(crate) fn sealed_state(path: &Path) -> Result<bool, CoreError> {
    if path.file_name().is_none_or(|n| n != "state.json") {
        return Ok(false);
    }
    let Some(session) = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        .filter(|s| super::super::detect::is_uuid(s))
    else {
        return Ok(false);
    };
    const LIMIT: usize = crate::adapters::jsonl::DEFAULT_MAX_LINE_BYTES;
    let mut bytes = Vec::new();
    crate::adapters::run_policy::checked_file(path)?
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Ok(false);
    }
    let Ok(state) = crate::adapters::run_policy::json_from_slice::<SessionState>(&bytes) else {
        return Ok(false);
    };
    let timestamp = |value: &str| value.parse::<jiff::Timestamp>().is_ok();
    Ok(state.session_id == session
        && timestamp(&state.created_at)
        && timestamp(&state.updated_at)
        && state.data.is_empty()
        && !state.items.is_empty()
        && state.items.values().all(|item| {
            timestamp(&item.c)
                && timestamp(&item.u)
                && item.n.len() == 16
                && base64_field(&item.n)
                && !item.n.ends_with('=')
                && item.t.len() == 24
                && item.t.ends_with("==")
                && base64_field(&item.t)
                && base64_field(&item.p)
        }))
}

pub(crate) fn legacy_probe(stored: &StoredScanState) -> bool {
    stored.parse_context.as_ref().is_some_and(|context| {
        context.as_object().is_some_and(|fields| fields.len() == 1)
            && context["parser_version"] == "qoder-transcript-2"
    })
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ProbeCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // Do not parse/import usage from unverified formats.
    // Store file length as cursor so unchanged files skip repeated diagnostics in the framework.
    // Later file changes trigger another probe through framework change detection.
    let sealed = sealed_state(&target.path)?;
    let diagnostics = if sealed {
        vec![]
    } else {
        vec![DiagnosticInput {
            event_id: None,
            code: "usage_fields_unverified".into(),
            field: None,
            position: Some(crate::adapters::framework::normalize_path(&target.path)),
            message: "This Qoder persistence format has no verified local token mapping".into(),
        }]
    };
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(ProbeCursor {
            generation: target.generation,
            offset: target.probe.len,
        })?),
        parse_context: Some(
            serde_json::json!({"parser_version":QODER_PARSER_VERSION,"sealed_state":sealed}),
        ),
        events: Vec::new(),
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 0,
        records_seen: 0,
        reconciliations: Vec::new(),
        health: if sealed { "active" } else { "degraded" }.into(),
    })
}
