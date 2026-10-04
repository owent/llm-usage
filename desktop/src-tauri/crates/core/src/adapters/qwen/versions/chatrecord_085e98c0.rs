//! Qwen Code ChatRecord JSONL 格式实现（`chatrecord_085e98c0`，固定源码依据 A18）。
//!
//! 格式依据（qwen-code 源码固定 commit 085e98c00cac2f8dd29eb39c760409bc6da889a9）：
//! - 路径：`~/.qwen/tmp/<project_id>/chats/<sessionId>.jsonl`（append-only JSONL；
//!   无文档化环境覆盖）。
//! - `ChatRecord{uuid, parentUuid, sessionId, timestamp(ISO), type:
//!   user|assistant|tool_result|system, subtype?(33 值枚举), provenance, cwd,
//!   version(CLI 版本), gitBranch?, message?, usageMetadata?, model?,
//!   contextWindowSize?, agentId?, agentName?, isSidechain?, goalContext?, ...}`。
//! - `recordAssistantTurn` 设 `record.usageMetadata = data.tokens`（
//!   GenerateContentResponseUsageMetadata，六分类各自可选）与 `record.model`；
//!   tokens 参数本身可选 ⇒ assistant 记录无 usageMetadata 属正常形状。
//! - `isSidechain:true` / `agentId` = 子 Agent 记录（与主会话同文件，无双计）。
//! - Goal 控制记录：`goal_state` 的 `goal.tokensUsed` 是跨 turn 累计表
//!   （"totalTokenCount summed per model call"），绝不当 request/总消耗（双计）；
//!   `goal_runtime`/`goal_turn_end`/`chat_compression` 同为控制记录，明确忽略。
//!   `goal_context` 标注的 assistant 记录仍是一次真实模型调用，照常计入。
//! - `session_model` subtype 是 daemon 恢复绑定，不能归给历史调用。
//!
//! fail closed（V17）：type 超出四值、subtype 超出 33 值枚举、或非 assistant
//! 记录携带 usageMetadata，整文件拒绝（游标不推进、下轮确定性再拒）。
//!
//! 版本策略（architecture.md#adapter-layout）：格式锚点是固定源码 commit，不做
//! 版本白名单——`record.version`（CLI 版本）逐条存 schema_version，不参与分派；
//! 注册表（[`super::versions`]）为与其他 Agent 统一的结构而设。
//! 本实现自根级单文件 qwen.rs 目录化平移（M2 目录化迁移，V30），行为约定不变。

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{map_genai_usage, GenaiUsage};

pub const QWEN_PARSER_VERSION: &str = "qwen-chatrecord-085e98c0-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 固定源码 ChatRecord.type 四值。
pub const RECORD_TYPES: &[&str] = &["user", "assistant", "tool_result", "system"];

/// 固定源码 ChatRecord.subtype 33 值枚举（commit 085e98c0）。
const RECORD_SUBTYPES: &[&str] = &[
    "chat_compression",
    "slash_command",
    "ui_telemetry",
    "at_command",
    "attribution_snapshot",
    "notification",
    "background_task_completed",
    "cron",
    "mid_turn_user_message",
    "custom_title",
    "parent_session",
    "session_source",
    "omni_recall",
    "session_model",
    "rewind",
    "agent_bootstrap",
    "agent_launch_prompt",
    "agent_retry",
    "agent_session_ready",
    "file_history_snapshot",
    "user_text_elements",
    "session_artifact_event",
    "session_artifact_snapshot",
    "session_sources_snapshot",
    "branch_checkpoint",
    "goal_state",
    "goal_runtime",
    "goal_turn_end",
    "realtime_message",
    "turn_result",
    "managed_session_header_v1",
    "managed_session_event_v1",
    "managed_session_commit_v1",
];

/// 明确忽略的控制记录 subtype（Goal 累计表/运行时/轮次边界/压缩检查点）：
/// 绝不产生事件（goal.tokensUsed 是跨 turn 累计表，计入即双计）。
const IGNORED_SUBTYPES: &[&str] = &[
    "goal_state",
    "goal_runtime",
    "goal_turn_end",
    "chat_compression",
];

