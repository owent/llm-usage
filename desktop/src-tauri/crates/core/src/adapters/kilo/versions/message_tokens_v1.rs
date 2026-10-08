//! Per-call usage from kilo.db's message table (message_tokens_v1).
//!
//! Format references: redacted native test data and read-only local SELECT queries, 2026-09-25.
//! - Database: <kilo home>/kilo.db; local ~/.local/share/kilo/kilo.db uses WAL
//!   with kilo.db-wal/-shm. Upstream opencode-rc.db is outside this parser's scope.
//! - Tables: message(id, session_id, time_created, time_updated, data),
//!   session(id, project_id, parent_id, …, version, …, tokens_input/output/
//!   reasoning/cache_read/cache_write, …); original DDL is in the test-data schema section.
//! - Per-call usage is message.data JSON with role="assistant" and
//!   tokens{input, output, reasoning?, cache{read, write}, total?}.
//!   total=input+output+reasoning+cache.read+cache.write; all five components are exclusive.
//!   All 13,342 locally read assistant rows had tokens; 42 lacked total
//!   when unfinished or failed. finish values: tool-calls/stop/length/other/error/null.
//! - Assistant messages carry modelID/providerID (request_field);
//!   time{created, completed?} uses epoch milliseconds.
//! - Nonempty session.parent_id marks that session's messages as sub_agent;
//!   child sessions have separate rows in the same database, without counting both tables.
//! - Five session tokens_* columns are cumulative snapshots; compare only their combined sum
//!   with per-call usage. 275 of 276 native sessions matched. In 7.4.8/7.4.9 test data,
//!   column names and values were misaligned; the newer local version matched by column.
//!   Column meanings vary by version; snapshots never become request events or add to request counts (A11).
//! - The observed session_message table was empty. Databases with only that table and no
//!   message table are unknown formats; verify them before adding a separate parser.
//!
//! Incremental SQLite rules: see the per-input strategies in architecture.md.
//! - Persist a schema fingerprint (tables/key columns) in the parse context; a changed fingerprint
//!   resets the processing position for a full reread with idempotent upserts by ID.
//! - Stable key: message.id; revision: message.time_updated (event source_revision).
//! - A 60-second overlap before the processed timestamp covers out-of-order same-millisecond
//!   writes from concurrent sessions; identical key/content pairs remain idempotent.
//! - message has both time_created/time_updated; it does not need a created_at-only bounded reread.
//! - Limit each round to 50,000 rows. BudgetExhausted retains the last complete millisecond.
//! - Cursor offset stays zero: an unchanged main-file size under WAL does not mean unchanged content.
//!   Do not use byte length to skip reads; framework checks and generation handling still apply.
//!   In-place page rewrites change the header change counter and trigger framework Rescan;
//!   this parser preserves the processing position because IDs/revisions remain valid in the same logical database.

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::kilo::common::{
    map_kilo, open_source_db, schema_fingerprint, KiloUsage, SourceDb, StagingLimits,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

pub const KILO_PARSER_VERSION: &str = "kilo-message-tokens-4";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// Timestamp overlap in milliseconds covers out-of-order same-millisecond child-session writes.
pub const WATERMARK_OVERLAP_MS: i64 = 60_000;
/// Per-round row limit; resume the incomplete millisecond on the next round.
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

/// Cursor persisted in ingestion_checkpoints.cursor_value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KiloCursor {
    generation: i64,
    /// Always zero; database change detection cannot use byte offsets under WAL.
    #[allow(dead_code)]
    offset: u64,
    /// Last processed message.time_updated, inclusive; None starts a complete scan.
    watermark_ms: Option<i64>,
}

/// Parse context stores the schema fingerprint and version selection alongside the cursor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct KiloParseContext {
    schema_fingerprint: Option<String>,
    version_basis: Option<VersionBasis>,
    db_version: Option<String>,
    #[serde(default)]
    has_unverified_records: bool,
    /// Record failures persist across incremental windows until the row is successfully reread.
    #[serde(default)]
    record_errors: std::collections::BTreeSet<String>,
}

