//! pi（pi-coding-agent）适配器：session JSONL（version 3）。
//!
//! 格式证据（固定源码 pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e，本阶段联网只读核对；
//! 本机 0.87.1 sessions 为空，真实核对状态 no_data，fixture 全部合成并标注）：
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
//!   （含被压缩/放弃分支，因为调用均已计费）——适配器同口径。
//! - `packages/coding-agent/src/config.ts`：目录由 `PI_CODING_AGENT_DIR`（agent 根）/
//!   `PI_CODING_AGENT_SESSION_DIR`（sessions 直指定）解析，默认 `~/.pi/agent/sessions`。
//!
//! 未知 session version fail closed（V17），不猜格式。

use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::framework::{
    Availability, CapabilityTable, DetectOutcome, DiscoverContext, DiscoveredRoot, RootBasis,
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, SourceAdapter, StoredScanState,
};
use super::jsonl::{read_jsonl, JsonlCursor, StopReason};
use super::usage_map::{map_pi_family, PiFamilyUsage};

pub const PI_FORMAT: &str = "pi-session-jsonl";
pub const PI_PARSER_VERSION: &str = "pi-session-1";
/// 固定源码 CURRENT_SESSION_VERSION=3；v1/v2 落盘格式不同（无 id/parentId），逐版本 fixture 前 fail closed。
pub const SUPPORTED_SESSION_VERSION: i64 = 3;
pub const PI_ENV_AGENT_DIR: &str = "PI_CODING_AGENT_DIR";
pub const PI_ENV_SESSION_DIR: &str = "PI_CODING_AGENT_SESSION_DIR";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// pi 适配器（无状态）。
pub struct PiAdapter;

impl Default for PiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PiAdapter {
    pub fn new() -> Self {
        PiAdapter
    }
}

