//! VS Code Copilot Chat 会话日志格式实现（`session_log_v3`，
//! vscode-chat-session-log-v3）。
//!
//! 格式依据（microsoft/vscode 源码 + 本机 VS Code 1.140.0 真实数据核对
//! 2026-10-01，10 请求 5.25MB/最大行 837KB 全字段核验）：
//! - 文件：`workspaceStorage/<hash>/chatSessions/<sessionId>.jsonl`，
//!   objectMutationLog Entry 逐行 JSON：
//!   `{kind:0,v}` 初始（压缩重写后再次出现）、`{kind:1,k,v}` Set、
//!   `{kind:2,k,v?,i?}` Push（i = 截断后长度再追加）、`{kind:3,k}` Delete。
//! - 请求级字段语义（chatModel.ts toJSON + 扩展 agentIntent.ts 直传 API
//!   usage.prompt_tokens/completion_tokens）：
//!   `promptTokens` = **末次模型调用输入**、`completionTokens` =
//!   **整 turn 跨调用累计输出**（modelTotals 存在时为权威整轮总量）、
//!   `elapsedMs`、`copilotCredits`（credit 非 token 不入账）、
//!   `modelState{value,completedAt}`（0 Pending/1 Complete/2 Cancelled/
//!   3 Failed/4 NeedsInput）。
//!
//! 增量约定：kv 更新是周期采样快照，不能按行增量拼接——每轮**全量重放**
//! （文件经 1024 条压缩有界），事件按 `vscode-chat:<sessionId>:<requestId>`
//! 键 upsert 幂等（同内容 unchanged，流式计数器增长走 Replace）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::finish;
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, FieldQuality, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, TokenQuality, TokenUsage, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const COPILOT_CHAT_PARSER_VERSION: &str = "vscode-copilot-chat-session-log-2";

// 解析格式未变；旧游标需重放一次以修正健康状态和日汇总缺字段计数。
// 保留既有 revision/tracked_events，不能清空上下文后退回低修订快照。
const SCAN_POLICY_VERSION: u32 = 1;

pub fn should_scan_unchanged(stored: &StoredScanState) -> bool {
    stored
        .parse_context
        .as_ref()
        .and_then(|v| v.get("scan_policy_version"))
        .and_then(Value::as_u64)
        != Some(u64::from(SCAN_POLICY_VERSION))
}

/// 单 turn 时长/TTFT 合理上限（30 天毫秒）：超出按损坏丢弃。
const MAX_DURATION_MS: i64 = 30 * 24 * 3600 * 1000;

/// 时间戳合理区间（毫秒）。
const PLAUSIBLE_MS: std::ops::RangeInclusive<i64> =
    crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CopilotChatParseContext {
    #[serde(default)]
    scan_policy_version: u32,
    #[serde(default)]
    version_basis: Option<VersionBasis>,
    #[serde(default)]
    revision: i64,
    #[serde(default)]
    snapshot_hash: String,
    #[serde(default)]
    tracked_events: Vec<EventInput>,
}

