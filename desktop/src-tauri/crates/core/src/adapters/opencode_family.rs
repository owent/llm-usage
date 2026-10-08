//! Shared SQLite helpers for OpenCode A17 and MiMo Code A14 (M3).
//!
//! Products in the OpenCode engine family retain separate identities, data roots,
//! instances, and registries; see adapters.md. This root-level module, like
//! kimi_wire.rs, shares serialized structures checked separately in each product's pinned source.
//! Each product supplies its namespace, agent name, parser version, and field policies.
//!
//! - Per-step part usage: data.type="step-finish" with cost and tokens fields,
//!   tokens{input, output, reasoning, cache{read, write}},
//!   and optional total. References: OpenCode commit 0027387,
//!   packages/core/src/session/projector.ts usage extraction,
//!   applyUsage counters, migration 20260510033149_session_usage.ts
//!   json_extract queries, and vendored client
//!   packages/app/vendor/opencode-ai-client-1.17.13-v2.tgz
//!   StepFinishPart/AssistantMessage types, with optional total.
//!   MiMo commit 456678b packages/opencode/src/session/message-v2.ts
//!   defines StepFinishPart with five required numeric fields and optional total.
//! - OpenCode field rules, verified independently in its pinned source:
//!   packages/core/src/session/runner/publish-llm-event.ts tokens()
//!   maps input=usage.nonCachedInputTokens, excluding cache;
//!   output=usage.visibleOutputTokens, with separate reasoning/cache read/write.
//!   packages/llm/src/protocols/anthropic-messages.ts mapUsage
//!   defines inclusive inputTokens=nonCached+cacheRead+cacheWrite through sumTokens.
//!   Anthropic does not split reasoning: outputTokens includes it, while reasoning
//!   defaults from undefined to zero through safe(). packages/llm/src/protocols/shared.ts
//!   derives totalTokens=inputTokens+outputTokens when the provider omits total.
//!   Derived input_total=input+cache.read+cache.write;
//!   output_total=output+reasoning works with the inspected provider mappings,
//!   where Anthropic reasoning is already in output and the separate reasoning field is zero.
//!   Total is the sum of five components. Compare an optional total field
//!   as source_total and diagnose differences; shared arithmetic does not verify another product.
//! - Both products' Timestamps default to Date.now(): epoch milliseconds.
//! - Pinned sql.ts table structures:
//!   part(id, message_id, session_id, time_created, time_updated, data),
//!   session(id, parent_id, version, …), message(id, …, data). MiMo has
//!   message.agent_id and lacks session tokens_* columns; OpenCode differs.
//!   Each product's common.rs defines its distinct detection fingerprint.
//!
//! Collection rules (adapters.md A14/A17):
//! - Assistant message.data.tokens aggregates a turn. OpenCode migrations sum its fields
//!   into cumulative session columns. Read only part step-finish usage for per-call events;
//!   message provides modelID/providerID attribution, without counting
//!   its tokens or cost again.
//! - OpenCode session.tokens_* columns are reconciliation only:
//!   projector applyUsage sums current step-finish parts, compensating for deleted rows.
//!   MiMo lacks those columns, so omit this reconciliation.
//! - Models come from message.data.modelID/providerID (request_field), required by
//!   the vendored AssistantMessage type and MiMo schema. Exact per-call completion time
//!   is not available here: part timestamps describe writes to a derived database view.
//!   Keep observed_at as time basis.
//! - A step-finish part has no independent call ID; use primary key part.id for event identity.
//!
//! Native samples cover OpenCode 1.18.34 and MiMo 0.1.15 under their documented route limits.
//! Each product selects record versions and zero-value rules independently; never infer one from the other.

