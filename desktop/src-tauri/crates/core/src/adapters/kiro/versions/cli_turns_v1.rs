//! Kiro CLI 会话头格式实现（`cli_turns_v1`，kiro-cli-turns-1）。
//!
//! 格式证据（tokscale 1d9a939 sessions/kiro.rs:48-113；闭源，本机未安装）：
//! - `~/.kiro/sessions/cli/*.json` 会话头：`session_id`、`cwd`、
//!   `session_state.rts_model_state.model_info.{model_id, context_window_tokens}`、
//!   `session_state.conversation_metadata.user_turn_metadatas[]`。
//! - turn 字段：`input_token_count`/`output_token_count`/
//!   `cache_read_input_token_count`/`cache_write_input_token_count`/
//!   `end_timestamp`/`total_request_count`/`metering_usage[]{value,unit:"credit"}`。
//! - **估算不采纳**（模块头 "ESTIMATED, not measured"）：Auto agent 常记 0、
//!   字节/4 折算与 context_window 差额路径全部跳过 ⇒ 只采至少一个计数
//!   字段在场的 turn（全缺失/全 0 跳过记诊断）。
//! - `end_timestamp` 单位第三方证据未标明：按量级判别（≥1e11 视为毫秒，
//!   否则秒×1000；与 tokscale crush 同型判别），越域拒绝。
//! - metering credit 是计价单位（0.04 USD/credit 为第三方换算，不采信）⇒
//!   不映射 cost。
//! - 同 stem `.jsonl` 是消息转录（Prompt/AssistantMessage/ToolResults），
//!   无 usage，不读。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

use super::KIRO_FORMAT_VERSION;

pub const KIRO_CLI_PARSER_VERSION: &str = "kiro-cli-turns-1";
pub const KIRO_MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
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

/// 量级判别：≥1e11 视为毫秒；其余按秒换算（越域拒绝）。
fn ts_to_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let n = value?.as_f64()?;
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    let ms = if n >= 1e11 { n } else { n * 1000.0 };
    if !((crate::domain::MIN_PLAUSIBLE_MS as f64)..=4_102_444_800_000.0).contains(&ms) {
        return None;
    }
    Some(ms.round() as i64)
}

/// 计数取值：Ok(Some(v))=有效计数；Ok(None)=字段缺失（未知）；
/// Err=形状偏离（区分非整数与越界，诊断归因用）。
fn count(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<i64>, &'static str> {
    match obj.get(key) {
        None => Ok(None),
        Some(v) => match v.as_i64() {
            Some(n) if (0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) => Ok(Some(n)),
            Some(_) => Err("out-of-range"),
            None => Err("non-integer"),
        },
    }
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut diagnostics = Vec::new();
    let mut events = Vec::new();
    if target.probe.len > KIRO_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "document",
                "session header exceeds the 32 MiB cap; cursor held",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    std::io::Read::take(
        &mut std::fs::File::open(&target.path)?,
        KIRO_MAX_FILE_BYTES + 1,
    )
    .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events,
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "session_unparseable",
                    "document",
                    "session header does not parse; retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let doc_session_id = document.get("session_id").and_then(|v| v.as_str());
    // 缺 session_id 时去重键用文件身份兜底：多个缺 id 文件的键不能都塌缩成
    // kiro:unknown:turn:N 而互相吞并；事件 session 维度不虚构，保持 None。
    let session_key = doc_session_id.unwrap_or(target.file_identity.as_str());
    let model = document
        .pointer("/session_state/rts_model_state/model_info/model_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let Some(turns) = document
        .pointer("/session_state/conversation_metadata/user_turn_metadatas")
        .and_then(|v| v.as_array())
    else {
        return Ok(ScanOutcome {
            status: ScanStatus::Complete,
            cursor: Some(serde_json::to_value(WholeFileCursor {
                generation: target.generation,
                offset: bytes.len() as u64,
                line_number: 1,
            })?),
            parse_context: None,
            events,
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 1,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    };
    let mut records_seen: u64 = 0;
    let mut zero_turns: u64 = 0;
    for (index, turn) in turns.iter().enumerate() {
        records_seen += 1;
        let Some(obj) = turn.as_object() else {
            diagnostics.push(diag(
                "record_shape_deviation",
                &format!("turn:{index}"),
                "turn entry is not an object; turn skipped",
            ));
            continue;
        };
        let fields = [
            "input_token_count",
            "output_token_count",
            "cache_read_input_token_count",
            "cache_write_input_token_count",
        ];
        let mut values = [None, None, None, None];
        let mut deviation: Option<String> = None;
        for (slot, key) in fields.iter().enumerate() {
            match count(obj, key) {
                Ok(v) => values[slot] = v,
                Err(kind) => {
                    deviation = Some(format!("field {key} carries a {kind} value"));
                    break;
                }
            }
        }
        if let Some(reason) = deviation {
            diagnostics.push(diag(
                "token_shape_deviation",
                &format!("turn:{index}"),
                &format!("{reason}; turn skipped"),
            ));
            continue;
        }
        let [input, output, cache_read, cache_write] = values;
        // 全 0/缺失：Auto agent 常记 0（第三方证据），无真实计数 ⇒ 不采。
        if input.unwrap_or(0) == 0
            && output.unwrap_or(0) == 0
            && cache_read.unwrap_or(0) == 0
            && cache_write.unwrap_or(0) == 0
        {
            zero_turns += 1;
            continue;
        }
        let Some(occurred_ms) = ts_to_ms(obj.get("end_timestamp")) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("turn:{index}"),
                "end_timestamp missing/implausible; turn skipped",
            ));
            continue;
        };
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: None,
                input_cache_read: cache_read,
                input_cache_write: cache_write,
                input_total: input,
                output_total: output,
                output_reasoning: None,
                total_tokens: None,
                source_total: None,
            },
            // 在场桶必须标 Reported：TokenQuality::default() 全 Unknown 会在
            // ingest 校验（domain.rs:421 值与质量不一致）被拒，整批事件无法入账。
            crate::domain::TokenQuality {
                input_cache_read: crate::domain::FieldQuality::Reported,
                input_cache_write: crate::domain::FieldQuality::Reported,
                input_total: crate::domain::FieldQuality::Reported,
                output_total: crate::domain::FieldQuality::Reported,
                ..Default::default()
            },
            Vec::new(),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("kiro:{session_key}:turn:{index}"),
            // 按 turn 聚合的真实计数（非逐请求）。
            record_kind: RecordKind::ModelCall,
            schema_version: KIRO_FORMAT_VERSION.to_string(),
            parser_version: KIRO_CLI_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: doc_session_id.map(str::to_string),
            parent_session_id: None,
            host_application: None,
            agent: "kiro".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw: model.clone(),
            model_canonical: None,
            model_attribution: ModelAttribution::StructuredChange,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
        // total_request_count（turn 内请求数）无事件字段承载：不入账，见能力表限制。
    }
    if zero_turns > 0 {
        diagnostics.push(diag(
            "zero_count_turns_skipped",
            session_key,
            &format!("{zero_turns} turns carry only zero/absent counts (Auto agent default); not adopted as evidence"),
        ));
    }
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnitude_disambiguation() {
        assert_eq!(
            ts_to_ms(Some(&serde_json::json!(1_790_000_000))),
            Some(1_790_000_000_000)
        );
        assert_eq!(
            ts_to_ms(Some(&serde_json::json!(1_790_000_000_000i64))),
            Some(1_790_000_000_000)
        );
        assert_eq!(ts_to_ms(Some(&serde_json::json!(0))), None);
    }
}
