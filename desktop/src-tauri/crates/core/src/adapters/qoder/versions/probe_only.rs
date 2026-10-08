//! Qoder CLI session-file probe: probe_only, qoder-pending-evidence.
//!
//! Preimplementation format checks on 2026-09-29: official documentation, unpacked
//! npm @qoder-ai/qodercli 1.1.64 and third-party scripts.
//! - Verified paths: ~/.qoder/projects/<processed-project-path-name>/<session-id>.jsonl
//!   conversation logs and <session-id>/state.json; QODER_CONFIG_DIR redirects
//!   the root directory.
//! - Persisted usage fields remain unverified. Obfuscated bundle code contains state.json
//!   modelRequests[]/compact_token_usage_json schema strings and OTel-to-storage
//!   input_tokens/cache_read_tokens mappings, without verifying actual persistence.
//!   /usage reports cloud Credits under docs.qoder.com/cli/usage.md.
//! - Discover/identify session files only, without parsing any usage field.
//!   Reject unverified reading with diagnostics until native samples establish actual
//!   state.json/session JSONL fields.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

pub const QODER_PARSER_VERSION: &str = "qoder-probe-1";

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
    // Do not parse/import usage; diagnostics describe unverified fields and required samples.
    // Store file length as cursor so unchanged files skip repeated diagnostics in the framework.
    // Later file changes trigger another probe through framework change detection.
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(ProbeCursor {
            generation: target.generation,
            offset: target.probe.len,
        })?),
        parse_context: None,
        events: Vec::new(),
        aggregates: Vec::new(),
        diagnostics: vec![DiagnosticInput {
            event_id: None,
            code: "usage_fields_unverified".to_string(),
            field: None,
            position: Some(crate::adapters::framework::normalize_path(&target.path)),
            message: "Qoder usage persistence is unverified (bundle schema hints only; \
billing is cloud Credits); parsing disabled pending a local fixture of \
<session-id>.jsonl and <session-id>/state.json"
                .to_string(),
        }],
        lines_read: 0,
        records_seen: 0,
        reconciliations: Vec::new(),
        health: "degraded".to_string(),
    })
}
