//! F2 price snapshot storage, daily cost rebuilding, and cost summary queries.
//!
//! Follow the daily_usage archive rules:
//! - Rebuild affected, unsealed daily_cost_usage rows after event batches commit.
//! - Mark daily costs sealed=1 when detail retention expires; keep historical amounts.
//! - Snapshot imports do not recalculate estimates. Event revisions or an explicit
//!   recompute_unsealed_cost_days request rebuild eligible rows and their references.

use crate::calendar::{parse_date, Calendar};
use crate::error::CoreError;
use crate::pricing::{
    EstimateOptions, EventEstimate, PriceBook, PriceRow, PriceSnapshot, SEED_SNAPSHOT_JSON,
};
use crate::storage::Storage;
use jiff::civil::Date;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Cost kind for this application's estimate using prices at occurrence time.
pub const KIND_ESTIMATE_AT_TIME: &str = "estimate_at_time";
/// Cost kind for amounts reported by the source (EventInput.cost, Reported).
pub const KIND_SOURCE_REPORTED: &str = "reported";
/// Cost kind for the source's own estimates (EventInput.cost, Estimated).
pub const KIND_SOURCE_ESTIMATE: &str = "source_estimate";

/// Snapshot import result; identical content under the same ID is skipped (A10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotImportOutcome {
    pub snapshot_id: String,
    pub inserted_rows: usize,
    pub already_present: bool,
}

/// Snapshot summary for the UI: source, age, and row count.
#[derive(Debug, Clone, Serialize)]
pub struct PriceSnapshotInfo {
    pub snapshot_id: String,
    pub source_type: String,
    pub source_urls: Vec<String>,
    pub fetched_at_ms: i64,
    pub verified_at_ms: Option<i64>,
    pub license: Option<String>,
    pub verified_by: Option<String>,
    pub note: Option<String>,
    pub row_count: i64,
}

/// Result of rebuilding daily costs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CostDayOutcome {
    pub day: String,
    pub rebuilt: bool,
    pub estimated_rows: usize,
    pub source_cost_rows: usize,
}

type CurrencyCosts = BTreeMap<String, CostCurrencyRow>;
type ModelCosts = BTreeMap<(String, String), (CurrencyCosts, CurrencyCosts)>;
type DayCosts = BTreeMap<(String, String, String), CurrencyCosts>;
type ModelRates = BTreeMap<(String, String), BTreeMap<String, PriceRow>>;

#[derive(Default)]
struct CurrentReference {
    detail_limited: bool,
    rows: Vec<CostCurrencyRow>,
    unpriced: BTreeMap<String, i64>,
    daily: DayCosts,
    rates: ModelRates,
    model_reasons: BTreeMap<(String, String), BTreeMap<String, i64>>,
    reference_models: BTreeMap<(String, String), BTreeSet<String>>,
}

