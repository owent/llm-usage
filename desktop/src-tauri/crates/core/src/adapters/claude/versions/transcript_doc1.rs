//! Claude Code transcript JSONL 格式实现（`transcript_doc1`，V30 目录迁移自
//! 根级 claude.rs 单文件，拒绝语义不变）。
//!
//! 格式证据（官方文档，A01，文档级证据待真实样本）：
//! - 路径：`$CLAUDE_CONFIG_DIR/projects/<project>/<session>.jsonl`（默认
//!   `~/.claude/projects/...`）；子 Agent transcript 在
//!   `projects/<project>/<session>/subagents/`；被替换的旧 transcript 以
//!   `<session>.orphaned-<ts>-<suffix>.jsonl` 与 `<session>.jsonl.superseded-<ts>`
//!   变体保留（内容与现行 transcript 重叠，按稳定身份 upsert 防双计）。
//! - monitoring-usage：usage 分类 input/output/cache_read/cache_creation；
//!   `requestId` 持久化在 assistant 条目上；"An API response is persisted as one
//!   transcript entry per content block"（同一响应多条目、usage 重复，按 requestId
//!   去重）；`query_source` ∈ {main, subagent, auxiliary}（OTel 侧，未接入）。
//! - 官方明示 "transcript entry format is internal ... not a stable contract"：
//!   条目形状按 Anthropic API usage 块口径（input_tokens/output_tokens/
//!   cache_read_input_tokens/cache_creation_input_tokens）实现，标注待真实样本。
//!
//! fail closed（V17）：未文档化记录 type、或非 usage 载体记录携带 usage 字段，
//! 整文件拒绝（游标不推进、下轮确定性再拒），不猜格式。
//!
//! 版本策略：无 CLI 版本字段可读，格式锚点是文档级 transcript-doc-1，
//! 事件 `parse_basis` 固定 KnownVersion，不存在"未知版本"兼容尝试路径。

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};

use super::super::common::{map_claude_transcript, ClaudeTranscriptUsage};
use super::CLAUDE_FORMAT_VERSION;

pub const CLAUDE_PARSER_VERSION: &str = "claude-transcript-doc1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 持久化解析上下文（跨增量轮次的"每文件一次性"诊断标志 + 版本选择依据）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClaudeParseContext {
    #[serde(default)]
    unmapped_usage_keys_reported: bool,
    #[serde(default)]
    assistant_without_usage_reported: bool,
    /// 版本选择依据（known_version / latest_fallback）；旧解析上下文缺省为 None，
    /// 迁移不重建来源、不重置游标（V30）。claude 固定为 KnownVersion。
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// 从存储的游标 JSON 还原；重扫或无效时回到文件头。
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> ClaudeParseContext {
    if rescan {
        return ClaudeParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ClaudeParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

fn diag(code: &str, field: Option<&str>, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(format!("line {line}")),
        message: message.to_string(),
    }
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn parse_ts_ms(value: &serde_json::Value) -> Option<(i64, String)> {
    let raw = json_str(value, "timestamp")?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

/// 解析 assistant 条目 message.usage 四字段；全部必需、非负、有界。
/// 缺失/类型错误/负值/超限返回 None（调用方记诊断）；未知额外键另行诊断但保留记录。
fn parse_usage(value: &serde_json::Value) -> Option<ClaudeTranscriptUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(ClaudeTranscriptUsage {
        input_tokens: get("input_tokens")?,
        output_tokens: get("output_tokens")?,
        cache_read_input_tokens: get("cache_read_input_tokens")?,
        cache_creation_input_tokens: get("cache_creation_input_tokens")?,
    })
}

const USAGE_KEYS: &[&str] = &[
    "input_tokens",
    "output_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
];

fn has_unknown_usage_keys(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .map(|obj| obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str())))
        .unwrap_or(false)
}

/// 文件是否位于 `.../<session>/subagents/` 下；是则返回父会话目录名。
fn subagent_parent_session(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    if parent.file_name()?.to_str()? != "subagents" {
        return None;
    }
    Some(parent.parent()?.file_name()?.to_str()?.to_string())
}