fn diag(code: &str, field: Option<&str>, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        // Store only message.id as the diagnostic position, excluding paths and message bodies.
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

struct MessageRow {
    id: String,
    session_id: String,
    time_created: i64,
    time_updated: i64,
    data: String,
    parent_id: Option<String>,
    session_version: Option<String>,
}

fn load_window(conn: &rusqlite::Connection, since_ms: i64) -> Result<Vec<MessageRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.session_id, m.time_created, m.time_updated, m.data, \
             s.parent_id, s.version \
             FROM message m LEFT JOIN session s ON s.id = m.session_id \
             WHERE m.time_updated >= ?1 \
             ORDER BY m.time_updated, m.id \
             LIMIT ?2",
        )
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![since_ms, MAX_ROWS_PER_ROUND], |r| {
            Ok(MessageRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                time_created: r.get(2)?,
                time_updated: r.get(3)?,
                data: r.get(4)?,
                parent_id: r.get(5)?,
                session_version: r.get(6)?,
            })
        })
        .map_err(CoreError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)
}

/// Highest numeric session.version; the same database may contain sessions from several versions.
fn max_session_version(conn: &rusqlite::Connection) -> Result<Option<String>, CoreError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT version FROM session")
        .map_err(CoreError::Sqlite)?;
    let versions = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(CoreError::Sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)?;
    Ok(versions
        .into_iter()
        .reduce(|a, b| super::version_max(&a, &b).to_string()))
}

/// Parse the five exclusive components in message.data.tokens.
/// Require input/output/cache.read/cache.write, present in all 13,342 native assistant rows;
/// optional reasoning/total remain unknown when absent. Invalid types or ranges return None.
fn parse_tokens(tokens: &serde_json::Value) -> Option<KiloUsage> {
    let obj = tokens.as_object()?;
    let get = |parent: Option<&serde_json::Map<String, serde_json::Value>>, key: &str| {
        parent?
            .get(key)?
            .as_i64()
            .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v))
    };
    let cache = obj.get("cache").and_then(|c| c.as_object());
    Some(KiloUsage {
        input: get(Some(obj), "input")?,
        output: get(Some(obj), "output")?,
        reasoning: obj
            .get("reasoning")
            .and_then(|v| v.as_i64())
            .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v)),
        cache_read: get(cache, "read")?,
        cache_write: get(cache, "write")?,
        total: obj
            .get("total")
            .and_then(|v| v.as_i64())
            .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v)),
    })
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    target: &ScanTarget,
    row: &MessageRow,
    data: &serde_json::Value,
    parse_basis: VersionBasis,
    now_ms: i64,
) -> EventInput {
    let json_i64 = |path: &[&str]| -> Option<i64> {
        let mut node = data;
        for key in path {
            node = node.get(key)?;
        }
        node.as_i64()
    };
    let json_str = |key: &str| data.get(key).and_then(|v| v.as_str());
    // Prefer completion time (source_completion), then start time (source_start),
    // then row time_created with uncertain time basis.
    let (occurred_ms, time_basis) = match (
        json_i64(&["time", "completed"]),
        json_i64(&["time", "created"]),
    ) {
        (Some(completed), _) => (completed, TimeBasis::SourceCompletion),
        (None, Some(created)) => (created, TimeBasis::SourceStart),
        (None, None) => (row.time_created, TimeBasis::Uncertain),
    };
    let source_time = json_i64(&["time", "completed"])
        .or_else(|| json_i64(&["time", "created"]))
        .map(|ms| ms.to_string());
    let duration_ms = match (
        json_i64(&["time", "created"]),
        json_i64(&["time", "completed"]),
    ) {
        (Some(created), Some(completed)) if completed >= created => Some(completed - created),
        _ => None,
    };
    let finish = json_str("finish");
    let has_error =
        data.get("error").map(|e| e.is_object()).unwrap_or(false) || finish == Some("error");
    // Missing finish and error leaves a partial event. Completion raises time_updated;
    // the same key can then replace it with a final event using source_revision.
    let lifecycle = if finish.is_some() || has_error {
        Lifecycle::Final
    } else {
        Lifecycle::Partial
    };
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("kilo:msg:{}", row.id),
        record_kind: RecordKind::ModelCall,
        schema_version: row
            .session_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: KILO_PARSER_VERSION.to_string(),
        parse_basis: Some(parse_basis),
        origin_call_id: json_str("parentID").map(str::to_string),
        attempt_id: None,
        session_id: Some(row.session_id.clone()),
        parent_session_id: row.parent_id.clone(),
        host_application: None,
        agent: "kilo-code".to_string(),
        call_category: if row.parent_id.is_some() {
            CallCategory::SubAgent
        } else {
            CallCategory::Primary
        },
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time,
        time_basis,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: json_str("providerID").map(str::to_string),
        model_raw: json_str("modelID").map(str::to_string),
        model_canonical: None,
        model_attribution: if data.get("modelID").is_some() {
            ModelAttribution::RequestField
        } else {
            ModelAttribution::Unknown
        },
        usage: crate::domain::TokenUsage::default(),
        quality: crate::domain::TokenQuality::default(),
        lifecycle,
        source_revision: Some(row.time_updated),
        error_status: if has_error {
            Some("error".to_string())
        } else {
            None
        },
        duration_ms,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }
}

