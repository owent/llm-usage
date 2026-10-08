//! Commit event updates, cursors, parse context and affected daily aggregates in one ingest transaction.
//! Replay uncommitted interrupted batches without duplicates: update events by key and recompute daily partitions.

use crate::calendar::Calendar;
use crate::domain::{EventInput, QualityBucket};
use crate::error::CoreError;
use crate::identity::{
    arbitrate, content_hash, event_content_hash, event_id, Arbitration, ExistingMeta,
};
use crate::jobs::{self, RunStats};
use crate::metrics::detect_contradictions;
use crate::storage::Storage;
use jiff::civil::Date;
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::BTreeSet;

/// Existing usage_events metadata used to resolve duplicates.
type ExistingRow = (
    ExistingMeta,
    i64,
    String,
    Option<i64>,
    Option<String>,
    String,
    Option<crate::domain::VersionBasis>,
);

/// Cursor and versioned parse-context updates: model state, cumulative baseline and unfinished requests.
#[derive(Debug, Clone)]
pub struct CheckpointUpdate {
    pub scope_key: String,
    pub cursor_value: Option<serde_json::Value>,
    pub parse_context: Option<serde_json::Value>,
    pub source_revision: Option<i64>,
}

/// Redacted batch diagnostics: field names, error codes and positions, without original row content.
#[derive(Debug, Clone)]
pub struct DiagnosticInput {
    pub event_id: Option<String>,
    pub code: String,
    pub field: Option<String>,
    pub position: Option<String>,
    pub message: String,
}

/// One ingest batch; every event belongs to the same source instance.
#[derive(Debug, Clone)]
pub struct IngestBatch {
    pub batch_id: String,
    /// Source instance for this batch.
    pub instance_id: String,
    /// Fixed IANA timezone for grouping; stored settings do not follow later system changes.
    pub timezone: String,
    pub now_ms: i64,
    pub events: Vec<EventInput>,
    pub checkpoints: Vec<CheckpointUpdate>,
    pub diagnostics: Vec<DiagnosticInput>,
    /// Associated running job; commit its batch statistics in the same transaction.
    pub run_id: Option<String>,
    /// UTC-millisecond retention cutoff: skip older events with diagnostics instead of restoring deleted details.
    pub retention_cutoff_ms: Option<i64>,
}

/// V09 fault-injection points; failure must roll back the whole transaction and permit identical replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    AfterEvents,
    AfterCheckpoint,
    AfterParseContext,
    BeforeAggregates,
    AfterAggregates,
    BeforeCommit,
}