impl Storage {
    /// Import a snapshot; identical IDs/content are skipped, differing content requires a new ID.
    pub fn import_price_snapshot(
        &self,
        snapshot: &PriceSnapshot,
        now_ms: i64,
    ) -> Result<SnapshotImportOutcome, CoreError> {
        let tx = self.conn().unchecked_transaction()?;
        let outcome = import_price_snapshot_tx(&tx, snapshot, now_ms)?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Import repository seed snapshots at startup without recalculating existing estimates.
    pub fn ensure_seed_price_snapshot(
        &self,
        now_ms: i64,
    ) -> Result<SnapshotImportOutcome, CoreError> {
        let snapshot = crate::pricing::parse_snapshot_json(SEED_SNAPSHOT_JSON)?;
        let supplement =
            crate::pricing::parse_snapshot_json(crate::pricing::SUPPLEMENT_SNAPSHOT_JSON)?;
        let tx = self.conn().unchecked_transaction()?;
        let result = import_price_snapshot_tx(&tx, &snapshot, now_ms)?;
        import_price_snapshot_tx(&tx, &supplement, now_ms)?;
        let hy4 =
            crate::pricing::parse_snapshot_json(include_str!("../../prices/seed-2026-10-03.json"))?;
        import_price_snapshot_tx(&tx, &hy4, now_ms)?;
        let current =
            crate::pricing::parse_snapshot_json(include_str!("../../prices/seed-2026-10-05.json"))?;
        import_price_snapshot_tx(&tx, &current, now_ms)?;
        tx.commit()?;
        // Preserve the original seed snapshot's public return values.
        Ok(result)
    }

    /// Load all price rows into the estimation price book.
    pub fn load_price_book(&self) -> Result<PriceBook, CoreError> {
        load_price_book_conn(self.conn())
    }

    /// List price snapshots with row counts.
    pub fn list_price_snapshots(&self) -> Result<Vec<PriceSnapshotInfo>, CoreError> {
        let mut stmt = self.conn().prepare(
            "SELECT s.snapshot_id, s.source_type, s.source_urls, s.fetched_at_ms,
                    s.verified_at_ms, s.license, s.verified_by, s.note,
                    (SELECT COUNT(*) FROM price_versions v WHERE v.snapshot_id = s.snapshot_id)
             FROM price_snapshots s ORDER BY s.fetched_at_ms DESC, s.snapshot_id",
        )?;
        let rows = stmt.query_map([], |r| {
            let urls: String = r.get(2)?;
            Ok(PriceSnapshotInfo {
                snapshot_id: r.get(0)?,
                source_type: r.get(1)?,
                source_urls: serde_json::from_str(&urls).unwrap_or_default(),
                fetched_at_ms: r.get(3)?,
                verified_at_ms: r.get(4)?,
                license: r.get(5)?,
                verified_by: r.get(6)?,
                note: r.get(7)?,
                row_count: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Select unsealed days in this timezone with revisions greater than revision_before.
    /// These days were rewritten by this collection batch; sealed days remain unchanged.
    /// Capture revision_before at refresh start: retention can increment the revision after
    /// scanning, so comparing against only the final current revision would omit changed days.
    pub fn cost_backfill_days_since(
        &self,
        timezone: &str,
        revision_before: i64,
    ) -> Result<Vec<String>, CoreError> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT local_day FROM daily_usage
             WHERE tz_version = ?1 AND sealed = 0 AND data_revision > ?2
             ORDER BY local_day",
        )?;
        let rows = stmt.query_map(rusqlite::params![timezone, revision_before], |r| r.get(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Correct retained, unsealed estimates once after a price matching rule changes.
    /// Catalog refreshes do not change this marker or historical amounts.
    pub fn ensure_cost_matching_policy(
        &self,
        timezone: &str,
        now_ms: i64,
        options: &EstimateOptions,
    ) -> Result<(), CoreError> {
        let key = format!("cost_matching_policy:{timezone}");
        let policy: Option<String> = self
            .conn()
            .query_row("SELECT value FROM settings WHERE key=?1", [&key], |r| {
                r.get(0)
            })
            .optional()?;
        if policy.as_deref() == Some("official-reference-5") {
            return Ok(());
        }
        self.recompute_unsealed_cost_days(timezone, now_ms, options)?;
        self.conn().execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,'official-reference-5',1,?2)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",params![key,now_ms])?;
        Ok(())
    }

    /// Rebuild an unsealed day after an event batch or an explicit recalculation.
    /// Keep existing costs when no detail events remain; they may represent expired details.
    pub fn recompute_cost_day(
        &self,
        timezone: &str,
        day: &str,
        now_ms: i64,
        options: &EstimateOptions,
    ) -> Result<CostDayOutcome, CoreError> {
        let calendar = Calendar::new(timezone)?;
        let day = parse_date(day)?;
        let tx = self.conn().unchecked_transaction()?;
        let revision = crate::storage::data_revision(&tx)?;
        let outcome = recompute_cost_day_tx(&tx, &calendar, day, now_ms, options, revision)?;
        crate::adapters::run_policy::check()?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Explicitly rebuild all unsealed days in this timezone that still contain detail events.
    pub fn recompute_unsealed_cost_days(
        &self,
        timezone: &str,
        now_ms: i64,
        options: &EstimateOptions,
    ) -> Result<Vec<CostDayOutcome>, CoreError> {
        let calendar = Calendar::new(timezone)?;
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT local_day FROM daily_usage
             WHERE tz_version = ?1 AND sealed = 0 ORDER BY local_day",
        )?;
        let days: Vec<String> = stmt
            .query_map([timezone], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        let mut out = Vec::with_capacity(days.len());
        let tx = conn.unchecked_transaction()?;
        let revision = crate::storage::data_revision(&tx)?;
        for day_str in days {
            crate::adapters::run_policy::check()?;
            let day = parse_date(&day_str)?;
            out.push(recompute_cost_day_tx(
                &tx, &calendar, day, now_ms, options, revision,
            )?);
        }
        crate::adapters::run_policy::check()?;
        tx.commit()?;
        Ok(out)
    }

    /// Historical estimates and source amounts come from daily costs. Current reference prices
    /// apply to the same usage view, using mutually exclusive retained details or archives.
    pub fn cost_summary(&self, request: &CostSummaryRequest) -> Result<CostSummary, CoreError> {
        self.cost_summary_selected(request, None)
    }

    /// Current references can select local hours; historical daily costs remain unchanged.
    pub fn cost_summary_selected(
        &self,
        request: &CostSummaryRequest,
        hours: Option<(&str, &str)>,
    ) -> Result<CostSummary, CoreError> {
        self.cost_summary_for_view(
            request,
            hours,
            if hours.is_some() {
                crate::query::Granularity::Hour
            } else {
                crate::query::Granularity::Day
            },
            crate::calendar::WeekStart::Monday,
        )
    }

    pub fn cost_summary_for_view(
        &self,
        request: &CostSummaryRequest,
        hours: Option<(&str, &str)>,
        granularity: crate::query::Granularity,
        week_start: crate::calendar::WeekStart,
    ) -> Result<CostSummary, CoreError> {
        if let Some((first, last)) = hours {
            let valid = |label: &str| {
                label.split_once(' ').is_some_and(|(day, hour)| {
                    parse_date(day).is_ok()
                        && hour.len() == 5
                        && hour
                            .strip_suffix(":00")
                            .is_some_and(|h| h.parse::<u32>().is_ok_and(|v| v < 24))
                })
            };
            if !valid(first) || !valid(last) || first > last {
                return Err(CoreError::Validation("invalid cost hour selection".into()));
            }
        }
        let calendar = Calendar::new(&request.timezone)?;
        let first_day = parse_date(&request.first_day)?;
        let last_day = parse_date(&request.last_day)?;
        if last_day < first_day {
            return Err(CoreError::Validation(
                "cost summary range: last_day before first_day".into(),
            ));
        }
        let (range_start, range_end) = (
            calendar.day_range_ms(first_day)?.0,
            calendar.day_range_ms(last_day)?.1,
        );

        let snapshot = self.conn().unchecked_transaction()?;
        // 1) Daily historical estimates and source amounts.
        let mut filter_sql = String::new();
        let filter_params: Vec<rusqlite::types::Value> = vec![
            request.timezone.clone().into(),
            first_day.to_string().into(),
            last_day.to_string().into(),
        ];
        if !request.filters.agents.is_empty() {
            filter_sql.push_str(&format!(
                " AND {}",
                fold_filter_condition("agent", &request.filters.agents)
            ));
        }
        if !request.filters.providers.is_empty() {
            filter_sql.push_str(&format!(
                " AND {}",
                fold_filter_condition("provider_id", &request.filters.providers)
            ));
        }
        if !request.filters.models.is_empty() {
            filter_sql.push_str(&format!(
                " AND {}",
                model_filter_condition(&request.filters.models)
            ));
        }
        if let Some(instances) = &request.filters.instances {
            let list = instances
                .iter()
                .map(|i| format!("'{}'", i.replace('\'', "")))
                .collect::<Vec<_>>()
                .join(",");
            if list.is_empty() {
                return Ok(CostSummary::empty());
            }
            filter_sql.push_str(&format!(" AND instance_id IN ({list})"));
        }
        let sql = format!(
            "SELECT currency, kind,
                    SUM(input_amount_minor), SUM(cache_read_amount_minor),
                    SUM(cache_write_amount_minor), SUM(output_amount_minor),
                    SUM(total_amount_minor), SUM(priced_tokens), SUM(known_tokens),
                    SUM(priced_event_count), SUM(unpriced_event_count),
                    SUM(partial_event_count), SUM(ttl_defaulted_events),
                    SUM(fallback_event_count), provider_id, model_raw, local_day
             FROM daily_cost_usage
             WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3{filter_sql}
             GROUP BY currency, kind, provider_id, model_raw, local_day"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(filter_params.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, i64>(12)?,
                r.get::<_, i64>(13)?,
                r.get::<_, String>(14)?,
                r.get::<_, String>(15)?,
                r.get::<_, String>(16)?,
            ))
        })?;
        let mut at_time: BTreeMap<String, CostCurrencyRow> = BTreeMap::new();
        let mut source_amounts: BTreeMap<String, CostCurrencyRow> = BTreeMap::new();
        let mut unpriced_reasons: BTreeMap<String, i64> = BTreeMap::new();
        let mut models = ModelCosts::new();
        let mut daily: BTreeMap<(String, String, String), CurrencyCosts> = BTreeMap::new();
        for row in rows {
            let (
                currency,
                kind,
                input_amount,
                cache_read_amount,
                cache_write_amount,
                output_amount,
                total_amount,
                priced_tokens,
                known_tokens,
                priced_events,
                unpriced_events,
                partial_events,
                ttl_defaulted,
                fallback_events,
                provider,
                model,
                day,
            ) = row?;
            let entry = CostCurrencyRow {
                currency: currency.clone(),
                total_amount_minor: total_amount,
                input_amount_minor: input_amount,
                cache_read_amount_minor: cache_read_amount,
                cache_write_amount_minor: cache_write_amount,
                output_amount_minor: output_amount,
                priced_tokens,
                known_tokens,
                priced_event_count: priced_events,
                unpriced_event_count: unpriced_events,
                partial_event_count: partial_events,
                ttl_defaulted_events: ttl_defaulted,
                fallback_event_count: fallback_events,
                ..Default::default()
            };
            match kind.as_str() {
                KIND_ESTIMATE_AT_TIME => {
                    let model_key = (
                        provider.trim().to_lowercase(),
                        crate::model_names::model_key(&model),
                    );
                    merge_currency_row(
                        &mut models.entry(model_key.clone()).or_default().0,
                        entry.clone(),
                    )?;
                    merge_currency_row(
                        daily.entry((day, model_key.0, model_key.1)).or_default(),
                        entry.clone(),
                    )?;
                    if currency.is_empty() {
                        // Empty-currency rows carry only unpriced event counts.
                        if let Some(row) = at_time.get_mut("") {
                            row.unpriced_event_count += unpriced_events;
                        } else {
                            at_time.insert(
                                String::new(),
                                CostCurrencyRow {
                                    currency: String::new(),
                                    total_amount_minor: 0,
                                    input_amount_minor: None,
                                    cache_read_amount_minor: None,
                                    cache_write_amount_minor: None,
                                    output_amount_minor: None,
                                    priced_tokens: 0,
                                    known_tokens: 0,
                                    priced_event_count: 0,
                                    unpriced_event_count: unpriced_events,
                                    partial_event_count: 0,
                                    ttl_defaulted_events: 0,
                                    fallback_event_count: 0,
                                    ..Default::default()
                                },
                            );
                        }
                    } else {
                        merge_currency_row(&mut at_time, entry)?;
                    }
                }
                KIND_SOURCE_REPORTED | KIND_SOURCE_ESTIMATE => {
                    merge_currency_row(&mut source_amounts, entry)?;
                }
                _ => {}
            }
        }
        drop(stmt);

        // Aggregate unpriced reasons from empty-currency rows.
        {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT unpriced_reasons FROM daily_cost_usage
                 WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
                   AND kind = '{KIND_ESTIMATE_AT_TIME}' AND currency = ''{filter_sql}
                   AND unpriced_reasons IS NOT NULL"
            ))?;
            let reasons: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(filter_params.iter()), |r| {
                    r.get(0)
                })?
                .filter_map(Result::ok)
                .collect();
            for json in reasons {
                if let Ok(map) = serde_json::from_str::<BTreeMap<String, i64>>(&json) {
                    for (reason, count) in map {
                        *unpriced_reasons.entry(reason).or_insert(0) += count;
                    }
                }
            }
        }
        // Use the maximum cost-row revision in the selected range.
        let data_revision: i64 = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT MAX(data_revision) FROM daily_cost_usage
                 WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3{filter_sql}"
            ))?;
            stmt.query_row(rusqlite::params_from_iter(filter_params.iter()), |r| {
                r.get(0)
            })
            .unwrap_or(0)
        };

        // 2) Apply current reference prices to the same detail/archive view.
        let mut reference = self.cost_summary_current_sim(
            range_start,
            range_end,
            request,
            &mut models,
            hours,
            (granularity, week_start),
        )?;
        let mut current_sim = CostModeSummary {
            rows: reference.rows,
            unpriced_reasons: reference.unpriced,
            ..Default::default()
        };
        current_sim.as_of_ms = request.now_ms;
        current_sim.detail_limited = reference.detail_limited;

        let mut price_basis = BTreeSet::new();
        {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT price_basis FROM daily_cost_usage
                 WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
                   AND kind = '{KIND_ESTIMATE_AT_TIME}' AND currency != ''{filter_sql}"
            ))?;
            let bases: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(filter_params.iter()), |r| {
                    r.get(0)
                })?
                .filter_map(Result::ok)
                .collect();
            for json in bases {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&json) {
                    price_basis.extend(list);
                }
            }
        }

        let result = CostSummary {
            at_time: CostModeSummary {
                rows: at_time.into_values().collect(),
                unpriced_reasons,
                ..Default::default()
            },
            source_amounts: source_amounts.into_values().collect(),
            current_sim,
            price_basis: price_basis.into_iter().collect(),
            data_revision,
            models: models
                .into_iter()
                .map(|((provider, model), (at_time, current_sim))| {
                    let unit_prices = reference
                        .rates
                        .get(&(provider.clone(), model.clone()))
                        .map(|rates| rates.values().cloned().collect())
                        .unwrap_or_default();
                    CostModelRow {
                        unpriced_reasons: reference
                            .model_reasons
                            .remove(&(provider.clone(), model.clone()))
                            .unwrap_or_default(),
                        reference_models: reference
                            .reference_models
                            .remove(&(provider.clone(), model.clone()))
                            .unwrap_or_default()
                            .into_iter()
                            .collect(),
                        provider,
                        model,
                        at_time: at_time.into_values().collect(),
                        current_sim: current_sim.into_values().collect(),
                        unit_prices,
                    }
                })
                .collect(),
            daily: daily
                .into_iter()
                .flat_map(|((day, provider, model), currencies)| {
                    currencies.into_values().map(move |sums| CostDayRow {
                        day: day.clone(),
                        provider: provider.clone(),
                        model: model.clone(),
                        sums,
                    })
                })
                .collect(),
            daily_current: cost_day_rows(reference.daily),
        };
        snapshot.commit()?;
        Ok(result)
    }

    /// Apply current prices to either details or the sealed summaries selected by usage queries.
    fn cost_summary_current_sim(
        &self,
        range_start: i64,
        range_end: i64,
        request: &CostSummaryRequest,
        models: &mut ModelCosts,
        hours: Option<(&str, &str)>,
        view: (crate::query::Granularity, crate::calendar::WeekStart),
    ) -> Result<CurrentReference, CoreError> {
        let book = self.load_price_book()?;
        let calendar = Calendar::new(&request.timezone)?;
        let rows =
            collect_events_for_pricing(self.conn(), range_start, range_end, &request.filters)?;
        let archives = crate::query::archived_pricing_rows(
            self,
            &crate::query::SummaryRequest {
                timezone: request.timezone.clone(),
                first_day: parse_date(&request.first_day)?,
                last_day: parse_date(&request.last_day)?,
                today: calendar.local_day_of(request.now_ms)?,
                granularity: view.0,
                week_start: view.1,
                retention_cutoff: None,
                filters: crate::query::Filters {
                    agents: request.filters.agents.clone(),
                    providers: request.filters.providers.clone(),
                    models: request.filters.models.clone(),
                    instances: request.filters.instances.clone(),
                    ..Default::default()
                },
            },
        )?;
        // Retained details can coexist with an imported or sealed aggregate.
        // Match the complete source partition and count only one representation.
        let mut sealed_partitions: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for a in &archives {
            sealed_partitions
                .entry((
                    a.instance.clone(),
                    a.agent.clone(),
                    a.event.provider_id.clone().unwrap_or_default(),
                    a.event.model_raw.clone().unwrap_or_default(),
                    a.category.clone(),
                    a.quality.clone(),
                ))
                .or_default()
                .push((a.event.occurred_at_ms, a.end_ms, a.hour));
        }
        let mut inputs = Vec::new();
        for r in rows {
            let hour = calendar.local_hour_of(r.event.occurred_at_ms)?;
            let sealed = sealed_partitions
                .get(&(
                    r.instance.clone(),
                    r.agent.clone(),
                    r.provider.clone(),
                    r.model.clone(),
                    r.category.clone(),
                    r.quality.clone(),
                ))
                .is_some_and(|ranges| {
                    ranges.iter().any(|(start, end, h)| {
                        *start <= r.event.occurred_at_ms
                            && r.event.occurred_at_ms < *end
                            && h.map_or(true, |h| h == i64::from(hour))
                    })
                });
            if !sealed {
                inputs.push((
                    calendar.local_day_of(r.event.occurred_at_ms)?.to_string(),
                    Some(i64::from(hour)),
                    r.event,
                    1,
                    false,
                    false,
                    false,
                    0,
                    true,
                ));
            }
        }
        for a in archives {
            inputs.push((
                a.day,
                a.hour,
                a.event,
                a.count,
                true,
                a.partial,
                a.from_period,
                a.known_tokens_floor,
                a.alias_stable,
            ));
        }
        let mut by_currency: BTreeMap<String, CostCurrencyRow> = BTreeMap::new();
        let mut unpriced: BTreeMap<String, i64> = BTreeMap::new();
        let mut unpriced_events_total = 0i64;
        let mut books: BTreeMap<(String, String), PriceBook> = BTreeMap::new();
        let mut daily = DayCosts::new();
        let mut coarse = DayCosts::new();
        let mut rates = ModelRates::new();
        let mut model_reasons: BTreeMap<(String, String), BTreeMap<String, i64>> = BTreeMap::new();
        let mut reference_models: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        let mut detail_limited = false;
        for (day, hour, event, count, aggregate, partial, from_period, known_floor, alias_stable) in
            inputs
        {
            if let Some((first, last)) = hours {
                let label = format!("{} {:02}:00", day, hour.unwrap_or(-1));
                if label.as_str() < first || label.as_str() > last {
                    continue;
                }
            }
            detail_limited |= aggregate;
            let key = crate::model_names::model_key(
                event
                    .model_canonical
                    .as_deref()
                    .filter(|m| !m.trim().is_empty())
                    .or(event.model_raw.as_deref())
                    .unwrap_or_default(),
            );
            let book = books
                .entry((
                    key.clone(),
                    crate::model_names::reference_model_key_for(
                        &key,
                        event.provider_id.as_deref(),
                        event.occurred_at_ms,
                    ),
                ))
                .or_insert_with(|| book.for_model(&event));
            let model_key = (
                event
                    .provider_id
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase(),
                crate::model_names::model_key(event.model_raw.as_deref().unwrap_or_default()),
            );
            let model = &mut models.entry(model_key.clone()).or_default().1;
            reference_models
                .entry(model_key.clone())
                .or_default()
                .insert(crate::model_names::reference_model_key_for(
                    event.model_raw.as_deref().unwrap_or_default(),
                    event.provider_id.as_deref(),
                    event.occurred_at_ms,
                ));
            let estimate = if !alias_stable {
                EventEstimate::Unpriced(crate::pricing::UnpricedReason::ModelAmbiguous)
            } else if aggregate {
                book.estimate_aggregate(&event, &request.options, request.now_ms)
            } else {
                book.estimate(&event, &request.options, request.now_ms)
            };
            match estimate {
                EventEstimate::Priced(mut amounts) => {
                    amounts.has_unknown_components |= partial;
                    amounts.known_tokens = amounts.known_tokens.max(known_floor);
                    {
                        let day_costs = (if from_period { &mut coarse } else { &mut daily })
                            .entry((day, model_key.0.clone(), model_key.1.clone()))
                            .or_default();
                        accumulate_amounts(
                            day_costs
                                .entry(amounts.currency.clone())
                                .or_insert_with(|| {
                                    CostCurrencyRow::empty(amounts.currency.clone())
                                }),
                            &amounts,
                            count,
                            aggregate,
                        )?;
                    }
                    let model_rates = rates.entry(model_key).or_default();
                    for id in &amounts.matched_price_ids {
                        if !model_rates.contains_key(id) {
                            if let Some(price) =
                                book.rows.iter().find(|price| &price.price_id == id)
                            {
                                model_rates.insert(id.clone(), price.clone());
                            }
                        }
                    }
                }
                EventEstimate::Unpriced(reason) => {
                    *model_reasons
                        .entry(model_key.clone())
                        .or_default()
                        .entry(reason.as_str().into())
                        .or_default() += count;
                    if !from_period {
                        daily
                            .entry((day, model_key.0, model_key.1))
                            .or_default()
                            .entry(String::new())
                            .or_default()
                            .unpriced_event_count += count;
                    }
                    unpriced_events_total += count;
                    *unpriced.entry(reason.as_str().to_string()).or_insert(0) += count;
                    model.entry(String::new()).or_default().unpriced_event_count += count;
                }
            }
        }
        // Sum exact component numerators within each displayed day/model/currency or indivisible
        // archive period before rounding; roll up those amounts to keep rows and totals consistent.
        for ((_, provider, model), currencies) in daily.iter().chain(coarse.iter()) {
            for entry in currencies.values().filter(|e| !e.currency.is_empty()) {
                merge_currency_row(&mut by_currency, entry.clone())?;
                merge_currency_row(
                    &mut models
                        .entry((provider.clone(), model.clone()))
                        .or_default()
                        .1,
                    entry.clone(),
                )?;
            }
        }
        if unpriced_events_total > 0 {
            by_currency.insert(
                String::new(),
                CostCurrencyRow {
                    currency: String::new(),
                    unpriced_event_count: unpriced_events_total,
                    ..CostCurrencyRow::empty(String::new())
                },
            );
        }
        Ok(CurrentReference {
            detail_limited,
            rows: by_currency.into_values().collect(),
            unpriced,
            daily,
            rates,
            model_reasons,
            reference_models,
        })
    }
}

