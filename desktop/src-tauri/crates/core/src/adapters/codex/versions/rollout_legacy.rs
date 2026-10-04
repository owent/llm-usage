//! Codex rollout JSONL 旧版格式实现（`rollout_legacy`，0.139–0.151 系列）。
//!
//! 格式依据（2026-09-26 本机 ~/.codex/sessions 全量 238 个 0.139–0.151 文件实读，
//! 21 个版本、13,481 条 token_count 事件逐条分桶；核验脚本输出存
//! build/codex-legacy-forensics/，gitignored）：
//! - 全部 238 个文件 **零 `token_usage_record`**（逐次载体缺失）——与 rollout_v1 的
//!   载体不同，故本实现从 `event_msg/token_count` 读取逐次用量。
//! - `token_count.info.total_token_usage`：累计快照，六字段同形；`last_token_usage`：
//!   最近一次调用的回声，六字段同形（input/cached/cache_write/output/reasoning/total）。
//!   envelope timestamp 为 ISO8601 毫秒 UTC（13,481/13,481）。
//! - **last 语义判据（total 增量法，逐条核对）**：
//!   * delta = 本次 total − 上次 total；delta > 0 ⇒ 覆盖 ≥1 次新调用，last 是其中
//!     最新一次（13,032 条 delta>0 全部伴随 last 变化；13,024 条 delta == last.total
//!     = 单次调用；8 条 delta > last.total = 区间内多次调用、仅最新一次在 last 中，
//!     例 0.139.0 rollout-…-019ec029….jsonl L1043–L1051：四条 function_call 分属两批
//!     模型调用，delta=400,696 而 last=199,034）。
//!   * delta == 0 且 last 与上一条相同（121 条）⇒ 同一调用的重复上报（UI 回显），
//!     去重跳过——判据：total 未动、回声未变。
//!   * delta == 0 且 last 变化（85 条）⇒ **全部 85/85 紧随 `compacted` 记录**，形状
//!     退化为 (0,0,0,0,0,N>0)：压缩摘要调用回声，源端自身将其排除出累计 total
//!     （total 跨 compaction 不变，例 0.146.0-alpha.3 rollout-…-019f9496….jsonl
//!     L335–L339：total=9,823,579 → compacted → total=9,823,579、last=16,894）。
//!     记为 carried 事件并对账排除（与 rollout_v1 的携带量处理规则相同：
//!     Σ逐次 == 最终快照 + Σ携带）。
//!   * delta < 0（4 条）⇒ 源端计数回退/重置（0.142.3 L338 3,015,122→407,209、
//!     L456 →258400==context_window 且 last 全 0；0.146.0-alpha.3 L434/L441 微降
//!     319/607），无结构标记 ⇒ 记 `snapshot_regression` 诊断并重定基线；last 有
//!     实际变化且非全零时按"宁多勿漏"仍发事件，残差进对账差异。
//! - 文件首条 token_count：last == total（227/238）⇒ 首次调用；last != total
//!   （11 条，续接会话：total 含上一文件遗留上下文、last 仅本次）⇒ 发 last，
//!   残差由对账差异暴露。
//! - `compacted`：旧版存在（91 条）但 `latest_token_usage_record` 全部为 null
//!   （91/91，与 0.155 携带记录副本不同），仅作上述 delta==0 判据的结构标记。
//! - `turn_context.model`：2,482/2,482 存在 ⇒ 按不晚于调用行的 turn_context 归属。
//! - 无 response_id ⇒ 身份 `seq:{session}:{行号}`。
//! - 记录类型：session_meta/turn_context/event_msg/response_item/world_state/compacted/
//!   inter_agent_communication_metadata（129 条，本系列已知结构类型，静默忽略）。
//! - 全量核算：222/238 文件 Σ逐次==最终快照（matched）；16 个 mismatch 均属上述
//!   已解释类别（续接基线/多次调用区间/源端回退），进诊断不伪造数据。
//!
//! 版本策略（architecture.md#unknown-version）：本实现仅服务注册表登记的
//! 0.139–0.151 版本；未收录版本仍走 LatestFallback → rollout_v1（行为不变）。
//! 已知限制（与 rollout_v1 同）：latest_fallback 已消费游标的文件在解析器升级后
//! 不自动重扫；本系列此前全部判 incompatible 且游标未推进，登记后从头解析。

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use super::super::common::{map_codex_record, CodexRecordUsage};

pub const CODEX_LEGACY_PARSER_VERSION: &str = "codex-rollout-legacy-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// usage 六字段合计（i128 防溢出）。
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
    usage: StoredUsage,
    line: u64,
    ts_ms: i64,
}

/// 持久化快照六字段（serde 友好）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct StoredUsage {
    input: i64,
    cached: i64,
    write: i64,
    output: i64,
    reasoning: i64,
    total: i64,
}

