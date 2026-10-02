//! OTel spans JSONL 格式实现（`spans_doc1`；Copilot native file parser v2）。
//!
//! 格式证据：2026-09-29 官方文档、2026-10-02 VS Code 1.140.0 本机 30 个 CLIENT span
//! （docs/validation/desktop-usage/dashboard-repair.md）；CLI/JetBrains 仍为独立待验版本。
//! - **VS Code Copilot Chat file exporter**（microsoft/vscode
//!   extensions/copilot/docs/monitoring/agent_monitoring.md @ bdc5ebe）：
//!   `github.copilot.chat.otel.exporterType="file"` + `outfile`，或
//!   `COPILOT_OTEL_FILE_EXPORTER_PATH`；"newline-delimited JSON records …
//!   it is not an OTLP JSON payload"，span 记录含 trace/span IDs、parent
//!   context、attributes、resource、scope、status；`startTime`/`endTime`/
//!   `duration` 用 `[seconds, nanoseconds]` 对。chat span（CLIENT，每 LLM 请求
//!   一个）属性：`gen_ai.request.model`/`gen_ai.response.model`/
//!   `gen_ai.usage.input_tokens`/`gen_ai.usage.output_tokens`/
//!   `gen_ai.usage.cache_read.input_tokens`/`gen_ai.usage.cache_creation.
//!   input_tokens`/`gen_ai.usage.reasoning.output_tokens`/
//!   `copilot_chat.time_to_first_token`（ms）/
//!   `gen_ai.response.time_to_first_chunk`（秒）。
//! - **Copilot CLI**（docs.github.com OpenTelemetry monitoring）：
//!   `COPILOT_OTEL_FILE_EXPORTER_PATH`（"JSON-lines"，行级 schema 未文档化，
//!   待本机样本——本实现按同族 span 记录容错解析）；chat span 属性同上 +
//!   `github.copilot.nano_aiu`；**invoke_agent 根 span 是全 turn 汇总，
//!   官方警告不得与子 chat span 求和双计** ⇒ 只采 chat，跳过汇总 span。
//! - **CodeBuddy agentlens**（codebuddy.ai/docs/cli/monitoring）：
//!   model_stream span（LLM）：无前缀 `usage.input_tokens`/`usage.output_tokens`/
//!   `usage.total_tokens`、`model_name`/`request.model`、
//!   `response.time_to_first_token`；model_request 不导出（官方：双计）。
//!   仅 OTLP/protobuf 导出 ⇒ 由本应用 OTLP 接收器归一化为同形状 JSONL。
//! - trace/span ID 键名文档未逐字给出 ⇒ 双拼写容错（spanId/span_id）；
//!   不可用记录跳行记诊断（不猜）。
//! - 已核验 VS Code Copilot gen_ai input 含缓存、reasoning 为 output 子集；仅两桶均已知派生总量。
//!   CLI/其他同型记录的包含关系不从 VS Code 推断，保持总量未知。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::jsonl::{JsonlCursor, StopReason};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::OTEL_FORMAT_VERSION;

pub const OTEL_PARSER_VERSION: &str = "otel-spans-file-2";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct OtelParseContext {
    #[serde(default)]
    policy_version: u32,
    #[serde(default)]
    skipped_names: Vec<String>,
    #[serde(default)]
    version_basis: Option<VersionBasis>,
}

pub fn should_scan_unchanged(stored: &StoredScanState) -> bool {
    stored
        .parse_context
        .as_ref()
        .and_then(|v| v.get("policy_version"))
        .and_then(serde_json::Value::as_u64)
        != Some(3)
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

/// 记录里的属性表（attributes / resource.attributes 均可）。
fn attrs_of(record: &serde_json::Value) -> Vec<&serde_json::Map<String, serde_json::Value>> {
    let mut out = Vec::new();
    if let Some(map) = record.get("attributes").and_then(|v| v.as_object()) {
        out.push(map);
    }
    if let Some(map) = record
        .pointer("/resource/attributes")
        .and_then(|v| v.as_object())
    {
        out.push(map);
    }
    out
}

fn attr_u64(record: &serde_json::Value, keys: &[&str]) -> Option<Option<i64>> {
    for map in attrs_of(record) {
        for key in keys {
            match map.get(*key) {
                None => continue,
                Some(v) => {
                    // 属性值可能是裸数或 {intValue:..}（OTLP JSON 形）。
                    let n = v
                        .as_i64()
                        .or_else(|| v.get("intValue").and_then(|x| x.as_i64()))
                        .or_else(|| {
                            v.get("asInt")
                                .and_then(|x| x.as_i64())
                                .or_else(|| v.get("doubleValue").and_then(|x| x.as_i64()))
                        })?;
                    // 越界（负/超上限）按格式偏离处理：返回 None 让调用方跳过
                    // 整条记录，不能与"键缺失"（Some(None)）混同而静默丢桶。
                    if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                        return None;
                    }
                    return Some(Some(n));
                }
            }
        }
    }
    Some(None)
}