use crate::adapters::framework::{
    Reconciliation, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// Millisecond overlap before the processing position covers out-of-order concurrent-session writes.
pub(crate) const WATERMARK_OVERLAP_MS: i64 = 60_000;
/// Per-round row limit; timestamp/ID continuation resumes within the same millisecond.
pub(crate) const MAX_ROWS_PER_ROUND: i64 = 50_000;

/// Five numeric step-finish token components and an optional total.
/// Input is uncached in the inspected mappings; cache components are separate.
/// Reasoning/output separation depends on the provider; Anthropic keeps reasoning
/// within output with a zero separate component. Derive input_total=input+cr+cw,
/// output_total=output+reasoning, and total from all five components; compare optional total
/// as source_total and diagnose differences. Products apply their own zero-value rules.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OpencodeFamilyUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// Optional source total; absence remains unknown.
    pub total: Option<i64>,
}

/// Component relationships follow the pinned mappings. Overflowing derived fields
/// remain None without panic or truncation; similar arithmetic does not establish another product's field rules.
pub(crate) fn map_opencode_family_usage(raw: &OpencodeFamilyUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let input_total = raw
        .input
        .checked_add(raw.cache_read)
        .and_then(|v| v.checked_add(raw.cache_write));
    let output_total = raw.output.checked_add(raw.reasoning);
    let derived_total = input_total.and_then(|i| output_total.and_then(|o| i.checked_add(o)));
    if let (Some(dt), Some(total)) = (derived_total, raw.total) {
        if dt != total {
            diagnostics.push(crate::metrics::Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: format!("opencode-family total {total} != derived sum {dt}"),
            });
        }
    }
    let usage = TokenUsage {
        input_uncached: Some(raw.input),
        input_cache_read: Some(raw.cache_read),
        input_cache_write: Some(raw.cache_write),
        input_total,
        output_total,
        output_reasoning: Some(raw.reasoning),
        total_tokens: derived_total.or(raw.total),
        source_total: raw.total,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: if output_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_reasoning: Q::Reported,
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Reported
        },
        source_total: if raw.total.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
    };
    finish(usage, quality, diagnostics)
}

/// MiMo 0.1.15 getUsage subtracts the normalized cache/reasoning
/// counters that it persists. Adding them back reconstructs positive
/// SDK totals even if a subcounter was defaulted. A zero subcounter alone
/// does not establish reported zero; this MiMo policy is independent of OpenCode.
pub(crate) fn map_mimo_usage(raw: &OpencodeFamilyUsage) -> MappedUsage {
    let mut mapped = map_opencode_family_usage(raw);
    for (value, quality) in [
        (
            &mut mapped.usage.input_uncached,
            &mut mapped.quality.input_uncached,
        ),
        (
            &mut mapped.usage.input_cache_read,
            &mut mapped.quality.input_cache_read,
        ),
        (
            &mut mapped.usage.input_cache_write,
            &mut mapped.quality.input_cache_write,
        ),
        (
            &mut mapped.usage.output_reasoning,
            &mut mapped.quality.output_reasoning,
        ),
        (
            &mut mapped.usage.input_total,
            &mut mapped.quality.input_total,
        ),
        (
            &mut mapped.usage.output_total,
            &mut mapped.quality.output_total,
        ),
        (
            &mut mapped.usage.source_total,
            &mut mapped.quality.source_total,
        ),
    ] {
        if *value == Some(0) {
            *value = None;
            *quality = Q::Unknown;
        }
    }
    if mapped.usage.input_total.is_none() || mapped.usage.output_total.is_none() {
        mapped.usage.total_tokens = mapped.usage.source_total;
        mapped.quality.total_tokens = if mapped.usage.source_total.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        };
    }
    finish(mapped.usage, mapped.quality, mapped.diagnostics)
}

