//! Historical source exchange rules (M1a, data-contract.md#provenance):
//! versioned exports and merge decisions for duplicates, newer revisions, disjoint contributions and conflicts.
//!
//! Boundaries:
//! - this module defines formats, builds exports and resolves records; aggregate writes live in exchange_import;
//! - display CSV/charts are not lossless reimport packages;
//! - batch_id prevents duplicate import operations without replacing source-record identity;
//! - exports may redact hostname labels while retaining stable source keys.

use crate::error::CoreError;
use crate::identity::{arbitrate, Arbitration, ExistingMeta};
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const EXCHANGE_FORMAT_VERSION: &str = "llm-usage-exchange-1";

/// Full or incremental export; omission from an incremental package is not deletion.
/// Explicit deletions declare removed source-record keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ExchangeKind {
    FullSnapshot,
    Incremental {
        /// Lower-bound event timestamp in milliseconds; None imposes no incremental lower bound.
        since_ms: Option<i64>,
        /// Deleted keys as (source_instance_id, source_record_key).
        /// Empty declarations mean no deletion.
        deletions: Vec<DeletedRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedRecord {
    pub source_instance_id: String,
    pub source_record_key: String,
    /// Deletion reason, such as observed source cleanup, without message content.
    pub reason: String,
}

/// Source host registration: hostname labels may be redacted, while host_id stays unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeHost {
    pub origin_host_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname_alias: Option<String>,
}

/// Original source-instance registration; an importing machine must not replace its ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeSource {
    pub source_instance_id: String,
    pub agent: String,
    pub format: String,
    pub parser_version: String,
    pub locality_basis: String,
    pub attribution_status: String,
    pub first_seen_ms: i64,
    /// Selected field coverage/capability JSON without message content.
    pub completeness: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_host_id: Option<String>,
}

/// Source plus record key identifies an exchange event; values and coverage travel with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeRecord {
    pub source_instance_id: String,
    pub source_record_key: String,
    pub record_kind: String,
    pub schema_version: String,
    pub parser_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_basis: Option<String>,
    pub occurred_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_time: Option<String>,
    pub time_basis: String,
    pub agent: String,
    pub call_category: String,
    pub lifecycle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// Eight nullable token fields; preserve unknowns without filling zeros.
    pub usage: ExchangeUsage,
    /// Event quality category: complete, partial, estimated or unknown, distinct from per-field quality.
    pub quality_bucket: String,
    pub source_revision: Option<i64>,
    pub conflict: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_note: Option<String>,
}

/// Values correspond to usage_events columns; null means unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_uncached: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_total: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_total: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_reasoning: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_total: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeDailyStatistics {
    pub attempt_count: i64,
    pub observation_count: i64,
    pub input_known_count: i64,
    pub input_unknown_count: i64,
    pub uncached_known_count: i64,
    pub cache_read_known_count: i64,
    pub cache_write_known_count: i64,
    pub output_known_count: i64,
    pub output_unknown_count: i64,
    pub total_known_count: i64,
    pub total_unknown_count: i64,
    pub ratio_sample_count: i64,
    pub uncached_known_sum: Option<i64>,
    pub ratio_input_sum: Option<i64>,
    pub ratio_cache_read_sum: Option<i64>,
}

/// Daily/sealed partitions preserve source/revision when details are deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeDailyPartition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics: Option<ExchangeDailyStatistics>,
    pub tz_version: String,
    pub local_day: String,
    pub instance_id: String,
    pub agent: String,
    pub provider_id: String,
    pub model_raw: String,
    pub call_category: String,
    pub quality_bucket: String,
    pub event_count: i64,
    pub call_count: i64,
    pub input_known_sum: Option<i64>,
    pub cache_read_known_sum: Option<i64>,
    pub cache_write_known_sum: Option<i64>,
    pub output_known_sum: Option<i64>,
    pub total_known_sum: Option<i64>,
    pub conflict_count: i64,
    pub sealed: bool,
    pub data_revision: i64,
}

