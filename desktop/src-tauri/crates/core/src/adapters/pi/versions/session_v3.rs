//! pi session JSONL 格式实现（`session_v3`，version 3）。
//!
//! 格式依据（固定源码 pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e，本阶段联网只读核对；
//! 真实核对以本机 fixture 为准，合成 fixture 均标注）：
//! - `packages/ai/src/types.ts`：`Usage{input,output,cacheRead,cacheWrite,cacheWrite1h?(⊆cacheWrite),
//!   reasoning?(⊆output),totalTokens,cost}`；anthropic-messages/openai-completions 均把
//!   `input` 规范化为未缓存桶，`totalTokens = input+output+cacheRead+cacheWrite`。
//! - `packages/coding-agent/src/core/session-manager.ts`：条目基座 `{type,id,parentId,timestamp}`；
//!   首行 `type:"session"` 头（`version`=3，`id`，`parentSession?`）；usage 载体四类：
//!   ① `message` 且 role=assistant（逐次 model_call；responseId 可选）；
//!   ② 独立 `usage` 条目（`kind` 如 cache_warm；provider/model 自有字段）；
//!   ③ `compaction` 条目 `usage?`（总结调用；无模型字段，按不晚于它的 model_change 归属）；
//!   ④ `branch_summary` 条目 `usage?`（同③）。toolResult 的 `usage?` 是工具执行自身消耗，
//!   不进主上下文记账（types.ts 注释），按辅助调用映射。
//!   fork（`fork`/`forkFrom`）把源文件全部非头条目**逐字复制**进新文件（id/parentId/timestamp
//!   不变，新 header 记 `parentSession`），继承条目不是新调用：事件键用条目四元组
//!   （type+id+parentId+timestamp），复制件在实例内 upsert 幂等去重。
//! - `packages/coding-agent/src/core/agent-session.ts` `getSessionStats`：对文件内全部条目求和
//!   （含被压缩/放弃分支，因为调用均已计费）——适配器规则相同。
//! - `packages/coding-agent/src/config.ts`：目录由 `PI_CODING_AGENT_DIR`（agent 根）/
//!   `PI_CODING_AGENT_SESSION_DIR`（sessions 直指定）解析，默认 `~/.pi/agent/sessions`。
//!
//! 版本策略（architecture.md#unknown-version，V30）：session 头经
//! [`super::select`]（探测/扫描同一注册表）分派；未收录数值用本实现（当前最新）
//! 兼容尝试，事件带 `parse_basis` 标记；已确认不兼容的版本（v1/v2/缺失 version）
//! 才跳过并记诊断。
//!
//! V30 目录迁移：本实现自根级 adapters/pi.rs 整体迁入（已验收行为保持原样）；
//! pi/omp 家族共享的 usage 解析与事件构造也在此实现，由上级 mod.rs 再导出。

use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{map_pi_family, PiFamilyUsage};

pub const PI_PARSER_VERSION: &str = "pi-session-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 持久化解析上下文（会话身份、模型状态、版本选择依据、未知类型登记）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PiParseContext {
    session_id: Option<String>,
    parent_session: Option<String>,
    header_version: Option<i64>,
    model: Option<String>,
    model_provider: Option<String>,
    #[serde(default)]
    unknown_types: Vec<String>,
    /// 版本选择依据（known_version / latest_fallback）；旧解析上下文缺省为 None，
    /// V30 目录迁移不重建来源、不重置游标。
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> PiParseContext {
    if rescan {
        return PiParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<PiParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// 解析 pi/omp 家族 usage 对象；必需数值缺失/类型错误/负值/超限返回 None（调用方记诊断）。
/// `reasoning`/`reasoningTokens` 缺字段保持 None（供应商未报告 ≠ 0）。
pub(crate) fn parse_usage(value: &serde_json::Value) -> Option<PiFamilyUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    let reasoning = obj
        .get("reasoning")
        .or_else(|| obj.get("reasoningTokens"))
        .and_then(|v| v.as_i64())
        .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v));
    Some(PiFamilyUsage {
        input: get("input")?,
        output: get("output")?,
        cache_read: get("cacheRead")?,
        cache_write: get("cacheWrite")?,
        total_tokens: get("totalTokens")?,
        reasoning,
    })
}

