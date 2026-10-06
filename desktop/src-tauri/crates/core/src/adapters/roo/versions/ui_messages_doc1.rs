//! Roo Code ui_messages.json 格式实现（`ui_messages_doc1`，文档级
//! roo-ui-messages-doc-1）。
//!
//! 格式依据（RooCodeInc/Roo-Code 固定源码 b867ec9145750d0ae1ff7f02d35406e9bf2a0b16，
//! 官方源码核验；另有末版 v3.54.0/27001b2b 官方 VSIX 的真实 extension-host/API
//! 默认与单调用对照，2026-10-06）：
//! - 路径：VS Code globalStorage `RooVeterinaryInc.roo-cline/tasks/<taskId>/
//!   ui_messages.json`（src/package.json publisher.name；storage.ts:53-57）；
//!   CLI `~/.vscode-mock/global-storage/tasks/`（vscode-shim paths）；
//!   `roo-code.customStoragePath` 可把任务移出 globalStorage（手工根覆盖）；
//!   .vscode-server 远端变体按 VS Code 机制（源码未见显式处理）。
//! - ui_messages 数组元素 = ClineMessage（packages/types/src/message.ts:249-279）：
//!   `ts`（毫秒）、`type ∈ {ask, say}`、`say`/`ask`、`text`、`contextCondense`。
//! - **api_req_started 的 text 为 JSON**（ClineApiReqInfo，vscode-extension-host.ts:
//!   780-790）：`tokensIn/tokensOut/cacheWrites/cacheReads/cost` 逐字段可选 +
//!   `request`/`cancelReason`/`streamingFailedMessage`/`apiProtocol`。
//!   **tokensIn 恒为含缓存的总输入**（三处源码核对结果：Task.ts:2662
//!   tokensIn=totalInputTokens；cost.ts:64-79/91-111 两协议注释；
//!   consolidateTokenUsage.ts:81-92 "no longer need to add cache separately"）——
//!   cacheReads/cacheWrites 是其子集，**不得再相加**（与 cline 适配器的
//!   四桶互斥关系不同血统分歧，见 adapters.md）。
//! - api_req_started/api_req_finished LIFO 配对（consolidateApiRequests.ts:50-85，
//!   finish 覆盖同名字段）；当前版本后端不再写 finished（Task.ts:2615-2621，
//!   legacy）。**api_req_deleted**（checkpoint 恢复时写入，checkpoints/index.ts:
//!   234-289）与旧 Cline **deleted_api_reqs** 是已扣除量备忘：**不计入**
//!   （consolidateTokenUsage 不统计，上游计算时自动扣除）。
//! - condense_context：`contextCondense.cost` 是压缩摘要独立调用的总成本
//!   （condense/index.ts:215），不走请求循环 ⇒ 按辅助调用入账（token 未知）；
//!   sliding_window_truncation 只有 token 计数无 cost，不入账。
//! - subtask：子任务独立目录独立计费，父任务 subtask_result 只是文本摘要；
//!   按任务去重不双计。
//! - 完整 say/ask 集合已在固定源码枚举（message.ts:27-40/144-172）：
//!   未列出的记录类型 fail closed（V17）。
//!
//! 四桶先初始化零；OpenAI-compatible 忽略 prompt_tokens_details.cached_tokens。
//! 因此零桶未知，只有正缓存子集均已知才推导未缓存；默认零费用亦未知。
//! 取消可能删除最后占位，默认实测 API 三次、原生两次，不补缺失调用。
//! 映射：input_total=tokensIn（含缓存）、cache 子集并列、input_uncached=
//! tokensIn−cacheWrites−cacheReads（派生，sub_checked 防负）、output=tokensOut、
//! total=tokensIn+tokensOut（派生）；cost=扩展自算（estimated micro-USD）。
//! 增量：整写 JSON 32 MiB 上限，字节游标 + generation 重扫，事件键
//! {taskId}:{say}:{ts} upsert 幂等。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use crate::metrics::Contradiction;
use std::io::Read;

