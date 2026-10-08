//! Agent-independent quota history.
//!
//! Normalize account/rate-limit/remaining-quota observations into QuotaObservation
//! and quota_history. These observations remain separate from recorded token usage;
//! display independently and never convert requests/credits into usage tokens (data-contract).
//! Copilot premium requests are the first integration. Other verified local quota formats
//! can reuse this table/query interface; examples do not establish Warp or other-client support.

use crate::calendar::Calendar;
use crate::error::CoreError;
use crate::storage::Storage;
use rusqlite::OptionalExtension;

/// One normalized quota observation, independent of Agent.
#[derive(Debug, Clone, PartialEq)]
pub struct QuotaObservation {
    /// Source snapshot time; use collection time only when absent.
    pub observed_at_ms: Option<i64>,
    /// Statistics Agent name, such as "copilot".
    pub agent: String,
    /// Quota identifier, such as "premium_interactions".
    pub quota_id: String,
    /// Quota kind: rate_limit / credits / balance / subscription_window.
    pub kind: String,
    /// Source unit: requests / credits / tokens / usd_minor, among others.
    pub unit: String,
    /// Period limit; None means unlimited or unknown.
    pub limit_value: Option<i64>,
    /// Used amount; None is unknown, without substituting zero.
    pub used: Option<i64>,
    /// Remaining amount; None is unknown.
    pub remaining: Option<i64>,
    pub percent_remaining: Option<f64>,
    pub window_start_ms: Option<i64>,
    pub window_end_ms: Option<i64>,
    /// True only for verified local usage; account-wide cross-device totals use false.
    pub locality_verified: bool,
    pub detail: Option<serde_json::Value>,
}

/// Latest quota row per (agent, quota_id), for the overview.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QuotaLatest {
    pub agent: String,
    pub quota_id: String,
    pub kind: String,
    pub unit: String,
    pub limit_value: Option<i64>,
    pub used: Option<i64>,
    pub remaining: Option<i64>,
    pub percent_remaining: Option<f64>,
    pub locality_verified: bool,
    pub observed_at_ms: i64,
}

/// Daily trend point: last observation on each local day.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QuotaDayPoint {
    pub local_day: String,
    pub used: Option<i64>,
    pub remaining: Option<i64>,
    pub limit_value: Option<i64>,
}

