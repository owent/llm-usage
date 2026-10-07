//! Aggregate exchange: validate first, then merge in one transaction.
//! Equal revisions retain the selected overwrite contract; identical replay is a no-op.
//! Imported days are sealed because request details are not restored.
use crate::error::CoreError;
use crate::exchange::{ExchangeExport, ExchangeHost, EXCHANGE_FORMAT_VERSION};
use crate::storage::Storage;
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImportAggregateOutcome {
    pub sources_registered: usize,
    pub daily_inserted: usize,
    pub daily_replaced: usize,
    pub daily_skipped: usize,
    pub daily_conflicts: usize,
    pub hourly_inserted: usize,
    pub hourly_replaced: usize,
    pub hourly_skipped: usize,
    pub period_inserted: usize,
    pub period_replaced: usize,
    pub period_skipped: usize,
}

pub fn import_aggregate(
    storage: &Storage,
    export: &ExchangeExport,
    now_ms: i64,
) -> Result<ImportAggregateOutcome, CoreError> {
    let tx = storage.conn().unchecked_transaction()?;
    let outcome = import_aggregate_tx(storage, &tx, export, now_ms)?;
    tx.commit()?;
    Ok(outcome)
}

pub(crate) fn import_aggregate_tx(
    storage: &Storage,
    tx: &Transaction<'_>,
    export: &ExchangeExport,
    now_ms: i64,
) -> Result<ImportAggregateOutcome, CoreError> {
    if export.format_version != EXCHANGE_FORMAT_VERSION {
        return Err(CoreError::Validation("unsupported exchange format".into()));
    }
    if matches!(&export.kind, crate::exchange::ExchangeKind::Incremental { deletions, .. } if !deletions.is_empty())
    {
        return Err(CoreError::Validation(
            "record deletion imports are not supported".into(),
        ));
    }
    if !export.records.is_empty()
        && export.daily_partitions.is_empty()
        && export.hourly_partitions.is_empty()
        && export.period_partitions.is_empty()
    {
        return Err(CoreError::Validation("this package contains only request details; aggregate import requires aggregate partitions".into()));
    }
    crate::calendar::Calendar::new(&export.timezone)?;
    if export.host.origin_host_id.is_empty() {
        return Err(CoreError::Validation("missing origin host".into()));
    }
    let declared: BTreeMap<_, _> = export
        .sources
        .iter()
        .map(|s| (s.source_instance_id.as_str(), s))
        .collect();
    if declared.len() != export.sources.len() {
        return Err(CoreError::Validation(
            "duplicate source declarations".into(),
        ));
    }
    let mut out = ImportAggregateOutcome::default();
    register_host(tx, &export.host, now_ms)?;
    for source in &export.sources {
        if source.source_instance_id.is_empty() || source.attribution_status != "verified" {
            return Err(CoreError::Validation(
                "aggregate source attribution is unverified".into(),
            ));
        }
        let host = source
            .origin_host_id
            .as_deref()
            .unwrap_or(&export.host.origin_host_id);
        if host.is_empty() {
            return Err(CoreError::Validation("missing source origin host".into()));
        }
        register_host(
            tx,
            &ExchangeHost {
                origin_host_id: host.into(),
                hostname_alias: None,
            },
            now_ms,
        )?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT origin_host_id FROM source_instances WHERE instance_id=?1",
                [&source.source_instance_id],
                |r| r.get(0),
            )
            .optional()?;
        if existing
            .as_deref()
            .is_some_and(|id| id != host && id != "legacy_unknown")
        {
            return Err(CoreError::Validation(
                "source identity belongs to a different host; cannot merge by path".into(),
            ));
        }
        out.sources_registered += tx.execute(
            "INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,format,parser_version,capabilities,health,origin_host_id,user_id,created_at_ms,updated_at_ms)
             VALUES (?1,?2,'remote_sync','verified',0,?3,?4,'{}','ok',?5,'default',?6,?6) ON CONFLICT(instance_id) DO NOTHING",
            params![source.source_instance_id,source.agent,source.format,source.parser_version,host,now_ms])?;
    }
    let mut max_revision = storage.data_revision()?;
    for p in &export.daily_partitions {
        let mut row = object(p)?;
        row.remove("statistics");
        let statistics = object(&p.statistics.clone().unwrap_or_default())?;
        row.extend(statistics);
        row.insert("sealed".into(), serde_json::json!(1));
        validate_partition(&row, &declared)?;
        max_revision = max_revision.max(p.data_revision);
        match merge_partition(tx, "daily_usage", DAILY_KEYS, &row)? {
            Merge::Inserted => out.daily_inserted += 1,
            Merge::Replaced => out.daily_replaced += 1,
            Merge::Unchanged => out.daily_skipped += 1,
            Merge::Older => {
                out.daily_conflicts += 1;
                older_diagnostic(tx, now_ms)?;
            }
        }
    }
    for p in &export.hourly_partitions {
        let row = object(p)?;
        validate_partition(&row, &declared)?;
        if !(0..24).contains(&p.hour) {
            return Err(CoreError::Validation("invalid aggregate hour".into()));
        }
        max_revision = max_revision.max(p.data_revision);
        match merge_partition(tx, "hourly_usage", HOURLY_KEYS, &row)? {
            Merge::Inserted => out.hourly_inserted += 1,
            Merge::Replaced => out.hourly_replaced += 1,
            Merge::Unchanged => out.hourly_skipped += 1,
            Merge::Older => {
                out.hourly_skipped += 1;
                older_diagnostic(tx, now_ms)?;
            }
        }
    }
    for p in &export.period_partitions {
        let row = object(p)?;
        validate_partition(&row, &declared)?;
        if !["week", "month", "year"].contains(&p.granularity.as_str())
            || p.period_key.is_empty()
            || p.period_start_day > p.period_end_day
        {
            return Err(CoreError::Validation("invalid aggregate period".into()));
        }
        crate::calendar::parse_date(&p.period_start_day)?;
        crate::calendar::parse_date(&p.period_end_day)?;
        max_revision = max_revision.max(p.data_revision);
        match merge_partition(tx, "period_usage", PERIOD_KEYS, &row)? {
            Merge::Inserted => out.period_inserted += 1,
            Merge::Replaced => out.period_replaced += 1,
            Merge::Unchanged => out.period_skipped += 1,
            Merge::Older => {
                out.period_skipped += 1;
                older_diagnostic(tx, now_ms)?;
            }
        }
    }
    let changed = out.daily_inserted
        + out.daily_replaced
        + out.hourly_inserted
        + out.hourly_replaced
        + out.period_inserted
        + out.period_replaced;
    if changed > 0 {
        // Keep local materialization revisions newer than imported partition revisions.
        tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES ('data_revision',?1,1,?2)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
            params![max_revision.to_string(),now_ms])?;
        Storage::bump_data_revision_tx(tx, now_ms)?;
        tx.execute(
            "INSERT INTO diagnostics(code,message,created_ms) VALUES ('import_completed',?1,?2)",
            params![format!("aggregate partitions changed: {changed}"), now_ms],
        )?;
    }
    Ok(out)
}