use super::ROO_FORMAT_VERSION;

pub const ROO_PARSER_VERSION: &str = "roo-ui-messages-doc2";
pub const ROO_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 固定源码完整枚举的 type 集合。
const DOCUMENTED_TYPES: &[&str] = &["ask", "say"];
/// 固定源码完整枚举的 say 集合（message.ts:144-172）。
const DOCUMENTED_SAY_KINDS: &[&str] = &[
    "error",
    "api_req_started",
    "api_req_finished",
    "api_req_retried",
    "api_req_retry_delayed",
    "api_req_rate_limit_wait",
    "api_req_deleted",
    "text",
    "image",
    "reasoning",
    "completion_result",
    "user_feedback",
    "user_feedback_diff",
    "command_output",
    "shell_integration_warning",
    "mcp_server_request_started",
    "mcp_server_response",
    "subtask_result",
    "checkpoint_saved",
    "rooignore_error",
    "diff_error",
    "condense_context",
    "condense_context_error",
    "sliding_window_truncation",
    "codebase_search_result",
    "user_edit_todos",
    "too_many_tools_warning",
    "tool",
];
/// 固定源码完整枚举的 ask 集合（message.ts:27-40）。
const DOCUMENTED_ASK_KINDS: &[&str] = &[
    "followup",
    "command",
    "command_output",
    "completion_result",
    "tool",
    "api_req_failed",
    "resume_task",
    "resume_completed_task",
    "mistake_limit_reached",
    "use_mcp_server",
    "auto_approval_max_req_reached",
];
/// 已扣除量备忘（不计入）：Roo api_req_deleted + 旧 Cline deleted_api_reqs。
const MEMO_KINDS: &[&str] = &["api_req_deleted", "deleted_api_reqs"];
/// api_req_started text JSON 的已文档化键。
const DOCUMENTED_TEXT_KEYS: &[&str] = &[
    "request",
    "tokensIn",
    "tokensOut",
    "cacheWrites",
    "cacheReads",
    "cost",
    "cancelReason",
    "streamingFailedMessage",
    "apiProtocol",
];

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct RooParseContext {
    #[serde(default)]
    without_numbers_reported: bool,
    #[serde(default)]
    memo_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
}

fn restore_context(stored: &StoredScanState, rescan: bool) -> RooParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<RooParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan {
        ctx = RooParseContext::default();
    }
    ctx
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

fn task_id_of(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-task")
        .to_string()
}

fn usd_cost(value: Option<f64>) -> Option<CostAmount> {
    let amount = value?;
    if !amount.is_finite() || amount <= 0.0 {
        return None;
    }
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        // 扩展按费率自算（cost.ts）：参考估算，非账单。
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    target: &ScanTarget,
    task_id: &str,
    say: &str,
    ts: i64,
    // 同任务同毫秒可有多条同类事件（retry/子任务并行）：键含序号防 upsert 吞并。
    // 整文件重扫序号确定，跨轮稳定（幂等）。
    seq: usize,
    category: CallCategory,
    mapped: crate::adapters::usage_map::MappedUsage,
    cost: Option<CostAmount>,
    now_ms: i64,
) -> EventInput {
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("{task_id}:{say}:{ts}:{seq}"),
        record_kind: RecordKind::ModelCall,
        schema_version: ROO_FORMAT_VERSION.to_string(),
        parser_version: ROO_PARSER_VERSION.to_string(),
        parse_basis: Some(VersionBasis::KnownVersion),
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(task_id.to_string()),
        parent_session_id: None,
        host_application: None,
        agent: "roo-code".to_string(),
        call_category: category,
        occurred_at_ms: ts,
        observed_at_ms: Some(now_ms),
        source_time: Some(ts.to_string()),
        time_basis: if say == "condense_context" {
            TimeBasis::Uncertain
        } else {
            TimeBasis::SourceStart
        },
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: None,
        model_canonical: None,
        model_attribution: ModelAttribution::Unknown,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost,
    }
}