/// 增量扫描一个 transcript JSONL 文件（统一入口 `ClaudeAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // 无版本字段可读：格式锚点是文档级 transcript-doc-1，固定 KnownVersion。
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String, &'static str)> = None;
    let parent_from_path = subagent_parent_session(&target.path);
    let outcome = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    for bad in &outcome.bad_lines {
        diagnostics.push(diag(
            bad.code,
            None,
            bad.number,
            "line is not valid UTF-8; isolated, content not stored",
        ));
    }
    for raw in &outcome.lines {
        records_seen += 1;
        let Ok(line) = serde_json::from_str::<serde_json::Value>(&raw.text) else {
            diagnostics.push(diag(
                "bad_json_line",
                None,
                raw.number,
                "line is not valid JSON; isolated, content not stored",
            ));
            continue;
        };
        let record_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match record_type {
            "assistant" => {
                let usage_value = line.get("message").and_then(|m| m.get("usage"));
                let Some(usage_value) = usage_value else {
                    // assistant 条目无 usage：无用量证据，不产事件（每文件一次性诊断）。
                    if !context.assistant_without_usage_reported {
                        context.assistant_without_usage_reported = true;
                        diagnostics.push(diag(
                            "assistant_without_usage",
                            Some("message.usage"),
                            raw.number,
                            "assistant entry without message.usage; no usage evidence, no event",
                        ));
                    }
                    continue;
                };
                let Some(usage) = parse_usage(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("message.usage"),
                        raw.number,
                        "assistant usage missing required numeric fields or out of range; record skipped",
                    ));
                    continue;
                };
                if !context.unmapped_usage_keys_reported && has_unknown_usage_keys(usage_value) {
                    context.unmapped_usage_keys_reported = true;
                    diagnostics.push(diag(
                        "unmapped_usage_keys",
                        Some("message.usage"),
                        raw.number,
                        "usage object carries keys beyond the documented four; mapped fields kept",
                    ));
                }
                let Some((occurred_ms, source_time)) = parse_ts_ms(&line) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "entry timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let session_id = json_str(&line, "sessionId").unwrap_or("unknown-session");
                let request_id = json_str(&line, "requestId");
                let message_id = line.get("message").and_then(|m| json_str(m, "id"));
                let entry_uuid = json_str(&line, "uuid");
                let (source_record_key, origin_call_id) = if let Some(rid) = request_id {
                    (format!("req:{rid}"), Some(rid.to_string()))
                } else if let Some(mid) = message_id {
                    diagnostics.push(diag(
                        "missing_request_id",
                        Some("requestId"),
                        raw.number,
                        "assistant entry without requestId; fallback identity message.id",
                    ));
                    (format!("msg:{mid}"), Some(mid.to_string()))
                } else if let Some(uuid) = entry_uuid {
                    diagnostics.push(diag(
                        "missing_request_id",
                        Some("requestId"),
                        raw.number,
                        "assistant entry without requestId/message.id; fallback identity entry uuid",
                    ));
                    (format!("uuid:{uuid}"), None)
                } else {
                    diagnostics.push(diag(
                        "missing_request_id",
                        Some("requestId"),
                        raw.number,
                        "assistant entry without requestId/message.id/uuid; fallback identity session + line",
                    ));
                    (format!("seq:{session_id}:{}", raw.number), None)
                };
                let is_sidechain = line
                    .get("isSidechain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let model = line
                    .get("message")
                    .and_then(|m| json_str(m, "model"))
                    .map(str::to_string);
                let mapped = map_claude_transcript(&usage);
                for contradiction in &mapped.diagnostics {
                    diagnostics.push(diag(
                        contradiction.code,
                        Some(contradiction.field),
                        raw.number,
                        &contradiction.detail,
                    ));
                }
                events.push(EventInput {
                    source_instance_id: target.instance_id.clone(),
                    source_record_key,
                    record_kind: RecordKind::ModelCall,
                    schema_version: CLAUDE_FORMAT_VERSION.to_string(),
                    parser_version: CLAUDE_PARSER_VERSION.to_string(),
                    parse_basis: Some(VersionBasis::KnownVersion),
                    origin_call_id,
                    attempt_id: None,
                    session_id: Some(session_id.to_string()),
                    parent_session_id: parent_from_path.clone(),
                    host_application: None,
                    agent: "claude-code".to_string(),
                    call_category: if is_sidechain || parent_from_path.is_some() {
                        CallCategory::SubAgent
                    } else {
                        CallCategory::Primary
                    },
                    occurred_at_ms: occurred_ms,
                    observed_at_ms: Some(now_ms),
                    source_time: Some(source_time),
                    time_basis: TimeBasis::SourceCompletion,
                    interval_start_ms: None,
                    interval_end_ms: None,
                    provider_id: Some("anthropic".to_string()),
                    model_raw: model.clone(),
                    model_canonical: None,
                    model_attribution: if model.is_some() {
                        ModelAttribution::RequestField
                    } else {
                        ModelAttribution::Unknown
                    },
                    usage: mapped.usage,
                    quality: mapped.quality,
                    lifecycle: Lifecycle::Final,
                    source_revision: None,
                    error_status: None,
                    duration_ms: None,
                    ttft_ms: None,
                    attribution_status: AttributionStatus::Verified,
                    exclusion_reason: None,
                    cost: None,
                });
            }
            "user" | "system" => {
                // 非 usage 载体记录携带 usage 字段：格式偏离，整文件 fail closed。
                let carries_usage = line.get("usage").is_some()
                    || line.get("message").and_then(|m| m.get("usage")).is_some();
                if carries_usage {
                    fail_closed = Some((
                        raw.number,
                        format!("record type {record_type:?} carries a usage field"),
                        "usage_on_unexpected_record_type",
                    ));
                    break;
                }
            }
            other => {
                fail_closed = Some((
                    raw.number,
                    format!("record type {other:?} not in documented set"),
                    "undocumented_record_type",
                ));
                break;
            }
        }
    }
    let mut status = match &outcome.stop {
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
    let mut health_degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line" | "usage_shape_deviation" | "line_too_long"
            )
        });
    // fail closed：本轮事件清空、游标不推进（不提交 checkpoint），下轮确定性再拒。
    let (cursor_out, context_out) = if let Some((line_no, detail, code)) = fail_closed {
        diagnostics.push(diag(code, Some("type"), line_no, &detail));
        events.clear();
        status = ScanStatus::Pending;
        health_degraded = true;
        (None, None)
    } else {
        (
            Some(serde_json::to_value(new_cursor)?),
            Some(serde_json::to_value(&context)?),
        )
    };
    Ok(ScanOutcome {
        status,
        cursor: cursor_out,
        parse_context: context_out,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: outcome.lines.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
        health: if health_degraded {
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
    fn parse_usage_requires_four_fields() {
        let v = serde_json::json!({
            "input_tokens": 10,
            "output_tokens": 2,
            "cache_read_input_tokens": 4,
            "cache_creation_input_tokens": 3
        });
        let usage = parse_usage(&v).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.cache_creation_input_tokens, 3);
        assert!(parse_usage(&serde_json::json!({"input_tokens": 10})).is_none());
        assert!(parse_usage(&serde_json::json!({
            "input_tokens": -1,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        }))
        .is_none());
    }

    #[test]
    fn unknown_usage_keys_detected() {
        let v = serde_json::json!({
            "input_tokens": 1,
            "output_tokens": 1,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "service_tier": "standard"
        });
        assert!(has_unknown_usage_keys(&v));
        assert!(parse_usage(&v).is_some(), "mapped fields kept");
        let clean = serde_json::json!({
            "input_tokens": 1,
            "output_tokens": 1,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        });
        assert!(!has_unknown_usage_keys(&clean));
    }

    #[test]
    fn subagent_parent_from_path() {
        let p = Path::new("/home/u/.claude/projects/proj/sess-1/subagents/agent-a.jsonl");
        assert_eq!(subagent_parent_session(p), Some("sess-1".to_string()));
        let main = Path::new("/home/u/.claude/projects/proj/sess-1.jsonl");
        assert_eq!(subagent_parent_session(main), None);
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // 旧解析上下文（无 version_basis 字段）反序列化不失败，basis 为 None；
        // 目录迁移不重建来源/重置游标（V30）。
        let legacy = serde_json::json!({
            "unmapped_usage_keys_reported": false,
            "assistant_without_usage_reported": false
        });
        let ctx: ClaudeParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
    }
}
