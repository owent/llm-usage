//! Map state.db session_model_usage model/task cumulative rows to native interval aggregates:
//! session_model_usage_v1.
//!
//! Fixed reference A24: ef70b3661cbfcf57e583008ad91dd04d8ba46070,
//! hermes_state_common.py schema, hermes_state_usage.py, and agent/turn_usage.py,
//! plus website/docs/developer-guide/session-storage.md.
//! - get_hermes_home()/state.db uses HERMES_HOME, then Windows LOCALAPPDATA/hermes
//!   or ~/.hermes elsewhere. Named profiles use <root>/profiles/<name>/state.db.
//!   state.db, -wal, and -shm share a directory under WAL mode.
//! - session/model/billing_provider/billing_base_url/billing_mode/task identify
//!   cumulative rows. api_call_count and five token columns increase by addition;
//!   first_seen is inserted once, while last_seen updates with subsequent contributions.
//!   Nonempty task rows record auxiliary consumption outside the main session total.
//!   Empty task rows are main model totals; absolute updates write sessions without model rows.
//! - REAL Unix-second first_seen/last_seen record aggregate writes,
//!   not individual request timestamps.
//! - Schema v20 backfills model rows from sessions with INSERT OR IGNORE;
//!   v22 adds task to the primary key. Neither backfilled rows nor the database-wide
//!   schema version identify every historical call's model or client release.
//!
//! Mapping follows Hermes local data rules and V03 numeric examples:
//! - Each row contributes one SourceAggregateInput retaining its native interval.
//!   Do not create model_call/usage_observation events; api_call_count is only
//!   reported_call_count, with default zero remaining unknown.
//! - Retain cross-day first_seen..last_seen intervals without assigning the entire
//!   sum to a day or distributing unknown request usage by duration.
//! - Main model and auxiliary task rows contribute independently under exclusive
//!   keys: coverage=Exclusive yields 100+20=120, with neither sum counted twice.
//! - Do not also read sessions cumulative columns, which can overlap absolute updates
//!   and v20 backfills. Subagent/compaction sessions own their model rows;
//!   do not attribute unexplained differences to the current model.
//! - Normalize billing_base_url in memory to scheme/host[:port] or a local digest.
//!   Never persist its full URL/query; scope_key hashes the six identity fields.
//!
//! Incremental database reads:
//! - Persist schema fingerprints; a change restarts reading at the head.
//!   Stable scope_key updates prevent duplicate aggregates.
//! - Track effective end milliseconds: last_seen, then session ended_at/started_at/
//!   first_seen, with a 60-second overlap. source_revision uses the effective end
//!   when source last_seen advances.
//! - Limit each scan to 50,000 rows; reaching the limit returns BudgetExhausted.
//! - Keep offset=0; database bytes under WAL cannot identify unchanged records.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::hermes::common::{
    db_schema_version, map_hermes, normalize_base_url, open_source_db, schema_probe, seconds_to_ms,
    short_probe, HermesUsage, StagingLimits,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::TimeBasis;
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

pub const HERMES_PARSER_VERSION: &str = "hermes-session-model-usage-2";

/// Reconstruct complete old summaries without arbitrary zero masks or token changes.
pub(crate) fn prior_usage_hashes(input: &SourceAggregateInput) -> Vec<String> {
    use crate::domain::FieldQuality as Q;
    if input.scope != AggregateScope::Session || !input.scope_key.starts_with("smu:") {
        return Vec::new();
    }
    let allowed_derived = |value: Option<i64>, quality| {
        matches!((value, quality), (Some(_), Q::Derived) | (None, Q::Unknown))
    };
    if !allowed_derived(input.usage.input_total, input.quality.input_total)
        || !allowed_derived(input.usage.total_tokens, input.quality.total_tokens)
        || input.usage.source_total.is_some()
        || input.quality.source_total != Q::Unknown
    {
        return Vec::new();
    }
    let old_bucket = |value, quality| match (value, quality) {
        (Some(v), Q::Reported) if v > 0 => Some(v),
        (None, Q::Unknown) => Some(0),
        _ => None,
    };
    let Some(old_input) = old_bucket(input.usage.input_uncached, input.quality.input_uncached)
    else {
        return Vec::new();
    };
    let mut old = input.clone();
    old.usage.input_uncached = None;
    old.quality.input_uncached = Q::Unknown;
    old.usage.input_total = Some(old_input);
    old.quality.input_total = Q::Reported;
    old.usage.total_tokens = None;
    old.quality.total_tokens = Q::Unknown;
    for (value, quality) in [
        (
            &mut old.usage.input_cache_read,
            &mut old.quality.input_cache_read,
        ),
        (
            &mut old.usage.input_cache_write,
            &mut old.quality.input_cache_write,
        ),
        (&mut old.usage.output_total, &mut old.quality.output_total),
        (
            &mut old.usage.output_reasoning,
            &mut old.quality.output_reasoning,
        ),
    ] {
        let Some(prior) = old_bucket(*value, *quality) else {
            return Vec::new();
        };
        *value = Some(prior);
        *quality = Q::Reported;
    }
    old.reported_call_count = Some(input.reported_call_count.unwrap_or(0));
    vec![crate::identity::content_hash(&old)]
}
/// Processing-time overlap in milliseconds for out-of-order aggregate writes within a second.
pub const WATERMARK_OVERLAP_MS: i64 = 60_000;
/// Maximum rows per scan.
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