/// Retained hourly rows merge by timezone/day/hour/dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeHourlyPartition {
    #[serde(default)]
    pub conflict_count: i64,
    pub tz_version: String,
    pub local_day: String,
    pub hour: i64,
    pub instance_id: String,
    pub agent: String,
    pub provider_id: String,
    pub model_raw: String,
    pub call_category: String,
    pub quality_bucket: String,
    pub event_count: i64,
    pub call_count: i64,
    pub input_known_sum: Option<i64>,
    pub cache_read_known_sum: Option<i64>,
    pub cache_write_known_sum: Option<i64>,
    pub output_known_sum: Option<i64>,
    pub total_known_sum: Option<i64>,
    pub data_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangePeriodPartition {
    pub tz_version: String,
    pub granularity: String,
    pub period_key: String,
    pub period_start_day: String,
    pub period_end_day: String,
    pub instance_id: String,
    pub agent: String,
    pub provider_id: String,
    pub model_raw: String,
    pub call_category: String,
    pub quality_bucket: String,
    pub event_count: i64,
    pub call_count: i64,
    pub conflict_count: i64,
    pub active_days: i64,
    pub materialized_at_ms: i64,
    pub data_revision: i64,
    pub input_known_sum: Option<i64>,
    pub cache_read_known_sum: Option<i64>,
    pub cache_write_known_sum: Option<i64>,
    pub output_known_sum: Option<i64>,
    pub total_known_sum: Option<i64>,
    pub distinct_sessions: Option<i64>,
}

/// Complete export package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeExport {
    pub format_version: String,
    pub kind: ExchangeKind,
    /// Batch identity prevents repeated imports without replacing record identity.
    pub batch_id: String,
    pub exported_at_ms: i64,
    pub timezone: String,
    pub host: ExchangeHost,
    pub sources: Vec<ExchangeSource>,
    pub records: Vec<ExchangeRecord>,
    /// All daily partitions, including recent days with retained details; aggregate imports use them
    /// without adding records again, and desktop aggregate exports omit records.
    #[serde(default)]
    pub daily_partitions: Vec<ExchangeDailyPartition>,
    /// Hourly history remains available after detail deletion.
    #[serde(default)]
    pub hourly_partitions: Vec<ExchangeHourlyPartition>,
    #[serde(default)]
    pub period_partitions: Vec<ExchangePeriodPartition>,
}

/// Export request.
#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub timezone: String,
    /// Event range [from_ms, to_ms) in milliseconds.
    pub from_ms: i64,
    pub to_ms: i64,
    /// None selects all sources; Some([]) selects none.
    pub instances: Option<Vec<String>>,
    /// True omits hostname labels from exports.
    pub redact_hostnames: bool,
    pub kind: ExchangeKind,
    pub batch_id: String,
}

/// Build an export through read-only queries.
pub fn build_export(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
) -> Result<ExchangeExport, CoreError> {
    build_export_inner(storage, request, now_ms, true)
}

/// Aggregate-only desktop exchange omits raw request/session identities.
pub fn build_aggregate_export(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
) -> Result<ExchangeExport, CoreError> {
    build_export_inner(storage, request, now_ms, false)
}

fn build_export_inner(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
    include_records: bool,
) -> Result<ExchangeExport, CoreError> {
    let snapshot = storage.conn().unchecked_transaction()?;
    let export = build_export_tx(storage, request, now_ms, include_records)?;
    snapshot.commit()?;
    Ok(export)
}

pub(crate) fn build_aggregate_export_tx(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
) -> Result<ExchangeExport, CoreError> {
    build_export_tx(storage, request, now_ms, false)
}