fn cost_day_rows(days: DayCosts) -> Vec<CostDayRow> {
    days.into_iter()
        .flat_map(|((day, provider, model), currencies)| {
            currencies.into_values().map(move |sums| CostDayRow {
                day: day.clone(),
                provider: provider.clone(),
                model: model.clone(),
                sums,
            })
        })
        .collect()
}

fn merge_currency_row(
    map: &mut BTreeMap<String, CostCurrencyRow>,
    entry: CostCurrencyRow,
) -> Result<(), CoreError> {
    let row = map
        .entry(entry.currency.clone())
        .or_insert_with(|| CostCurrencyRow::empty(entry.currency.clone()));
    for model in entry.substitute_models {
        if !row.substitute_models.contains(&model) {
            row.substitute_models.push(model);
        }
    }
    row.substitute_models.sort();
    if row.upper_amount_minor.is_some() || entry.upper_amount_minor.is_some() {
        row.upper_amount_minor = Some(
            row.upper_amount_minor
                .unwrap_or(row.total_amount_minor)
                .checked_add(entry.upper_amount_minor.unwrap_or(entry.total_amount_minor))
                .ok_or(CoreError::Overflow("cost upper total"))?,
        );
    }
    row.total_amount_minor = row
        .total_amount_minor
        .checked_add(entry.total_amount_minor)
        .ok_or(CoreError::Overflow("cost total"))?;
    row.input_amount_minor = sum_opt(row.input_amount_minor, entry.input_amount_minor)?;
    row.cache_read_amount_minor =
        sum_opt(row.cache_read_amount_minor, entry.cache_read_amount_minor)?;
    row.cache_write_amount_minor =
        sum_opt(row.cache_write_amount_minor, entry.cache_write_amount_minor)?;
    row.output_amount_minor = sum_opt(row.output_amount_minor, entry.output_amount_minor)?;
    for (target, value) in [
        (&mut row.priced_tokens, entry.priced_tokens),
        (&mut row.known_tokens, entry.known_tokens),
        (&mut row.priced_event_count, entry.priced_event_count),
        (&mut row.unpriced_event_count, entry.unpriced_event_count),
        (&mut row.partial_event_count, entry.partial_event_count),
        (&mut row.ttl_defaulted_events, entry.ttl_defaulted_events),
        (&mut row.fallback_event_count, entry.fallback_event_count),
        (&mut row.aggregate_event_count, entry.aggregate_event_count),
    ] {
        *target = target
            .checked_add(value)
            .ok_or(CoreError::Overflow("cost coverage total"))?;
    }
    Ok(())
}

