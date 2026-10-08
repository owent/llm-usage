//! Kimi Code wire_v15 JSONL reader, protocol_version 1.5.
//!
//! Local references checked 2026-09-24/25 with Kimi Code desktop 1.0.3:
//! redacted tests/fixtures/kimi-code/{session-main,subagent-agent-0}.
//! - KIMI_CODE_HOME/sessions/<wd_hash>/session_<uuid>/agents/<agent>/wire.jsonl,
//!   default ~/.kimi-code; first-line metadata{protocol_version:"1.5", created_at}.
//! - usage.record{agentId, model, usage{inputOther,output,inputCacheRead,
//!   inputCacheCreation}, usageScope: turn|session, time} supplies per-call usage.
//! - context.append_loop_event.event(step.end).usage is an echo. Its uuid/messageId/
//!   duration fields have no shared record key: reconcile only, without usage addition.
//! - subagent.completed.usage snapshots child wire sums through completed.time;
//!   do not create events from it or duplicate child usage.
//! - usageScope=session during full_compaction identifies auxiliary summary calls.
//!
//! Shared kimi_wire implements semantics verified against real Kimi Code/Work data.
//! This module binds product identity, retaining separate source statistics.

use crate::adapters::framework::{ScanLimits, ScanOutcome, ScanTarget, StoredScanState};
use crate::adapters::kimi_wire::{scan_wire, WireProduct};
use crate::error::CoreError;

pub const PARSER_VERSION: &str = "kimi-wire-15-1";

/// Product binding: kimi-code event namespace and Agent name.
pub(crate) const PRODUCT: WireProduct = WireProduct {
    ns: "kimi-code",
    agent: "kimi-code",
    parser_version: PARSER_VERSION,
};

/// Incrementally scan wire.jsonl through KimiCodeAdapter::scan;
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
