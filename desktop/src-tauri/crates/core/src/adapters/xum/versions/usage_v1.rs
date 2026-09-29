//! Xum session-usage.json 格式实现（`usage_v1`，文档级
//! xum-session-usage-doc-1）。
//!
//! 格式证据（第三方解析器 tokscale 固定提交 1d9a9395418efc6952944b794097935d7d6fa1e8
//! sessions/mux.rs；产品开源仓库 coder/xum；本机未安装、无真实样本）：
//! - 路径 `~/.mux/sessions/<workspaceId>/session-usage.json`（clients.rs:543-552）；
//!   源码未见环境覆盖；产品更名（mux→Xum）需保留旧根发现。
//! - JSON：`version`（u32）、`byModel`（map，键 `"provider:model"`，splitn(2,':')）、
//!   每模型 `{ input:{tokens,cost_usd}, cached:{...}, cacheCreate:{...},
//!   output:{...}, reasoning:{...} }`、`lastRequest{ model, timestamp }`
//!   （timestamp 毫秒；缺失回退 mtime）（mux.rs:12-43）。
//! - 语义：**会话级累计、按模型一行**（IntervalAggregate，不展开伪造逐次）；
//!   cost_usd 之和为该模型会话成本（产品自报 ⇒ Reported，micro-USD）。
//! - dedup `mux:<workspaceId>:<model_key>`（mux.rs:96-102）。
//! - 五桶包含关系未知 ⇒ 并列报告不派生总量（hermes 同型）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;

pub const XUM_PARSER_VERSION: &str = "xum-session-usage-1";
/// 单文件有界读取上限。
pub const XUM_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

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

fn workspace_id_of(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-workspace")
        .to_string()
}

/// 桶对象 {tokens, cost_usd}：tokens 必须在场且非负有界；cost_usd 可选。
fn bucket(value: Option<&serde_json::Value>) -> Option<(Option<i64>, Option<f64>)> {
    let Some(obj) = value else {
        return Some((None, None));
    };
    if !obj.is_object() {
        return None;
    }
    let tokens = match obj.get("tokens") {
        None => None,
        Some(v) => {
            let n = v.as_i64()?;
            if !(0..=MAX_REASONABLE_TOKEN).contains(&n) {
                return None;
            }
            Some(n)
        }
    };
    let cost = obj
        .get("cost_usd")
        .and_then(|v| v.as_f64())
        .filter(|c| c.is_finite() && *c >= 0.0);
    Some((tokens, cost))
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let workspace_id = workspace_id_of(&target.path);
    let mut aggregates = Vec::new();
    let mut diagnostics = Vec::new();
    if target.probe.len > XUM_MAX_FILE_BYTES {
        diagnostics.push(diag(
            "file_exceeds_size_cap",
            "document",
            "session-usage.json exceeds the 16 MiB cap; cursor held for controlled retry",
        ));
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates,
            diagnostics,
            lines_read: 0,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&target.path)?
        .take(XUM_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            // 半程写入：游标不推进，下轮确定性重试。
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates,
                diagnostics: vec![diag(
                    "session_usage_unparseable",
                    "document",
                    "session-usage.json does not parse (mid-write or corrupt); retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let Some(models) = document.get("byModel").and_then(|v| v.as_object()) else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates,
            diagnostics: vec![diag(
                "session_schema_deviation",
                "document",
                "byModel is not a JSON object; fail closed",
            )],
            lines_read: 1,
            records_seen: 0,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let end_ms = document
        .pointer("/lastRequest/timestamp")
        .and_then(|v| v.as_i64())
        .filter(|&ts| (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(&ts))
        .unwrap_or(target.probe.mtime_ms);
    let mut records_seen: u64 = 0;
    for (model_key, entry) in models {
        records_seen += 1;
        let position = format!("byModel[{model_key}]");
        let (input, _input_cost) = match bucket(entry.get("input")) {
            Some(v) => v,
            None => {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    &position,
                    "input bucket carries a negative/out-of-range value; model skipped",
                ));
                continue;
            }
        };
        let (cached, _) = match bucket(entry.get("cached")) {
            Some(v) => v,
            None => {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    &position,
                    "cached bucket invalid; model skipped",
                ));
                continue;
            }
        };
        let (cache_create, _) = match bucket(entry.get("cacheCreate")) {
            Some(v) => v,
            None => {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    &position,
                    "cacheCreate bucket invalid; model skipped",
                ));
                continue;
            }
        };
        let (output, _) = match bucket(entry.get("output")) {
            Some(v) => v,
            None => {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    &position,
                    "output bucket invalid; model skipped",
                ));
                continue;
            }
        };
        let (reasoning, _) = match bucket(entry.get("reasoning")) {
            Some(v) => v,
            None => {
                diagnostics.push(diag(
                    "token_shape_deviation",
                    &position,
                    "reasoning bucket invalid; model skipped",
                ));
                continue;
            }
        };
        if input.is_none()
            && cached.is_none()
            && cache_create.is_none()
            && output.is_none()
            && reasoning.is_none()
        {
            continue;
        }
        let usage = TokenUsage {
            input_uncached: None,
            input_cache_read: cached,
            input_cache_write: cache_create,
            input_total: input,
            output_total: output,
            output_reasoning: reasoning,
            total_tokens: None,
            source_total: None,
        };
        let quality = TokenQuality {
            input_cache_read: cached.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            input_cache_write: cache_create.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            input_total: input.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            output_total: output.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            output_reasoning: reasoning.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            ..Default::default()
        };
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: format!("xum:{workspace_id}:{model_key}"),
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
        });
    }
    let health = if diagnostics.is_empty() {
        "active".to_string()
    } else {
        "degraded".to_string()
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
        aggregates,
        diagnostics,
        lines_read: 1,
        records_seen,
        reconciliations: Vec::new(),
        health,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_parsing() {
        let (tokens, cost) =
            bucket(Some(&serde_json::json!({"tokens": 10, "cost_usd": 0.5}))).unwrap();
        assert_eq!(tokens, Some(10));
        assert_eq!(cost, Some(0.5));
        assert_eq!(bucket(None), Some((None, None)));
        assert!(bucket(Some(&serde_json::json!({"tokens": -1}))).is_none());
    }

    #[test]
    fn workspace_from_path() {
        let p = std::path::Path::new("/home/u/.mux/sessions/ws-123/session-usage.json");
        assert_eq!(workspace_id_of(p), "ws-123");
    }
}
