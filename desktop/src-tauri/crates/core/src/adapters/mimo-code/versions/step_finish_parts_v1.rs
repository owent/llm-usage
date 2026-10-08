//! Per-step usage reader for mimocode.db part rows: step_finish_parts_v1.
//!
//! A14 fixed source: 456678b6a5afb0eef3fe2754575637218cfb3c84.
//! Native 0.1.15 samples are now checked separately; record versions retain compatibility reading.
//! - Usage comes from part data.type="step-finish" (message-v2.ts
//!   StepFinishPart zod fields: tokens{total?, input, output, reasoning,
//!   cache{read, write}} and cost).
//! - Assistant message.data.tokens aggregates turns; skip it to avoid duplicating steps.
//!   Read only message modelID/providerID for ownership (adapters.md A14),
//!   as required by the Assistant zod schema.
//! - The referenced MiMo session schema lacks OpenCode tokens_* totals, so has no session reconciliation.
//!
//! Shared opencode_family reads the independently checked table layout,
//! using product-specific normalization. This module binds MiMo identity/options.

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::mimo_code::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::adapters::opencode_family::{scan_step_finish_parts, PartProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "mimo-code-step-finish-parts-2";

/// Product binding: event namespace, Agent name and reconciliation options.
pub(crate) const PRODUCT: PartProduct = PartProduct {
    ns: "mimo-code",
    agent: "mimo-code",
    parser_version: PARSER_VERSION,
    // MiMo session.sql.ts lacks cumulative tokens_* columns, so has no reconciliation target.
    reconcile_session_counters: false,
    default_zero_unknown: true,
};

/// Incrementally scan mimocode.db through the MimoCodeAdapter::scan dispatcher.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let _sql = crate::adapters::run_policy::SqliteScope::new(conn)?;
    let fingerprint = schema_fingerprint(conn)?;
    // A schema replaced after detection is rejected here, preserving existing results.
    let Some(fingerprint) = fingerprint else {
        return Err(CoreError::Validation(
            "mimocode.db schema fingerprint no longer matches; fail closed".to_string(),
        ));
    };
    let db_version = crate::adapters::opencode_family::max_session_version(conn)?;
    let selection = super::select(db_version.as_deref());
    scan_step_finish_parts(
        conn,
        target,
        stored,
        now_ms,
        crate::adapters::opencode_family::PartScanContext {
            product: PRODUCT,
            fingerprint,
            db_version,
            basis: selection.basis,
            record_basis: |version| super::select(version).basis,
            row_limit: limits
                .jsonl
                .max_lines
                .unwrap_or(crate::adapters::opencode_family::MAX_ROWS_PER_ROUND as u64)
                .min(crate::adapters::opencode_family::MAX_ROWS_PER_ROUND as u64)
                as i64,
        },
    )
}