/// Effective end seconds; all missing values use a sentinel rejected by Rust validation.
const EFFECTIVE_END_S: &str =
    "COALESCE(u.last_seen, s.ended_at, s.started_at, u.first_seen, -1.0e18)";

/// Cursor stored in ingestion_checkpoints.cursor_value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HermesCursor {
    generation: i64,
    /// Keep zero offset; WAL database reads do not use byte-size unchanged checks.
    #[allow(dead_code)]
    offset: u64,
    /// Last processed effective end in milliseconds, inclusive; None restarts at the head.
    watermark_ms: Option<i64>,
}

/// Schema fingerprint and database schema snapshot; neither identifies each record's client version.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HermesParseContext {
    schema_fingerprint: Option<String>,
    db_schema_version: Option<i64>,
}

fn diag(code: &str, field: Option<&str>, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        // Diagnostic position stores only an identity digest, excluding paths, bodies, and URLs.
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

struct SmuRow {
    session_id: String,
    model: String,
    billing_provider: String,
    billing_base_url: String,
    billing_mode: String,
    task: String,
    api_call_count: i64,
    usage: HermesUsage,
    first_seen: Option<f64>,
    last_seen: Option<f64>,
    session_started: Option<f64>,
    session_ended: Option<f64>,
}

impl SmuRow {
    fn effective_end_ms(&self) -> Option<i64> {
        let end_s = self
            .last_seen
            .or(self.session_ended)
            .or(self.session_started)
            .or(self.first_seen);
        seconds_to_ms(end_s.unwrap_or(f64::NAN))
    }

    /// Aggregate identity hashes six fields with a normalized base URL rather than the raw value.
    fn scope_key(&self) -> String {
        let route = normalize_base_url(&self.billing_base_url);
        let raw = [
            self.session_id.as_str(),
            self.model.as_str(),
            self.billing_provider.as_str(),
            route.as_str(),
            self.billing_mode.as_str(),
            self.task.as_str(),
        ]
        .join("\u{1}");
        format!("smu:{}", crate::identity::content_hash(&raw))
    }
}

fn load_window(
    conn: &rusqlite::Connection,
    since_s: f64,
) -> Result<Vec<Result<SmuRow, rusqlite::Error>>, CoreError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT u.session_id, u.model, u.billing_provider, u.billing_base_url, \
                 u.billing_mode, u.task, u.api_call_count, \
                 u.input_tokens, u.output_tokens, u.cache_read_tokens, \
                 u.cache_write_tokens, u.reasoning_tokens, \
                 u.first_seen, u.last_seen, s.started_at, s.ended_at \
             FROM session_model_usage u LEFT JOIN sessions s ON s.id = u.session_id \
             WHERE {EFFECTIVE_END_S} >= ?1 \
             ORDER BY {EFFECTIVE_END_S}, u.session_id, u.model, u.billing_provider, \
                 u.billing_base_url, u.billing_mode, u.task \
             LIMIT ?2"
        ))
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![since_s, MAX_ROWS_PER_ROUND], |r| {
            Ok(SmuRow {
                session_id: r.get(0)?,
                model: r.get(1)?,
                billing_provider: r.get(2)?,
                billing_base_url: r.get(3)?,
                billing_mode: r.get(4)?,
                task: r.get(5)?,
                api_call_count: r.get(6)?,
                usage: HermesUsage {
                    input_tokens: r.get(7)?,
                    output_tokens: r.get(8)?,
                    cache_read_tokens: r.get(9)?,
                    cache_write_tokens: r.get(10)?,
                    reasoning_tokens: r.get(11)?,
                },
                first_seen: r.get(12)?,
                last_seen: r.get(13)?,
                session_started: r.get(14)?,
                session_ended: r.get(15)?,
            })
        })
        .map_err(CoreError::Sqlite)?;
    Ok(rows.collect())
}