/// 解析 api_req_started text JSON：tokensIn（含缓存总输入）+ 子集桶 + cost。
/// 四桶（tokensIn/tokensOut/cacheWrites/cacheReads，缺省=未知）。
type RooUsageBuckets = (Option<i64>, Option<i64>, Option<i64>, Option<i64>);

fn parse_usage_text(
    text: &str,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<(RooUsageBuckets, Option<f64>, bool)> {
    let value: serde_json::Value = crate::adapters::run_policy::json_from_str(text).ok()?;
    let obj = value.as_object()?;
    let get = |key: &str| -> Option<Option<i64>> {
        match obj.get(key) {
            None => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                Some((0..=MAX_REASONABLE_TOKEN).contains(&n).then_some(n))
            }
        }
    };
    let tokens_in = get("tokensIn")?;
    let tokens_out = get("tokensOut")?;
    let cache_writes = get("cacheWrites")?;
    let cache_reads = get("cacheReads")?;
    let cost = match obj.get("cost") {
        None => None,
        Some(v) => match v.as_f64() {
            Some(c) if c.is_finite() && c >= 0.0 => Some(c),
            _ => {
                diagnostics.push(diag(
                    "cost_shape_deviation",
                    position,
                    "cost not a finite non-negative number; cost left unknown",
                ));
                None
            }
        },
    };
    let unknown = obj
        .keys()
        .any(|k| !DOCUMENTED_TEXT_KEYS.contains(&k.as_str()));
    Some((
        (tokens_in, tokens_out, cache_writes, cache_reads),
        cost,
        unknown,
    ))
}

/// Roo 字段语义：tokensIn 含缓存 ⇒ input_uncached = tokensIn − writes − reads（派生）。
fn map_roo_usage(
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    cache_writes: Option<i64>,
    cache_reads: Option<i64>,
) -> crate::adapters::usage_map::MappedUsage {
    let mut contradictions: Vec<Contradiction> = Vec::new();
    let uncached = match (tokens_in, cache_writes, cache_reads) {
        (Some(total), Some(w), Some(r)) => crate::adapters::usage_map::sub_checked(
            "input_uncached",
            total,
            w.saturating_add(r),
            &mut contradictions,
        ),
        _ => None,
    };
    let input_total = tokens_in;
    let total = match (input_total, tokens_out) {
        (Some(i), Some(o)) => i.checked_add(o),
        _ => None,
    };
    let usage = crate::domain::TokenUsage {
        input_uncached: uncached,
        input_cache_read: cache_reads,
        input_cache_write: cache_writes,
        input_total,
        output_total: tokens_out,
        output_reasoning: None,
        total_tokens: total,
        source_total: None,
    };
    let quality = crate::domain::TokenQuality {
        input_uncached: if uncached.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        input_cache_read: cache_reads
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_cache_write: cache_writes
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        input_total: input_total
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        output_total: tokens_out
            .map(|_| crate::domain::FieldQuality::Reported)
            .unwrap_or(crate::domain::FieldQuality::Unknown),
        total_tokens: if total.is_some() {
            crate::domain::FieldQuality::Derived
        } else {
            crate::domain::FieldQuality::Unknown
        },
        ..Default::default()
    };
    crate::adapters::usage_map::finish(usage, quality, contradictions)
}

