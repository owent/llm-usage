//! Fallback probe for unverified state.json and other Qoder persistence formats.
//! CLI transcript fields have an independent source-derived reader; Credits are not tokens.

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
    // Do not parse/import usage from unverified formats.
    // Store file length as cursor so unchanged files skip repeated diagnostics in the framework.
    // Later file changes trigger another probe through framework change detection.
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(ProbeCursor {
            generation: target.generation,
            offset: target.probe.len,
        })?),
        parse_context: Some(
            serde_json::json!({"parser_version":super::transcript_v1::PARSER_VERSION}),
        ),
        events: Vec::new(),
        aggregates: Vec::new(),
        diagnostics: vec![DiagnosticInput {
            event_id: None,
            code: "usage_fields_unverified".to_string(),
            field: None,
            position: Some(crate::adapters::framework::normalize_path(&target.path)),
            message: "This Qoder persistence format has no verified local token mapping; \
only the independently validated CLI transcript envelope is parsed"
                .to_string(),
        }],
        lines_read: 0,
        records_seen: 0,
        reconciliations: Vec::new(),
        health: "degraded".to_string(),
    })
}
