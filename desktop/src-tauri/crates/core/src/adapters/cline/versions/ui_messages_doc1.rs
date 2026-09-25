//! Cline ui_messages.json 格式实现（`ui_messages_doc1`，文档级 ui-messages-doc-1）。
//!
//! 格式证据（固定源码 dcf8c3c33596e3d561a941202297c564a1cbcd49，A03，文档级
//! 证据待真实样本；本机 not_found）：
//! - 路径：宿主（VS Code 扩展 saoudrizwan.claude-dev）globalStorage 下
//!   `tasks/<taskId>/ui_messages.json`（disk.ts：ensureTaskDirectoryExists =
//!   getGlobalStorageDir("tasks", taskId)，GlobalFileNames.uiMessages）；
//!   单文件 JSON **数组**，非 JSONL，消息删除/合并时整写重写。
//! - usage 载体（getApiMetrics.ts）：`type=="say"` 且
//!   `say ∈ {api_req_started, deleted_api_reqs, subagent_usage}`，`text` 为
//!   JSON 字符串，字段 tokensIn/tokensOut/cacheWrites/cacheReads/cost 逐字段
//!   可选；api_req_started 已与对应 api_req_finished 合并（不能每条 say 算请求，
//!   也无流式中间值落盘）。say="compaction" 的 tokensBefore/tokensAfter 是
//!   SDK 估算（chars/4 级），不进入用量。
//! - `ts` 是固定源码示例中唯一证据的消息身份字段（epoch 毫秒数字）；
//!   数组下标会因删除移位，不进身份。
//!
//! 增量语义（整写 JSON）：全量有界读取（32 MiB 初值）；游标存已消费字节数复用
//! 框架无变化短路；改写/截断走 generation 重扫，事件按稳定身份 upsert 幂等；
//! 半程写入（parse 失败）不推进游标，下轮确定性重试。
//!
//! 删除流程墓碑（deleted_api_reqs 的配套语义）：消息删除后原 api_req_started
//! 从数组消失、其用量以 deleted_api_reqs 聚合重述。整写重读时对"上一轮已入账、
//! 本轮消失"的键发射 Corrected/Excluded 墓碑（复用 ingest 仲裁
//! Corrected > Final 的 Replace 语义，撤销旧贡献），防止与聚合双计；
//! 已墓碑键再次原样恢复时 Corrected 优先保持排除（记录限制）。
//!
//! fail closed（V17）：非 say 记录类型、未文档化 say 种类 ⇒ 整文件拒绝
//! （游标不推进、下轮确定性再拒），不猜格式。真实文件中的 ask / say=text 等
//! 非用量消息未在固定源码中枚举，按未文档化处理，待真实样本扩展。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;
use std::path::Path;

use super::super::common::{map_cline_usage, ClineUsage};
use super::CLINE_FORMAT_VERSION;

pub const CLINE_PARSER_VERSION: &str = "cline-ui-messages-doc1";
/// 单文件有界读取上限（初值 32 MiB）。
pub const CLINE_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 文档化 say 种类（getApiMetrics.ts 三个 usage 载体 + compaction 估算载体）。
const DOCUMENTED_SAY_KINDS: &[&str] = &[
    "api_req_started",
    "deleted_api_reqs",
    "subagent_usage",
    "compaction",
];
const COMPACTION_SAY_KIND: &str = "compaction";
/// text JSON 的 usage 五键（tokensIn/tokensOut/cacheWrites/cacheReads/cost）。
const USAGE_KEYS: &[&str] = &[
    "request",
    "tokensIn",
    "tokensOut",
    "cacheWrites",
    "cacheReads",
    "cost",
];

/// 持久化解析上下文：上一轮已入账键（删除流程墓碑的差分基，跨重扫保留）
/// 与每文件一次性诊断标志 + 版本选择依据。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ClineParseContext {
    /// 上一轮从本文件入账的记录键（差分基，重扫不重置）。
    #[serde(default)]
    tracked_keys: Vec<String>,
    #[serde(default)]
    compaction_reported: bool,
    #[serde(default)]
    without_numbers_reported: bool,
    #[serde(default)]
    unmapped_keys_reported: bool,
    /// 版本选择依据（known_version / latest_fallback）；旧上下文缺省为 None。
    /// cline 固定为 KnownVersion（文档级锚点）。
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

/// 整写 JSON 游标：复用框架 JsonlCursor 形状（offset=已消费字节数，line_number 恒 1），
/// 无变化短路依赖 probe.len == cursor.offset。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    line_number: u64,
}

