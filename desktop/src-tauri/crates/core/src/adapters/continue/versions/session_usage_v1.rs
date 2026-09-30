//! Continue 会话文件格式实现（`session_usage_v1`，continue-session-usage-1）。
//!
//! 格式证据（continuedev/continue 固定源码 5522c6f44ca0ac3528b37244818fbfa39b5af470；
//! 官方源码核验；本机未安装、无真实样本）：
//! - 路径：`$CONTINUE_GLOBAL_DIR`（默认 ~/.continue）/sessions/&lt;uuidv4&gt;.json；
//!   整写 JSON 对象 `{sessionId, title, workspaceDirectory, history[], usage?}`。
//! - **顶层 `usage` 仅 CLI 写入**（extensions/cli session.ts:149-178
//!   trackUsage 每请求累加并立即持久化）：`promptTokens`/`completionTokens`/
//!   `promptTokensDetails?{cachedTokens?, cacheWriteTokens?}`/`totalCost`。
//!   VS Code/JetBrains GUI 会话不写 usage（内存流）⇒ 无 usage 字段 = 无数据。
//!   dev_data/devdata.sqlite 的 tokens_generated 是本地 tokenizer 估算，
//!   **不采纳**（估算路径一律不采信）。
//! - usage 是**会话累计非逐 turn 明细**（逐请求 API 值只在内存）⇒
//!   IntervalAggregate（Session 级），不展开伪造逐次。
//! - 缓存包含关系混合 provider（OpenAI cached ⊆ prompt；Anthropic 分立）：
//!   来源不区分 ⇒ hermes 同型并列报告，不派生总量。
//! - 会话无内嵌时间戳（sessions.json 索引有 dateCreated 但格式随端而变：
//!   core 写毫秒字符串、CLI 写 ISO）⇒ 区间端点用文件 mtime（Uncertain），
//!   不读 sessions.json（避免解析不稳定索引）。
//! - JetBrains 插件硬编码 ~/.continue（不读 CONTINUE_GLOBAL_DIR）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{FieldQuality as Q, TimeBasis, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use std::io::Read as _;

pub const CONTINUE_PARSER_VERSION: &str = "continue-session-usage-1";
pub const CONTINUE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

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
    path.file_stem()?.to_str().map(str::to_string)
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    if target.probe.len > CONTINUE_MAX_FILE_BYTES {
        return Ok(ScanOutcome {
            status: ScanStatus::LineTooLong,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "file_exceeds_size_cap",
                "session JSON exceeds the 64 MiB cap; cursor held",
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
        CONTINUE_MAX_FILE_BYTES + 1,
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
                    "session_unparseable",
                    "session JSON does not parse (mid-write?); retry next round",
                )],
                lines_read: 1,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "active".to_string(),
            });
        }
    };
    let Some(session_id) = document
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| session_id_of(&target.path))
    else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "session_schema_deviation",
                "sessionId missing; not a Continue session file",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    // GUI 会话无 usage 字段：无数据（不补零），正常完成。
    let Some(usage) = document.get("usage").and_then(|v| v.as_object()) else {
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
            diagnostics: Vec::new(),
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    };
    let get = |key: &str| -> Option<Option<i64>> {
        match usage.get(key) {
            None => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                Some(
                    (0..=crate::domain::MAX_TOKEN_VALUE)
                        .contains(&n)
                        .then_some(n),
                )
            }
        }
    };
    let (Some(prompt), Some(completion)) = (get("promptTokens"), get("completionTokens")) else {
        return Ok(ScanOutcome {
            status: ScanStatus::Pending,
            cursor: None,
            parse_context: None,
            events: Vec::new(),
            aggregates: Vec::new(),
            diagnostics: vec![diag(
                "token_shape_deviation",
                "usage token field carries an out-of-range value; fail closed",
            )],
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "degraded".to_string(),
        });
    };
    let details = usage.get("promptTokensDetails").and_then(|v| v.as_object());
    let mut detail_deviation = false;
    let detail = |key: &str, deviation: &mut bool| {
        match details.and_then(|d| d.get(key)) {
            None => None,
            Some(v) => match v.as_i64() {
                Some(n) if (0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) => Some(n),
                // 子字段越界：与主字段同口径记诊断（不 fail closed，置未知）。
                _ => {
                    *deviation = true;
                    None
                }
            },
        }
    };
    let cached = detail("cachedTokens", &mut detail_deviation);
    let cache_write = detail("cacheWriteTokens", &mut detail_deviation);
    if prompt.is_none() && completion.is_none() {
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
            diagnostics: Vec::new(),
            lines_read: 1,
            records_seen: 1,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        });
    }
    let usage_tokens = TokenUsage {
        input_uncached: None,
        input_cache_read: cached,
        input_cache_write: cache_write,
        input_total: prompt,
        output_total: completion,
        output_reasoning: None,
        total_tokens: None,
        source_total: None,
    };
    let quality = TokenQuality {
        input_cache_read: cached.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_cache_write: cache_write.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        input_total: prompt.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        output_total: completion.map(|_| Q::Reported).unwrap_or(Q::Unknown),
        ..Default::default()
    };
    // mtime 作修订号前过 plausibility；interval_end_ms 保持原值由聚合层
    // 校验（implausible 会被拒并记诊断，fail closed 方向）。
    let mtime_ms = (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&target.probe.mtime_ms)
        .then_some(target.probe.mtime_ms);
    let mut diagnostics = Vec::new();
    if detail_deviation {
        diagnostics.push(diag(
            "token_shape_deviation",
            "promptTokensDetails sub-field out of range; kept unknown",
        ));
    }
    let aggregate = SourceAggregateInput {
        instance_id: target.instance_id.clone(),
        scope: AggregateScope::Session,
        scope_key: format!("continue:session:{session_id}"),
        interval_start_ms: None,
        interval_end_ms: target.probe.mtime_ms,
        interval_end_inclusive: true,
        usage: usage_tokens,
        quality,
        reported_call_count: None,
        coverage: Coverage::Exclusive,
        duplicate_of: None,
        time_basis: TimeBasis::Uncertain,
        source_revision: mtime_ms,
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
    fn session_id_from_path() {
        assert_eq!(
            session_id_of(std::path::Path::new(
                "/home/u/.continue/sessions/0b6c3a2e-1111-2222-3333-444455556666.json"
            ))
            .as_deref(),
            Some("0b6c3a2e-1111-2222-3333-444455556666")
        );
    }
}
