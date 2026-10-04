//! kimi wire JSONL 家族共享解析件（Kimi Code A12 / Kimi Work A13，M4）。
//!
//! 两产品是同一 wire 协议家族的不同产品身份（adapters.md「分家族复用的范围」：
//! 数据根、实例身份和日志 revision 独立，不因内核同名合并）；本模块是类比
//! jsonl.rs 的**根级家族共享件**，只放经两产品真实数据测试证明一致的辅助逻辑：
//!
//! - metadata 首行探测（protocol_version 字符串锚点，1.5/1.4 均实测为字符串）；
//! - usage.record 提取：camelCase 四字段 {inputOther, output, inputCacheRead,
//!   inputCacheCreation} 互斥、无 total（m0-agent-fixtures.md 实读结论）；
//! - epoch 毫秒时间：本机 82 个 wire.jsonl 实读全部为毫秒（1.78e12–1.79e12），
//!   无秒级样本——超出合理毫秒域的值记诊断跳过，**不做 ×1000 猜测**；
//! - `event.usage` 回声去重：`context.append_loop_event` 的 step.end 事件内
//!   `usage` 是 usage.record 的逐字段回声（M0 实读：二选一计账，防止双计）；
//!   实测 958/960（1.5）与 1329/1329（1.4）——被打断的步可能没有回声，
//!   回声只会少不会多，因此只按 usage.record 计账、回声仅作对账；
//! - subagent.completed 对账：主线 wire 的 `subagent.completed.usage` 是子代理
//!   wire 截至 completed.time 的逐次 Σ 快照（本机 2026-09-25 复证逐字段相等），
//!   子代理 wire 自身已逐次入账 ⇒ completed 绝不产生事件（计入即双计）；
//! - usageScope 语义（实读）：`turn` = 主循环 LLM 调用；`session` = 会话级
//!   辅助调用（本机全部出现在 full_compaction.begin…complete 区间 = 压缩摘要
//!   调用，llm.request kind=compaction 对应）⇒ 按辅助调用入账；
//! - 逐次身份：usage.record **无 uuid/messageId**（稳定 ID 只存在于回声侧：
//!   step.end 的 `uuid`/`messageId` 字段，与记录侧无关联键）⇒ 事件键采用
//!   `{session 目录}:{agent}:{time}:{同毫秒序号}`——本机 Kimi Work 实测存在
//!   跨文件同毫秒的 usage.record（swarm 并行子代理与主线同毫秒完成 2 对），
//!   键必须含身份段；同文件同毫秒重复时追加序号并记诊断，确定性且重扫稳定。
//!
//! 差异（分别用各自 fixture 核验，见 tests/fixtures/kimi-code|kimi-work）：
//! - Kimi Code 1.5：usage.record 带 `agentId`；目录 `sessions/<wd>/session_<uuid>/`；
//!   model 为 `alias/model` 组合串。本机 desktop 1.0.3。
//! - Kimi Work 1.4：usage.record **无** agentId（身份来自 agents/<id>/ 目录）；
//!   目录 `sessions/<wd>/<conv-*|ctitle-*>/`；model 为裸 id；宿主 daimon
//!   （state.json createdBy=daimon-kernel-adapter）。

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// 合理毫秒下界（domain）：早于此的 time 视为秒级/异常，跳过不猜测换算。
const MIN_TIME_MS: i64 = crate::domain::MIN_PLAUSIBLE_MS;
/// 合理毫秒上界（2286 年）：防御未来异常值；实读上限 1.79e12。
const MAX_TIME_MS: i64 = 10_000_000_000_000;

/// kimi wire usage.record 原始四字段：互斥、无 total（M0 实读结论）。
/// 自 usage_map.rs 随家族模块下沉（M4；Kimi Code / Kimi Work 共用）。
#[derive(Debug, Clone, Copy)]
pub struct KimiWireUsage {
    pub input_other: i64,
    pub input_cache_read: i64,
    pub input_cache_creation: i64,
    pub output: i64,
}