fn attr_str<'a>(record: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    for map in attrs_of(record) {
        for key in keys {
            if let Some(v) = map.get(*key) {
                if let Some(s) = v.as_str() {
                    return Some(s);
                }
                if let Some(s) = v.get("stringValue").and_then(|x| x.as_str()) {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// 浮点属性（TTFT 秒值可含小数；裸数或 {doubleValue}/{intValue} 形）。
fn attr_f64(record: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    for map in attrs_of(record) {
        for key in keys {
            if let Some(v) = map.get(*key) {
                let f = v
                    .as_f64()
                    .or_else(|| v.get("doubleValue").and_then(|x| x.as_f64()))
                    .or_else(|| v.get("intValue").and_then(|x| x.as_i64()).map(|n| n as f64));
                if let Some(f) = f {
                    if f.is_finite() && f >= 0.0 && f <= MAX_REASONABLE_TOKEN as f64 {
                        return Some(f);
                    }
                }
            }
        }
    }
    None
}

/// service.name → 统计 agent 名（无则按 OTel 来源未知处理）。
/// vs-copilot 与 vs_copilot 适配器（VS 自动遥测）同维度：两载体取其一防双计。
fn agent_of(record: &serde_json::Value) -> &'static str {
    match attr_str(record, &["service.name", "service_name"]).unwrap_or("") {
        "github-copilot" | "copilot-cli" => "copilot-cli",
        "copilot-chat" | "vscode-copilot-chat" => "vscode-copilot-chat",
        "vs-copilot" | "visualstudio-copilot" => "vs-copilot",
        "codebuddy" | "codebuddy-code" | "codebuddy_code" => "codebuddy",
        _ => "otel-unknown",
    }
}

/// startTime：[秒, 纳秒] 对或毫秒整数。
fn start_ms(record: &serde_json::Value) -> Option<i64> {
    match record.get("startTime") {
        Some(serde_json::Value::Array(pair)) if pair.len() == 2 => {
            let secs = pair[0].as_i64()?;
            let nanos = pair[1].as_i64().unwrap_or(0);
            let ms = secs.checked_mul(1000)?.checked_add(nanos / 1_000_000)?;
            (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
                .contains(&ms)
                .then_some(ms)
        }
        Some(v) => {
            let ms = v.as_i64()?;
            (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
                .contains(&ms)
                .then_some(ms)
        }
        None => None,
    }
}

/// 逐请求 span 判定：chat（Copilot/VS Code）或 model_stream（CodeBuddy）。
/// 汇总 span（invoke_agent / codebuddy_code.interaction）与 model_request
/// 跳过（官方防双计）。
fn per_request(name: &str) -> bool {
    matches!(name, "chat" | "model_stream") || name.starts_with("chat ")
}

fn span_identity(value: &serde_json::Value, span: &str) -> String {
    match value
        .get("traceId")
        .or_else(|| value.get("trace_id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(trace) => serde_json::to_string(&(trace, span)).expect("string tuple serialization"),
        None => span.to_string(),
    }
}

fn is_summary(name: &str) -> bool {
    matches!(
        name,
        "invoke_agent" | "codebuddy_code.interaction" | "model_request"
    )
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    scan_with_byte_budget(target, stored, limits, now_ms, None)
}

/// A configuration check samples bounded file bytes without advancing stored state.
pub fn scan_with_byte_budget(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
    max_bytes: Option<u64>,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<OtelParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = OtelParseContext::default();
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let cursor = restore_cursor(
        stored,
        target.generation,
        target.rescan || context.policy_version != 3,
    );
    let read = crate::adapters::jsonl::read_jsonl_with_byte_budget(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
        max_bytes,
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
        let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if is_summary(name) {
            if !context.skipped_names.contains(&name.to_string()) {
                context.skipped_names.push(name.to_string());
            }
            continue; // 汇总 span：官方警告不得与逐请求求和。
        }
        if !per_request(name) {
            continue; // 非用量 span（execute_tool/日志/指标行等）。
        }
        // The native file mixes spans, metrics and logs. CLIENT is numeric 2 in
        // the SDK export (OTLP uses 3); reject explicitly non-CLIENT chat spans.
        if name.starts_with("chat")
            && value.get("kind").is_some_and(|k| {
                k.as_i64() != Some(2)
                    && k.as_str() != Some("CLIENT")
                    && k.as_str() != Some("SPAN_KIND_CLIENT")
            })
        {
            continue;
        }
        // token：gen_ai.*（Copilot/VS Code）或无前缀 usage.*（CodeBuddy）。
        let input = attr_u64(&value, &["gen_ai.usage.input_tokens", "usage.input_tokens"]);
        let output = attr_u64(
            &value,
            &["gen_ai.usage.output_tokens", "usage.output_tokens"],
        );
        let cache_read = attr_u64(
            &value,
            &[
                "gen_ai.usage.cache_read.input_tokens",
                "usage.cache_read_input_tokens",
            ],
        );
        let cache_write = attr_u64(
            &value,
            &[
                "gen_ai.usage.cache_creation.input_tokens",
                "usage.cache_creation_input_tokens",
            ],
        );
        let reasoning = attr_u64(
            &value,
            &[
                "gen_ai.usage.reasoning.output_tokens",
                "gen_ai.usage.reasoning_tokens",
                "usage.reasoning_tokens",
            ],
        );
        if [
            input.is_none(),
            output.is_none(),
            cache_read.is_none(),
            cache_write.is_none(),
            reasoning.is_none(),
        ]
        .iter()
        .any(|v| *v)
        {
            diagnostics.push(diag(
                "token_shape_deviation",
                line.number,
                "a usage attribute carries an out-of-range value; record skipped",
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
        let Some(occurred_ms) = start_ms(&value) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                line.number,
                "startTime missing/implausible; record skipped",
            ));
            continue;
        };
        // spanId 双拼写容错（文档未逐字给出键名）。
        let span_id = value
            .get("spanId")
            .or_else(|| value.get("span_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if span_id.is_empty() {
            diagnostics.push(diag(
                "span_id_missing",
                line.number,
                "span record without span ID; skipped",
            ));
            continue;
        }
        let model = attr_str(
            &value,
            &[
                "gen_ai.request.model",
                "gen_ai.response.model",
                "model_name",
                "request.model",
            ],
        )
        .map(str::to_string);
        // TTFT 三键单位不同（文档证据）：copilot_chat.* 是毫秒、
        // gen_ai.response.time_to_first_chunk 是秒（可含小数）、
        // response.time_to_first_token（agentlens）单位未标。
        // 已标单位的键按文档换算；量级启发（>1e4 视为毫秒）只用于未标单位的键。
        let ttft_ms = {
            let documented_ms =
                attr_u64(&value, &["copilot_chat.time_to_first_token"]).unwrap_or(None);
            let documented_s = attr_f64(&value, &["gen_ai.response.time_to_first_chunk"]);
            let unmarked = attr_u64(&value, &["response.time_to_first_token"]).unwrap_or(None);
            if let Some(ms) = documented_ms {
                Some(ms)
            } else if let Some(s) = documented_s {
                Some((s * 1000.0).round() as i64)
            } else {
                unmarked.map(|v| {
                    if v > 10_000 {
                        v
                    } else {
                        v.saturating_mul(1000)
                    }
                })
            }
        };
        // span status（OTel Status.code）：ERROR 不能把失败调用当成功入账；
        // JSON 形 "STATUS_CODE_ERROR" 或枚举数值 2。
        let error_status = match value.pointer("/status/code") {
            Some(serde_json::Value::String(s))
                if s == "STATUS_CODE_ERROR" || s.eq_ignore_ascii_case("error") =>
            {
                Some("error".to_string())
            }
            Some(serde_json::Value::Number(n)) if n.as_i64() == Some(2) => {
                Some("error".to_string())
            }
            _ => None,
        };
        let copilot = agent_of(&value) == "vscode-copilot-chat";
        let total = if copilot {
            input.zip(output).and_then(|(i, o)| i.checked_add(o))
        } else {
            None
        };
        let uncached = if copilot {
            input
                .zip(cache_read)
                .zip(cache_write)
                .and_then(|((i, r), w)| i.checked_sub(r)?.checked_sub(w))
                .filter(|v| *v >= 0)
        } else {
            None
        };
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: uncached,
                input_cache_read: cache_read,
                input_cache_write: cache_write,
                input_total: input,
                output_total: output,
                output_reasoning: reasoning,
                total_tokens: total,
                source_total: None,
            },
            // 在场桶必须标 Reported：全 Unknown 会在 ingest 校验
            // （domain.rs 值与质量一致性）被拒，事件无法入账。
            crate::domain::TokenQuality {
                input_uncached: crate::domain::FieldQuality::Derived,
                total_tokens: crate::domain::FieldQuality::Derived,
                input_cache_read: crate::domain::FieldQuality::Reported,
                input_cache_write: crate::domain::FieldQuality::Reported,
                input_total: crate::domain::FieldQuality::Reported,
                output_total: crate::domain::FieldQuality::Reported,
                output_reasoning: crate::domain::FieldQuality::Reported,
                ..Default::default()
            },
            Vec::new(),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("otel:{}", span_identity(&value, span_id)),
            record_kind: RecordKind::ModelCall,
            schema_version: OTEL_FORMAT_VERSION.to_string(),
            parser_version: OTEL_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: Some(format!("otel-span:{}", span_identity(&value, span_id))),
            attempt_id: None,
            session_id: attr_str(
                &value,
                &[
                    "copilot_chat.chat_session_id",
                    "gen_ai.conversation.id",
                    "copilot_chat.session_id",
                    "gen_ai.session.id",
                ],
            )
            .map(str::to_string),
            parent_session_id: attr_str(&value, &["copilot_chat.parent_chat_session_id"])
                .map(str::to_string),
            host_application: (agent_of(&value) == "vscode-copilot-chat")
                .then(|| "vscode".to_string()),
            agent: agent_of(&value).to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceStart,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: model,
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Corrected,
            source_revision: None,
            error_status,
            duration_ms: None,
            ttft_ms,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
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
    if matches!(status, ScanStatus::Complete) && diagnostics.is_empty() {
        context.policy_version = 3;
    }
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
    fn start_time_pair_and_epoch() {
        let record: serde_json::Value =
            serde_json::from_str(r#"{"startTime": [1780000000, 500000000]}"#).unwrap();
        assert_eq!(start_ms(&record), Some(1_780_000_000_500));
        let record: serde_json::Value =
            serde_json::from_str(r#"{"startTime": 1780000000500}"#).unwrap();
        assert_eq!(start_ms(&record), Some(1_780_000_000_500));
    }

    #[test]
    fn attribute_shapes_bare_and_otlp() {
        let record: serde_json::Value = serde_json::from_str(
            r#"{"name":"chat","spanId":"ab","attributes":{"gen_ai.usage.input_tokens":100,
               "gen_ai.usage.cache_read.input_tokens":{"intValue":40},
               "copilot_chat.time_to_first_token":7298}}"#,
        )
        .unwrap();
        assert_eq!(
            attr_u64(&record, &["gen_ai.usage.input_tokens"]),
            Some(Some(100))
        );
        assert_eq!(
            attr_u64(&record, &["gen_ai.usage.cache_read.input_tokens"]),
            Some(Some(40))
        );
        assert_eq!(attr_u64(&record, &["usage.input_tokens"]), Some(None));
        assert_eq!(
            attr_str(&record, &["gen_ai.request.model", "model_name"]),
            None
        );
    }

    #[test]
    fn span_classification() {
        assert!(per_request("chat"));
        assert!(per_request("model_stream"));
        assert!(is_summary("invoke_agent"));
        assert!(is_summary("codebuddy_code.interaction"));
        assert!(is_summary("model_request"));
        assert!(!per_request("execute_tool"));
    }
}