impl FaultPoint {
    fn name(self) -> &'static str {
        match self {
            FaultPoint::AfterEvents => "after_events",
            FaultPoint::AfterCheckpoint => "after_checkpoint",
            FaultPoint::AfterParseContext => "after_parse_context",
            FaultPoint::BeforeAggregates => "before_aggregates",
            FaultPoint::AfterAggregates => "after_aggregates",
            FaultPoint::BeforeCommit => "before_commit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome {
    pub added: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub skipped: i64,
    pub errors: i64,
    pub conflicts: i64,
    pub data_revision: i64,
    /// Local days affected by recomputation, formatted YYYY-MM-DD.
    pub affected_days: Vec<String>,
}

fn check_fault(fault: Option<FaultPoint>, point: FaultPoint) -> Result<(), CoreError> {
    if fault == Some(point) {
        return Err(CoreError::FaultInjected(point.name()));
    }
    Ok(())
}

/// Commit in one transaction; the fault test hook forces complete rollback.
pub fn commit_batch(
    storage: &Storage,
    batch: &IngestBatch,
    fault: Option<FaultPoint>,
) -> Result<BatchOutcome, CoreError> {
    let tx = storage.conn().unchecked_transaction()?;
    let outcome = commit_batch_tx(storage, &tx, batch, fault, false)?;
    tx.commit()?;
    Ok(outcome)
}

pub(crate) fn commit_batch_tx(
    storage: &Storage,
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    fault: Option<FaultPoint>,
    archive_snapshot: bool,
) -> Result<BatchOutcome, CoreError> {
    let calendar = Calendar::new(&batch.timezone)?;
    // Test-frozen schemas before v3 lack parse_basis; write using the historical schema
    // without that field. Normal startup migrates to SCHEMA_VERSION first.
    let parse_basis_column = storage.schema_version().unwrap_or(u32::MAX) >= 3;
    if batch
        .events
        .iter()
        .any(|e| e.source_instance_id != batch.instance_id)
    {
        return Err(CoreError::Validation(
            "batch events must belong to the batch source instance".into(),
        ));
    }
    if let Some(run_id) = &batch.run_id {
        let running: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM ingest_runs WHERE run_id = ?1 AND instance_id = ?2 AND status = 'running')",
            params![run_id, batch.instance_id], |r| r.get(0),
        )?;
        if !running {
            return Err(CoreError::JobState(
                "batch requires a running job for the same source".into(),
            ));
        }
    }
    let persisted_cutoff: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key = 'detail_retention_floor_ms'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let persisted_cutoff = persisted_cutoff
        .map(|v| {
            v.parse::<i64>()
                .map_err(|_| CoreError::Validation("invalid retention floor".into()))
        })
        .transpose()?;
    let cutoff = batch
        .retention_cutoff_ms
        .into_iter()
        .chain(persisted_cutoff)
        .max();
    let mut outcome = BatchOutcome {
        added: 0,
        updated: 0,
        unchanged: 0,
        skipped: 0,
        errors: 0,
        conflicts: 0,
        data_revision: 0,
        affected_days: Vec::new(),
    };
    let mut affected: BTreeSet<Date> = BTreeSet::new();
    let mut pending_diagnostics: Vec<(Option<String>, DiagnosticInput)> = Vec::new();
    let mut content_conflicts = BTreeSet::new();

    // 1. Insert or update events.
    for event in &batch.events {
        crate::adapters::run_policy::check()?;
        let eid = event_id(&event.source_instance_id, &event.source_record_key);
        if let Err(e) = event.validate() {
            outcome.errors += 1;
            pending_diagnostics.push((
                Some(eid),
                DiagnosticInput {
                    event_id: None,
                    code: "validation_failed".to_string(),
                    field: None,
                    position: None,
                    message: e.to_string(),
                },
            ));
            continue;
        }
        if let Some(cutoff) = cutoff.filter(|_| !archive_snapshot) {
            if event.occurred_at_ms < cutoff {
                outcome.skipped += 1;
                pending_diagnostics.push((
                    Some(eid),
                    DiagnosticInput {
                        event_id: None,
                        code: "expired_by_retention".to_string(),
                        field: Some("occurred_at_ms".to_string()),
                        position: None,
                        message: "event is older than the retention cutoff; not restored"
                            .to_string(),
                    },
                ));
                continue;
            }
        }
        let hash = event_content_hash(event);
        let mut existing: Option<ExistingRow> = tx
            .query_row(
                &format!("SELECT lifecycle, source_revision, content_hash, occurred_at_ms, event_id, observed_at_ms, source_time, parser_version, {}
                 FROM usage_events WHERE source_instance_id = ?1 AND source_record_key = ?2", if parse_basis_column { "parse_basis" } else { "NULL" }),
                params![event.source_instance_id, event.source_record_key],
                |r| {
                    Ok((
                        ExistingMeta {
                            lifecycle: crate::domain::Lifecycle::parse(&r.get::<_, String>(0)?)
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                            source_revision: r.get(1)?,
                            content_hash: r.get(2)?,
                        },
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, String>(7)?,
                        r.get::<_, Option<String>>(8)?.map(|s|crate::domain::VersionBasis::parse(&s)).transpose().map_err(|e|rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                    ))
                },
            )
            .optional()?;
        // The v1 content hash included observed_at; an observation-time change alone is duplicate content.
        // Same-key final repeats differing only in occurrence/observation/native time are repeated reports, not conflicts.
        if let Some((meta, old_ms, _, observed, old_source_time, _, _)) = &mut existing {
            let mut legacy = event.clone();
            legacy.observed_at_ms = *observed;
            if meta.content_hash == content_hash(&legacy) {
                meta.content_hash = hash.clone();
            }
            legacy.occurred_at_ms = *old_ms;
            legacy.source_time = old_source_time.clone();
            if meta.content_hash == event_content_hash(&legacy) {
                meta.content_hash = hash.clone();
            }
        }
        let eid = existing
            .as_ref()
            .map(|(_, _, id, _, _, _, _)| id.clone())
            .unwrap_or(eid);
        let arbitration = arbitrate(
            existing.as_ref().map(|(m, _, _, _, _, _, _)| m),
            event,
            &hash,
        );
        if matches!(arbitration, Arbitration::Conflict | Arbitration::Keep) {
            if let Some(old) = existing
                .as_ref()
                .filter(|old| parser_metadata_upgrade(old, event))
            {
                // Full prior hashes permit only parser-metadata changes; retain original time, usage and source revision.
                // Later metadata updates cannot clear a real conflict already observed in this batch.
                let mut current = event.clone();
                current.occurred_at_ms = old.1;
                current.source_time = old.4.clone();
                let new_hash = event_content_hash(&current);
                let basis_sql = if parse_basis_column {
                    ", parse_basis=?5"
                } else {
                    ""
                };
                let sql = format!(
                    "UPDATE usage_events SET parser_version=?1,content_hash=?2,updated_at_ms=?3,
                     conflict=CASE WHEN ?4 THEN conflict ELSE 0 END{basis_sql} WHERE event_id=?{}",
                    if parse_basis_column { 6 } else { 5 }
                );
                let mut values = vec![
                    rusqlite::types::Value::Text(event.parser_version.clone()),
                    rusqlite::types::Value::Text(new_hash),
                    rusqlite::types::Value::Integer(batch.now_ms),
                    rusqlite::types::Value::Integer(i64::from(content_conflicts.contains(&eid))),
                ];
                if parse_basis_column {
                    values.push(opt_text(&event.parse_basis.map(|b| b.as_str().to_string())));
                }
                values.push(rusqlite::types::Value::Text(eid.clone()));
                tx.execute(&sql, rusqlite::params_from_iter(values))?;
                outcome.updated += 1;
                affected.insert(calendar.local_day_of(old.1)?);
                pending_diagnostics.push((Some(eid), DiagnosticInput {
                    event_id: None,
                    code: "parser_metadata_updated".into(),
                    field: Some("parser_version".into()),
                    position: None,
                    message: "full stored content matched after normalizing parser metadata; usage preserved".into(),
                }));
                continue;
            }
        }
        if arbitration == Arbitration::Conflict {
            if let Some(old) = existing.as_ref().filter(|old| {
                gajae_policy_upgrade(old, event)
                    || junie_policy_upgrade(old, event)
                    || ui_message_policy_upgrade(old, event)
                    || mimo_policy_upgrade(old, event)
                    || claude_native_policy_upgrade(old, event)
            }) {
                // Explicit source-correction policies compare the complete old hash.
                // Retain first observation, real conflict flags and all change history
                // while correcting only verified fields.
                let conflict: i64 = tx.query_row(
                    "SELECT conflict FROM usage_events WHERE event_id=?1",
                    [&eid],
                    |r| r.get(0),
                )?;
                let mut current = event.clone();
                current.observed_at_ms = old.3;
                if event.agent == "claude-code" {
                    current.occurred_at_ms = old.1;
                    current.source_time = old.4.clone();
                }
                update_event(
                    tx,
                    &current,
                    &eid,
                    &event_content_hash(&current),
                    batch.now_ms,
                    parse_basis_column,
                )?;
                if conflict != 0 || content_conflicts.contains(&eid) {
                    tx.execute(
                        "UPDATE usage_events SET conflict=1 WHERE event_id=?1",
                        [&eid],
                    )?;
                }
                outcome.updated += 1;
                affected.insert(calendar.local_day_of(old.1)?);
                pending_diagnostics.push((Some(eid), DiagnosticInput {
                    event_id: None,
                    code: "parser_policy_updated".into(),
                    field: Some("parser_version".into()),
                    position: None,
                    message: format!("full prior {} digest matched verified source policy correction; revision and audit preserved", old.5),
                }));
                continue;
            }
        }
        let arbitration = if arbitration == Arbitration::Conflict
            && existing
                .as_ref()
                .is_some_and(|(meta, _, _, _, _, _, _)| vs_copilot_policy_upgrade(meta, event))
        {
            Arbitration::Replace
        } else {
            arbitration
        };
        match arbitration {
            Arbitration::Insert => {
                insert_event(tx, event, &eid, &hash, batch.now_ms, parse_basis_column)?;
                outcome.added += 1;
                affected.insert(calendar.local_day_of(event.occurred_at_ms)?);
            }
            Arbitration::Replace => {
                update_event(tx, event, &eid, &hash, batch.now_ms, parse_basis_column)?;
                outcome.updated += 1;
                affected.insert(calendar.local_day_of(event.occurred_at_ms)?);
                if let Some((_, old_ms, _, _, _, _, _)) = existing {
                    affected.insert(calendar.local_day_of(old_ms)?);
                }
            }
            Arbitration::Keep => {
                outcome.unchanged += 1;
            }
            Arbitration::Conflict => {
                content_conflicts.insert(eid.clone());
                // Keep the existing conflicting value rather than choose a larger one; diagnose and recompute its daily conflict count.
                tx.execute(
                    "UPDATE usage_events SET conflict = 1 WHERE event_id = ?1",
                    params![eid],
                )?;
                outcome.conflicts += 1;
                if let Some((_, old_ms, _, _, _, _, _)) = existing {
                    affected.insert(calendar.local_day_of(old_ms)?);
                }
                pending_diagnostics.push((
                    Some(eid.clone()),
                    DiagnosticInput {
                        event_id: None,
                        code: "update_conflict".to_string(),
                        field: None,
                        position: None,
                        message: "incoming record conflicts with existing; kept existing, not MAX"
                            .to_string(),
                    },
                ));
            }
        }
        // Record mathematical contradictions as diagnostics instead of hiding them with max(0, ...).
        for contradiction in detect_contradictions(&event.usage) {
            pending_diagnostics.push((
                Some(eid.clone()),
                DiagnosticInput {
                    event_id: None,
                    code: contradiction.code.to_string(),
                    field: Some(contradiction.field.to_string()),
                    position: None,
                    message: contradiction.detail,
                },
            ));
        }
    }
    crate::copilot_carriers::upgrade_otel_identity(tx, batch, &calendar, &mut affected)?;
    crate::copilot_carriers::select(tx, batch, &calendar, &mut affected)?;
    crate::qwen_carriers::select(tx, batch, &calendar, &mut affected)?;
    check_fault(fault, FaultPoint::AfterEvents)?;

    // 2. Commit cursors and parse context together; separate stages make fault-injection points explicit.
    for checkpoint in &batch.checkpoints {
        crate::adapters::run_policy::check()?;
        tx.execute(
            "INSERT INTO ingestion_checkpoints (instance_id, scope_key, cursor_value, source_revision, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(instance_id, scope_key) DO UPDATE SET
               cursor_value = excluded.cursor_value,
               source_revision = excluded.source_revision,
               updated_at_ms = excluded.updated_at_ms",
            params![
                batch.instance_id,
                checkpoint.scope_key,
                checkpoint.cursor_value.as_ref().map(serde_json::to_string).transpose()?,
                checkpoint.source_revision,
                batch.now_ms
            ],
        )?;
    }
    check_fault(fault, FaultPoint::AfterCheckpoint)?;
    for checkpoint in &batch.checkpoints {
        if let Some(context) = &checkpoint.parse_context {
            tx.execute(
                "UPDATE ingestion_checkpoints SET parse_context = ?1, updated_at_ms = ?2
                 WHERE instance_id = ?3 AND scope_key = ?4",
                params![
                    serde_json::to_string(context)?,
                    batch.now_ms,
                    batch.instance_id,
                    checkpoint.scope_key
                ],
            )?;
        }
    }
    check_fault(fault, FaultPoint::AfterParseContext)?;

    // 3. Advance the data revision when days are affected; recomputed daily rows carry that revision.
    let next_revision = if affected.is_empty() {
        crate::storage::data_revision(tx)?
    } else {
        Storage::bump_data_revision_tx(tx, batch.now_ms)?
    };
    outcome.data_revision = next_revision;
    check_fault(fault, FaultPoint::BeforeAggregates)?;

    // 4. Recompute affected timezone/day partitions without adding to sealed days; persist hours
    // in the same transaction so hourly archives survive later deletion of details.
    for day in &affected {
        crate::adapters::run_policy::check()?;
        recompute_day(tx, &calendar, *day, next_revision)?;
        persist_hourly_day(tx, &calendar, *day, next_revision)?;
    }
    outcome.affected_days = affected.iter().map(Date::to_string).collect();
    check_fault(fault, FaultPoint::AfterAggregates)?;

    // 5. Store redacted diagnostics.
    for (event_id, diag) in &pending_diagnostics {
        crate::adapters::run_policy::check()?;
        insert_diagnostic(tx, batch, event_id.as_deref(), diag)?;
    }
    for diag in &batch.diagnostics {
        crate::adapters::run_policy::check()?;
        insert_diagnostic(tx, batch, diag.event_id.as_deref(), diag)?;
    }

    // 6. Commit job progress in the same transaction.
    if let Some(run_id) = &batch.run_id {
        jobs::merge_run_stats_tx(
            tx,
            run_id,
            RunStats {
                added: outcome.added,
                updated: outcome.updated,
                unchanged: outcome.unchanged,
                skipped: outcome.skipped,
                errors: outcome.errors,
            },
            next_revision,
        )?;
    }
    check_fault(fault, FaultPoint::BeforeCommit)?;

    Ok(outcome)
}

/// Compare complete prior events; a renamed parser is not a source revision or permission to change other fields.
fn parser_metadata_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    if old.5 == event.parser_version && old.6 == event.parse_basis {
        return false;
    }
    let mut legacy = event.clone();
    legacy.parser_version = old.5.clone();
    legacy.parse_basis = old.6;
    // Match normal same-key final-repeat time handling; metadata updates still retain original timestamps.
    for preserve_time in [false, true] {
        if preserve_time {
            legacy.occurred_at_ms = old.1;
            legacy.source_time = old.4.clone();
        }
        if event_content_hash(&legacy) == old.0.content_hash {
            return true;
        }
        legacy.observed_at_ms = old.3;
        if content_hash(&legacy) == old.0.content_hash {
            return true;
        }
    }
    false
}

