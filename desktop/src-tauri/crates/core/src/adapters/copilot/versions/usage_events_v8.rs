//! Copilot CLI assistant_usage_events 格式实现（`usage_events_v8`，
//! assistant-usage-events-v8）。
//!
//! 格式证据（M0 m0-agent-fixtures.md 1.0.73 实读取证 + 2026-09-29 本机
//! schema_version=8 真实数据复核，36/36 行语义核验通过）：
//! - 库：`~/.copilot/session-store.db`（WAL）；`assistant_usage_events`
//!   id INTEGER PK（append-only）、session_id、turn_index、model、
//!   input/output/cache_read/cache_write/reasoning_tokens、duration_ms、
//!   time_to_first_token_ms（REAL ms）、created_at（ISO8601 字符串）。
//! - **input_tokens = 未缓存 + cache_read + cache_write** ⇒ uncached 派生
//!   （真实数据 0 违例）；reasoning 与 output 包含关系未证 ⇒ 并列报告。
//! - request_multiplier（实测恒 27.0）是 premium 付费倍率、total_nano_aiu 是
//!   nano AIU 计量：均非 token，不入账。
//! - events.jsonl 事件流无逐次 token（M0）：不采集。
//!
//! 增量合同（append-only 表）：游标 = 已处理最大 id；单轮 50,000 行；
//! 事件键 copilot:usage:&lt;id&gt; upsert 幂等。

use crate::adapters::copilot::common::{
    map_copilot, open_source_db, read_schema_version, short_probe, CopilotUsage, StagingLimits,
};
use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