fn build_export_tx(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
    include_records: bool,
) -> Result<ExchangeExport, CoreError> {
    let local_host = storage.local_host_id()?;
    // The package uses the local host; each source retains its original host ownership on reexport.
    let Some(host_id) = local_host else {
        return Err(CoreError::Validation(
            "export requires an initialized local origin host".into(),
        ));
    };
    let hostname_alias = if request.redact_hostnames {
        None
    } else {
        let name: Option<String> = storage
            .conn()
            .query_row(
                "SELECT hostname FROM origin_host_names WHERE host_id = ?1
                 ORDER BY last_seen_ms DESC LIMIT 1",
                params![host_id],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        name
    };

    // Register included source instances and their original origin_host_id metadata.
    let instance_filter = &request.instances;
    let mut sources = Vec::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT instance_id, agent, format, parser_version, locality_basis,
                    attribution_status, created_at_ms, capabilities, origin_host_id
             FROM source_instances",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })?;
        for row in rows {
            let (
                instance_id,
                agent,
                format,
                parser_version,
                locality_basis,
                attribution_status,
                created_at_ms,
                capabilities,
                origin_host_id,
            ) = row?;
            if let Some(list) = &instance_filter {
                if !list.contains(&instance_id) {
                    continue;
                }
            }
            let completeness = capabilities
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .map(|c| c.get("fields").cloned().unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null);
            sources.push(ExchangeSource {
                source_instance_id: instance_id,
                agent,
                format: format.unwrap_or_default(),
                parser_version: parser_version.unwrap_or_default(),
                locality_basis,
                attribution_status,
                first_seen_ms: created_at_ms,
                completeness,
                origin_host_id: origin_host_id.filter(|id| id != "legacy_unknown"),
            });
        }
    }

    // Event details.
    let mut records = Vec::new();
    let wanted: BTreeSet<String> = sources
        .iter()
        .map(|s| s.source_instance_id.clone())
        .collect();
    if include_records {
        let mut stmt = storage.conn().prepare(
            "SELECT source_instance_id, source_record_key, record_kind, schema_version,
                    parser_version, parse_basis, occurred_at_ms, source_time, time_basis,
                    agent, call_category, lifecycle, provider_id, model_raw, origin_call_id,
                    session_id, parent_session_id, input_uncached, input_cache_read,
                    input_cache_write, input_total, output_total, output_reasoning,
                    total_tokens, source_total, quality_bucket, source_revision, conflict
             FROM usage_events
             WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2",
        )?;
        let rows = stmt.query_map(params![request.from_ms, request.to_ms], |r| {
            Ok(ExchangeRecord {
                source_instance_id: r.get(0)?,
                source_record_key: r.get(1)?,
                record_kind: r.get(2)?,
                schema_version: r.get(3)?,
                parser_version: r.get(4)?,
                parse_basis: r.get(5)?,
                occurred_at_ms: r.get(6)?,
                source_time: r.get(7)?,
                time_basis: r.get(8)?,
                agent: r.get(9)?,
                call_category: r.get(10)?,
                lifecycle: r.get(11)?,
                provider_id: r.get(12)?,
                model_raw: r.get(13)?,
                origin_call_id: r.get(14)?,
                session_id: r.get(15)?,
                parent_session_id: r.get(16)?,
                usage: ExchangeUsage {
                    input_uncached: r.get(17)?,
                    input_cache_read: r.get(18)?,
                    input_cache_write: r.get(19)?,
                    input_total: r.get(20)?,
                    output_total: r.get(21)?,
                    output_reasoning: r.get(22)?,
                    total_tokens: r.get(23)?,
                    source_total: r.get(24)?,
                },
                quality_bucket: r.get(25)?,
                source_revision: r.get(26)?,
                conflict: r.get::<_, i64>(27)? != 0,
                parse_note: None,
            })
        })?;
        for row in rows {
            let record = row?;
            if wanted.contains(&record.source_instance_id) {
                records.push(record);
            }
        }
    }

    // All daily partitions, including recent unsealed days, retain source/revision/coverage counts.
    let mut partitions = Vec::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT tz_version, local_day, instance_id, agent, provider_id, model_raw,
                    call_category, quality_bucket, event_count, call_count,
                    input_known_sum, cache_read_known_sum, cache_write_known_sum,
                    output_known_sum, total_known_sum, conflict_count, sealed, data_revision,
                    attempt_count, observation_count, input_known_count, input_unknown_count, uncached_known_count, cache_read_known_count, cache_write_known_count, output_known_count, output_unknown_count, total_known_count, total_unknown_count, ratio_sample_count, uncached_known_sum, ratio_input_sum, ratio_cache_read_sum
             FROM daily_usage WHERE tz_version = ?1",
        )?;
        let rows = stmt.query_map([&request.timezone], |r| {
            Ok(ExchangeDailyPartition {
                tz_version: r.get(0)?,
                local_day: r.get(1)?,
                instance_id: r.get(2)?,
                agent: r.get(3)?,
                provider_id: r.get(4)?,
                model_raw: r.get(5)?,
                call_category: r.get(6)?,
                quality_bucket: r.get(7)?,
                event_count: r.get(8)?,
                call_count: r.get(9)?,
                input_known_sum: r.get(10)?,
                cache_read_known_sum: r.get(11)?,
                cache_write_known_sum: r.get(12)?,
                output_known_sum: r.get(13)?,
                total_known_sum: r.get(14)?,
                conflict_count: r.get(15)?,
                sealed: r.get::<_, i64>(16)? != 0,
                data_revision: r.get(17)?,
                statistics: Some(ExchangeDailyStatistics {
                    attempt_count: r.get(18)?,
                    observation_count: r.get(19)?,
                    input_known_count: r.get(20)?,
                    input_unknown_count: r.get(21)?,
                    uncached_known_count: r.get(22)?,
                    cache_read_known_count: r.get(23)?,
                    cache_write_known_count: r.get(24)?,
                    output_known_count: r.get(25)?,
                    output_unknown_count: r.get(26)?,
                    total_known_count: r.get(27)?,
                    total_unknown_count: r.get(28)?,
                    ratio_sample_count: r.get(29)?,
                    uncached_known_sum: r.get(30)?,
                    ratio_input_sum: r.get(31)?,
                    ratio_cache_read_sum: r.get(32)?,
                }),
            })
        })?;
        for row in rows {
            let p = row?;
            if wanted.contains(&p.instance_id) && day_in_request(request, &p.local_day)? {
                partitions.push(p);
            }
        }
    }

    // Export only retained hourly rows; import merges by revision.
    let mut hourly = Vec::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT tz_version, local_day, hour, instance_id, agent, provider_id, model_raw,
                    call_category, quality_bucket, event_count, call_count,
                    input_known_sum, cache_read_known_sum, cache_write_known_sum,
                    output_known_sum, total_known_sum, data_revision, conflict_count
             FROM hourly_usage WHERE tz_version = ?1",
        )?;
        let rows = stmt.query_map([&request.timezone], |r| {
            Ok(ExchangeHourlyPartition {
                tz_version: r.get(0)?,
                local_day: r.get(1)?,
                hour: r.get(2)?,
                instance_id: r.get(3)?,
                agent: r.get(4)?,
                provider_id: r.get(5)?,
                model_raw: r.get(6)?,
                call_category: r.get(7)?,
                quality_bucket: r.get(8)?,
                event_count: r.get(9)?,
                call_count: r.get(10)?,
                input_known_sum: r.get(11)?,
                cache_read_known_sum: r.get(12)?,
                cache_write_known_sum: r.get(13)?,
                output_known_sum: r.get(14)?,
                total_known_sum: r.get(15)?,
                data_revision: r.get(16)?,
                conflict_count: r.get(17)?,
            })
        })?;
        for row in rows {
            let h = row?;
            if wanted.contains(&h.instance_id) && day_in_request(request, &h.local_day)? {
                hourly.push(h);
            }
        }
    }

    let mut periods = Vec::new();
    {
        let mut stmt = storage.conn().prepare("SELECT tz_version, granularity, period_key, period_start_day, period_end_day, instance_id, agent, provider_id, model_raw, call_category, quality_bucket, event_count, call_count, conflict_count, active_days, materialized_at_ms, data_revision, input_known_sum, cache_read_known_sum, cache_write_known_sum, output_known_sum, total_known_sum, distinct_sessions FROM period_usage WHERE tz_version=?1")?;
        let rows = stmt.query_map([&request.timezone], |r| {
            Ok(ExchangePeriodPartition {
                tz_version: r.get(0)?,
                granularity: r.get(1)?,
                period_key: r.get(2)?,
                period_start_day: r.get(3)?,
                period_end_day: r.get(4)?,
                instance_id: r.get(5)?,
                agent: r.get(6)?,
                provider_id: r.get(7)?,
                model_raw: r.get(8)?,
                call_category: r.get(9)?,
                quality_bucket: r.get(10)?,
                event_count: r.get(11)?,
                call_count: r.get(12)?,
                conflict_count: r.get(13)?,
                active_days: r.get(14)?,
                materialized_at_ms: r.get(15)?,
                data_revision: r.get(16)?,
                input_known_sum: r.get(17)?,
                cache_read_known_sum: r.get(18)?,
                cache_write_known_sum: r.get(19)?,
                output_known_sum: r.get(20)?,
                total_known_sum: r.get(21)?,
                distinct_sessions: r.get(22)?,
            })
        })?;
        for row in rows {
            let p = row?;
            if wanted.contains(&p.instance_id)
                && day_in_request(request, &p.period_start_day)?
                && day_in_request(request, &p.period_end_day)?
            {
                periods.push(p);
            }
        }
    }
    Ok(ExchangeExport {
        format_version: EXCHANGE_FORMAT_VERSION.to_string(),
        kind: request.kind.clone(),
        batch_id: request.batch_id.clone(),
        exported_at_ms: now_ms,
        timezone: request.timezone.clone(),
        host: ExchangeHost {
            origin_host_id: host_id,
            hostname_alias,
        },
        sources,
        records,
        daily_partitions: partitions,
        hourly_partitions: hourly,
        period_partitions: periods,
    })
}