/// Reconstruct the complete doc1 event for the verified native writer only.
/// All positive counters, identity and attribution must still match the old hash.
fn claude_native_policy_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    use crate::adapters::claude::{
        common::map_claude_native, map_claude_transcript, ClaudeTranscriptUsage,
    };
    if event.agent != "claude-code"
        || event.schema_version != "2.1.197"
        || event.parser_version != "claude-transcript-doc2"
        || event.parse_basis != Some(crate::domain::VersionBasis::KnownVersion)
        || old.5 != "claude-transcript-doc1"
        || old.0.source_revision != event.source_revision
    {
        return false;
    }
    let raw = ClaudeTranscriptUsage {
        input_tokens: event.usage.input_uncached.unwrap_or(0),
        output_tokens: event.usage.output_total.unwrap_or(0),
        cache_read_input_tokens: event.usage.input_cache_read.unwrap_or(0),
        cache_creation_input_tokens: event.usage.input_cache_write.unwrap_or(0),
    };
    let expected = map_claude_native(&raw);
    if event.usage != expected.usage
        || event.quality != expected.quality
        || event.provider_id.is_some()
    {
        return false;
    }
    let mapped = map_claude_transcript(&raw);
    let mut legacy = event.clone();
    legacy.schema_version = "transcript-doc-1".into();
    legacy.parser_version = old.5.clone();
    legacy.parse_basis = Some(crate::domain::VersionBasis::KnownVersion);
    legacy.provider_id = Some("anthropic".into());
    legacy.usage = mapped.usage;
    legacy.quality = mapped.quality;
    legacy.observed_at_ms = old.3;
    for preserve_time in [false, true] {
        if preserve_time {
            legacy.occurred_at_ms = old.1;
            legacy.source_time = old.4.clone();
        }
        if event_content_hash(&legacy) == old.0.content_hash
            || content_hash(&legacy) == old.0.content_hash
        {
            return true;
        }
    }
    false
}

