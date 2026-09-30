//! gajae-code（gjc）会话 JSONL 格式实现（`session_v3like`，文档级
//! gjc-session-doc-1）。
//!
//! 格式证据（Yeachan-Heo/gajae-code 固定源码 7e54f9cbcf712cfa7f633d3c8da58a6d89f7f301，
//! 官方源码核验 + docs/session.md；pi 血统（共同祖先 v3，非现行 pi 子集）；
//! 本机未安装、无真实样本）：
//! - 布局：`<agentDir>/sessions/<scope>/<ISO-ts-dashes>_<uuid7>.jsonl`；
//!   子代理文件在父会话同名目录（去 .jsonl）下任意嵌套（官方 stats
//!   parser.ts:105-118 按路径深度推断角色；去重键 = 会话文件 + 条目 id）。
//! - 首行 `{type:"session", version:5, id, timestamp(ISO), cwd, ...}`；
//!   条目 `{type, id(8hex), parentId, timestamp(ISO)}`；assistant message：
//!   `{type:"message", message:{role, api, provider, model, responseId?, usage,
//!   timestamp(Unix **毫秒**，优先于条目 ISO)}}`。
//! - `usage`（packages/ai types.ts:713-760，**已归一化互斥桶**）：
//!   `input`（非缓存）/`output`（含 thinking）/`cacheRead`/`cacheWrite`/
//!   `totalTokens`（四桶之和）；`reasoningTokens?`⊆output；
//!   `cost{input,output,cacheRead,cacheWrite,total}`（USD，费率自算 ⇒
//!   Estimated）。与 pi 家族 map_pi_family 口径一致（官方归一化证据）。
//! - 官方 stats 口径（parser.ts:71-77,176-199）：只统计 role==assistant、
//!   五桶齐全（非负有限数）且 model/provider/api 非空的行；缺桶/缺 id 行
//!   跳过不造数（同本方"未知不补零"）。
//! - 会话头 timestamp 与条目 timestamp 为 ISO；message.timestamp 为毫秒
//!   （两种单位并存，勿混用——以 message.timestamp 优先）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::adapters::usage_map::{map_pi_family, PiFamilyUsage};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::GJC_FORMAT_VERSION;