/// usage.cost.total（USD，Agent 自带价目估算，非供应商账单）→ estimated 费用。
/// 0 与无价目不可区分，不映射（None = unknown）；非有限/负值/溢出记诊断返回 None。
pub(crate) fn map_cost(
    usage: &serde_json::Value,
    price_version: Option<String>,
    diagnostics: &mut Vec<DiagnosticInput>,
    line: u64,
) -> Option<CostAmount> {
    let total = usage.get("cost")?.get("total")?.as_f64()?;
    if !total.is_finite() || total < 0.0 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost.total"),
            line,
            "usage.cost.total not a finite non-negative number; cost left unknown",
        ));
        return None;
    }
    if total == 0.0 {
        return None;
    }
    let micros = (total * 1_000_000.0).round();
    if micros > i64::MAX as f64 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost.total"),
            line,
            "usage.cost.total overflows micro-unit i64; cost left unknown",
        ));
        return None;
    }
    Some(CostAmount {
        amount_minor: micros as i64,
        currency: "USD".to_string(),
        kind: CostKind::Estimated,
        price_version,
        billing_scope: None,
    })
}

pub(crate) fn parse_entry_ts(value: &serde_json::Value) -> Option<(i64, String)> {
    let raw = value.get("timestamp")?.as_str()?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

pub(crate) fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

pub(crate) fn diag(code: &str, field: Option<&str>, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(format!("line {line}")),
        message: message.to_string(),
    }
}

/// 条目身份：fork 逐字复制保持四元组一致（types+id+parentId+timestamp），
/// 实例内 upsert 幂等；`parentId` 为空用 "-" 占位。`ns` 为适配器命名空间（pi/omp）。
pub(crate) fn family_entry_key(ns: &str, prefix: &str, entry: &serde_json::Value) -> String {
    let id = json_str(entry, "id").unwrap_or("noid");
    let parent = json_str(entry, "parentId").unwrap_or("-");
    let ts = json_str(entry, "timestamp").unwrap_or("notime");
    format!("{ns}:{prefix}:{id}:{parent}:{ts}")
}

/// pi 命名空间下的条目键。
fn entry_key(prefix: &str, entry: &serde_json::Value) -> String {
    family_entry_key("pi", prefix, entry)
}

pub(crate) struct UsageEventBase<'a> {
    pub key: String,
    pub category: CallCategory,
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub attribution: ModelAttribution,
    pub origin_call_id: Option<&'a str>,
    pub error_status: Option<String>,
}

/// 由任意 usage 载体条目构造 pi 家族事件（pi/omp 共用）。`usage` 为 None 时
/// token 全未知（无 usage 的 assistant 消息仍计一次调用；不补零）。
/// duration/ttft 仅 omp 会话的 assistant 消息有字段（types.ts duration?/ttft?），
/// 其余载体与 pi 一律 None。`parse_basis` 为版本选择依据（V30 兼容标记）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_pi_family_event(
    target: &ScanTarget,
    agent: &str,
    parser_version: &str,
    session_id: Option<&str>,
    parent_session_id: Option<&str>,
    header_version: Option<i64>,
    parse_basis: Option<VersionBasis>,
    base: &UsageEventBase<'_>,
    duration_ms: Option<i64>,
    ttft_ms: Option<i64>,
    usage: Option<(&PiFamilyUsage, &serde_json::Value)>,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    let (mapped, cost) = match usage {
        Some((usage, usage_json)) => {
            let mapped = map_pi_family(usage);
            for contradiction in &mapped.diagnostics {
                diagnostics.push(diag(
                    contradiction.code,
                    Some(contradiction.field),
                    line,
                    &contradiction.detail,
                ));
            }
            let cost = map_cost(usage_json, None, diagnostics, line);
            (mapped, cost)
        }
        None => (
            crate::adapters::usage_map::MappedUsage {
                usage: crate::domain::TokenUsage::default(),
                quality: crate::domain::TokenQuality::default(),
                diagnostics: Vec::new(),
            },
            None,
        ),
    };
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: base.key.clone(),
        record_kind: RecordKind::ModelCall,
        schema_version: header_version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: parser_version.to_string(),
        parse_basis,
        origin_call_id: base.origin_call_id.map(str::to_string),
        attempt_id: None,
        session_id: session_id.map(str::to_string),
        parent_session_id: parent_session_id.map(str::to_string),
        host_application: None,
        agent: agent.to_string(),
        call_category: base.category,
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(source_time.to_string()),
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: base.provider.map(str::to_string),
        model_raw: base.model.map(str::to_string),
        model_canonical: None,
        model_attribution: base.attribution,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: base.error_status.clone(),
        duration_ms,
        ttft_ms,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost,
    }
}

/// pi 会话事件构造：无 duration/ttft 字段（pi types.ts 未定义），共享实现见
/// [`build_pi_family_event`]；`parse_basis` 取自解析上下文的版本选择依据。
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    context: &PiParseContext,
    base: &UsageEventBase<'_>,
    usage: Option<(&PiFamilyUsage, &serde_json::Value)>,
    occurred_ms: i64,
    source_time: &str,
    line: u64,
    now_ms: i64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> EventInput {
    build_pi_family_event(
        target,
        "pi",
        PI_PARSER_VERSION,
        context.session_id.as_deref(),
        context.parent_session.as_deref(),
        context.header_version,
        context.version_basis,
        base,
        None,
        None,
        usage,
        occurred_ms,
        source_time,
        line,
        now_ms,
        diagnostics,
    )
}

