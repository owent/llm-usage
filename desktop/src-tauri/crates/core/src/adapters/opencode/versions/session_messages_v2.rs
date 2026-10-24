//! OpenCode 2.0.32 session_v2/session_message projections, independently checked
//! against packages/schema/src/session-message.ts and core/src/session/{sql,
//! usage,message-updater,projector}.ts at the official v2.0.32 tag.
//! Positive components are exclusive; upstream default zeros remain unknown.
//! Session totals and durable events are not added to the message projections.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::opencode::common::{open_source_db, short_probe, StagingLimits};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, FieldQuality as Q, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, TokenQuality, TokenUsage, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use rusqlite::{types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const FORMAT: &str = "opencode-sqlite-session-messages-v2";
pub const PARSER_VERSION: &str = "opencode-session-messages-2";

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum CursorId {
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl CursorId {
    fn to_sql(&self) -> SqlValue {
        match self {
            Self::Integer(value) => SqlValue::Integer(*value),
            Self::Real(value) => SqlValue::Real(*value),
            Self::Text(value) => SqlValue::Text(value.clone()),
            Self::Blob(value) => SqlValue::Blob(value.clone()),
        }
    }
}

impl From<SqlValue> for CursorId {
    fn from(value: SqlValue) -> Self {
        match value {
            SqlValue::Integer(value) => Self::Integer(value),
            SqlValue::Real(value) => Self::Real(value),
            SqlValue::Text(value) => Self::Text(value),
            SqlValue::Blob(value) => Self::Blob(value),
            SqlValue::Null => Self::Text(String::new()),
        }
    }
}

pub(crate) fn matches_schema(conn: &Connection) -> Result<bool, CoreError> {
    for (table, required) in [
        (
            "session_v2",
            &[
                "id",
                "parent_id",
                "version",
                "fork_session_id",
                "fork_boundary",
            ][..],
        ),
        (
            "session_message",
            &[
                "id",
                "session_id",
                "type",
                "seq",
                "time_created",
                "time_updated",
                "data",
            ][..],
        ),
    ] {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if !required.iter().all(|name| cols.iter().any(|c| c == name)) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Default, Serialize, Deserialize)]
struct Context {
    #[serde(default)]
    validated_usage: bool,
    #[serde(default)]
    policy: String,
    after_id: Option<CursorId>,
    #[serde(default)]
    skip_nonfinite: bool,
    #[serde(default)]
    completed: Option<bool>,
    #[serde(default)]
    errors: BTreeSet<String>,
    #[serde(default)]
    pending_errors: BTreeSet<String>,
}

fn diagnostic(id: &str, code: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: None,
        position: Some(id.into()),
        message: message.into(),
    }
}

fn timestamp(v: &Value) -> Option<i64> {
    v.as_i64().filter(|v| *v >= crate::domain::MIN_PLAUSIBLE_MS)
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
}

/// SessionUsage.tokens supplies zero for absent/invalid provider components.
/// Positive visible output proves the upstream subtraction was not clamped, so
/// visible + reasoning recovers inclusive output even without a reasoning breakdown.
/// Zero visible output cannot distinguish absent output from a clamped contradiction.
fn usage(tokens: &Value) -> Result<(TokenUsage, TokenQuality), ()> {
    let mut parts = Vec::new();
    for v in [
        &tokens["input"],
        &tokens["cache"]["read"],
        &tokens["cache"]["write"],
        &tokens["output"],
        &tokens["reasoning"],
    ] {
        let n = v
            .as_i64()
            .filter(|n| (0..=crate::domain::MAX_TOKEN_VALUE).contains(n))
            .ok_or(())?;
        parts.push((n > 0).then_some(n));
    }
    let sum = |values: &[Option<i64>]| -> Option<i64> {
        values
            .iter()
            .try_fold(0i64, |sum, v| sum.checked_add((*v)?))
    };
    let input_total = sum(&parts[..3]);
    let output_total = parts[3].and_then(|visible| visible.checked_add(parts[4].unwrap_or(0)));
    let total = input_total
        .zip(output_total)
        .and_then(|(i, o)| i.checked_add(o));
    if (parts[..3].iter().all(Option::is_some) && input_total.is_none())
        || (parts[3].is_some() && output_total.is_none())
        || (input_total.is_some() && output_total.is_some() && total.is_none())
        || input_total.is_some_and(|v| v > crate::domain::MAX_TOKEN_VALUE)
        || output_total.is_some_and(|v| v > crate::domain::MAX_TOKEN_VALUE)
        || total.is_some_and(|v| v > crate::domain::MAX_TOKEN_VALUE)
    {
        return Err(());
    }
    let reported = |v: Option<i64>| if v.is_some() { Q::Reported } else { Q::Unknown };
    let derived = |v: Option<i64>| if v.is_some() { Q::Derived } else { Q::Unknown };
    Ok((
        TokenUsage {
            input_uncached: parts[0],
            input_cache_read: parts[1],
            input_cache_write: parts[2],
            input_total,
            output_total,
            output_reasoning: parts[4],
            total_tokens: total,
            source_total: None,
        },
        TokenQuality {
            input_uncached: reported(parts[0]),
            input_cache_read: reported(parts[1]),
            input_cache_write: reported(parts[2]),
            input_total: derived(input_total),
            output_total: derived(output_total),
            output_reasoning: reported(parts[4]),
            total_tokens: derived(total),
            source_total: Q::Unknown,
        },
    ))
}

