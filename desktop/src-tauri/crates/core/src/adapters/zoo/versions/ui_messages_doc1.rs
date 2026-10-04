//! Zoo ui_messages.json 格式实现（`ui_messages_doc1`，文档级
//! zoo-ui-messages-doc-1）。
//!
//! 格式依据（固定源码 f7806475331fcae5f4e8b5558d04415eeb5da88c，A19，
//! 按文档或源码实现，待真实样本核验；本机 not_found）：
//! - 路径：宿主 globalStorage 下 `tasks/<taskId>/ui_messages.json`
//!   （src/shared/globalFileNames.ts uiMessages；packages/core/src/
//!   task-persistence/taskMessages.ts：整文件 JSON **数组**，非 JSONL，
//!   消息删除/合并时整写重写）；VS Code 扩展身份
//!   ZooCodeOrganization.zoo-code（src/package.json publisher/name），
//!   CLI 缺省 `~/.vscode-mock/global-storage`（apps/cli task-history +
//!   vscode-shim paths）。
//! - usage 载体（consolidateTokenUsage.ts + consolidateApiRequests.ts）：
//!   `type=="say"` 且 `say ∈ {api_req_started, condense_context}`；
//!   api_req_started 的 `text` 为 JSON 字符串（tokensIn/tokensOut/
//!   cacheWrites/cacheReads/cost 逐字段可选 + apiProtocol ∈
//!   {anthropic, openai}），cost 仅在 consolidateApiRequests 把对应
//!   api_req_finished 的 text 合并进来后在场（LIFO 配对：finished 弹出最近
//!   未合并的 started，`{...startData, ...finishData}` finish 覆盖；无配对
//!   的 finished 被丢弃）；**tokensIn 存总输入（含缓存）**（两协议规则相同，
//!   固定源码注释），contextTokens = tokensIn + tokensOut 是上游自算的
//!   per-request 总量算术。
//! - condense_context：`contextCondense.cost` 计入上游 totalCost（固定源码）；
//!   无 token 字段（contextTokens 换用 newContextTokens 属上下文规模，
//!   不是用量）⇒ 按辅助调用入账（有 cost 映射 cost，token 全未知）。
//! - `ts` 是消息中唯一已确认的身份字段（epoch 毫秒数字）；数组下标会因删除
//!   移位，不进身份。
//!
//! fail closed（V17）：非 say 记录类型、未文档化 say 种类 ⇒ 整文件拒绝
//! （游标不推进、下轮确定性再拒）。真实文件中的 ask / say=text 等非用量
//! 消息未在固定源码中枚举，按未文档化处理，待真实样本扩展
//! （与 cline 适配器同一保守约定）。
//!
//! 增量语义（整写 JSON）：全量有界读取（32 MiB 初值）；游标存已消费字节数
//! 复用框架无变化短路；改写/截断走 generation 重扫，事件按稳定身份 upsert
//! 幂等；半程写入（parse 失败）不推进游标，下轮确定性重试。消息删除流程
//! 未在固定源码文档化：已入账事件保持，待真实样本核验（无墓碑推导）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;
use std::path::Path;

use super::super::common::{map_zoo_cost, map_zoo_usage, ZooUsage};
use super::ZOO_FORMAT_VERSION;

pub const ZOO_PARSER_VERSION: &str = "zoo-ui-messages-doc1";
/// 单文件有界读取上限（初值 32 MiB）。
pub const ZOO_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 固定源码文档化的 say 种类（两个 usage 载体 + 配对合并的 finished）。
const DOCUMENTED_SAY_KINDS: &[&str] = &["api_req_started", "api_req_finished", "condense_context"];
/// text JSON 的已文档化键（@example 的 request + ParsedApiReqStartedTextType 全键）。
const DOCUMENTED_TEXT_KEYS: &[&str] = &[
    "request",
    "tokensIn",
    "tokensOut",
    "cacheWrites",
    "cacheReads",
    "cost",
    "apiProtocol",
];