/// Reconcile one session's five per-message assistant components against five cumulative session columns.
fn reconcile_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Reconciliation, CoreError> {
    let (detail, incomplete): (i64, i64) = conn
        .query_row(
            "WITH records AS (SELECT CASE WHEN json_valid(data) THEN data ELSE '{}' END data \
               FROM message WHERE session_id=?1), \
             usage AS (SELECT json_extract(data,'$.role') role, \
               json_extract(data,'$.tokens.input') i,json_extract(data,'$.tokens.output') o, \
               json_extract(data,'$.tokens.reasoning') r,json_extract(data,'$.tokens.cache.read') cr, \
               json_extract(data,'$.tokens.cache.write') cw, \
               json_type(data,'$.tokens.input')='integer' AND json_type(data,'$.tokens.output')='integer' \
               AND json_type(data,'$.tokens.reasoning')='integer' AND json_type(data,'$.tokens.cache.read')='integer' \
               AND json_type(data,'$.tokens.cache.write')='integer' types_valid FROM records), \
             checked AS (SELECT *,COALESCE(types_valid AND i BETWEEN 0 AND ?2 AND o BETWEEN 0 AND ?2 \
               AND r BETWEEN 0 AND ?2 AND cr BETWEEN 0 AND ?2 AND cw BETWEEN 0 AND ?2,0) valid FROM usage) \
             SELECT COALESCE(SUM(CASE WHEN role='assistant' AND valid THEN i+o+r+cr+cw ELSE 0 END),0), \
               COALESCE(SUM(CASE WHEN role IS NULL OR (role='assistant' AND NOT valid) THEN 1 ELSE 0 END),0) \
             FROM checked",
            rusqlite::params![session_id, MAX_REASONABLE_TOKEN],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(CoreError::Sqlite)?;
    let snapshot: Option<f64> = conn
        .query_row(
            "SELECT tokens_input + tokens_output + tokens_reasoning \
               + tokens_cache_read + tokens_cache_write \
             FROM session WHERE id = ?1",
            [session_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(CoreError::Sqlite)?;
    let snapshot_final = snapshot.map(|v| v.round() as i64);
    let (difference, verdict) = match snapshot_final {
        _ if incomplete > 0 => (None, "detail_incomplete"),
        Some(snap) => {
            let diff = detail - snap;
            (Some(diff), if diff == 0 { "matched" } else { "mismatch" })
        }
        None => (None, "no_snapshot"),
    };
    Ok(Reconciliation {
        series: "session_cumulative_snapshot".to_string(),
        detail_sum: detail,
        snapshot_final,
        carried_sum: 0,
        difference,
        verdict: verdict.to_string(),
    })
}

/// Incrementally scan kilo.db, called by KiloAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // Do not filter the cursor by generation: in-place rewrites of the same logical database trigger Rescan,
    // but IDs/revisions retain a valid processing position. A changed logical identity has empty stored state and scans fully.
    let cursor: KiloCursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(KiloCursor {
            generation: target.generation,
            offset: 0,
            watermark_ms: None,
        });
    let context: KiloParseContext = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let fingerprint = schema_fingerprint(conn)?;
    // A changed schema fingerprint resets the old position for a full, idempotent reread by ID.
    let fingerprint_reset =
        context.schema_fingerprint.is_some() && context.schema_fingerprint != fingerprint;
    let watermark = if fingerprint_reset {
        None
    } else {
        cursor.watermark_ms
    };
    let since_ms = watermark
        .map(|w| w.saturating_sub(WATERMARK_OVERLAP_MS))
        .unwrap_or(i64::MIN);

    let mut rows = load_window(conn, since_ms)?;
    let hit_cap = rows.len() as i64 >= MAX_ROWS_PER_ROUND;
    let version = max_session_version(conn)?;
    let selection = super::select(version.as_deref());
    let mut has_unverified_records = !fingerprint_reset && context.has_unverified_records;
    let mut record_errors = if watermark.is_none() {
        std::collections::BTreeSet::new()
    } else {
        context.record_errors
    };
    // A mutable source can remove invalid rows; deleted rows do not degrade its current health.
    record_errors.retain(|id| {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM message WHERE id=?1)",
            [id],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(true)
    });

    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut touched_sessions: Vec<String> = Vec::new();
    for row in rows.iter_mut() {
        crate::adapters::run_policy::check()?;
        record_errors.remove(&row.id);
        records_seen += 1;
        if !touched_sessions.iter().any(|s| s == &row.session_id) {
            touched_sessions.push(row.session_id.clone());
        }
        let data: serde_json::Value = match crate::adapters::run_policy::json_from_str(&row.data) {
            Ok(v) => v,
            Err(_) => {
                record_errors.insert(row.id.clone());
                diagnostics.push(diag(
                    "bad_data_json",
                    Some("data"),
                    &format!("kilo:msg:{}", row.id),
                    "message.data is not valid JSON; row isolated, content not stored",
                ));
                continue;
            }
        };
        // Only assistant messages contribute model_call usage (A11).
        let role = data.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "assistant" {
            if role.is_empty() {
                record_errors.insert(row.id.clone());
                diagnostics.push(diag(
                    "missing_role",
                    Some("role"),
                    &format!("kilo:msg:{}", row.id),
                    "message.data without role; skipped, not counted as a call",
                ));
            }
            continue;
        }
        // Mixed-version databases require each session's version; the highest version does not verify other sessions.
        let row_selection = super::select(row.session_version.as_deref());
        has_unverified_records |= row_selection.basis == VersionBasis::LatestFallback;
        let mut event = build_event(target, row, &data, row_selection.basis, now_ms);
        match data.get("tokens") {
            Some(tokens) => match parse_tokens(tokens) {
                Some(usage) => {
                    let mapped = map_kilo(&usage);
                    event.usage = mapped.usage;
                    event.quality = mapped.quality;
                    for contradiction in &mapped.diagnostics {
                        record_errors.insert(row.id.clone());
                        diagnostics.push(diag(
                            contradiction.code,
                            Some(contradiction.field),
                            &event.source_record_key,
                            &contradiction.detail,
                        ));
                    }
                }
                None => {
                    record_errors.insert(row.id.clone());
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("tokens"),
                        &format!("kilo:msg:{}", row.id),
                        "assistant tokens missing required numeric fields; call counted, tokens unknown",
                    ));
                }
            },
            None => {
                record_errors.insert(row.id.clone());
                // An assistant without usage still identifies a call:
                // count the call and leave every token field unknown.
                diagnostics.push(diag(
                    "usage_shape_deviation",
                    Some("tokens"),
                    &format!("kilo:msg:{}", row.id),
                    "assistant message without tokens; call counted, tokens unknown",
                ));
            }
        }
        events.push(event);
    }

    // At the row limit, retain the last complete millisecond; replay the incomplete millisecond idempotently.
    let new_watermark = if hit_cap {
        rows.last().map(|r| r.time_updated - 1)
    } else {
        rows.last().map(|r| r.time_updated)
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

    let mut reconciliations: Vec<Reconciliation> = Vec::new();
    if status == ScanStatus::Complete {
        for session_id in &touched_sessions {
            let rec = reconcile_session(conn, session_id)?;
            if rec.verdict == "mismatch" {
                diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "reconcile_mismatch".to_string(),
                    field: Some("total_tokens".to_string()),
                    position: None,
                    message: format!(
                        "session assistant detail sum {} != snapshot five-column sum {} (diff {})",
                        rec.detail_sum,
                        rec.snapshot_final.unwrap_or(0),
                        rec.difference.unwrap_or(0)
                    ),
                });
            }
            reconciliations.push(rec);
        }
    }

    let new_cursor = KiloCursor {
        generation: target.generation,
        offset: 0,
        watermark_ms: next_watermark,
    };
    let degraded = !record_errors.is_empty();
    let new_context = KiloParseContext {
        schema_fingerprint: fingerprint,
        version_basis: Some(selection.basis),
        db_version: version,
        has_unverified_records,
        record_errors,
    };
    // Session columns are separate cumulative snapshots; do not use them to infer per-call usage.
    // Retain reconciliation differences without invalidating valid per-message usage.
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(new_context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations,
        health: if degraded {
            "degraded".to_string()
        } else if has_unverified_records {
            "active_compat".to_string()
        } else {
            "active".to_string()
        },
    })
}

