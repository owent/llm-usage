//! Goose sessions.db 格式实现（`usage_ledger_v1`，goose-usage-ledger-1）。
//!
//! 格式证据（aaif-goose/goose 固定源码 a701bb1756f0c6a49a7dbc10ac8a90f94dd24bd1，
//! 官方源码核验；本机未安装、无真实样本）：
//! - `usage_ledger`（迁移 15）：每 provider 响应一行 INSERT（append-only，
//!   AUTOINCREMENT id 单调）；`created_timestamp` Unix **秒**；列
//!   session_id/model/input_tokens/output_tokens/total_tokens/cache_read_tokens/
//!   cache_write_tokens/cost REAL/cost_source/is_compaction。
//! - `cost_source`：`provider_reported` ⇒ Reported（micro-USD）；`estimated`
//!   与 `carried_forward`（补账差额行）⇒ cost 不映射（估算/混合来源不采信）；
//!   carried_forward 行的 **token 计入**（它是产品自己的累计补账，非推断）。
//! - `is_compaction=1` ⇒ Auxiliary（auto-compaction 后的 retained-context
//!   基线调用）；`input_tokens` 含 cache 读/写（官方 token_usage.rs 口径）⇒
//!   input_uncached 减法派生。
//! - 旧库（schema &lt; 15）无 usage_ledger：按 `sessions.accumulated_*` 会话级
//!   聚合兜底（IntervalAggregate；非 accumulated 单次列是最后快照，不用）；
//!   created_at/updated_at 为 SQLite 文本 UTC（`YYYY-MM-DD HH:MM:SS`）。
//!   两载体互斥：有 ledger 的库不再读 accumulated（防双计）。
//! - fork/copy 不复制用量；subagent 会话独立行，全表求和不双计（官方口径）。
//!
//! 增量合同（append-only 表）：游标 = 已处理最大 ledger id（行只 INSERT 不
//! UPDATE，无重叠窗需求）；schema 指纹变化 ⇒ 重置全量重读（事件键
//! goose:ledger:&lt;id&gt; upsert 幂等）；单轮 50,000 行上限。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::goose::common::{
    map_goose_ledger, open_source_db, short_probe, GooseLedgerUsage, StagingLimits,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

use super::GOOSE_FORMAT_VERSION;