/// Convert client-estimated cost from floating-point USD to micro-USD.
pub(crate) fn map_family_cost(cost: Option<f64>) -> Option<CostAmount> {
    let total = cost?;
    if !total.is_finite() || total < 0.0 {
        return None;
    }
    let micros = total * 1_000_000.0;
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

/// Extract step-finish usage from the independently inspected product structures:
/// - type must be step-finish;
/// - cost and tokens must both exist, as in OpenCode projector usage();
/// - five numeric tokens{input, output, reasoning, cache{read, write}} fields are required;
///   total is optional in MiMo's schema and the vendored client type;
/// - require bounded nonnegative values; invalid data returns None, leaving the caller's counted call with unknown tokens.
pub(crate) fn parse_step_finish(value: &serde_json::Value) -> Option<OpencodeFamilyUsage> {
    let obj = value.as_object()?;
    if obj.get("type").and_then(|t| t.as_str()) != Some("step-finish") {
        return None;
    }
    if !obj.contains_key("cost") || !obj.contains_key("tokens") {
        return None;
    }
    let tokens = obj.get("tokens")?.as_object()?;
    let cache = tokens.get("cache").and_then(|c| c.as_object());
    let num = |v: Option<&serde_json::Value>| {
        v.and_then(|x| x.as_i64())
            .filter(|n| (0..=MAX_REASONABLE_TOKEN).contains(n))
    };
    Some(OpencodeFamilyUsage {
        input: num(tokens.get("input"))?,
        output: num(tokens.get("output"))?,
        reasoning: num(tokens.get("reasoning"))?,
        cache_read: num(cache.and_then(|c| c.get("read")))?,
        cache_write: num(cache.and_then(|c| c.get("write")))?,
        total: num(tokens.get("total")),
    })
}

/// Product identity supplied during scanning; this module holds no product state.
pub(crate) struct PartProduct {
    /// Event-key namespace: adapter_id opencode or mimo-code.
    pub ns: &'static str,
    /// Agent name used in statistics.
    pub agent: &'static str,
    /// Parser version for this product implementation.
    pub parser_version: &'static str,
    /// Reconcile OpenCode session.tokens_*; MiMo has no cumulative columns to reconcile.
    pub reconcile_session_counters: bool,
    /// Each product explicitly selects its independently verified zero-value policy.
    pub default_zero_unknown: bool,
}

/// Cursor persisted in ingestion_checkpoints.cursor_value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PartCursor {
    pub generation: i64,
    /// Always zero; database change detection cannot use byte offsets under WAL.
    #[allow(dead_code)]
    pub offset: u64,
    /// Last processed part.time_updated, inclusive; None starts a complete scan.
    pub watermark_ms: Option<i64>,
    #[serde(default)]
    pub continuation: Option<(i64, String)>,
    #[serde(default)]
    pub window_start_ms: Option<i64>,
}

/// Parse context stores schema fingerprints and version-selection state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PartParseContext {
    pub schema_fingerprint: Option<String>,
    pub version_basis: Option<VersionBasis>,
    pub db_version: Option<String>,
    #[serde(default)]
    pub scan_policy_version: Option<String>,
    #[serde(default)]
    pub replay_policy_version: Option<String>,
    #[serde(default)]
    pub has_unverified_records: bool,
    #[serde(default)]
    pub record_errors: std::collections::BTreeSet<String>,
}

