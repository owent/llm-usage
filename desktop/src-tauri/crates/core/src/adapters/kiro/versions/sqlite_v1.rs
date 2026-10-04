//! Kiro kiro-cli data.sqlite3 格式实现（`sqlite_v1`，kiro-cli-sqlite-1）。
//!
//! 格式依据（tokscale 1d9a939 sessions/kiro.rs:1491,1777-1805；闭源，本机未安装）：
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
//! - 模型字段未确认（第三方解析器未见）⇒ 模型未知，标注 Unknown。
//! - 会话级 `user_turn_metadata.usage_info[]{value,unit:"credit"}` 是计价
//!   单位：不映射。
//! - 与 CLI 载体（~/.kiro/sessions/cli）的重叠关系尚未核验 ⇒ 两实例分列 +
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
    /// 本轮在稳定排序中的已处理行数；读到末页后归零以复查可变行。
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

/// 别名组取值（平铺层优先；嵌套 token_usage|usage 同形回退）。
/// 返回语义：Some(Some(v))=命中有效值；Some(None)=全部别名缺失；
/// None=命中别名但值类型错误/越界（格式偏离，调用方跳过该条目）。
/// 类型不符的别名不遮蔽同层后续有效别名（多版本兼容分支必须可达）。
fn alias(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<Option<i64>> {
    let pick = |source: &serde_json::Map<String, serde_json::Value>| -> Option<Option<i64>> {
        let mut type_deviation = false;
        for key in keys {
            match source.get(*key) {
                None => continue,
                Some(v) => {
                    let Some(n) = v.as_i64() else {
                        type_deviation = true;
                        continue;
                    };
                    if !(0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) {
                        return None; // 越界=格式偏离
                    }
                    return Some(Some(n));
                }
            }
        }
        if type_deviation {
            None
        } else {
            Some(None)
        }
    };
    match pick(obj) {
        Some(Some(v)) => return Some(Some(v)),
        None => return None,
        Some(None) => {}
    }
    for nested_key in ["token_usage", "usage"] {
        if let Some(nested) = obj.get(nested_key).and_then(|v| v.as_object()) {
            match pick(nested) {
                Some(Some(v)) => return Some(Some(v)),
                None => return None,
                Some(None) => {}
            }
        }
    }
    Some(None)
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // 只读连接（小库；busy 时短暂重试由 busy_timeout 覆盖，不做暂存副本——
    // kiro-cli 库写频低，尚未观测到 WAL 高竞争；失败保留旧结果由框架处理）。
    let conn = Connection::open_with_flags(
        &target.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(CoreError::Sqlite)?;
    crate::adapters::run_policy::install_sqlite_control(&conn)?;
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
        crate::adapters::run_policy::check()?;
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
    let offset = if target.rescan {
        0
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<SqliteCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .map(|c| c.offset)
            .unwrap_or(0)
    };
    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, value FROM conversations_v2
             ORDER BY conversation_id, key LIMIT ?1 OFFSET ?2",
        )
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(
            rusqlite::params![
                MAX_ENTRIES_PER_ROUND + 1,
                i64::try_from(offset).unwrap_or(i64::MAX)
            ],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(CoreError::Sqlite)?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    for row in rows {
        crate::adapters::run_policy::check()?;
        // 行级容错：单行类型错误不中止整轮（SQLite 动态类型）。
        let (conversation_id, value_json) = match row {
            Ok(r) => r,
            Err(e) => {
                records_seen += 1;
                diagnostics.push(diag(
                    "row_read_failed",
                    "conversations_v2",
                    &format!("row read failed: {e}; row skipped"),
                ));
                continue;
            }
        };
        records_seen += 1;
        let Ok(value) =
            crate::adapters::run_policy::json_from_str::<serde_json::Value>(&value_json)
        else {
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
            // 区间端点先后校验：倒置不报区间（起止时间自相矛盾）。
            let (interval_start_ms, interval_end_ms) = {
                let s = ms_field(meta.get("request_start_timestamp_ms"));
                let e = ms_field(meta.get("stream_end_timestamp_ms"));
                match (s, e) {
                    (Some(s), Some(e)) if e < s => {
                        diagnostics.push(diag(
                            "interval_inverted",
                            &format!("{conversation_id}:{index}"),
                            "stream end before request start; interval dropped",
                        ));
                        (None, None)
                    }
                    _ => (s, e),
                }
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
                // 在场桶必须标 Reported：全 Unknown 会在 ingest 校验
                // （domain.rs 值与质量一致性）被拒，事件无法入账。
                crate::domain::TokenQuality {
                    input_cache_read: crate::domain::FieldQuality::Reported,
                    input_cache_write: crate::domain::FieldQuality::Reported,
                    input_total: crate::domain::FieldQuality::Reported,
                    output_total: crate::domain::FieldQuality::Reported,
                    output_reasoning: crate::domain::FieldQuality::Reported,
                    ..Default::default()
                },
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
                interval_start_ms,
                interval_end_ms,
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
    // 多取一行判定是否还有更多（恰好 MAX 行不误报 BudgetExhausted）。
    let hit_cap = records_seen > MAX_ENTRIES_PER_ROUND as u64;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(SqliteCursor {
            generation: target.generation,
            offset: if hit_cap {
                offset.saturating_add(records_seen)
            } else {
                0
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn alias_flat_priority_and_nested_fallback() {
        // 平铺优先。
        let o = obj(serde_json::json!({"input_tokens": 5, "usage": {"input_tokens": 9}}));
        assert_eq!(alias(&o, &["input_tokens"]), Some(Some(5)));
        // 平铺缺失回退嵌套 token_usage/usage。
        let o = obj(serde_json::json!({"token_usage": {"input_tokens": 7}}));
        assert_eq!(alias(&o, &["input_tokens"]), Some(Some(7)));
        // 全缺失。
        let o = obj(serde_json::json!({"other": 1}));
        assert_eq!(alias(&o, &["input_tokens"]), Some(None));
    }

    #[test]
    fn alias_type_error_does_not_shadow_valid_alias() {
        // 首个别名类型错误（字符串）不能遮蔽同层后续有效别名。
        let o = obj(serde_json::json!({"input_tokens": "many", "input_token_count": 42}));
        assert_eq!(
            alias(&o, &["input_tokens", "input_token_count"]),
            Some(Some(42))
        );
        // 全部别名都类型错误 ⇒ 格式偏离（None），不是"缺失"。
        let o = obj(serde_json::json!({"input_tokens": "many"}));
        assert_eq!(alias(&o, &["input_tokens"]), None);
    }

    #[test]
    fn alias_out_of_range_is_deviation_not_missing() {
        let o = obj(serde_json::json!({"input_tokens": -3}));
        assert_eq!(alias(&o, &["input_tokens"]), None);
        let o = obj(serde_json::json!({"input_tokens": i64::MAX}));
        assert_eq!(alias(&o, &["input_tokens"]), None);
    }
}
