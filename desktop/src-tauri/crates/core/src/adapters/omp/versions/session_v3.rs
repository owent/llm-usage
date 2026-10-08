//! oh-my-pi (omp) session JSONL parser: session_v3.
//!
//! Native reference: Scoop 18.2.7, 58 session files dated 2026-08-21 through 2026-09-24.
//! Selected fields/types were inspected alongside oh-my-pi 62bc57b and omp.exe strings.
//! - Main sessions: ~/.omp/agent/sessions/<encoded-cwd>/<ts>_<uuid>.jsonl.
//!   Subagents: <ts>_<parent_uuid>/<Name>.jsonl; nested subagents:
//!   <ts>_<parent_uuid>/<Name>/<Name>.<sub>.jsonl. Do not read companion .json/.md/.log files.
//!   All 58 files start with type=title and v/title/updatedAt/pad metadata;
//!   exclude title bodies and read the later session header.
//! - All 58 headers have version=3. parentSession was not observed locally;
//!   its compatibility path follows pi. Entry fields are type/id/parentId/timestamp.
//! - Three redacted samples use combined model_change.model=provider/model.
//!   Separate modelId/provider is a compatibility fallback, unobserved in these omp samples.
//! - 8752 assistant entries carry model/provider/stopReason/responseId and
//!   input/output/cacheRead/cacheWrite/totalTokens/cost usage, including error/aborted calls.
//!   85 entries also report reasoningTokens as an output subset; duration/ttft are float milliseconds.
//!   Every checked row has totalTokens=input+output+cacheRead+cacheWrite.
//! - 6643 toolResult, 37 compaction, and 2 branch_summary records have no usage.
//!   tokensBefore/tokensAfter estimate context size; standalone usage was not observed.
//! - Ignore observed title/title_change/credential_pin/session_init/
//!   ttsr_injection/service_tier_change/custom/custom_message metadata entries.
//! - ~/.omp/logs/omp.*.log contained context-size debug estimates, not per-call usage.
//!   No local title-generator record established overlap with session usage.
//!
//! V30 session header selection uses super::super::versions::select.
//! Registered format 3 selects this implementation as KnownVersion.
//! Unregistered/missing versions attempt compatibility reading with parse_basis markers.
//! Unknown version alone does not reject data; older omp formats remain unverified
//! and have no separately checked incompatible-version branch.
//!
//! V30 moved adapters/omp.rs without recreating sources, resetting cursors, or changing scan behavior.

use crate::domain::{CallCategory, EventInput, ModelAttribution, VersionBasis};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::pi::{
    build_pi_family_event, diag, family_entry_key, json_str, parse_entry_ts, parse_usage,
    UsageEventBase,
};
use crate::adapters::usage_map::PiFamilyUsage;

pub const OMP_PARSER_VERSION: &str = "omp-session-1";

/// Persist session identity, model state, unknown types, and version selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OmpParseContext {
    session_id: Option<String>,
    parent_session: Option<String>,
    header_version: Option<i64>,
    model: Option<String>,
    model_provider: Option<String>,
    #[serde(default)]
    unknown_types: Vec<String>,
    /// Format basis known_version/latest_fallback; older contexts default to None.
    /// The V30 move does not recreate sources or reset cursors.
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

fn restore_cursor(stored: &StoredScanState, generation: i64, rescan: bool) -> JsonlCursor {
    if rescan {
        return JsonlCursor {
            generation,
            offset: 0,
            line_number: 1,
        };
    }
    stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
        .filter(|c| c.generation == generation)
        .unwrap_or(JsonlCursor {
            generation,
            offset: 0,
            line_number: 1,
        })
}