/// Only the verified gajae v5/OpenAI-completions policy permits these v2
/// unknown fields/start-time basis. Reconstruct every old field before comparing
/// canonical or legacy full hashes; resolve unrelated content changes normally.
fn gajae_policy_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    use crate::domain::{FieldQuality as Q, TimeBasis, VersionBasis};
    if event.agent != "gajae-code"
        || event.schema_version != "gjc-session-doc-1"
        || event.parser_version != "gjc-session-2"
        || old.5 != "gjc-session-1"
        || event.parse_basis != Some(VersionBasis::KnownVersion)
        || old.0.source_revision != event.source_revision
    {
        return false;
    }
    let mut legacy = event.clone();
    legacy.parser_version = old.5.clone();
    legacy.parse_basis = old.6;
    legacy.observed_at_ms = old.3;
    let mut changed = false;
    for (value, quality, old_quality) in [
        (
            &mut legacy.usage.input_cache_read,
            &mut legacy.quality.input_cache_read,
            Q::Reported,
        ),
        (
            &mut legacy.usage.input_cache_write,
            &mut legacy.quality.input_cache_write,
            Q::Reported,
        ),
        (
            &mut legacy.usage.input_total,
            &mut legacy.quality.input_total,
            Q::Derived,
        ),
        (
            &mut legacy.usage.output_total,
            &mut legacy.quality.output_total,
            Q::Reported,
        ),
        (
            &mut legacy.usage.source_total,
            &mut legacy.quality.source_total,
            Q::Reported,
        ),
    ] {
        if value.is_none() && *quality == Q::Unknown {
            *value = Some(0);
            *quality = old_quality;
            changed = true;
        }
    }
    if legacy.usage.input_uncached.is_none() && legacy.quality.input_uncached == Q::Unknown {
        let Some(uncached) = legacy
            .usage
            .input_total
            .zip(legacy.usage.input_cache_read)
            .zip(legacy.usage.input_cache_write)
            .and_then(|((i, r), w)| i.checked_sub(r)?.checked_sub(w))
            .filter(|v| *v >= 0)
        else {
            return false;
        };
        legacy.usage.input_uncached = Some(uncached);
        legacy.quality.input_uncached = Q::Reported;
        changed = true;
    }
    if legacy.usage.total_tokens.is_none() && legacy.quality.total_tokens == Q::Unknown {
        legacy.usage.total_tokens = legacy
            .usage
            .input_total
            .zip(legacy.usage.output_total)
            .and_then(|(i, o)| i.checked_add(o))
            .filter(|v| *v <= crate::domain::MAX_TOKEN_VALUE);
        legacy.quality.total_tokens = if legacy.usage.total_tokens.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        };
        changed = true;
    }
    if legacy.time_basis == TimeBasis::SourceStart {
        legacy.time_basis = TimeBasis::SourceCompletion;
        changed = true;
    }
    changed
        && (event_content_hash(&legacy) == old.0.content_hash
            || content_hash(&legacy) == old.0.content_hash)
}

/// Junie native uncached input and default-zero corrections compare at most 128
/// candidates restoring only missing/default-zero fields from the old parser;
/// every other field must match its full canonical or legacy hash.
fn junie_policy_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    use crate::domain::{CostAmount, CostKind, FieldQuality as Q, VersionBasis};
    if event.agent != "junie"
        || event.schema_version != "junie-events-doc-1"
        || event.parser_version != "junie-events-doc2"
        || old.5 != "junie-events-doc1"
        || event.parse_basis != Some(VersionBasis::KnownVersion)
        || old.0.source_revision != event.source_revision
        || event.usage.input_total.is_some()
        || event.quality.input_total != Q::Unknown
    {
        return false;
    }
    for mask in 0u8..128 {
        let mut legacy = event.clone();
        legacy.parser_version = old.5.clone();
        legacy.parse_basis = old.6;
        legacy.observed_at_ms = old.3;
        legacy.usage.input_total = legacy.usage.input_uncached.take();
        legacy.quality.input_total = legacy.quality.input_uncached;
        legacy.quality.input_uncached = Q::Unknown;
        if let Some(cost) = &mut legacy.cost {
            if cost.kind == CostKind::Estimated {
                cost.kind = CostKind::Reported;
            }
        }
        for (bit, value, quality) in [
            (
                0,
                &mut legacy.usage.input_total,
                &mut legacy.quality.input_total,
            ),
            (
                1,
                &mut legacy.usage.output_total,
                &mut legacy.quality.output_total,
            ),
            (
                2,
                &mut legacy.usage.input_cache_read,
                &mut legacy.quality.input_cache_read,
            ),
            (
                3,
                &mut legacy.usage.input_cache_write,
                &mut legacy.quality.input_cache_write,
            ),
            (
                4,
                &mut legacy.usage.output_reasoning,
                &mut legacy.quality.output_reasoning,
            ),
        ] {
            if mask & (1 << bit) != 0 && value.is_none() && *quality == Q::Unknown {
                *value = Some(0);
                *quality = Q::Reported;
            }
        }
        if mask & 32 != 0 && legacy.cost.is_none() {
            legacy.cost = Some(CostAmount {
                amount_minor: 0,
                currency: "USD".into(),
                kind: CostKind::Reported,
                price_version: None,
                billing_scope: None,
            });
        }
        if mask & 64 != 0 && legacy.duration_ms.is_none() && legacy.interval_start_ms.is_none() {
            legacy.duration_ms = Some(0);
            legacy.interval_start_ms = Some(legacy.occurred_at_ms);
        }
        if event_content_hash(&legacy) == old.0.content_hash
            || content_hash(&legacy) == old.0.content_hash
        {
            return true;
        }
    }
    false
}

