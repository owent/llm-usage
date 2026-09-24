//! Codex 适配器：本机 0.155.0-alpha.16.3 rollout JSONL。
//!
//! 格式证据（build/desktop-usage-validation M0 fixtures，本机实读）：
//! - `token_usage_record`：逐次 model_call 证据；`payload.usage` 六字段
//!   （input/cached/cache_write/output/reasoning/total），cached⊆input、reasoning⊆output、
//!   total=input+output（319/319 成立）；response_id 为稳定身份。
//! - `event_msg/token_count`：`info.total_token_usage` 是累计快照，只能取最终值或对
//!   逐次记录求和，不能把多条快照相加；compaction 处携带记录被排除出快照
//!   （实读核对：Σ逐次 == 最终快照 + Σ compacted 携带记录）。
//! - `event_msg/token_count` 的 `info.last_token_usage` 是逐次回声，忽略防双计。
//! - `compacted`：`payload.latest_token_usage_record` 是被压缩排除的边界记录副本，
//!   正常与逐次流中记录同 response_id（去重），不计入快照。
//! - usage 无 model 字段：按不晚于调用的 `turn_context` 位置归属；无证据则 unknown。
//! - `session_meta`：版本探测（payload.cli_version）；parent_thread_id 存在 ⇒ 子 Agent 会话。
//!
//! 未知 cli_version fail closed（V17），不猜格式。

use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind, TimeBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::framework::{
    Availability, CapabilityTable, DetectOutcome, DiscoverContext, DiscoveredRoot, Reconciliation,
    RootBasis, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, SourceAdapter, StoredScanState,
};
use super::jsonl::{read_jsonl, JsonlCursor, StopReason};
use super::usage_map::{map_codex_record, CodexRecordUsage};

pub const CODEX_FORMAT: &str = "codex-rollout-jsonl";
pub const CODEX_PARSER_VERSION: &str = "codex-rollout-1";
/// 仅有本机实读 fixture 证据的版本；其他版本 fail closed。
pub const SUPPORTED_CLI_VERSIONS: &[&str] = &["0.155.0-alpha.16.3"];
pub const CODEX_ENV_HOME: &str = "CODEX_HOME";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Codex 适配器（无状态）。
pub struct CodexAdapter;

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexAdapter {
    pub fn new() -> Self {
        CodexAdapter
    }
}

/// usage 六字段合计（累计/携带/快照对账用；i128 防溢出）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct UsageSums {
    input: i128,
    cached: i128,
    write: i128,
    output: i128,
    reasoning: i128,
    total: i128,
    calls: u64,
}

impl UsageSums {
    fn add(&mut self, usage: &CodexRecordUsage) {
        self.input += i128::from(usage.input_tokens);
        self.cached += i128::from(usage.cached_input_tokens);
        self.write += i128::from(usage.cache_write_input_tokens);
        self.output += i128::from(usage.output_tokens);
        self.reasoning += i128::from(usage.reasoning_output_tokens);
        self.total += i128::from(usage.total_tokens);
        self.calls += 1;
    }
}

/// 最终快照状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct SnapshotState {
    usage_input: i64,
    usage_cached: i64,
    usage_write: i64,
    usage_output: i64,
    usage_reasoning: i64,
    usage_total: i64,
    line: u64,
    ts_ms: i64,
}