/// kimi wire 四互斥字段 → 规范化 usage：input_total/total_tokens 派生求和，
/// 无 source total；reasoning 无字段保持 unknown。
pub fn map_kimi_wire(raw: &KimiWireUsage) -> MappedUsage {
    let input_total = raw
        .input_other
        .checked_add(raw.input_cache_read)
        .and_then(|v| v.checked_add(raw.input_cache_creation));
    let total = input_total.and_then(|i| i.checked_add(raw.output));
    let usage = TokenUsage {
        input_uncached: Some(raw.input_other),
        input_cache_read: Some(raw.input_cache_read),
        input_cache_write: Some(raw.input_cache_creation),
        input_total,
        output_total: Some(raw.output),
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: Q::Derived,
        output_total: Q::Reported,
        output_reasoning: Q::Unknown,
        total_tokens: Q::Derived,
        source_total: Q::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// 扫描时传入的产品身份（kimi_wire 不持有产品状态，身份由各产品目录注入）。
pub(crate) struct WireProduct {
    /// 事件键命名空间（= adapter_id：kimi-code / kimi-work）。
    pub ns: &'static str,
    /// 统计归属 Agent 名（kimi-code / kimi-work）。
    pub agent: &'static str,
    /// 本版本实现的解析器版本串。
    pub parser_version: &'static str,
}

/// 实读已观测、明确不产事件的记录类型（静默忽略；清单外类型一次性诊断）。
/// 覆盖本机 82 个 wire.jsonl 的全部观测类型（1.5 + 1.4 并集）。
pub(crate) const KNOWN_IGNORED_TYPES: &[&str] = &[
    // 生命周期/控制
    "agent.message.appended",
    "agent.switched",
    "agent.turn.started",
    "agent.turn.ended",
    "config.update",
    "task.started",
    "task.terminated",
    "task.waitDelivered",
    "turn.prompt",
    "turn.ended",
    "turn.cancel",
    "turn.steer",
    "turn.step.interrupted",
    "turn.step.retrying",
    "prompt.aborted",
    "prompt.completed",
    "prompt.steered",
    "plugin.session_start",
    "runtime.set_binding",
    "profile.bind",
    "permission.set_mode",
    "permission.record_approval_result",
    "swarm_mode.enter",
    "swarm_mode.exit",
    // 子代理生命周期（completed 单独处理：快照不产事件）
    "subagent.spawned",
    "subagent.started",
    "subagent.failed",
    "subagent.cancelled",
    // 上下文/压缩控制
    "context.append_message",
    "context.apply_compaction",
    "context.undo",
    "context.undone",
    "full_compaction.begin",
    "full_compaction.complete",
    "micro_compaction.apply",
    // 请求/工具快照（llm.request 与 usage.record 无稳定关联键，不并账）
    "llm.request",
    "llm.tools_snapshot",
    "mcp.tools_discovered",
    "tools.update_store",
    "tools.set_active_tools",
    "tools.register_user_tool",
    // token_counting 是上下文估算/累计表（非逐次 usage），计入会造成计算错误
    "token_counting.measured",
    "token_counting.rebased",
    "token_counting.truncated",
    "token_counting.turn_recorded",
    // 文件历史/杂项
    "file_history.tracked",
    "file_history.checkpoint",
];

/// metadata 首行探测结果（家族共享；detect 与 scan 共用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataHead {
    pub protocol_version: Option<String>,
}

/// 首行探测结论：Pending（无完整行）/ Metadata（身份确认）/ NotMetadata
/// （fail closed 原因）。IO 错误走 `Err`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadProbe {
    Pending,
    Metadata(MetadataHead),
    NotMetadata(String),
}