/// MiMo required raw counters and client prices default to zero. Restore only those
/// absent/default fields and their old derived values; the complete prior hash
/// still preserves identity, every positive value, quality and attribution.
fn mimo_policy_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    use crate::domain::{CostAmount, CostKind, FieldQuality as Q};
    if event.agent != "mimo-code"
        || event.parser_version != "mimo-code-step-finish-parts-2"
        || old.5 != "mimo-code-step-finish-parts-1"
        || event.parse_basis != Some(crate::domain::VersionBasis::LatestFallback)
        || old.6 == Some(crate::domain::VersionBasis::KnownVersion)
        || old.0.source_revision != event.source_revision
    {
        return false;
    }
    let Some(output) = event
        .usage
        .output_total
        .unwrap_or(0)
        .checked_sub(event.usage.output_reasoning.unwrap_or(0))
        .filter(|v| *v >= 0)
    else {
        return false;
    };
    let expected = crate::adapters::opencode_family::map_mimo_usage(
        &crate::adapters::opencode_family::OpencodeFamilyUsage {
            input: event.usage.input_uncached.unwrap_or(0),
            output,
            reasoning: event.usage.output_reasoning.unwrap_or(0),
            cache_read: event.usage.input_cache_read.unwrap_or(0),
            cache_write: event.usage.input_cache_write.unwrap_or(0),
            total: event.usage.source_total,
        },
    );
    if expected.usage != event.usage || expected.quality != event.quality {
        return false;
    }
    // The old parser required five raw counters. Reconstruct its complete summary;
    // an unchanged part ID alone cannot authorize a correction.
    for mask in 0u8..4 {
        let mut legacy = event.clone();
        legacy.parser_version = old.5.clone();
        legacy.parse_basis = old.6;
        legacy.observed_at_ms = old.3;
        for (value, quality) in [
            (
                &mut legacy.usage.input_uncached,
                &mut legacy.quality.input_uncached,
            ),
            (
                &mut legacy.usage.input_cache_read,
                &mut legacy.quality.input_cache_read,
            ),
            (
                &mut legacy.usage.input_cache_write,
                &mut legacy.quality.input_cache_write,
            ),
            (
                &mut legacy.usage.output_reasoning,
                &mut legacy.quality.output_reasoning,
            ),
        ] {
            if value.is_none() && *quality == Q::Unknown {
                *value = Some(0);
                *quality = Q::Reported;
            }
        }
        legacy.usage.input_total = legacy
            .usage
            .input_uncached
            .zip(legacy.usage.input_cache_read)
            .zip(legacy.usage.input_cache_write)
            .and_then(|((i, r), w)| i.checked_add(r)?.checked_add(w));
        legacy.quality.input_total = if legacy.usage.input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        };
        if legacy.usage.output_total.is_none() && legacy.quality.output_total == Q::Unknown {
            legacy.usage.output_total = Some(0);
            legacy.quality.output_total = Q::Derived;
        }
        if mask & 1 != 0
            && legacy.usage.source_total.is_none()
            && legacy.quality.source_total == Q::Unknown
        {
            legacy.usage.source_total = Some(0);
            legacy.quality.source_total = Q::Reported;
        }
        let derived = legacy
            .usage
            .input_total
            .zip(legacy.usage.output_total)
            .and_then(|(i, o)| i.checked_add(o));
        legacy.usage.total_tokens = derived.or(legacy.usage.source_total);
        legacy.quality.total_tokens = if derived.is_some() {
            Q::Derived
        } else if legacy.usage.source_total.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        };
        if mask & 2 != 0 && legacy.cost.is_none() {
            legacy.cost = Some(CostAmount {
                amount_minor: 0,
                currency: "USD".into(),
                kind: CostKind::Estimated,
                price_version: None,
                billing_scope: None,
            });
        }
        if event_content_hash(&legacy) == old.0.content_hash
            || content_hash(&legacy) == old.0.content_hash
        {
            return true;
        }
    }
    false
}

fn ui_message_policy_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    use crate::domain::{CostAmount, CostKind, FieldQuality as Q, VersionBasis};
    let (schema, current, prior, derive_uncached) = match event.agent.as_str() {
        "roo-code" => (
            "roo-ui-messages-doc-1",
            "roo-ui-messages-doc2",
            "roo-ui-messages-doc1",
            true,
        ),
        "zoo-code" => (
            "zoo-ui-messages-doc-1",
            "zoo-ui-messages-doc2",
            "zoo-ui-messages-doc1",
            false,
        ),
        _ => return false,
    };
    if event.schema_version != schema
        || event.parser_version != current
        || old.5 != prior
        || event.parse_basis != Some(VersionBasis::KnownVersion)
        || old.0.source_revision != event.source_revision
    {
        return false;
    }
    for mask in 0u8..32 {
        let mut legacy = event.clone();
        legacy.parser_version = old.5.clone();
        legacy.parse_basis = old.6;
        legacy.observed_at_ms = old.3;
        for (bit, value, quality) in [
            (
                0,
                &mut legacy.usage.input_total,
                &mut legacy.quality.input_total,
            ),
            (
                1,
                &mut legacy.usage.output_total,
                &mut legacy.quality.output_total,
            ),
            (
                2,
                &mut legacy.usage.input_cache_read,
                &mut legacy.quality.input_cache_read,
            ),
            (
                3,
                &mut legacy.usage.input_cache_write,
                &mut legacy.quality.input_cache_write,
            ),
        ] {
            if mask & (1 << bit) != 0 && value.is_none() && *quality == Q::Unknown {
                *value = Some(0);
                *quality = Q::Reported;
            }
        }
        if derive_uncached
            && legacy.usage.input_uncached.is_none()
            && legacy.quality.input_uncached == Q::Unknown
        {
            legacy.usage.input_uncached = legacy
                .usage
                .input_total
                .zip(legacy.usage.input_cache_read)
                .zip(legacy.usage.input_cache_write)
                .and_then(|((i, r), w)| i.checked_sub(r)?.checked_sub(w))
                .filter(|v| *v >= 0);
            if legacy.usage.input_uncached.is_some() {
                legacy.quality.input_uncached = Q::Derived;
            }
        }
        if legacy.usage.total_tokens.is_none() && legacy.quality.total_tokens == Q::Unknown {
            legacy.usage.total_tokens = legacy
                .usage
                .input_total
                .zip(legacy.usage.output_total)
                .and_then(|(i, o)| i.checked_add(o));
            if legacy.usage.total_tokens.is_some() {
                legacy.quality.total_tokens = Q::Derived;
            }
        }
        if mask & 16 != 0 && legacy.cost.is_none() {
            legacy.cost = Some(CostAmount {
                amount_minor: 0,
                currency: "USD".into(),
                kind: CostKind::Estimated,
                price_version: None,
                billing_scope: None,
            });
        }
        if event_content_hash(&legacy) == old.0.content_hash
            || content_hash(&legacy) == old.0.content_hash
        {
            return true;
        }
    }
    false
}