/// 持久化解析上下文（模型状态、累计基线、对账合计）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CodexParseContext {
    model: Option<String>,
    cli_version: Option<String>,
    thread_id: Option<String>,
    parent_thread: Option<String>,
    originator: Option<String>,
    model_provider: Option<String>,
    category: Option<String>,
    session_started_ms: Option<i64>,
    series_last_value: Option<i64>,
    series_last_observed_ms: Option<i64>,
    compacted_since_snapshot: bool,
    sum_per_call: UsageSums,
    sum_carried: UsageSums,
    final_snapshot: Option<SnapshotState>,
    turns_started: u64,
    turns_completed: u64,
    turns_aborted: u64,
    #[serde(default)]
    unknown_types: Vec<String>,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> CodexParseContext {
    if rescan {
        return CodexParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<CodexParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// 解析 usage 对象的六个必需数值字段；缺失/类型错误/负值/超限返回 None（调用方记诊断）。
fn parse_usage(value: &serde_json::Value) -> Option<CodexRecordUsage> {
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<i64> {
        let v = obj.get(key)?.as_i64()?;
        if !(0..=MAX_REASONABLE_TOKEN).contains(&v) {
            return None;
        }
        Some(v)
    };
    Some(CodexRecordUsage {
        input_tokens: get("input_tokens")?,
        cached_input_tokens: get("cached_input_tokens")?,
        cache_write_input_tokens: get("cache_write_input_tokens")?,
        output_tokens: get("output_tokens")?,
        reasoning_output_tokens: get("reasoning_output_tokens")?,
        total_tokens: get("total_tokens")?,
    })
}

fn parse_envelope_ts(line: &serde_json::Value) -> Option<(i64, String)> {
    let raw = line.get("timestamp")?.as_str()?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// originator → 宿主映射（版本化规则；未知宿主不留空猜测）。
fn map_originator(originator: Option<&str>) -> Option<String> {
    match originator {
        Some("codex_vscode") => Some("vscode".to_string()),
        _ => None,
    }
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

struct UsageRecordIds<'a> {
    response_id: Option<&'a str>,
    thread_id: Option<&'a str>,
    session_id: Option<&'a str>,
}

impl CodexAdapter {
    /// 从 token_usage_record / compacted 携带记录构造 model_call 事件。
    #[allow(clippy::too_many_arguments)]
    fn build_usage_event(
        &self,
        target: &ScanTarget,
        context: &CodexParseContext,
        ids: &UsageRecordIds<'_>,
        usage: &CodexRecordUsage,
        occurred_ms: i64,
        source_time: &str,
        line: u64,
        now_ms: i64,
        diagnostics: &mut Vec<DiagnosticInput>,
    ) -> EventInput {
        let mapped = map_codex_record(usage);
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                line,
                &contradiction.detail,
            ));
        }
        let session_key = context
            .thread_id
            .as_deref()
            .or(ids.thread_id)
            .or(ids.session_id)
            .unwrap_or("unknown-session");
        let source_record_key = match ids.response_id {
            Some(rid) => format!("resp:{rid}"),
            None => {
                diagnostics.push(diag(
                    "missing_response_id",
                    Some("response_id"),
                    line,
                    "usage record without response_id; fallback identity session UUID + line number",
                ));
                format!("seq:{session_key}:{line}")
            }
        };
        EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: context
                .cli_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            parser_version: CODEX_PARSER_VERSION.to_string(),
            origin_call_id: ids.response_id.map(str::to_string),
            attempt_id: None,
            session_id: Some(session_key.to_string()),
            parent_session_id: context.parent_thread.clone(),
            host_application: map_originator(context.originator.as_deref()),
            agent: self.agent().to_string(),
            call_category: match context.category.as_deref() {
                Some("sub_agent") => CallCategory::SubAgent,
                _ => CallCategory::Primary,
            },
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(source_time.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: context.model_provider.clone(),
            model_raw: context.model.clone(),
            model_canonical: None,
            model_attribution: if context.model.is_some() {
                ModelAttribution::ProviderMapping
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
}

impl SourceAdapter for CodexAdapter {
    fn adapter_id(&self) -> &'static str {
        "codex"
    }

    fn agent(&self) -> &'static str {
        "codex"
    }

    fn discover(&self, ctx: &DiscoverContext) -> Vec<DiscoveredRoot> {
        let mut roots: Vec<(PathBuf, RootBasis)> = Vec::new();
        if let Some(home) = ctx.env.get(CODEX_ENV_HOME) {
            roots.push((
                PathBuf::from(home),
                RootBasis::EnvOverride(CODEX_ENV_HOME.to_string()),
            ));
        }
        if let Some(home) = &ctx.home_dir {
            roots.push((home.join(".codex"), RootBasis::DefaultHome));
        }
        for manual in &ctx.manual_roots {
            roots.push((manual.clone(), RootBasis::Manual));
        }
        let mut out = Vec::new();
        for (root, basis) in roots {
            let sessions = root.join("sessions");
            if !sessions.is_dir() {
                continue;
            }
            // sessions/<YYYY>/<MM>/<DD>/rollout-*.jsonl：深度 3，有界枚举。
            let files = super::framework::enumerate_files_bounded(&sessions, 3, &|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            });
            if !files.is_empty() {
                out.push(DiscoveredRoot { root, basis, files });
            }
        }
        out
    }

    fn instance_id(&self, root: &DiscoveredRoot) -> String {
        format!("codex@{}", super::framework::normalize_path(&root.root))
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
        if line.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "first record type is not session_meta".to_string(),
            });
        }
        let payload = line
            .get("payload")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let Some(version) = payload.get("cli_version").and_then(|v| v.as_str()) else {
            return Ok(DetectOutcome::UnknownFormat {
                reason: "session_meta without cli_version".to_string(),
            });
        };
        if SUPPORTED_CLI_VERSIONS.contains(&version) {
            Ok(DetectOutcome::Supported {
                format: CODEX_FORMAT.to_string(),
                format_version: version.to_string(),
            })
        } else {
            Ok(DetectOutcome::UnsupportedVersion {
                format: CODEX_FORMAT.to_string(),
                found: version.to_string(),
            })
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
        let mut aggregates: Vec<SourceAggregateInput> = Vec::new();
        let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
        let mut reconciliations: Vec<Reconciliation> = Vec::new();
        let mut records_seen: u64 = 0;
        let mut emitted_ids: HashSet<String> = HashSet::new();
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
            let payload = line
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            match record_type {
                "session_meta" => {
                    if context.cli_version.is_some() {
                        diagnostics.push(diag(
                            "unexpected_session_meta",
                            Some("type"),
                            raw.number,
                            "second session_meta in one rollout file; ignored",
                        ));
                        continue;
                    }
                    let version = json_str(&payload, "cli_version").unwrap_or("unknown");
                    if !SUPPORTED_CLI_VERSIONS.contains(&version) {
                        diagnostics.push(diag(
                            "unsupported_version",
                            Some("cli_version"),
                            raw.number,
                            "session_meta cli_version not in supported set; fail closed",
                        ));
                        continue;
                    }
                    context.cli_version = Some(version.to_string());
                    context.thread_id = json_str(&payload, "id")
                        .or_else(|| json_str(&payload, "session_id"))
                        .map(str::to_string);
                    context.parent_thread =
                        json_str(&payload, "parent_thread_id").map(str::to_string);
                    context.originator = json_str(&payload, "originator").map(str::to_string);
                    context.model_provider =
                        json_str(&payload, "model_provider").map(str::to_string);
                    let subagent_source = payload
                        .get("source")
                        .map(|s| s.is_object() && s.get("subagent").is_some())
                        .unwrap_or(false);
                    context.category = Some(
                        if context.parent_thread.is_some() || subagent_source {
                            "sub_agent"
                        } else {
                            "primary"
                        }
                        .to_string(),
                    );
                    context.session_started_ms = json_str(&payload, "timestamp")
                        .and_then(|s| s.parse::<jiff::Timestamp>().ok())
                        .map(|t| t.as_millisecond());
                }
                "turn_context" => {
                    if let Some(model) = json_str(&payload, "model") {
                        context.model = Some(model.to_string());
                    }
                }
                "token_usage_record" => {
                    let Some(usage) = payload.get("usage").and_then(parse_usage) else {
                        diagnostics.push(diag(
                            "usage_shape_deviation",
                            Some("usage"),
                            raw.number,
                            "token_usage_record usage missing required numeric fields; record skipped",
                        ));
                        continue;
                    };
                    let Some((occurred_ms, source_time)) = parse_envelope_ts(&line) else {
                        diagnostics.push(diag(
                            "timestamp_unparseable",
                            Some("timestamp"),
                            raw.number,
                            "envelope timestamp missing or unparseable; record skipped",
                        ));
                        continue;
                    };
                    let ids = UsageRecordIds {
                        response_id: json_str(&payload, "response_id"),
                        thread_id: json_str(&payload, "thread_id"),
                        session_id: json_str(&payload, "session_id"),
                    };
                    // 事件全量发出（commit 管线按身份去重/裁决冲突）；对账合计按
                    // response_id 每轮首次出现计一次，重复 final 不破坏快照对账。
                    // 跨轮追加的重复/矛盾记录会如实触发冲突与 reconcile_mismatch 诊断。
                    let first_sighting = match ids.response_id {
                        Some(rid) => emitted_ids.insert(rid.to_string()),
                        None => true,
                    };
                    if first_sighting {
                        context.sum_per_call.add(&usage);
                    }
                    events.push(self.build_usage_event(
                        target,
                        &context,
                        &ids,
                        &usage,
                        occurred_ms,
                        &source_time,
                        raw.number,
                        now_ms,
                        &mut diagnostics,
                    ));
                }
                "compacted" => {
                    context.compacted_since_snapshot = true;
                    let carried = payload.get("latest_token_usage_record").cloned();
                    if let Some(carried) = carried {
                        match carried.get("usage").and_then(parse_usage) {
                            Some(usage) => {
                                context.sum_carried.add(&usage);
                                let rid = json_str(&carried, "response_id");
                                // 携带记录正常在逐次流中（去重）；不在流中属异常：
                                // 作为恢复事件补出并记诊断，对账差异会显形。
                                if !rid.map(|r| emitted_ids.contains(r)).unwrap_or(false) {
                                    diagnostics.push(diag(
                                        "compacted_carried_not_in_stream",
                                        Some("latest_token_usage_record"),
                                        raw.number,
                                        "compaction carried record not seen in per-call stream; recovered as model_call",
                                    ));
                                    if let Some((occurred_ms, source_time)) =
                                        parse_envelope_ts(&line)
                                    {
                                        let ids = UsageRecordIds {
                                            response_id: rid,
                                            thread_id: json_str(&carried, "thread_id"),
                                            session_id: json_str(&carried, "session_id"),
                                        };
                                        events.push(self.build_usage_event(
                                            target,
                                            &context,
                                            &ids,
                                            &usage,
                                            occurred_ms,
                                            &source_time,
                                            raw.number,
                                            now_ms,
                                            &mut diagnostics,
                                        ));
                                    }
                                }
                            }
                            None => diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("latest_token_usage_record.usage"),
                                raw.number,
                                "compacted carried usage missing required numeric fields",
                            )),
                        }
                    }
                }
                "event_msg" => {
                    let sub = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match sub {
                        "token_count" => {
                            let Some(total) = payload
                                .get("info")
                                .and_then(|i| i.get("total_token_usage"))
                                .and_then(parse_usage)
                            else {
                                diagnostics.push(diag(
                                    "usage_shape_deviation",
                                    Some("info.total_token_usage"),
                                    raw.number,
                                    "token_count snapshot missing required numeric fields; skipped",
                                ));
                                continue;
                            };
                            let Some((observed_ms, _)) = parse_envelope_ts(&line) else {
                                diagnostics.push(diag(
                                    "timestamp_unparseable",
                                    Some("timestamp"),
                                    raw.number,
                                    "envelope timestamp missing or unparseable; snapshot skipped",
                                ));
                                continue;
                            };
                            // 累计快照序列：compaction 后携带记录被排除出快照（实读核验）。
                            let previous = context.series_last_value.map(|last| {
                                crate::aggregates::CumulativeState {
                                    series_key: "thread".to_string(),
                                    last_value: last,
                                    last_observed_ms: context.series_last_observed_ms.unwrap_or(0),
                                    start_ms: context.session_started_ms,
                                }
                            });
                            let (state, cum_outcome) = crate::aggregates::observe_cumulative(
                                "thread",
                                previous.as_ref(),
                                total.total_tokens,
                                observed_ms,
                                context.compacted_since_snapshot,
                            );
                            match cum_outcome {
                                crate::aggregates::CumulativeOutcome::Regression {
                                    previous,
                                    observed,
                                } => {
                                    diagnostics.push(diag(
                                        "snapshot_regression",
                                        Some("info.total_token_usage"),
                                        raw.number,
                                        &format!("cumulative snapshot decreased {previous} -> {observed} without compaction evidence"),
                                    ));
                                }
                                crate::aggregates::CumulativeOutcome::OutOfOrder => {
                                    diagnostics.push(diag(
                                        "snapshot_out_of_order",
                                        Some("info.total_token_usage"),
                                        raw.number,
                                        "cumulative snapshot out of order; baseline kept",
                                    ));
                                }
                                crate::aggregates::CumulativeOutcome::Reset { .. }
                                | crate::aggregates::CumulativeOutcome::FirstObservation {
                                    ..
                                }
                                | crate::aggregates::CumulativeOutcome::Delta { .. } => {}
                            }
                            context.series_last_value = Some(state.last_value);
                            context.series_last_observed_ms = Some(state.last_observed_ms);
                            context.compacted_since_snapshot = false;
                            context.final_snapshot = Some(SnapshotState {
                                usage_input: total.input_tokens,
                                usage_cached: total.cached_input_tokens,
                                usage_write: total.cache_write_input_tokens,
                                usage_output: total.output_tokens,
                                usage_reasoning: total.reasoning_output_tokens,
                                usage_total: total.total_tokens,
                                line: raw.number,
                                ts_ms: observed_ms,
                            });
                        }
                        "task_started" => context.turns_started += 1,
                        "task_complete" => context.turns_completed += 1,
                        "turn_aborted" => context.turns_aborted += 1,
                        _ => {}
                    }
                }
                "response_item" | "world_state" => {}
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
        // 对账只在读到当前文件尾时进行（文件可能仍在增长）。
        if status == ScanStatus::Complete {
            let detail = context.sum_per_call.total;
            let snapshot = context.final_snapshot.map(|s| i128::from(s.usage_total));
            let carried = context.sum_carried.total;
            let (difference, verdict) = match snapshot {
                Some(snap) => {
                    let diff = detail - (snap + carried);
                    let verdict = if diff == 0 { "matched" } else { "mismatch" };
                    (Some(diff), verdict)
                }
                None => (None, "no_snapshot"),
            };
            if verdict == "mismatch" {
                diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "reconcile_mismatch".to_string(),
                    field: Some("total_tokens".to_string()),
                    position: None,
                    message: format!(
                        "per-call sum {} != final snapshot {} + compaction carried {} (diff {})",
                        detail,
                        snapshot.unwrap_or(0),
                        carried,
                        difference.unwrap_or(0)
                    ),
                });
            }
            reconciliations.push(Reconciliation {
                series: "session_cumulative_snapshot".to_string(),
                detail_sum: detail.min(i128::from(i64::MAX)) as i64,
                snapshot_final: snapshot.map(|v| v.min(i128::from(i64::MAX)) as i64),
                carried_sum: carried.min(i128::from(i64::MAX)) as i64,
                difference: difference.map(|v| v.min(i128::from(i64::MAX)) as i64),
                verdict: verdict.to_string(),
            });
            // 最终快照存为来源原生区间汇总，仅作对照，不参与求和。
            if let Some(snap) = context.final_snapshot {
                let mapped = map_codex_record(&CodexRecordUsage {
                    input_tokens: snap.usage_input,
                    cached_input_tokens: snap.usage_cached,
                    cache_write_input_tokens: snap.usage_write,
                    output_tokens: snap.usage_output,
                    reasoning_output_tokens: snap.usage_reasoning,
                    total_tokens: snap.usage_total,
                });
                aggregates.push(SourceAggregateInput {
                    instance_id: target.instance_id.clone(),
                    scope: AggregateScope::Session,
                    scope_key: format!(
                        "codex-snapshot:{}",
                        context.thread_id.as_deref().unwrap_or("unknown-session")
                    ),
                    interval_start_ms: context.session_started_ms,
                    interval_end_ms: snap.ts_ms,
                    interval_end_inclusive: false,
                    usage: mapped.usage,
                    quality: mapped.quality,
                    reported_call_count: None,
                    coverage: Coverage::Duplicate,
                    duplicate_of: None,
                    time_basis: TimeBasis::SourceCompletion,
                    source_revision: Some((target.generation << 48) | snap.line as i64),
                });
            }
        }
        let new_cursor = JsonlCursor {
            generation: target.generation,
            offset: outcome.next_offset,
            line_number: outcome.next_line_number,
        };
        let degraded = !outcome.bad_lines.is_empty()
            || diagnostics.iter().any(|d| {
                matches!(
                    d.code.as_str(),
                    "bad_json_line"
                        | "usage_shape_deviation"
                        | "line_too_long"
                        | "reconcile_mismatch"
                        | "snapshot_regression"
                )
            });
        Ok(ScanOutcome {
            status,
            cursor: Some(serde_json::to_value(new_cursor)?),
            parse_context: Some(serde_json::to_value(&context)?),
            events,
            aggregates,
            diagnostics,
            lines_read: outcome.lines.len() as u64,
            records_seen,
            reconciliations,
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
        fields.insert("tokens".into(), field(Availability::Available, "token_usage_record 逐次六字段；cached⊆input、reasoning⊆output、total=input+output（本机 319/319 实读成立）"));
        fields.insert(
            "cache_read".into(),
            field(Availability::Available, "cached_input_tokens reported"),
        );
        fields.insert(
            "cache_write".into(),
            field(
                Availability::Partial(
                    "真实样本仅覆盖 cache_write=0；cache_write⊆input 为映射假设，矛盾进诊断".into(),
                ),
                "cache_write_input_tokens reported",
            ),
        );
        fields.insert(
            "per_request_calls".into(),
            field(
                Availability::Available,
                "每条 token_usage_record 是一次模型调用；response_id 稳定身份",
            ),
        );
        fields.insert(
            "model".into(),
            field(
                Availability::Partial(
                    "usage 无 model 字段；按不晚于调用的 turn_context 位置归属，无证据 unknown"
                        .into(),
                ),
                "turn_context.model",
            ),
        );
        fields.insert(
            "time".into(),
            field(
                Availability::Available,
                "envelope ISO8601 毫秒 UTC，source_completion 口径",
            ),
        );
        fields.insert(
            "cost".into(),
            field(
                Availability::Unavailable("本地无费用字段；远端账单/账号不接入".into()),
                "无",
            ),
        );
        fields.insert("latency".into(), field(Availability::Partial("task_complete 有 turn 级 duration_ms/time_to_first_token_ms；M2-A 未映射到逐次事件（迟到信息会引发同键冲突），记为缺口".into()), "turn 级"));
        CapabilityTable {
            adapter_id: "codex".to_string(),
            product: "Codex CLI / 桌面 / IDE 宿主".to_string(),
            surfaces: vec!["cli".into(), "vscode-extension".into(), "desktop".into()],
            supported_versions: SUPPORTED_CLI_VERSIONS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            discovery: serde_json::json!({
                "default_roots": ["$CODEX_HOME", "<home>/.codex"],
                "env_override": CODEX_ENV_HOME,
                "manual_roots": true,
                "bounded": true,
                "pattern": "sessions/<YYYY>/<MM>/<DD>/rollout-*.jsonl",
                "profile": "无 profile 概念",
            }),
            detection: serde_json::json!({
                "magic": "首行 JSONL type=session_meta",
                "version_field": "payload.cli_version",
                "fail_closed": true,
                "unknown_version": "unsupported_version，不猜格式",
            }),
            fields,
            lifecycle: serde_json::json!({
                "model_call": "token_usage_record（final，response_id 身份）",
                "cumulative_snapshot": "token_count.total_token_usage 取最终值；compaction 携带记录被排除出快照；Σ逐次==最终快照+Σ携带（实读核对）",
                "last_token_usage": "逐次回声，忽略防双计",
                "aborted": "turn_aborted 已观测；已返回部分经其 token_usage_record 记账",
                "retries": "格式内未观测到 transport 重试记录",
                "subagent": "session_meta.parent_thread_id 存在 ⇒ sub_agent；子 Agent 是独立 rollout 文件",
            }),
            incremental: serde_json::json!({
                "cursor": "文件身份 + generation + 完整行字节偏移 + 解析上下文",
                "rewrite_detection": ["截断", "同长替换", "改名重探测", "重建（创建时间变化）"],
                "budget": "单源每轮 30s 初值；单行 8 MiB；单块 4 MiB",
                "half_line": "半行不前移游标",
            }),
            dedup: serde_json::json!({
                "primary": "resp:{response_id}（实例命名空间）",
                "fallback": "seq:{session UUID}:{行号}（缺 response_id，已验证替代）",
                "cross_source": "state_5.sqlite threads.tokens_used 是另一存储的线程级累计；M2-A 只读 rollout，不相加",
            }),
            integrity: serde_json::json!({
                "success_only": false,
                "aborted_turns": "usage 按已返回部分记账",
                "hidden_calls": "未知；auto-review 等子 Agent 会话是独立 rollout 文件，各自计入",
                "sampling": "未观测到采样；坏行逐条隔离记诊断",
                "source_retention": "源端保留未知；可回填范围以现存文件为准",
            }),
            maintenance: serde_json::json!({
                "parser_version": CODEX_PARSER_VERSION,
                "format_evidence": "M0 本机 fixture（0.155.0-alpha.16.3，3 会话）",
                "upgrade_policy": "新 cli_version 先 fail closed，取得 fixture 后扩展支持集",
            }),
            scheduling: serde_json::json!({
                "entry": "统一 run_adapter_scan；手动/间隔/监听触发按源合并",
                "incremental_cost": "字节偏移续读；无变化文件探测短路",
                "pause_cancel": "文件间可停；单轮预算有界",
            }),
            limitations: vec![
                "turn 级 duration/TTFT 未映射到逐次事件（迟到信息同键冲突风险）".into(),
                "cache_write>0 仅有合成样本；cache_write⊆input 为映射假设".into(),
                "附属 SQLite（state_5 等）不在 M2-A 范围；threads.tokens_used 不与 rollout 相加"
                    .into(),
                "符号链接/ junction 不跟随；Windows 无稳定文件索引号，身份靠创建时间+首采样".into(),
                "无 response_id 记录用会话 UUID+行号身份，文件同位替换后可能形成新键".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_usage_requires_six_fields() {
        let v = serde_json::json!({
            "input_tokens": 10,
            "cached_input_tokens": 4,
            "cache_write_input_tokens": 0,
            "output_tokens": 2,
            "reasoning_output_tokens": 1,
            "total_tokens": 12
        });
        assert_eq!(parse_usage(&v).unwrap().total_tokens, 12);
        let missing = serde_json::json!({"input_tokens": 10});
        assert!(parse_usage(&missing).is_none());
        let negative = serde_json::json!({
            "input_tokens": -1,
            "cached_input_tokens": 0,
            "cache_write_input_tokens": 0,
            "output_tokens": 0,
            "reasoning_output_tokens": 0,
            "total_tokens": 0
        });
        assert!(parse_usage(&negative).is_none());
    }

    #[test]
    fn originator_mapping_is_versioned() {
        assert_eq!(
            map_originator(Some("codex_vscode")),
            Some("vscode".to_string())
        );
        assert_eq!(map_originator(Some("codex_cli")), None);
        assert_eq!(map_originator(None), None);
    }
}