/// 有界读取首行并按家族指纹识别 metadata 头。
pub(crate) fn read_metadata_head(path: &std::path::Path) -> Result<HeadProbe, CoreError> {
    let limits = super::jsonl::JsonlLimits {
        chunk_bytes: 64 * 1024,
        max_line_bytes: super::jsonl::DEFAULT_MAX_LINE_BYTES,
        max_lines: Some(1),
        time_budget: Some(std::time::Duration::from_secs(5)),
    };
    let outcome = read_jsonl(path, 0, 1, &limits)?;
    let Some(first) = outcome.lines.first() else {
        return Ok(HeadProbe::Pending);
    };
    let Ok(line) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&first.text)
    else {
        return Ok(HeadProbe::NotMetadata("first line is not JSON".to_string()));
    };
    if line.get("type").and_then(|t| t.as_str()) != Some("metadata") {
        return Ok(HeadProbe::NotMetadata(
            "first record type is not metadata header".to_string(),
        ));
    }
    Ok(HeadProbe::Metadata(MetadataHead {
        // 实读 1.4/1.5 均为字符串；非字符串形态按 None 处理（走注册表回退语义）。
        protocol_version: line
            .get("protocol_version")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }))
}

/// 持久化解析上下文（跨增量轮次的计数、对账累计与一次性诊断标志）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WireParseContext {
    pub protocol_version: Option<String>,
    pub version_basis: Option<VersionBasis>,
    /// 事件键序号状态：同毫秒冲突时递增（本机实读未观测到冲突）。
    usage_records_seen: u64,
    last_usage_time: Option<i64>,
    dup_in_last_time: u64,
    /// 对账累计（i64 饱和；回声侧只用于对账不用于计账）。
    /// 记录侧只累计 **turn scope**：session scope（压缩摘要）无回声
    /// （实读 1.4：1329 回声 == 1329 turn 记录；session 8 条全无回声）。
    record_turn_total_sum: i64,
    echo_total_sum: i64,
    subagent_completed: u64,
    #[serde(default)]
    unknown_types: Vec<String>,
    #[serde(default)]
    missing_session_id_reported: bool,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> WireParseContext {
    if rescan {
        return WireParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<WireParseContext>(v.clone()).ok())
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

/// epoch 毫秒合理性（实读全为毫秒；越界记诊断，不换算秒值）。
fn plausible_time_ms(value: i64) -> bool {
    (MIN_TIME_MS..=MAX_TIME_MS).contains(&value)
}

/// 解析 usage 四互斥字段；缺失/类型错误/负值/超限返回 None（调用方记诊断跳过）。
pub(crate) fn parse_wire_usage(value: &serde_json::Value) -> Option<KimiWireUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(KimiWireUsage {
        input_other: get("inputOther")?,
        input_cache_read: get("inputCacheRead")?,
        input_cache_creation: get("inputCacheCreation")?,
        output: get("output")?,
    })
}

fn wire_total(raw: &KimiWireUsage) -> i64 {
    raw.input_other
        .saturating_add(raw.input_cache_read)
        .saturating_add(raw.input_cache_creation)
        .saturating_add(raw.output)
}

/// 由文件路径推导会话/代理身份：`…/<session>/agents/<agent>/wire.jsonl`
/// （agent = 父目录名；session = 再上一层的 agents 目录的父目录名）。
/// Kimi Work 1.4 的 usage.record 无 agentId 字段，身份只能来自目录。
pub(crate) fn identity_from_path(path: &std::path::Path) -> (Option<String>, Option<String>) {
    let agent = path
        .ancestors()
        .nth(1)
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string);
    let session = path
        .ancestors()
        .nth(3)
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string);
    (session, agent)
}

