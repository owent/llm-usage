//! DSH 持久会话日志 JSONL 格式实现（`session_log_doc1`，文档级
//! session-log-doc-1）。
//!
//! 格式证据（固定 token-meter README 46a7f68b0922371ce7144b668b90e377d8e799f4，
//! A08，文档级证据待真实样本；本机 not_found）：
//! - 事件词汇（README 枚举）：`step/start`、`assistant/message`、
//!   `llm/retry-started`、`request/context`、`request/header`、`image/offload`；
//!   usage 样本挂在 assistant/message 上，字段
//!   uncachedInputTokens/outputTokens/cacheReadTokens/cacheWriteTokens 各自可选。
//! - 替换语义："A final assistant-message sample replaces streaming usage from
//!   the same attempt; `llm/retry-started` ends that replacement scope, so a
//!   retry in the same step contributes another billed attempt"；
//!   "Usage folds replace samples within each attempt; totals need not be
//!   monotone"。
//! - 估算排除：contextPressure（pressureTokens/projectedTokens/contextWindow）
//!   与 contextBreakdown（systemTokens/toolsTokens/messageTokens）"are
//!   estimates … not its provider-billed size"，不进入用量。
//! - 落盘路径与行序列化未文档化：JSONL 行形状为合成假设（fixtures 标
//!   synthetic），真实样本到达后核验。
//!
//! 折叠/仲裁机制（复用既有 ingest 仲裁，不发明新机制）：
//! - attempt 键 = `{file_identity}:s{step}:a{attempt}`（step 由 step/start 计数、
//!   attempt 由 llm/retry-started 在 step 内计数；确定性重放）；
//! - 同 attempt 每个样本立即入账（lifecycle=Partial，source_revision=样本序号）
//!   ——样本序号 + 跨轮单调 revision_floor 保证后续样本按"更高修订号 Replace"
//!   撤销旧贡献（V03 样本 7 语义），重扫重放不产生同级内容冲突；
//! - attempt 边界（llm/retry-started / step/start）到达时对最后一个样本补发
//!   Final（更高修订号 Replace 收口）；日志尾部未闭合 attempt 保持 Partial
//!   （值正确，生命周期证据缺失）；
//! - 重扫后消失的 attempt 键发射 Corrected/Excluded 墓碑（防截断改写双计）；
//!   revision_floor 与 attempt 键集跨重扫保留，折叠计数器重置。
//!
//! fail closed（V17）：未文档化事件 type ⇒ 整文件拒绝（游标不推进、下轮
//! 确定性再拒）。usage 四字段值违例（负/非整数/超限）逐条跳过记诊断（部分可用）。
//! pinned README 未记载逐事件时间字段：occurred_at 用观察时间（observed_at 口径）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::super::common::{map_dsh_usage, DshUsage};
use super::DSH_FORMAT_VERSION;

pub const DSH_PARSER_VERSION: &str = "dsh-session-log-doc1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// README 枚举的事件类型（detect 与扫描共用同一接受集）。
pub const DOCUMENTED_EVENT_TYPES: &[&str] = &[
    "step/start",
    "assistant/message",
    "llm/retry-started",
    "request/context",
    "request/header",
    "image/offload",
];

/// 待收口样本（attempt 打开期间的最后一个已入账样本，跨轮持久化）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct PendingSample {
    revision: i64,
    usage: DshUsage,
}