fn insert_diagnostic(
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    event_id: Option<&str>,
    diag: &DiagnosticInput,
) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO diagnostics (batch_id, run_id, instance_id, event_id, code, field, position, message, created_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            batch.batch_id,
            batch.run_id,
            batch.instance_id,
            event_id,
            diag.code,
            diag.field,
            diag.position,
            diag.message,
            batch.now_ms
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    tx: &Transaction<'_>,
    event: &EventInput,
    event_id: &str,
    hash: &str,
    now_ms: i64,
    parse_basis_column: bool,
) -> Result<(), CoreError> {
    let sql = if parse_basis_column {
        "INSERT INTO usage_events (
           event_id, source_instance_id, source_record_key, record_kind, schema_version, parser_version,
           origin_call_id, attempt_id, session_id, parent_session_id, host_application, agent, call_category,
           occurred_at_ms, observed_at_ms, source_time, time_basis, interval_start_ms, interval_end_ms,
           provider_id, model_raw, model_canonical, model_attribution,
           input_uncached, input_cache_read, input_cache_write, input_total, output_total, output_reasoning,
           total_tokens, source_total, quality_json, quality_bucket, lifecycle, source_revision,
           error_status, duration_ms, ttft_ms, attribution_status, exclusion_reason, conflict, content_hash,
           cost_amount_minor, cost_currency, cost_kind, price_version, billing_scope,
           created_at_ms, updated_at_ms, parse_basis
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
           ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36,
           ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50
         )"
    } else {
        "INSERT INTO usage_events (
           event_id, source_instance_id, source_record_key, record_kind, schema_version, parser_version,
           origin_call_id, attempt_id, session_id, parent_session_id, host_application, agent, call_category,
           occurred_at_ms, observed_at_ms, source_time, time_basis, interval_start_ms, interval_end_ms,
           provider_id, model_raw, model_canonical, model_attribution,
           input_uncached, input_cache_read, input_cache_write, input_total, output_total, output_reasoning,
           total_tokens, source_total, quality_json, quality_bucket, lifecycle, source_revision,
           error_status, duration_ms, ttft_ms, attribution_status, exclusion_reason, conflict, content_hash,
           cost_amount_minor, cost_currency, cost_kind, price_version, billing_scope,
           created_at_ms, updated_at_ms
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
           ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36,
           ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49
         )"
    };
    let mut params = event_params(event, event_id, hash, now_ms, now_ms);
    if !parse_basis_column {
        params.pop();
    }
    tx.execute(sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

fn update_event(
    tx: &Transaction<'_>,
    event: &EventInput,
    event_id: &str,
    hash: &str,
    now_ms: i64,
    parse_basis_column: bool,
) -> Result<(), CoreError> {
    let sql = if parse_basis_column {
        "UPDATE usage_events SET
           record_kind = ?4, schema_version = ?5, parser_version = ?6,
           origin_call_id = ?7, attempt_id = ?8, session_id = ?9, parent_session_id = ?10,
           host_application = ?11, agent = ?12, call_category = ?13,
           occurred_at_ms = ?14, observed_at_ms = ?15, source_time = ?16, time_basis = ?17,
           interval_start_ms = ?18, interval_end_ms = ?19,
           provider_id = ?20, model_raw = ?21, model_canonical = ?22, model_attribution = ?23,
           input_uncached = ?24, input_cache_read = ?25, input_cache_write = ?26, input_total = ?27,
           output_total = ?28, output_reasoning = ?29, total_tokens = ?30, source_total = ?31,
           quality_json = ?32, quality_bucket = ?33, lifecycle = ?34, source_revision = ?35,
           error_status = ?36, duration_ms = ?37, ttft_ms = ?38,
           attribution_status = ?39, exclusion_reason = ?40, conflict = 0, content_hash = ?42,
           cost_amount_minor = ?43, cost_currency = ?44, cost_kind = ?45, price_version = ?46,
           billing_scope = ?47, updated_at_ms = ?49, parse_basis = ?50
         WHERE event_id = ?1"
    } else {
        "UPDATE usage_events SET
           record_kind = ?4, schema_version = ?5, parser_version = ?6,
           origin_call_id = ?7, attempt_id = ?8, session_id = ?9, parent_session_id = ?10,
           host_application = ?11, agent = ?12, call_category = ?13,
           occurred_at_ms = ?14, observed_at_ms = ?15, source_time = ?16, time_basis = ?17,
           interval_start_ms = ?18, interval_end_ms = ?19,
           provider_id = ?20, model_raw = ?21, model_canonical = ?22, model_attribution = ?23,
           input_uncached = ?24, input_cache_read = ?25, input_cache_write = ?26, input_total = ?27,
           output_total = ?28, output_reasoning = ?29, total_tokens = ?30, source_total = ?31,
           quality_json = ?32, quality_bucket = ?33, lifecycle = ?34, source_revision = ?35,
           error_status = ?36, duration_ms = ?37, ttft_ms = ?38,
           attribution_status = ?39, exclusion_reason = ?40, conflict = 0, content_hash = ?42,
           cost_amount_minor = ?43, cost_currency = ?44, cost_kind = ?45, price_version = ?46,
           billing_scope = ?47, updated_at_ms = ?49
         WHERE event_id = ?1"
    };
    let mut params = event_params(event, event_id, hash, 0, now_ms);
    if !parse_basis_column {
        params.pop();
    }
    tx.execute(sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

/// Shared INSERT/UPDATE parameter order; ?41 is conflict (INSERT uses literal zero).
/// ?48 created_at_ms is used only by INSERT, not UPDATE.
fn event_params(
    event: &EventInput,
    event_id: &str,
    hash: &str,
    _created_ms: i64,
    now_ms: i64,
) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value;
    let quality_json = serde_json::to_string(&event.quality).unwrap_or_else(|_| "{}".to_string());
    let bucket = QualityBucket::of(&event.usage, &event.quality);
    let (cost_minor, cost_currency, cost_kind, price_version, billing_scope) = match &event.cost {
        Some(c) => (
            Some(c.amount_minor),
            Some(c.currency.clone()),
            Some(c.kind.as_str().to_string()),
            c.price_version.clone(),
            c.billing_scope.clone(),
        ),
        None => (None, None, None, None, None),
    };
    vec![
        Value::Text(event_id.to_string()),                     // 1 event_id
        Value::Text(event.source_instance_id.clone()),         // 2 source_instance_id
        Value::Text(event.source_record_key.clone()),          // 3 source_record_key
        Value::Text(event.record_kind.as_str().to_string()),   // 4 record_kind
        Value::Text(event.schema_version.clone()),             // 5 schema_version
        Value::Text(event.parser_version.clone()),             // 6 parser_version
        opt_text(&event.origin_call_id),                       // 7 origin_call_id
        opt_text(&event.attempt_id),                           // 8 attempt_id
        opt_text(&event.session_id),                           // 9 session_id
        opt_text(&event.parent_session_id),                    // 10 parent_session_id
        opt_text(&event.host_application),                     // 11 host_application
        Value::Text(event.agent.clone()),                      // 12 agent
        Value::Text(event.call_category.as_str().to_string()), // 13 call_category
        Value::Integer(event.occurred_at_ms),                  // 14 occurred_at_ms
        opt_int(event.observed_at_ms),                         // 15 observed_at_ms
        opt_text(&event.source_time),                          // 16 source_time
        Value::Text(event.time_basis.as_str().to_string()),    // 17 time_basis
        opt_int(event.interval_start_ms),                      // 18 interval_start_ms
        opt_int(event.interval_end_ms),                        // 19 interval_end_ms
        opt_text(&event.provider_id),                          // 20 provider_id
        opt_text(&event.model_raw),                            // 21 model_raw
        opt_text(&event.model_canonical),                      // 22 model_canonical
        Value::Text(event.model_attribution.as_str().to_string()), // 23 model_attribution
        opt_int(event.usage.input_uncached),                   // 24 input_uncached
        opt_int(event.usage.input_cache_read),                 // 25 input_cache_read
        opt_int(event.usage.input_cache_write),                // 26 input_cache_write
        opt_int(event.usage.input_total),                      // 27 input_total
        opt_int(event.usage.output_total),                     // 28 output_total
        opt_int(event.usage.output_reasoning),                 // 29 output_reasoning
        opt_int(event.usage.total_tokens),                     // 30 total_tokens
        opt_int(event.usage.source_total),                     // 31 source_total
        Value::Text(quality_json),                             // 32 quality_json
        Value::Text(bucket.as_str().to_string()),              // 33 quality_bucket
        Value::Text(event.lifecycle.as_str().to_string()),     // 34 lifecycle
        opt_int(event.source_revision),                        // 35 source_revision
        opt_text(&event.error_status),                         // 36 error_status
        opt_int(event.duration_ms),                            // 37 duration_ms
        opt_int(event.ttft_ms),                                // 38 ttft_ms
        Value::Text(event.attribution_status.as_str().to_string()), // 39 attribution_status
        opt_text(&event.exclusion_reason),                     // 40 exclusion_reason
        Value::Integer(0),                                     // 41 conflict placeholder
        Value::Text(hash.to_string()),                         // 42 content_hash
        opt_int(cost_minor),                                   // 43 cost_minor
        opt_text(&cost_currency),                              // 44 cost_currency
        opt_text(&cost_kind),                                  // 45 cost_kind
        opt_text(&price_version),                              // 46 price_version
        opt_text(&billing_scope),                              // 47 billing_scope
        Value::Integer(now_ms),                                // 48 created_at_ms (INSERT)
        Value::Integer(now_ms),                                // 49 updated_at_ms
        opt_text(&event.parse_basis.map(|b| b.as_str().to_string())), // 50 parse_basis
    ]
}

fn opt_text(value: &Option<String>) -> rusqlite::types::Value {
    match value {
        Some(v) => rusqlite::types::Value::Text(v.clone()),
        None => rusqlite::types::Value::Null,
    }
}

fn opt_int(value: Option<i64>) -> rusqlite::types::Value {
    match value {
        Some(v) => rusqlite::types::Value::Integer(v),
        None => rusqlite::types::Value::Null,
    }
}

/// Rebuild one local day from usage_events after deleting its unsealed rows.
/// Include only verified ownership in totals; transport_attempt contributes to attempt_count only.
pub(crate) fn recompute_day(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
    data_revision: i64,
) -> Result<(), CoreError> {
    let day_str = day.to_string();
    tx.execute(
        "DELETE FROM daily_usage WHERE tz_version = ?1 AND local_day = ?2 AND sealed = 0",
        params![calendar.tz_name(), day_str],
    )?;
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    // Since v4, daily_usage keeps each source-instance contribution; queries sum across sources (M1a).
    // A v2 migration rebuilds before v4 partitioning, using the old shape without instance_id.
    let partitioned: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('daily_usage') WHERE name = 'instance_id')",
        [],
        |r| r.get(0),
    )?;
    let insert_head = if partitioned {
        "INSERT INTO daily_usage (
           tz_version, local_day, instance_id, agent, provider_id, model_raw, call_category, quality_bucket,
           event_count, call_count, attempt_count, observation_count,
           input_known_sum, input_known_count, input_unknown_count,
           uncached_known_sum, uncached_known_count,
           cache_read_known_sum, cache_read_known_count,
           cache_write_known_sum, cache_write_known_count,
           output_known_sum, output_known_count, output_unknown_count,
           total_known_sum, total_known_count, total_unknown_count,
           ratio_input_sum, ratio_cache_read_sum, ratio_sample_count,
           conflict_count, sealed, data_revision
         )
         SELECT
           ?1, ?2, source_instance_id, agent,"
    } else {
        "INSERT INTO daily_usage (
           tz_version, local_day, agent, provider_id, model_raw, call_category, quality_bucket,
           event_count, call_count, attempt_count, observation_count,
           input_known_sum, input_known_count, input_unknown_count,
           uncached_known_sum, uncached_known_count,
           cache_read_known_sum, cache_read_known_count,
           cache_write_known_sum, cache_write_known_count,
           output_known_sum, output_known_count, output_unknown_count,
           total_known_sum, total_known_count, total_unknown_count,
           ratio_input_sum, ratio_cache_read_sum, ratio_sample_count,
           conflict_count, sealed, data_revision
         )
         SELECT
           ?1, ?2, agent,"
    };
    let group_by = if partitioned {
        "GROUP BY source_instance_id, agent, COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category, quality_bucket"
    } else {
        "GROUP BY agent, COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category, quality_bucket"
    };
    let unsealed = if partitioned {
        "AND NOT EXISTS (SELECT 1 FROM daily_usage d WHERE d.tz_version=?1
         AND d.local_day=?2 AND d.instance_id=source_instance_id AND d.sealed=1)"
    } else {
        "AND NOT EXISTS (SELECT 1 FROM daily_usage d WHERE d.tz_version=?1 AND d.local_day=?2 AND d.sealed=1)"
    };
    // input/output/total_unknown_count exclude quality_bucket='unknown' records:
    // records with no known token field include failed calls and Copilot round markers.
    // Calls remain counted while turn observations supply usage separately;
    // include call/event counts without treating these as partially observed missing token fields.
    let sql = format!(
        "{insert_head}
           COALESCE(provider_id, ''), COALESCE(model_raw, ''),
           call_category, quality_bucket,
           COUNT(*),
           SUM(CASE WHEN record_kind = 'model_call' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind = 'transport_attempt' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind = 'usage_observation' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_input END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND quality_bucket != 'unknown' AND known_input IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_uncached END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_uncached IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_read END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_read IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_write END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_write IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_output END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_output IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND quality_bucket != 'unknown' AND known_output IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_total END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_total IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND quality_bucket != 'unknown' AND known_total IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL AND known_read IS NOT NULL THEN known_input END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL AND known_read IS NOT NULL THEN known_read END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL AND known_read IS NOT NULL THEN 1 ELSE 0 END),
           SUM(conflict),
           0, ?3
         FROM (
           SELECT *,
             CASE WHEN json_extract(quality_json, '$.input_total') IN ('reported', 'derived') THEN input_total END AS known_input,
             CASE WHEN json_extract(quality_json, '$.input_uncached') IN ('reported', 'derived') THEN input_uncached END AS known_uncached,
             CASE WHEN json_extract(quality_json, '$.input_cache_read') IN ('reported', 'derived') THEN input_cache_read END AS known_read,
             CASE WHEN json_extract(quality_json, '$.input_cache_write') IN ('reported', 'derived') THEN input_cache_write END AS known_write,
             CASE WHEN json_extract(quality_json, '$.output_total') IN ('reported', 'derived') THEN output_total END AS known_output,
             CASE WHEN json_extract(quality_json, '$.total_tokens') IN ('reported', 'derived') THEN total_tokens END AS known_total
           FROM usage_events
         )
         WHERE occurred_at_ms >= ?4 AND occurred_at_ms < ?5
           AND attribution_status = 'verified'
           AND record_kind IN ('model_call', 'transport_attempt', 'usage_observation')
         {unsealed} {group_by}"
    );
    tx.execute(
        &sql,
        params![calendar.tz_name(), day_str, data_revision, start_ms, end_ms],
    )?;
    crate::query_acceleration::rebuild_day(tx, calendar, day)?;
    Ok(())
}

