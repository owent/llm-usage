//! Xum session-usage.json 格式实现（`usage_v1`，文档级
//! xum-session-usage-doc-1）。
//!
//! 格式依据：官方 npm 0.30.0 与对应 coder/xum 提交
//! 81b0b744db6e27a4416f3596d70bf88529171caf；默认/网关对照真实本地模型样本。
//! 原始文档依据：tokscale 固定提交 1d9a939 sessions/mux.rs。
//! - 路径 `~/.mux/sessions/<workspaceId>/session-usage.json`（clients.rs:543-552）；
//!   新根 ~/.xum/sessions；XUM_ROOT/MUX_ROOT 与 RUN_SESSION_ROOT 支持 sessions 发现。
//! - JSON：`version`（u32）、`byModel`（map，键 `"provider:model"`，splitn(2,':')）、
//!   每模型 `{ input:{tokens,cost_usd}, cached:{...}, cacheCreate:{...},
//!   output:{...}, reasoning:{...} }`、`lastRequest{ model, timestamp }`
//!   （timestamp 毫秒；缺失回退 mtime）（mux.rs:12-43）。
//! - 语义：**会话级累计、按模型一行**（IntervalAggregate，不展开伪造逐次）；
//!   cost_usd 不映射（会话级累计成本与逐次成本单位不同，见能力表）。
//! - dedup `xum:<workspaceId>:<model_key>`（mux.rs:96-102 同形）。
//! - displayUsage.ts 将 input 归一为未缓存输入、output 排除推理，缺字段默认零。
//!   正文本输出加已知推理；推理未知时为下界。总输入/完整总量不派生。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read;

pub const XUM_PARSER_VERSION: &str = "xum-session-usage-2";
/// 单文件有界读取上限。
pub const XUM_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;

/// Reconstruct only the old parser's misplaced input, zero defaults and text-only output.
/// Every candidate hashes the complete aggregate, keeping revision and other fields fixed.
pub(crate) fn prior_display_hashes(input: &SourceAggregateInput) -> Vec<String> {
    if input.scope != AggregateScope::Session
        || !input.scope_key.starts_with("xum:")
        || input.usage.input_total.is_some()
        || input.quality.input_total != Q::Unknown
    {
        return Vec::new();
    }
    let mut prior = input.clone();
    prior.usage.input_total = input.usage.input_uncached;
    prior.quality.input_total = input.quality.input_uncached;
    prior.usage.input_uncached = None;
    prior.quality.input_uncached = Q::Unknown;
    if input.quality.output_total == Q::Derived {
        let Some(text) = input
            .usage
            .output_total
            .zip(input.usage.output_reasoning)
            .and_then(|(total, reasoning)| total.checked_sub(reasoning))
            .filter(|n| *n > 0)
        else {
            return Vec::new();
        };
        prior.usage.output_total = Some(text);
        prior.quality.output_total = Q::Reported;
    }
    let mut hashes = std::collections::BTreeSet::new();
    for mask in 0..32 {
        let mut candidate = prior.clone();
        let fields = [
            (
                &mut candidate.usage.input_total,
                &mut candidate.quality.input_total,
            ),
            (
                &mut candidate.usage.input_cache_read,
                &mut candidate.quality.input_cache_read,
            ),
            (
                &mut candidate.usage.input_cache_write,
                &mut candidate.quality.input_cache_write,
            ),
            (
                &mut candidate.usage.output_total,
                &mut candidate.quality.output_total,
            ),
            (
                &mut candidate.usage.output_reasoning,
                &mut candidate.quality.output_reasoning,
            ),
        ];
        let mut valid = true;
        for (bit, (value, quality)) in fields.into_iter().enumerate() {
            if mask & (1 << bit) != 0 {
                if value.is_some() || *quality != Q::Unknown {
                    valid = false;
                    break;
                }
                *value = Some(0);
                *quality = Q::Reported;
            }
        }
        if valid {
            hashes.insert(crate::identity::content_hash(&candidate));
        }
    }
    hashes.into_iter().collect()
}

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
    crate::adapters::run_policy::checked_file(&target.path)?
        .take(XUM_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&bytes) {
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
        crate::adapters::run_policy::check()?;
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
        // The display carrier loses whether a zero was reported or initialized.
        let input = input.filter(|n| *n > 0);
        let cached = cached.filter(|n| *n > 0);
        let cache_create = cache_create.filter(|n| *n > 0);
        let output = output.filter(|n| *n > 0);
        let reasoning = reasoning.filter(|n| *n > 0);
        let (output_total, output_quality) = match (output, reasoning) {
            (Some(text), Some(reasoning)) => {
                let total = text
                    .checked_add(reasoning)
                    .filter(|n| *n <= MAX_REASONABLE_TOKEN);
                if total.is_none() {
                    diagnostics.push(diag(
                        "token_shape_deviation",
                        &position,
                        "text plus reasoning exceeds token cap; output total kept unknown",
                    ));
                }
                (total, total.map(|_| Q::Derived).unwrap_or(Q::Unknown))
            }
            (Some(text), None) => {
                diagnostics.push(diag("xum_output_incomplete", &position,
                    "positive text output is a lower bound; reasoning zero/missing cannot certify complete output"));
                (Some(text), Q::Reported)
            }
            (None, _) => (None, Q::Unknown),
        };
        let usage = TokenUsage {
            input_uncached: input,
            input_cache_read: cached,
            input_cache_write: cache_create,
            input_total: None,
            output_total,
            output_reasoning: reasoning,
            total_tokens: None,
            source_total: None,
        };
        let quality = TokenQuality {
            input_uncached: input.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            input_cache_read: cached.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            input_cache_write: cache_create.map(|_| Q::Reported).unwrap_or(Q::Unknown),
            output_total: output_quality,
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
    let health = if diagnostics
        .iter()
        .all(|d| d.code == "xum_output_incomplete")
    {
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