type Object = serde_json::Map<String, serde_json::Value>;
fn object(value: &impl serde::Serialize) -> Result<Object, CoreError> {
    serde_json::to_value(value)?
        .as_object()
        .cloned()
        .ok_or_else(|| CoreError::Validation("expected aggregate object".into()))
}
const DAILY_KEYS: &[&str] = &[
    "tz_version",
    "local_day",
    "instance_id",
    "agent",
    "provider_id",
    "model_raw",
    "call_category",
    "quality_bucket",
];
const HOURLY_KEYS: &[&str] = &[
    "tz_version",
    "local_day",
    "hour",
    "instance_id",
    "agent",
    "provider_id",
    "model_raw",
    "call_category",
    "quality_bucket",
];
const PERIOD_KEYS: &[&str] = &[
    "tz_version",
    "granularity",
    "period_key",
    "instance_id",
    "agent",
    "provider_id",
    "model_raw",
    "call_category",
    "quality_bucket",
];

fn validate_partition(
    row: &Object,
    sources: &BTreeMap<&str, &crate::exchange::ExchangeSource>,
) -> Result<(), CoreError> {
    let text = |field: &str| row.get(field).and_then(|v| v.as_str()).unwrap_or("");
    let source = sources
        .get(text("instance_id"))
        .ok_or_else(|| CoreError::Validation("aggregate references an undeclared source".into()))?;
    if source.agent.to_lowercase() != text("agent").to_lowercase() {
        return Err(CoreError::Validation(
            "aggregate agent differs from its source".into(),
        ));
    }
    crate::calendar::Calendar::new(text("tz_version"))?;
    if row.contains_key("local_day") {
        crate::calendar::parse_date(text("local_day"))?;
    }
    for value in row.values() {
        if value.as_i64().is_some_and(|n| n < 0) {
            return Err(CoreError::Validation("negative aggregate value".into()));
        }
    }
    let n = |key: &str| row.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
    if n("call_count") > n("event_count") || n("conflict_count") > n("event_count") {
        return Err(CoreError::Validation(
            "aggregate count exceeds event count".into(),
        ));
    }
    Ok(())
}
enum Merge {
    Inserted,
    Replaced,
    Unchanged,
    Older,
}
fn sql_value(value: &serde_json::Value) -> Result<rusqlite::types::Value, CoreError> {
    use rusqlite::types::Value;
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::String(v) => Ok(Value::Text(v.clone())),
        serde_json::Value::Number(v) => v
            .as_i64()
            .map(Value::Integer)
            .ok_or_else(|| CoreError::Validation("aggregate integer out of range".into())),
        _ => Err(CoreError::Validation("invalid aggregate value".into())),
    }
}

