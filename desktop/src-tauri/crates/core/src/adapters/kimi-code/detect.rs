//! Bounded Kimi Code metadata-header detection uses shared family fields,
//! then selects through the super::versions registry.
//!
//! Rules: architecture.md#unknown-version, V17/V30.
//! - Non-JSON/non-metadata first line is unknown format and rejected.
//! - Registered protocol_version 1.5 selects KnownVersion.
//! - Missing/unregistered versions use marked LatestFallback compatibility attempts;
//!   no incompatible Kimi Code version is confirmed, unlike rejected Pi v1/v2.

use crate::adapters::framework::DetectOutcome;
use crate::adapters::kimi_wire::{read_metadata_head, HeadProbe};
use crate::error::CoreError;
use std::path::Path;

use super::versions;

pub const KIMI_CODE_FORMAT: &str = "kimi-wire-jsonl";

/// Detect wire.jsonl and select a version. Metadata fields are family-shared;
/// discovery roots establish product identity; Kimi Work has an independent registry.
pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    match read_metadata_head(path)? {
        HeadProbe::Pending => Ok(DetectOutcome::Pending),
        HeadProbe::NotMetadata(reason) => Ok(DetectOutcome::UnknownFormat { reason }),
        HeadProbe::Metadata(head) => {
            let selection = versions::select(head.protocol_version.as_deref());
            Ok(DetectOutcome::Supported {
                format: KIMI_CODE_FORMAT.to_string(),
                format_version: head.protocol_version,
                basis: selection.basis,
            })
        }
    }
}
