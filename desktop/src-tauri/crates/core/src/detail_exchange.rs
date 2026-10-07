//! Complete statistical details, merged with the ordinary ingest arbitration.
use crate::aggregates::SourceAggregateInput;
use crate::domain::EventInput;
use crate::error::CoreError;
use crate::exchange::{ExchangeExport, ExchangeKind, ExportRequest};
use crate::ingest::{commit_batch_tx, IngestBatch};
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT: &str = "llm-usage-details-1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailRecord {
    pub event: EventInput,
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailExchange {
    pub format_version: String,
    pub archive: ExchangeExport,
    pub details: Vec<DetailRecord>,
    pub cumulative: Vec<SourceAggregateInput>,
}

#[derive(Debug, Default, Serialize)]
pub struct DetailImportOutcome {
    #[serde(flatten)]
    pub archive: crate::exchange_import::ImportAggregateOutcome,
    pub details_added: i64,
    pub details_updated: i64,
    pub details_unchanged: i64,
    pub details_skipped: i64,
    pub details_conflicts: i64,
    pub cumulative_changed: usize,
}

// All keys are the normalized statistics whitelist; no source payload is serialized.
const EVENT_JSON: &str = "json_object(
 'source_instance_id',source_instance_id,'source_record_key',source_record_key,
 'record_kind',record_kind,'schema_version',schema_version,'parser_version',parser_version,
 'parse_basis',parse_basis,'origin_call_id',origin_call_id,'attempt_id',attempt_id,
 'session_id',session_id,'parent_session_id',parent_session_id,'host_application',host_application,
 'agent',agent,'call_category',call_category,'occurred_at_ms',occurred_at_ms,
 'observed_at_ms',observed_at_ms,'source_time',source_time,'time_basis',time_basis,
 'interval_start_ms',interval_start_ms,'interval_end_ms',interval_end_ms,
 'provider_id',provider_id,'model_raw',model_raw,'model_canonical',model_canonical,
 'model_attribution',model_attribution,'usage',json_object(
 'input_uncached',input_uncached,'input_cache_read',input_cache_read,'input_cache_write',input_cache_write,
 'input_total',input_total,'output_total',output_total,'output_reasoning',output_reasoning,
 'total_tokens',total_tokens,'source_total',source_total),
 'quality',json(quality_json),'lifecycle',lifecycle,'source_revision',source_revision,
 'error_status',error_status,'duration_ms',duration_ms,'ttft_ms',ttft_ms,
 'attribution_status',attribution_status,'exclusion_reason',exclusion_reason,
 'cost',CASE WHEN cost_amount_minor IS NULL THEN NULL ELSE json_object(
 'amount_minor',cost_amount_minor,'currency',cost_currency,'kind',cost_kind,
 'price_version',price_version,'billing_scope',billing_scope) END)";