/// Column names come only from typed serializers above, never from input JSON.
fn merge_partition(
    tx: &Transaction<'_>,
    table: &str,
    keys: &[&str],
    row: &Object,
) -> Result<Merge, CoreError> {
    let columns: Vec<_> = row.keys().map(String::as_str).collect();
    let values: Vec<_> = row.values().map(sql_value).collect::<Result<_, _>>()?;
    let predicate = keys
        .iter()
        .enumerate()
        .map(|(i, k)| format!("{k}=?{}", i + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let key_values: Vec<_> = keys
        .iter()
        .map(|k| sql_value(&row[*k]))
        .collect::<Result<_, _>>()?;
    let sql = format!(
        "SELECT {} FROM {table} WHERE {predicate}",
        columns.join(",")
    );
    let existing = tx
        .query_row(&sql, rusqlite::params_from_iter(key_values), |r| {
            (0..columns.len())
                .map(|i| r.get::<_, rusqlite::types::Value>(i))
                .collect::<Result<Vec<_>, _>>()
        })
        .optional()?;
    if let Some(old) = &existing {
        if old == &values {
            return Ok(Merge::Unchanged);
        }
        // Re-importing our own snapshot must not seal a live source/day and
        // prevent its next requests from entering the aggregate.
        if table == "daily_usage" {
            let sealed = columns.iter().position(|k| *k == "sealed").unwrap();
            if old[sealed] == rusqlite::types::Value::Integer(0)
                && old
                    .iter()
                    .zip(&values)
                    .enumerate()
                    .all(|(i, (a, b))| i == sealed || a == b)
            {
                return Ok(Merge::Unchanged);
            }
        }
        let revision = columns.iter().position(|k| *k == "data_revision").unwrap();
        if let (rusqlite::types::Value::Integer(old), rusqlite::types::Value::Integer(new)) =
            (&old[revision], &values[revision])
        {
            if old > new {
                return Ok(Merge::Older);
            }
        }
    }
    if table == "daily_usage" || table == "hourly_usage" {
        let calendar = crate::calendar::Calendar::new(row["tz_version"].as_str().unwrap())?;
        let day = crate::calendar::parse_date(row["local_day"].as_str().unwrap())?;
        let (start, end) = calendar.day_range_ms(day)?;
        let live: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM usage_events WHERE source_instance_id=?1 AND occurred_at_ms>=?2 AND occurred_at_ms<?3)",
            params![row["instance_id"].as_str().unwrap(),start,end], |r| r.get(0))?;
        if live {
            return Err(CoreError::Validation("aggregate import overlaps retained local request details; live partitions cannot be replaced with a snapshot".into()));
        }
    }
    let placeholders = (1..=columns.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let updates = columns
        .iter()
        .filter(|k| !keys.contains(k))
        .map(|k| format!("{k}=excluded.{k}"))
        .collect::<Vec<_>>()
        .join(",");
    tx.execute(&format!("INSERT INTO {table}({}) VALUES ({placeholders}) ON CONFLICT({}) DO UPDATE SET {updates}", columns.join(","),keys.join(",")),
        rusqlite::params_from_iter(values))?;
    Ok(if existing.is_some() {
        Merge::Replaced
    } else {
        Merge::Inserted
    })
}
fn older_diagnostic(tx: &Transaction<'_>, now: i64) -> Result<(), CoreError> {
    tx.execute("INSERT INTO diagnostics(code,message,created_ms) VALUES ('import_revision_older','Imported partition is older; existing values were kept.',?1)", [now])?;
    Ok(())
}

/// 注册导出包的主机（外部来源；本机身份不覆盖）。
fn register_host(tx: &Transaction<'_>, host: &ExchangeHost, now_ms: i64) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO origin_hosts (host_id, is_local, note, first_seen_ms, last_seen_ms)
         VALUES (?1, 0, 'imported exchange package', ?2, ?2)
         ON CONFLICT(host_id) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
        params![host.origin_host_id, now_ms],
    )?;
    if let Some(alias) = &host.hostname_alias {
        tx.execute(
            "INSERT INTO origin_host_names (host_id, hostname, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(host_id, hostname) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
            params![host.origin_host_id, alias, now_ms],
        )?;
    }
    Ok(())
}
