//! Aider `--analytics-log` JSONL 格式实现（`analytics_doc1`，文档级
//! aider-analytics-doc-1）。
//!
//! 格式依据（Aider-AI/aider 固定源码 5dc9490bb35f9729ef2c95d00a19ccd30c26339c，
//! A30；本机未安装，无真实样本）：
//! - 启用：`--analytics-log <file>`（args.py:575-579，默认不开启）；即使遥测
//!   未 opt-in，只要设置 logfile 本地照写（analytics.py:213-214 event() 守卫）。
//!   **不回填历史**（写入仅发生在事件时刻），文件路径由用户指定。
//! - 每行 `{event, properties, user_id, time}`；`time` 是 `int(time.time())`
//!   Unix **秒**（analytics.py:242-254，追加模式）。
//! - `message_send` 事件（base_coder.py:2113-2122）properties：
//!   `main_model`/`weak_model`/`editor_model`/`edit_format` +
//!   `prompt_tokens`/`completion_tokens`/`total_tokens` + `cost`/`total_cost`。
//!   **无 cache 分项**：Anthropic cache_creation 并入 prompt_tokens
//!   （base_coder.py:2011-2019），cache_hit 只进成本公式不上报。
//!   `cost` 是本条消息成本（litellm 费率自算 ⇒ Estimated）；
//!   `total_cost` 是会话累计（不入账，避免双计）。
//! - 模型名可能被 `_redact_model_name` 脱敏（models.db 未知且含 `/` 的只留
//!   provider 前缀 + "/REDACTED"，analytics.py:190-199）：按原文入账。
//!
//! 映射：input_total=prompt_tokens（含 cache 写，官方字段语义），cache 两桶 Unknown，
//! output_total=completion_tokens，total_tokens=total_tokens（直报）。
//! 事件键 = 整行内容哈希（无消息 ID；追加式文件，重放幂等）。

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

use super::AIDER_FORMAT_VERSION;

pub const AIDER_PARSER_VERSION: &str = "aider-analytics-doc1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct AiderParseContext {
    #[serde(default)]
    skipped_events: Vec<String>,
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

fn restore_context(stored: &StoredScanState, rescan: bool) -> AiderParseContext {
    if rescan {
        return AiderParseContext::default();
    }
    stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<AiderParseContext>(v.clone()).ok())
        .unwrap_or_default()
}

/// Unix 秒 → UTC 毫秒（越域拒绝）。
fn seconds_to_ms(secs: i64) -> Option<i64> {
    let ms = secs.checked_mul(1000)?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn bounded(value: Option<&serde_json::Value>) -> Option<Option<i64>> {
    match value {
        None => Some(None),
        Some(v) => match v.as_i64() {
            Some(n) if (0..=MAX_REASONABLE_TOKEN).contains(&n) => Some(Some(n)),
            _ => None,
        },
    }
}

fn usd_cost(value: Option<&serde_json::Value>) -> Option<CostAmount> {
    let amount = value?.as_f64()?;
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    // 与全库约定一致：amount_minor 存 micro-USD（zoo/pi/opencode/cline 规则相同）。
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        // litellm 费率自算（base_coder.py show_exhausted_tokens_report 成本公式）。
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
        let event_name = value.get("event").and_then(|v| v.as_str()).unwrap_or("");
        if event_name != "message_send" {
            if !context.skipped_events.contains(&event_name.to_string()) {
                context.skipped_events.push(event_name.to_string());
            }
            continue;
        }
        let Some(properties) = value.get("properties") else {
            diagnostics.push(diag(
                "message_send_without_properties",
                line.number,
                "message_send event lacks properties object; skipped",
            ));
            continue;
        };
        let Some(Some(prompt)) = bounded(properties.get("prompt_tokens")) else {
            diagnostics.push(diag(
                "token_shape_deviation",
                line.number,
                "prompt_tokens missing/negative/out-of-range; record skipped",
            ));
            continue;
        };
        let Some(Some(completion)) = bounded(properties.get("completion_tokens")) else {
            diagnostics.push(diag(
                "token_shape_deviation",
                line.number,
                "completion_tokens missing/negative/out-of-range; record skipped",
            ));
            continue;
        };
        let total = match bounded(properties.get("total_tokens")) {
            Some(Some(t)) => Some(t),
            _ => None,
        };
        if total.is_none() {
            diagnostics.push(diag(
                "token_shape_deviation",
                line.number,
                "total_tokens missing/negative/out-of-range; kept unknown",
            ));
        }
        let Some(time_s) = value.get("time").and_then(|v| v.as_i64()) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "time field missing/not an integer second timestamp; record skipped",
            ));
            continue;
        };
        let Some(occurred_ms) = seconds_to_ms(time_s) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "time field outside plausible range; record skipped",
            ));
            continue;
        };
        let model = properties
            .get("main_model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let (provider, model_raw) = match model.split_once('/') {
            Some((provider, rest)) => (Some(provider.to_string()), Some(rest.to_string())),
            None => (None, (!model.is_empty()).then_some(model.clone())),
        };
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: None,
                input_cache_read: None,
                input_cache_write: None,
                input_total: Some(prompt),
                output_total: Some(completion),
                output_reasoning: None,
                total_tokens: total,
                source_total: total,
            },
            crate::domain::TokenQuality {
                input_total: crate::domain::FieldQuality::Reported,
                output_total: crate::domain::FieldQuality::Reported,
                total_tokens: total
                    .map(|_| crate::domain::FieldQuality::Reported)
                    .unwrap_or(crate::domain::FieldQuality::Unknown),
                source_total: total
                    .map(|_| crate::domain::FieldQuality::Reported)
                    .unwrap_or(crate::domain::FieldQuality::Unknown),
                ..Default::default()
            },
            Vec::new(),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            // 无消息 ID：追加式文件，行号+内容哈希是稳定身份（重放幂等；
            // 完全相同的重复发送靠行号区分，不被哈希折叠）。
            source_record_key: format!(
                "aider:{}:{}",
                line.number,
                crate::identity::content_hash(&line.text)
            ),
            record_kind: RecordKind::ModelCall,
            schema_version: AIDER_FORMAT_VERSION.to_string(),
            parser_version: AIDER_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: None,
            parent_session_id: None,
            host_application: None,
            agent: "aider".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(time_s.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: provider,
            model_raw,
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
            cost: usd_cost(properties.get("cost")),
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
    fn seconds_to_ms_bounds() {
        assert_eq!(seconds_to_ms(1_755_100_406), Some(1_755_100_406_000));
        assert_eq!(seconds_to_ms(0), None);
    }

    #[test]
    fn cost_mapping_micro_usd() {
        let cost = usd_cost(Some(&serde_json::json!(0.0133175))).unwrap();
        assert_eq!(cost.amount_minor, 13_318);
        assert_eq!(cost.kind, CostKind::Estimated);
        assert!(usd_cost(Some(&serde_json::json!(-1.0))).is_none());
    }
}