pub fn build_details(
    storage: &Storage,
    request: &ExportRequest,
    now_ms: i64,
) -> Result<DetailExchange, CoreError> {
    if !matches!(request.kind, ExchangeKind::FullSnapshot) {
        return Err(CoreError::Validation(
            "details export requires a full snapshot".into(),
        ));
    }
    // One read snapshot spans events, cumulative values and archive partitions.
    let snapshot = storage.conn().unchecked_transaction()?;
    let mut archive = crate::exchange::build_aggregate_export_tx(storage, request, now_ms)?;
    let wanted: BTreeSet<_> = archive
        .sources
        .iter()
        .map(|s| s.source_instance_id.as_str())
        .collect();
    let mut details = Vec::new();
    let mut stmt = snapshot.prepare(&format!("SELECT {EVENT_JSON}, conflict FROM usage_events WHERE occurred_at_ms>=?1 AND occurred_at_ms<?2 ORDER BY source_instance_id,source_record_key"))?;
    let rows = stmt.query_map(params![request.from_ms, request.to_ms], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
    })?;
    for row in rows {
        let (json, conflict) = row?;
        let event: EventInput = serde_json::from_str(&json)?;
        if wanted.contains(event.source_instance_id.as_str()) {
            details.push(DetailRecord { event, conflict });
        }
    }
    drop(stmt);
    let calendar = crate::calendar::Calendar::new(&request.timezone)?;
    let mut counts = BTreeMap::<(String, String), usize>::new();
    for record in &details {
        *counts
            .entry((
                record.event.source_instance_id.clone(),
                calendar
                    .local_day_of(record.event.occurred_at_ms)?
                    .to_string(),
            ))
            .or_default() += 1;
    }
    let live: BTreeSet<_> = counts.keys().cloned().collect();
    for (instance, day) in &live {
        let (start, end) = calendar.day_range_ms(crate::calendar::parse_date(day)?)?;
        let complete:i64=snapshot.query_row("SELECT COUNT(*) FROM usage_events WHERE source_instance_id=?1 AND occurred_at_ms>=?2 AND occurred_at_ms<?3",params![instance,start,end],|r|r.get(0))?;
        let selected = counts[&(instance.clone(), day.clone())];
        if complete != selected as i64 {
            return Err(CoreError::Validation(
                "details export requires complete source-days".into(),
            ));
        }
    }
    // Live source-days are rebuilt from details and are never counted twice.
    archive
        .daily_partitions
        .retain(|p| !live.contains(&(p.instance_id.clone(), p.local_day.clone())));
    archive
        .hourly_partitions
        .retain(|p| !live.contains(&(p.instance_id.clone(), p.local_day.clone())));
    let mut cumulative = Vec::new();
    let mut stmt = snapshot.prepare("SELECT json_object('instance_id',instance_id,'scope',scope,'scope_key',scope_key,'interval_start_ms',interval_start_ms,'interval_end_ms',interval_end_ms,'interval_end_inclusive',json(CASE WHEN interval_end_inclusive=1 THEN 'true' ELSE 'false' END),'usage',json_object('input_uncached',input_uncached,'input_cache_read',input_cache_read,'input_cache_write',input_cache_write,'input_total',input_total,'output_total',output_total,'output_reasoning',output_reasoning,'total_tokens',total_tokens,'source_total',source_total),'quality',json(quality_json),'reported_call_count',reported_call_count,'coverage',coverage,'duplicate_of',duplicate_of,'time_basis',time_basis,'source_revision',source_revision) FROM source_aggregates WHERE interval_end_ms>=?1 AND interval_end_ms<?2 ORDER BY instance_id,scope,scope_key")?;
    let rows = stmt.query_map(params![request.from_ms, request.to_ms], |r| {
        r.get::<_, String>(0)
    })?;
    for row in rows {
        let value: SourceAggregateInput = serde_json::from_str(&row?)?;
        if wanted.contains(value.instance_id.as_str()) {
            cumulative.push(value);
        }
    }
    drop(stmt);
    snapshot.commit()?;
    Ok(DetailExchange {
        format_version: FORMAT.into(),
        archive,
        details,
        cumulative,
    })
}

pub fn import_details(
    storage: &Storage,
    package: &DetailExchange,
    now_ms: i64,
) -> Result<DetailImportOutcome, CoreError> {
    import_details_with_pricing(storage, package, now_ms, None)
}