pub const GOOSE_PARSER_VERSION: &str = "goose-usage-ledger-1";
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GooseCursor {
    generation: i64,
    /// 已处理的最大 ledger id（append-only 表；旧库兜底模式恒 0）。
    last_ledger_id: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GooseParseContext {
    /// schema 指纹（表集合投影；变化 ⇒ 框架按 parser_version 逻辑重扫）。
    schema_fingerprint: Option<String>,
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

fn seconds_to_ms(secs: i64) -> Option<i64> {
    let ms = secs.checked_mul(1000)?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

/// SQLite 文本 UTC 时间（datetime('now') 形 `YYYY-MM-DD HH:MM:SS`，或 RFC3339）。
fn text_ts_ms(value: Option<&str>) -> Option<i64> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(ts) = raw.parse::<jiff::Timestamp>() {
        return Some(ts.as_millisecond());
    }
    let civil: jiff::civil::DateTime = raw.parse().ok()?;
    let zoned = civil.to_zoned(jiff::tz::TimeZone::UTC).ok()?;
    let ms = zoned.timestamp().as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn usd_cost(cost: Option<f64>, cost_source: Option<&str>) -> Option<CostAmount> {
    let amount = cost?;
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    // 只有 provider_reported 的 cost 入账；estimated/carried_forward 不映射。
    if !matches!(cost_source, Some("provider_reported")) {
        return None;
    }
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        kind: CostKind::Reported,
        price_version: None,
        billing_scope: None,
    })
}

fn column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({table})")) else {
        return Vec::new();
    };
    let Ok(mut rows) = stmt.query([]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        if let Ok(name) = row.get::<_, String>(1) {
            out.push(name);
        }
    }
    out
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let db = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let ledger_columns = column_names(db.conn(), "usage_ledger");
    let has_ledger = !ledger_columns.is_empty();
    let fingerprint = format!("ledger={has_ledger};cols={}", ledger_columns.join(","));
    let mut context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<GooseParseContext>(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan || context.schema_fingerprint.as_deref() != Some(fingerprint.as_str()) {
        context = GooseParseContext::default();
        context.schema_fingerprint = Some(fingerprint.clone());
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let last_id =
        if target.rescan || context.schema_fingerprint.as_deref() == Some(fingerprint.as_str()) {
            stored
                .cursor
                .as_ref()
                .and_then(|v| serde_json::from_value::<GooseCursor>(v.clone()).ok())
                .filter(|c| c.generation == target.generation)
                .map(|c| c.last_ledger_id)
                .unwrap_or(0)
        } else {
            0
        };

    if has_ledger {
        let mut stmt = db.conn().prepare(
            "SELECT id, session_id, created_timestamp, model, input_tokens, output_tokens,
                    total_tokens, cache_read_tokens, cache_write_tokens, cost, cost_source,
                    is_compaction
             FROM usage_ledger WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;
        let rows = stmt.query_map([last_id, MAX_ROWS_PER_ROUND], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<f64>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<i64>>(11)?,
            ))
        })?;
        let mut events = Vec::new();
        let mut diagnostics = Vec::new();
        let mut records_seen: u64 = 0;
        let mut max_id = last_id;
        for row in rows {
            let (
                id,
                session_id,
                created_s,
                model,
                input,
                output,
                total,
                cache_read,
                cache_write,
                cost,
                cost_source,
                is_compaction,
            ) = row?;
            records_seen += 1;
            max_id = max_id.max(id);
            let Some(occurred_ms) = created_s.and_then(seconds_to_ms) else {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    &format!("ledger:{id}"),
                    "created_timestamp missing/implausible; row skipped",
                ));
                continue;
            };
            let usage = GooseLedgerUsage {
                input_tokens: input,
                output_tokens: output,
                total_tokens: total,
                cache_read_tokens: cache_read,
                cache_write_tokens: cache_write,
            };
            if usage.input_tokens.is_none()
                && usage.output_tokens.is_none()
                && usage.total_tokens.is_none()
                && usage.cache_read_tokens.is_none()
                && usage.cache_write_tokens.is_none()
            {
                continue;
            }
            let mapped = map_goose_ledger(&usage);
            events.push(EventInput {
                source_instance_id: target.instance_id.clone(),
                source_record_key: format!("goose:ledger:{id}"),
                record_kind: RecordKind::ModelCall,
                schema_version: GOOSE_FORMAT_VERSION.to_string(),
                parser_version: GOOSE_PARSER_VERSION.to_string(),
                parse_basis: Some(VersionBasis::KnownVersion),
                origin_call_id: None,
                attempt_id: None,
                session_id: Some(session_id),
                parent_session_id: None,
                host_application: None,
                agent: "goose".to_string(),
                call_category: if is_compaction.unwrap_or(0) == 1 {
                    CallCategory::Auxiliary
                } else {
                    CallCategory::Primary
                },
                occurred_at_ms: occurred_ms,
                observed_at_ms: Some(now_ms),
                source_time: created_s.map(|s| s.to_string()),
                time_basis: TimeBasis::SourceCompletion,
                interval_start_ms: None,
                interval_end_ms: None,
                provider_id: None,
                model_raw: model,
                model_canonical: None,
                model_attribution: ModelAttribution::RequestField,
                usage: mapped.usage,
                quality: mapped.quality,
                lifecycle: Lifecycle::Final,
                source_revision: None,
                error_status: None,
                duration_ms: None,
                ttft_ms: None,
                attribution_status: AttributionStatus::Verified,
                exclusion_reason: None,
                cost: usd_cost(cost, cost_source.as_deref()),
            });
        }
        let hit_cap = records_seen as i64 >= MAX_ROWS_PER_ROUND;
        Ok(ScanOutcome {
            status: if hit_cap {
                ScanStatus::BudgetExhausted
            } else {
                ScanStatus::Complete
            },
            cursor: Some(serde_json::to_value(GooseCursor {
                generation: target.generation,
                last_ledger_id: max_id,
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
    } else {
        // 旧库兜底：sessions.accumulated_* 会话级聚合（无逐请求表）。
        let mut stmt = db.conn().prepare(
            "SELECT id, accumulated_total_tokens, accumulated_input_tokens,
                    accumulated_output_tokens, created_at, updated_at
             FROM sessions LIMIT ?1",
        )?;
        let rows = stmt.query_map([MAX_ROWS_PER_ROUND], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut aggregates = Vec::new();
        let mut diagnostics = Vec::new();
        let mut records_seen: u64 = 0;
        for row in rows {
            let (id, total, input, output, created_at, updated_at) = row?;
            records_seen += 1;
            if input.is_none() && output.is_none() && total.is_none() {
                continue;
            }
            let Some(end_ms) = text_ts_ms(updated_at.as_deref()) else {
                diagnostics.push(diag(
                    "timestamp_unparseable",
                    &format!("session:{id}"),
                    "updated_at unparseable; session skipped",
                ));
                continue;
            };
            let start_ms = text_ts_ms(created_at.as_deref()).filter(|s| *s <= end_ms);
            let usage = crate::domain::TokenUsage {
                input_total: input,
                output_total: output,
                source_total: total,
                ..Default::default()
            };
            aggregates.push(SourceAggregateInput {
                instance_id: target.instance_id.clone(),
                scope: AggregateScope::Session,
                scope_key: format!("goose:session:{id}"),
                interval_start_ms: start_ms,
                interval_end_ms: end_ms,
                interval_end_inclusive: false,
                usage,
                quality: crate::domain::TokenQuality {
                    input_total: crate::domain::FieldQuality::Reported,
                    output_total: crate::domain::FieldQuality::Reported,
                    source_total: crate::domain::FieldQuality::Reported,
                    ..Default::default()
                },
                reported_call_count: None,
                coverage: Coverage::Exclusive,
                duplicate_of: None,
                time_basis: TimeBasis::Uncertain,
                source_revision: Some(end_ms),
            });
        }
        let hit_cap = records_seen as i64 >= MAX_ROWS_PER_ROUND;
        Ok(ScanOutcome {
            status: if hit_cap {
                ScanStatus::BudgetExhausted
            } else {
                ScanStatus::Complete
            },
            cursor: Some(serde_json::to_value(GooseCursor {
                generation: target.generation,
                last_ledger_id: 0,
            })?),
            parse_context: Some(serde_json::to_value(&context)?),
            events: Vec::new(),
            aggregates,
            diagnostics,
            lines_read: records_seen,
            records_seen,
            reconciliations: Vec::new(),
            health: "active".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_timestamps_sqlite_and_rfc3339() {
        assert_eq!(
            text_ts_ms(Some("2026-09-29 05:00:00")),
            Some(1_790_658_000_000)
        );
        assert_eq!(
            text_ts_ms(Some("2026-09-29T05:00:00Z")),
            Some(1_790_658_000_000)
        );
        assert_eq!(text_ts_ms(Some("")), None);
        assert_eq!(text_ts_ms(None), None);
    }

    #[test]
    fn cost_source_gate() {
        assert!(usd_cost(Some(0.5), Some("provider_reported")).is_some());
        assert!(usd_cost(Some(0.5), Some("estimated")).is_none());
        assert!(usd_cost(Some(0.5), Some("carried_forward")).is_none());
        assert!(usd_cost(Some(-1.0), Some("provider_reported")).is_none());
    }
}
