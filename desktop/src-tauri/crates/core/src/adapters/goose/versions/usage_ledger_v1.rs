//! Goose sessions.db implementation usage_ledger_v1; format goose-usage-ledger-1.
//!
//! Source reference: aaif-goose/goose commit a701bb1756f0c6a49a7dbc10ac8a90f94dd24bd1.
//! Native 1.53.0 container CLI/local-model API/DB checks are recorded in the M8 samples.
//! - usage_ledger (migration 15) appends one INSERT per provider response, with
//!   monotonic AUTOINCREMENT id and created_timestamp in Unix seconds. Columns:
//!   session_id/model/input_tokens/output_tokens/total_tokens/cache_read_tokens/
//!   cache_write_tokens/cost REAL/cost_source/is_compaction.
//! - provider_reported cost becomes Reported micro-USD. estimated and
//!   carried_forward cost are excluded because their amounts are estimated or mixed.
//!   Include carried_forward token rows as the product's recorded cumulative corrections.
//! - is_compaction=1 becomes Auxiliary for the retained-context baseline after
//!   compaction. Source token_usage.rs defines input_tokens as including cache reads/writes;
//!   derive input_uncached by subtraction.
//! - Databases before schema 15 have no usage_ledger: fall back to session accumulated_*
//!   interval aggregates. Ignore non-accumulated columns containing only the last snapshot.
//!   created_at/updated_at are SQLite UTC text (YYYY-MM-DD HH:MM:SS).
//!   Read ledger or accumulated values exclusively to prevent duplicate usage.
//! - Source inspection describes fork/copy and separate subagent rows; native checks cover neither.
//!
//! Incremental scanning follows the append-only ledger: remember the greatest processed id.
//! A changed ledger column fingerprint resets scanning. Repeated goose:ledger:<id> keys
//! update the same event. Pages read up to 50,001 rows; an extra row indicates continuation.

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
    /// Greatest processed ledger id; stays zero in legacy aggregate mode.
    last_ledger_id: i64,
    /// Greatest processed legacy session id, in lexical order; an empty value starts at the beginning.
    #[serde(default)]
    last_session_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GooseParseContext {
    /// Ledger presence and column-name fingerprint; changes reset this reader and its stored cursor.
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

/// Parse SQLite UTC text (datetime('now'), YYYY-MM-DD HH:MM:SS) or RFC3339.
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
    // Import cost only when cost_source is provider_reported; exclude estimated/carried_forward.
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
    // Changed columns may change field meaning: reread from the beginning and discard the old cursor.
    let fingerprint_changed = context.schema_fingerprint.as_deref() != Some(fingerprint.as_str());
    if target.rescan || fingerprint_changed {
        context = GooseParseContext::default();
        context.schema_fingerprint = Some(fingerprint.clone());
    }
    context.version_basis = Some(VersionBasis::KnownVersion);
    let stored_cursor = if target.rescan || fingerprint_changed {
        None
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<GooseCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
    };
    let last_id = stored_cursor
        .as_ref()
        .map(|c| c.last_ledger_id)
        .unwrap_or(0);
    let last_session_id = stored_cursor
        .as_ref()
        .map(|c| c.last_session_id.clone())
        .unwrap_or_default();

    if has_ledger {
        let mut stmt = db.conn().prepare(
            "SELECT id, session_id, created_timestamp, model, input_tokens, output_tokens,
                    total_tokens, cache_read_tokens, cache_write_tokens, cost, cost_source,
                    is_compaction
             FROM usage_ledger WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;
        let rows = stmt.query_map([last_id, MAX_ROWS_PER_ROUND + 1], |r| {
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
            // SQLite row type errors produce diagnostics without aborting the remaining rows.
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
            ) = match row {
                Ok(r) => r,
                Err(e) => {
                    records_seen += 1;
                    diagnostics.push(diag(
                        "row_read_failed",
                        "usage_ledger",
                        &format!("row read failed: {e}; row skipped"),
                    ));
                    continue;
                }
            };
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
            let cost_mapped = usd_cost(cost, cost_source.as_deref());
            if usage.input_tokens.is_none()
                && usage.output_tokens.is_none()
                && usage.total_tokens.is_none()
                && usage.cache_read_tokens.is_none()
                && usage.cache_write_tokens.is_none()
                && cost_mapped.is_none()
            {
                // Skip rows with all token fields NULL and no valid provider-reported cost.
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
                cost: cost_mapped,
            });
        }
        let hit_cap = records_seen > MAX_ROWS_PER_ROUND as u64;
        Ok(ScanOutcome {
            status: if hit_cap {
                ScanStatus::BudgetExhausted
            } else {
                ScanStatus::Complete
            },
            cursor: Some(serde_json::to_value(GooseCursor {
                generation: target.generation,
                last_ledger_id: max_id,
                last_session_id: last_session_id.clone(),
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
        // Fall back to session accumulated_* aggregates when there is no per-request ledger.
        // Page by session id when the row limit is reached; clear the cursor after the last page.
        // Legacy cumulative values can change existing sessions, so the next scan must revisit old ids.
        let mut stmt = db.conn().prepare(
            "SELECT id, accumulated_total_tokens, accumulated_input_tokens,
                    accumulated_output_tokens, created_at, updated_at
             FROM sessions WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![&last_session_id, MAX_ROWS_PER_ROUND + 1],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            },
        )?;
        let mut aggregates = Vec::new();
        let mut diagnostics = Vec::new();
        let mut records_seen: u64 = 0;
        let mut max_session_id = last_session_id.clone();
        for row in rows {
            // SQLite row type errors produce diagnostics without aborting the remaining rows.
            let (id, total, input, output, created_at, updated_at) = match row {
                Ok(r) => r,
                Err(e) => {
                    records_seen += 1;
                    diagnostics.push(diag(
                        "row_read_failed",
                        "sessions",
                        &format!("row read failed: {e}; row skipped"),
                    ));
                    continue;
                }
            };
            records_seen += 1;
            if id.as_str() > max_session_id.as_str() {
                max_session_id = id.clone();
            }
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
        let hit_cap = records_seen > MAX_ROWS_PER_ROUND as u64;
        Ok(ScanOutcome {
            status: if hit_cap {
                ScanStatus::BudgetExhausted
            } else {
                ScanStatus::Complete
            },
            cursor: Some(serde_json::to_value(GooseCursor {
                generation: target.generation,
                last_ledger_id: 0,
                last_session_id: if hit_cap {
                    max_session_id
                } else {
                    String::new()
                },
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
