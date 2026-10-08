//! Kimi Work wire_v14 JSONL reader, protocol_version 1.4.
//!
//! Embedded kimi-code home checked locally on 2026-09-25:
//! redacted tests/fixtures/kimi-work/{conv-main,agent-44-subagent}.
//! - …/Kimi/share/daimon-share/daimon/runtime/kimi-code/home/sessions/
//!   <wd_hash>/<conv-<hexid>|ctitle-<uuid>>/agents/<agent>/wire.jsonl.
//!   daimon host and state.json createdBy=daimon-kernel-adapter identify the product.
//! - usage.record{model, usage{inputOther,output,inputCacheRead,
//!   inputCacheCreation}, usageScope: turn|session, time} differs from 1.5:
//!   no agentId field; agents/<id>/ supplies identity, and model is a bare ID.
//! - Real echo deduplication, child snapshots, millisecond times and four exclusive
//!   buckets match 1.5, so the verified semantics share kimi_wire.
//!   1.4-specific tools.register_user_tool, permission.record_approval_result
//!   and micro_compaction.apply are explicitly excluded nonusage event types.
//!
//! Bind kimi-work namespace/Agent identity here, retaining independent statistics.

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::kimi_wire::{scan_wire, WireProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "kimi-wire-14-1";

/// Product binding: kimi-work event namespace and Agent name.
pub(crate) const PRODUCT: WireProduct = WireProduct {
    ns: "kimi-work",
    agent: "kimi-work",
    parser_version: PARSER_VERSION,
};

/// Incrementally scan wire.jsonl through KimiWorkAdapter::scan;
/// detection and scanning share version selection in super::super::versions.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    scan_wire(target, stored, limits, now_ms, &PRODUCT, &|found| {
        super::select(found).basis
    })
}
