//! Optional schema-11 query caches, maintained within transactions.
use crate::{calendar::Calendar, error::CoreError};
use jiff::civil::Date;
use rusqlite::{params, Connection, Transaction};

const METRICS: &[&str] = &[
    "event_count",
    "call_count",
    "attempt_count",
    "observation_count",
    "input_known_sum",
    "input_known_count",
    "input_unknown_count",
    "uncached_known_sum",
    "uncached_known_count",
    "cache_read_known_sum",
    "cache_read_known_count",
    "cache_write_known_sum",
    "cache_write_known_count",
    "output_known_sum",
    "output_known_count",
    "output_unknown_count",
    "total_known_sum",
    "total_known_count",
    "total_unknown_count",
    "ratio_input_sum",
    "ratio_cache_read_sum",
    "ratio_sample_count",
    "conflict_count",
];
const LAYOUT_VERSION: i64 = 1;

pub(crate) fn install(conn: &Connection) -> Result<(), CoreError> {
    let installation = conn.unchecked_transaction()?;
    let conn = &*installation;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS query_accel_timezones(tz_version TEXT PRIMARY KEY);
        CREATE TABLE IF NOT EXISTS query_accel_meta(layout_version INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS query_accel_days(tz_version TEXT NOT NULL,local_day TEXT NOT NULL,start_ms INTEGER NOT NULL,end_ms INTEGER NOT NULL,valid INTEGER NOT NULL,PRIMARY KEY(tz_version,local_day));
        CREATE UNIQUE INDEX IF NOT EXISTS query_accel_start ON query_accel_days(tz_version,start_ms);
        CREATE TABLE IF NOT EXISTS query_session_days(tz_version TEXT NOT NULL,local_day TEXT NOT NULL,instance_id TEXT NOT NULL,session_id TEXT NOT NULL,duration_sum INTEGER NOT NULL,duration_count INTEGER NOT NULL,PRIMARY KEY(tz_version,local_day,instance_id,session_id));")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS query_provider_session_days(tz_version TEXT NOT NULL,local_day TEXT NOT NULL,instance_id TEXT NOT NULL,provider_id TEXT NOT NULL,session_id TEXT NOT NULL,duration_sum INTEGER NOT NULL,duration_count INTEGER NOT NULL,PRIMARY KEY(tz_version,local_day,instance_id,provider_id,session_id));")?;
    for axis in ["day", "model", "agent", "provider_day", "provider_agent"] {
        conn.execute_batch(&format!("CREATE TABLE IF NOT EXISTS query_rollup_{axis} AS SELECT * FROM daily_usage WHERE 0;
            CREATE INDEX IF NOT EXISTS query_rollup_{axis}_range ON query_rollup_{axis}(tz_version,local_day);"))?;
    }
    // Invalidation removes cached identities too: retention and older writers
    // must not leave session identifiers in an unusable cache.
    conn.execute_batch("DROP TRIGGER IF EXISTS query_accel_discard;
        CREATE TRIGGER query_accel_discard AFTER UPDATE OF valid ON query_accel_days WHEN OLD.valid=1 AND NEW.valid=0 BEGIN
        DELETE FROM query_rollup_day WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        DELETE FROM query_rollup_model WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        DELETE FROM query_rollup_agent WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        DELETE FROM query_rollup_provider_day WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        DELETE FROM query_rollup_provider_agent WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        DELETE FROM query_session_days WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        DELETE FROM query_provider_session_days WHERE tz_version=NEW.tz_version AND local_day=NEW.local_day;
        END;")?;
    for (operation, references) in [
        ("INSERT", vec!["NEW"]),
        ("DELETE", vec!["OLD"]),
        ("UPDATE", vec!["OLD", "NEW"]),
    ] {
        let daily = references.iter().map(|r| format!("UPDATE query_accel_days SET valid=0 WHERE tz_version={r}.tz_version AND local_day={r}.local_day AND valid=1;")).collect::<String>();
        let events = references.iter().map(|r| format!("UPDATE query_accel_days SET valid=0 WHERE valid=1 AND end_ms>{r}.occurred_at_ms AND (tz_version,start_ms) IN
            (SELECT t.tz_version,(SELECT d.start_ms FROM query_accel_days d WHERE d.tz_version=t.tz_version AND d.start_ms<={r}.occurred_at_ms ORDER BY d.start_ms DESC LIMIT 1) FROM query_accel_timezones t);")).collect::<String>();
        conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS query_accel_daily_{operation} AFTER {operation} ON daily_usage BEGIN {daily} END;
            CREATE TRIGGER IF NOT EXISTS query_accel_event_{operation} AFTER {operation} ON usage_events BEGIN {events} END;"))?;
    }
    let current: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM query_accel_meta WHERE layout_version=?1)",
        [LAYOUT_VERSION],
        |r| r.get(0),
    )?;
    if !current {
        conn.execute("UPDATE query_accel_days SET valid=0", [])?;
        conn.execute("DELETE FROM query_accel_meta", [])?;
        conn.execute("INSERT INTO query_accel_meta VALUES(?1)", [LAYOUT_VERSION])?;
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_daily_model_query_v1 ON daily_usage(tz_version,fold_name(model_key(model_raw)),local_day);
        CREATE INDEX IF NOT EXISTS idx_daily_agent_query_v1 ON daily_usage(tz_version,fold_name(agent),local_day);
        CREATE INDEX IF NOT EXISTS idx_daily_provider_query_v1 ON daily_usage(tz_version,fold_name(provider_id),local_day);
        CREATE INDEX IF NOT EXISTS idx_events_model_query_v1 ON usage_events(fold_name(model_key(model_raw)),occurred_at_ms,source_instance_id,session_id,duration_ms,record_kind,agent,provider_id) WHERE attribution_status='verified' AND record_kind IN ('model_call','transport_attempt','usage_observation');
        CREATE INDEX IF NOT EXISTS idx_events_agent_query_v1 ON usage_events(fold_name(agent),occurred_at_ms,source_instance_id,session_id,duration_ms,record_kind,model_raw,provider_id) WHERE attribution_status='verified' AND record_kind IN ('model_call','transport_attempt','usage_observation');")?;
    // Include every table and retain bounded SQLite analysis so
    // combined filters can choose the more selective expression index.
    conn.execute_batch("PRAGMA optimize=0x10012")?;
    installation.commit()?;
    Ok(())
}

pub(crate) fn rebuild_day(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
) -> Result<(), CoreError> {
    let present: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='query_accel_days')",
        [],
        |r| r.get(0),
    )?;
    if !present {
        return Ok(());
    }
    tx.execute_batch("SAVEPOINT query_projection")?;
    match build_day(tx, calendar, day) {
        Ok(()) => tx.execute_batch("RELEASE query_projection")?,
        Err(error) => {
            tx.execute_batch("ROLLBACK TO query_projection; RELEASE query_projection")?;
            // Optional caching must not reject valid events from separate groups
            // when only the combined metric exceeds i64.
            if matches!(&error, CoreError::Sqlite(rusqlite::Error::SqliteFailure(_, Some(message))) if message == "integer overflow")
            {
                tx.execute(
                    "UPDATE query_accel_days SET valid=0 WHERE tz_version=?1 AND local_day=?2",
                    params![calendar.tz_name(), day.to_string()],
                )?;
            } else {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn build_day(tx: &Transaction<'_>, calendar: &Calendar, day: Date) -> Result<(), CoreError> {
    let tz = calendar.tz_name();
    let date = day.to_string();
    let (start, end) = calendar.day_range_ms(day)?;
    tx.execute("INSERT INTO query_accel_meta SELECT ?1 WHERE NOT EXISTS(SELECT 1 FROM query_accel_meta WHERE layout_version=?1)",[LAYOUT_VERSION])?;
    tx.execute(
        "INSERT OR IGNORE INTO query_accel_timezones VALUES(?1)",
        [tz],
    )?;
    for axis in ["day", "model", "agent", "provider_day", "provider_agent"] {
        tx.execute(
            &format!("DELETE FROM query_rollup_{axis} WHERE tz_version=?1 AND local_day=?2"),
            params![tz, date],
        )?;
        let (agent, provider, model) = match axis {
            "agent" => ("fold_name(agent)", "''", "''"),
            "model" => ("''", "fold_name(provider_id)", "model_key(model_raw)"),
            "provider_day" => ("''", "fold_name(provider_id)", "''"),
            "provider_agent" => ("fold_name(agent)", "fold_name(provider_id)", "''"),
            _ => ("''", "''", "''"),
        };
        let columns = METRICS.join(",");
        let sums = METRICS
            .iter()
            .map(|m| format!("SUM({m})"))
            .collect::<Vec<_>>()
            .join(",");
        tx.execute(&format!("INSERT INTO query_rollup_{axis}(tz_version,local_day,instance_id,agent,provider_id,model_raw,call_category,quality_bucket,{columns},sealed,data_revision)
            SELECT ?1,?2,instance_id,{agent},{provider},{model},'','',{sums},MAX(sealed),MAX(data_revision)
            FROM daily_usage WHERE tz_version=?1 AND local_day=?2 GROUP BY instance_id,{agent},{provider},{model}"),params![tz,date])?;
    }
    tx.execute(
        "DELETE FROM query_session_days WHERE tz_version=?1 AND local_day=?2",
        params![tz, date],
    )?;
    tx.execute("INSERT INTO query_session_days SELECT ?1,?2,source_instance_id,COALESCE(session_id,''),
        SUM(CASE WHEN record_kind='model_call' AND duration_ms>=0 THEN duration_ms ELSE 0 END),
        COUNT(CASE WHEN record_kind='model_call' AND duration_ms>=0 THEN 1 END)
        FROM usage_events WHERE occurred_at_ms>=?3 AND occurred_at_ms<?4 AND attribution_status='verified'
        AND record_kind IN ('model_call','transport_attempt','usage_observation')
        GROUP BY source_instance_id,COALESCE(session_id,'')",params![tz,date,start,end])?;
    tx.execute(
        "DELETE FROM query_provider_session_days WHERE tz_version=?1 AND local_day=?2",
        params![tz, date],
    )?;
    tx.execute("INSERT INTO query_provider_session_days SELECT ?1,?2,source_instance_id,COALESCE(fold_name(provider_id),''),COALESCE(session_id,''),
        SUM(CASE WHEN record_kind='model_call' AND duration_ms>=0 THEN duration_ms ELSE 0 END),
        COUNT(CASE WHEN record_kind='model_call' AND duration_ms>=0 THEN 1 END)
        FROM usage_events WHERE occurred_at_ms>=?3 AND occurred_at_ms<?4 AND attribution_status='verified'
        AND record_kind IN ('model_call','transport_attempt','usage_observation')
        GROUP BY source_instance_id,COALESCE(fold_name(provider_id),''),COALESCE(session_id,'')",params![tz,date,start,end])?;
    tx.execute("INSERT INTO query_accel_days VALUES(?1,?2,?3,?4,1) ON CONFLICT(tz_version,local_day) DO UPDATE SET start_ms=excluded.start_ms,end_ms=excluded.end_ms,valid=1",params![tz,date,start,end])?;
    Ok(())
}

pub(crate) fn repair(conn: &Connection) -> Result<(), CoreError> {
    let ranges = conn
        .prepare(
            "SELECT tz_version,MIN(local_day),MAX(local_day) FROM daily_usage GROUP BY tz_version",
        )?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (tz, first, last) in ranges {
        let calendar = Calendar::new(&tz)?;
        let mut day = crate::calendar::parse_date(&first)?;
        let last = crate::calendar::parse_date(&last)?;
        // Outside this window, long retained histories use the original query.
        day = day.max(last.checked_sub(jiff::Span::new().days(749))?);
        let tx = conn.unchecked_transaction()?;
        while day <= last {
            let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM query_accel_days WHERE tz_version=?1 AND local_day=?2 AND valid=1)",params![tz,day.to_string()],|r| r.get(0))?;
            if !valid {
                rebuild_day(&tx, &calendar, day)?;
            }
            day = day.checked_add(jiff::Span::new().days(1))?;
        }
        tx.commit()?;
    }
    Ok(())
}

pub(crate) fn usable(
    conn: &Connection,
    request: &crate::query::SummaryRequest,
) -> Result<bool, CoreError> {
    if request.granularity != crate::query::Granularity::Day
        || !request.filters.agents.is_empty()
        || !request.filters.models.is_empty()
        || !request.filters.quality_buckets.is_empty()
        || request.filters.providers.iter().any(String::is_empty)
    {
        return Ok(false);
    }
    let present: bool = conn.query_row(
        "SELECT COUNT(*)=2 FROM sqlite_master WHERE name IN ('query_accel_days','query_accel_meta') AND type='table'",
        [],
        |r| r.get(0),
    )?;
    if !present {
        return Ok(false);
    }
    let current: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM query_accel_meta WHERE layout_version=?1)",
        [LAYOUT_VERSION],
        |r| r.get(0),
    )?;
    if !current {
        return Ok(false);
    }
    let expected = request.first_day.until(request.last_day)?.get_days() + 1;
    let (count,invalid):(i64,i64)=conn.query_row("SELECT COUNT(*),COALESCE(SUM(valid=0),0) FROM query_accel_days WHERE tz_version=?1 AND local_day>=?2 AND local_day<=?3",params![request.timezone,request.first_day.to_string(),request.last_day.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if invalid > 0 {
        return Ok(false);
    }
    if count == i64::from(expected) {
        return Ok(true);
    }
    // Today, days before the first observation and calendar gaps need no
    // stored zero rows. Check absent partitions against daily_usage and usage_events
    // so an older event-only writer cannot make a stale cache appear valid.
    let days=conn.prepare("SELECT local_day FROM query_accel_days WHERE tz_version=?1 AND local_day>=?2 AND local_day<=?3 AND valid=1")?.query_map(params![request.timezone,request.first_day.to_string(),request.last_day.to_string()],|r|r.get::<_,String>(0))?.collect::<Result<std::collections::BTreeSet<_>,_>>()?;
    let calendar = Calendar::new(&request.timezone)?;
    let mut day = request.first_day;
    while day <= request.last_day {
        let date = day.to_string();
        if !days.contains(&date) {
            let (start, end) = calendar.day_range_ms(day)?;
            let nonempty:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM daily_usage WHERE tz_version=?1 AND local_day=?2) OR EXISTS(SELECT 1 FROM usage_events WHERE occurred_at_ms>=?3 AND occurred_at_ms<?4)",params![request.timezone,date,start,end],|r|r.get(0))?;
            if nonempty {
                return Ok(false);
            }
        }
        day = day.checked_add(jiff::Span::new().days(1))?;
    }
    Ok(true)
}