fn sum_opt(a: Option<i64>, b: Option<i64>) -> Result<Option<i64>, CoreError> {
    Ok(match (a, b) {
        (None, None) => None,
        (Some(v), None) | (None, Some(v)) => Some(v),
        (Some(x), Some(y)) => Some(
            x.checked_add(y)
                .ok_or(CoreError::Overflow("cost component sum"))?,
        ),
    })
}

/// Import within a transaction: skip identical IDs/content and reject changed content.
pub(crate) fn import_price_snapshot_tx(
    tx: &Transaction<'_>,
    snapshot: &PriceSnapshot,
    now_ms: i64,
) -> Result<SnapshotImportOutcome, CoreError> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT content_hash FROM price_snapshots WHERE snapshot_id = ?1",
            [&snapshot.snapshot_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(hash) = existing {
        if hash == snapshot.content_hash_hex() && snapshot_provenance_matches(tx, snapshot)? {
            return Ok(SnapshotImportOutcome {
                snapshot_id: snapshot.snapshot_id.clone(),
                inserted_rows: 0,
                already_present: true,
            });
        }
        return Err(CoreError::Validation(format!(
            "price snapshot {} exists with different content; corrections require a new snapshot id",
            snapshot.snapshot_id
        )));
    }
    tx.execute(
        "INSERT INTO price_snapshots (
           snapshot_id, source_type, source_urls, fetched_at_ms, verified_at_ms,
           content_hash, license, verified_by, note, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            snapshot.snapshot_id,
            snapshot.source_type,
            serde_json::to_string(&snapshot.source_urls)
                .map_err(|e| CoreError::Validation(format!("source urls: {e}")))?,
            snapshot.fetched_at_ms,
            snapshot.verified_at_ms,
            snapshot.content_hash_hex(),
            snapshot.license,
            snapshot.verified_by,
            snapshot.note,
            now_ms,
        ],
    )?;
    let mut insert = tx.prepare(
        "INSERT INTO price_versions (
           price_id, snapshot_id, provider_id, model, region, channel, service_tier,
           context_threshold_tokens, effective_from_ms, effective_to_ms,
           input_per_mtok_hundredths, cache_read_per_mtok_hundredths,
           cache_write_5m_per_mtok_hundredths, cache_write_1h_per_mtok_hundredths,
           output_per_mtok_hundredths, cache_storage_per_mtok_hour_hundredths,
           currency, official_vendor, note, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                   ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
    )?;
    for row in &snapshot.rows {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM price_versions WHERE price_id = ?1)",
            [&row.price_id],
            |r| r.get(0),
        )?;
        if exists {
            return Err(CoreError::Validation(format!(
                "price row id {} already exists in another snapshot",
                row.price_id
            )));
        }
        insert.execute(rusqlite::params![
            row.price_id,
            row.snapshot_id,
            row.provider_id,
            row.model,
            row.region,
            row.channel,
            row.service_tier,
            row.context_threshold_tokens,
            row.effective_from_ms,
            row.effective_to_ms,
            row.input_per_mtok_hundredths,
            row.cache_read_per_mtok_hundredths,
            row.cache_write_5m_per_mtok_hundredths,
            row.cache_write_1h_per_mtok_hundredths,
            row.output_per_mtok_hundredths,
            row.cache_storage_per_mtok_hour_hundredths,
            row.currency,
            row.official_vendor,
            row.note,
            now_ms,
        ])?;
    }
    Ok(SnapshotImportOutcome {
        snapshot_id: snapshot.snapshot_id.clone(),
        inserted_rows: snapshot.rows.len(),
        already_present: false,
    })
}