/// Repair only the verified v1/v2 total omission; every other native field must match.
fn vs_copilot_policy_upgrade(existing: &crate::identity::ExistingMeta, event: &EventInput) -> bool {
    let expected_total = event
        .usage
        .input_total
        .zip(event.usage.output_total)
        .and_then(|(i, o)| i.checked_add(o))
        .filter(|v| *v <= crate::domain::MAX_TOKEN_VALUE);
    let expected_quality = if expected_total.is_some() {
        crate::domain::FieldQuality::Derived
    } else {
        crate::domain::FieldQuality::Unknown
    };
    if event.agent != "vs-copilot"
        || event.parser_version != "vs-copilot-otlp-traces-3"
        || event.quality.total_tokens != expected_quality
        || event.usage.total_tokens != expected_total
    {
        return false;
    }
    let mut legacy = event.clone();
    legacy.usage.total_tokens = None;
    legacy.quality.total_tokens = crate::domain::FieldQuality::Unknown;
    ["vs-copilot-otlp-traces-1", "vs-copilot-otlp-traces-2"]
        .into_iter()
        .any(|parser| {
            legacy.parser_version = parser.into();
            existing.content_hash == event_content_hash(&legacy)
        })
}

/// Recompute local days covering inclusive [from_ms, to_ms] in the requested timezone.
/// Use retained events, skip sealed days and commit recomputation with its revision in one transaction.
/// Return the new revision committed by this transaction.
pub fn recompute_days_in_tz(
    storage: &Storage,
    timezone: &str,
    from_ms: i64,
    to_ms: i64,
    now_ms: i64,
) -> Result<i64, CoreError> {
    let calendar = Calendar::new(timezone)?;
    let mut day = calendar.local_day_of(from_ms)?;
    let last = calendar.local_day_of(to_ms)?;
    // Limit recomputation to 750 days; reject larger ranges for the caller to split.
    let mut guard = 0;
    let conn = storage.conn();
    let tx = conn.unchecked_transaction()?;
    let revision = Storage::bump_data_revision_tx(&tx, now_ms)?;
    while day <= last {
        guard += 1;
        if guard > 750 {
            return Err(CoreError::Validation(
                "recompute_days_in_tz: range exceeds 750 days; batch the repair".into(),
            ));
        }
        recompute_day(&tx, &calendar, day, revision)?;
        persist_hourly_day(&tx, &calendar, day, revision)?;
        day = day
            .checked_add(jiff::Span::new().days(1))
            .map_err(|e| CoreError::Validation(format!("day advance: {e}")))?;
    }
    tx.commit()?;
    Ok(revision)
}