fn diag(code: &str, field: Option<&str>, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        // Store only part.id as the diagnostic position, excluding paths and message bodies.
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

/// One window row: a step-finish part and session/message attribution columns.
pub(crate) struct PartRow {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub time_created: i64,
    pub time_updated: i64,
    pub data: String,
    pub parent_id: Option<String>,
    pub session_version: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
}

/// Query the timestamp window using part LEFT JOIN session/message;
/// json_valid checks message.data before extracting modelID/providerID, without loading message bodies.
fn load_window(
    conn: &Connection,
    since_ms: i64,
    after: Option<&(i64, String)>,
    limit: i64,
) -> Result<Vec<PartRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.message_id, p.session_id, p.time_created, p.time_updated, p.data, \
                  s.parent_id, CASE WHEN typeof(s.version)='text' THEN s.version END, \
                 CASE WHEN m.data IS NOT NULL AND json_valid(m.data) \
                       THEN CASE WHEN json_type(m.data, '$.modelID')='text' THEN json_extract(m.data, '$.modelID') END END, \
                 CASE WHEN m.data IS NOT NULL AND json_valid(m.data) \
                       THEN CASE WHEN json_type(m.data, '$.providerID')='text' THEN json_extract(m.data, '$.providerID') END END \
             FROM part p \
             LEFT JOIN session s ON s.id = p.session_id \
             LEFT JOIN message m ON m.id = p.message_id \
              WHERE p.time_updated >= ?1 \
                AND (?3 IS NULL OR p.time_updated > ?3 OR (p.time_updated = ?3 AND p.id > ?4)) \
                AND CASE WHEN json_valid(p.data) THEN json_extract(p.data, '$.type') = 'step-finish' ELSE 1 END \
             ORDER BY p.time_updated, p.id \
             LIMIT ?2",
        )
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(
            rusqlite::params![
                since_ms,
                limit,
                after.map(|a| a.0),
                after.map(|a| a.1.as_str())
            ],
            |r| {
                Ok(PartRow {
                    id: r.get(0)?,
                    message_id: r.get(1)?,
                    session_id: r.get(2)?,
                    time_created: r.get(3)?,
                    time_updated: r.get(4)?,
                    data: r.get(5)?,
                    parent_id: r.get(6)?,
                    session_version: r.get(7)?,
                    model_id: r.get(8)?,
                    provider_id: r.get(9)?,
                })
            },
        )
        .map_err(CoreError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)
}

/// Highest numeric session.version in either product's database; an empty table returns None.
pub(crate) fn max_session_version(conn: &Connection) -> Result<Option<String>, CoreError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT version FROM session")
        .map_err(CoreError::Sqlite)?;
    let versions = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(CoreError::Sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)?;
    // Compare version components numerically, using the same ordering as kilo version_max.
    Ok(versions
        .into_iter()
        .reduce(|a, b| version_max(&a, &b).to_string()))
}

/// Numeric-component version ordering, copied within the family with kilo's ordering rules.
fn version_max<'a>(a: &'a str, b: &'a str) -> &'a str {
    let parse = |s: &str| -> Vec<i64> {
        s.split('.')
            .map(|p| p.parse::<i64>().unwrap_or(0))
            .collect()
    };
    let (va, vb) = (parse(a), parse(b));
    if va >= vb {
        a
    } else {
        b
    }
}

/// OpenCode reconciliation compares five per-step components with five cumulative session columns;
/// projector applyUsage includes current parts and compensates for deleted rows.
fn reconcile_session_counters(
    conn: &Connection,
    session_id: &str,
) -> Result<Reconciliation, CoreError> {
    let detail: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(json_extract(data,'$.tokens.input')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.output')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.reasoning')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.cache.read')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.cache.write')),0) \
             FROM part WHERE session_id = ?1 AND json_valid(data) \
               AND json_extract(data,'$.type') = 'step-finish'",
            [session_id],
            |r| r.get(0),
        )
        .map_err(CoreError::Sqlite)?;
    let snapshot: Option<i64> = conn
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
    let (difference, verdict) = match snapshot {
        Some(snap) => {
            let diff = detail - snap;
            (Some(diff), if diff == 0 { "matched" } else { "mismatch" })
        }
        None => (None, "no_snapshot"),
    };
    Ok(Reconciliation {
        series: "session_cumulative_snapshot".to_string(),
        detail_sum: detail,
        snapshot_final: snapshot,
        carried_sum: 0,
        difference,
        verdict: verdict.to_string(),
    })
}

/// Shared scan: step-finish parts produce model_call events and a persisted processing position.
/// Product callers supply a read-only connection, schema fingerprint, and version selection.
/// This function handles state restoration, resets, windows, events, reconciliation, and cursor updates.
/// Scan context: product identity, schema fingerprint, database version, and selection basis.
pub(crate) struct PartScanContext {
    pub product: PartProduct,
    pub fingerprint: String,
    pub db_version: Option<String>,
    pub basis: VersionBasis,
    pub record_basis: fn(Option<&str>) -> VersionBasis,
    pub row_limit: i64,
}