/// Older stored hashes cover price fields only. Reimports must also compare source metadata
/// and row notes; retain the old hash algorithm while detecting changes under the same ID.
fn snapshot_provenance_matches(
    tx: &Transaction<'_>,
    snapshot: &PriceSnapshot,
) -> Result<bool, CoreError> {
    struct StoredProvenance {
        source_type: String,
        urls_json: String,
        fetched_at_ms: i64,
        verified_at_ms: Option<i64>,
        license: Option<String>,
        verified_by: Option<String>,
        note: Option<String>,
    }
    let stored = tx.query_row(
        "SELECT source_type, source_urls, fetched_at_ms, verified_at_ms,
                license, verified_by, note
         FROM price_snapshots WHERE snapshot_id = ?1",
        [&snapshot.snapshot_id],
        |r| {
            Ok(StoredProvenance {
                source_type: r.get(0)?,
                urls_json: r.get(1)?,
                fetched_at_ms: r.get(2)?,
                verified_at_ms: r.get(3)?,
                license: r.get(4)?,
                verified_by: r.get(5)?,
                note: r.get(6)?,
            })
        },
    )?;
    let urls: Vec<String> = serde_json::from_str(&stored.urls_json)
        .map_err(|e| CoreError::Validation(format!("stored price source urls: {e}")))?;
    if stored.source_type != snapshot.source_type
        || urls != snapshot.source_urls
        || stored.fetched_at_ms != snapshot.fetched_at_ms
        || stored.verified_at_ms != snapshot.verified_at_ms
        || stored.license != snapshot.license
        || stored.verified_by != snapshot.verified_by
        || stored.note != snapshot.note
    {
        return Ok(false);
    }
    let mut stmt =
        tx.prepare("SELECT price_id, note FROM price_versions WHERE snapshot_id = ?1")?;
    let notes: BTreeMap<String, Option<String>> = stmt
        .query_map([&snapshot.snapshot_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(notes.len() == snapshot.rows.len()
        && snapshot
            .rows
            .iter()
            .all(|row| notes.get(&row.price_id) == Some(&row.note)))
}

/// Build case-insensitive SQL filters. Selecting unknown also includes empty and NULL
/// values, matching query::Filters; other selections exclude those values.
/// Callers supply fixed column names; strip single quotes from selected values.
fn model_filter_condition(values: &[String]) -> String {
    fold_filter_condition(
        "model_key(model_raw)",
        &values
            .iter()
            .map(|m| crate::model_names::model_key(m))
            .collect::<Vec<_>>(),
    )
}

fn fold_filter_condition(column: &str, values: &[String]) -> String {
    let folded: Vec<String> = values
        .iter()
        .map(|v| v.to_lowercase().replace('\'', ""))
        .collect();
    let list = folded
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(",");
    let unknown_match = if folded.iter().any(|v| v == "unknown") {
        format!(" OR {column} IS NULL OR {column} = ''")
    } else {
        String::new()
    };
    format!("(fold_name({column}) IN ({list}){unknown_match})")
}

fn load_price_book_conn(conn: &Connection) -> Result<PriceBook, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT v.price_id, v.snapshot_id, v.provider_id, v.model, v.region, v.channel,
                v.service_tier, v.context_threshold_tokens, v.effective_from_ms,
                v.effective_to_ms, v.input_per_mtok_hundredths,
                v.cache_read_per_mtok_hundredths, v.cache_write_5m_per_mtok_hundredths,
                v.cache_write_1h_per_mtok_hundredths, v.output_per_mtok_hundredths,
                v.cache_storage_per_mtok_hour_hundredths, v.currency, v.official_vendor, v.note
         FROM price_versions v
         JOIN price_snapshots s ON s.snapshot_id = v.snapshot_id
         ORDER BY CASE s.source_type
                    WHEN 'manual' THEN 2
                    WHEN 'seed' THEN 1
                    ELSE 0 END DESC,
                  s.fetched_at_ms DESC, s.created_at_ms DESC,
                  v.effective_from_ms DESC, v.price_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(PriceRow {
            price_id: r.get(0)?,
            snapshot_id: r.get(1)?,
            provider_id: r.get(2)?,
            model: r.get(3)?,
            region: r.get(4)?,
            channel: r.get(5)?,
            service_tier: r.get(6)?,
            context_threshold_tokens: r.get(7)?,
            effective_from_ms: r.get(8)?,
            effective_to_ms: r.get(9)?,
            input_per_mtok_hundredths: r.get(10)?,
            cache_read_per_mtok_hundredths: r.get(11)?,
            cache_write_5m_per_mtok_hundredths: r.get(12)?,
            cache_write_1h_per_mtok_hundredths: r.get(13)?,
            output_per_mtok_hundredths: r.get(14)?,
            cache_storage_per_mtok_hour_hundredths: r.get(15)?,
            currency: r.get(16)?,
            official_vendor: r.get(17)?,
            note: r.get(18)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(PriceBook { rows: out })
}

/// Filtered, time-bounded model_call or usage_observation events with verified ownership.
/// Map token fields with quality other than reported/derived to None.
pub(crate) struct EventWithDims {
    pub category: String,
    pub quality: String,
    pub event: crate::pricing::PricingEvent,
    pub instance: String,
    pub agent: String,
    pub provider: String,
    pub model: String,
    /// Source amounts from cost_* columns; keep Reported and Estimated rows separate.
    pub source_amount_minor: Option<i64>,
    pub source_currency: Option<String>,
    pub source_kind: Option<String>,
}

pub(crate) fn collect_events_for_pricing(
    conn: &Connection,
    range_start: i64,
    range_end: i64,
    filters: &CostFilters,
) -> Result<Vec<EventWithDims>, CoreError> {
    let mut sql = String::from(
        "SELECT provider_id, model_canonical, model_raw, occurred_at_ms,
                input_uncached, input_cache_read, input_cache_write,
                input_total, output_total, quality_json,
                source_instance_id, agent,
                COALESCE(provider_id, ''), COALESCE(model_raw, ''),
                cost_amount_minor, cost_currency, cost_kind, call_category, quality_bucket
         FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
           AND attribution_status = 'verified' AND record_kind IN ('model_call','usage_observation')",
    );
    if !filters.agents.is_empty() {
        sql.push_str(&format!(
            " AND {}",
            fold_filter_condition("agent", &filters.agents)
        ));
    }
    if !filters.providers.is_empty() {
        sql.push_str(&format!(
            " AND {}",
            fold_filter_condition("provider_id", &filters.providers)
        ));
    }
    if !filters.models.is_empty() {
        sql.push_str(&format!(" AND {}", model_filter_condition(&filters.models)));
    }
    if let Some(instances) = &filters.instances {
        if instances.is_empty() {
            return Ok(Vec::new());
        }
        let list: Vec<String> = instances
            .iter()
            .map(|i| format!("'{}'", i.replace('\'', "")))
            .collect();
        sql.push_str(&format!(" AND source_instance_id IN ({})", list.join(",")));
    }
    sql.push_str(" ORDER BY event_id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([range_start, range_end], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<i64>>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, String>(10)?,
            r.get::<_, String>(11)?,
            r.get::<_, String>(12)?,
            r.get::<_, String>(13)?,
            r.get::<_, Option<i64>>(14)?,
            r.get::<_, Option<String>>(15)?,
            r.get::<_, Option<String>>(16)?,
            r.get::<_, String>(17)?,
            r.get::<_, String>(18)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (
            provider_id,
            model_canonical,
            model_raw,
            occurred_at_ms,
            input_uncached,
            input_cache_read,
            input_cache_write,
            input_total,
            output_total,
            quality_json,
            instance,
            agent,
            provider,
            model,
            source_amount_minor,
            source_currency,
            source_kind,
            category,
            quality,
        ) = row?;
        let token_quality: Option<serde_json::Value> = serde_json::from_str(&quality_json).ok();
        let known = |field: &str, value: Option<i64>| -> Option<i64> {
            let status = token_quality
                .as_ref()
                .and_then(|q| q.get(field))
                .and_then(|v| v.as_str());
            match status {
                Some("reported") | Some("derived") => value,
                _ => None,
            }
        };
        out.push(EventWithDims {
            category,
            quality,
            event: crate::pricing::PricingEvent {
                provider_id,
                model_canonical,
                model_raw,
                occurred_at_ms,
                input_uncached: known("input_uncached", input_uncached),
                input_cache_read: known("input_cache_read", input_cache_read),
                input_cache_write: known("input_cache_write", input_cache_write),
                input_total: known("input_total", input_total),
                output_total: known("output_total", output_total),
            },
            instance,
            agent,
            provider,
            model,
            source_amount_minor,
            source_currency,
            source_kind,
        });
    }
    Ok(out)
}

/// Rebuild a day in one transaction: replace unsealed costs with estimates and source amounts.
pub(crate) fn recompute_cost_day_tx(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
    now_ms: i64,
    options: &EstimateOptions,
    data_revision: i64,
) -> Result<CostDayOutcome, CoreError> {
    let day_str = day.to_string();
    let tz = calendar.tz_name();
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    let has_events: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM usage_events WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2)",
        [start_ms, end_ms],
        |r| r.get(0),
    )?;
    if !has_events {
        return Ok(CostDayOutcome {
            day: day_str,
            rebuilt: false,
            estimated_rows: 0,
            source_cost_rows: 0,
        });
    }
    tx.execute(
        "DELETE FROM daily_cost_usage WHERE tz_version = ?1 AND local_day = ?2 AND sealed = 0",
        [tz, day_str.as_str()],
    )?;

    let book = load_price_book_conn(tx)?;
    // Group by instance, agent, provider, model, currency, and cost kind.
    struct Acc {
        exact_cost: CostCurrencyRow,
        instance: String,
        agent: String,
        provider: String,
        model: String,
        currency: String,
        kind: String,
        priced_events: i64,
        unpriced_events: i64,
        partial_events: i64,
        ttl_defaulted: i64,
        fallback_events: i64,
        input_amount: Option<i64>,
        cache_read_amount: Option<i64>,
        cache_write_amount: Option<i64>,
        output_amount: Option<i64>,
        total_amount: i64,
        priced_tokens: i64,
        known_tokens: i64,
        unpriced_reasons: BTreeMap<String, i64>,
        price_basis: BTreeSet<String>,
    }
    impl Acc {
        fn key_matches(
            &self,
            instance: &str,
            agent: &str,
            provider: &str,
            model: &str,
            currency: &str,
            kind: &str,
        ) -> bool {
            self.instance == instance
                && self.agent == agent
                && self.provider == provider
                && self.model == model
                && self.currency == currency
                && self.kind == kind
        }
        fn blank(
            instance: &str,
            agent: &str,
            provider: &str,
            model: &str,
            currency: &str,
            kind: &str,
        ) -> Acc {
            Acc {
                exact_cost: CostCurrencyRow::default(),
                instance: instance.to_string(),
                agent: agent.to_string(),
                provider: provider.to_string(),
                model: model.to_string(),
                currency: currency.to_string(),
                kind: kind.to_string(),
                priced_events: 0,
                unpriced_events: 0,
                partial_events: 0,
                ttl_defaulted: 0,
                fallback_events: 0,
                input_amount: None,
                cache_read_amount: None,
                cache_write_amount: None,
                output_amount: None,
                total_amount: 0,
                priced_tokens: 0,
                known_tokens: 0,
                unpriced_reasons: BTreeMap::new(),
                price_basis: BTreeSet::new(),
            }
        }
    }
    fn account_mut<'a>(
        accounts: &'a mut Vec<Acc>,
        instance: &str,
        agent: &str,
        provider: &str,
        model: &str,
        currency: &str,
        kind: &str,
    ) -> &'a mut Acc {
        if let Some(idx) = accounts
            .iter()
            .position(|a| a.key_matches(instance, agent, provider, model, currency, kind))
        {
            return &mut accounts[idx];
        }
        accounts.push(Acc::blank(instance, agent, provider, model, currency, kind));
        accounts.last_mut().expect("just pushed")
    }

    let mut accounts: Vec<Acc> = Vec::new();
    let events = collect_events_for_pricing(tx, start_ms, end_ms, &CostFilters::default())?;
    let mut books: BTreeMap<(String, String), PriceBook> = BTreeMap::new();
    for row in &events {
        crate::adapters::run_policy::check()?;
        let key = crate::model_names::model_key(
            row.event
                .model_canonical
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .or(row.event.model_raw.as_deref())
                .unwrap_or_default(),
        );
        let book = books
            .entry((
                key.clone(),
                crate::model_names::reference_model_key_for(
                    &key,
                    row.event.provider_id.as_deref(),
                    row.event.occurred_at_ms,
                ),
            ))
            .or_insert_with(|| book.for_model(&row.event));
        // 1) Estimate using prices at occurrence time.
        match book.estimate_at_time(&row.event, options) {
            EventEstimate::Priced(amounts) => {
                let partial =
                    amounts.priced_tokens < amounts.known_tokens || amounts.has_unknown_components;
                let acc = account_mut(
                    &mut accounts,
                    &row.instance,
                    &row.agent,
                    &row.provider,
                    &row.model,
                    &amounts.currency,
                    KIND_ESTIMATE_AT_TIME,
                );
                acc.priced_events += 1;
                if partial {
                    acc.partial_events += 1;
                }
                if amounts.ttl_defaulted {
                    acc.ttl_defaulted += 1;
                }
                if amounts.official_fallback {
                    acc.fallback_events += 1;
                }
                accumulate_amounts(&mut acc.exact_cost, &amounts, 1, false)?;
                acc.input_amount = acc.exact_cost.input_amount_minor;
                acc.cache_read_amount = acc.exact_cost.cache_read_amount_minor;
                acc.cache_write_amount = acc.exact_cost.cache_write_amount_minor;
                acc.output_amount = acc.exact_cost.output_amount_minor;
                acc.total_amount = acc.exact_cost.total_amount_minor;
                acc.priced_tokens = acc
                    .priced_tokens
                    .checked_add(amounts.priced_tokens)
                    .ok_or(CoreError::Overflow("daily cost priced tokens"))?;
                acc.known_tokens = acc
                    .known_tokens
                    .checked_add(amounts.known_tokens)
                    .ok_or(CoreError::Overflow("daily cost known tokens"))?;
                for id in &amounts.matched_price_ids {
                    if let Some(price_row) = book.rows.iter().find(|r| &r.price_id == id) {
                        acc.price_basis.insert(price_row.snapshot_id.clone());
                    }
                }
            }
            EventEstimate::Unpriced(reason) => {
                let acc = account_mut(
                    &mut accounts,
                    &row.instance,
                    &row.agent,
                    &row.provider,
                    &row.model,
                    "",
                    KIND_ESTIMATE_AT_TIME,
                );
                acc.unpriced_events += 1;
                *acc.unpriced_reasons
                    .entry(reason.as_str().to_string())
                    .or_insert(0) += 1;
            }
        }
        // 2) Retain reported amounts and source estimates as separate kinds.
        if let (Some(amount), Some(currency), Some(kind_str)) = (
            row.source_amount_minor,
            row.source_currency.clone(),
            row.source_kind.clone(),
        ) {
            let kind = match kind_str.as_str() {
                "reported" => KIND_SOURCE_REPORTED,
                "estimated" => KIND_SOURCE_ESTIMATE,
                _ => continue,
            };
            let acc = account_mut(
                &mut accounts,
                &row.instance,
                &row.agent,
                &row.provider,
                &row.model,
                &currency,
                kind,
            );
            acc.priced_events += 1;
            acc.total_amount = acc
                .total_amount
                .checked_add(amount)
                .ok_or(CoreError::Overflow("daily source cost"))?;
        }
    }

    let mut estimated_rows = 0usize;
    let mut source_cost_rows = 0usize;
    {
        let mut insert = tx.prepare(
            "INSERT INTO daily_cost_usage (
               tz_version, local_day, instance_id, agent, provider_id, model_raw,
               currency, kind, priced_event_count, unpriced_event_count,
               partial_event_count, ttl_defaulted_events, fallback_event_count,
               input_amount_minor, cache_read_amount_minor, cache_write_amount_minor,
               output_amount_minor, total_amount_minor, priced_tokens, known_tokens,
               unpriced_reasons, price_basis, sealed, data_revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                       ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, 0, ?23)",
        )?;
        for acc in &accounts {
            let is_estimate = acc.kind == KIND_ESTIMATE_AT_TIME;
            let reasons_json = if is_estimate && !acc.unpriced_reasons.is_empty() {
                Some(serde_json::to_string(&acc.unpriced_reasons).map_err(|e| {
                    CoreError::Validation(format!("unpriced reasons serialization: {e}"))
                })?)
            } else {
                None
            };
            insert.execute(rusqlite::params![
                tz,
                day_str,
                acc.instance,
                acc.agent,
                acc.provider,
                acc.model,
                acc.currency,
                acc.kind,
                acc.priced_events,
                acc.unpriced_events,
                acc.partial_events,
                acc.ttl_defaulted,
                acc.fallback_events,
                acc.input_amount,
                acc.cache_read_amount,
                acc.cache_write_amount,
                acc.output_amount,
                acc.total_amount,
                acc.priced_tokens,
                acc.known_tokens,
                reasons_json,
                serde_json::to_string(&acc.price_basis.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_else(|_| "[]".to_string()),
                data_revision,
            ])?;
            if is_estimate {
                estimated_rows += 1;
            } else {
                source_cost_rows += 1;
            }
        }
    }
    let _ = now_ms;
    Ok(CostDayOutcome {
        day: day_str,
        rebuilt: true,
        estimated_rows,
        source_cost_rows,
    })
}

