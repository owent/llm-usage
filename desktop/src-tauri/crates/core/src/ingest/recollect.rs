use super::{insert_event, persist_source_hourly_day, recompute_source_day, update_event};
use crate::adapters::framework::{
    DetectOutcome, DiscoverContext, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, SourceAdapter,
    StoredScanState,
};
use crate::adapters::zed::ZedAdapter;
use crate::calendar::{parse_date, Calendar};
use crate::cancellation::OperationControl;
use crate::domain::TimeBasis;
use crate::error::CoreError;
use crate::identity::{content_hash, event_content_hash, event_id};
use crate::storage::Storage;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RecollectWindow {
    pub start: String,
    pub end: String,
    pub timezone: String,
    start_ms: i64,
    end_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecollectHour {
    pub day: String,
    pub hour: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecollectRange {
    RecentDays {
        days: u32,
    },
    Hours {
        start: RecollectHour,
        end: RecollectHour,
    },
}

impl RecollectRange {
    pub fn window(
        &self,
        timezone: &str,
        now_ms: i64,
        limit: u32,
    ) -> Result<RecollectWindow, CoreError> {
        let calendar = Calendar::new(timezone)?;
        let today = calendar.today(now_ms)?;
        let tomorrow = today.tomorrow()?;
        let (_, latest_end) = calendar.day_range_ms(today)?;
        let (start, end, start_ms, end_ms) = match self {
            Self::RecentDays { days } if (1..=limit).contains(days) => {
                let first = calendar.retention_cutoff_day(today, *days)?;
                let (start_ms, _) = calendar.day_range_ms(first)?;
                (
                    format!("{first} 00:00"),
                    format!("{tomorrow} 00:00"),
                    start_ms,
                    latest_end,
                )
            }
            Self::RecentDays { .. } => {
                return Err(CoreError::Validation("outside detail retention".into()))
            }
            Self::Hours { start, end } => {
                let first = parse_date(&start.day)?;
                let last = parse_date(&end.day)?;
                let start_ms = calendar.hour_boundary_ms(first, start.hour)?;
                let end_ms = calendar.hour_boundary_ms(last, end.hour)?;
                let floor = calendar.retention_cutoff_day(today, limit)?;
                if first < floor || start_ms > now_ms || end_ms > latest_end || start_ms >= end_ms {
                    return Err(CoreError::Validation(
                        "hour range must be ordered and within recent detail retention".into(),
                    ));
                }
                (
                    format!("{first} {:02}:00", start.hour),
                    format!("{last} {:02}:00", end.hour),
                    start_ms,
                    end_ms,
                )
            }
        };
        Ok(RecollectWindow {
            start,
            end,
            timezone: timezone.into(),
            start_ms,
            end_ms,
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct RecollectOutcome {
    pub changed_events: usize,
    pub removed_events: usize,
    pub changed_aggregates: usize,
    pub data_revision: i64,
}

pub fn read_zed_source(
    context: &DiscoverContext,
    instance: &str,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let adapter = ZedAdapter;
    let mut roots = adapter
        .discover(context)
        .into_iter()
        .filter(|root| adapter.instance_id(root) == instance);
    let root = roots
        .next()
        .ok_or_else(|| CoreError::Validation("Zed source was not found".into()))?;
    let [path] = root.files.as_slice() else {
        return Err(CoreError::Validation("ambiguous Zed source files".into()));
    };
    if roots.any(|candidate| {
        candidate.files.iter().any(|candidate_path| {
            crate::adapters::framework::normalize_path(candidate_path)
                != crate::adapters::framework::normalize_path(path)
        })
    }) {
        return Err(CoreError::Validation("ambiguous Zed source files".into()));
    }
    if !matches!(adapter.detect(path)?, DetectOutcome::Supported { .. }) {
        return Err(CoreError::Validation("Zed source is not readable".into()));
    }
    let probe = crate::adapters::jsonl::probe_file(path)?;
    let target = ScanTarget {
        instance_id: instance.into(),
        path: path.clone(),
        file_id: String::new(),
        file_identity: String::new(),
        probe,
        generation: 0,
        rescan: false,
    };
    let scan = adapter.scan(
        &target,
        &StoredScanState::default(),
        &ScanLimits::default(),
        now_ms,
    )?;
    if scan.status != ScanStatus::Complete || scan.health != "active" {
        return Err(CoreError::Validation(
            "Zed source scan was incomplete".into(),
        ));
    }
    Ok(scan)
}

pub fn replace_zed_window(
    storage: &Storage,
    instance: &str,
    window: &RecollectWindow,
    hourly_days: u32,
    now_ms: i64,
    scan: ScanOutcome,
    control: &OperationControl,
) -> Result<RecollectOutcome, CoreError> {
    control.check()?;
    let tx = storage.conn().unchecked_transaction()?;
    let calendar = Calendar::new(&window.timezone)?;
    let (mut start, end) = (window.start_ms, window.end_ms);
    let detail_floor: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key='detail_retention_floor_ms'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(floor) = detail_floor {
        start = start.max(
            floor
                .parse()
                .map_err(|_| CoreError::Validation("invalid detail retention floor".into()))?,
        );
    }
    if let Some(floor) = crate::retention::hard_retention_floor(&tx)? {
        start = start.max(floor);
    }
    let calendars = stored_calendars(&tx, instance, &calendar, &mut start)?;
    let mut outcome = RecollectOutcome::default();
    let affected_times =
        replace_events(&tx, instance, start, end, now_ms, scan.events, &mut outcome)?;
    outcome.changed_aggregates =
        replace_aggregates(&tx, instance, start, end, now_ms, scan.aggregates)?;
    outcome.data_revision = if affected_times.is_empty() && outcome.changed_aggregates == 0 {
        crate::storage::data_revision(&tx)?
    } else {
        Storage::bump_data_revision_tx(&tx, now_ms)?
    };
    for calendar in calendars.values() {
        let hourly_floor = calendar.retention_cutoff_day(calendar.today(now_ms)?, hourly_days)?;
        let days = affected_times
            .iter()
            .map(|time| calendar.local_day_of(*time))
            .collect::<Result<BTreeSet<_>, _>>()?;
        for day in days {
            control.check()?;
            // Complete daily inputs remain queryable and can regenerate these derived periods.
            tx.execute(
                "DELETE FROM period_usage AS p WHERE instance_id=?1 AND tz_version=?2
                 AND period_start_day<=?3 AND period_end_day>=?3 AND active_days>0
                 AND period_start_day>=COALESCE((SELECT value FROM settings
                   WHERE key='daily_retention_floor:'||?2), '')
                 AND active_days=(SELECT COUNT(*) FROM daily_usage d
                   WHERE d.tz_version=p.tz_version AND d.instance_id=p.instance_id
                     AND d.local_day BETWEEN p.period_start_day AND p.period_end_day
                     AND d.agent=p.agent AND d.provider_id=p.provider_id AND d.model_raw=p.model_raw
                     AND d.call_category=p.call_category AND d.quality_bucket=p.quality_bucket)",
                params![instance, calendar.tz_name(), day.to_string()],
            )?;
            recompute_source_day(&tx, calendar, day, outcome.data_revision, Some(instance))?;
            if day >= hourly_floor {
                persist_source_hourly_day(
                    &tx,
                    calendar,
                    day,
                    outcome.data_revision,
                    Some(instance),
                )?;
            } else {
                tx.execute("DELETE FROM hourly_usage WHERE tz_version=?1 AND local_day=?2 AND instance_id=?3",
                    params![calendar.tz_name(), day.to_string(), instance])?;
            }
        }
    }
    crate::adapters::run_policy::check()?;
    control.enter_commit()?;
    tx.commit()?;
    Ok(outcome)
}

fn stored_calendars(
    tx: &Transaction<'_>,
    instance: &str,
    current: &Calendar,
    start: &mut i64,
) -> Result<BTreeMap<String, Calendar>, CoreError> {
    let mut calendars = BTreeMap::from([(current.tz_name().to_string(), current.clone())]);
    let mut stmt = tx.prepare(
        "SELECT tz_version, MAX(CASE WHEN sealed=1 THEN local_day END)
         FROM daily_usage WHERE instance_id=?1 GROUP BY tz_version
         UNION SELECT tz_version, NULL FROM hourly_usage WHERE instance_id=?1
         UNION SELECT tz_version, NULL FROM period_usage WHERE instance_id=?1",
    )?;
    let mut rows = stmt.query([instance])?;
    while let Some(row) = rows.next()? {
        let tz: String = row.get(0)?;
        let calendar = Calendar::new(&tz)?;
        if let Some(sealed_through) = row.get::<_, Option<String>>(1)? {
            let (_, sealed_end) = calendar.day_range_ms(parse_date(&sealed_through)?)?;
            *start = (*start).max(sealed_end);
        }
        calendars.insert(tz, calendar);
    }
    Ok(calendars)
}

fn replace_events(
    tx: &Transaction<'_>,
    instance: &str,
    start: i64,
    end: i64,
    now_ms: i64,
    events: Vec<crate::domain::EventInput>,
    outcome: &mut RecollectOutcome,
) -> Result<BTreeSet<i64>, CoreError> {
    let mut times = BTreeSet::new();
    let mut covered_sessions = BTreeSet::new();
    let mut replayed_keys = BTreeSet::new();
    let mut lookup = tx.prepare(
        "SELECT event_id, content_hash, occurred_at_ms,
                CASE WHEN time_basis='bounded_interval' THEN interval_start_ms ELSE occurred_at_ms END,
                CASE WHEN time_basis='bounded_interval' THEN interval_end_ms ELSE occurred_at_ms END,
                conflict
         FROM usage_events WHERE source_instance_id=?1 AND source_record_key=?2",
    )?;
    for event in events {
        crate::adapters::run_policy::check()?;
        if event.source_instance_id != instance || event.agent != "zed" {
            return Err(CoreError::Validation(
                "unexpected Zed event ownership".into(),
            ));
        }
        if event.occurred_at_ms < start
            || event.occurred_at_ms >= end
            || (event.time_basis == TimeBasis::BoundedInterval
                && !in_window(event.interval_start_ms, event.interval_end_ms, start, end))
        {
            continue;
        }
        event.validate()?;
        let hash = event_content_hash(&event);
        let old = lookup
            .query_row(params![instance, event.source_record_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            })
            .optional()?;
        if old.as_ref().is_some_and(|(_, _, at, from, to, _)| {
            *at < start || *at >= end || !in_window(*from, *to, start, end)
        }) {
            continue;
        }
        replayed_keys.insert(event.source_record_key.clone());
        covered_sessions.extend(event.session_id.clone());
        if let Some((id, previous, old_time, _, _, conflict)) = old {
            if previous == hash {
                continue;
            }
            update_event(tx, &event, &id, &hash, now_ms, true)?;
            if conflict {
                tx.execute(
                    "UPDATE usage_events SET conflict=1 WHERE event_id=?1",
                    [&id],
                )?;
            }
            times.insert(old_time);
        } else {
            let id = event_id(instance, &event.source_record_key);
            insert_event(tx, &event, &id, &hash, now_ms, true)?;
        }
        outcome.changed_events += 1;
        times.insert(event.occurred_at_ms);
    }

    // A missing telemetry match can mean log rotation. Only replace a session's projections
    // when the adapter has reconstructed that session from the current native files.
    let mut stmt = tx.prepare(
        "SELECT event_id, source_record_key, session_id, occurred_at_ms FROM usage_events
         WHERE source_instance_id=?1 AND occurred_at_ms>=?2 AND occurred_at_ms<?3 AND conflict=0
           AND (time_basis!='bounded_interval' OR (interval_start_ms>=?2 AND interval_end_ms<?3))
           AND NOT EXISTS (SELECT 1 FROM event_aliases
             WHERE canonical_event_id=event_id OR member_event_id=event_id)",
    )?;
    let obsolete = stmt
        .query_map(params![instance, start, end], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, key, session, at) in obsolete {
        if session.is_some_and(|session| covered_sessions.contains(&session))
            && !replayed_keys.contains(&key)
        {
            tx.execute("DELETE FROM usage_events WHERE event_id=?1", [id])?;
            times.insert(at);
            outcome.removed_events += 1;
        }
    }
    Ok(times)
}

fn replace_aggregates(
    tx: &Transaction<'_>,
    instance: &str,
    start: i64,
    end: i64,
    now_ms: i64,
    aggregates: Vec<crate::aggregates::SourceAggregateInput>,
) -> Result<usize, CoreError> {
    let mut changed = 0;
    let mut lookup = tx.prepare(
        "SELECT aggregate_id, content_hash, interval_start_ms, interval_end_ms FROM source_aggregates
         WHERE instance_id=?1 AND scope=?2 AND scope_key=?3",
    )?;
    for aggregate in aggregates {
        crate::adapters::run_policy::check()?;
        if aggregate.instance_id != instance {
            return Err(CoreError::Validation(
                "unexpected Zed aggregate ownership".into(),
            ));
        }
        if !in_window(
            aggregate.interval_start_ms,
            Some(aggregate.interval_end_ms),
            start,
            end,
        ) {
            continue;
        }
        let old = lookup
            .query_row(
                params![instance, aggregate.scope.as_str(), aggregate.scope_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        if let Some((id, hash, from, to)) = old {
            if hash == content_hash(&aggregate) || !in_window(from, Some(to), start, end) {
                continue;
            }
            tx.execute("DELETE FROM source_aggregates WHERE aggregate_id=?1", [id])?;
        }
        changed += usize::from(crate::aggregates::upsert_source_aggregate_tx(
            tx, &aggregate, now_ms,
        )?);
    }
    Ok(changed)
}

fn in_window(from: Option<i64>, to: Option<i64>, start: i64, end: i64) -> bool {
    from.zip(to)
        .is_some_and(|(from, to)| start <= from && from <= to && to < end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hour_boundaries_respect_the_statistics_timezone() {
        let hours = |day: &str, start, end| RecollectRange::Hours {
            start: RecollectHour {
                day: day.into(),
                hour: start,
            },
            end: RecollectHour {
                day: day.into(),
                hour: end,
            },
        };
        let tz = "America/New_York";
        let now = |stamp: &str| stamp.parse::<jiff::Timestamp>().unwrap().as_millisecond();
        let spring = now("2026-03-08T20:00:00Z");
        let fall = now("2026-11-01T20:00:00Z");
        let gap = hours("2026-03-08", 1, 3).window(tz, spring, 7).unwrap();
        assert_eq!(gap.end_ms - gap.start_ms, 3_600_000);
        assert!(hours("2026-03-08", 2, 3).window(tz, spring, 7).is_err());
        let fold = hours("2026-11-01", 1, 2).window(tz, fall, 7).unwrap();
        assert_eq!(fold.start_ms, now("2026-11-01T05:00:00Z"));
        assert_eq!(fold.end_ms - fold.start_ms, 7_200_000);
        assert!(hours("2026-10-31", 0, 1).window(tz, fall, 1).is_err());
        assert!(hours("2026-11-01", 20, 21).window(tz, fall, 7).is_err());
        assert!(hours("2026-11-01", 6, 6).window(tz, fall, 7).is_err());
        assert!(hours("2026-11-01", 6, 24).window(tz, fall, 7).is_err());
        let day = RecollectRange::RecentDays { days: 1 }
            .window(tz, fall, 7)
            .unwrap();
        assert_eq!(day.start, "2026-11-01 00:00");
        assert_eq!(day.end, "2026-11-02 00:00");
        assert_eq!(day.end_ms - day.start_ms, 25 * 3_600_000);
    }
}
