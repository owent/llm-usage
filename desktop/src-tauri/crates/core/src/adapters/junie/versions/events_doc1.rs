//! Junie CLI events.jsonl 格式实现（`events_doc1`，文档级 junie-events-doc-1）。
//!
//! 格式依据（第三方开源解析器 tokscale 固定提交
//! 1d9a9395418efc6952944b794097935d7d6fa1e8 sessions/junie.rs；闭源产品，
//! 以及官方 26.9.22（3419.29）发行包与真实本地 OpenAICompletion 样本）：
//! - 路径 `~/.junie/sessions/<session-id>/events.jsonl`（clients.rs:757-766）；
//!   官方 JUNIE_HOME 覆盖已实际核验。
//! - 用量事件判定 `event.agentEvent.kind == "LlmResponseMetadataEvent"`
//!   （junie.rs:54-57）；顶层 `timestampMs`（int 毫秒，**响应结束时刻**，
//!   junie.rs:113-119）；`event.agentEvent.modelUsage[]` 逐轮：
//!   `model`、input=`inputTokens|input`、output=`outputTokens|output`、
//!   cache_read=`cacheInputTokens|cacheReadInputTokens|cacheRead`、
//!   cache_write=`cacheCreateTokens|cacheCreationInputTokens|cacheWrite`、
//!   reasoning=`reasoningTokens|reasoningOutputTokens|thinkingTokens`、
//!   `cost`（正值为客户端 Estimated USD；付费渠道未验收）、`time`（正调用延迟 ms）、
//!   `provider`（junie.rs:224-245）。
//! - 起始时间 = timestampMs − time（仅当 time 在场，junie.rs:126-130）⇒
//!   occurred_at 取 timestampMs（SourceCompletion），duration=time。
//! - 会话目录名 `session-<yyMMdd>-<HHmmss>` 仅作备用时间来源（未采用：timestampMs
//!   在场才入账，缺时间戳跳行记诊断）。
//! - 对账键：junie:&lt;session&gt;:&lt;ts&gt;:&lt;model&gt;:&lt;五桶值&gt;:&lt;cost12位&gt;
//!   :&lt;行号&gt;:&lt;数组内索引&gt;（tokscale junie.rs:100-108 原键不含行号——
//!   两条不同行的同毫秒同内容事件会折叠；本仓约定要求同毫秒重复记录都入账，
//!   键含行号区分；JSONL 追加源行号稳定，rescan 重放行号一致，幂等性不变）。
//! - 官方 UsageTokens/inputTokens 是非缓存输入；缺字段默认零会进入
//!   ModelUsage，五桶、费用及耗时的零不能认证报告零。正桶独立保留，
//!   无 API 类型/产品版本及完整桶依据，不派生总输入/总 token。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{read_jsonl, JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::JUNIE_FORMAT_VERSION;

