//! User-configured retention for details and hourly/daily/weekly/monthly/yearly archives.
//! Longer aggregate periods retain longer history; materialize weeks/months/years before deleting days.
//! Compute distinct_sessions from retained details; materialized NULL leaves that count unknown (V06).
//! Materialized weeks use Monday/ISO labels; changing their week-start requires explicit rebuilding.

use crate::calendar::Calendar;
use crate::error::CoreError;
use crate::storage::Storage;
use jiff::civil::Date;
use jiff::Span;
use rusqlite::{params, OptionalExtension, Transaction};

/// Tiered retention durations in days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TieredRetentionPolicy {
    pub events_days: u32,
    pub hourly_days: u32,
    pub daily_days: u32,
    pub weekly_days: u32,
    pub monthly_days: u32,
    /// None retains yearly history indefinitely.
    pub yearly_days: Option<u32>,
}

impl Default for TieredRetentionPolicy {
    fn default() -> Self {
        // User-selected defaults from 2026-09-26: hourly 3 days, daily 90 days, weekly 3 years, monthly 10 years.
        TieredRetentionPolicy {
            events_days: 7,
            hourly_days: 3,
            daily_days: 90,
            weekly_days: 1095,
            monthly_days: 3650,
            yearly_days: None,
        }
    }
}

