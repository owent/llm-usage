//! Droid `<uuid>.settings.json` 格式实现（`settings_doc1`，文档级
//! droid-settings-doc-1）。
//!
//! 格式依据（第三方解析器 tokscale 固定提交
//! 1d9a9395418efc6952944b794097935d7d6fa1e8 sessions/droid.rs:16-39；
//! 闭源产品 Factory.ai，本机未安装、无真实样本）：
//! - 路径 `~/.factory/sessions/<uuid>.settings.json`（clients.rs:473-482）；
//!   源码未见环境覆盖/多根。
//! - JSON：`model`、`providerLock`、`providerLockTimestamp`（RFC3339）、
//!   `tokenUsage{ inputTokens, outputTokens, cacheCreationTokens,
//!   cacheReadTokens, thinkingTokens }`。
//! - 语义：**会话级累计快照**（IntervalAggregate，不展开伪造逐次）；
//!   同名 `<uuid>.jsonl` 转录不含 token（tokscale 按字节权重分摊属估计，
//!   **不采纳**——M8 估算路径一律不采信）；无费用字段。
//! - 端点 = max(mtime, providerLockTimestamp)（tokscale droid.rs:186-193
//!   单行回退同款）；模型名规范化（去 `custom:` 前缀与 `[...]`）不沿用
//!   （按原文入账）。
//! - dedup `droid:<uuid>`。
//! - 五桶包含关系未知 ⇒ 并列报告不派生总量（hermes 同型）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;

pub const DROID_PARSER_VERSION: &str = "droid-settings-doc1";
/// 单文件有界读取上限。
pub const DROID_MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct WholeFileCursor {
    generation: i64,
    offset: u64,
    #[allow(dead_code)]
    line_number: u64,
}

fn diag(code: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some("document".to_string()),
        message: message.to_string(),
    }
}

fn session_id_of(path: &std::path::Path) -> Option<String> {
    path.file_name()?
        .to_str()?
        .strip_suffix(".settings.json")
        .map(str::to_string)
}

fn rfc3339_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let s: &str = value?.as_str()?;
    let ts: jiff::Timestamp = s.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn bucket(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<Option<i64>> {
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

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let diagnostics = Vec::new();
    if target.probe.len > DROID_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "settings.json exceeds the 8 MiB cap; cursor held for controlled retry",
            )],
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    crate::adapters::run_policy::checked_file(&target.path)?
        .take(DROID_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "settings_unparseable",
                    "settings.json does not parse (mid-write or corrupt); retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let Some(session_id) = session_id_of(&target.path) else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "session_schema_deviation",
                "file name is not <uuid>.settings.json; fail closed",
            )],
            lines_read: 1,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let Some(usage_obj) = document
        .get("tokenUsage")
        .and_then(|v| v.as_object().cloned())
    else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "session_schema_deviation",
                "tokenUsage missing or not an object; fail closed",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let input = bucket(&usage_obj, "inputTokens");
    let output = bucket(&usage_obj, "outputTokens");
    let cache_read = bucket(&usage_obj, "cacheReadTokens");
    let cache_write = bucket(&usage_obj, "cacheCreationTokens");
    let thinking = bucket(&usage_obj, "thinkingTokens");
    if [input, output, cache_read, cache_write, thinking]
        .iter()
        .any(|v| v.is_none())
    {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "token_shape_deviation",
                "a tokenUsage bucket carries a negative/out-of-range value; fail closed",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let (input, output, cache_read, cache_write, thinking) = (
        input.unwrap(),
        output.unwrap(),
        cache_read.unwrap(),
        cache_write.unwrap(),
        thinking.unwrap(),
    );
    if input.is_none()
        && output.is_none()
        && cache_read.is_none()
        && cache_write.is_none()
        && thinking.is_none()
    {
        return Ok(ScanOutcome {
            status: ScanStatus::Complete,
            cursor: Some(serde_json::to_value(WholeFileCursor {
                generation: target.generation,
                offset: bytes.len() as u64,
                line_number: 1,
            })?),
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics,
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    }
    // 端点 = max(mtime, providerLockTimestamp)。
    let end_ms = rfc3339_ms(document.get("providerLockTimestamp"))
        .map(|lock| lock.max(target.probe.mtime_ms))
        .unwrap_or(target.probe.mtime_ms);
    let usage = TokenUsage {
        input_uncached: None,
        input_cache_read: cache_read,
        input_cache_write: cache_write,
        input_total: input,
        output_total: output,
        output_reasoning: thinking,
        total_tokens: None,
        source_total: None,
    };
    let quality = TokenQuality {
        input_cache_read: cache_read.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_write: cache_write.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_total: input.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: output.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_reasoning: thinking.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        ..Default::default()
    };
    let aggregate = SourceAggregateInput {
        instance_id: target.instance_id.clone(),
        scope: AggregateScope::Session,
        scope_key: format!("droid:{session_id}"),
        interval_start_ms: None,
        interval_end_ms: end_ms,
        interval_end_inclusive: true,
        usage,
        quality,
        reported_call_count: None,
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::Uncertain,
        source_revision: Some(end_ms),
    };
    Ok(ScanOutcome {
        status: ScanStatus::Complete,
        cursor: Some(serde_json::to_value(WholeFileCursor {
            generation: target.generation,
            offset: bytes.len() as u64,
            line_number: 1,
        })?),
        parse_context: None,
        events: Vec::new(),
        aggregates: vec![aggregate],
        diagnostics,
        lines_read: 1,
        records_seen: 1,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_from_filename() {
        assert_eq!(
            session_id_of(std::path::Path::new(
                "/home/u/.factory/sessions/6f1c2ab4.settings.json"
            ))
            .as_deref(),
            Some("6f1c2ab4")
        );
        assert_eq!(session_id_of(std::path::Path::new("other.json")), None);
    }

    #[test]
    fn optional_buckets() {
        let obj: serde_json::Map<String, serde_json::Value> =
            crate::adapters::run_policy::json_from_str(r#"{"inputTokens": 5}"#).unwrap();
        assert_eq!(bucket(&obj, "inputTokens"), Some(Some(5)));
        assert_eq!(bucket(&obj, "outputTokens"), Some(None));
        let bad: serde_json::Map<String, serde_json::Value> =
            crate::adapters::run_policy::json_from_str(r#"{"inputTokens": -2}"#).unwrap();
        assert_eq!(bucket(&bad, "inputTokens"), None);
    }
}