/// restore：一次性标志在重扫时重置（重读重新报告）；tracked_keys 跨重扫保留
/// （它是删除流程差分基，不是解析位置状态）。
fn restore_context(stored: &StoredScanState, rescan: bool) -> ClineParseContext {
    let mut ctx = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ClineParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if rescan {
        ctx.compaction_reported = false;
        ctx.without_numbers_reported = false;
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
/// 返回 None 表示 text 不是 JSON 对象或数值违例（调用方记 shape 诊断跳过；
/// 上游对 parse 失败静默忽略，本层可见化）。未文档化额外键返回 true。
fn parse_usage_text(
    text: &str,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<(ClineUsage, Option<f64>, bool)> {
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
    let usage = ClineUsage {
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
    let unknown = obj.keys().any(|k| !USAGE_KEYS.contains(&k.as_str()));
    Some((usage, cost, unknown))
}

/// cost 浮点美元 → micro-USD（estimated；来源口径未证实，按扩展自算估算入账，
/// 与 pi 适配器 map_cost 同规则）。溢出记诊断留空。
fn map_cost(
    cost: Option<f64>,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<CostAmount> {
    let total = cost?;
    if !total.is_finite() || total < 0.0 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost"),
            position,
            "cost not a finite non-negative number; cost left unknown",
        ));
        return None;
    }
    let micros = total * 1_000_000.0;
    if micros > i64::MAX as f64 {
        diagnostics.push(diag(
            "cost_shape_deviation",
            Some("cost"),
            position,
            "cost overflows micro-unit i64; cost left unknown",
        ));
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

/// 增量扫描一个任务 ui_messages.json（统一入口 `ClineAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = restore_context(stored, target.rescan);
    // 无版本字段可读：格式锚点是文档级 ui-messages-doc-1，固定 KnownVersion。
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
    if target.probe.len > CLINE_MAX_FILE_BYTES {
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
        .take(CLINE_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CLINE_MAX_FILE_BYTES {
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
    let mut records_seen: u64 = 0;
    let mut current_keys: Vec<String> = Vec::new();
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
            // 固定源码只证实 type="say" 的消息；其余记录类型未文档化 ⇒ fail closed。
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
        if say_kind == COMPACTION_SAY_KIND {
            // compaction 的 tokensBefore/tokensAfter 是 SDK 估算（chars/4 级），
            // 只驱动上下文条显示，不进入用量（一次性诊断可见）。
            if !context.compaction_reported {
                context.compaction_reported = true;
                diagnostics.push(diag(
                    "compaction_estimate_excluded",
                    Some("say"),
                    &position,
                    "compaction tokensBefore/tokensAfter are SDK estimates; excluded from usage",
                ));
            }
            continue;
        }
        let Some(text) = message_obj.get("text").and_then(|t| t.as_str()) else {
            // usage 载体无 text：上游短路不读；无用量证据，不产事件。
            continue;
        };
        let Some((usage, cost, unknown_keys)) = parse_usage_text(text, &position, &mut diagnostics)
        else {
            // 上游对 JSON parse 失败静默忽略；本层逐条诊断后跳过（部分可用）。
            diagnostics.push(diag(
                "usage_shape_deviation",
                Some("text"),
                &position,
                "usage text not a JSON object or a value is negative/non-integer/out of range; record skipped",
            ));
            continue;
        };
        if usage.is_empty() {
            // api_req_started 无 finished 等场景：载体无 usage 数字（可能只有
            // request/cost 描述）⇒ 无 token 证据不产事件；其余记录继续入账
            // （部分可用，一次性诊断）。cost 单独无 token 不入账，保持同源。
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
        if unknown_keys && !context.unmapped_keys_reported {
            context.unmapped_keys_reported = true;
            diagnostics.push(diag(
                "unmapped_usage_keys",
                Some("text"),
                &position,
                "usage text carries keys beyond the documented five; mapped fields kept",
            ));
        }
        let Some(ts) = message_obj.get("ts").and_then(|t| t.as_i64()) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                Some("ts"),
                &position,
                "message ts missing or not an integer; record skipped",
            ));
            continue;
        };
        if !(crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(&ts) {
            // 2000-01-01 至 2100-01-01 之外视为秒/毫秒误判，跳过该记录。
            diagnostics.push(diag(
                "timestamp_implausible",
                Some("ts"),
                &position,
                "message ts outside plausible millisecond range; record skipped",
            ));
            continue;
        }
        let is_aggregate = say_kind != "api_req_started";
        let source_record_key = format!("{task_id}:{say_kind}:{ts}");
        current_keys.push(source_record_key.clone());
        let mapped = map_cline_usage(&usage);
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                &position,
                &contradiction.detail,
            ));
        }
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: CLINE_FORMAT_VERSION.to_string(),
            parser_version: CLINE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(task_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "cline".to_string(),
            call_category: if say_kind == "subagent_usage" {
                CallCategory::SubAgent
            } else {
                CallCategory::Primary
            },
            occurred_at_ms: ts,
            observed_at_ms: Some(now_ms),
            source_time: Some(ts.to_string()),
            // api_req_started 的 ts 是请求起点（usage 由 finished 合并写回）；
            // 聚合记录的 ts 是聚合消息写入时刻，非任何单次调用起讫。
            time_basis: if is_aggregate {
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
            cost: map_cost(cost, &position, &mut diagnostics),
        });
    }
    // 删除流程墓碑：上一轮已入账、本轮消失的键（消息被删、用量以
    // deleted_api_reqs 聚合重述）⇒ Corrected/Excluded 撤销旧贡献防双计。
    let tracked: std::collections::BTreeSet<String> =
        context.tracked_keys.iter().cloned().collect();
    let current: std::collections::BTreeSet<String> = current_keys.iter().cloned().collect();
    for vanished in tracked.difference(&current) {
        diagnostics.push(diag(
            "source_message_removed",
            None,
            vanished.as_str(),
            "previously ingested key absent after rewrite; tombstoned Corrected/Excluded (deleted-api-req flow)",
        ));
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: vanished.clone(),
            record_kind: RecordKind::ModelCall,
            schema_version: CLINE_FORMAT_VERSION.to_string(),
            parser_version: CLINE_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(task_id.clone()),
            parent_session_id: None,
            host_application: None,
            agent: "cline".to_string(),
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
                "source message removed from ui_messages.json (deleted-api-req flow); usage restated via aggregate"
                    .to_string(),
            ),
            cost: None,
        });
    }
    context.tracked_keys = current_keys;
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
    fn parse_usage_text_optional_fields() {
        let mut diags = Vec::new();
        let (usage, cost, unknown) = parse_usage_text(
            r#"{"request":"syn","tokensIn":100,"tokensOut":20,"cacheWrites":10,"cacheReads":40,"cost":0.005}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.tokens_in, Some(100));
        assert_eq!(usage.cache_reads, Some(40));
        assert_eq!(cost, Some(0.005));
        assert!(!unknown);

        let (usage, cost, _) = parse_usage_text(
            r#"{"tokensIn":10,"tokensOut":2}"#,
            "messages[0]",
            &mut diags,
        )
        .unwrap();
        assert_eq!(usage.cache_writes, None, "missing stays unknown");
        assert_eq!(cost, None);

        assert!(parse_usage_text("not json", "messages[0]", &mut diags).is_none());
        assert!(
            parse_usage_text(r#"{"tokensIn":-1}"#, "messages[0]", &mut diags).is_none(),
            "negative value rejected"
        );
        let extra =
            parse_usage_text(r#"{"tokensIn":1,"surprise":2}"#, "messages[0]", &mut diags).unwrap();
        assert!(extra.2, "unknown keys flagged, mapped fields kept");
    }

    #[test]
    fn map_cost_micro_usd_estimated() {
        let mut diags = Vec::new();
        let cost = map_cost(Some(0.005), "messages[0]", &mut diags).unwrap();
        assert_eq!(cost.amount_minor, 5_000);
        assert_eq!(cost.currency, "USD");
        assert_eq!(cost.kind, CostKind::Estimated);
        assert!(map_cost(None, "messages[0]", &mut diags).is_none());
        assert!(map_cost(Some(-1.0), "messages[0]", &mut diags).is_none());
    }

    #[test]
    fn task_id_from_path() {
        let p = Path::new(
            "/home/u/globalStorage/saoudrizwan.claude-dev/tasks/syn-task-1/ui_messages.json",
        );
        assert_eq!(task_id_of(p), "syn-task-1".to_string());
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        let legacy = serde_json::json!({"tracked_keys": ["k1"]});
        let ctx: ClineParseContext = serde_json::from_value(legacy).expect("restore");
        assert_eq!(ctx.version_basis, None);
        assert_eq!(ctx.tracked_keys, vec!["k1".to_string()]);
    }
}