/// 持久化折叠状态。重扫语义：折叠计数器（step/attempt/ordinal/pending）重置
/// 从头重折（确定性重放）；revision_floor 与 tracked_keys 跨重扫保留——
/// 前者防重放产生同级内容冲突，后者是消失 attempt 的墓碑差分基。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct DshParseContext {
    /// 已发射的最大 source_revision（跨轮单调）。
    #[serde(default)]
    revision_floor: i64,
    /// 上一轮入账的 attempt 键（墓碑差分基，重扫不重置）。
    #[serde(default)]
    tracked_keys: Vec<String>,
    /// 已见 step/start 数（0 = 尚无 step）。
    #[serde(default)]
    step_index: u64,
    /// 当前 step 内已见 llm/retry-started 数（attempt 序号）。
    #[serde(default)]
    attempt_index: u64,
    /// 当前 attempt 已发射样本数 + 1（下一个样本序号）。
    #[serde(default)]
    sample_ordinal: i64,
    /// 打开 attempt 的最后样本（边界事件收口用）。
    #[serde(default)]
    pending: Option<PendingSample>,
    #[serde(default)]
    without_usage_reported: bool,
    #[serde(default)]
    estimate_excluded_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    /// 版本选择依据；dsh 固定为 KnownVersion（文档级锚点）。
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

impl DshParseContext {
    /// 重扫：重置折叠位置状态，保留差分基与修订号地板。
    fn reset_fold(&mut self) {
        self.step_index = 0;
        self.attempt_index = 0;
        self.sample_ordinal = 0;
        self.pending = None;
        self.without_usage_reported = false;
        self.estimate_excluded_reported = false;
        self.unmapped_keys_reported = false;
    }

    fn attempt_key(&self, file_identity: &str) -> String {
        format!(
            "{file_identity}:s{}:a{}",
            self.step_index, self.attempt_index
        )
    }
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> DshParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<DshParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan {
        ctx.reset_fold();
    }
    ctx
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

/// usage 对象四可选字段（i64 非负有界）；非对象返回 None；
/// 未文档化额外键返回 true（保留已映射字段，一次性诊断）。
const USAGE_KEYS: &[&str] = &[
    "uncachedInputTokens",
    "outputTokens",
    "cacheReadTokens",
    "cacheWriteTokens",
];

fn parse_usage(value: &serde_json::Value) -> Option<(DshUsage, bool)> {
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
    let usage = DshUsage {
        uncached_input_tokens: get("uncachedInputTokens")?,
        output_tokens: get("outputTokens")?,
        cache_read_tokens: get("cacheReadTokens")?,
        cache_write_tokens: get("cacheWriteTokens")?,
    };
    let unknown = obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str()));
    Some((usage, unknown))
}