impl TieredRetentionPolicy {
    pub fn validate(&self) -> Result<(), CoreError> {
        // Detail retention is independent of aggregate tiers and may exceed hourly retention.
        // Aggregate durations must satisfy hourly <= daily <= weekly <= monthly <= yearly.
        if self.events_days < 1 {
            return Err(CoreError::Validation(
                "tiered retention requires events_days >= 1".into(),
            ));
        }
        if self.daily_days < self.hourly_days {
            return Err(CoreError::Validation(
                "tiered retention requires hourly_days <= daily_days".into(),
            ));
        }
        if self.weekly_days < self.daily_days || self.monthly_days < self.weekly_days {
            return Err(CoreError::Validation(
                "tiered retention requires daily_days <= weekly_days <= monthly_days".into(),
            ));
        }
        if let Some(y) = self.yearly_days {
            if y < self.monthly_days {
                return Err(CoreError::Validation(
                    "tiered retention requires monthly_days <= yearly_days".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TieredRetentionOutcome {
    pub deleted_events: i64,
    pub deleted_diagnostics: i64,
    pub deleted_hourly_rows: i64,
    pub deleted_daily_rows: i64,
    pub deleted_period_rows: i64,
    pub materialized_period_rows: i64,
    pub data_revision: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MaterializationMemo {
    version: u8,
    scope: String,
    revision: i64,
    daily_rows: i64,
    period_rows: i64,
}

struct PartitionCount {
    rows: i64,
    revision: i64,
}

// A day can belong to an incomplete week and a completed month/year. Use the
// latest boundary so every day affecting any completed period is considered.
fn completed_before(calendar: &Calendar, today: Date) -> Result<Date, CoreError> {
    Ok(calendar
        .week_start_of(today, crate::calendar::WeekStart::Monday)
        .max(calendar.month_start_of(today))
        .max(Date::new(today.year(), 1, 1)?))
}

fn materialization_counts(
    tx: &Transaction<'_>,
    timezone: &str,
    before: Date,
) -> Result<(PartitionCount, PartitionCount), CoreError> {
    let daily = tx.query_row(
        "SELECT COUNT(*),COALESCE(MAX(data_revision),0) FROM daily_usage
         WHERE tz_version=?1 AND local_day<?2",
        params![timezone, before.to_string()],
        |r| {
            Ok(PartitionCount {
                rows: r.get(0)?,
                revision: r.get(1)?,
            })
        },
    )?;
    let periods = tx.query_row(
        "SELECT COUNT(*),COALESCE(MAX(data_revision),0) FROM period_usage WHERE tz_version=?1",
        [timezone],
        |r| {
            Ok(PartitionCount {
                rows: r.get(0)?,
                revision: r.get(1)?,
            })
        },
    )?;
    Ok((daily, periods))
}

/// Enforce details, hourly, materialized week/month/year, daily, then period retention.
/// One transaction; the detail floor prevents expired records from returning on rescan (V14).
pub fn enforce_tiered_retention(
    storage: &Storage,
    timezone: &str,
    now_ms: i64,
    policy: &TieredRetentionPolicy,
) -> Result<TieredRetentionOutcome, CoreError> {
    crate::adapters::run_policy::check()?;
    enforce_tiered_retention_controlled(storage, timezone, now_ms, policy, None)
}

pub fn enforce_tiered_retention_controlled(
    storage: &Storage,
    timezone: &str,
    now_ms: i64,
    policy: &TieredRetentionPolicy,
    control: Option<&crate::cancellation::OperationControl>,
) -> Result<TieredRetentionOutcome, CoreError> {
    if let Some(control) = control {
        control.check()?;
    }
    policy.validate()?;
    let calendar = Calendar::new(timezone)?;
    let today = calendar.today(now_ms)?;
    let mut outcome = TieredRetentionOutcome::default();
    let conn = storage.conn();
    let tx = conn.unchecked_transaction()?;
    let memo_key = format!("retention_applied:{timezone}");
    let revision_before = crate::storage::data_revision(&tx)?;
    let signature = |revision| format!("{today}|{policy:?}|{revision}");
    let materialization_key = format!("retention_materialization:{timezone}");
    let materialization_scope = format!("{today}|{policy:?}");
    let complete_before = completed_before(&calendar, today)?;
    let last: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [&memo_key],
            |r| r.get(0),
        )
        .optional()?;
    if last.as_deref() == Some(signature(revision_before).as_str()) {
        if let Some(control) = control {
            control.enter_commit()?;
        }
        outcome.data_revision = revision_before;
        return Ok(outcome);
    }
    let revision = Storage::bump_data_revision_tx(&tx, now_ms)?;

    // 1. Seal expired daily partitions, then delete expired events/diagnostics.
    {
        let cutoff_day = calendar.retention_cutoff_day(today, policy.events_days)?;
        let (cutoff_ms, _) = calendar.day_range_ms(cutoff_day)?;
        tx.execute(
            "INSERT INTO settings (key, value, schema_version, updated_at_ms)
             VALUES ('detail_retention_floor_ms', ?1, 1, ?2)
             ON CONFLICT(key) DO UPDATE SET
               value = CAST(MAX(CAST(value AS INTEGER), CAST(excluded.value AS INTEGER)) AS TEXT),
               updated_at_ms = excluded.updated_at_ms",
            params![cutoff_ms.to_string(), now_ms],
        )?;
        tx.execute(
            "UPDATE daily_usage SET sealed = 1, sealed_at_ms = ?2, seal_tz = ?3,
               seal_field_version = ?4
             WHERE tz_version = ?1 AND local_day < ?5 AND sealed = 0",
            params![
                timezone,
                now_ms,
                timezone,
                crate::retention::SEAL_FIELD_VERSION,
                cutoff_day.to_string()
            ],
        )?;
        outcome.deleted_events = tx.execute(
            "DELETE FROM usage_events WHERE occurred_at_ms < ?1",
            params![cutoff_ms],
        )? as i64;
        outcome.deleted_diagnostics = tx.execute(
            "DELETE FROM diagnostics WHERE created_ms < ?1",
            params![cutoff_ms],
        )? as i64;
        // F2: Seal daily cost rows after details expire; keep historical amounts unchanged and queryable.
        crate::storage::pricing::seal_cost_days_tx(&tx, timezone, &cutoff_day.to_string())?;
    }

    // 2. Apply hourly retention.
    {
        let cutoff_day = calendar.retention_cutoff_day(today, policy.hourly_days)?;
        outcome.deleted_hourly_rows = tx.execute(
            "DELETE FROM hourly_usage WHERE tz_version = ?1 AND local_day < ?2",
            params![timezone, cutoff_day.to_string()],
        )? as i64;
    }

    // 3. Completed history is stable when only open periods change. Row counts
    // detect removed partitions; revisions detect equal-sized replacements and
    // late corrections. Imported future revisions conservatively force rebuild.
    {
        let raw: Option<String> = tx
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                [&materialization_key],
                |r| r.get(0),
            )
            .optional()?;
        let memo = raw.and_then(|raw| serde_json::from_str::<MaterializationMemo>(&raw).ok());
        let (daily, periods) = materialization_counts(&tx, timezone, complete_before)?;
        let unchanged = memo.is_some_and(|memo| {
            memo.version == 1
                && memo.scope == materialization_scope
                && memo.revision <= revision_before
                && memo.daily_rows == daily.rows
                && daily.revision <= memo.revision
                && memo.period_rows == periods.rows
                && periods.revision <= memo.revision
        });
        let oldest: Option<String> = tx
            .query_row(
                "SELECT MIN(local_day) FROM daily_usage WHERE tz_version = ?1",
                params![timezone],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(oldest) = oldest.filter(|_| !unchanged) {
            let from = crate::calendar::parse_date(&oldest)?;
            outcome.materialized_period_rows =
                materialize_periods(&tx, &calendar, from, today, now_ms, revision)?;
        }
    }

    // 4. Delete daily rows after materialization; protect starts of ongoing weeks/months/years.
    // Deleting those days would lose the inputs needed to finish the period.
    // Retain from the earliest protected year/month/week start, then include its whole ISO boundary week.
    {
        let mut cutoff_day = calendar.retention_cutoff_day(today, policy.daily_days)?;
        let week_start = calendar.week_start_of(today, crate::calendar::WeekStart::Monday);
        let month_start = calendar.month_start_of(today);
        if let Ok(year_start) = jiff::civil::Date::new(today.year(), 1, 1) {
            cutoff_day = cutoff_day.min(year_start);
        }
        cutoff_day = cutoff_day.min(week_start).min(month_start);
        // Keep a whole ISO week at the boundary. A later cleanup must not
        // rebuild that week from only its surviving January days.
        cutoff_day = calendar.week_start_of(cutoff_day, crate::calendar::WeekStart::Monday);
        outcome.deleted_daily_rows = tx.execute(
            "DELETE FROM daily_usage WHERE tz_version = ?1 AND local_day < ?2",
            params![timezone, cutoff_day.to_string()],
        )? as i64;
        // F2: Prune daily cost rows with daily retention after detail/estimate history expires.
        crate::storage::pricing::prune_cost_days_tx(&tx, timezone, &cutoff_day.to_string())?;
        tx.execute(
            "INSERT INTO settings(key, value, schema_version, updated_at_ms) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(key) DO UPDATE SET value = MAX(value, excluded.value), updated_at_ms = excluded.updated_at_ms",
            params![format!("daily_retention_floor:{timezone}"), cutoff_day.to_string(), now_ms],
        )?;
    }

    // 5. Apply weekly/monthly retention and optional yearly retention; None retains years indefinitely.
    for (granularity, days) in [("week", policy.weekly_days), ("month", policy.monthly_days)] {
        let cutoff_day = calendar.retention_cutoff_day(today, days)?;
        outcome.deleted_period_rows += tx.execute(
            "DELETE FROM period_usage WHERE tz_version = ?1 AND granularity = ?2
             AND period_end_day < ?3",
            params![timezone, granularity, cutoff_day.to_string()],
        )? as i64;
    }
    if let Some(yearly) = policy.yearly_days {
        let cutoff_day = calendar.retention_cutoff_day(today, yearly)?;
        outcome.deleted_period_rows += tx.execute(
            "DELETE FROM period_usage WHERE tz_version = ?1 AND granularity = 'year'
             AND period_end_day < ?2",
            params![timezone, cutoff_day.to_string()],
        )? as i64;
    }

    outcome.data_revision = revision;
    let (daily, periods) = materialization_counts(&tx, timezone, complete_before)?;
    let materialization = MaterializationMemo {
        version: 1,
        scope: materialization_scope,
        revision,
        daily_rows: daily.rows,
        period_rows: periods.rows,
    };
    tx.execute(
        "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,?2,1,?3)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
        params![
            materialization_key,
            serde_json::to_string(&materialization)?,
            now_ms
        ],
    )?;
    tx.execute(
        "INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,?2,1,?3)
        ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
        params![memo_key, signature(revision), now_ms],
    )?;
    if let Some(control) = control {
        control.enter_commit()?;
    }
    crate::adapters::run_policy::check()?;
    tx.commit()?;
    Ok(outcome)
}

/// Materialize completed weeks/months/years represented in inclusive [from, to]; delete old period rows before inserting replacements.
/// Skip periods with end_day >= to; daily rows continue to serve ongoing periods.
pub(crate) fn materialize_periods(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    from: Date,
    to: Date,
    now_ms: i64,
    revision: i64,
) -> Result<i64, CoreError> {
    let timezone = calendar.tz_name().to_string();
    struct Agg {
        dims: (String, String, String, String, String, String),
        start: Date,
        end: Date,
        event_count: i64,
        call_count: i64,
        input: i64,
        known_input: bool,
        cache_read: i64,
        known_read: bool,
        cache_write: i64,
        known_write: bool,
        output: i64,
        known_output: bool,
        total: i64,
        known_total: bool,
        conflict: i64,
        days: std::collections::BTreeSet<String>,
    }
    let mut stmt = tx.prepare(
        "SELECT local_day, instance_id, agent, provider_id, model_raw, call_category,
                quality_bucket, event_count, call_count, input_known_sum,
                cache_read_known_sum, cache_write_known_sum, output_known_sum,
                total_known_sum, conflict_count
         FROM daily_usage WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3",
    )?;
    let rows = stmt.query_map(params![timezone, from.to_string(), to.to_string()], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, i64>(8)?,
            r.get::<_, Option<i64>>(9)?,
            r.get::<_, Option<i64>>(10)?,
            r.get::<_, Option<i64>>(11)?,
            r.get::<_, Option<i64>>(12)?,
            r.get::<_, Option<i64>>(13)?,
            r.get::<_, i64>(14)?,
        ))
    })?;
    let mut periods: std::collections::BTreeMap<(String, String, [String; 6]), Agg> =
        std::collections::BTreeMap::new();
    for row in rows {
        crate::adapters::run_policy::check()?;
        let (
            day_str,
            instance,
            agent,
            provider,
            model,
            category,
            bucket,
            event_count,
            call_count,
            input,
            cache_read,
            cache_write,
            output,
            total,
            conflict,
        ) = row?;
        let day = crate::calendar::parse_date(&day_str)?;
        let dims = [instance, agent, provider, model, category, bucket];
        for (granularity, key, start, end) in period_keys_of(calendar, day) {
            let agg = periods
                .entry((granularity, key.clone(), dims.clone()))
                .or_insert_with(|| Agg {
                    dims: (
                        dims[0].clone(),
                        dims[1].clone(),
                        dims[2].clone(),
                        dims[3].clone(),
                        dims[4].clone(),
                        dims[5].clone(),
                    ),
                    start,
                    end,
                    event_count: 0,
                    call_count: 0,
                    input: 0,
                    known_input: false,
                    cache_read: 0,
                    known_read: false,
                    cache_write: 0,
                    known_write: false,
                    output: 0,
                    known_output: false,
                    total: 0,
                    known_total: false,
                    conflict: 0,
                    days: std::collections::BTreeSet::new(),
                });
            agg.event_count += event_count;
            agg.call_count += call_count;
            if let Some(v) = input {
                agg.input += v;
                agg.known_input = true;
            }
            if let Some(v) = cache_read {
                agg.cache_read += v;
                agg.known_read = true;
            }
            if let Some(v) = cache_write {
                agg.cache_write += v;
                agg.known_write = true;
            }
            if let Some(v) = output {
                agg.output += v;
                agg.known_output = true;
            }
            if let Some(v) = total {
                agg.total += v;
                agg.known_total = true;
            }
            agg.conflict += conflict;
            agg.days.insert(day_str.clone());
        }
    }
    drop(stmt);
    let mut written = 0i64;
    let floor: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [format!("daily_retention_floor:{timezone}")],
            |r| r.get(0),
        )
        .optional()?;
    let mut replaced = std::collections::BTreeSet::new();
    let mut insert = tx.prepare(
        "INSERT INTO period_usage (
           tz_version, granularity, period_key, period_start_day, period_end_day,
           instance_id, agent, provider_id, model_raw, call_category, quality_bucket,
           event_count, call_count, input_known_sum, cache_read_known_sum,
           cache_write_known_sum, output_known_sum, total_known_sum, conflict_count,
           active_days, distinct_sessions, materialized_at_ms, data_revision
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, NULL, ?21, ?22)",
    )?;
    for ((granularity, key, _), agg) in periods {
        crate::adapters::run_policy::check()?;
        if agg.end >= to {
            continue;
        }
        if floor
            .as_ref()
            .is_some_and(|floor| agg.start.to_string() < *floor)
        {
            continue;
        }
        if replaced.insert((granularity.clone(), key.clone())) {
            tx.execute(
                "DELETE FROM period_usage WHERE tz_version = ?1 AND granularity = ?2
             AND period_key = ?3",
                params![timezone, granularity, key],
            )?;
        }
        insert.execute(params![
            timezone,
            granularity,
            key,
            agg.start.to_string(),
            agg.end.to_string(),
            agg.dims.0,
            agg.dims.1,
            agg.dims.2,
            agg.dims.3,
            agg.dims.4,
            agg.dims.5,
            agg.event_count,
            agg.call_count,
            if agg.known_input {
                Some(agg.input)
            } else {
                None
            },
            if agg.known_read {
                Some(agg.cache_read)
            } else {
                None
            },
            if agg.known_write {
                Some(agg.cache_write)
            } else {
                None
            },
            if agg.known_output {
                Some(agg.output)
            } else {
                None
            },
            if agg.known_total {
                Some(agg.total)
            } else {
                None
            },
            agg.conflict,
            agg.days.len() as i64,
            now_ms,
            revision,
        ])?;
        written += 1;
    }
    Ok(written)
}

/// Week/month/year containing a day; weeks use ISO Monday labels as described above.
fn period_keys_of(calendar: &Calendar, day: Date) -> Vec<(String, String, Date, Date)> {
    let mut out = Vec::with_capacity(3);
    let week_start = calendar.week_start_of(day, crate::calendar::WeekStart::Monday);
    if let Ok(week_end) = week_start.checked_add(Span::new().days(6)) {
        out.push((
            "week".to_string(),
            calendar.week_label(day, crate::calendar::WeekStart::Monday),
            week_start,
            week_end,
        ));
    }
    let month_start = calendar.month_start_of(day);
    let month_end = crate::calendar::month_end(day);
    out.push((
        "month".to_string(),
        calendar.month_label(day),
        month_start,
        month_end,
    ));
    let year = day.year();
    if let (Ok(y_start), Ok(y_end)) = (Date::new(year, 1, 1), Date::new(year, 12, 31)) {
        out.push(("year".to_string(), format!("{year}"), y_start, y_end));
    }
    out
}
