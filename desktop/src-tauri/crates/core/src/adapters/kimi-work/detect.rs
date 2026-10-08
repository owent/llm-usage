//! Bounded Kimi Work metadata-header detection uses shared family fields,
//! then selects through its own super::versions registry, verified protocol 1.4.
//!
//! Rules: architecture.md#unknown-version, V17/V30.
//! - Non-JSON/non-metadata first line is unknown format and rejected.
//! - Registered protocol_version 1.4 selects KnownVersion.
//! - Missing/unregistered, including Kimi Code 1.5 which is unverified here,
//!   uses marked LatestFallback compatibility attempts.

use crate::adapters::framework::DetectOutcome;
use crate::adapters::kimi_wire::{read_metadata_head, HeadProbe};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const KIMI_WORK_FORMAT: &str = "kimi-wire-jsonl";

/// Detect wire.jsonl with family-shared metadata fields; discovery identifies product.
/// Format registries remain independent for Kimi Work/Code (A12/A13).
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    match read_metadata_head(path)? {
        HeadProbe::Pending => Ok(DetectOutcome::Pending),
        HeadProbe::NotMetadata(reason) => Ok(DetectOutcome::UnknownFormat { reason }),
        HeadProbe::Metadata(head) => {
            let selection = versions::select(head.protocol_version.as_deref());
            Ok(DetectOutcome::Supported {
                format: KIMI_WORK_FORMAT.to_string(),
                format_version: head.protocol_version,
                basis: selection.basis,
            })
        }
    }
}