/// Cost filters use query::Filters rules: AND across fields, OR within a field;
/// unknown matches empty values, and instances Some([]) selects no available source.
#[derive(Debug, Clone, Default)]
pub struct CostFilters {
    pub agents: Vec<String>,
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub instances: Option<Vec<String>>,
}

/// Cost summary request.
#[derive(Debug, Clone)]
pub struct CostSummaryRequest {
    pub timezone: String,
    /// Inclusive local date in YYYY-MM-DD format.
    pub first_day: String,
    pub last_day: String,
    pub filters: CostFilters,
    pub now_ms: i64,
    pub options: EstimateOptions,
}

/// Amounts by currency; currencies are never merged (E8).
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostCurrencyRow {
    pub substitute_models: Vec<String>,
    /// Upper bound for the same priced components when archived request tiers are unknown.
    pub upper_amount_minor: Option<i64>,
    pub aggregate_event_count: i64,
    #[serde(skip)]
    exact: [Option<i128>; 4],
    #[serde(skip)]
    exact_upper: [Option<i128>; 4],
    pub currency: String,
    pub total_amount_minor: i64,
    pub input_amount_minor: Option<i64>,
    pub cache_read_amount_minor: Option<i64>,
    pub cache_write_amount_minor: Option<i64>,
    pub output_amount_minor: Option<i64>,
    pub priced_tokens: i64,
    pub known_tokens: i64,
    pub priced_event_count: i64,
    pub unpriced_event_count: i64,
    pub partial_event_count: i64,
    pub ttl_defaulted_events: i64,
    /// Events priced with an official pay-as-you-go reference because no exact match exists.
    pub fallback_event_count: i64,
}