/// usageMetadata 六分类键（GenerateContentResponseUsageMetadata，各自可选）。
const USAGE_KEYS: &[&str] = &[
    "promptTokenCount",
    "candidatesTokenCount",
    "cachedContentTokenCount",
    "thoughtsTokenCount",
    "toolUsePromptTokenCount",
    "totalTokenCount",
];

/// 持久化解析上下文（跨增量轮次的"每文件一次性"诊断标志与版本选择依据）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct QwenParseContext {
    #[serde(default)]
    unmapped_usage_keys_reported: bool,
    /// 版本选择依据（known_version）；格式锚点固定 ⇒ 恒为 Some(KnownVersion)。
    /// 旧解析上下文缺省为 None，迁移不重建来源、不重置游标（V30）。
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> QwenParseContext {
    if rescan {
        return QwenParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<QwenParseContext>(v.clone()).ok())
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

/// 解析 usageMetadata 六可选字段；存在的值必须非负有界（违例返回 None，调用方记诊断）。
/// 未知额外键返回 true（保留已映射字段，每文件一次性诊断）。
fn parse_usage_metadata(value: &serde_json::Value) -> Option<(GenaiUsage, bool)> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<Option<i64>> {
        match obj.get(key) {
            None => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                    return None;
                }
                Some(Some(n))
            }
        }
    };
    let usage = GenaiUsage {
        prompt_tokens: get("promptTokenCount")?,
        candidates_tokens: get("candidatesTokenCount")?,
        cached_tokens: get("cachedContentTokenCount")?,
        thoughts_tokens: get("thoughtsTokenCount")?,
        tool_tokens: get("toolUsePromptTokenCount")?,
        total_tokens: get("totalTokenCount")?,
    };
    let unknown = obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str()));
    Some((usage, unknown))
}

