//! ZCode model-io JSONL 格式实现（`modelio_v1`）。
//!
//! 格式证据（M0 fixtures + tests/fixtures/zcode/real-*，本机 ZCode 3.14.3 实读）：
//! - 路径：`~/.zcode/cli/rollout/model-io-<sessionId>.jsonl`（append-only JSONL，
//!   每次模型调用一条记录；无文档化环境覆盖）。
//! - 每条：`type="model_io"`、`attempt`、`sessionId`、`requestId`、`turnId`、`traceId`、
//!   `querySource ∈ {main_turn, subagent, session_title}`（db.model_usage 实读另证
//!   session_title 来源）、`model{modelId,providerId}`、`startedAt/completedAt`
//!   ISO8601 UTC 毫秒字符串、`durationMs`、`request.headers["x-zcode-app-version"]`。
//! - **双口径**（同一记录两个 usage 视图，互斥不混算）：
//!   - 主口径 AI SDK camelCase `response.usage` 五键：`inputTokens`（含缓存读）、
//!     `outputTokens`、`totalTokens`、`cacheReadTokens`、`cacheWriteTokens`；
//!   - 对照口径 anthropic snake_case `response.providerMetadata.anthropic.usage`：
//!     `input_tokens`（不含缓存）、`output_tokens`、`cache_read_input_tokens`、
//!     `cache_creation_input_tokens?`；缓存创建亦见 camel
//!     `providerMetadata.anthropic.cacheCreationInputTokens`（null 表示未报）。
//!
//! 两视图同时在场时做一致性校验（in+cr+cw 与 out 逐条对比），矛盾记
//! `dual_caliber_mismatch` 诊断并保留 AI SDK 主口径；AI SDK 视图缺席而
//! anthropic 在场时回退对照口径（记诊断），绝不相加。
//! - 在途尾部：`response.finishReason=null` 且无 usage/providerMetadata 属正常形状，
//!   不产事件、不失败。
//! - 缺 requestId：回退身份 `seq:{sessionId}:{行号}` 并记诊断（不得全部变成
//!   zcode:None，adapters.md ZCode 行）。
//!
//! fail closed（V17）：`type` 非 `model_io` 的未文档化记录类型整文件拒绝
//!（事件清空、游标不推进、下轮确定性再拒）——model-io 文件实读只含 model_io
//! 一种类型，出现其他类型即格式漂移，不得静默跳过。
//!
//! 版本策略（architecture.md#unknown-version）：版本锚点
//! `x-zcode-app-version` 经 [`super::super::versions::select`] 分派；未收录/缺失
//! 版本用本实现（当前最新）兼容尝试，事件带 `parse_basis` 标记，不因版本号
//! 未收录直接拒绝。

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::zcode::common::{
    map_zcode_ai_sdk, map_zcode_anthropic, ZcodeAiSdkUsage, ZcodeAnthropicUsage,
};

pub const ZCODE_PARSER_VERSION: &str = "zcode-modelio-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// epoch 数值折算阈值：小于该值视为秒（synthetic-epoch-timestamps 防御合同）。
const EPOCH_SECONDS_THRESHOLD: i64 = 100_000_000_000;

/// 已证实的 querySource → 调用分类（db.model_usage 实读值域：
/// main_turn / subagent / session_title）。
fn map_query_source(source: Option<&str>) -> (CallCategory, bool) {
    match source {
        Some("main_turn") => (CallCategory::Primary, false),
        Some("subagent") => (CallCategory::SubAgent, false),
        Some("session_title") => (CallCategory::Auxiliary, false),
        _ => (CallCategory::Unknown, true),
    }
}