impl CostCurrencyRow {
    fn empty(currency: String) -> Self {
        CostCurrencyRow {
            currency,
            ..Default::default()
        }
    }
}

fn accumulate_amounts(
    row: &mut CostCurrencyRow,
    amounts: &crate::pricing::EventEstimateAmounts,
    count: i64,
    aggregate: bool,
) -> Result<(), CoreError> {
    if let Some(model) = &amounts.substitute_model {
        if !row.substitute_models.contains(model) {
            row.substitute_models.push(model.clone());
            row.substitute_models.sort();
        }
    }
    fn add(target: &mut [Option<i128>; 4], value: [Option<i128>; 4]) -> Result<(), CoreError> {
        for (target, value) in target.iter_mut().zip(value) {
            if let Some(value) = value {
                *target = Some(
                    target
                        .unwrap_or(0)
                        .checked_add(value)
                        .ok_or(CoreError::Overflow("cost numerator sum"))?,
                );
            }
        }
        Ok(())
    }
    fn rounded(values: [Option<i128>; 4]) -> Result<([Option<i64>; 4], i64), CoreError> {
        let mut out = [None; 4];
        let mut total = 0i64;
        for (slot, value) in out.iter_mut().zip(values) {
            if let Some(value) = value {
                let v = i64::try_from(
                    value
                        .checked_add(50_000_000)
                        .ok_or(CoreError::Overflow("cost rounding"))?
                        / 100_000_000,
                )
                .map_err(|_| CoreError::Overflow("cost amount"))?;
                *slot = Some(v);
                total = total
                    .checked_add(v)
                    .ok_or(CoreError::Overflow("cost total"))?;
            }
        }
        Ok((out, total))
    }
    add(&mut row.exact, amounts.component_numerators)?;
    add(
        &mut row.exact_upper,
        amounts
            .upper_numerators
            .unwrap_or(amounts.component_numerators),
    )?;
    let (components, total) = rounded(row.exact)?;
    [
        row.input_amount_minor,
        row.cache_read_amount_minor,
        row.cache_write_amount_minor,
        row.output_amount_minor,
    ] = components;
    row.total_amount_minor = total;
    if amounts.upper_numerators.is_some() || row.upper_amount_minor.is_some() {
        row.upper_amount_minor = Some(rounded(row.exact_upper)?.1);
    }
    row.priced_tokens = row
        .priced_tokens
        .checked_add(amounts.priced_tokens)
        .ok_or(CoreError::Overflow("priced tokens"))?;
    row.known_tokens = row
        .known_tokens
        .checked_add(amounts.known_tokens)
        .ok_or(CoreError::Overflow("known tokens"))?;
    row.priced_event_count = row
        .priced_event_count
        .checked_add(count)
        .ok_or(CoreError::Overflow("priced events"))?;
    if aggregate {
        row.aggregate_event_count += count;
    }
    if amounts.priced_tokens < amounts.known_tokens || amounts.has_unknown_components {
        row.partial_event_count += count;
    }
    if amounts.ttl_defaulted {
        row.ttl_defaulted_events += count;
    }
    if amounts.official_fallback {
        row.fallback_event_count += count;
    }
    Ok(())
}