pub const JUNIE_PARSER_VERSION: &str = "junie-events-doc2";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct JunieParseContext {
    #[serde(default)]
    skipped_kinds: Vec<String>,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> JunieParseContext {
    if rescan {
        return JunieParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<JunieParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

fn session_id_of(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-session")
        .to_string()
}

/// 别名组取值：第一个在场的有效数值别名；越界（负/超限）返回 None
/// （调用方记诊断）；非整数别名不遮蔽同层后续有效别名（多版本兼容分支
/// 必须可达），全部别名非法才算形状偏离。
fn alias_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<Option<i64>> {
    let mut type_deviation = false;
    for key in keys {
        match obj.get(*key) {
            None => continue,
            Some(v) => {
                let Some(n) = v.as_i64() else {
                    type_deviation = true;
                    continue;
                };
                if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                    return None;
                }
                return Some(Some(n));
            }
        }
    }
    if type_deviation {
        // 有别名在场但全非整数：形状偏离（调用方记诊断跳过该条目）。
        None
    } else {
        // 无一别名在场。
        Some(None)
    }
}

fn usd_cost(value: Option<&serde_json::Value>) -> Option<CostAmount> {
    let amount = value?.as_f64()?;
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
    context.version_basis = Some(VersionBasis::KnownVersion);
    let cursor = restore_cursor(stored, target.generation, target.rescan);
    let session_id = session_id_of(&target.path);
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
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let Ok(value) = crate::adapters::run_policy::json_from_str::<serde_json::Value>(&line.text)
        else {
            diagnostics.push(diag("invalid_json_line", line.number, "line is not JSON"));
            continue;
        };
        let kind = value
            .pointer("/event/agentEvent/kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if kind != "LlmResponseMetadataEvent" {
            if !kind.is_empty() && !context.skipped_kinds.contains(&kind.to_string()) {
                context.skipped_kinds.push(kind.to_string());
            }
            continue;
        }
        let Some(timestamp_ms) = value
            .get("timestampMs")
            .and_then(|v| v.as_i64())
            .filter(|&ts| (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(&ts))
        else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "timestampMs missing/implausible; line skipped (session-dir fallback not adopted)",
            ));
            continue;
        };
        let Some(usage_array) = value
            .pointer("/event/agentEvent/modelUsage")
            .and_then(|v| v.as_array())
        else {
            diagnostics.push(diag(
                "model_usage_missing",
                line.number,
                "LlmResponseMetadataEvent without modelUsage array; skipped",
            ));
            continue;
        };
        for (index, entry) in usage_array.iter().enumerate() {
            let Some(obj) = entry.as_object() else {
                diagnostics.push(diag(
                    "model_usage_shape_deviation",
                    line.number,
                    "modelUsage entry is not an object; entry skipped",
                ));
                continue;
            };
            let model = obj.get("model").and_then(|v| v.as_str()).unwrap_or("");
            let input = alias_u64(obj, &["inputTokens", "input"]);
            let output = alias_u64(obj, &["outputTokens", "output"]);
            let cache_read = alias_u64(
                obj,
                &["cacheInputTokens", "cacheReadInputTokens", "cacheRead"],
            );
            let cache_write = alias_u64(
                obj,
                &[
                    "cacheCreateTokens",
                    "cacheCreationInputTokens",
                    "cacheWrite",
                ],
            );
            let reasoning = alias_u64(
                obj,
                &["reasoningTokens", "reasoningOutputTokens", "thinkingTokens"],
            );
            if input.is_none()
                && output.is_none()
                && cache_read.is_none()
                && cache_write.is_none()
                && reasoning.is_none()
            {
                diagnostics.push(diag(
                    "usage_without_numbers",
                    line.number,
                    "modelUsage entry carries no token alias; entry skipped",
                ));
                continue;
            }
            let invalid = [
                ("input", input),
                ("output", output),
                ("cache_read", cache_read),
                ("cache_write", cache_write),
                ("reasoning", reasoning),
            ]
            .iter()
            .any(|(_, v)| v.is_none());
            if invalid {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    line.number,
                    "a token alias carries a negative/out-of-range value; entry skipped",
                ));
                continue;
            }
            let (input, output, cache_read, cache_write, reasoning) = (
                input.unwrap(),
                output.unwrap(),
                cache_read.unwrap(),
                cache_write.unwrap(),
                reasoning.unwrap(),
            );
            let duration = obj.get("time").and_then(|v| v.as_i64()).filter(|d| *d > 0);
            let cost_micros = usd_cost(obj.get("cost"));
            let mapped = crate::adapters::usage_map::finish(
                crate::domain::TokenUsage {
                    input_uncached: input.filter(|v| *v > 0),
                    input_cache_read: cache_read.filter(|v| *v > 0),
                    input_cache_write: cache_write.filter(|v| *v > 0),
                    input_total: None,
                    output_total: output.filter(|v| *v > 0),
                    output_reasoning: reasoning.filter(|v| *v > 0),
                    total_tokens: None,
                    source_total: None,
                },
                // 在场桶必须标 Reported：TokenQuality::default() 全 Unknown 会在
                // ingest 校验（domain.rs:421 值与质量不一致）被拒，整批事件无法入账。
                crate::domain::TokenQuality {
                    input_cache_read: crate::domain::FieldQuality::Reported,
                    input_cache_write: crate::domain::FieldQuality::Reported,
                    input_uncached: crate::domain::FieldQuality::Reported,
                    output_total: crate::domain::FieldQuality::Reported,
                    output_reasoning: crate::domain::FieldQuality::Reported,
                    ..Default::default()
                },
                Vec::new(),
            );
            let bucket_sig = format!(
                "{}-{}-{}-{}-{}",
                input.unwrap_or_default(),
                output.unwrap_or_default(),
                cache_read.unwrap_or_default(),
                cache_write.unwrap_or_default(),
                reasoning.unwrap_or_default()
            );
            let cost_sig = obj
                .get("cost")
                .and_then(|v| v.as_f64())
                .map(|c| format!("{c:.12}"))
                .unwrap_or_else(|| "none".to_string());
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: format!(
                    "junie:{session_id}:{timestamp_ms}:{model}:{bucket_sig}:{cost_sig}:{}:{index}",
                    line.number
                ),
                record_kind: RecordKind::ModelCall,
                schema_version: JUNIE_FORMAT_VERSION.to_string(),
                parser_version: JUNIE_PARSER_VERSION.to_string(),
                parse_basis: Some(VersionBasis::KnownVersion),
                origin_call_id: None,
                attempt_id: None,
                session_id: Some(session_id.clone()),
                parent_session_id: None,
                host_application: None,
                agent: "junie".to_string(),
                call_category: CallCategory::Primary,
                occurred_at_ms: timestamp_ms,
                observed_at_ms: Some(now_ms),
                source_time: Some(timestamp_ms.to_string()),
                // timestampMs 是响应结束时刻（依据第三方解析器 junie.rs:113-119）。
                time_basis: TimeBasis::SourceCompletion,
                // duration 超过响应结束时刻 ⇒ 起点为负（数据矛盾）：
                // 起点置未知不编负值，端点保持真实报告。
                interval_start_ms: duration
                    .and_then(|d| timestamp_ms.checked_sub(d))
                    .filter(|s| *s >= 0),
                interval_end_ms: Some(timestamp_ms),
                provider_id: entry
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                model_raw: (!model.is_empty()).then(|| model.to_string()),
                model_canonical: None,
                model_attribution: ModelAttribution::RequestField,
                usage: mapped.usage,
                quality: mapped.quality,
                lifecycle: Lifecycle::Final,
                source_revision: None,
                error_status: None,
                duration_ms: duration,
                ttft_ms: None,
                attribution_status: AttributionStatus::Verified,
                exclusion_reason: None,
                cost: cost_micros,
            });
        }
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
    fn alias_groups_pick_first_present() {
        let obj: serde_json::Map<String, serde_json::Value> =
            crate::adapters::run_policy::json_from_str(
                r#"{"inputTokens": 10, "cacheRead": 4, "reasoningTokens": 2}"#,
            )
            .unwrap();
        assert_eq!(alias_u64(&obj, &["inputTokens", "input"]), Some(Some(10)));
        assert_eq!(alias_u64(&obj, &["input"]), Some(None));
        assert_eq!(alias_u64(&obj, &["cacheRead"]), Some(Some(4)));
        assert_eq!(
            alias_u64(&obj, &["reasoningTokens", "thinkingTokens"]),
            Some(Some(2))
        );
        let bad: serde_json::Map<String, serde_json::Value> =
            crate::adapters::run_policy::json_from_str(r#"{"inputTokens": -5}"#).unwrap();
        assert_eq!(alias_u64(&bad, &["inputTokens"]), None);
    }

    #[test]
    fn session_id_from_path() {
        let p = std::path::Path::new("/home/u/.junie/sessions/session-260901-101010/events.jsonl");
        assert_eq!(session_id_of(p), "session-260901-101010");
    }
}