/// 持久化解析上下文：一次性诊断标志（重扫时重置）+ 版本选择依据。
/// Zoo 无 cline 的 deleted_api_reqs 文档化删除流程：不维护墓碑差分基。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ZooParseContext {
    #[serde(default)]
    without_numbers_reported: bool,
    #[serde(default)]
    unpaired_finished_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    /// 版本选择依据（known_version / latest_fallback）；zoo 固定为
    /// KnownVersion（文档级锚点）。
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// 整写 JSON 游标：offset=已消费字节数，line_number 恒 1；
/// 无变化短路依赖 probe.len == cursor.offset。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    line_number: u64,
}

/// restore：一次性标志在重扫时重置（重读重新报告）。
fn restore_context(stored: &StoredScanState, rescan: bool) -> ZooParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ZooParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan {
        ctx.without_numbers_reported = false;
        ctx.unpaired_finished_reported = false;
        ctx.unmapped_keys_reported = false;
    }
    ctx
}

fn diag(code: &str, field: Option<&str>, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

/// 任务目录名（tasks/<taskId>）作为会话身份。
fn task_id_of(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-task")
        .to_string()
}

/// 解析 text JSON 的 usage 四可选字段（i64 非负有界）与 cost。
/// 返回 None 表示 text 不是 JSON 对象或数值违例（记 shape 诊断跳过）。
/// 未文档化额外键返回 true。
fn parse_usage_text(
    text: &str,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<(ZooUsage, Option<f64>, bool)> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
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
    let usage = ZooUsage {
        tokens_in: get("tokensIn")?,
        tokens_out: get("tokensOut")?,
        cache_writes: get("cacheWrites")?,
        cache_reads: get("cacheReads")?,
    };
    let cost = match obj.get("cost") {
        None => None,
        Some(v) => match v.as_f64() {
            Some(c) if c.is_finite() && c >= 0.0 => Some(c),
            _ => {
                diagnostics.push(diag(
                    "cost_shape_deviation",
                    Some("cost"),
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
    Some((usage, cost, unknown))
}

/// 消息的 ts（epoch 毫秒）：缺失/非整数/越域返回 None（调用方跳过记诊断）。
fn message_ts(message: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    let ts = message.get("ts")?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ts)
        .then_some(ts)
}

/// 合并后的请求载体（api_req_started + 可选配对的 api_req_finished text）。
struct ConsolidatedRequest {
    text: Option<String>,
    ts: Option<i64>,
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    target: &ScanTarget,
    task_id: &str,
    say: &str,
    ts: i64,
    category: CallCategory,
    mapped: crate::adapters::usage_map::MappedUsage,
    cost: Option<crate::domain::CostAmount>,
    now_ms: i64,
) -> EventInput {
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("{task_id}:{say}:{ts}"),
        record_kind: RecordKind::ModelCall,
        schema_version: ZOO_FORMAT_VERSION.to_string(),
        parser_version: ZOO_PARSER_VERSION.to_string(),
        parse_basis: Some(VersionBasis::KnownVersion),
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(task_id.to_string()),
        parent_session_id: None,
        host_application: None,
        agent: "zoo-code".to_string(),
        call_category: category,
        occurred_at_ms: ts,
        observed_at_ms: Some(now_ms),
        source_time: Some(ts.to_string()),
        // api_req_started 的 ts 是请求起点（cost 由 finished 合并写回）；
        // condense_context 的 ts 是消息写入时刻，非调用起讫。
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

/// 增量扫描一个任务 ui_messages.json（统一入口 `ZooAdapter::scan` 分派）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = restore_context(stored, target.rescan);
    // 无版本字段可读：格式锚点是文档级 zoo-ui-messages-doc-1，固定 KnownVersion。
    context.version_basis = Some(VersionBasis::KnownVersion);
    let task_id = task_id_of(&target.path);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let cursor_at = |offset: u64| -> Result<serde_json::Value, CoreError> {
        Ok(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset,
            line_number: 1,
        })?)
    };
    // 超限：受限，游标停在起点，受控重试（不静默丢弃）。
    if target.probe.len > ZOO_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            None,
            "document",
            "ui_messages.json exceeds the 32 MiB cap; cursor held at start for controlled retry",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: Some(cursor_at(0)?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&target.path)?
        .take(ZOO_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > ZOO_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            None,
            "document",
            "ui_messages.json grew past the 32 MiB cap during read; cursor held at start",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: Some(cursor_at(0)?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let consumed = bytes.len() as u64;
    // 半程写入：parse 失败不推进游标，下轮确定性重试（暂态，非降级）。
    let document: serde_json::Value = match serde_json::from_slice(super::super::strip_bom(&bytes))
    {
        Ok(v) => v,
        Err(_) => {
            diagnostics.push(diag(
                "ui_messages_unparseable",
                None,
                "document",
                "ui_messages.json does not parse (mid-write or corrupt); cursor held for retry",
            ));
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events,
                aggregates: Vec::new(),
                diagnostics,
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let Some(messages) = document.as_array() else {
        diagnostics.push(diag(
            "session_schema_deviation",
            None,
            "document",
            "top-level value is not a JSON array of messages",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 1,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };

    // ---- 第一遍：consolidateApiRequests 配对（固定源码 LIFO 算法）----
    // started 进结果集并入栈；finished 弹出最近未合并的 started 合并 text
    // （{...startData, ...finishData}，finish 覆盖）；无配对的 finished 丢弃。
    let mut requests: Vec<ConsolidatedRequest> = Vec::new();
    let mut condenses: Vec<(Option<i64>, Option<f64>)> = Vec::new();
    let mut open_started: Vec<usize> = Vec::new();
    let mut records_seen: u64 = 0;
    for (index, message) in messages.iter().enumerate() {
        records_seen += 1;
        let position = format!("messages[{index}]");
        let fail_closed = |diagnostics: &mut Vec<DiagnosticInput>,
                           code: &'static str,
                           detail: String|
         -> ScanOutcome {
            diagnostics.push(diag(code, None, &position, &detail));
            ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: std::mem::take(diagnostics),
                lines_read: 1,
                records_seen,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            }
        };
        let Some(message_obj) = message.as_object() else {
            let outcome = fail_closed(
                &mut diagnostics,
                "session_schema_deviation",
                format!("{position} is not an object"),
            );
            return Ok(outcome);
        };
        let record_type = message_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if record_type != "say" {
            // 固定源码只处理 type="say"；其余记录类型未文档化 ⇒ fail closed。
            let outcome = fail_closed(
                &mut diagnostics,
                "undocumented_record_type",
                format!("record type {record_type:?} not in documented set (say)"),
            );
            return Ok(outcome);
        }
        let say_kind = message_obj
            .get("say")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if !DOCUMENTED_SAY_KINDS.contains(&say_kind) {
            let outcome = fail_closed(
                &mut diagnostics,
                "undocumented_say_kind",
                format!("say kind {say_kind:?} not in documented set"),
            );
            return Ok(outcome);
        }
        match say_kind {
            "api_req_started" => {
                requests.push(ConsolidatedRequest {
                    text: message_obj
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(str::to_string),
                    ts: message_ts(message_obj),
                });
                open_started.push(requests.len() - 1);
            }
            "api_req_finished" => {
                match open_started.pop() {
                    Some(start_index) => {
                        let started = &mut requests[start_index];
                        let parse_obj =
                            |text: Option<&str>| -> serde_json::Map<String, serde_json::Value> {
                                text.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
                                    .and_then(|v| v.as_object().cloned())
                                    .unwrap_or_default()
                            };
                        let mut merged = parse_obj(started.text.as_deref());
                        for (key, value) in
                            parse_obj(message_obj.get("text").and_then(|t| t.as_str()))
                        {
                            merged.insert(key, value);
                        }
                        started.text = Some(serde_json::to_string(&merged)?);
                    }
                    None => {
                        // 固定源码：无配对的 finished 不进合并结果 ⇒ 不产事件。
                        if !context.unpaired_finished_reported {
                            context.unpaired_finished_reported = true;
                            diagnostics.push(diag(
                                "unpaired_finished_dropped",
                                Some("say"),
                                &position,
                                "api_req_finished without open api_req_started; dropped per consolidateApiRequests",
                            ));
                        }
                    }
                }
            }
            "condense_context" => {
                // 固定源码：contextCondense.cost 计入 totalCost；无 token 字段。
                let cost = message_obj
                    .get("contextCondense")
                    .and_then(|c| c.get("cost"))
                    .and_then(|c| c.as_f64())
                    .filter(|c| c.is_finite() && *c >= 0.0);
                condenses.push((message_ts(message_obj), cost));
            }
            _ => unreachable!("say kind 已在文档化集合内校验"),
        }
    }

    // ---- 第二遍：consolidateTokenUsage 计账语义 ----
    for request in &requests {
        let Some(text) = request.text.as_deref() else {
            // usage 载体无 text：上游短路不读；未记录用量，不产事件。
            continue;
        };
        let position = format!("{}:api_req_started", task_id);
        let Some((usage, cost, unknown_keys)) = parse_usage_text(text, &position, &mut diagnostics)
        else {
            diagnostics.push(diag(
                "usage_shape_deviation",
                Some("text"),
                &position,
                "usage text not a JSON object or a value is negative/non-integer/out of range; record skipped",
            ));
            continue;
        };
        if unknown_keys && !context.unmapped_keys_reported {
            context.unmapped_keys_reported = true;
            diagnostics.push(diag(
                "unmapped_usage_keys",
                Some("text"),
                &position,
                "usage text carries keys beyond the documented set; mapped fields kept",
            ));
        }
        let Some(ts) = request.ts else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("ts"),
                &position,
                "message ts missing/implausible; record skipped",
            ));
            continue;
        };
        if usage.is_empty() {
            // 未配对 finished 的 started：载体无 usage 数字 ⇒ 未记录 token
            // 不产事件（一次性诊断；cost 单独无 token 不入账，保持同源）。
            if !context.without_numbers_reported {
                context.without_numbers_reported = true;
                diagnostics.push(diag(
                    "usage_carrier_without_numbers",
                    Some("text"),
                    &position,
                    "usage carrier without token numbers (unfinished request?); no event",
                ));
            }
            continue;
        }
        let mapped = map_zoo_usage(&usage);
        events.push(build_event(
            target,
            &task_id,
            "api_req_started",
            ts,
            CallCategory::Primary,
            mapped,
            map_zoo_cost(cost),
            now_ms,
        ));
    }
    for (ts, cost) in &condenses {
        let position = format!("{}:condense_context", task_id);
        let Some(ts) = ts else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("ts"),
                &position,
                "condense_context ts missing/implausible; record skipped",
            ));
            continue;
        };
        // condense（上下文压缩摘要）表明发生过一次辅助调用：计调用、token 全
        // 未知（固定源码只有 cost 贡献），cost 有则映射（estimated）。
        events.push(build_event(
            target,
            &task_id,
            "condense_context",
            *ts,
            CallCategory::Auxiliary,
            crate::adapters::usage_map::finish(
                crate::domain::TokenUsage::default(),
                crate::domain::TokenQuality::default(),
                Vec::new(),
            ),
            map_zoo_cost(*cost),
            now_ms,
        ));
    }

    let health_degraded = diagnostics
        .iter()
        .any(|d| d.code == "usage_shape_deviation");
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(cursor_at(consumed)?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
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
    fn parse_usage_text_optional_fields_and_protocol_key() {
        let mut diags = Vec::new();
        let (usage, cost, unknown) = parse_usage_text(
            r#"{"request":"syn","tokensIn":100,"tokensOut":20,"cacheWrites":10,"cacheReads":40,"cost":0.005,"apiProtocol":"anthropic"}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.tokens_in, Some(100));
        assert_eq!(usage.cache_reads, Some(40));
        assert_eq!(cost, Some(0.005));
        assert!(!unknown, "apiProtocol 在固定源码类型内");

        let (usage, cost, _) = parse_usage_text(
            r#"{"tokensIn":10,"tokensOut":2}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.cache_writes, None, "缺失保持未知");
        assert_eq!(cost, None);

        assert!(parse_usage_text("not json", "messages[0]", &mut diags).is_none());
        assert!(
            parse_usage_text(r#"{"tokensIn":-1}"#, "messages[0]", &mut diags).is_none(),
            "负值拒绝"
        );
        let extra =
            parse_usage_text(r#"{"tokensIn":1,"surprise":2}"#, "messages[0]", &mut diags).unwrap();
        assert!(extra.2, "未文档化键报告，已映射字段保留");
    }

    #[test]
    fn task_id_from_path() {
        let p = Path::new("/home/u/.vscode-mock/global-storage/tasks/syn-zoo-1/ui_messages.json");
        assert_eq!(task_id_of(p), "syn-zoo-1".to_string());
    }
}