fn day_in_request(request: &ExportRequest, day: &str) -> Result<bool, CoreError> {
    let calendar = crate::calendar::Calendar::new(&request.timezone)?;
    let (start, end) = calendar.day_range_ms(crate::calendar::parse_date(day)?)?;
    Ok(start >= request.from_ms && end <= request.to_ms)
}

/// Existing record metadata for an import merge decision.
#[derive(Debug, Clone)]
pub struct ExistingRecord {
    pub source_instance_id: String,
    pub source_record_key: String,
    pub lifecycle: crate::domain::Lifecycle,
    pub source_revision: Option<i64>,
    pub content_hash: String,
}

/// Merge result, following the data rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeDecision {
    /// Add a new record with independent coverage;
    /// still deduplicate mirrored events rather than summing solely because source IDs differ.
    AddIndependent,
    /// Same source/key/revision/content is a duplicate; repeated exchange does not add usage.
    SkipIdempotent,
    /// Replace after revoking the old contribution when revision/lifecycle establishes a newer correction;
    /// otherwise record Conflict without choosing by token magnitude.
    ReplaceAfterRevoke,
    /// Conflicting content with unresolved ordering retains the existing record and marks conflict.
    Conflict,
}

/// Reuse ingest resolution: revision first, then lifecycle;
/// conflicting content at the same level remains a conflict.
pub fn decide_record_merge(
    incoming: &ExchangeRecord,
    incoming_content_hash: &str,
    incoming_lifecycle: crate::domain::Lifecycle,
    existing: Option<&ExistingRecord>,
) -> MergeDecision {
    let Some(existing) = existing else {
        return MergeDecision::AddIndependent;
    };
    let same_identity = existing.source_instance_id == incoming.source_instance_id
        && existing.source_record_key == incoming.source_record_key;
    if !same_identity {
        return MergeDecision::AddIndependent;
    }
    let meta = ExistingMeta {
        lifecycle: existing.lifecycle,
        source_revision: existing.source_revision,
        content_hash: existing.content_hash.clone(),
    };
    match arbitrate(
        Some(&meta),
        &arbitration_event(incoming, incoming_lifecycle),
        incoming_content_hash,
    ) {
        Arbitration::Insert => MergeDecision::AddIndependent,
        Arbitration::Replace => MergeDecision::ReplaceAfterRevoke,
        Arbitration::Keep => {
            if meta.content_hash == incoming_content_hash {
                MergeDecision::SkipIdempotent
            } else {
                MergeDecision::Conflict
            }
        }
        Arbitration::Conflict => MergeDecision::Conflict,
    }
}

