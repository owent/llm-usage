//! Crush crush.db implementation (`sessions_cost_v1`, crush-sessions-cost-1).
//!
//! References: charmbracelet/crush source commit
//! 1f3827bcd2d20f38076b2d46123683271e6ed9ba and v0.97.1; real API/CLI/SQLite checks
//! matched main-loop plus automatic-title cost=0.005059 using explicit test rates.
//! Context snapshot 4901 does not replace API total 5063; other scenarios/versions remain unverified.
//! - Each project has a WAL database `<data_dir>/crush.db`; sessions columns include
//!   id/parent_session_id/title/message_count/prompt_tokens/completion_tokens/
//!   cost/updated_at/created_at (Unix seconds).
//! - Token columns are context-size snapshots, not usage (agent.go:2060-2086):
//!   SET overwrites them, summarization resets them and title requests add another contribution.
//!   Summing would overcount; the upstream stats.sql approximation is not imported.
//! - Cost is cumulative (agent.go:2065); finished child sessions roll into their parent
//!   (coordinator.go:1742-1758). Read only parent_session_id IS NULL root rows
//!   to avoid duplicate cost, as in the upstream statistics query.
//! - Each root becomes one cost-only UsageObservation with all token fields unknown.
//!   Model-rate calculations include OpenRouter overrides, FlatRate=0 and zero for
//!   estimated usage; positive values use CostKind::Estimated in micro-USD.
//! - Paginate ordered session ids with up to 50,001 rows per round; EOF resets the cursor.
//!   Key crush:session:&lt;session id&gt; is scoped by source instance; hashes deduplicate, updated_at orders changes.

use crate::adapters::crush::common::{open_source_db, short_probe, StagingLimits};
use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::CRUSH_FORMAT_VERSION;

pub const CRUSH_PARSER_VERSION: &str = "crush-sessions-cost-1";
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CrushCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
    #[serde(default)]
    last_session_id: String,
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

fn usd_cost(amount: f64) -> Option<CostAmount> {
    // Default 0.0/FlatRate cost does not establish reported zero; emit no observation.
    if !amount.is_finite() || amount <= 0.0 {
        return None;
    }
    let micros = amount * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let db = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let after_id = if target.rescan {
        String::new()
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<CrushCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .map(|c| c.last_session_id)
            .unwrap_or_default()
    };
    // Read one extra row to distinguish exactly MAX rows from a remaining page (BudgetExhausted).
    let mut stmt = db.conn().prepare(
        "SELECT id, title, cost, created_at, updated_at
         FROM sessions WHERE parent_session_id IS NULL AND id > ?1
         ORDER BY id LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![&after_id, MAX_ROWS_PER_ROUND + 1], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<f64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    let mut last_session_id = after_id;
    for row in rows {
        crate::adapters::run_policy::check()?;
        // Diagnose and skip individual SQLite column-type errors while continuing the round.
        let (session_id, _title, cost, created_at, updated_at) = match row {
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
        last_session_id = session_id.clone();
        let Some(cost) = cost.and_then(usd_cost) else {
            continue;
        };
        let end_ms = updated_at.and_then(seconds_to_ms);
        let Some(occurred_ms) = end_ms.or(created_at.and_then(seconds_to_ms)) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("session:{session_id}"),
                "created_at/updated_at missing/implausible; session skipped",
            ));
            continue;
        };
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("crush:session:{session_id}"),
            // Cost-only observation is not a model call; context-snapshot token columns remain unknown.
            record_kind: RecordKind::UsageObservation,
            schema_version: CRUSH_FORMAT_VERSION.to_string(),
            parser_version: CRUSH_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(session_id),
            parent_session_id: None,
            host_application: None,
            agent: "crush".to_string(),
            call_category: CallCategory::Unknown,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: updated_at.map(|s| s.to_string()),
            time_basis: TimeBasis::Uncertain,
            interval_start_ms: created_at.and_then(seconds_to_ms),
            interval_end_ms: end_ms,
            provider_id: None,
            model_raw: None,
            model_canonical: None,
            model_attribution: ModelAttribution::Unknown,
            usage: crate::domain::TokenUsage::default(),
            quality: crate::domain::TokenQuality::default(),
            lifecycle: Lifecycle::Final,
            // Root-session cost is cumulative; valid updated_at milliseconds order revisions.
            // Without that order, changed content may conflict with the previous Final event.
            source_revision: end_ms,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: Some(cost),
        });
    }
    let hit_cap = records_seen > MAX_ROWS_PER_ROUND as u64;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(CrushCursor {
            generation: target.generation,
            offset: 0,
            last_session_id: if hit_cap {
                last_session_id
            } else {
                String::new()
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

    #[test]
    fn cost_gate() {
        assert_eq!(usd_cost(0.5).unwrap().amount_minor, 500_000);
        assert!(usd_cost(-0.1).is_none());
        assert!(usd_cost(f64::NAN).is_none());
    }
}