/// 构造一条 usage.record 事件（家族共享；身份与计算规则的依据见模块头）。
#[allow(clippy::too_many_arguments)]
fn build_usage_event(
    target: &ScanTarget,
    product: &WireProduct,
    context: &WireParseContext,
    basis: Option<VersionBasis>,
    mapped: MappedUsage,
    model: Option<&str>,
    category: CallCategory,
    session_id: Option<&str>,
    agent_id: Option<&str>,
    occurred_ms: i64,
    now_ms: i64,
) -> EventInput {
    EventInput {
        source_instance_id: target.instance_id.clone(),
        // 稳定身份：ns + 会话目录 + 代理 + 记录侧 time + 同毫秒序号
        // （usage.record 无 uuid/messageId）。session/agent 段必需：本机 Kimi Work
        // 实测存在**跨文件同毫秒**的 usage.record（swarm 并行子代理与主线同毫秒
        // 完成 2 对），仅靠 time+序号会跨文件撞键（conflict 丢事件）。
        source_record_key: format!(
            "{}:usage:{}:{}:{}:{}",
            product.ns,
            session_id.unwrap_or("nosession"),
            agent_id.unwrap_or("noagent"),
            occurred_ms,
            context.dup_in_last_time
        ),
        record_kind: RecordKind::ModelCall,
        schema_version: context
            .protocol_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: product.parser_version.to_string(),
        parse_basis: basis,
        origin_call_id: None,
        attempt_id: None,
        session_id: session_id.map(str::to_string),
        parent_session_id: None,
        host_application: None,
        agent: product.agent.to_string(),
        call_category: category,
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time: Some(occurred_ms.to_string()),
        time_basis: TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: model.map(str::to_string),
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
    }
}

