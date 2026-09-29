//! AtomCode 会话元数据格式实现（`meta_turns_v1`，atomcode-meta-turns-1）。
//!
//! 格式证据（AtomGit atomgit_atomcode/atomcode 固定源码
//! e4215f733eeba4cede553e28f9b559e6b3dc34ef（GitHub 镜像同 SHA 核验）；
//! 本机未安装、无真实样本）：
//! - 路径：`$ATOMCODE_HOME`（默认 ~/.atomcode）/sessions/&lt;project_hash&gt;/
//!   &lt;id&gt;.meta（SessionMeta JSON）；旧版单文件 &lt;id&gt;.json（messages +
//!   turn_stats，LegacyCatalogMeta）同构读取。
//! - SessionMeta（manager.rs:376-430）：`v`（schema 版）、`id`、`working_dir`、
//!   `created_at`/`updated_at`（epoch **毫秒**）、`turn_stats[]`、
//!   `detached_model_usage[]`、`detached_unattributed_tokens`。
//! - TurnStat（manager.rs:634-668）：`turn_id`、`round_count`（LLM 往返数）、
//!   `duration_ms`、`total_tokens`（本轮末次请求 prompt+completion，非累计）、
//!   `model_usage[]`：`{provider_id, model_id, tokens: TokenBreakdown}`。
//!   **TurnStat 无时间戳** ⇒ 会话区间聚合（interval 语义），round_count 合计
//!   作 reported_call_count（不虚构逐次）。
//! - TokenBreakdown（manager.rs:681-687 + usage_provider.rs:64-71）：
//!   `input` = prompt − cached（**非缓存输入**）、`cached_input` = min(cached,
//!   prompt)（缓存命中部分）、`output` = completion。无 cache_write 桶
//!   （kernel TokenUsage{prompt,completion,cached} 三列）⇒ input_total =
//!   input + cached_input（派生）、input_uncached=input（直报）、
//!   input_cache_read=cached_input（直报）。
//! - <id>.jsonl 逐消息转录、.snapshot/.todos/.rewind 等不读（usage 在 .meta）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

pub const ATOMCODE_PARSER_VERSION: &str = "atomcode-meta-turns-1";
pub const ATOMCODE_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

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

fn ms_field(value: Option<&serde_json::Value>) -> Option<i64> {
    let n = value?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&n)
        .then_some(n)
}

fn bucket(value: Option<&serde_json::Value>, key: &str) -> Option<i64> {
    let n = value?.get(key)?.as_i64()?;
    (0..=crate::domain::MAX_TOKEN_VALUE)
        .contains(&n)
        .then_some(n)
}

/// 按模型累计桶与调用数。
#[derive(Default, Clone, Copy)]
struct ModelAccum {
    input: i64,
    cached_input: i64,
    output: i64,
    rounds: i64,
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    if target.probe.len > ATOMCODE_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "session meta exceeds the 16 MiB cap; cursor held",
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
        ATOMCODE_MAX_FILE_BYTES + 1,
    )
    .read_to_end(&mut bytes)?;
    let document: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "meta_unparseable",
                    "session meta does not parse; retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let session_id = document
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let start_ms = ms_field(document.get("created_at"));
    let end_ms = ms_field(document.get("updated_at")).or(start_ms);
    let diagnostics = Vec::new();
    let Some(end_ms) = end_ms else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "timestamp_unparseable",
                "created_at/updated_at missing/implausible; fail closed",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let mut records_seen: u64 = 0;
    // turn_stats[] + detached_model_usage[] → 按 (provider, model) 累计。
    let mut acc: std::collections::BTreeMap<(String, String), ModelAccum> = Default::default();
    if let Some(turns) = document.get("turn_stats").and_then(|v| v.as_array()) {
        for turn in turns {
            records_seen += 1;
            let rounds = turn
                .get("round_count")
                .and_then(|v| v.as_i64())
                .filter(|r| *r >= 0)
                .unwrap_or(0);
            if let Some(models) = turn.get("model_usage").and_then(|v| v.as_array()) {
                for model in models {
                    let key = (
                        model
                            .get("provider_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        model
                            .get("model_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    );
                    let tokens = model.get("tokens");
                    let entry = acc.entry(key).or_default();
                    if let Some(input) = bucket(tokens, "input") {
                        entry.input = entry.input.saturating_add(input);
                    }
                    if let Some(cached) = bucket(tokens, "cached_input") {
                        entry.cached_input = entry.cached_input.saturating_add(cached);
                    }
                    if let Some(output) = bucket(tokens, "output") {
                        entry.output = entry.output.saturating_add(output);
                    }
                    entry.rounds = entry.rounds.saturating_add(rounds);
                }
            }
        }
    }
    if let Some(detached) = document
        .get("detached_model_usage")
        .and_then(|v| v.as_array())
    {
        for model in detached {
            records_seen += 1;
            let key = (
                model
                    .get("provider_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                model
                    .get("model_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            );
            let tokens = model.get("tokens");
            let entry = acc.entry(key).or_default();
            if let Some(input) = bucket(tokens, "input") {
                entry.input = entry.input.saturating_add(input);
            }
            if let Some(cached) = bucket(tokens, "cached_input") {
                entry.cached_input = entry.cached_input.saturating_add(cached);
            }
            if let Some(output) = bucket(tokens, "output") {
                entry.output = entry.output.saturating_add(output);
            }
        }
    }
    if acc.is_empty() {
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
            records_seen,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    }
    let mut aggregates = Vec::new();
    for ((provider, model), entry) in acc {
        if entry.input == 0 && entry.cached_input == 0 && entry.output == 0 {
            continue;
        }
        // 官方口径：input = prompt − cached、cached_input = min(cached, prompt)。
        let usage = TokenUsage {
            input_uncached: Some(entry.input),
            input_cache_read: Some(entry.cached_input),
            input_cache_write: None,
            input_total: entry.input.checked_add(entry.cached_input),
            output_total: Some(entry.output),
            output_reasoning: None,
            total_tokens: entry
                .input
                .checked_add(entry.cached_input)
                .and_then(|i| i.checked_add(entry.output)),
            source_total: None,
        };
        let quality = TokenQuality {
            input_uncached: Q::Reported,
            input_cache_read: Q::Reported,
            input_cache_write: Q::Unknown,
            input_total: Q::Derived,
            output_total: Q::Reported,
            total_tokens: Q::Derived,
            ..Default::default()
        };
        let scope_model = if model.is_empty() { "unknown" } else { &model };
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: format!("atomcode:session:{session_id}:{provider}/{scope_model}"),
            interval_start_ms: start_ms,
            interval_end_ms: end_ms,
            interval_end_inclusive: false,
            usage,
            quality,
            reported_call_count: (entry.rounds > 0).then_some(entry.rounds),
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        });
    }
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
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_bounds() {
        assert_eq!(
            ms_field(Some(&serde_json::json!(1_790_000_000_000i64))),
            Some(1_790_000_000_000)
        );
        assert_eq!(ms_field(Some(&serde_json::json!(5))), None);
        assert_eq!(ms_field(None), None);
    }
}
