//! Kiro kiro-cli data.sqlite3 格式实现（`sqlite_v1`，kiro-cli-sqlite-1）。
//!
//! 格式证据（tokscale 1d9a939 sessions/kiro.rs:1491,1777-1805；闭源，本机未安装）：
//! - `~/.local/share/kiro-cli/data.sqlite3`（macOS 备选
//!   ~/Library/Application Support/kiro-cli/data.sqlite3）`conversations_v2`
//!   表：`SELECT key, conversation_id, value FROM conversations_v2`（key=cwd）。
//! - `value` JSON：`history[]{user, assistant, request_metadata}`；
//!   `request_metadata`（毫秒时间戳）：`request_start_timestamp_ms`/
//!   `stream_end_timestamp_ms`、input=`input_tokens|uncached_input_tokens|
//!   input_token_count`、output=`output_tokens|output_token_count`、
//!   cache_read=`cache_read_input_tokens|cache_read_tokens|cache_read`、
//!   cache_write=`cache_write_input_tokens|cache_write_tokens|
//!   cache_creation_input_tokens|cache_write`、reasoning=`reasoning_tokens|
//!   reasoning_token_count|thinking_tokens`、request_count=`request_count|
//!   user_turn_request_count|total_request_count`（嵌套 token_usage|usage
//!   同形，平铺优先）。
//! - 无模型字段（第三方证据未见）⇒ 模型维度 Unknown，如实标注。
//! - 会话级 `user_turn_metadata.usage_info[]{value,unit:"credit"}` 是计价
//!   单位：不映射。
//! - 与 CLI 载体（~/.kiro/sessions/cli）的交叉重叠无证据 ⇒ 两实例分列 +
//!   限制标注（真实样本后补对账）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use rusqlite::{Connection, OpenFlags};
use std::time::Duration;

use super::KIRO_SQLITE_FORMAT_VERSION;

pub const KIRO_SQLITE_PARSER_VERSION: &str = "kiro-cli-sqlite-1";
pub const MAX_ENTRIES_PER_ROUND: i64 = 50_000;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct SqliteCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
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

fn ms_field(value: Option<&serde_json::Value>) -> Option<i64> {
    let n = value?.as_i64()?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&n)
        .then_some(n)
}