/// Scan state.db, dispatched by HermesAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor: HermesCursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(HermesCursor {
            generation: target.generation,
            offset: 0,
            watermark_ms: None,
        });
    let context: HermesParseContext = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let probe = schema_probe(conn)?;
    let Some(fingerprint) = probe.fingerprint else {
        // Detection rejects incompatible schemas; if the database changes before scanning,
        // reject the now-unknown format and preserve previous results.
        return Err(CoreError::Validation(
            "hermes state.db schema fingerprint no longer matches; fail closed".to_string(),
        ));
    };
    // Changed schema fingerprints restart reading; stable aggregate keys prevent duplicate counts.
    let fingerprint_reset = context.schema_fingerprint.is_some()
        && context.schema_fingerprint.as_deref() != Some(fingerprint.as_str());
    let watermark = if fingerprint_reset {
        None
    } else {
        cursor.watermark_ms
    };
    let since_s = watermark
        .map(|w| ((w - WATERMARK_OVERLAP_MS) as f64) / 1000.0)
        .unwrap_or(-1.0e19);

    let mut rows = load_window(conn, since_s)?;
    let hit_cap = rows.len() as i64 >= MAX_ROWS_PER_ROUND;
    let db_version = db_schema_version(conn)?;

    let mut aggregates: Vec<SourceAggregateInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    for row in rows.iter_mut() {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        let row = match row {
            Ok(row) => row,
            Err(error) => {
                diagnostics.push(diag(
                    "invalid_row_type",
                    None,
                    "session-model-usage-row",
                    &error.to_string(),
                ));
                continue;
            }
        };
        let scope_key = row.scope_key();
        let Some(end_ms) = row.effective_end_ms() else {
            diagnostics.push(diag(
                "implausible_or_missing_time",
                Some("last_seen"),
                &scope_key,
                "session_model_usage row without plausible end time \
                 (last_seen/session window all missing or pre-2000); row skipped, no zero fill",
            ));
            continue;
        };
        let start_ms = row.first_seen.and_then(seconds_to_ms);
        if let Some(start) = start_ms {
            if end_ms < start {
                diagnostics.push(diag(
                    "inconsistent_interval",
                    Some("first_seen"),
                    &scope_key,
                    "last_seen before first_seen; row skipped, not clamped",
                ));
                continue;
            }
        }
        let counters = [
            row.usage.input_tokens,
            row.usage.output_tokens,
            row.usage.cache_read_tokens,
            row.usage.cache_write_tokens,
            row.usage.reasoning_tokens,
        ];
        if counters.iter().any(|v| *v < 0) || row.api_call_count < 0 {
            diagnostics.push(diag(
                "negative_counter",
                Some("tokens"),
                &scope_key,
                "negative token/call counter; row skipped, not zeroed",
            ));
            continue;
        }
        let mapped = map_hermes(&row.usage);
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                &scope_key,
                &contradiction.detail,
            ));
        }
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: scope_key.clone(),
            interval_start_ms: start_ms,
            interval_end_ms: end_ms,
            // last_seen represents an inclusive instant at the end of the aggregate interval.
            interval_end_inclusive: true,
            usage: mapped.usage,
            quality: mapped.quality,
            // Keep source-reported call sums without constructing individual model_call events.
            reported_call_count: (row.api_call_count != 0).then_some(row.api_call_count),
            // Main/task keys are separate; auxiliary amounts are not already in the main total.
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            // first_seen/last_seen identify aggregate writes rather than individual request timestamps.
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        });
    }

    // At the limit, retain the last complete millisecond and reread the overlap next scan.
    let last_end = rows
        .last()
        .and_then(|r| r.as_ref().ok())
        .and_then(|r| r.effective_end_ms());
    let new_watermark = if hit_cap {
        last_end.map(|w| w - 1)
    } else {
        last_end
    };
    let next_watermark = match (watermark, new_watermark) {
        (_, Some(w)) => Some(w.max(watermark.unwrap_or(i64::MIN))),
        (None, None) => None,
        (Some(w), None) => Some(w),
    };
    let status = if hit_cap {
        ScanStatus::BudgetExhausted
    } else {
        ScanStatus::Complete
    };
    let new_cursor = HermesCursor {
        generation: target.generation,
        offset: 0,
        watermark_ms: next_watermark,
    };
    let new_context = HermesParseContext {
        schema_fingerprint: Some(fingerprint),
        db_schema_version: db_version,
    };
    let degraded = !diagnostics.is_empty();
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(new_context)?),
        events: Vec::new(),
        aggregates,
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: if degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}