fn restore_context(stored: &StoredScanState, rescan: bool) -> OmpParseContext {
    if rescan {
        return OmpParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<OmpParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// Entry key in the omp namespace.
fn entry_key(prefix: &str, entry: &serde_json::Value) -> String {
    family_entry_key("omp", prefix, entry)
}

/// Round float milliseconds to i64; nonfinite/negative/overflow values return None for diagnostics.
fn float_ms(value: Option<&serde_json::Value>) -> Option<Option<i64>> {
    match value {
        None => Some(None),
        Some(v) => {
            let f = v.as_f64()?;
            if !f.is_finite() || f < 0.0 || f > i64::MAX as f64 {
                return None;
            }
            Some(Some(f.round() as i64))
        }
    }
}

/// Check the ts_uuid path shape, such as 2026-08-21T02-19-22-638Z_01a0221d-....
/// Require a digit-leading timestamp with T and a sufficiently long hex-leading ID suffix.
/// All 58 checked main filenames match; subagent filenames/intermediate name directories do not.
fn is_session_dir_shape(name: &str) -> bool {
    let Some(pos) = name.find('_') else {
        return false;
    };
    let (ts, uuid) = name.split_at(pos);
    let uuid = &uuid[1..];
    ts.len() >= 20
        && ts.starts_with(|c: char| c.is_ascii_digit())
        && ts.contains('T')
        && uuid.len() >= 8
        && uuid.starts_with(|c: char| c.is_ascii_hexdigit())
}

/// Main filename follows ts_uuid.jsonl after removing the extension.
fn is_main_session_file(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(is_session_dir_shape)
        .unwrap_or(false)
}

/// For a non-main filename, select the nearest ts_uuid ancestor directory
/// and take the ID after its first underscore as parent session identity.
/// Nested named directories do not become session IDs.
fn subagent_parent_from_path(path: &Path) -> Option<String> {
    if is_main_session_file(path) {
        return None;
    }
    for ancestor in path.ancestors().skip(1) {
        let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_session_dir_shape(name) {
            let pos = name.find('_').unwrap();
            return Some(name[pos + 1..].to_string());
        }
    }
    None
}

/// Construct omp events with rounded duration/ttft; share other mappings with pi.
/// V30 parse context selects known_version/latest_fallback,
/// passed to storage through the shared pi-family constructor.
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    context: &OmpParseContext,
    base: &UsageEventBase<'_>,
    usage: Option<(&PiFamilyUsage, &serde_json::Value)>,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    duration_ms: Option<i64>,
    ttft_ms: Option<i64>,
    parent_from_path: Option<&str>,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    build_pi_family_event(
        target,
        "oh-my-pi",
        OMP_PARSER_VERSION,
        context.session_id.as_deref(),
        // Prefer header parentSession; otherwise use the parent identified from the path.
        context.parent_session.as_deref().or(parent_from_path),
        context.header_version,
        context.version_basis,
        base,
        duration_ms,
        ttft_ms,
        usage,
        occurred_ms,
        source_time,
        line,
        now_ms,
        diagnostics,
    )
}

/// Scan omp session JSONL, dispatched by OmpAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    // Identify subagents from filename/ancestor shapes without depending on root paths
    // or separator differences between enumeration and normalized paths.
    let parent_from_path = subagent_parent_from_path(&target.path);
    let outcome = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    for bad in &outcome.bad_lines {
        crate::adapters::run_policy::check()?;
        diagnostics.push(diag(
            bad.code,
            None,
            bad.number,
            "line is not valid UTF-8; isolated, content not stored",
        ));
    }
    for raw in &outcome.lines {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let Ok(entry) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&raw.text)
        else {
            diagnostics.push(diag(
                "bad_json_line",
                None,
                raw.number,
                "line is not valid JSON; isolated, content not stored",
            ));
            continue;
        };
        let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match entry_type {
            "session" => {
                if context.session_id.is_some() {
                    diagnostics.push(diag(
                        "unexpected_session_header",
                        Some("type"),
                        raw.number,
                        "second session header in one file; first kept",
                    ));
                    continue;
                }
                // Detection/scanning share the registry; registered versions use mapped implementations.
                // Unknown/missing versions keep a compatibility marker and continue parsing.
                // Older omp formats lack the independently checked incompatible rules used by pi.
                let found = entry.get("version").and_then(|v| v.as_i64());
                let selection = super::super::versions::select(found);
                context.version_basis = Some(selection.basis);
                context.header_version = found;
                context.session_id = json_str(&entry, "id").map(str::to_string);
                context.parent_session = json_str(&entry, "parentSession").map(str::to_string);
                if context.session_id.is_none() {
                    diagnostics.push(diag(
                        "missing_session_id",
                        Some("id"),
                        raw.number,
                        "session header without id; events fall back to no session identity",
                    ));
                }
            }
            "model_change" => {
                // Three native redacted datasets use combined model_change.model
                // as provider/model. Separate modelId/provider fields remain a compatibility fallback
                // without local omp acceptance for that shape.
                if let Some(combined) = json_str(&entry, "model") {
                    let (provider, model) = combined.split_once('/').unwrap_or(("", combined));
                    context.model = Some(model.to_string());
                    if !provider.is_empty() {
                        context.model_provider = Some(provider.to_string());
                    }
                } else {
                    if let Some(model) = json_str(&entry, "modelId") {
                        context.model = Some(model.to_string());
                    }
                    if let Some(provider) = json_str(&entry, "provider") {
                        context.model_provider = Some(provider.to_string());
                    }
                }
            }
            "message" => {
                let message = entry
                    .get("message")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let role = message.get("role").and_then(|r| r.as_str()).unwrap_or("");
                match role {
                    "assistant" => {
                        let usage_json = message.get("usage").cloned();
                        let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "entry timestamp missing or unparseable; record skipped",
                            ));
                            continue;
                        };
                        let stop_reason = message
                            .get("stopReason")
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
                        let error_status = match stop_reason {
                            "error" => Some("error".to_string()),
                            "aborted" => Some("aborted".to_string()),
                            _ => None,
                        };
                        let duration_ms = match float_ms(message.get("duration")) {
                            Some(v) => v,
                            None => {
                                diagnostics.push(diag(
                                    "duration_shape_deviation",
                                    Some("message.duration"),
                                    raw.number,
                                    "duration not a finite non-negative number; left unknown",
                                ));
                                None
                            }
                        };
                        let ttft_ms = match float_ms(message.get("ttft")) {
                            Some(v) => v,
                            None => {
                                diagnostics.push(diag(
                                    "ttft_shape_deviation",
                                    Some("message.ttft"),
                                    raw.number,
                                    "ttft not a finite non-negative number; left unknown",
                                ));
                                None
                            }
                        };
                        let base = UsageEventBase {
                            key: entry_key("message", &entry),
                            category: if parent_from_path.is_some() {
                                CallCategory::SubAgent
                            } else {
                                CallCategory::Primary
                            },
                            provider: json_str(&message, "provider"),
                            model: json_str(&message, "model"),
                            attribution: ModelAttribution::RequestField,
                            origin_call_id: json_str(&message, "responseId"),
                            error_status,
                        };
                        match usage_json.as_ref().and_then(parse_usage) {
                            Some(usage) => {
                                events.push(build_usage_event(
                                    target,
                                    &context,
                                    &base,
                                    Some((&usage, usage_json.as_ref().unwrap())),
                                    occurred_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    duration_ms,
                                    ttft_ms,
                                    parent_from_path.as_deref(),
                                    &mut diagnostics,
                                ));
                            }
                            None => {
                                // Missing or invalid assistant usage still counts an observed call
                                // with unknown tokens; all 8752 checked local assistant entries carry usage.
                                diagnostics.push(diag(
                                    "usage_shape_deviation",
                                    Some("message.usage"),
                                    raw.number,
                                    "assistant message without complete usage; call counted, tokens unknown",
                                ));
                                events.push(build_usage_event(
                                    target,
                                    &context,
                                    &base,
                                    None,
                                    occurred_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    duration_ms,
                                    ttft_ms,
                                    parent_from_path.as_deref(),
                                    &mut diagnostics,
                                ));
                            }
                        }
                    }
                    "toolResult" => {
                        // Tool execution usage maps to auxiliary consumption.
                        // All 6643 checked local toolResult entries lack usage.
                        let Some(usage_json) = message.get("usage").cloned() else {
                            continue;
                        };
                        let Some(usage) = parse_usage(&usage_json) else {
                            diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("message.usage"),
                                raw.number,
                                "toolResult usage missing required numeric fields; skipped",
                            ));
                            continue;
                        };
                        let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "entry timestamp missing or unparseable; record skipped",
                            ));
                            continue;
                        };
                        let base = UsageEventBase {
                            key: entry_key("toolresult", &entry),
                            category: CallCategory::Auxiliary,
                            provider: None,
                            model: None,
                            attribution: ModelAttribution::Unknown,
                            origin_call_id: None,
                            error_status: None,
                        };
                        events.push(build_usage_event(
                            target,
                            &context,
                            &base,
                            Some((&usage, &usage_json)),
                            occurred_ms,
                            &source_time,
                            raw.number,
                            now_ms,
                            None,
                            None,
                            parent_from_path.as_deref(),
                            &mut diagnostics,
                        ));
                    }
                    _ => {}
                }
            }
            "usage" => {
                // Standalone usage such as cache_warm is auxiliary with its own provider/model.
                // The pi-compatible path exists without native omp samples for it.
                let usage_json = entry
                    .get("usage")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let Some(usage) = parse_usage(&usage_json) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage entry missing required numeric fields; skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let base = UsageEventBase {
                    key: entry_key("usage", &entry),
                    category: CallCategory::Auxiliary,
                    provider: json_str(&entry, "provider"),
                    model: json_str(&entry, "model"),
                    attribution: ModelAttribution::RequestField,
                    origin_call_id: None,
                    error_status: None,
                };
                events.push(build_usage_event(
                    target,
                    &context,
                    &base,
                    Some((&usage, &usage_json)),
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    None,
                    None,
                    parent_from_path.as_deref(),
                    &mut diagnostics,
                ));
            }
            "compaction" | "branch_summary" => {
                // Optional compaction/branch_summary usage tracks preceding model_change ownership.
                // The checked 37 compaction and 2 branch_summary entries have no usage;
                // tokensBefore/tokensAfter are context estimates and do not count as consumption.
                let Some(usage_json) = entry.get("usage").cloned() else {
                    continue;
                };
                let Some(usage) = parse_usage(&usage_json) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "compaction/branch_summary usage missing required numeric fields; skipped",
                    ));
                    continue;
                };
                let Some((occurred_ms, source_time)) = parse_entry_ts(&entry) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let (attribution, model, provider) = match context.model.clone() {
                    Some(model) => (
                        ModelAttribution::StructuredChange,
                        Some(model),
                        context.model_provider.clone(),
                    ),
                    None => (ModelAttribution::Unknown, None, None),
                };
                let base = UsageEventBase {
                    key: entry_key(entry_type, &entry),
                    category: CallCategory::Auxiliary,
                    provider: provider.as_deref(),
                    model: model.as_deref(),
                    attribution,
                    origin_call_id: None,
                    error_status: None,
                };
                events.push(build_usage_event(
                    target,
                    &context,
                    &base,
                    Some((&usage, &usage_json)),
                    occurred_ms,
                    &source_time,
                    raw.number,
                    now_ms,
                    None,
                    None,
                    parent_from_path.as_deref(),
                    &mut diagnostics,
                ));
            }
            "title"
            | "title_change"
            | "credential_pin"
            | "session_init"
            | "ttsr_injection"
            | "service_tier_change"
            | "thinking_level_change"
            | "custom"
            | "label"
            | "session_info"
            | "custom_message"
            | "context_edit" => {}
            other => {
                if !context.unknown_types.iter().any(|t| t == other) {
                    context.unknown_types.push(other.to_string());
                    diagnostics.push(diag(
                        "unknown_record_type",
                        Some("type"),
                        raw.number,
                        "record type not mapped by this parser version; ignored",
                    ));
                }
            }
        }
    }
    let status = match &outcome.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, .. } => {
            diagnostics.push(diag(
                "line_too_long",
                None,
                *number,
                "line exceeds the 8 MiB limit; cursor held at line start for controlled retry",
            ));
            ScanStatus::LineTooLong
        }
    };
    let new_cursor = JsonlCursor {
        generation: target.generation,
        offset: outcome.next_offset,
        line_number: outcome.next_line_number,
    };
    let degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line" | "usage_shape_deviation" | "line_too_long"
            )
        });
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: outcome.lines.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
        health: if degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_ms_rounds_and_rejects_bad_values() {
        let v = serde_json::json!(17309.34289999993);
        assert_eq!(float_ms(Some(&v)), Some(Some(17309)));
        let half = serde_json::json!(1.5);
        assert_eq!(float_ms(Some(&half)), Some(Some(2)));
        let neg = serde_json::json!(-1.0);
        assert_eq!(float_ms(Some(&neg)), None);
        let text = serde_json::json!("abc");
        assert_eq!(float_ms(Some(&text)), None);
        assert_eq!(float_ms(None), Some(None));
    }

    #[test]
    fn subagent_parent_requires_session_dir_shape() {
        let main = Path::new(
            "/home/u/.omp/agent/sessions/--C--x--/2026-08-21T02-19-22-638Z_01a0221d-a28e-7000-a2a6-a32976b3696f.jsonl",
        );
        assert_eq!(subagent_parent_from_path(main), None);
        let sub = Path::new(
            "/home/u/.omp/agent/sessions/--C--x--/2026-08-21T02-19-22-638Z_01a0221d-a28e-7000-a2a6-a32976b3696f/CommunityResearch.jsonl",
        );
        assert_eq!(
            subagent_parent_from_path(sub),
            Some("01a0221d-a28e-7000-a2a6-a32976b3696f".to_string())
        );
        let nested = Path::new(
            "/home/u/.omp/agent/sessions/--C--x--/2026-09-12T05-01-52-715Z_01a093fe-50cb-72cd-97c8-20029d52ad34/PostgresStoreSlice/PostgresStoreSlice.AwaitifyStoreTests.jsonl",
        );
        assert_eq!(
            subagent_parent_from_path(nested),
            Some("01a093fe-50cb-72cd-97c8-20029d52ad34".to_string())
        );
        // Intermediate directories lacking ts_uuid shape do not establish a subagent parent.
        let stray = Path::new("/home/u/.omp/agent/sessions/--C--x--/random_dir/file.jsonl");
        assert_eq!(subagent_parent_from_path(stray), None);
        // An underscore alone without timestamp shape must not identify a session directory.
        let not_ts = Path::new("/home/u/.omp/agent/sessions/--C--x--/Foo_12345678/file.jsonl");
        assert_eq!(subagent_parent_from_path(not_ts), None);
    }

    #[test]
    fn entry_key_uses_omp_namespace() {
        let entry = serde_json::json!({
            "type": "message",
            "id": "a1b2c3d4",
            "parentId": null,
            "timestamp": "2026-01-05T15:00:02.000Z"
        });
        assert_eq!(
            entry_key("message", &entry),
            "omp:message:a1b2c3d4:-:2026-01-05T15:00:02.000Z"
        );
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // Older contexts without version_basis deserialize successfully to None.
        // The V30 directory move does not recreate sources or reset cursors.
        let legacy = serde_json::json!({"session_id": null, "header_version": null});
        let ctx: OmpParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
    }
}