/// Skip identical snapshot times/fields; atomically replace corrections at the same time.
/// Use source time for local-day grouping; equal values at a new time remain a new observation.
pub fn record(
    storage: &Storage,
    observations: &[QuotaObservation],
    timezone: &str,
    now_ms: i64,
) -> Result<u64, CoreError> {
    if observations.is_empty() {
        return Ok(0);
    }
    let calendar = Calendar::new(timezone)?;
    let tx = storage.conn().unchecked_transaction()?;
    let floor: Option<i64> = tx
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM settings WHERE key='hard_retention_floor_ms'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let mut inserted = 0u64;
    for obs in observations {
        let observed_at_ms = obs.observed_at_ms.unwrap_or(now_ms);
        if floor.is_some_and(|floor| observed_at_ms < floor) {
            continue;
        }
        if obs.agent.trim().is_empty()
            || obs.quota_id.trim().is_empty()
            || [obs.limit_value, obs.used, obs.remaining]
                .into_iter()
                .flatten()
                .any(|v| v < 0)
            || obs
                .percent_remaining
                .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
        {
            return Err(CoreError::Validation("invalid quota observation".into()));
        }
        let local_day = calendar.local_day_of(observed_at_ms)?.to_string();
        let detail = obs.detail.as_ref().map(serde_json::to_string).transpose()?;
        let unchanged: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM quota_history WHERE agent=?1 AND quota_id=?2
                 AND observed_at_ms=?3 AND kind=?4 AND unit=?5 AND limit_value IS ?6
                 AND used IS ?7 AND remaining IS ?8 AND percent_remaining IS ?9
                 AND window_start_ms IS ?10 AND window_end_ms IS ?11
                 AND locality_verified=?12 AND detail_json IS ?13)",
            rusqlite::params![
                obs.agent,
                obs.quota_id,
                observed_at_ms,
                obs.kind,
                obs.unit,
                obs.limit_value,
                obs.used,
                obs.remaining,
                obs.percent_remaining,
                obs.window_start_ms,
                obs.window_end_ms,
                obs.locality_verified as i64,
                detail
            ],
            |r| r.get(0),
        )?;
        if unchanged {
            continue;
        }
        inserted += tx.execute(
            "INSERT INTO quota_history
               (agent, quota_id, observed_at_ms, local_day, kind, unit, limit_value,
                used, remaining, percent_remaining, window_start_ms, window_end_ms,
                locality_verified, detail_json, created_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
             ON CONFLICT(agent, quota_id, observed_at_ms) DO UPDATE SET
               local_day=excluded.local_day, kind=excluded.kind, unit=excluded.unit,
               limit_value=excluded.limit_value, used=excluded.used,
               remaining=excluded.remaining, percent_remaining=excluded.percent_remaining,
               window_start_ms=excluded.window_start_ms, window_end_ms=excluded.window_end_ms,
               locality_verified=excluded.locality_verified, detail_json=excluded.detail_json",
            rusqlite::params![
                obs.agent,
                obs.quota_id,
                observed_at_ms,
                local_day,
                obs.kind,
                obs.unit,
                obs.limit_value,
                obs.used,
                obs.remaining,
                obs.percent_remaining,
                obs.window_start_ms,
                obs.window_end_ms,
                obs.locality_verified as i64,
                detail,
                now_ms
            ],
        )? as u64;
    }
    if inserted > 0 {
        Storage::bump_data_revision_tx(&tx, now_ms)?;
    }
    tx.commit()?;
    Ok(inserted)
}

/// Latest row per (agent, quota_id); agent=None selects all Agents.
pub fn latest(storage: &Storage, agent: Option<&str>) -> Result<Vec<QuotaLatest>, CoreError> {
    let mut sql = String::from(
        "SELECT h.agent, h.quota_id, h.kind, h.unit, h.limit_value, h.used, h.remaining,
                h.percent_remaining, h.locality_verified, h.observed_at_ms
         FROM quota_history h
         JOIN (SELECT agent, quota_id, MAX(observed_at_ms) m FROM quota_history
               GROUP BY agent, quota_id) x
           ON x.agent=h.agent AND x.quota_id=h.quota_id AND x.m=h.observed_at_ms",
    );
    if agent.is_some() {
        sql.push_str(" WHERE h.agent=?1");
    }
    sql.push_str(" ORDER BY h.agent, h.quota_id");
    let mut stmt = storage.conn().prepare(&sql)?;
    let map = |r: &rusqlite::Row<'_>| {
        Ok(QuotaLatest {
            agent: r.get(0)?,
            quota_id: r.get(1)?,
            kind: r.get(2)?,
            unit: r.get(3)?,
            limit_value: r.get(4)?,
            used: r.get(5)?,
            remaining: r.get(6)?,
            percent_remaining: r.get(7)?,
            locality_verified: r.get::<_, i64>(8)? != 0,
            observed_at_ms: r.get(9)?,
        })
    };
    let rows: Result<Vec<QuotaLatest>, rusqlite::Error> = match agent {
        Some(a) => stmt.query_map(rusqlite::params![a], map)?.collect(),
        None => stmt.query_map([], map)?.collect(),
    };
    Ok(rows?)
}