pub const GJC_PARSER_VERSION: &str = "gjc-session-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// 固定源码 + docs/session.md 枚举的条目类型；未列出 ⇒ fail closed（V17）。
const DOCUMENTED_ENTRY_TYPES: &[&str] = &[
    "session",
    "message",
    "model_change",
    "thinking_level_change",
    "service_tier_change",
    "compaction",
    "branch_summary",
    "custom",
    "custom_message",
    "label",
    "ttsr_injection",
    "session_init",
    "mode_change",
    "mcp_tool_selection",
    "discovered_builtin_tool_selection",
    "header_patch",
    "entry_patch",
];

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct GjcParseContext {
    session_id: Option<String>,
    #[serde(default)]
    skipped_types: Vec<String>,
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

fn diag(code: &str, line: u64, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(format!("line:{line}")),
        message: message.to_string(),
    }
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> GjcParseContext {
    if rescan {
        return GjcParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<GjcParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

fn bounded(value: Option<&serde_json::Value>) -> Option<i64> {
    let n = value?.as_i64()?;
    (0..=MAX_REASONABLE_TOKEN).contains(&n).then_some(n)
}

fn rfc3339_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let s: &str = value?.as_str()?;
    let ts: jiff::Timestamp = s.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn usd_cost(total: Option<f64>) -> Option<CostAmount> {
    let amount = total?;
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
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

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = restore_context(stored, target.rescan);
    // version_basis 由会话头 version 字段确定（见 entry_type=="session" 分支）；
    // 增量续扫时从 parse_context 恢复，不硬编码。
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let read = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    for line in &read.lines {
        records_seen += 1;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line.text) else {
            diagnostics.push(diag("invalid_json_line", line.number, "line is not JSON"));
            continue;
        };
        let entry_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !DOCUMENTED_ENTRY_TYPES.contains(&entry_type) {
            // fail closed：整文件停下（游标不推进），待真实样本扩展。
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "undocumented_entry_type",
                    line.number,
                    &format!("entry type {entry_type:?} not in the documented set"),
                )],
                lines_read: read.lines.len() as u64,
                records_seen,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            });
        }
        if entry_type == "session" {
            context.session_id = value.get("id").and_then(|v| v.as_str()).map(str::to_string);
            // 与 detect 同规则：version==5 为已验证版本；其他/缺失按
            // LatestFallback 如实标注（不能虚标 KnownVersion）。
            context.version_basis = Some(match value.get("version").and_then(|v| v.as_i64()) {
                Some(5) => VersionBasis::KnownVersion,
                _ => VersionBasis::LatestFallback,
            });
            continue;
        }
        if entry_type != "message" {
            if !context.skipped_types.contains(&entry_type.to_string()) {
                context.skipped_types.push(entry_type.to_string());
            }
            continue;
        }
        let Some(message) = value.get("message").and_then(|v| v.as_object()) else {
            continue;
        };
        if message.get("role").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        // 官方 stats 口径：model/provider 非空才统计。
        let model = message.get("model").and_then(|v| v.as_str()).unwrap_or("");
        let provider = message
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if model.is_empty() || provider.is_empty() {
            continue;
        }
        let Some(usage) = message.get("usage").and_then(|v| v.as_object()) else {
            continue;
        };
        // 五桶齐全（官方 parser 同口径）；缺桶跳过不补零。
        let (Some(input), Some(output), Some(cache_read), Some(cache_write), Some(total)) = (
            bounded(usage.get("input")),
            bounded(usage.get("output")),
            bounded(usage.get("cacheRead")),
            bounded(usage.get("cacheWrite")),
            bounded(usage.get("totalTokens")),
        ) else {
            diagnostics.push(diag(
                "usage_bucket_missing",
                line.number,
                "assistant usage lacks one of the five buckets (official parser skips too); skipped",
            ));
            continue;
        };
        let Some(entry_id) = value.get("id").and_then(|v| v.as_str()) else {
            diagnostics.push(diag(
                "entry_without_id",
                line.number,
                "message entry lacks the required id; skipped",
            ));
            continue;
        };
        // message.timestamp（毫秒）优先；缺则条目 ISO。
        let occurred_ms = message
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .filter(|&ts| (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(&ts))
            .or_else(|| rfc3339_ms(value.get("timestamp")));
        let Some(occurred_ms) = occurred_ms else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "no plausible message/entry timestamp; skipped",
            ));
            continue;
        };
        let session_key = context
            .session_id
            .clone()
            .unwrap_or_else(|| "unknown-session".to_string());
        let reasoning = bounded(usage.get("reasoningTokens"));
        let mapped = map_pi_family(&PiFamilyUsage {
            input,
            output,
            cache_read,
            cache_write,
            total_tokens: total,
            reasoning,
        });
        let cost_total = usage
            .get("cost")
            .and_then(|c| c.get("total"))
            .and_then(|v| v.as_f64())
            .filter(|c| c.is_finite() && *c >= 0.0);
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("gjc:{session_key}:{entry_id}"),
            record_kind: RecordKind::ModelCall,
            schema_version: GJC_FORMAT_VERSION.to_string(),
            parser_version: GJC_PARSER_VERSION.to_string(),
            parse_basis: Some(
                context
                    .version_basis
                    .unwrap_or(VersionBasis::LatestFallback),
            ),
            origin_call_id: message
                .get("responseId")
                .and_then(|v| v.as_str())
                .map(|id| format!("gjc-response:{id}")),
            attempt_id: None,
            session_id: Some(session_key),
            parent_session_id: None,
            host_application: None,
            agent: "gajae-code".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: Some(provider.to_string()),
            model_raw: Some(model.to_string()),
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: usd_cost(cost_total),
        });
    }
    let status = match read.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineBudget | StopReason::TimeBudget => ScanStatus::BudgetExhausted,
        StopReason::LineTooLong { number, offset } => {
            diagnostics.push(diag(
                "line_exceeds_cap",
                number,
                &format!("line at byte {offset} exceeds the cap; cursor held for retry"),
            ));
            ScanStatus::LineTooLong
        }
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
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_micro_usd() {
        assert_eq!(usd_cost(Some(0.5)).unwrap().amount_minor, 500_000);
        assert!(usd_cost(None).is_none());
    }

    #[test]
    fn dual_timestamp_precedence() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"type":"message","id":"ab12","parentId":null,"timestamp":"2026-09-29T00:00:00Z",
                "message":{"role":"assistant","provider":"anthropic","model":"m",
                "timestamp":1790000000000,
                "usage":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4,"totalTokens":10}}}"#,
        )
        .unwrap();
        let message = value.get("message").unwrap();
        let ms = message
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .or_else(|| rfc3339_ms(value.get("timestamp")));
        assert_eq!(ms, Some(1_790_000_000_000));
    }
}