pub const COPILOT_PARSER_VERSION: &str = "copilot-usage-events-8";
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CopilotCursor {
    generation: i64,
    /// 已处理的最大 usage 事件 id（append-only 表）。
    last_id: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CopilotParseContext {
    #[serde(default)]
    version_basis: Option<VersionBasis>,
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

fn iso_ms(value: Option<&str>) -> Option<i64> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let ts: jiff::Timestamp = raw.parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn opt_col(value: Option<i64>) -> (Option<i64>, bool) {
    match value {
        Some(n) if (0..=crate::domain::MAX_TOKEN_VALUE).contains(&n) => (Some(n), false),
        // 越界（负/超上限）：桶置未知并由调用方记诊断，不静默丢桶。
        Some(_) => (None, true),
        None => (None, false),
    }
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<CopilotParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan {
        context = CopilotParseContext::default();
    }
    let last_id = if target.rescan {
        0
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<CopilotCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .map(|c| c.last_id)
            .unwrap_or(0)
    };
    let db = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    // 版本依据按库内 schema_version 表如实推导（与 detect 同规则，V30）：
    // 不硬编码 KnownVersion——schema 变化后回退 LatestFallback 并让
    // framework 的 latest_fallback 保护与 active_compat 标记生效。
    let found_version = read_schema_version(db.conn());
    context.version_basis = Some(super::select(found_version.as_deref()).basis);
    let event_basis = context
        .version_basis
        .unwrap_or(VersionBasis::LatestFallback);
    let mut stmt = db.conn().prepare(
        "SELECT id, session_id, turn_index, model, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, reasoning_tokens,
                duration_ms, time_to_first_token_ms, created_at
         FROM assistant_usage_events WHERE id > ?1 ORDER BY id LIMIT ?2",
    )?;
    let mut rows = stmt.query([last_id, MAX_ROWS_PER_ROUND])?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    let mut max_id = last_id;
    loop {
        // 行级错误跳过不中止整轮（单行损坏不拖垮本轮已处理数据）。
        let Some(row) = (match rows.next() {
            Ok(row) => row,
            Err(_) => {
                diagnostics.push(diag(
                    "row_read_failed",
                    &format!("usage:after:{max_id}"),
                    "row iteration failed mid-scan; round kept partial results",
                ));
                break;
            }
        }) else {
            break;
        };
        let id: i64 = match row.get(0) {
            Ok(id) => id,
            Err(_) => {
                // 无法取得 id ⇒ 游标不能越过本行，下轮重试；保留本轮已处理结果。
                diagnostics.push(diag(
                    "row_read_failed",
                    &format!("usage:after:{max_id}"),
                    "row id unreadable; remaining rows held for next round",
                ));
                break;
            }
        };
        records_seen += 1;
        max_id = max_id.max(id);
        type Row12 = (
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<f64>,
            Option<f64>,
            Option<String>,
        );
        let cols = || -> rusqlite::Result<Row12> {
            Ok((
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<f64>>(9)?,
                row.get::<_, Option<f64>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        };
        let Ok((
            session_id,
            _turn_index,
            model,
            input,
            output,
            cache_read,
            cache_write,
            reasoning,
            duration_ms,
            ttft_ms,
            created_at,
        )) = cols()
        else {
            // 列类型确定损坏（重试不会自愈）：跳过本行、游标越过并记诊断。
            diagnostics.push(diag(
                "row_read_failed",
                &format!("usage:{id}"),
                "row columns undecodable; row skipped",
            ));
            continue;
        };
        let (input, bad_input) = opt_col(input);
        let (output, bad_output) = opt_col(output);
        let (cache_read, bad_read) = opt_col(cache_read);
        let (cache_write, bad_write) = opt_col(cache_write);
        let (reasoning, bad_reasoning) = opt_col(reasoning);
        let mut bad_fields = Vec::new();
        if bad_input {
            bad_fields.push("input_tokens");
        }
        if bad_output {
            bad_fields.push("output_tokens");
        }
        if bad_read {
            bad_fields.push("cache_read_tokens");
        }
        if bad_write {
            bad_fields.push("cache_write_tokens");
        }
        if bad_reasoning {
            bad_fields.push("reasoning_tokens");
        }
        if !bad_fields.is_empty() {
            diagnostics.push(diag(
                "token_shape_deviation",
                &format!("usage:{id}"),
                &format!(
                    "columns {} out of range; kept unknown",
                    bad_fields.join(",")
                ),
            ));
        }
        let usage = CopilotUsage {
            input_tokens: input,
            cached_input_tokens: cache_read,
            cache_creation_input_tokens: cache_write,
            output_tokens: output,
            reasoning_tokens: reasoning,
        };
        if usage.input_tokens.is_none()
            && usage.output_tokens.is_none()
            && usage.cached_input_tokens.is_none()
            && usage.cache_creation_input_tokens.is_none()
            && usage.reasoning_tokens.is_none()
        {
            continue;
        }
        let Some(occurred_ms) = iso_ms(created_at.as_deref()) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("usage:{id}"),
                "created_at missing/implausible; row skipped",
            ));
            continue;
        };
        let mapped = map_copilot(&usage);
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("copilot:usage:{id}"),
            record_kind: RecordKind::ModelCall,
            schema_version: super::COPILOT_FORMAT_VERSION.to_string(),
            parser_version: COPILOT_PARSER_VERSION.to_string(),
            parse_basis: Some(event_basis),
            origin_call_id: None,
            attempt_id: None,
            session_id,
            parent_session_id: None,
            host_application: None,
            agent: "copilot-cli".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: created_at,
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: Some("github-copilot".to_string()),
            model_raw: model,
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: None,
            duration_ms: duration_ms.map(|d| d.round() as i64).filter(|d| *d >= 0),
            ttft_ms: ttft_ms.map(|d| d.round() as i64).filter(|d| *d >= 0),
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
    }
    let hit_cap = records_seen as i64 >= MAX_ROWS_PER_ROUND;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(CopilotCursor {
            generation: target.generation,
            last_id: max_id,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
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

    #[test]
    fn iso8601_parse() {
        assert_eq!(
            iso_ms(Some("2026-08-03T14:25:56.483Z")),
            Some(1_785_767_156_483)
        );
        assert_eq!(iso_ms(Some("")), None);
        assert_eq!(iso_ms(None), None);
    }

    #[test]
    fn input_includes_cache_derivation() {
        let mapped = map_copilot(&CopilotUsage {
            input_tokens: Some(36_004),
            cached_input_tokens: Some(0),
            cache_creation_input_tokens: Some(36_002),
            output_tokens: Some(133),
            reasoning_tokens: Some(20),
        });
        assert_eq!(mapped.usage.input_uncached, Some(2));
        assert_eq!(mapped.usage.input_cache_write, Some(36_002));
        assert_eq!(mapped.usage.output_reasoning, Some(20));
        assert_eq!(mapped.usage.total_tokens, Some(36_137));
    }
}