/// 增量扫描一个 ChatRecord JSONL 文件（统一入口 `QwenAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // 格式锚点是固定源码 commit（已收录注册表）⇒ 解析依据恒为 known_version；
    // 持久化解析上下文形状与其他 Agent 一致（V30）。
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String, &'static str)> = None;
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
        let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&raw.text)
        else {
            diagnostics.push(diag(
                "bad_json_line",
                None,
                raw.number,
                "line is not valid JSON; isolated, content not stored",
            ));
            continue;
        };
        let record_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !RECORD_TYPES.contains(&record_type) {
            fail_closed = Some((
                raw.number,
                format!("record type {record_type:?} not in ChatRecord set"),
                "undocumented_record_type",
            ));
            break;
        }
        if let Some(subtype) = line.get("subtype").and_then(|s| s.as_str()) {
            if !RECORD_SUBTYPES.contains(&subtype) {
                fail_closed = Some((
                    raw.number,
                    format!("record subtype {subtype:?} not in pinned 33-value enum"),
                    "undocumented_record_subtype",
                ));
                break;
            }
            // 控制记录明确忽略（Goal 累计表等，计入即双计）。
            if IGNORED_SUBTYPES.contains(&subtype) {
                continue;
            }
        }
        match record_type {
            "assistant" => {
                let usage_value = line.get("usageMetadata");
                let Some(usage_value) = usage_value else {
                    // tokens 在固定源码中可选：无 usageMetadata 属正常形状，不产事件。
                    continue;
                };
                let Some((usage, unknown_keys)) = parse_usage_metadata(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usageMetadata"),
                        raw.number,
                        "usageMetadata value negative, non-integer or out of range; record skipped",
                    ));
                    continue;
                };
                if unknown_keys && !context.unmapped_usage_keys_reported {
                    context.unmapped_usage_keys_reported = true;
                    diagnostics.push(diag(
                        "unmapped_usage_keys",
                        Some("usageMetadata"),
                        raw.number,
                        "usageMetadata carries keys beyond the pinned six; mapped fields kept",
                    ));
                }
                let Some((occurred_ms, source_time)) =
                    json_str(&line, "timestamp").and_then(|raw_ts| {
                        raw_ts
                            .parse::<jiff::Timestamp>()
                            .ok()
                            .map(|t| (t.as_millisecond(), raw_ts.to_string()))
                    })
                else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("timestamp"),
                        raw.number,
                        "record timestamp missing or unparseable; record skipped",
                    ));
                    continue;
                };
                let session_id = json_str(&line, "sessionId").unwrap_or("unknown-session");
                let (source_record_key, origin_call_id) = match json_str(&line, "uuid") {
                    Some(uuid) => (format!("qwen:{uuid}"), Some(uuid.to_string())),
                    None => {
                        diagnostics.push(diag(
                            "missing_uuid",
                            Some("uuid"),
                            raw.number,
                            "assistant record without uuid; fallback identity session + line",
                        ));
                        (format!("seq:{session_id}:{}", raw.number), None)
                    }
                };
                let is_subagent = line
                    .get("isSidechain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || line.get("agentId").is_some();
                let model = json_str(&line, "model").map(str::to_string);
                let mapped = map_genai_usage(&usage);
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
                    schema_version: json_str(&line, "version").unwrap_or("unknown").to_string(),
                    parser_version: QWEN_PARSER_VERSION.to_string(),
                    // 格式锚点固定（固定源码 commit 注册表已收录）⇒ 恒为 known_version。
                    parse_basis: Some(VersionBasis::KnownVersion),
                    origin_call_id,
                    attempt_id: None,
                    session_id: Some(session_id.to_string()),
                    parent_session_id: None,
                    host_application: None,
                    agent: "qwen-code".to_string(),
                    call_category: if is_subagent {
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
                    provider_id: None,
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
            // 非 assistant 记录携带 usageMetadata：格式偏离，整文件 fail closed。
            _ => {
                if line.get("usageMetadata").is_some() {
                    fail_closed = Some((
                        raw.number,
                        format!("record type {record_type:?} carries usageMetadata"),
                        "usage_on_unexpected_record_type",
                    ));
                    break;
                }
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
    fn subtype_enum_matches_pinned_source() {
        assert_eq!(RECORD_SUBTYPES.len(), 33);
        for required in [
            "goal_state",
            "goal_runtime",
            "goal_turn_end",
            "chat_compression",
            "session_model",
        ] {
            assert!(RECORD_SUBTYPES.contains(&required), "missing {required}");
        }
        for ignored in IGNORED_SUBTYPES {
            assert!(RECORD_SUBTYPES.contains(ignored), "ignored not in enum");
        }
    }

    #[test]
    fn parse_usage_metadata_optional_fields() {
        let full = serde_json::json!({
            "promptTokenCount": 100,
            "candidatesTokenCount": 20,
            "cachedContentTokenCount": 30,
            "thoughtsTokenCount": 5,
            "toolUsePromptTokenCount": 7,
            "totalTokenCount": 120
        });
        let (usage, unknown) = parse_usage_metadata(&full).unwrap();
        assert_eq!(usage.prompt_tokens, Some(100));
        assert_eq!(usage.tool_tokens, Some(7));
        assert!(!unknown);

        let partial = serde_json::json!({"promptTokenCount": 10});
        let (usage, unknown) = parse_usage_metadata(&partial).unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.total_tokens, None);
        assert!(!unknown);

        let negative = serde_json::json!({"promptTokenCount": -1});
        assert!(parse_usage_metadata(&negative).is_none());

        let extra = serde_json::json!({"promptTokenCount": 1, "newField": 2});
        let (usage, unknown) = parse_usage_metadata(&extra).unwrap();
        assert_eq!(usage.prompt_tokens, Some(1));
        assert!(unknown);
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // 旧解析上下文（无 version_basis 字段）反序列化不失败，basis 为 None；
        // 目录迁移不重建来源/重置游标（V30）。
        let legacy = serde_json::json!({"unmapped_usage_keys_reported": true});
        let ctx: QwenParseContext = serde_json::from_value(legacy).expect("restore");
        assert!(ctx.unmapped_usage_keys_reported);
        assert_eq!(ctx.version_basis, None);
    }
}