/// 别名组取值（平铺层优先命中即返回；嵌套 token_usage|usage 同形回退）。
fn alias(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<Option<i64>> {
    let pick = |source: &serde_json::Map<String, serde_json::Value>| -> Option<Option<i64>> {
        for key in keys {
            match source.get(*key) {
                None => continue,
                Some(v) => {
                    let n = v.as_i64()?;
                    return Some(
                        (0..=crate::domain::MAX_TOKEN_VALUE)
                            .contains(&n)
                            .then_some(n),
                    );
                }
            }
        }
        None // 该层无命中别名。
    };
    if let Some(hit) = pick(obj) {
        return Some(hit);
    }
    for nested_key in ["token_usage", "usage"] {
        if let Some(nested) = obj.get(nested_key).and_then(|v| v.as_object()) {
            if let Some(hit) = pick(nested) {
                return Some(hit);
            }
        }
    }
    Some(None)
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // 只读连接（小库；busy 时短暂重试由 busy_timeout 覆盖，不做暂存副本——
    // kiro-cli 库写频低，证据未显示 WAL 高竞争；失败保留旧结果由框架处理）。
    let conn = Connection::open_with_flags(
        &target.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(CoreError::Sqlite)?;
    conn.busy_timeout(Duration::from_millis(150))
        .map_err(CoreError::Sqlite)?;
    let columns: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(conversations_v2)")
            .map_err(CoreError::Sqlite)?;
        let mut rows = stmt.query([]).map_err(CoreError::Sqlite)?;
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            if let Ok(name) = row.get::<_, String>(1) {
                out.push(name);
            }
        }
        out
    };
    for required in ["key", "conversation_id", "value"] {
        if !columns.iter().any(|c| c == required) {
            return Ok(ScanOutcome {
                status: ScanStatus::Pending,
                cursor: None,
                parse_context: None,
                events: Vec::new(),
                aggregates: Vec::new(),
                diagnostics: vec![diag(
                    "schema_deviation",
                    "conversations_v2",
                    &format!("missing required column {required:?}; fail closed"),
                )],
                lines_read: 0,
                records_seen: 0,
                reconciliations: Vec::new(),
                health: "degraded".to_string(),
            });
        }
    }
    let mut stmt = conn
        .prepare("SELECT conversation_id, value FROM conversations_v2 LIMIT ?1")
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map([MAX_ENTRIES_PER_ROUND], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(CoreError::Sqlite)?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    for row in rows {
        let (conversation_id, value_json) = row?;
        records_seen += 1;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&value_json) else {
            diagnostics.push(diag(
                "value_unparseable",
                &conversation_id,
                "conversations_v2.value is not JSON; row skipped",
            ));
            continue;
        };
        let Some(history) = value.get("history").and_then(|v| v.as_array()) else {
            continue;
        };
        for (index, entry) in history.iter().enumerate() {
            let Some(meta) = entry.get("request_metadata").and_then(|v| v.as_object()) else {
                continue;
            };
            let input = alias(
                meta,
                &["input_tokens", "uncached_input_tokens", "input_token_count"],
            );
            let output = alias(meta, &["output_tokens", "output_token_count"]);
            let cache_read = alias(
                meta,
                &["cache_read_input_tokens", "cache_read_tokens", "cache_read"],
            );
            let cache_write = alias(
                meta,
                &[
                    "cache_write_input_tokens",
                    "cache_write_tokens",
                    "cache_creation_input_tokens",
                    "cache_write",
                ],
            );
            let reasoning = alias(
                meta,
                &[
                    "reasoning_tokens",
                    "reasoning_token_count",
                    "thinking_tokens",
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
                    &format!("{conversation_id}:{index}"),
                    "a request_metadata alias carries an out-of-range value; entry skipped",
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
            if input.is_none()
                && output.is_none()
                && cache_read.is_none()
                && cache_write.is_none()
                && reasoning.is_none()
            {
                continue;
            }
            let occurred_ms = ms_field(meta.get("request_start_timestamp_ms"))
                .or_else(|| ms_field(meta.get("stream_end_timestamp_ms")));
            let Some(occurred_ms) = occurred_ms else {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    &format!("{conversation_id}:{index}"),
                    "request timestamps missing/implausible; entry skipped",
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
                    output_reasoning: reasoning,
                    total_tokens: None,
                    source_total: None,
                },
                crate::domain::TokenQuality::default(),
                Vec::new(),
            );
            events.push(crate::domain::EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: format!("kiro-sqlite:{conversation_id}:{index}"),
                record_kind: crate::domain::RecordKind::ModelCall,
                schema_version: KIRO_SQLITE_FORMAT_VERSION.to_string(),
                parser_version: KIRO_SQLITE_PARSER_VERSION.to_string(),
                parse_basis: Some(crate::domain::VersionBasis::KnownVersion),
                origin_call_id: None,
                attempt_id: None,
                session_id: Some(conversation_id.clone()),
                parent_session_id: None,
                host_application: Some("kiro-cli".to_string()),
                agent: "kiro".to_string(),
                call_category: crate::domain::CallCategory::Primary,
                occurred_at_ms: occurred_ms,
                observed_at_ms: Some(now_ms),
                source_time: Some(occurred_ms.to_string()),
                time_basis: crate::domain::TimeBasis::SourceStart,
                interval_start_ms: ms_field(meta.get("request_start_timestamp_ms")),
                interval_end_ms: ms_field(meta.get("stream_end_timestamp_ms")),
                provider_id: None,
                model_raw: None,
                model_canonical: None,
                model_attribution: crate::domain::ModelAttribution::Unknown,
                usage: mapped.usage,
                quality: mapped.quality,
                lifecycle: crate::domain::Lifecycle::Final,
                source_revision: None,
                error_status: None,
                duration_ms: None,
                ttft_ms: None,
                attribution_status: crate::domain::AttributionStatus::Verified,
                exclusion_reason: None,
                cost: None,
            });
        }
    }
    let hit_cap = records_seen as i64 >= MAX_ENTRIES_PER_ROUND;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(SqliteCursor {
            generation: target.generation,
            offset: 0,
        })?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}