/// 持久化解析上下文（会话身份、模型状态、未知类型登记）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PiParseContext {
    session_id: Option<String>,
    parent_session: Option<String>,
    header_version: Option<i64>,
    model: Option<String>,
    model_provider: Option<String>,
    #[serde(default)]
    unknown_types: Vec<String>,
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
/// 其余载体与 pi 一律 None。
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_pi_family_event(
    target: &ScanTarget,
    agent: &str,
    parser_version: &str,
    session_id: Option<&str>,
    parent_session_id: Option<&str>,
    header_version: Option<i64>,
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
            super::usage_map::MappedUsage {
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

impl PiAdapter {
    /// pi 会话事件构造：无 duration/ttft 字段（pi types.ts 未定义），共享实现见上。
    #[allow(clippy::too_many_arguments)]
    fn build_usage_event(
        &self,
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
            self.agent(),
            PI_PARSER_VERSION,
            context.session_id.as_deref(),
            context.parent_session.as_deref(),
            context.header_version,
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
}

impl SourceAdapter for PiAdapter {
    fn adapter_id(&self) -> &'static str {
        "pi"
    }

    fn agent(&self) -> &'static str {
        "pi"
    }

    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        // (sessions 目录, basis)；root 统一取 sessions 目录，跨来源去重后同目录只扫一次。
        let mut candidates: Vec<(PathBuf, RootBasis)> = Vec::new();
        if let Some(dir) = ctx.env.get(PI_ENV_AGENT_DIR) {
            candidates.push((
                PathBuf::from(dir).join("sessions"),
                RootBasis::EnvOverride(PI_ENV_AGENT_DIR.to_string()),
            ));
        }
        if let Some(dir) = ctx.env.get(PI_ENV_SESSION_DIR) {
            candidates.push((
                PathBuf::from(dir),
                RootBasis::EnvOverride(PI_ENV_SESSION_DIR.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            candidates.push((
                home.join(".pi").join("agent").join("sessions"),
                RootBasis::DefaultHome,
            ));
        }
        for manual in &ctx.manual_roots {
            // 手工根语义：含 sessions 子目录按 agent 根解析，否则按 sessions 目录本身。
            let sessions = if manual.join("sessions").is_dir() {
                manual.join("sessions")
            } else {
                manual.clone()
            };
            candidates.push((sessions, RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (sessions, basis) in candidates {
            if !sessions.is_dir() {
                continue;
            }
            // sessions/<encoded-cwd>/*.jsonl：深度 2，有界枚举；根下散落 jsonl 一并接受。
            let files = super::framework::enumerate_files_bounded(&sessions, 2, &|p| {
                p.extension().and_then(|e| e.to_str()) == Some("jsonl")
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot {
                    root: sessions,
                    basis,
                    files,
                });
            }
        }
        out
    }

    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!("pi@{}", super::framework::normalize_path(&root.root))
    }

    fn detect(&self, path: &Path) -> Result<DetectOutcome, CoreError> {
        let limits = super::jsonl::JsonlLimits {
            chunk_bytes: 64 * 1024,
            max_line_bytes: super::jsonl::DEFAULT_MAX_LINE_BYTES,
            max_lines: Some(1),
            time_budget: Some(std::time::Duration::from_secs(5)),
        };
        let outcome = read_jsonl(path, 0, 1, &limits)?;
        let Some(first) = outcome.lines.first() else {
            return Ok(DetectOutcome::Pending);
        };
        let Ok(line) = serde_json::from_str::<serde_json::Value>(&first.text) else {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "first line is not JSON".to_string(),
            });
        };
        if line.get("type").and_then(|t| t.as_str()) != Some("session") {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "first record type is not session header".to_string(),
            });
        }
        match line.get("version").and_then(|v| v.as_i64()) {
            Some(v) if v == SUPPORTED_SESSION_VERSION => Ok(DetectOutcome::Supported {
                format: PI_FORMAT.to_string(),
                format_version: v.to_string(),
            }),
            Some(v) => Ok(DetectOutcome::UnsupportedVersion {
                format: PI_FORMAT.to_string(),
                found: v.to_string(),
            }),
            None => Ok(DetectOutcome::UnsupportedVersion {
                format: PI_FORMAT.to_string(),
                found: "legacy-v1 (no version field)".to_string(),
            }),
        }
    }

    fn scan(
        &self,
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
            diagnostics.push(diag(
                bad.code,
                None,
                bad.number,
                "line is not valid UTF-8; isolated, content not stored",
            ));
        }
        for raw in &outcome.lines {
            records_seen += 1;
            let Ok(entry) = serde_json::from_str::<serde_json::Value>(&raw.text) else {
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
                    match entry.get("version").and_then(|v| v.as_i64()) {
                        Some(v) if v == SUPPORTED_SESSION_VERSION => {
                            context.header_version = Some(v);
                        }
                        _ => {
                            diagnostics.push(diag(
                                "unsupported_version",
                                Some("version"),
                                raw.number,
                                "session header version not in supported set; fail closed",
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
                    let message = entry.get("message").cloned().unwrap_or(serde_json::Value::Null);
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
                                    events.push(self.build_usage_event(
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
                                    // 无 usage 的 assistant 消息仍是一次调用的证据：
                                    // 计调用数，token 全未知（不补零）。
                                    diagnostics.push(diag(
                                        "usage_shape_deviation",
                                        Some("message.usage"),
                                        raw.number,
                                        "assistant message without complete usage; call counted, tokens unknown",
                                    ));
                                    events.push(self.build_usage_event(
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
                            events.push(self.build_usage_event(
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
                    let usage_json = entry.get("usage").cloned().unwrap_or(serde_json::Value::Null);
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
                    events.push(self.build_usage_event(
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
                    // model_change 归属（结构化变更证据），无证据 unknown。
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
                    events.push(self.build_usage_event(
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
                "thinking_level_change" | "custom" | "label" | "session_info"
                | "custom_message" | "context_edit" => {}
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

    fn capability(&self) -> CapabilityTable {
        let mut fields = serde_json::Map::new();
        let field = |availability: Availability, note: &str| {
            serde_json::json!({
                "availability": availability,
                "note": note,
            })
        };
        fields.insert(
            "tokens".into(),
            field(
                Availability::Available,
                "Usage 六字段；input/cacheRead/cacheWrite 互斥（固定源码两个 provider 实现均如此规范化），totalTokens=四桶之和；reasoning⊆output 不再加",
            ),
        );
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cacheRead reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Available,
                "cacheWrite reported；cacheWrite1h 是 cacheWrite 子集，不再加",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "assistant message 每条一次调用；独立 usage/compaction/branch_summary/toolResult usage 各计一次辅助调用",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Available,
                "assistant/provider、model 为请求自身字段；独立 usage 同；compaction/branch_summary 无模型字段，按不晚于它的 model_change 归属，无证据 unknown",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "条目 timestamp（ISO8601 UTC，追加即完成时）source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Partial(
                    "usage.cost 为 Agent 自带价目估算（非供应商账单）；价目版本不随文件记录，>0 才映射为 estimated，0 与无价目不可区分记 unknown".into(),
                ),
                "usage.cost.total（USD）",
            ),
        );
        fields.insert(
            "latency".into(),
            field(
                Availability::Unavailable("session JSONL 无逐次延迟/TTFT 字段".into()),
                "无",
            ),
        );
        CapabilityTable {
            adapter_id: "pi".to_string(),
            product: "pi（pi-coding-agent）".to_string(),
            surfaces: vec!["cli".into()],
            supported_versions: vec![SUPPORTED_SESSION_VERSION.to_string()],
            discovery: serde_json::json!({
                "default_roots": ["~/.pi/agent/sessions"],
                "env_override": [PI_ENV_AGENT_DIR, PI_ENV_SESSION_DIR],
                "manual_roots": "含 sessions 子目录按 agent 根解析，否则按 sessions 目录本身",
                "bounded": true,
                "pattern": "sessions/<encoded-cwd>/*.jsonl（散落根下 jsonl 一并接受）",
                "profile": "无 profile 概念（config.ts 未定义）",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=session",
                "version_field": "version（固定源码 CURRENT_SESSION_VERSION=3）",
                "fail_closed": true,
                "unknown_version": "v1 无 version 字段、v2 及 >3 均 unsupported_version，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "message(role=assistant).usage（final；responseId 可选作 origin_call_id）",
                "auxiliary": "独立 usage 条目（kind 如 cache_warm）、compaction.usage、branch_summary.usage、toolResult.usage 各计一次辅助调用",
                "error_aborted": "stopReason=error/aborted → error_status；无 usage 的 assistant 计调用、token 未知不补零",
                "streaming": "流式部分值不落盘（固定源码：完成条目才追加），文件内只有 final",
                "compaction": "tokensBefore 是上下文估算，不是 usage，不计账",
                "branch": "文件内全条目求和（含放弃分支，调用均已计费；与固定源码 getSessionStats 同口径）",
                "fork": "fork 逐字复制条目（id/parentId/timestamp 不变）到新文件，继承不是新调用；事件键四元组实例内幂等去重",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）", "原子整写"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            dedup: serde_json::json!({
                "primary": "pi:{message|usage|compaction|branch_summary|toolresult}:{entry id}:{parentId}:{timestamp}（实例命名空间）",
                "fork_copies": "fork 复制件四元组逐字相同，upsert 幂等（同键同内容 Keep）",
                "cross_source": "无第二本机来源；auth/models-store 非用量不读",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "hidden_calls": "cache_warm 等辅助调用有独立 usage 条目即覆盖；无条目的辅助调用（如未启用/旧版）不可见",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "no_timestamp": "条目必有 timestamp；缺失跳过并记诊断",
            }),
            maintenance: serde_json::json!({
                "parser_version": PI_PARSER_VERSION,
                "format_evidence": "固定源码 pi-mono b45597504eeaba1f11a9920a1d1048c361ed4b8e（types.ts/session-manager.ts/config.ts/provider 实现）；本机 0.87.1 sessions 为空（no_data），fixture 全部合成",
                "upgrade_policy": "新 session version 先 fail closed，取得真实 fixture 后扩展支持集",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界；不启动 Agent",
            }),
            limitations: vec![
                "本机无真实会话（0.87.1 sessions 空）：全部 fixture 合成，真实核对状态 no_data".into(),
                "responseModel 与 model 不一致时以请求字段 model 为准，差异不另记".into(),
                "usage.cost 为 Agent 估算；0 与无价目不可区分，均不映射".into(),
                "fork 继承条目归属到字典序首个被扫文件的会话（逐字相同，谁先谁留），distinct 会话数不因此虚增".into(),
                "符号链接/junction 不跟随；Windows 身份靠创建时间+首采样".into(),
                "cache_warm 之外的独立 usage kind 未见真实样本（固定源码仅 cache-warmer 一处写入）".into(),
            ],
        }
    }
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
        assert_eq!(usage.reasoning, None, "absent reasoning stays unknown, not zero");
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