impl StoredUsage {
    fn to_record(self) -> CodexRecordUsage {
        CodexRecordUsage {
            input_tokens: self.input,
            cached_input_tokens: self.cached,
            cache_write_input_tokens: self.write,
            output_tokens: self.output,
            reasoning_output_tokens: self.reasoning,
            total_tokens: self.total,
        }
    }
}

/// 持久化解析上下文（模型状态、上一快照/回声、对账合计、版本选择依据）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LegacyParseContext {
    model: Option<String>,
    cli_version: Option<String>,
    thread_id: Option<String>,
    parent_thread: Option<String>,
    originator: Option<String>,
    model_provider: Option<String>,
    category: Option<String>,
    session_started_ms: Option<i64>,
    /// 上一条 token_count 的 total（增量法基线；回归后重定基线）。
    prev_total: Option<i64>,
    /// 上一条 token_count 的 last 六字段（重复上报去重判据）。
    prev_last: Option<[i64; 6]>,
    /// 上一 token_count 以来是否见过 compacted（delta==0 且 last 变化的结构标记）。
    saw_compacted: bool,
    sum_per_call: UsageSums,
    sum_carried: UsageSums,
    final_snapshot: Option<SnapshotState>,
    turns_started: u64,
    turns_completed: u64,
    turns_aborted: u64,
    #[serde(default)]
    unknown_types: Vec<String>,
    #[serde(default)]
    has_record_errors: bool,
    /// 版本选择依据（known_version / latest_fallback）。
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> LegacyParseContext {
    if rescan {
        return LegacyParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<LegacyParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// 解析 usage 对象的六个必需数值字段；缺失/类型错误/负值/超限返回 None。
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

fn usage_six(usage: &CodexRecordUsage) -> [i64; 6] {
    [
        usage.input_tokens,
        usage.cached_input_tokens,
        usage.cache_write_input_tokens,
        usage.output_tokens,
        usage.reasoning_output_tokens,
        usage.total_tokens,
    ]
}

fn parse_envelope_ts(line: &serde_json::Value) -> Option<(i64, String)> {
    let raw = line.get("timestamp")?.as_str()?;
    let ts = raw.parse::<jiff::Timestamp>().ok()?.as_millisecond();
    Some((ts, raw.to_string()))
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

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

/// last_token_usage 语义判定结论（文件头核验依据的分类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastVerdict {
    /// 新调用（delta>0，或首条有效，或回归基线上的真实回声变化）。
    NewCall,
    /// 压缩摘要调用回声（delta==0 且 last 变化且紧随 compacted）：发事件但对账排除。
    CompactionEcho,
    /// 同一调用的重复上报（delta==0 且 last 未变）：去重跳过。
    DuplicateReport,
    /// 无逐次用量（last 缺失/全零）：跳过，残差由对账报告。
    NoPerCallEvidence,
}

/// 按核验判据分类一条 token_count 的 last 回声。
fn classify_last(
    prev_total: Option<i64>,
    cur_total: i64,
    prev_last: Option<[i64; 6]>,
    last: Option<[i64; 6]>,
    saw_compacted: bool,
) -> LastVerdict {
    let last_valid = last.is_some_and(|l| l[5] > 0);
    match prev_total {
        None => {
            // 首条：last 有实际用量即首调（或续接会话的首调），残差由对账暴露。
            if last_valid {
                LastVerdict::NewCall
            } else {
                LastVerdict::NoPerCallEvidence
            }
        }
        Some(prev) => {
            let delta = cur_total - prev;
            if delta > 0 {
                // 区间内 ≥1 次调用；last 是最新一次（核验：13,032/13,032 伴随 last 变化）。
                if last_valid {
                    LastVerdict::NewCall
                } else {
                    LastVerdict::NoPerCallEvidence
                }
            } else if delta == 0 {
                if last != prev_last && last_valid {
                    if saw_compacted {
                        LastVerdict::CompactionEcho
                    } else {
                        // 实读未出现（0/13,481）；按宁多勿漏发事件，残差进对账差异。
                        LastVerdict::NewCall
                    }
                } else {
                    LastVerdict::DuplicateReport
                }
            } else {
                // 源端计数回退：真实回声变化仍发事件（宁多勿漏），基线重定。
                if last_valid && last != prev_last {
                    LastVerdict::NewCall
                } else {
                    LastVerdict::NoPerCallEvidence
                }
            }
        }
    }
}

/// 从 token_count 的 last_token_usage 构造 model_call 事件（身份 seq:{session}:{行号}）。
#[allow(clippy::too_many_arguments)]
fn build_last_event(
    target: &ScanTarget,
    context: &LegacyParseContext,
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
    let session_key = context.thread_id.as_deref().unwrap_or("unknown-session");
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("seq:{session_key}:{line}"),
        record_kind: RecordKind::ModelCall,
        schema_version: context
            .cli_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: CODEX_LEGACY_PARSER_VERSION.to_string(),
        parse_basis: context.version_basis,
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(session_key.to_string()),
        parent_session_id: context.parent_thread.clone(),
        host_application: map_originator(context.originator.as_deref()),
        agent: "codex".to_string(),
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

/// 增量扫描一个旧版 rollout JSONL 文件（统一入口 `CodexAdapter::scan` 按注册表分派）。
pub fn scan(
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
                let version = json_str(&payload, "cli_version");
                let selection = super::super::versions::select(version);
                context.version_basis = Some(selection.basis);
                context.cli_version = version.map(str::to_string);
                context.thread_id = json_str(&payload, "id")
                    .or_else(|| json_str(&payload, "session_id"))
                    .map(str::to_string);
                context.parent_thread = json_str(&payload, "parent_thread_id").map(str::to_string);
                context.originator = json_str(&payload, "originator").map(str::to_string);
                context.model_provider = json_str(&payload, "model_provider").map(str::to_string);
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
            "compacted" => {
                // 旧版 latest_token_usage_record 全为 null（91/91 实读），无携带记录副本；
                // 仅作为 delta==0 且 last 变化的结构标记。
                context.saw_compacted = true;
            }
            "event_msg" => {
                let sub = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match sub {
                    "token_count" => {
                        if super::super::common::token_count_has_no_usage(&payload) {
                            continue;
                        }
                        let info = payload
                            .get("info")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let Some(total) = info.get("total_token_usage").and_then(parse_usage)
                        else {
                            diagnostics.push(diag(
                                "usage_shape_deviation",
                                Some("info.total_token_usage"),
                                raw.number,
                                "token_count snapshot missing required numeric fields; skipped",
                            ));
                            continue;
                        };
                        let last = info.get("last_token_usage").and_then(parse_usage);
                        let Some((observed_ms, source_time)) = parse_envelope_ts(&line) else {
                            diagnostics.push(diag(
                                "timestamp_unparseable",
                                Some("timestamp"),
                                raw.number,
                                "envelope timestamp missing or unparseable; snapshot skipped",
                            ));
                            continue;
                        };
                        let cur_total = total.total_tokens;
                        let verdict = classify_last(
                            context.prev_total,
                            cur_total,
                            context.prev_last,
                            last.as_ref().map(usage_six),
                            context.saw_compacted,
                        );
                        if let Some(prev) = context.prev_total.filter(|p| cur_total < *p) {
                            diagnostics.push(diag(
                                "snapshot_regression",
                                Some("info.total_token_usage"),
                                raw.number,
                                &format!(
                                    "cumulative snapshot decreased {prev} -> {cur_total} without structural reset evidence; delta baseline rebased"
                                ),
                            ));
                        }
                        match verdict {
                            LastVerdict::NewCall | LastVerdict::CompactionEcho => {
                                let usage = last.expect("verdict guarantees last");
                                if verdict == LastVerdict::NewCall {
                                    context.sum_per_call.add(&usage);
                                } else {
                                    context.sum_carried.add(&usage);
                                }
                                events.push(build_last_event(
                                    target,
                                    &context,
                                    &usage,
                                    observed_ms,
                                    &source_time,
                                    raw.number,
                                    now_ms,
                                    &mut diagnostics,
                                ));
                            }
                            LastVerdict::DuplicateReport | LastVerdict::NoPerCallEvidence => {}
                        }
                        context.prev_total = Some(cur_total);
                        context.prev_last = last.as_ref().map(usage_six);
                        context.saw_compacted = false;
                        context.final_snapshot = Some(SnapshotState {
                            usage: StoredUsage {
                                input: total.input_tokens,
                                cached: total.cached_input_tokens,
                                write: total.cache_write_input_tokens,
                                output: total.output_tokens,
                                reasoning: total.reasoning_output_tokens,
                                total: total.total_tokens,
                            },
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
            // 旧系列已知结构类型：内容不含 usage，静默忽略。
            "response_item" | "world_state" | "inter_agent_communication_metadata" => {}
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
    // 对账只在读到当前文件尾时进行（文件可能仍在增长）：
    // Σ逐次（regular + carried）== 最终快照 + Σ carried（压缩摘要回声被源端排除出快照）。
    if status == ScanStatus::Complete {
        let detail = context.sum_per_call.total + context.sum_carried.total;
        let snapshot = context.final_snapshot.map(|s| i128::from(s.usage.total));
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
                    "per-call sum {} != final snapshot {} + compaction carried {} (diff {}); known causes: resumed-session baseline, multi-call interval, source counter regression",
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
            let mapped = map_codex_record(&snap.usage.to_record());
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
    context.has_record_errors |= !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            // 旧载体依赖 total/last 一起识别调用，快照形状异常也可能丢失调用。
            matches!(
                d.code.as_str(),
                "bad_json_line"
                    | "usage_shape_deviation"
                    | "timestamp_unparseable"
                    | "line_too_long"
            )
        });
    let degraded = context.has_record_errors;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn six_usage(
        input: i64,
        cached: i64,
        write: i64,
        output: i64,
        reasoning: i64,
        total: i64,
    ) -> [i64; 6] {
        [input, cached, write, output, reasoning, total]
    }

    #[test]
    fn first_token_count_emits_when_last_positive() {
        assert_eq!(
            classify_last(
                None,
                100,
                None,
                Some(six_usage(80, 20, 0, 20, 5, 100)),
                false
            ),
            LastVerdict::NewCall
        );
        assert_eq!(
            classify_last(None, 0, None, Some(six_usage(0, 0, 0, 0, 0, 0)), false),
            LastVerdict::NoPerCallEvidence
        );
        assert_eq!(
            classify_last(None, 100, None, None, false),
            LastVerdict::NoPerCallEvidence
        );
    }

    #[test]
    fn positive_delta_is_new_call_even_when_equal_to_previous() {
        // delta>0 恒为新调用（实读 13,032/13,032；不依赖 last 是否与上一条相同）。
        let last = six_usage(80, 20, 0, 20, 5, 100);
        assert_eq!(
            classify_last(Some(0), 100, Some(last), Some(last), false),
            LastVerdict::NewCall
        );
        // last 缺失时无法生成逐次用量事件。
        assert_eq!(
            classify_last(Some(0), 100, Some(last), None, false),
            LastVerdict::NoPerCallEvidence
        );
    }

    #[test]
    fn zero_delta_dedups_repeated_report() {
        let last = six_usage(80, 20, 0, 20, 5, 100);
        assert_eq!(
            classify_last(Some(100), 100, Some(last), Some(last), false),
            LastVerdict::DuplicateReport
        );
    }

    #[test]
    fn zero_delta_changed_last_after_compaction_is_carried() {
        // 压缩摘要回声：total 不动、last 变化（退化为仅 total>0）。
        let prev = six_usage(214636, 211840, 0, 982, 391, 215618);
        let echo = six_usage(0, 0, 0, 0, 0, 13444);
        assert_eq!(
            classify_last(Some(3_000_000), 3_000_000, Some(prev), Some(echo), true),
            LastVerdict::CompactionEcho
        );
        // 无 compacted 标记的零增量变化（实读未出现）：宁多勿漏发事件。
        assert_eq!(
            classify_last(Some(3_000_000), 3_000_000, Some(prev), Some(echo), false),
            LastVerdict::NewCall
        );
    }

    #[test]
    fn regression_rebases_and_emits_only_real_change() {
        let prev_last = six_usage(146293, 140160, 0, 1014, 0, 147307);
        let new_last = six_usage(147942, 140160, 0, 867, 516, 148809);
        // 计数回退 + 真实回声变化（0.142.3 L338 实读形状）⇒ 发事件。
        assert_eq!(
            classify_last(
                Some(3_015_122),
                407_209,
                Some(prev_last),
                Some(new_last),
                false
            ),
            LastVerdict::NewCall
        );
        // 回退且 last 全零（0.142.3 L456 实读形状）⇒ 无逐次用量。
        assert_eq!(
            classify_last(
                Some(568_759),
                258_400,
                Some(new_last),
                Some(six_usage(0, 0, 0, 0, 0, 0)),
                false
            ),
            LastVerdict::NoPerCallEvidence
        );
        // 回退且 last 未变 ⇒ 无新调用。
        assert_eq!(
            classify_last(
                Some(568_759),
                258_400,
                Some(new_last),
                Some(new_last),
                false
            ),
            LastVerdict::NoPerCallEvidence
        );
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        let legacy = serde_json::json!({
            "model": Some("gpt-5-codex"),
            "cli_version": Some("0.142.5"),
            "thread_id": Some("t"),
            "parent_thread": null,
            "originator": null,
            "model_provider": Some("openai"),
            "category": Some("primary"),
            "session_started_ms": Some(1),
            "prev_total": Some(100),
            "prev_last": Some([80, 20, 0, 20, 5, 100]),
            "saw_compacted": false,
            "sum_per_call": UsageSums::default(),
            "sum_carried": UsageSums::default(),
            "final_snapshot": null,
            "turns_started": 1,
            "turns_completed": 1,
            "turns_aborted": 0,
            "unknown_types": [],
        });
        let ctx: LegacyParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
        assert_eq!(ctx.cli_version.as_deref(), Some("0.142.5"));
        assert_eq!(ctx.prev_last, Some([80, 20, 0, 20, 5, 100]));
    }
}