/// 持久化解析上下文（一次性诊断标志与版本选择依据）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ZcodeParseContext {
    #[serde(default)]
    unmapped_query_source_reported: bool,
    /// 版本选择依据（known_version / latest_fallback）；旧解析上下文缺省为 None，
    /// 迁移不重建来源、不重置游标（V30）。
    #[serde(default)]
    version_basis: Option<VersionBasis>,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> ZcodeParseContext {
    if rescan {
        return ZcodeParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<ZcodeParseContext>(v.clone()).ok())
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

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// 数值字段：存在时必须是有界非负整数（违例返回 None，调用方记诊断）。
fn token_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<Option<i64>> {
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
}

/// AI SDK camelCase 五键视图：inputTokens/outputTokens 必填，
/// cacheRead/cacheWrite/total/reasoning 可选；任一在场键违例返回 None。
fn parse_ai_sdk_usage(value: &serde_json::Value) -> Option<ZcodeAiSdkUsage> {
    let obj = value.as_object()?;
    Some(ZcodeAiSdkUsage {
        input_tokens: token_field(obj, "inputTokens")??,
        cached_input_tokens: token_field(obj, "cacheReadTokens")?,
        cache_creation_input_tokens: token_field(obj, "cacheWriteTokens")?,
        output_tokens: token_field(obj, "outputTokens")??,
        reasoning_tokens: token_field(obj, "reasoningTokens")?,
        total_tokens: token_field(obj, "totalTokens")?,
    })
}

/// anthropic snake_case 视图（含 camel `cacheCreationInputTokens` 回退）。
/// `anthropic_parent` 是 `providerMetadata.anthropic` 对象。
fn parse_anthropic_usage(
    usage: &serde_json::Value,
    anthropic_parent: &serde_json::Value,
) -> Option<ZcodeAnthropicUsage> {
    let obj = usage.as_object()?;
    let creation = token_field(obj, "cache_creation_input_tokens")?.or_else(|| {
        anthropic_parent
            .get("cacheCreationInputTokens")
            .and_then(|v| v.as_i64())
            .filter(|&n| (0..=MAX_REASONABLE_TOKEN).contains(&n))
    });
    Some(ZcodeAnthropicUsage {
        input_tokens: token_field(obj, "input_tokens")??,
        cache_read_input_tokens: token_field(obj, "cache_read_input_tokens")?,
        cache_creation_input_tokens: creation,
        output_tokens: token_field(obj, "output_tokens")??,
    })
}

/// 时间戳：实读为 ISO8601 毫秒字符串；数值按防御合同折算毫秒（<1e11 视为秒）。
/// 返回 (ms, 原始白名单字符串)。
fn parse_ts(value: &serde_json::Value) -> Option<(i64, String)> {
    match value {
        serde_json::Value::String(s) => s
            .parse::<jiff::Timestamp>()
            .ok()
            .map(|t| (t.as_millisecond(), s.clone())),
        serde_json::Value::Number(n) => {
            let v = n.as_i64()?;
            let ms = if v < EPOCH_SECONDS_THRESHOLD {
                v.checked_mul(1000)?
            } else {
                v
            };
            Some((ms, n.to_string()))
        }
        _ => None,
    }
}

/// 版本锚点（每条记录自带；探测与事件 schema_version 共用）。
pub fn version_anchor(line: &serde_json::Value) -> Option<&str> {
    line.get("request")?
        .get("headers")?
        .get("x-zcode-app-version")?
        .as_str()
}

/// 增量扫描一个 model-io JSONL 文件（统一入口 `ZcodeAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let mut context = restore_context(stored, target.rescan);
    // 版本分派（探测/扫描同一注册表）：首条记录的版本锚点决定解析依据并持久化
    //（跨增量轮次稳定）；锚点缺失 ⇒ latest_fallback 兼容尝试（V17/V30）。
    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut fail_closed: Option<(u64, String, &'static str)> = None;
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
        if record_type != "model_io" {
            // 未文档化记录类型：fail closed（事件清空、游标不推进、下轮确定性再拒）。
            fail_closed = Some((
                raw.number,
                format!("record type {record_type:?} not model_io"),
                "undocumented_record_type",
            ));
            break;
        }
        // 版本锚点：首次见到时按注册表确定解析依据并持久化。
        let anchor = version_anchor(&line);
        if context.version_basis.is_none() {
            let selection = super::super::versions::select(anchor);
            context.version_basis = Some(selection.basis);
        }
        // 双口径视图解析。
        let response = line
            .get("response")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let ai_sdk_value = response.get("usage").filter(|u| u.is_object());
        let anthropic_parent = response
            .get("providerMetadata")
            .and_then(|pm| pm.get("anthropic"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let anthropic_value = anthropic_parent.get("usage").filter(|u| u.is_object());
        // 在途尾部（无任何 usage 视图）：正常形状，不产事件、不失败。
        if ai_sdk_value.is_none() && anthropic_value.is_none() {
            continue;
        }
        // 时间：completedAt 优先，缺失/不可解析回退 startedAt；均失败跳过该条。
        let (occurred_ms, source_time) = match line
            .get("completedAt")
            .and_then(parse_ts)
            .or_else(|| line.get("startedAt").and_then(parse_ts))
        {
            Some(pair) => pair,
            None => {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    Some("completedAt"),
                    raw.number,
                    "record timestamps missing or unparseable; record skipped",
                ));
                continue;
            }
        };
        // usage 解析 + 口径选择（互斥取一，绝不相加）。
        let mapped = match ai_sdk_value {
            Some(value) => {
                let Some(sdk) = parse_ai_sdk_usage(value) else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("response.usage"),
                        raw.number,
                        "AI SDK usage missing required numeric fields or out of range; record skipped",
                    ));
                    continue;
                };
                // 双口径一致性校验（对照视图在场时）：矛盾进诊断，主口径保留。
                if let Some(anth_usage) =
                    anthropic_value.and_then(|v| parse_anthropic_usage(v, &anthropic_parent))
                {
                    let anth_input = anth_usage
                        .input_tokens
                        .saturating_add(anth_usage.cache_read_input_tokens.unwrap_or(0))
                        .saturating_add(anth_usage.cache_creation_input_tokens.unwrap_or(0));
                    if anth_input != sdk.input_tokens
                        || anth_usage.output_tokens != sdk.output_tokens
                    {
                        diagnostics.push(diag(
                            "dual_caliber_mismatch",
                            Some("response.usage"),
                            raw.number,
                            &format!(
                                "anthropic view in+cr+cw={} out={} != AI SDK inputTokens={} outputTokens={}; AI SDK caliber kept",
                                anth_input, anth_usage.output_tokens, sdk.input_tokens, sdk.output_tokens
                            ),
                        ));
                    }
                }
                map_zcode_ai_sdk(&sdk)
            }
            None => {
                // AI SDK 视图缺席、anthropic 在场：回退对照口径并记诊断。
                let anth_usage =
                    anthropic_value.and_then(|v| parse_anthropic_usage(v, &anthropic_parent));
                let Some(anth) = anth_usage else {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("providerMetadata.anthropic.usage"),
                        raw.number,
                        "anthropic usage missing required numeric fields or out of range; record skipped",
                    ));
                    continue;
                };
                diagnostics.push(diag(
                    "ai_sdk_usage_missing",
                    Some("response.usage"),
                    raw.number,
                    "response.usage absent; exclusive fallback to anthropic view (never summed)",
                ));
                map_zcode_anthropic(&anth)
            }
        };
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                raw.number,
                &contradiction.detail,
            ));
        }
        // 分类（querySource 实读值域；未映射值一次诊断 + unknown，不猜）。
        let query_source = json_str(&line, "querySource");
        let (category, unmapped) = map_query_source(query_source);
        if unmapped && !context.unmapped_query_source_reported {
            context.unmapped_query_source_reported = true;
            diagnostics.push(diag(
                "unmapped_query_source",
                Some("querySource"),
                raw.number,
                "querySource value outside evidenced set {main_turn, subagent, session_title}; classified unknown",
            ));
        }
        // 身份：zcode:{requestId}:{attempt}；缺 requestId 回退 seq:{sessionId}:{行号}。
        let session_id = json_str(&line, "sessionId");
        let attempt = line.get("attempt").and_then(|v| v.as_i64()).unwrap_or(1);
        let (source_record_key, origin_call_id) = match json_str(&line, "requestId") {
            Some(rid) => (format!("zcode:{rid}:{attempt}"), Some(rid.to_string())),
            None => {
                diagnostics.push(diag(
                    "missing_request_id",
                    Some("requestId"),
                    raw.number,
                    "model_io without requestId; fallback identity session + line number",
                ));
                (
                    format!(
                        "seq:{}:{}",
                        session_id.unwrap_or("unknown-session"),
                        raw.number
                    ),
                    None,
                )
            }
        };
        let model = line
            .get("model")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let model_raw = json_str(&model, "modelId").map(str::to_string);
        let provider_id = json_str(&model, "providerId").map(str::to_string);
        let duration_ms = line
            .get("durationMs")
            .and_then(|v| v.as_i64())
            .filter(|&d| d >= 0);
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: anchor.unwrap_or("unknown").to_string(),
            parser_version: ZCODE_PARSER_VERSION.to_string(),
            parse_basis: context.version_basis,
            origin_call_id,
            attempt_id: Some(attempt.to_string()),
            session_id: session_id.map(str::to_string),
            parent_session_id: None,
            host_application: None,
            agent: "zcode".to_string(),
            call_category: category,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(source_time),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id,
            model_raw: model_raw.clone(),
            model_canonical: None,
            model_attribution: if model_raw.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
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
    let new_cursor = JsonlCursor {
        generation: target.generation,
        offset: outcome.next_offset,
        line_number: outcome.next_line_number,
    };
    let mut health_degraded = !outcome.bad_lines.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_json_line"
                    | "usage_shape_deviation"
                    | "timestamp_unparseable"
                    | "line_too_long"
            )
        });
    // fail closed：本轮事件清空、游标不推进（不提交 checkpoint），下轮确定性再拒。
    let (cursor_out, context_out) = if let Some((line_no, detail, code)) = fail_closed {
        diagnostics.push(diag(code, Some("type"), line_no, &detail));
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
    fn epoch_numbers_normalize_to_milliseconds() {
        let (ms1, raw1) = parse_ts(&serde_json::json!(1_800_000_000_000_i64)).unwrap();
        assert_eq!(ms1, 1_800_000_000_000);
        assert_eq!(raw1, "1800000000000");
        // 秒级数值（<1e11）折算毫秒。
        let (ms2, _) = parse_ts(&serde_json::json!(1_800_000_000_i64)).unwrap();
        assert_eq!(ms2, 1_800_000_000_000);
        // ISO 字符串（实读形状）。
        let (ms3, raw3) = parse_ts(&serde_json::json!("2026-09-25T09:10:45.285Z")).unwrap();
        assert_eq!(raw3, "2026-09-25T09:10:45.285Z");
        assert_eq!(
            ms3,
            "2026-09-25T09:10:45.285Z"
                .parse::<jiff::Timestamp>()
                .unwrap()
                .as_millisecond()
        );
        assert!(parse_ts(&serde_json::json!(null)).is_none());
    }

    #[test]
    fn ai_sdk_usage_requires_input_output() {
        let full = serde_json::json!({
            "inputTokens": 2000, "outputTokens": 100, "totalTokens": 2100,
            "cacheReadTokens": 800, "cacheWriteTokens": 200
        });
        let sdk = parse_ai_sdk_usage(&full).unwrap();
        assert_eq!(sdk.cache_creation_input_tokens, Some(200));
        let missing = serde_json::json!({"inputTokens": 10});
        assert!(parse_ai_sdk_usage(&missing).is_none());
        // 负值（synthetic-negative-usage）：违例返回 None。
        let negative = serde_json::json!({
            "inputTokens": -5, "outputTokens": 10, "totalTokens": 5,
            "cacheReadTokens": 0, "cacheWriteTokens": 0
        });
        assert!(parse_ai_sdk_usage(&negative).is_none());
    }

    #[test]
    fn anthropic_creation_falls_back_to_camel_field() {
        let parent = serde_json::json!({
            "usage": {"input_tokens": 1000, "output_tokens": 100, "cache_read_input_tokens": 800},
            "cacheCreationInputTokens": 200
        });
        let anth = parse_anthropic_usage(&parent["usage"], &parent).unwrap();
        assert_eq!(anth.cache_creation_input_tokens, Some(200));
        // snake 在场优先；两者均缺（实读形状：null/缺席）⇒ None。
        let real = serde_json::json!({
            "usage": {
                "input_tokens": 139, "output_tokens": 120, "cache_read_input_tokens": 390976
            },
            "cacheCreationInputTokens": null
        });
        let anth = parse_anthropic_usage(&real["usage"], &real).unwrap();
        assert_eq!(anth.cache_creation_input_tokens, None);
    }

    #[test]
    fn query_source_maps_evidenced_values() {
        assert_eq!(
            map_query_source(Some("main_turn")),
            (CallCategory::Primary, false)
        );
        assert_eq!(
            map_query_source(Some("subagent")),
            (CallCategory::SubAgent, false)
        );
        assert_eq!(
            map_query_source(Some("session_title")),
            (CallCategory::Auxiliary, false)
        );
        assert_eq!(
            map_query_source(Some("mystery")),
            (CallCategory::Unknown, true)
        );
        assert_eq!(map_query_source(None), (CallCategory::Unknown, true));
    }

    #[test]
    fn old_parse_context_without_basis_still_restores() {
        // 旧解析上下文（无 version_basis 字段）反序列化不失败，basis 为 None；
        // 目录迁移不重建来源/重置游标（V30）。
        let legacy = serde_json::json!({"unmapped_query_source_reported": true});
        let ctx: ZcodeParseContext = serde_json::from_value(legacy).expect("restore");
        assert!(ctx.unmapped_query_source_reported);
        assert_eq!(ctx.version_basis, None);
    }
}