fn diag(code: &str, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

/// token 列合理域（0..=MAX_TOKEN_VALUE）；越界置未知并记诊断。
fn token_col(value: Option<i64>) -> (Option<i64>, bool) {
    match value {
        Some(n) if (0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) => (Some(n), false),
        Some(_) => (None, true),
        None => (None, false),
    }
}

/// 沿路径段（对象键/数组下标）解析可变引用。
fn resolve_mut<'a>(node: &'a mut Value, path: &[Value]) -> Option<&'a mut Value> {
    let mut cur = node;
    for seg in path {
        cur = match seg {
            Value::String(key) => cur.as_object_mut()?.get_mut(key)?,
            Value::Number(idx) => {
                let i = idx.as_u64()? as usize;
                cur.as_array_mut()?.get_mut(i)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// Set：显式 null 保留；仅缺失 v 表示 JavaScript undefined。
fn apply_set(state: &mut Value, path: &[Value], value: Option<Value>) -> Result<(), ()> {
    let (last, parents) = path.split_last().ok_or(())?;
    let parent = resolve_mut(state, parents).ok_or(())?;
    match last {
        Value::String(key) => {
            let obj = parent.as_object_mut().ok_or(())?;
            match value {
                Some(v) => {
                    obj.insert(key.clone(), v);
                }
                None => {
                    obj.remove(key);
                }
            }
            Ok(())
        }
        Value::Number(idx) => {
            let i = idx.as_u64().ok_or(())? as usize;
            let arr = parent.as_array_mut().ok_or(())?;
            match value {
                Some(v) if i < arr.len() => {
                    arr[i] = v;
                    Ok(())
                }
                // 越界 Set 未见产生源：不静默扩张，记路径失败由调用方诊断。
                _ => Err(()),
            }
        }
        _ => Err(()),
    }
}

/// Push：末端按对象键定位数组，先截断到 `start` 长度再追加 items。
fn apply_push(
    state: &mut Value,
    path: &[Value],
    items: Vec<Value>,
    start: Option<u64>,
) -> Result<(), ()> {
    let (last, parents) = path.split_last().ok_or(())?;
    let key = match last {
        Value::String(key) => key.clone(),
        // 数组键（对象属性名）也可能以数字形态出现于路径段，此处末端必是键。
        _ => return Err(()),
    };
    let parent = resolve_mut(state, parents).ok_or(())?;
    let obj = parent.as_object_mut().ok_or(())?;
    let mut arr = obj
        .get(&key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(i) = start {
        if i > arr.len() as u64 {
            return Err(());
        }
        arr.truncate(i as usize);
    }
    arr.extend(items);
    obj.insert(key, Value::Array(arr));
    Ok(())
}

/// 重放完整 mutation 日志，得到最终会话状态。
fn replay(
    lines: &[crate::adapters::jsonl::RawLine],
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<Value> {
    let mut state: Option<Value> = None;
    for line in lines {
        let line_no = line.number;
        let text = line.text.as_str();
        let doc: Value = match crate::adapters::run_policy::json_from_str(text) {
            Ok(doc) => doc,
            Err(_) => {
                diagnostics.push(diag(
                    "line_json_invalid",
                    &format!("line:{line_no}"),
                    "mutation entry is not valid JSON; line skipped",
                ));
                continue;
            }
        };
        match doc.get("kind").and_then(Value::as_i64) {
            Some(0) => {
                // 初始（含压缩重写后的新初始）：整体替换状态。
                state = doc.get("v").cloned().filter(Value::is_object);
            }
            Some(1) => {
                let Some(current) = state.as_mut() else {
                    diagnostics.push(diag(
                        "missing_initial_entry",
                        &format!("line:{line_no}"),
                        "Set entry before any Initial entry; line skipped",
                    ));
                    continue;
                };
                let path = doc
                    .get("k")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let value = doc.get("v").cloned();
                if apply_set(current, &path, value).is_err() {
                    diagnostics.push(diag(
                        "path_resolve_failed",
                        &format!("line:{line_no}"),
                        "Set path unresolved against current state; line skipped",
                    ));
                }
            }
            Some(2) => {
                let Some(current) = state.as_mut() else {
                    diagnostics.push(diag(
                        "missing_initial_entry",
                        &format!("line:{line_no}"),
                        "Push entry before any Initial entry; line skipped",
                    ));
                    continue;
                };
                let path = doc
                    .get("k")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let items = doc
                    .get("v")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let start = doc.get("i").and_then(Value::as_u64);
                if apply_push(current, &path, items, start).is_err() {
                    diagnostics.push(diag(
                        "path_resolve_failed",
                        &format!("line:{line_no}"),
                        "Push path unresolved against current state; line skipped",
                    ));
                }
            }
            Some(3) => {
                let Some(current) = state.as_mut() else {
                    diagnostics.push(diag(
                        "missing_initial_entry",
                        &format!("line:{line_no}"),
                        "Delete entry before any Initial entry; line skipped",
                    ));
                    continue;
                };
                let path = doc
                    .get("k")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if apply_set(current, &path, None).is_err() {
                    diagnostics.push(diag(
                        "path_resolve_failed",
                        &format!("line:{line_no}"),
                        "Delete path unresolved against current state; line skipped",
                    ));
                }
            }
            _ => {
                diagnostics.push(diag(
                    "entry_kind_unknown",
                    &format!("line:{line_no}"),
                    "mutation entry has unknown kind; line skipped",
                ));
            }
        }
    }
    state
}

/// 请求重放最终值的字段视图（只取白名单，不触碰消息正文）。
struct RequestUsage {
    request_id: Option<String>,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    model_totals: Vec<ModelTotalEntry>,
    elapsed_ms: Option<i64>,
    ttft_ms: Option<i64>,
    model_state_value: Option<i64>,
    completed_at_ms: Option<i64>,
    timestamp_ms: Option<i64>,
    resolved_model: Option<String>,
    request_model_id: Option<String>,
    last_round_model: Option<String>,
    rounds: Vec<Value>,
}

struct ModelTotalEntry {
    model: Option<String>,
    input_tokens: Option<i64>,
    cached_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

fn extract_request(r: &Value) -> RequestUsage {
    let result = r
        .get("result")
        .filter(|v| v.get("timings").is_some() || v.get("metadata").is_some());
    let metadata = result.and_then(|v| v.get("metadata"));
    let rounds = metadata
        .and_then(|m| m.get("toolCallRounds"))
        .and_then(Value::as_array);
    RequestUsage {
        request_id: r
            .get("requestId")
            .and_then(Value::as_str)
            .map(str::to_string),
        prompt_tokens: r.get("promptTokens").and_then(Value::as_i64),
        completion_tokens: r.get("completionTokens").and_then(Value::as_i64),
        model_totals: r
            .get("modelTotals")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|e| ModelTotalEntry {
                        model: e.get("model").and_then(Value::as_str).map(str::to_string),
                        input_tokens: e.get("inputTokens").and_then(Value::as_i64),
                        cached_tokens: e.get("cachedTokens").and_then(Value::as_i64),
                        output_tokens: e.get("outputTokens").and_then(Value::as_i64),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        elapsed_ms: r.get("elapsedMs").and_then(Value::as_i64),
        ttft_ms: result
            .and_then(|v| v.get("timings"))
            .and_then(|t| t.get("firstProgress"))
            .and_then(Value::as_i64),
        model_state_value: r
            .get("modelState")
            .and_then(|m| m.get("value"))
            .and_then(Value::as_i64),
        completed_at_ms: r
            .get("modelState")
            .and_then(|m| m.get("completedAt"))
            .and_then(Value::as_i64),
        timestamp_ms: r.get("timestamp").and_then(Value::as_i64),
        resolved_model: metadata
            .and_then(|m| m.get("resolvedModel"))
            .and_then(Value::as_str)
            .map(str::to_string),
        request_model_id: r.get("modelId").and_then(Value::as_str).map(str::to_string),
        last_round_model: rounds
            .and_then(|arr| arr.last())
            .and_then(|round| round.get("modelId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        rounds: rounds.cloned().unwrap_or_default(),
    }
}

impl RequestUsage {
    /// 任一 token 字段有值才入账（未知用量不补零；仅 credit 不构成事件）。
    fn has_token_signal(&self) -> bool {
        self.prompt_tokens.is_some()
            || self.completion_tokens.is_some()
            || self.model_totals.iter().any(|e| {
                e.input_tokens.is_some() || e.cached_tokens.is_some() || e.output_tokens.is_some()
            })
    }

    /// modelState 1/2/3（Complete/Cancelled/Failed）封口；0/4/缺失仍流式。
    fn lifecycle(&self) -> Lifecycle {
        match self.model_state_value {
            Some(1) | Some(2) | Some(3) => Lifecycle::Final,
            _ => Lifecycle::Partial,
        }
    }

    fn occurred_ms(&self) -> Option<i64> {
        self.completed_at_ms
            .filter(|t| PLAUSIBLE_MS.contains(t))
            .or_else(|| self.timestamp_ms.filter(|t| PLAUSIBLE_MS.contains(t)))
    }

    fn time_basis(&self) -> TimeBasis {
        if self
            .completed_at_ms
            .filter(|t| PLAUSIBLE_MS.contains(t))
            .is_some()
        {
            TimeBasis::SourceCompletion
        } else {
            TimeBasis::SourceStart
        }
    }

    /// 模型归属：resolvedModel（结果元数据）> 请求 modelId（去 copilot/ 前缀）
    /// > 末轮 modelId。
    fn model_raw(&self) -> Option<String> {
        let round_models: std::collections::BTreeSet<_> = self
            .rounds
            .iter()
            .filter_map(|r| r.get("modelId").and_then(Value::as_str))
            .collect();
        if round_models.len() > 1 {
            return None;
        }
        if let Some(resolved) = &self.resolved_model {
            return Some(resolved.clone());
        }
        if let Some(model_id) = &self.request_model_id {
            return Some(
                model_id
                    .strip_prefix("copilot/")
                    .unwrap_or(model_id)
                    .to_string(),
            );
        }
        self.last_round_model.clone()
    }
}

/// 默认路径映射：input = 末次调用输入（下界）、output = 整 turn 累计。
fn map_default_usage(
    prompt: Option<i64>,
    completion: Option<i64>,
) -> crate::adapters::usage_map::MappedUsage {
    // 输入只覆盖末次调用，输出覆盖整轮；两者不能派生整轮总 token。
    let total = None;
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: None,
        input_cache_write: None,
        input_total: prompt,
        output_total: completion,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: crate::domain::FieldQuality::Unknown,
        input_cache_read: crate::domain::FieldQuality::Unknown,
        input_cache_write: crate::domain::FieldQuality::Unknown,
        input_total: prompt
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        output_total: completion
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        output_reasoning: crate::domain::FieldQuality::Unknown,
        total_tokens: if total.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        source_total: crate::domain::FieldQuality::Unknown,
    };
    finish(usage, quality, Vec::new())
}

/// modelTotals 权威映射：input 含 cached（uncached 派生）、output 直报。
fn map_model_total_usage(entry: &ModelTotalEntry) -> crate::adapters::usage_map::MappedUsage {
    let mut diagnostics = Vec::new();
    let uncached = match (entry.input_tokens, entry.cached_tokens) {
        (Some(total), Some(cached)) => crate::adapters::usage_map::sub_checked(
            "input_uncached",
            total,
            cached,
            &mut diagnostics,
        ),
        _ => None,
    };
    let total = match (entry.input_tokens, entry.output_tokens) {
        (Some(i), Some(o)) => match i.checked_add(o) {
            Some(t) if t <= crate::domain::MAX_TOKEN_VALUE => Some(t),
            _ => {
                diagnostics.push(crate::metrics::Contradiction {
                    code: "token_shape_deviation",
                    field: "total_tokens",
                    detail: format!("derived total {i} + {o} out of range; kept unknown"),
                });
                None
            }
        },
        _ => None,
    };
    let usage = TokenUsage {
        input_uncached: uncached,
        input_cache_read: entry.cached_tokens,
        input_cache_write: None,
        input_total: entry.input_tokens,
        output_total: entry.output_tokens,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = TokenQuality {
        input_uncached: if uncached.is_some() {
            FieldQuality::Derived
        } else {
            FieldQuality::Unknown
        },
        input_cache_read: entry
            .cached_tokens
            .map(|_| FieldQuality::Reported)
            .unwrap_or(FieldQuality::Unknown),
        input_cache_write: FieldQuality::Unknown,
        input_total: entry
            .input_tokens
            .map(|_| FieldQuality::Reported)
            .unwrap_or(FieldQuality::Unknown),
        output_total: entry
            .output_tokens
            .map(|_| FieldQuality::Reported)
            .unwrap_or(FieldQuality::Unknown),
        output_reasoning: FieldQuality::Unknown,
        total_tokens: if total.is_some() {
            FieldQuality::Derived
        } else {
            FieldQuality::Unknown
        },
        source_total: FieldQuality::Unknown,
    };
    finish(usage, quality, diagnostics)
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // 全量重放约定：不消费旧游标（kv 采样快照不可增量拼接）；游标仅记录
    // 已读字节供 framework 的无变化短路使用。
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<CopilotChatParseContext>(v.clone()).ok())
        .unwrap_or_default();
    let policy_changed = context.scan_policy_version != SCAN_POLICY_VERSION;
    let read = match read_jsonl(&target.path, 0, 1, &limits.jsonl) {
        Ok(read) => read,
        Err(err) if crate::adapters::framework::is_transient_io(&err) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: Vec::new(),
                lines_read: 0,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
        Err(err) => return Err(err.into()),
    };
    let mut diagnostics = Vec::new();
    for bad in &read.bad_lines {
        crate::adapters::run_policy::check()?;
        diagnostics.push(diag(
            "line_json_invalid",
            &format!("line:{}", bad.number),
            "line rejected by bounded reader; content not inspected",
        ));
    }
    let status = match read.stop {
        StopReason::Eof if read.pending_bytes == 0 => ScanStatus::Complete,
        StopReason::Eof => ScanStatus::Pending,
        StopReason::LineTooLong { .. } => ScanStatus::LineTooLong,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
    };
    if status != ScanStatus::Complete {
        // 行超限：不推进游标（重放语义下也不提交事件），受控重试。
        return Ok(ScanOutcome {
            status,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let Some(state) = replay(&read.lines, &mut diagnostics) else {
        // 无初始条目（空/损坏）：等待或下轮重试，不虚报成功。
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics,
            lines_read: read.lines.len() as u64,
            records_seen: read.lines.len() as u64,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let found_version = state
        .get("version")
        .and_then(Value::as_i64)
        .map(|n| n.to_string());
    let basis = super::select(found_version.as_deref()).basis;
    context.version_basis = Some(basis);

    let session_id = state
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let Some(session_id) = session_id else {
        diagnostics.push(diag(
            "session_id_missing",
            "header",
            "initial record lacks sessionId; no events emitted",
        ));
        return Ok(ScanOutcome {
            status,
            cursor: Some(serde_json::to_value(JsonlCursor {
                generation: target.generation,
                offset: read.next_offset,
                line_number: read.next_line_number,
            })?),
            parse_context: Some(serde_json::to_value(&context)?),
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics,
            lines_read: read.lines.len() as u64,
            records_seen: read.lines.len() as u64,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };

    let requests = state
        .get("requests")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut events = Vec::new();
    let mut replaced_turns = std::collections::BTreeSet::new();
    for (index, request) in requests.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        // chatSessions 是 VS Code 的通用会话库；其他扩展的轮次不能归为 Copilot。
        if !request
            .pointer("/agent/id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("github.copilot."))
        {
            continue;
        }
        let invalid = |record: &Value, keys: &[&str]| {
            keys.iter().any(|key| {
                record
                    .get(*key)
                    .is_some_and(|v| !v.is_null() && v.as_i64().is_none())
            })
        };
        if invalid(request, &["promptTokens", "completionTokens"])
            || request
                .get("modelTotals")
                .and_then(Value::as_array)
                .is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        invalid(entry, &["inputTokens", "cachedTokens", "outputTokens"])
                    })
                })
        {
            diagnostics.push(diag(
                "token_shape_deviation",
                &format!("request:{index}"),
                "non-integer or overflowing token field; snapshot withheld",
            ));
            continue;
        }
        let view = extract_request(request);
        if !view.has_token_signal() && view.rounds.is_empty() {
            continue;
        }
        let Some(occurred_ms) = view.occurred_ms() else {
            diagnostics.push(diag(
                "timestamp_missing",
                &format!("request:{index}"),
                "no plausible completedAt/timestamp; request skipped",
            ));
            continue;
        };
        let base_key = match &view.request_id {
            Some(id) => format!("vscode-chat:{session_id}:{id}"),
            None => {
                diagnostics.push(diag(
                    "request_id_missing",
                    &format!("request:{index}"),
                    "requestId absent; unstable positional identity rejected",
                ));
                continue;
            }
        };
        let lifecycle = view.lifecycle();
        replaced_turns.insert(base_key.clone());
        let time_basis = view.time_basis();
        let model = view.model_raw();
        let duration = view
            .elapsed_ms
            .filter(|d| (0..=MAX_DURATION_MS).contains(d));
        let ttft = view.ttft_ms.filter(|d| (0..=MAX_DURATION_MS).contains(d));
        let (prompt, bad_prompt) = token_col(view.prompt_tokens);
        let (completion, bad_completion) = token_col(view.completion_tokens);
        if bad_prompt || bad_completion {
            diagnostics.push(diag(
                "token_shape_deviation",
                &format!("request:{index}"),
                "prompt/completion tokens out of range; kept unknown",
            ));
        }
        let usable_totals: Vec<&ModelTotalEntry> = view
            .model_totals
            .iter()
            .filter(|e| {
                e.input_tokens.is_some() || e.cached_tokens.is_some() || e.output_tokens.is_some()
            })
            .collect();
        if usable_totals.is_empty() {
            let mapped = map_default_usage(prompt, completion);
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: base_key.clone(),
                record_kind: RecordKind::UsageObservation,
                schema_version: super::COPILOT_CHAT_FORMAT_VERSION.to_string(),
                parser_version: COPILOT_CHAT_PARSER_VERSION.to_string(),
                parse_basis: Some(basis),
                origin_call_id: None,
                attempt_id: None,
                session_id: Some(session_id.clone()),
                parent_session_id: None,
                host_application: Some("VS Code".to_string()),
                agent: "vscode-copilot-chat".to_string(),
                call_category: CallCategory::Primary,
                occurred_at_ms: occurred_ms,
                observed_at_ms: Some(now_ms),
                source_time: None,
                time_basis,
                interval_start_ms: None,
                interval_end_ms: None,
                provider_id: Some("github-copilot".to_string()),
                model_raw: model.clone(),
                model_canonical: None,
                model_attribution: ModelAttribution::RequestField,
                usage: mapped.usage,
                quality: mapped.quality,
                lifecycle,
                source_revision: None,
                error_status: (view.model_state_value == Some(3)).then(|| "failed".to_string()),
                duration_ms: duration,
                ttft_ms: ttft,
                attribution_status: AttributionStatus::Verified,
                exclusion_reason: None,
                cost: None,
            });
            diagnostics.push(diag("turn_input_incomplete", &format!("request:{index}"),
                "promptTokens covers the last call only; whole-turn input/total and auxiliary-call coverage are unknown"));
        } else {
            let mut models = std::collections::BTreeSet::new();
            // 权威整轮逐模型总量（agent host 会话）：一段一事件，键附加模型段。
            for (mt_index, entry) in usable_totals.iter().enumerate() {
                if !models.insert(entry.model.as_deref().unwrap_or("unknown")) {
                    diagnostics.push(diag(
                        "duplicate_model_total",
                        &format!("request:{index}"),
                        "duplicate modelTotals identity; snapshot withheld",
                    ));
                    continue;
                }
                let (input_tokens, bad_input) = token_col(entry.input_tokens);
                let (cached_tokens, bad_cached) = token_col(entry.cached_tokens);
                let (output_tokens, bad_output) = token_col(entry.output_tokens);
                if bad_input || bad_cached || bad_output {
                    diagnostics.push(diag(
                        "token_shape_deviation",
                        &format!("request:{index}:model:{mt_index}"),
                        "modelTotals token out of range; entry skipped",
                    ));
                    continue;
                }
                let mapped = map_model_total_usage(&ModelTotalEntry {
                    model: entry.model.clone(),
                    input_tokens,
                    cached_tokens,
                    output_tokens,
                });
                for d in &mapped.diagnostics {
                    diagnostics.push(diag(
                        d.code,
                        &format!("request:{index}:model:{mt_index}"),
                        &d.detail,
                    ));
                }
                let key = format!(
                    "{base_key}#model:{}",
                    entry.model.as_deref().unwrap_or("unknown")
                );
                events.push(EventInput {
                    source_instance_id: target.instance_id.clone(),
                    source_record_key: key,
                    record_kind: RecordKind::UsageObservation,
                    schema_version: super::COPILOT_CHAT_FORMAT_VERSION.to_string(),
                    parser_version: COPILOT_CHAT_PARSER_VERSION.to_string(),
                    parse_basis: Some(basis),
                    origin_call_id: None,
                    attempt_id: None,
                    session_id: Some(session_id.clone()),
                    parent_session_id: None,
                    host_application: Some("VS Code".to_string()),
                    agent: "vscode-copilot-chat".to_string(),
                    call_category: CallCategory::Primary,
                    occurred_at_ms: occurred_ms,
                    observed_at_ms: Some(now_ms),
                    source_time: None,
                    time_basis,
                    interval_start_ms: None,
                    interval_end_ms: None,
                    provider_id: Some("github-copilot".to_string()),
                    model_raw: entry.model.clone().or_else(|| model.clone()),
                    model_canonical: None,
                    model_attribution: ModelAttribution::RequestField,
                    usage: mapped.usage,
                    quality: mapped.quality,
                    lifecycle,
                    source_revision: None,
                    error_status: (view.model_state_value == Some(3)).then(|| "failed".to_string()),
                    duration_ms: duration,
                    ttft_ms: ttft,
                    attribution_status: AttributionStatus::Verified,
                    exclusion_reason: None,
                    cost: None,
                });
            }
        }
        // 每条工具循环 round 是已观测的一次主循环调用；token 仍由 turn
        // observation 贡献。不能把轮次或多模型汇总行数当作调用次数。
        let mut round_ids = std::collections::BTreeSet::new();
        for round in &view.rounds {
            let Some(id) = round
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let Some(ts) = round
                .get("timestamp")
                .and_then(Value::as_i64)
                .filter(|t| PLAUSIBLE_MS.contains(t))
            else {
                continue;
            };
            if !round_ids.insert(id) {
                continue;
            }
            let Some(mut call) = events
                .iter()
                .rev()
                .find(|e| {
                    e.source_record_key.starts_with(&base_key)
                        && e.record_kind == RecordKind::UsageObservation
                })
                .cloned()
            else {
                continue;
            };
            call.source_record_key = format!("{base_key}#round:{id}");
            call.record_kind = RecordKind::ModelCall;
            call.model_raw = round
                .get("modelId")
                .and_then(Value::as_str)
                .map(str::to_string);
            call.occurred_at_ms = ts;
            call.time_basis = TimeBasis::SourceStart;
            call.usage = TokenUsage::default();
            call.quality = TokenQuality::default();
            call.duration_ms = None;
            call.ttft_ms = None;
            call.error_status = None;
            call.lifecycle = Lifecycle::Final;
            events.push(call);
        }
    }
    // 只对白名单事件视图求修订。正文变化不改变修订；完整重放后的更正
    // 可降低计数。断行、达到读取上限或坏行均不能撤销或覆盖旧快照。
    if !diagnostics.iter().any(|d| {
        matches!(
            d.code.as_str(),
            "line_json_invalid"
                | "path_resolve_failed"
                | "entry_kind_unknown"
                | "missing_initial_entry"
                | "duplicate_model_total"
                | "token_shape_deviation"
        )
    }) {
        let snapshot_hash = crate::identity::content_hash(
            &events
                .iter()
                .map(crate::identity::event_content_hash)
                .collect::<Vec<_>>(),
        );
        if context.snapshot_hash != snapshot_hash || policy_changed {
            context.revision = context
                .revision
                .checked_add(1)
                .ok_or(CoreError::Overflow("copilot snapshot revision"))?;
        }
        let current: std::collections::BTreeSet<_> =
            events.iter().map(|e| e.source_record_key.clone()).collect();
        for old in &context.tracked_events {
            if !current.contains(&old.source_record_key)
                && old.record_kind == RecordKind::UsageObservation
                && replaced_turns.iter().any(|base| {
                    old.source_record_key == *base
                        || old.source_record_key.starts_with(&format!("{base}#model:"))
                })
            {
                let mut tombstone = old.clone();
                tombstone.lifecycle = Lifecycle::Corrected;
                tombstone.attribution_status = AttributionStatus::Excluded;
                tombstone.exclusion_reason =
                    Some("replaced by complete Copilot session snapshot".to_string());
                tombstone.source_revision = Some(context.revision);
                events.push(tombstone);
            } else if policy_changed && !current.contains(&old.source_record_key) {
                // 来源已清理的历史仍保留。规则更新时重发既有白名单快照，
                // 让这些旧事件所在日也重算；不恢复正文或已过期明细。
                let mut retained = old.clone();
                retained.observed_at_ms = Some(now_ms);
                events.push(retained);
            }
        }
        for event in &mut events {
            event.source_revision = Some(context.revision);
        }
        context.snapshot_hash = snapshot_hash;
        context.scan_policy_version = SCAN_POLICY_VERSION;
        let mut tracked: std::collections::BTreeMap<_, _> = context
            .tracked_events
            .into_iter()
            .map(|e| (e.source_record_key.clone(), e))
            .collect();
        for event in &events {
            if event.attribution_status == AttributionStatus::Excluded {
                tracked.remove(&event.source_record_key);
            } else {
                let mut e = event.clone();
                e.source_revision = None;
                e.observed_at_ms = None;
                tracked.insert(e.source_record_key.clone(), e);
            }
        }
        context.tracked_events = tracked.into_values().collect();
    } else {
        events.clear();
    }
    // 输入覆盖提示（turn_input_incomplete）是该格式固有限制——promptTokens 只覆盖末次
    // 调用，不代表坏记录或对账差异，不降级来源健康（architecture.md#unknown-version：
    // 仅坏记录/对账差异提示需核对）。其余诊断（坏行、非法 token、重复键、缺失归属等）仍降级。
    let health = if diagnostics
        .iter()
        .all(|d| d.code == "turn_input_incomplete")
    {
        "active".to_string()
    } else {
        "degraded".to_string()
    };
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(JsonlCursor {
            generation: target.generation,
            offset: read.next_offset,
            line_number: read.next_line_number,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: read.lines.len() as u64,
        records_seen: read.lines.len() as u64,
        reconciliations: Vec::new(),
        health,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log(name: &str, lines: &[String]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-copilot-chat-{}-{}-{}.jsonl",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        let lines: Vec<String> = lines
            .iter()
            .map(|line| {
                let Ok(mut doc) = crate::adapters::run_policy::json_from_str::<Value>(line) else {
                    return line.clone();
                };
                let requests = if doc["kind"] == 0 {
                    doc.pointer_mut("/v/requests")
                } else if doc["kind"] == 2 && doc["k"] == serde_json::json!(["requests"]) {
                    doc.get_mut("v")
                } else {
                    None
                };
                if let Some(requests) = requests.and_then(Value::as_array_mut) {
                    for request in requests {
                        if request.get("agent").is_none() {
                            request["agent"] =
                                serde_json::json!({"id":"github.copilot.editsAgent"});
                        }
                    }
                }
                doc.to_string()
            })
            .collect();
        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(&path, text).unwrap();
        path
    }

    fn target_for(path: &std::path::Path) -> ScanTarget {
        let probe = crate::adapters::jsonl::probe_file(path).unwrap();
        ScanTarget {
            instance_id: "copilot-chat@test".to_string(),
            path: path.to_path_buf(),
            file_id: "test".to_string(),
            file_identity: "test".to_string(),
            probe,
            generation: 0,
            rescan: false,
        }
    }

    fn run(path: &std::path::Path) -> ScanOutcome {
        scan(
            &target_for(path),
            &StoredScanState::default(),
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap()
    }

    fn header(requests: &str) -> String {
        format!(
            r#"{{"kind":0,"v":{{"version":3,"creationDate":1790781064329,"sessionId":"11111111-2222-3333-4444-555555555555","requests":{requests},"pendingRequests":[]}}}}"#
        )
    }

    #[test]
    fn replays_set_updates_last_wins_and_maps_event() {
        let path = temp_log(
            "map",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_a","timestamp":1790783284143,"modelId":"copilot/claude-opus-4.8"}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"promptTokens"],"v":17310}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":1931}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"promptTokens"],"v":91756}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":12733}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"elapsedMs"],"v":577886}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"modelState"],"v":{"value":1,"completedAt":1790783862030}}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"result"],"v":{"timings":{"firstProgress":9162,"totalElapsed":576142},"metadata":{"resolvedModel":"claude-opus-4-8","promptTokens":91756,"outputTokens":1003}}}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.status, ScanStatus::Complete);
        assert_eq!(outcome.events.len(), 1);
        let e = &outcome.events[0];
        assert_eq!(
            e.source_record_key,
            "vscode-chat:11111111-2222-3333-4444-555555555555:request_a"
        );
        // last-wins：末次调用输入 + 整 turn 累计输出。
        assert_eq!(e.usage.input_total, Some(91_756));
        assert_eq!(e.usage.output_total, Some(12_733));
        assert_eq!(e.usage.total_tokens, None);
        assert_eq!(e.usage.input_cache_read, None);
        assert_eq!(e.model_raw.as_deref(), Some("claude-opus-4-8"));
        assert_eq!(e.occurred_at_ms, 1_790_783_862_030);
        assert_eq!(e.time_basis, TimeBasis::SourceCompletion);
        assert_eq!(e.duration_ms, Some(577_886));
        assert_eq!(e.ttft_ms, Some(9_162));
        assert_eq!(e.lifecycle, Lifecycle::Final);
        assert_eq!(e.agent, "vscode-copilot-chat");
        assert_eq!(e.record_kind, RecordKind::UsageObservation);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pending_model_state_is_partial_and_start_time_basis() {
        let path = temp_log(
            "partial",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_b","timestamp":1790783284143}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"promptTokens"],"v":5000}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":120}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"modelState"],"v":{"value":0}}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].lifecycle, Lifecycle::Partial);
        assert_eq!(outcome.events[0].time_basis, TimeBasis::SourceStart);
        assert_eq!(outcome.events[0].occurred_at_ms, 1_790_783_284_143);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn push_truncate_replaces_tail_requests() {
        let path = temp_log(
            "truncate",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_a","timestamp":1790783284143},{"requestId":"request_b","timestamp":1790783284144}]}"#.to_string(),
                // 截断到长度 1（移除 request_b）再追加 request_c。
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_c","timestamp":1790783284145}],"i":1}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":10}"#.to_string(),
                r#"{"kind":1,"k":["requests",1,"completionTokens"],"v":20}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        let keys: Vec<&str> = outcome
            .events
            .iter()
            .map(|e| e.source_record_key.as_str())
            .collect();
        assert!(keys.iter().all(|k| !k.contains("request_b")));
        assert!(keys.iter().any(|k| k.ends_with("request_a")));
        assert!(keys.iter().any(|k| k.ends_with("request_c")));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn delete_entry_removes_key() {
        let path = temp_log(
            "delete",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_a","timestamp":1790783284143}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"promptTokens"],"v":5000}"#.to_string(),
                r#"{"kind":3,"k":["requests",0,"promptTokens"]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":120}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].usage.input_total, None);
        assert_eq!(outcome.events[0].usage.output_total, Some(120));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn compaction_rewrite_resets_state() {
        let path = temp_log(
            "compact",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_a","timestamp":1790783284143}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":10}"#.to_string(),
                // 压缩重写：新初始行（状态里已含 request_a 最终值）。
                header(
                    r#"[{"requestId":"request_a","timestamp":1790783284143,"completionTokens":10}]"#,
                ),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].usage.output_total, Some(10));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn request_without_usage_is_skipped() {
        let path = temp_log(
            "nousage",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_x","timestamp":1790783284143,"copilotCredits":0}]}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert!(outcome.events.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn model_totals_single_entry_is_authoritative() {
        let path = temp_log(
            "mt1",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_m","timestamp":1790783284143}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"promptTokens"],"v":900}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":100}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"modelTotals"],"v":[{"model":"gpt-5.5","inputTokens":1000,"cachedTokens":400,"outputTokens":200}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"modelState"],"v":{"value":1,"completedAt":1790783862030}}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 1);
        let e = &outcome.events[0];
        assert!(e.source_record_key.ends_with("#model:gpt-5.5"));
        assert_eq!(e.usage.input_total, Some(1000));
        assert_eq!(e.usage.input_cache_read, Some(400));
        assert_eq!(e.usage.input_uncached, Some(600));
        assert_eq!(e.usage.output_total, Some(200));
        assert_eq!(e.model_raw.as_deref(), Some("gpt-5.5"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn model_totals_multi_entry_emits_per_model() {
        let path = temp_log(
            "mt2",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_m2","timestamp":1790783284143}]}"#.to_string(),
                r#"{"kind":1,"k":["requests",0,"modelTotals"],"v":[{"model":"gpt-5.5","inputTokens":100,"outputTokens":20},{"model":"claude-opus-4.8","inputTokens":200,"cachedTokens":50,"outputTokens":30}]}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 2);
        let models: Vec<&str> = outcome
            .events
            .iter()
            .filter_map(|e| e.model_raw.as_deref())
            .collect();
        assert_eq!(models, vec!["gpt-5.5", "claude-opus-4.8"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_line_is_diagnosed_not_fatal() {
        let path = temp_log(
            "badline",
            &[
                header("[]"),
                "not json".to_string(),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_z","timestamp":1790783284143,"completionTokens":7}]}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert_eq!(outcome.events.len(), 0);
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "line_json_invalid"));
        assert_eq!(outcome.health, "degraded");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_timestamp_skips_request() {
        let path = temp_log(
            "nots",
            &[
                header("[]"),
                r#"{"kind":2,"k":["requests"],"v":[{"requestId":"request_nots","completionTokens":10}]}"#.to_string(),
            ],
        );
        let outcome = run(&path);
        assert!(outcome.events.is_empty());
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "timestamp_missing"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_log_yields_pending() {
        let path = temp_log("empty", &[]);
        let outcome = run(&path);
        assert_eq!(outcome.status, ScanStatus::Pending);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn budget_exhaustion_and_partial_tail_withhold_snapshot() {
        let path = temp_log(
            "budget",
            &[
                header(r#"[{"requestId":"a","timestamp":1790783284143,"completionTokens":30}]"#),
                r#"{"kind":1,"k":["requests",0,"completionTokens"],"v":10}"#.into(),
            ],
        );
        let mut limits = ScanLimits::default();
        limits.jsonl.max_lines = Some(1);
        let out = scan(
            &target_for(&path),
            &StoredScanState::default(),
            &limits,
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(out.status, ScanStatus::BudgetExhausted);
        assert!(out.events.is_empty());
        assert!(out.cursor.is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn null_set_and_nested_array_paths_preserve_replay_shape() {
        let mut state = serde_json::json!({"requests":[{"result":{"metadata":{}}}]});
        apply_set(
            &mut state,
            &serde_json::json!(["requests", 0, "result"])
                .as_array()
                .unwrap()
                .clone(),
            Some(Value::Null),
        )
        .unwrap();
        assert_eq!(state["requests"][0]["result"], Value::Null);
        assert!(state["requests"][0].get("result").is_some());
        assert!(apply_push(
            &mut state,
            &[Value::String("requests".into())],
            vec![],
            Some(100)
        )
        .is_err());
    }

    #[test]
    fn oversized_or_fractional_model_totals_are_diagnosed_without_panicking() {
        for value in [
            serde_json::json!(-1),
            serde_json::json!(u64::MAX),
            serde_json::json!(0.5),
        ] {
            let path = temp_log("bad-model-total",&[header(&serde_json::json!([{
                "requestId":"a","timestamp":1790783284143i64,"modelTotals":[{"model":"model-a","inputTokens":value,"outputTokens":1}],
                "result":{"metadata":{"toolCallRounds":[{"id":"a","timestamp":1790783284143i64}]}}
            }]).to_string())]);
            let out = run(&path);
            assert!(out.events.is_empty());
            assert!(out
                .diagnostics
                .iter()
                .any(|d| d.code == "token_shape_deviation"));
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn turn_input_incomplete_alone_keeps_source_active() {
        // 默认路径 turn（无 modelTotals）：promptTokens 只覆盖末次调用，产出
        // turn_input_incomplete 覆盖提示。该提示是格式固有限制、非坏记录或对账差异，
        // 不降级来源健康（否则每个普通 Copilot 会话都会被误标“需核对”）。
        let path = temp_log(
            "coverage-hint-active",
            &[header(
                &serde_json::json!([{
                    "requestId":"req_a","timestamp":1_790_783_284_143i64,
                    "promptTokens":1000,"completionTokens":200,
                    "modelState":{"value":1,"completedAt":1_790_783_862_030i64},
                    "result":{"metadata":{"toolCallRounds":[
                        {"id":"r1","modelId":"claude-opus-4.8","timestamp":1_790_783_284_143i64},
                        {"id":"r2","modelId":"claude-opus-4.8","timestamp":1_790_783_284_200i64}
                    ]}}
                }])
                .to_string(),
            )],
        );
        let outcome = run(&path);
        assert_eq!(outcome.status, ScanStatus::Complete);
        assert!(!outcome.diagnostics.is_empty());
        assert!(outcome
            .diagnostics
            .iter()
            .all(|d| d.code == "turn_input_incomplete"));
        assert_eq!(outcome.health, "active");
        // 一条 turn observation 承载 token；两条 round 计 model_call、逐轮 token 未知。
        let obs = outcome
            .events
            .iter()
            .filter(|e| e.record_kind == RecordKind::UsageObservation)
            .count();
        let rounds: Vec<&EventInput> = outcome
            .events
            .iter()
            .filter(|e| e.record_kind == RecordKind::ModelCall)
            .collect();
        assert_eq!((obs, rounds.len()), (1, 2));
        assert!(rounds
            .iter()
            .all(|e| e.usage.input_total.is_none() && e.usage.output_total.is_none()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_policy_is_not_marked_repaired_until_valid_snapshot() {
        let lines = [header(
            r#"[{"requestId":"req_a","timestamp":1790783284143,"promptTokens":100,"completionTokens":20}]"#,
        )];
        let path = temp_log("policy-retry", &lines);
        let complete_log = std::fs::read(&path).unwrap();
        let initial = run(&path);
        let mut context = initial.parse_context.unwrap();
        context
            .as_object_mut()
            .unwrap()
            .remove("scan_policy_version");
        context["revision"] = serde_json::json!(7);
        let legacy = StoredScanState {
            cursor: initial.cursor,
            parse_context: Some(context),
        };
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"bad json\n")
            .unwrap();
        let failed = scan(
            &target_for(&path),
            &legacy,
            &ScanLimits::default(),
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(failed.health, "degraded");
        assert!(failed.events.is_empty());
        let retry = StoredScanState {
            cursor: failed.cursor,
            parse_context: failed.parse_context,
        };
        assert!(
            should_scan_unchanged(&retry),
            "withheld snapshot still needs policy replay"
        );
        assert_eq!(retry.parse_context.as_ref().unwrap()["revision"], 7);
        std::fs::write(&path, complete_log).unwrap();
        let repaired = scan(
            &target_for(&path),
            &retry,
            &ScanLimits::default(),
            1_800_000_000_001,
        )
        .unwrap();
        assert_eq!(repaired.health, "active");
        assert_eq!(repaired.events.len(), 1);
        assert_eq!(repaired.events[0].source_revision, Some(8));
        assert!(!should_scan_unchanged(&StoredScanState {
            cursor: repaired.cursor,
            parse_context: repaired.parse_context
        }));
        let _ = std::fs::remove_file(path);
    }
}