/// One estimation mode: currency rows and unpriced reasons.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostModeSummary {
    pub rows: Vec<CostCurrencyRow>,
    /// Counts of events by unpriced reason.
    pub unpriced_reasons: BTreeMap<String, i64>,
    /// Reference evaluation time and detail coverage flag; unused by at_time.
    pub as_of_ms: i64,
    pub detail_limited: bool,
}

/// Cost response with data_revision and the referenced price_basis snapshot set.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostSummary {
    /// Historical estimates stored in daily cost rows.
    pub at_time: CostModeSummary,
    /// Source reports and source estimates, kept separate by kind and currency.
    pub source_amounts: Vec<CostCurrencyRow>,
    /// Current reference estimates from mutually exclusive known detail or archive usage.
    pub current_sim: CostModeSummary,
    /// Price snapshots referenced by at_time.
    pub price_basis: Vec<String>,
    pub data_revision: i64,
    pub models: Vec<CostModelRow>,
    pub daily: Vec<CostDayRow>,
    pub daily_current: Vec<CostDayRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostModelRow {
    pub provider: String,
    pub model: String,
    pub at_time: Vec<CostCurrencyRow>,
    pub current_sim: Vec<CostCurrencyRow>,
    pub unit_prices: Vec<PriceRow>,
    pub unpriced_reasons: BTreeMap<String, i64>,
    pub reference_models: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CostDayRow {
    pub day: String,
    pub provider: String,
    pub model: String,
    pub sums: CostCurrencyRow,
}

impl CostSummary {
    fn empty() -> Self {
        CostSummary::default()
    }
}

/// Seal daily costs at detail retention expiry, preserving historical amounts.
pub(crate) fn seal_cost_days_tx(
    tx: &Transaction<'_>,
    tz: &str,
    cutoff_day: &str,
) -> Result<usize, CoreError> {
    Ok(tx.execute(
        "UPDATE daily_cost_usage SET sealed = 1
         WHERE tz_version = ?1 AND local_day < ?2 AND sealed = 0",
        [tz, cutoff_day],
    )?)
}

/// Delete daily costs with daily_usage using the same daily retention cutoff.
pub(crate) fn prune_cost_days_tx(
    tx: &Transaction<'_>,
    tz: &str,
    cutoff_day: &str,
) -> Result<usize, CoreError> {
    Ok(tx.execute(
        "DELETE FROM daily_cost_usage WHERE tz_version = ?1 AND local_day < ?2",
        [tz, cutoff_day],
    )?)
}