/// Revisit the full projection in bounded ID pages. Updated earlier IDs are read
/// in the next cycle; timestamps need no uniqueness assumption or overlap heuristic.
/// Complete-cycle health replaces old errors only after every row was visited.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let _scope = crate::adapters::run_policy::SqliteScope::new(conn)?;
    conn.execute_batch("BEGIN")?;
    if !matches_schema(conn)? {
        return Err(CoreError::Validation(
            "OpenCode v2 schema changed during scan".into(),
        ));
    }
    let mut context: Context = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if context.policy != PARSER_VERSION {
        context = Context::default();
    }
    if context.after_id.is_none() && context.completed.unwrap_or(true) {
        context.pending_errors.clear();
        context.skip_nonfinite = false;
    }
    let limit = limits.jsonl.max_lines.unwrap_or(50_000).clamp(1, 50_000) as i64;
    let mut diagnostics = Vec::new();
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_message WHERE id IS NULL)",
        [],
        |row| row.get::<_, bool>(0),
    )? {
        context.pending_errors.insert("<null>".into());
        diagnostics.push(diagnostic(
            "<null>",
            "invalid_message_identity",
            "OpenCode v2 message has a null ID; other messages remain readable",
        ));
    }
    // Extract only usage/identity/time metadata; message bodies never leave SQLite.
    let mut stmt = conn.prepare(
        "SELECT m.id,m.session_id,m.type,m.seq,m.time_updated,s.id,s.parent_id,s.version,
          s.fork_session_id, CASE WHEN json_valid(s.fork_boundary) THEN json_extract(s.fork_boundary,'$.type') END,
          b.seq,CASE WHEN json_valid(m.data) THEN json_object(
            'time',json_extract(m.data,'$.time'),'tokens',json_extract(m.data,'$.tokens'),
            'model',json_extract(m.data,'$.model'),'cost',json_extract(m.data,'$.cost'),
            'status',json_extract(m.data,'$.status'),'error',json_type(m.data,'$.error'),
            'finish',json_extract(m.data,'$.finish')) END
         FROM session_message m LEFT JOIN session_v2 s ON s.id=m.session_id
         LEFT JOIN session_message b ON b.session_id=s.fork_session_id AND b.id=
           CASE WHEN json_valid(s.fork_boundary) THEN json_extract(s.fork_boundary,'$.messageID') END
         WHERE m.id IS NOT NULL AND (?1 IS NULL OR m.id>?1)
           AND (?2 = 0 OR (m.id NOT IN (-1e999,1e999) AND m.id = m.id))
         ORDER BY m.id LIMIT ?3")?;
    let after_id = context.after_id.as_ref().map(CursorId::to_sql);
    let rows = stmt.query_map(
        rusqlite::params![after_id, context.skip_nonfinite, limit + 1],
        |row| {
            let id = row.get::<_, SqlValue>(0)?;
            Ok((
                id,
                (|| {
                    Ok::<_, rusqlite::Error>((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<i64>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ))
                })(),
            ))
        },
    )?;
    let mut events = Vec::new();
    let mut count = 0;
    let mut last = context.after_id.clone();
    let mut capped = false;
    for row in rows {
        crate::adapters::run_policy::check()?;
        if count == limit {
            capped = true;
            break;
        }
        count += 1;
        let (cursor, values) = row?;
        let cursor = CursorId::from(cursor);
        let nonfinite = matches!(&cursor, CursorId::Real(value) if !value.is_finite());
        if !nonfinite {
            last = Some(cursor.clone());
        }
        let id = match cursor {
            CursorId::Text(value) if !value.is_empty() => value,
            _ => {
                if nonfinite {
                    context.skip_nonfinite = true;
                }
                context.pending_errors.insert("<invalid-id>".into());
                diagnostics.push(diagnostic(
                    "<invalid-id>",
                    "invalid_message_identity",
                    "OpenCode v2 message ID must be nonempty text; other messages remain readable",
                ));
                continue;
            }
        };
        let mut problem = |code, message| {
            context.pending_errors.insert(id.clone());
            diagnostics.push(diagnostic(&id, code, message));
        };
        let (
            session,
            kind,
            seq,
            revision,
            owner,
            parent,
            version,
            fork,
            boundary_type,
            boundary,
            data,
        ) = match values {
            Ok(values) => values,
            Err(_) => {
                problem(
                    "row_read_failed",
                    "OpenCode v2 required SQLite columns have unexpected types; message skipped",
                );
                continue;
            }
        };
        let data: Value = data
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        if owner.is_none() || revision < crate::domain::MIN_PLAUSIBLE_MS {
            problem(
                "invalid_message_identity",
                "OpenCode v2 message lacks a valid owning session or revision",
            );
            continue;
        }
        if fork.is_some() {
            match (boundary, boundary_type.as_deref()) {
                (Some(b), Some("before" | "after")) => {
                    if seq < b || (seq == b && boundary_type.as_deref() == Some("after")) {
                        continue;
                    }
                }
                _ => {
                    problem(
                        "fork_ownership_unknown",
                        "Cannot distinguish inherited OpenCode v2 fork history",
                    );
                    continue;
                }
            }
        }
        let Some(created) = timestamp(&data["time"]["created"]) else {
            problem(
                "invalid_message_time",
                "OpenCode v2 message has invalid native creation time",
            );
            continue;
        };
        if !matches!(kind.as_str(), "assistant" | "compaction") {
            if !matches!(
                kind.as_str(),
                "user"
                    | "system"
                    | "synthetic"
                    | "skill"
                    | "shell"
                    | "idle"
                    | "agent-switched"
                    | "model-switched"
                    | "location-switched"
            ) {
                problem(
                    "unknown_record_type",
                    "Unrecognized OpenCode v2 message type; coverage requires review",
                );
            }
            continue;
        }
        let completed = timestamp(&data["time"]["completed"]);
        let terminal = if kind == "assistant" {
            completed.is_some()
        } else {
            matches!(data["status"].as_str(), Some("completed" | "failed"))
        };
        if !terminal {
            if (kind == "assistant" && !data["time"]["completed"].is_null())
                || (kind == "compaction" && data["status"] != "running")
            {
                problem(
                    "invalid_message_lifecycle",
                    "OpenCode v2 message has an invalid completion time or status",
                );
            }
            continue;
        }
        let (usage, quality) = if data["tokens"].is_null() {
            // Failed requests may finish without returned usage; calls still count.
            (TokenUsage::default(), TokenQuality::default())
        } else {
            match usage(&data["tokens"]) {
                Ok(mapped) => mapped,
                Err(()) => {
                    problem(
                        "usage_shape_deviation",
                        "OpenCode v2 token fields must be five bounded nonnegative integers",
                    );
                    (TokenUsage::default(), TokenQuality::default())
                }
            }
        };
        let model = text(&data["model"]["id"]);
        let provider = text(&data["model"]["providerID"]);
        if kind == "assistant" && (model.is_none() || provider.is_none()) {
            problem(
                "missing_model_identity",
                "OpenCode v2 assistant requires its own model and provider",
            );
        }
        let cost = crate::adapters::opencode_family::map_family_cost(
            data["cost"].as_f64().filter(|v| *v > 0.0),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("opencode:v2:{id}"),
            // Compaction.spend adds all attempts into one projection; it has no per-call count.
            record_kind: if kind == "compaction" {
                RecordKind::UsageObservation
            } else {
                RecordKind::ModelCall
            },
            schema_version: version.unwrap_or_else(|| "unknown".into()),
            parser_version: PARSER_VERSION.into(),
            parse_basis: Some(VersionBasis::LatestFallback),
            origin_call_id: Some(id),
            attempt_id: None,
            session_id: Some(session),
            parent_session_id: parent.clone(),
            host_application: None,
            agent: "opencode".into(),
            call_category: if kind == "compaction" {
                CallCategory::Auxiliary
            } else if parent.is_some() {
                CallCategory::SubAgent
            } else {
                CallCategory::Primary
            },
            occurred_at_ms: completed.unwrap_or(created),
            observed_at_ms: Some(now_ms),
            source_time: None,
            time_basis: if completed.is_some() {
                TimeBasis::SourceCompletion
            } else {
                TimeBasis::SourceStart
            },
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: provider,
            model_raw: model.clone(),
            model_canonical: None,
            model_attribution: if model.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage,
            quality,
            lifecycle: Lifecycle::Final,
            source_revision: Some(revision),
            error_status: (data["error"].as_str().is_some_and(|s| s != "null")
                || data["status"] == "failed")
                .then(|| "failed".into()),
            duration_ms: completed
                .and_then(|end| end.checked_sub(created))
                .filter(|v| *v > 0),
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost,
        });
    }
    context.validated_usage |= !events.is_empty();
    context.after_id = if capped { last } else { None };
    context.completed = Some(!capped);
    if !capped {
        context.errors = context.pending_errors.clone();
    }
    context.policy = PARSER_VERSION.into();
    let degraded = !context.errors.is_empty() || !context.pending_errors.is_empty();
    Ok(ScanOutcome {
        status: if capped {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(json!({"offset":0,"generation":target.generation})),
        parse_context: Some(serde_json::to_value(context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: count as u64,
        records_seen: count as u64,
        reconciliations: Vec::new(),
        health: if degraded {
            "degraded"
        } else {
            "active_compat"
        }
        .into(),
    })
}