/// Build an EventInput for resolution of identity/revision/lifecycle/content hash.
/// This comparison does not read usage, so the temporary event has empty token fields.
fn arbitration_event(
    incoming: &ExchangeRecord,
    lifecycle: crate::domain::Lifecycle,
) -> crate::domain::EventInput {
    crate::domain::EventInput {
        source_instance_id: incoming.source_instance_id.clone(),
        source_record_key: incoming.source_record_key.clone(),
        record_kind: crate::domain::RecordKind::ModelCall,
        schema_version: incoming.schema_version.clone(),
        parser_version: incoming.parser_version.clone(),
        parse_basis: None,
        origin_call_id: None,
        attempt_id: None,
        session_id: None,
        parent_session_id: None,
        host_application: None,
        agent: incoming.agent.clone(),
        call_category: crate::domain::CallCategory::Primary,
        occurred_at_ms: incoming.occurred_at_ms,
        observed_at_ms: None,
        source_time: None,
        time_basis: crate::domain::TimeBasis::SourceCompletion,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: None,
        model_canonical: None,
        model_attribution: crate::domain::ModelAttribution::Unknown,
        usage: crate::domain::TokenUsage::default(),
        quality: crate::domain::TokenQuality::default(),
        lifecycle,
        source_revision: incoming.source_revision,
        error_status: None,
        duration_ms: None,
        ttft_ms: None,
        attribution_status: crate::domain::AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }
}