/// A short read-only query checks that this connection can read the database consistently.
fn short_probe(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// Read-only source opening for tests/examples; stage a copy if the database is busy.
pub fn open_readonly_source(path: &std::path::Path) -> Result<SourceDb, CoreError> {
    open_source_db(path, short_probe, &StagingLimits::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tokens_requires_core_fields_keeps_optionals_absent() {
        let full = serde_json::json!({
            "input": 20040, "output": 51, "reasoning": 477,
            "cache": {"read": 0, "write": 0}, "total": 20568
        });
        let usage = parse_tokens(&full).unwrap();
        assert_eq!(usage.total, Some(20568));
        let no_total = serde_json::json!({
            "input": 0, "output": 0, "reasoning": 0,
            "cache": {"read": 0, "write": 0}
        });
        let usage = parse_tokens(&no_total).unwrap();
        assert_eq!(usage.total, None, "missing total stays unknown, not zero");
        assert_eq!(
            usage.reasoning,
            Some(0),
            "explicit zero is a reported value"
        );
        // Missing cache or any required component prevents the exclusive-component mapping.
        assert!(parse_tokens(&serde_json::json!({"input": 1, "output": 2})).is_none());
        assert!(parse_tokens(&serde_json::json!({
            "input": 1, "output": 2, "cache": {"read": 0}
        }))
        .is_none());
        // Reject negative or excessive values.
        assert!(parse_tokens(&serde_json::json!({
            "input": -1, "output": 2, "cache": {"read": 0, "write": 0}
        }))
        .is_none());
    }

    #[test]
    fn version_max_prefers_numeric_order() {
        assert_eq!(super::super::version_max("7.4.9", "7.4.10"), "7.4.10");
        assert_eq!(super::super::version_max("7.7.12", "7.4.9"), "7.7.12");
    }
}