/// 增量扫描一个 pi session JSONL 文件（统一入口 `PiAdapter::scan` 分派到本实现）。
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
                // 版本分派（探测/扫描共用同一注册表，V30）：KnownVersion/LatestFallback
                // 都继续解析（未知版本数据照常入库，事件带 parse_basis 标记）；
                // 已确认不兼容（v1/v2/缺失 version）才跳过并记诊断。
                let header_version = entry.get("version").and_then(|v| v.as_i64());
                match super::select(header_version) {
                    Ok(selection) => {
                        context.version_basis = Some(selection.basis);
                        context.header_version = header_version;
                    }
                    Err(reason) => {
                        diagnostics.push(diag(
                            "unsupported_version",
                            Some("version"),
                            raw.number,
                            &format!("session header version incompatible; fail closed: {reason}"),
                        ));
                        continue;
                    }
                }
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
                if let Some(model) = json_str(&entry, "modelId") {
                    context.model = Some(model.to_string());
                }
                if let Some(provider) = json_str(&entry, "provider") {
                    context.model_provider = Some(provider.to_string());
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
                        let base = UsageEventBase {
                            key: entry_key("message", &entry),
                            category: CallCategory::Primary,
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
                                    &mut diagnostics,
                                ));
                            }
                            None => {
                                // 无 usage 的 assistant 消息仍表明发生过一次调用：
                                // 计调用数，token 全未知（不补零）。
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
                                    &mut diagnostics,
                                ));
                            }
                        }
                    }
                    "toolResult" => {
                        // 工具执行自身的 usage（不进主上下文记账）→ 辅助调用。
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
                            &mut diagnostics,
                        ));
                    }
                    _ => {}
                }
            }
            "usage" => {
                // 独立 usage 条目（kind 如 cache_warm）：辅助调用，provider/model 自有字段。
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
                    &mut diagnostics,
                ));
            }
            "compaction" | "branch_summary" => {
                // 总结/分支总结调用（usage 可选）：无模型字段，按不晚于它的
                // 按 model_change 记录模型变更，无法确认所属模型时保持 unknown。
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
                    &mut diagnostics,
                ));
            }
            "thinking_level_change"
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
    fn parse_usage_requires_five_fields_and_keeps_reasoning_absent() {
        let v = serde_json::json!({
            "input": 100,
            "output": 50,
            "cacheRead": 800,
            "cacheWrite": 100,
            "totalTokens": 1050
        });
        let usage = parse_usage(&v).unwrap();
        assert_eq!(usage.total_tokens, 1050);
        assert_eq!(
            usage.reasoning, None,
            "absent reasoning stays unknown, not zero"
        );
        let with_reasoning = serde_json::json!({
            "input": 100,
            "output": 50,
            "cacheRead": 800,
            "cacheWrite": 100,
            "totalTokens": 1050,
            "reasoning": 0
        });
        assert_eq!(parse_usage(&with_reasoning).unwrap().reasoning, Some(0));
        let missing = serde_json::json!({"input": 100, "output": 50});
        assert!(parse_usage(&missing).is_none());
        let negative = serde_json::json!({
            "input": -1,
            "output": 0,
            "cacheRead": 0,
            "cacheWrite": 0,
            "totalTokens": 0
        });
        assert!(parse_usage(&negative).is_none());
    }

    #[test]
    fn entry_key_uses_stable_entry_quadruple() {
        let entry = serde_json::json!({
            "type": "message",
            "id": "a1b2c3d4",
            "parentId": null,
            "timestamp": "2026-01-05T15:00:02.000Z"
        });
        assert_eq!(
            entry_key("message", &entry),
            "pi:message:a1b2c3d4:-:2026-01-05T15:00:02.000Z"
        );
    }

    #[test]
    fn cost_maps_only_positive_finite_values() {
        let mut diags = Vec::new();
        let usage = serde_json::json!({"cost": {"total": 0.005}});
        let cost = map_cost(&usage, None, &mut diags, 1).unwrap();
        assert_eq!(cost.amount_minor, 5000);
        assert_eq!(cost.currency, "USD");
        assert_eq!(cost.kind, CostKind::Estimated);
        let zero = serde_json::json!({"cost": {"total": 0.0}});
        assert!(map_cost(&zero, None, &mut diags, 1).is_none());
        assert!(diags.is_empty());
        let negative = serde_json::json!({"cost": {"total": -1.0}});
        assert!(map_cost(&negative, None, &mut diags, 1).is_none());
        assert_eq!(diags.len(), 1);
    }
}
