//! Per-step usage reader for opencode.db part rows: step_finish_parts_v1.
//!
//! A17 fixed source: 0027387dc5c59793c12dfc531abc78f825ed6868,
//! checked against real redacted 1.18.34 (aec0b9a6) samples.
//! - Usage comes from part data.type="step-finish" with cost and tokens
//!   present under projector.ts usage(); tokens{input, output,
//!   reasoning, cache{read, write}}, with optional total in the vendored client type.
//! - Five session.tokens_* columns sum current step-finish parts; applyUsage updates
//!   incrementally/compensates deletion, and migration 20260510033149 backfills message.data.
//!   These counters reconcile only and are never added as per-call usage (A17).
//! - Assistant message.data.tokens aggregates turns; skip it to avoid step duplication.
//!   Read only message modelID/providerID for ownership.
//!
//! Shared opencode_family reads the independently checked OpenCode/MiMo table layout,
//! retaining product-specific options. This module binds OpenCode identity
//! and read-only opening helpers.

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::opencode::common::{
    open_source_db, schema_fingerprint, short_probe, StagingLimits,
};
use crate::adapters::opencode_family::{scan_step_finish_parts, PartProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "opencode-step-finish-parts-2";

/// Product binding: event namespace, Agent name and reconciliation options.
pub(crate) const PRODUCT: PartProduct = PartProduct {
    ns: "opencode",
    agent: "opencode",
    parser_version: PARSER_VERSION,
    // Five session.tokens_* columns exist in fixed sql.ts and can reconcile per-step sums.
    reconcile_session_counters: true,
    default_zero_unknown: false,
};

/// Incrementally scan opencode.db through the OpenCodeAdapter::scan dispatcher.
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
            "opencode.db schema fingerprint no longer matches; fail closed".to_string(),
        ));
    };
    let (db_version, basis) = crate::adapters::opencode::detect::usage_version_summary(conn)?
        .unwrap_or((None, crate::domain::VersionBasis::LatestFallback));
    scan_step_finish_parts(
        conn,
        target,
        stored,
        now_ms,
        crate::adapters::opencode_family::PartScanContext {
            product: PRODUCT,
            fingerprint,
            db_version,
            basis,
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