pub fn import_details_with_pricing(
    storage: &Storage,
    package: &DetailExchange,
    now_ms: i64,
    pricing: Option<&crate::pricing::EstimateOptions>,
) -> Result<DetailImportOutcome, CoreError> {
    if package.format_version != FORMAT
        || !matches!(package.archive.kind, ExchangeKind::FullSnapshot)
        || !package.archive.records.is_empty()
    {
        return Err(CoreError::Validation("unsupported details package".into()));
    }
    let calendar = crate::calendar::Calendar::new(&package.archive.timezone)?;
    let sources: BTreeMap<_, _> = package
        .archive
        .sources
        .iter()
        .map(|s| (s.source_instance_id.as_str(), s))
        .collect();
    let mut groups: BTreeMap<String, Vec<EventInput>> = BTreeMap::new();
    let mut keys = BTreeSet::new();
    let mut live = BTreeSet::new();
    for record in &package.details {
        let event = &record.event;
        event.validate()?;
        if event.source_record_key.is_empty()
            || !sources
                .get(event.source_instance_id.as_str())
                .is_some_and(|s| s.agent == event.agent)
            || !keys.insert((
                event.source_instance_id.clone(),
                event.source_record_key.clone(),
            ))
        {
            return Err(CoreError::Validation(
                "invalid or duplicate detail identity".into(),
            ));
        }
        live.insert((
            event.source_instance_id.clone(),
            calendar.local_day_of(event.occurred_at_ms)?.to_string(),
        ));
        groups
            .entry(event.source_instance_id.clone())
            .or_default()
            .push(event.clone());
    }
    if package
        .archive
        .daily_partitions
        .iter()
        .any(|p| live.contains(&(p.instance_id.clone(), p.local_day.clone())))
        || package
            .archive
            .hourly_partitions
            .iter()
            .any(|p| live.contains(&(p.instance_id.clone(), p.local_day.clone())))
    {
        return Err(CoreError::Validation(
            "details overlap archive source-day".into(),
        ));
    }
    for value in &package.cumulative {
        value.validate()?;
        if !sources.contains_key(value.instance_id.as_str()) {
            return Err(CoreError::Validation("undeclared cumulative source".into()));
        }
    }
    let tx = storage.conn().unchecked_transaction()?;
    // A sealed source-day cannot be reconstructed from a partial retained export.
    for (instance, day) in &live {
        let (start, end) = calendar.day_range_ms(crate::calendar::parse_date(day)?)?;
        let sealed:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM daily_usage WHERE instance_id=?1 AND local_day=?2 AND tz_version=?3 AND (sealed=1 OR NOT EXISTS(SELECT 1 FROM usage_events WHERE source_instance_id=?1 AND occurred_at_ms>=?4 AND occurred_at_ms<?5)))",params![instance,day,package.archive.timezone,start,end],|r|r.get(0))?;
        if sealed {
            return Err(CoreError::Validation(
                "details cannot replace a sealed source-day".into(),
            ));
        }
    }
    let archive =
        crate::exchange_import::import_aggregate_tx(storage, &tx, &package.archive, now_ms)?;
    let mut out = DetailImportOutcome {
        archive,
        ..Default::default()
    };
    for (instance, events) in groups {
        let result = commit_batch_tx(
            storage,
            &tx,
            &IngestBatch {
                batch_id: package.archive.batch_id.clone(),
                instance_id: instance,
                timezone: package.archive.timezone.clone(),
                now_ms,
                events,
                checkpoints: vec![],
                diagnostics: vec![],
                run_id: None,
                retention_cutoff_ms: None,
            },
            None,
            false,
        )?;
        if result.errors > 0 {
            return Err(CoreError::Validation("details ingest failed".into()));
        }
        out.details_added += result.added;
        out.details_updated += result.updated;
        out.details_unchanged += result.unchanged;
        out.details_skipped += result.skipped;
        out.details_conflicts += result.conflicts;
    }
    let mut conflict_changed = false;
    for record in package.details.iter().filter(|r| r.conflict) {
        let event = &record.event;
        let changed=tx.execute("UPDATE usage_events SET conflict=1 WHERE source_instance_id=?1 AND source_record_key=?2 AND conflict=0",params![event.source_instance_id,event.source_record_key])?;
        conflict_changed |= changed > 0;
    }
    if conflict_changed {
        let revision = Storage::bump_data_revision_tx(&tx, now_ms)?;
        for (_, day) in &live {
            let day = crate::calendar::parse_date(day)?;
            crate::ingest::recompute_day(&tx, &calendar, day, revision)?;
            crate::ingest::persist_hourly_day(&tx, &calendar, day, revision)?;
        }
    }
    for value in &package.cumulative {
        out.cumulative_changed += usize::from(crate::aggregates::upsert_source_aggregate_tx(
            &tx, value, now_ms,
        )?);
    }
    if out.cumulative_changed > 0 {
        Storage::bump_data_revision_tx(&tx, now_ms)?;
    }
    if out.details_added + out.details_updated + out.details_conflicts > 0 || conflict_changed {
        if let Some(options) = pricing {
            let revision = storage.data_revision()?;
            let days: BTreeSet<_> = live.iter().map(|(_, day)| day).collect();
            for day in days {
                crate::storage::pricing::recompute_cost_day_tx(
                    &tx,
                    &calendar,
                    crate::calendar::parse_date(day)?,
                    now_ms,
                    options,
                    revision,
                )?;
            }
        }
    }
    tx.commit()?;
    Ok(out)
}