fn message_ts(message: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    let ts = message.get("ts")?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ts)
        .then_some(ts)
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = restore_context(stored, target.rescan);
    context.version_basis = Some(VersionBasis::KnownVersion);
    let task_id = task_id_of(&target.path);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    if target.probe.len > ROO_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "ui_messages.json exceeds the 32 MiB cap; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    crate::adapters::run_policy::checked_file(&target.path)?
        .take(ROO_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > ROO_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "ui_messages.json grew past the cap during read; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let document: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events,
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "ui_messages_unparseable",
                    "document",
                    "ui_messages.json does not parse (mid-write or corrupt); retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let Some(messages) = document.as_array() else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "session_schema_deviation",
                "document",
                "top-level value is not a JSON array of messages",
            )],
            lines_read: 1,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };

    // LIFO 配对（consolidateApiRequests 同款）：started 入栈；finished 弹出
    // 最近 started 合并 text（finish 覆盖）。
    struct Started {
        text: Option<String>,
        ts: Option<i64>,
    }
    let mut requests: Vec<Started> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut condenses: Vec<(Option<i64>, Option<f64>)> = Vec::new();
    let mut records_seen: u64 = 0;
    for (index, message) in messages.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let position = format!("messages[{index}]");
        let fail_closed = |diagnostics: &mut Vec<DiagnosticInput>,
                           code: &'static str,
                           detail: String|
         -> ScanOutcome {
            let mut d = std::mem::take(diagnostics);
            d.push(diag(code, &position, &detail));
            ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: d,
                lines_read: 1,
                records_seen,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            }
        };
        let Some(message_obj) = message.as_object() else {
            return Ok(fail_closed(
                &mut diagnostics,
                "session_schema_deviation",
                format!("{position} is not an object"),
            ));
        };
        let record_type = message_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if !DOCUMENTED_TYPES.contains(&record_type) {
            return Ok(fail_closed(
                &mut diagnostics,
                "undocumented_record_type",
                format!("record type {record_type:?} not in documented set (ask/say)"),
            ));
        }
        let kind = message_obj
            .get(record_type)
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let documented = match record_type {
            "say" => DOCUMENTED_SAY_KINDS.contains(&kind),
            _ => DOCUMENTED_ASK_KINDS.contains(&kind),
        };
        // 旧 Cline 血统的 deleted_api_reqs 备忘也按备忘处理（不入账）。
        if !documented && !MEMO_KINDS.contains(&kind) {
            return Ok(fail_closed(
                &mut diagnostics,
                "undocumented_kind",
                format!("{record_type} kind {kind:?} not in the documented set"),
            ));
        }
        if MEMO_KINDS.contains(&kind) {
            // 已扣除量备忘：上游不统计，本适配器也按该规则排除。
            if !context.memo_reported {
                context.memo_reported = true;
                diagnostics.push(diag(
                    "deleted_request_memo_skipped",
                    &position,
                    "api_req_deleted/deleted_api_reqs is a memo of removed usage; not counted (upstream semantics)",
                ));
            }
            continue;
        }
        if record_type != "say" {
            continue;
        }
        match kind {
            "api_req_started" => {
                requests.push(Started {
                    text: message_obj
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(str::to_string),
                    ts: message_ts(message_obj),
                });
                open.push(requests.len() - 1);
            }
            "api_req_finished" => {
                if let Some(start_index) = open.pop() {
                    let parse_obj =
                        |text: Option<&str>| -> serde_json::Map<String, serde_json::Value> {
                            text.and_then(|t| {
                                crate::adapters::run_policy::json_from_str::<serde_json::Value>(t)
                                    .ok()
                            })
                            .and_then(|v| v.as_object().cloned())
                            .unwrap_or_default()
                        };
                    let started = &mut requests[start_index];
                    let mut merged = parse_obj(started.text.as_deref());
                    for (key, value) in parse_obj(message_obj.get("text").and_then(|t| t.as_str()))
                    {
                        merged.insert(key, value);
                    }
                    started.text = Some(serde_json::to_string(&merged)?);
                }
            }
            "condense_context" => {
                let cost = message_obj
                    .get("contextCondense")
                    .and_then(|c| c.get("cost"))
                    .and_then(|c| c.as_f64())
                    .filter(|c| c.is_finite() && *c >= 0.0);
                condenses.push((message_ts(message_obj), cost));
            }
            _ => {}
        }
    }

    for (req_seq, request) in requests.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        let Some(text) = request.text.as_deref() else {
            continue;
        };
        let position = format!("{task_id}:api_req_started");
        let Some(((tokens_in, tokens_out, cache_writes, cache_reads), cost, unknown_keys)) =
            parse_usage_text(text, &position, &mut diagnostics)
        else {
            diagnostics.push(diag(
                "usage_shape_deviation",
                &position,
                "usage text not a JSON object or a value is out of range; record skipped",
            ));
            continue;
        };
        if unknown_keys && !context.unmapped_keys_reported {
            context.unmapped_keys_reported = true;
            diagnostics.push(diag(
                "unmapped_usage_keys",
                &position,
                "usage text carries keys beyond the documented set; mapped fields kept",
            ));
        }
        let Some(ts) = request.ts else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &position,
                "message ts missing/implausible; record skipped",
            ));
            continue;
        };
        if tokens_in.is_none()
            && tokens_out.is_none()
            && cache_writes.is_none()
            && cache_reads.is_none()
        {
            // 占位/未完成请求：无 token 数字 ⇒ 不产事件（上游恢复时 splice 删除）。
            if !context.without_numbers_reported {
                context.without_numbers_reported = true;
                diagnostics.push(diag(
                    "usage_carrier_without_numbers",
                    &position,
                    "api_req_started without token numbers (interrupted request?); no event",
                ));
            }
            continue;
        }
        let positive = |value: Option<i64>| value.filter(|v| *v > 0);
        let mapped = map_roo_usage(
            positive(tokens_in),
            positive(tokens_out),
            positive(cache_writes),
            positive(cache_reads),
        );
        events.push(build_event(
            target,
            &task_id,
            "api_req_started",
            ts,
            req_seq,
            CallCategory::Primary,
            mapped,
            usd_cost(cost),
            now_ms,
        ));
    }
    for (condense_seq, (ts, cost)) in condenses.iter().enumerate() {
        crate::adapters::run_policy::check()?;
        let position = format!("{task_id}:condense_context");
        let Some(ts) = ts else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &position,
                "condense_context ts missing/implausible; record skipped",
            ));
            continue;
        };
        // 压缩摘要独立调用：cost 有则映射（estimated），token 全未知。
        events.push(build_event(
            target,
            &task_id,
            "condense_context",
            *ts,
            condense_seq,
            CallCategory::Auxiliary,
            crate::adapters::usage_map::finish(
                crate::domain::TokenUsage::default(),
                crate::domain::TokenQuality::default(),
                Vec::new(),
            ),
            usd_cost(*cost),
            now_ms,
        ));
    }
    let health = if diagnostics
        .iter()
        .any(|d| d.code == "usage_shape_deviation")
    {
        "degraded".to_string()
    } else {
        "active".to_string()
    };
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations: Vec::new(),
        health,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_in_includes_cache() {
        let mapped = map_roo_usage(Some(100), Some(20), Some(10), Some(30));
        // tokensIn=100 含缓存：uncached = 100-10-30 = 60。
        assert_eq!(mapped.usage.input_total, Some(100));
        assert_eq!(mapped.usage.input_uncached, Some(60));
        assert_eq!(mapped.usage.total_tokens, Some(120));
        assert_eq!(mapped.usage.input_cache_read, Some(30));
    }

    #[test]
    fn negative_derived_reports_contradiction() {
        let mapped = map_roo_usage(Some(10), Some(5), Some(8), Some(8));
        assert_eq!(mapped.usage.input_uncached, None);
        assert!(mapped
            .diagnostics
            .iter()
            .any(|d| d.code == "negative_derived_field"));
    }

    #[test]
    fn usage_text_optional_fields() {
        let mut diags = Vec::new();
        let ((tokens_in, _tokens_out, writes, _reads), cost, unknown) = parse_usage_text(
            r#"{"tokensIn":9,"tokensOut":2,"cacheReads":4,"cost":0.01,"apiProtocol":"anthropic"}"#,
            "p",
            &mut diags,
        )
        .unwrap();
        assert_eq!(tokens_in, Some(9));
        assert_eq!(writes, None);
        assert_eq!(cost, Some(0.01));
        assert!(!unknown);
    }
}