/// 家族共享增量扫描（kimi-code / kimi-work 的版本实现统一委托到这里）。
pub(crate) fn scan_wire(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
    product: &WireProduct,
    select_version: &dyn Fn(Option<&str>) -> VersionBasis,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let (path_session, path_agent) = identity_from_path(&target.path);

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
        match record_type {
            "metadata" => {
                if context.protocol_version.is_some() {
                    diagnostics.push(diag(
                        "unexpected_metadata_header",
                        Some("type"),
                        raw.number,
                        "second metadata header in one file; first kept",
                    ));
                    continue;
                }
                context.protocol_version = line
                    .get("protocol_version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                // 版本分派（探测/扫描同一注册表，由产品目录注入）。
                context.version_basis = Some(select_version(context.protocol_version.as_deref()));
            }
            "usage.record" => {
                let scope = line
                    .get("usageScope")
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                if !matches!(scope, "turn" | "session") {
                    diagnostics.push(diag(
                        "usage_scope_unknown",
                        Some("usageScope"),
                        raw.number,
                        "usage.record scope outside {turn, session}; skipped (no guess)",
                    ));
                    continue;
                }
                let Some(usage_value) = line.get("usage") else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage.record without usage object; skipped",
                    ));
                    continue;
                };
                let Some(usage) = parse_wire_usage(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage fields missing, negative or out of range; record skipped",
                    ));
                    continue;
                };
                let time = line.get("time").and_then(|t| t.as_i64());
                let Some(time) = time.filter(|t| plausible_time_ms(*t)) else {
                    diagnostics.push(diag(
                        "timestamp_unparseable",
                        Some("time"),
                        raw.number,
                        "time outside plausible epoch-ms range; skipped without unit guessing",
                    ));
                    continue;
                };
                // 同毫秒序号（重扫稳定的确定性身份）。
                if context.last_usage_time == Some(time) {
                    context.dup_in_last_time += 1;
                    if context.dup_in_last_time == 1 {
                        diagnostics.push(diag(
                            "usage_time_collision",
                            Some("time"),
                            raw.number,
                            "two usage.records share one millisecond; sequence suffix appended",
                        ));
                    }
                } else {
                    context.last_usage_time = Some(time);
                    context.dup_in_last_time = 0;
                }
                context.usage_records_seen += 1;
                if scope == "turn" {
                    context.record_turn_total_sum = context
                        .record_turn_total_sum
                        .saturating_add(wire_total(&usage));
                }

                let mapped = map_kimi_wire(&usage);
                for contradiction in &mapped.diagnostics {
                    diagnostics.push(diag(
                        contradiction.code,
                        Some(contradiction.field),
                        raw.number,
                        &contradiction.detail,
                    ));
                }
                // 分类：session scope = 会话级辅助调用（压缩摘要，实读证据）；
                // turn scope 按代理身份：非 main 目录 = sub_agent。
                let record_agent = line.get("agentId").and_then(|a| a.as_str());
                let effective_agent = record_agent.or(path_agent.as_deref());
                let category = if scope == "session" {
                    CallCategory::Auxiliary
                } else if effective_agent.is_some_and(|a| a != "main") {
                    CallCategory::SubAgent
                } else {
                    CallCategory::Primary
                };
                let model = line.get("model").and_then(|m| m.as_str());
                let session_ref = path_session.as_deref();
                if session_ref.is_none() && !context.missing_session_id_reported {
                    context.missing_session_id_reported = true;
                    diagnostics.push(diag(
                        "missing_session_identity",
                        Some("path"),
                        raw.number,
                        "wire path outside sessions/<wd>/<session>/agents/<agent>; no session identity",
                    ));
                }
                events.push(build_usage_event(
                    target,
                    product,
                    &context,
                    context.version_basis,
                    mapped,
                    model,
                    category,
                    session_ref,
                    effective_agent,
                    time,
                    now_ms,
                ));
            }
            "context.append_loop_event" => {
                // step.end 的 event.usage 是 usage.record 的回声（防双计二选一）：
                // 只累计对账，不产事件。
                let event = line.get("event");
                if event.and_then(|e| e.get("type")).and_then(|t| t.as_str()) == Some("step.end") {
                    if let Some(echo) = event
                        .and_then(|e| e.get("usage"))
                        .and_then(parse_wire_usage)
                    {
                        context.echo_total_sum =
                            context.echo_total_sum.saturating_add(wire_total(&echo));
                    }
                }
            }
            "subagent.completed" => {
                // 主线快照：等于子代理 wire 截至 completed.time 的 Σ（M0 + 本机复证）。
                // 子代理 wire 已逐次入账 ⇒ 不产事件；仅计数供对账说明。
                context.subagent_completed += 1;
            }
            other if KNOWN_IGNORED_TYPES.contains(&other) => {}
            "(no-type)" => {
                diagnostics.push(diag(
                    "missing_record_type",
                    Some("type"),
                    raw.number,
                    "record without type field; ignored",
                ));
            }
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
    // 回声对账只在读到文件尾时进行（文件仍在增长时半程对账无意义）。
    let mut reconciliations = Vec::new();
    if status == ScanStatus::Complete && context.usage_records_seen > 0 {
        // 记录侧 = turn scope Σ；回声 ⊆ turn 记录（打断步无回声，实读 958/960）：
        // 差值>0 是信息性子集关系；回声超过记录侧才是异常（格式变化信号）。
        let echo_exceeds = context.echo_total_sum > context.record_turn_total_sum;
        let difference = context.record_turn_total_sum - context.echo_total_sum;
        let verdict = if echo_exceeds {
            "mismatch"
        } else if difference == 0 {
            "matched"
        } else {
            "echo_subset"
        };
        if echo_exceeds {
            diagnostics.push(diag(
                "echo_exceeds_records",
                Some("event.usage"),
                new_cursor.line_number.saturating_sub(1),
                "step.end usage echo sum exceeds usage.record sum; format change suspected",
            ));
        }
        reconciliations.push(Reconciliation {
            series: "kimi_wire_step_end_echo".to_string(),
            detail_sum: context.record_turn_total_sum,
            snapshot_final: Some(context.echo_total_sum),
            carried_sum: 0,
            difference: Some(difference),
            verdict: verdict.to_string(),
        });
    }
    // 主线 subagent.completed 快照：明细在子代理自己的 wire（另一文件/实例），
    // 本文件无对账明细侧——如实报告计数与 no_detail_in_file，不伪造比较。
    if status == ScanStatus::Complete && context.subagent_completed > 0 {
        reconciliations.push(Reconciliation {
            series: format!(
                "kimi_subagent_completed_snapshot:count={}",
                context.subagent_completed
            ),
            detail_sum: 0,
            snapshot_final: None,
            carried_sum: 0,
            difference: None,
            verdict: "no_detail_in_file".to_string(),
        });
    }
    // 无 usage 记录的文件是正常形态（如仅 error 步的 wire）：不降级、不补零。
    let health_degraded = !outcome.bad_lines.is_empty()
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
        reconciliations,
        health: if health_degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}