pub(crate) fn scan_step_finish_parts(
    conn: &Connection,
    target: &ScanTarget,
    stored: &StoredScanState,
    now_ms: i64,
    ctx: PartScanContext,
) -> Result<ScanOutcome, CoreError> {
    let PartScanContext {
        product,
        fingerprint,
        db_version,
        basis,
        record_basis,
        row_limit,
    } = ctx;
    // Do not filter the cursor by generation: same-database in-place rewrites trigger Rescan,
    // but part IDs and revisions retain a valid processing position.
    let cursor: PartCursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(PartCursor {
            generation: target.generation,
            offset: 0,
            watermark_ms: None,
            continuation: None,
            window_start_ms: None,
        });
    let context: PartParseContext = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // A changed schema fingerprint resets the position for a complete idempotent reread by ID.
    let fingerprint_reset = context.schema_fingerprint.is_some()
        && context.schema_fingerprint.as_deref() != Some(fingerprint.as_str());
    let policy_reset = context.scan_policy_version.as_deref() != Some(product.parser_version)
        && context.replay_policy_version.as_deref() != Some(product.parser_version);
    let reset = fingerprint_reset || policy_reset;
    let watermark = if reset { None } else { cursor.watermark_ms };
    let continuation = if reset {
        None
    } else {
        cursor.continuation.as_ref()
    };
    let since_ms = continuation.and(cursor.window_start_ms).unwrap_or_else(|| {
        watermark
            .map(|w| w.saturating_sub(WATERMARK_OVERLAP_MS))
            .unwrap_or(i64::MIN)
    });

    let mut rows = load_window(conn, since_ms, continuation, row_limit + 1)?;
    let hit_cap = rows.len() as i64 > row_limit;
    rows.truncate(row_limit as usize);
    let mut record_errors = if reset {
        Default::default()
    } else {
        context.record_errors.clone()
    };
    let mut has_unverified_records = !reset && context.has_unverified_records;

    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut touched_sessions: Vec<String> = Vec::new();
    for row in rows.iter() {
        crate::adapters::run_policy::check()?;
        records_seen += 1;
        if !touched_sessions.iter().any(|s| s == &row.session_id) {
            touched_sessions.push(row.session_id.clone());
        }
        let record_key = format!("{}:part:{}", product.ns, row.id);
        record_errors.remove(&record_key);
        let data: serde_json::Value = match crate::adapters::run_policy::json_from_str(&row.data) {
            Ok(v) => v,
            Err(_) => {
                // SQL already filters with json_valid; protect against rows changed during this read.
                diagnostics.push(diag(
                    "bad_data_json",
                    Some("data"),
                    &record_key,
                    "part.data is not valid JSON; row isolated, content not stored",
                ));
                record_errors.insert(record_key);
                continue;
            }
        };
        // Row time describes a derived-view write; skip timestamps before 2000 without guessing replacements.
        if row.time_created < crate::domain::MIN_PLAUSIBLE_MS {
            diagnostics.push(diag(
                "timestamp_implausible",
                Some("time_created"),
                &record_key,
                "part.time_created before 2000-01-01; row skipped, not zero-filled",
            ));
            record_errors.insert(record_key);
            continue;
        }
        let (usage, quality, cost) = match parse_step_finish(&data) {
            Some(raw) => {
                let mapped = if product.default_zero_unknown {
                    map_mimo_usage(&raw)
                } else {
                    map_opencode_family_usage(&raw)
                };
                for problem in &mapped.diagnostics {
                    diagnostics.push(diag(
                        problem.code,
                        Some(problem.field),
                        &record_key,
                        &problem.detail,
                    ));
                    record_errors.insert(record_key.clone());
                }
                (
                    mapped.usage,
                    mapped.quality,
                    map_family_cost(
                        data.get("cost")
                            .and_then(|c| c.as_f64())
                            .filter(|v| !product.default_zero_unknown || *v > 0.0),
                    ),
                )
            }
            None => {
                // A step-finish record identifies a completed step; count the call even when tokens are unknown.
                diagnostics.push(diag(
                    "usage_shape_deviation",
                    Some("tokens"),
                    &record_key,
                    "step-finish part without usable cost/tokens numbers; call counted, tokens unknown",
                ));
                record_errors.insert(record_key.clone());
                (
                    TokenUsage::default(),
                    TokenQuality::default(),
                    map_family_cost(
                        data.get("cost")
                            .and_then(|c| c.as_f64())
                            .filter(|v| !product.default_zero_unknown || *v > 0.0),
                    ),
                )
            }
        };
        let row_basis = record_basis(row.session_version.as_deref());
        has_unverified_records |= row_basis == VersionBasis::LatestFallback;
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: row
                .session_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            parser_version: product.parser_version.to_string(),
            parse_basis: Some(row_basis),
            origin_call_id: Some(row.message_id.clone()),
            attempt_id: None,
            session_id: Some(row.session_id.clone()),
            parent_session_id: row.parent_id.clone(),
            host_application: None,
            agent: product.agent.to_string(),
            call_category: if row.parent_id.is_some() {
                CallCategory::SubAgent
            } else {
                CallCategory::Primary
            },
            occurred_at_ms: row.time_created,
            observed_at_ms: Some(now_ms),
            source_time: Some(row.time_created.to_string()),
            // Use the part write time as observed_at; it does not establish exact upstream call time (A17).
            time_basis: TimeBasis::ObservedAt,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: row.provider_id.clone(),
            model_raw: row.model_id.clone(),
            model_canonical: None,
            model_attribution: if row.model_id.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage,
            quality,
            // The current part value is Final; later edits can replace it through time_updated revisions.
            lifecycle: Lifecycle::Final,
            source_revision: Some(row.time_updated),
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost,
        });
    }

    // Resume by timestamp and stable key; dense overlaps and same-millisecond rows must not repeat only the first page.
    let new_watermark = rows.last().map(|r| r.time_updated);
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
    if product.reconcile_session_counters && status == ScanStatus::Complete {
        for session_id in &touched_sessions {
            let rec = reconcile_session_counters(conn, session_id)?;
            if rec.verdict == "mismatch" {
                diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "reconcile_mismatch".to_string(),
                    field: Some("total_tokens".to_string()),
                    position: None,
                    message: format!(
                        "session step-finish detail sum {} != snapshot five-column sum {} (diff {})",
                        rec.detail_sum,
                        rec.snapshot_final.unwrap_or(0),
                        rec.difference.unwrap_or(0)
                    ),
                });
            }
            reconciliations.push(rec);
        }
    }

    let new_cursor = PartCursor {
        generation: target.generation,
        offset: 0,
        watermark_ms: next_watermark,
        continuation: if hit_cap {
            rows.last().map(|r| (r.time_updated, r.id.clone()))
        } else {
            None
        },
        window_start_ms: hit_cap.then_some(since_ms),
    };
    let new_context = PartParseContext {
        schema_fingerprint: Some(fingerprint),
        version_basis: Some(basis),
        db_version,
        scan_policy_version: if status == ScanStatus::Complete && record_errors.is_empty() {
            Some(product.parser_version.to_string())
        } else {
            context.scan_policy_version
        },
        replay_policy_version: if status == ScanStatus::BudgetExhausted {
            Some(product.parser_version.to_string())
        } else {
            None
        },
        has_unverified_records,
        record_errors,
    };
    let degraded = !new_context.record_errors.is_empty()
        || diagnostics.iter().any(|d| {
            matches!(
                d.code.as_str(),
                "bad_data_json" | "usage_shape_deviation" | "reconcile_mismatch"
            )
        });
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
        } else if has_unverified_records || basis == VersionBasis::LatestFallback {
            "active_compat".to_string()
        } else {
            "active".to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_step_finish_requires_projector_rule_and_five_fields() {
        let full = serde_json::json!({
            "type": "step-finish", "reason": "stop",
            "cost": 0.012,
            "tokens": {"total": 1230, "input": 1000, "output": 200,
                       "reasoning": 10, "cache": {"read": 15, "write": 5}}
        });
        let usage = parse_step_finish(&full).unwrap();
        assert_eq!(usage.input, 1000);
        assert_eq!(usage.total, Some(1230));

        // Missing cost or tokens returns None, matching projector's extraction rule.
        assert!(parse_step_finish(&serde_json::json!({
            "type": "step-finish", "tokens": {"input": 1, "output": 1, "reasoning": 0,
                                              "cache": {"read": 0, "write": 0}}
        }))
        .is_none());
        // A type other than step-finish returns None.
        assert!(parse_step_finish(&serde_json::json!({
            "type": "text", "cost": 0.1,
            "tokens": {"input": 1, "output": 1, "reasoning": 0,
                       "cache": {"read": 0, "write": 0}}
        }))
        .is_none());
        // Missing or excessive required numeric fields return None, matching the MiMo schema.
        assert!(parse_step_finish(&serde_json::json!({
            "type": "step-finish", "cost": 0.1,
            "tokens": {"input": 1, "output": 1, "reasoning": 0,
                       "cache": {"read": 0}}
        }))
        .is_none());
        assert!(parse_step_finish(&serde_json::json!({
            "type": "step-finish", "cost": 0.1,
            "tokens": {"input": -1, "output": 1, "reasoning": 0,
                       "cache": {"read": 0, "write": 0}}
        }))
        .is_none());
    }

    #[test]
    fn map_derives_inclusive_totals_from_pinned_semantics() {
        let m = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: 100,
            output: 40,
            reasoning: 5,
            cache_read: 50,
            cache_write: 10,
            total: Some(205),
        });
        // Input excludes cache in pinned publish-llm-event tokens(); derive inclusive input.
        assert_eq!(m.usage.input_uncached, Some(100));
        assert_eq!(
            m.usage.input_total,
            Some(160),
            "input+cr+cw（上游 sumTokens）"
        );
        assert_eq!(m.usage.output_total, Some(45), "output+reasoning 派生");
        assert_eq!(m.usage.total_tokens, Some(205), "五字段之和");
        assert_eq!(m.usage.source_total, Some(205));
        assert!(m.diagnostics.is_empty(), "直报 total 与派生一致");

        // Missing total leaves source_total=None while preserving the derived total.
        let missing = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: 10,
            output: 5,
            reasoning: 2,
            cache_read: 0,
            cache_write: 0,
            total: None,
        });
        assert_eq!(missing.usage.source_total, None);
        assert_eq!(missing.usage.total_tokens, Some(17));

        // Diagnose differences between source total and derived total when component relationships permit comparison.
        let bad = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: 10,
            output: 5,
            reasoning: 0,
            cache_read: 0,
            cache_write: 0,
            total: Some(99),
        });
        assert_eq!(bad.usage.total_tokens, Some(15), "规范化总量按派生口径");
        assert_eq!(bad.usage.source_total, Some(99), "直报值独立保留");
        assert!(bad
            .diagnostics
            .iter()
            .any(|d| d.code == "source_total_mismatch"));
    }

    #[test]
    fn map_overflow_degrades_to_unknown_not_panic() {
        let m = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: i64::MAX,
            output: 1,
            reasoning: 0,
            cache_read: 1,
            cache_write: 0,
            total: None,
        });
        // Overflowing derived fields remain None without panic or numeric truncation.
        assert_eq!(m.usage.input_total, None);
        assert_eq!(m.usage.total_tokens, None);
    }

    #[test]
    fn cost_maps_to_estimated_micro_usd() {
        let cost = map_family_cost(Some(0.0125)).unwrap();
        assert_eq!(cost.amount_minor, 12_500);
        assert_eq!(cost.kind, CostKind::Estimated);
        assert!(map_family_cost(None).is_none());
        assert!(map_family_cost(Some(-1.0)).is_none());
    }

    #[test]
    fn version_max_prefers_numeric_order() {
        assert_eq!(version_max("1.9.0", "1.10.0"), "1.10.0");
        assert_eq!(version_max("0.5.1", "0.5.0"), "0.5.1");
    }
}