/// 增量扫描一个持久会话日志（统一入口 `DshAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // 无版本字段可读：格式锚点是文档级 session-log-doc-1，固定 KnownVersion。
    context.version_basis = Some(VersionBasis::KnownVersion);
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String)> = None;
    // 本轮折叠结果中的 attempt 键集：增量续读从上一轮累计集出发（append-only
    // 下旧 attempt 不因无新行而"消失"）；重扫从头重折则从空集出发
    // （差分 = 上一轮累计集 - 本轮完整折叠结果）。
    let mut current_keys: Vec<String> = if target.rescan {
        Vec::new()
    } else {
        context.tracked_keys.clone()
    };

    // 构造一个 usage 样本事件（streaming 收 Partial；收口样本 Final）。
    let sample_event = |target: &ScanTarget,
                        context: &DshParseContext,
                        key: &str,
                        usage: &DshUsage,
                        revision: i64,
                        lifecycle: Lifecycle|
     -> EventInput {
        let mapped = map_dsh_usage(usage);
        EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: key.to_string(),
            record_kind: RecordKind::ModelCall,
            schema_version: DSH_FORMAT_VERSION.to_string(),
            parser_version: DSH_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: Some(format!(
                "s{}:a{}",
                context.step_index, context.attempt_index
            )),
            session_id: None,
            parent_session_id: None,
            host_application: None,
            agent: "deepseek-harness".to_string(),
            call_category: CallCategory::Primary,
            // pinned README 未记载逐事件时间字段：观察时间口径，日归属受限。
            occurred_at_ms: now_ms,
            observed_at_ms: Some(now_ms),
            source_time: None,
            time_basis: TimeBasis::ObservedAt,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: None,
            model_canonical: None,
            model_attribution: ModelAttribution::Unknown,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle,
            source_revision: Some(revision),
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        }
    };

    // attempt 边界收口：对打开 attempt 的最后样本补发 Final（更高修订号
    // Replace 收口）；无待收口样本则只推进边界。
    macro_rules! finalize_attempt {
        () => {
            if let Some(pending) = context.pending.take() {
                let key = context.attempt_key(&target.file_identity);
                let revision = context.revision_floor + 1;
                context.revision_floor = revision;
                events.push(sample_event(
                    target,
                    &context,
                    &key,
                    &pending.usage,
                    revision,
                    Lifecycle::Final,
                ));
            }
        };
    }

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
        let event_type = line.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !DOCUMENTED_EVENT_TYPES.contains(&event_type) {
            fail_closed = Some((
                raw.number,
                format!("event type {event_type:?} not in documented set"),
            ));
            break;
        }
        match event_type {
            "step/start" => {
                // 步边界：收口当前 attempt，进入新 step（attempt 序号归零）。
                finalize_attempt!();
                context.step_index += 1;
                context.attempt_index = 0;
                context.sample_ordinal = 0;
            }
            "llm/retry-started" => {
                // retry 边界：结束替换范围（收口当前 attempt），同 step 新开
                // attempt（新键，独立计费）。
                finalize_attempt!();
                context.attempt_index += 1;
                context.sample_ordinal = 0;
            }
            "assistant/message" => {
                let usage_value = line.get("usage");
                let Some(usage_value) = usage_value else {
                    // 无 usage 的 assistant/message：无用量证据，不产事件
                    // （一次性诊断；部分可用）。
                    if !context.without_usage_reported {
                        context.without_usage_reported = true;
                        diagnostics.push(diag(
                            "assistant_message_without_usage",
                            Some("usage"),
                            raw.number,
                            "assistant/message without usage object; no usage evidence, no event",
                        ));
                    }
                    continue;
                };
                let Some((usage, unknown_keys)) = parse_usage(usage_value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("usage"),
                        raw.number,
                        "usage value not an object or a field is negative/non-integer/out of range; record skipped",
                    ));
                    continue;
                };
                if usage.is_empty() {
                    if !context.without_usage_reported {
                        context.without_usage_reported = true;
                        diagnostics.push(diag(
                            "assistant_message_without_usage",
                            Some("usage"),
                            raw.number,
                            "usage object without any of the four documented fields; no event",
                        ));
                    }
                    continue;
                }
                if unknown_keys && !context.unmapped_keys_reported {
                    context.unmapped_keys_reported = true;
                    diagnostics.push(diag(
                        "unmapped_usage_keys",
                        Some("usage"),
                        raw.number,
                        "usage object carries keys beyond the documented four; mapped fields kept",
                    ));
                }
                // 同 attempt 的流式/最终样本：立即入账 Partial，样本序号 +
                // revision_floor 构成单调修订号 ⇒ 后续样本 Replace 撤销旧贡献。
                context.sample_ordinal += 1;
                let revision = context.revision_floor + context.sample_ordinal;
                let key = context.attempt_key(&target.file_identity);
                if !current_keys.contains(&key) {
                    current_keys.push(key.clone());
                }
                events.push(sample_event(
                    target,
                    &context,
                    &key,
                    &usage,
                    revision,
                    Lifecycle::Partial,
                ));
                context.revision_floor = revision;
                context.pending = Some(PendingSample { revision, usage });
            }
            "request/context" => {
                // contextPressure（pressureTokens/projectedTokens/contextWindow）
                // 是估算/投影，不进入用量（一次性诊断可见）。
                if !context.estimate_excluded_reported {
                    context.estimate_excluded_reported = true;
                    diagnostics.push(diag(
                        "context_estimate_not_counted",
                        Some("request/context"),
                        raw.number,
                        "contextPressure fields are estimates/projections; excluded from usage",
                    ));
                }
            }
            "request/header" | "image/offload" => {
                // 文档化非用量事件：无用量语义，跳过。
            }
            other => {
                fail_closed = Some((
                    raw.number,
                    format!("event type {other:?} not in documented set"),
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
    // 消失 attempt 墓碑（重扫差分）：上一轮入账、本轮折叠结果中不存在的
    // attempt 键 ⇒ Corrected/Excluded 撤销旧贡献，防截断改写双计。
    // 仅在完整读到文件尾时更新差分基（预算中断不做差分判断）。
    let mut tracked_update = None;
    if status == ScanStatus::Complete {
        let tracked: std::collections::BTreeSet<String> =
            context.tracked_keys.iter().cloned().collect();
        let current: std::collections::BTreeSet<String> = current_keys.iter().cloned().collect();
        for vanished in tracked.difference(&current) {
            diagnostics.push(DiagnosticInput {
                event_id: None,
                code: "attempt_vanished_after_rewrite".to_string(),
                field: None,
                position: Some(vanished.clone()),
                message: "attempt key absent from rewritten log; tombstoned Corrected/Excluded"
                    .to_string(),
            });
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: vanished.clone(),
                record_kind: RecordKind::ModelCall,
                schema_version: DSH_FORMAT_VERSION.to_string(),
                parser_version: DSH_PARSER_VERSION.to_string(),
                parse_basis: Some(VersionBasis::KnownVersion),
                origin_call_id: None,
                attempt_id: None,
                session_id: None,
                parent_session_id: None,
                host_application: None,
                agent: "deepseek-harness".to_string(),
                call_category: CallCategory::Unknown,
                occurred_at_ms: now_ms,
                observed_at_ms: Some(now_ms),
                source_time: None,
                time_basis: TimeBasis::ObservedAt,
                interval_start_ms: None,
                interval_end_ms: None,
                provider_id: None,
                model_raw: None,
                model_canonical: None,
                model_attribution: ModelAttribution::Unknown,
                usage: crate::domain::TokenUsage::default(),
                quality: crate::domain::TokenQuality::default(),
                lifecycle: Lifecycle::Corrected,
                source_revision: None,
                error_status: None,
                duration_ms: None,
                ttft_ms: None,
                attribution_status: AttributionStatus::Excluded,
                exclusion_reason: Some(
                    "attempt absent from rewritten log; tombstoned Corrected/Excluded".to_string(),
                ),
                cost: None,
            });
        }
        tracked_update = Some(current_keys);
    }
    if let Some(keys) = tracked_update {
        context.tracked_keys = keys;
    }
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
    let (cursor_out, context_out) = if let Some((line_no, detail)) = fail_closed {
        diagnostics.push(diag(
            "undocumented_event_type",
            Some("type"),
            line_no,
            &detail,
        ));
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
    fn parse_usage_optional_fields() {
        let full = serde_json::json!({
            "uncachedInputTokens": 75,
            "outputTokens": 12,
            "cacheReadTokens": 20,
            "cacheWriteTokens": 5
        });
        let (usage, unknown) = parse_usage(&full).unwrap();
        assert_eq!(usage.uncached_input_tokens, Some(75));
        assert_eq!(usage.cache_read_tokens, Some(20));
        assert!(!unknown);

        let partial = serde_json::json!({"uncachedInputTokens": 10, "outputTokens": 2});
        let (usage, _) = parse_usage(&partial).unwrap();
        assert_eq!(usage.cache_write_tokens, None, "missing stays unknown");
        assert!(usage.output_tokens.is_some());

        assert!(parse_usage(&serde_json::json!({"uncachedInputTokens": -1})).is_none());
        assert!(parse_usage(&serde_json::json!("not an object")).is_none());
        let extra = serde_json::json!({"uncachedInputTokens": 1, "reasoningTokens": 3});
        let (_, unknown) = parse_usage(&extra).unwrap();
        assert!(
            unknown,
            "unevidenced extra keys flagged, mapped fields kept"
        );
    }

    #[test]
    fn old_parse_context_defaults_restore() {
        // 旧/空上下文反序列化不失败：差分基与修订号地板缺省 0。
        let ctx: DshParseContext = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(ctx.revision_floor, 0);
        assert!(ctx.tracked_keys.is_empty());
        assert!(ctx.pending.is_none());
    }
}