/// Daily trend for (agent, quota_id), selecting the last observation of each local day.
pub fn daily_series(
    storage: &Storage,
    agent: &str,
    quota_id: &str,
    timezone: &str,
) -> Result<Vec<QuotaDayPoint>, CoreError> {
    let calendar = Calendar::new(timezone)?;
    let mut stmt = storage.conn().prepare(
        "SELECT observed_at_ms, used, remaining, limit_value FROM quota_history
         WHERE agent=?1 AND quota_id=?2 ORDER BY observed_at_ms",
    )?;
    let rows = stmt.query_map(rusqlite::params![agent, quota_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
        ))
    })?;
    let mut days = std::collections::BTreeMap::new();
    for row in rows {
        let (time, used, remaining, limit_value) = row?;
        let day = calendar.local_day_of(time)?.to_string();
        days.insert(
            day.clone(),
            QuotaDayPoint {
                local_day: day,
                used,
                remaining,
                limit_value,
            },
        );
    }
    Ok(days.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(agent: &str, used: i64, remaining: i64) -> QuotaObservation {
        QuotaObservation {
            observed_at_ms: Some(1000),
            agent: agent.to_string(),
            quota_id: "premium_interactions".to_string(),
            kind: "rate_limit".to_string(),
            unit: "requests".to_string(),
            limit_value: Some(1500),
            used: Some(used),
            remaining: Some(remaining),
            percent_remaining: Some(remaining as f64 / 15.0),
            window_start_ms: None,
            window_end_ms: None,
            locality_verified: false,
            detail: None,
        }
    }

    fn temp_storage() -> (std::path::PathBuf, Storage) {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-quota-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Storage::open(&dir.join("q.sqlite")).unwrap();
        (dir, storage)
    }

    #[test]
    fn record_dedups_unchanged_and_tracks_history() {
        let (_dir, storage) = temp_storage();
        assert_eq!(
            record(&storage, &[obs("copilot", 1300, 200)], "UTC", 1000).unwrap(),
            1
        );
        // Identical source time and values: skip.
        assert_eq!(
            record(&storage, &[obs("copilot", 1300, 200)], "UTC", 2000).unwrap(),
            0
        );
        // Changed values at the same source time: record the correction.
        assert_eq!(
            record(&storage, &[obs("copilot", 1363, 137)], "UTC", 3000).unwrap(),
            1
        );
        let latest = latest(&storage, Some("copilot")).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].used, Some(1363));
        assert_eq!(latest[0].remaining, Some(137));
        assert!(!latest[0].locality_verified);
    }

    #[test]
    fn latest_filters_by_agent() {
        let (_dir, storage) = temp_storage();
        record(&storage, &[obs("copilot", 10, 1490)], "UTC", 1000).unwrap();
        record(&storage, &[obs("other", 5, 1495)], "UTC", 1000).unwrap();
        assert_eq!(latest(&storage, Some("copilot")).unwrap().len(), 1);
        assert_eq!(latest(&storage, None).unwrap().len(), 2);
    }

    #[test]
    fn records_new_time_same_values_and_same_time_metadata_corrections() {
        let (_dir, storage) = temp_storage();
        let mut q = obs("copilot", 10, 1490);
        assert_eq!(
            record(&storage, std::slice::from_ref(&q), "UTC", 1000).unwrap(),
            1
        );
        let revision = storage.data_revision().unwrap();
        assert_eq!(
            record(&storage, std::slice::from_ref(&q), "UTC", 2000).unwrap(),
            0
        );
        assert_eq!(storage.data_revision().unwrap(), revision);
        q.limit_value = Some(1600);
        q.percent_remaining = Some(90.0);
        assert_eq!(
            record(&storage, std::slice::from_ref(&q), "UTC", 2000).unwrap(),
            1
        );
        assert_eq!(latest(&storage, None).unwrap()[0].limit_value, Some(1600));
        q.observed_at_ms = Some(86_401_000);
        assert_eq!(record(&storage, &[q], "UTC", 2000).unwrap(), 1);
        let points = daily_series(&storage, "copilot", "premium_interactions", "UTC").unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].local_day, "1970-01-01");
        assert_eq!(points[1].local_day, "1970-01-02");
        let western = daily_series(
            &storage,
            "copilot",
            "premium_interactions",
            "America/Los_Angeles",
        )
        .unwrap();
        assert_eq!(western[0].local_day, "1969-12-31");
        assert_eq!(western[1].local_day, "1970-01-01");
    }
}