/// Rebuild hourly aggregates by local day for the hourly retention layer.
/// Use the same instance/Agent/provider/model/category/quality dimensions as daily totals.
/// Call with recompute_day in the same transaction; sealed days keep their frozen hourly rows.
/// Use the local offset at each event time, preserving 23/25-hour DST days.
pub(crate) fn persist_hourly_day(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
    data_revision: i64,
) -> Result<(), CoreError> {
    // Test-frozen schemas before v5 have no hourly_usage table; skip hourly persistence.
    // Migration enables it later without changing daily/detail meanings.
    let has_table: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('hourly_usage'))",
        [],
        |r| r.get(0),
    )?;
    if !has_table {
        return Ok(());
    }
    let tz = calendar.tz_name().to_string();
    let day_str = day.to_string();
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    tx.execute(
        "DELETE FROM hourly_usage WHERE tz_version = ?1 AND local_day = ?2
         AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.tz_version=?1 AND d.local_day=?2
           AND d.instance_id=hourly_usage.instance_id AND d.sealed=1)",
        params![tz, day_str],
    )?;
    let mut stmt = tx.prepare(
        "SELECT source_instance_id, occurred_at_ms, record_kind, agent,
                COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category,
                quality_json, input_total, input_cache_read, input_cache_write,
                output_total, total_tokens, quality_bucket, conflict
         FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
           AND attribution_status = 'verified'
           AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.tz_version=?3 AND d.local_day=?4
             AND d.instance_id=source_instance_id AND d.sealed=1)
           AND record_kind IN ('model_call', 'transport_attempt', 'usage_observation')",
    )?;
    #[derive(Default)]
    struct Bucket {
        event_count: i64,
        call_count: i64,
        conflicts: i64,
        input: Option<i64>,
        cache_read: Option<i64>,
        cache_write: Option<i64>,
        output: Option<i64>,
        total: Option<i64>,
    }
    type HourKey = (u32, String, String, String, String, String, String);
    let mut buckets: std::collections::BTreeMap<HourKey, Bucket> = Default::default();
    let mut rows = stmt.query(params![start_ms, end_ms, tz, day_str])?;
    while let Some(row) = rows.next()? {
        let hour = calendar.local_hour_of(row.get(1)?)?;
        let kind: String = row.get(2)?;
        let key = (
            hour,
            row.get(0)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(13)?,
        );
        let bucket = buckets.entry(key).or_default();
        bucket.event_count += 1;
        bucket.call_count += i64::from(kind == "model_call");
        bucket.conflicts += row.get::<_, i64>(14)?;
        if kind == "transport_attempt" {
            continue;
        }
        let quality: serde_json::Value = serde_json::from_str(&row.get::<_, String>(7)?)?;
        for (sum, field, index) in [
            (&mut bucket.input, "input_total", 8),
            (&mut bucket.cache_read, "input_cache_read", 9),
            (&mut bucket.cache_write, "input_cache_write", 10),
            (&mut bucket.output, "output_total", 11),
            (&mut bucket.total, "total_tokens", 12),
        ] {
            if matches!(quality[field].as_str(), Some("reported" | "derived")) {
                if let Some(value) = row.get::<_, Option<i64>>(index)? {
                    *sum = Some(
                        sum.unwrap_or(0)
                            .checked_add(value)
                            .ok_or(CoreError::Overflow("hourly tokens"))?,
                    );
                }
            }
        }
    }
    drop(rows);
    drop(stmt);
    let mut insert = tx.prepare(
        "INSERT INTO hourly_usage (
           tz_version, local_day, hour, instance_id, agent, provider_id, model_raw,
           call_category, quality_bucket, event_count, call_count,
           input_known_sum, cache_read_known_sum, cache_write_known_sum,
           output_known_sum, total_known_sum, conflict_count, data_revision
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
    )?;
    for ((hour, instance, agent, provider, model, category, quality), bucket) in buckets {
        insert.execute(params![
            tz,
            day_str,
            hour,
            instance,
            agent,
            provider,
            model,
            category,
            quality,
            bucket.event_count,
            bucket.call_count,
            bucket.input,
            bucket.cache_read,
            bucket.cache_write,
            bucket.output,
            bucket.total,
            bucket.conflicts,
            data_revision
        ])?;
    }
    Ok(())
}
