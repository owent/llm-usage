//! Qwen 0.25.0 SDK spans are authoritative for a verified local session/day.
//! Native and duplicate exports stay stored; token/time equality is not identity.
use crate::{calendar::Calendar, error::CoreError, ingest::IngestBatch};
use jiff::civil::Date;
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::BTreeSet;

const KEY: &str = "qwen_sdk_scopes_v1";
const PARSER: &str = "qwen-sdk-file-0.25.0-1";
type Scope = (String, String, String, String, String);

pub(crate) fn select(
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    calendar: &Calendar,
    affected: &mut BTreeSet<Date>,
) -> Result<(), CoreError> {
    if !batch.events.iter().any(|e| e.agent == "qwen-code") {
        return Ok(());
    }
    let owner: Option<(String, String)> = tx
        .query_row(
            "SELECT origin_host_id,user_id FROM source_instances WHERE instance_id=?1",
            [&batch.instance_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let owner = owner.filter(|(h, u)| !h.is_empty() && h != "legacy_unknown" && !u.is_empty());
    // Missing provenance cannot safely replace or add to a native partition.
    if owner.is_none() {
        for e in batch.events.iter().filter(|e| e.parser_version == PARSER) {
            let changed=tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='qwen_sdk_owner_unverified' WHERE source_instance_id=?1 AND source_record_key=?2 AND attribution_status='verified'",params![e.source_instance_id,e.source_record_key])?;
            if changed > 0 {
                affected.insert(calendar.local_day_of(e.occurred_at_ms)?);
            }
        }
    }
    let duplicates = {
        let mut stmt=tx.prepare("SELECT e.event_id,e.occurred_at_ms,e.origin_call_id,s.origin_host_id,s.user_id FROM usage_events e JOIN source_instances s ON s.instance_id=e.source_instance_id WHERE e.agent='qwen-code' AND e.parser_version=?1 AND e.attribution_status='verified' AND e.origin_call_id IS NOT NULL AND s.origin_host_id<>'legacy_unknown' ORDER BY s.created_at_ms,s.instance_id,e.event_id")?;
        let rows = stmt
            .query_map([PARSER], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut identities = BTreeSet::new();
    for (id, ms, origin, host, user) in duplicates {
        crate::adapters::run_policy::check()?;
        if !identities.insert((origin, host, user)) {
            tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='qwen_sdk_span_duplicate' WHERE event_id=?1",[id])?;
            affected.insert(calendar.local_day_of(ms)?);
        }
    }
    let old: Option<String> = tx
        .query_row("SELECT value FROM settings WHERE key=?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    let mut scopes: BTreeSet<Scope> = old
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_default();
    let before = scopes.clone();
    if let Some((host, user)) = owner {
        for e in batch.events.iter().filter(|e| e.parser_version == PARSER) {
            crate::adapters::run_policy::check()?;
            let accepted: Option<(Option<String>,i64)> = tx.query_row("SELECT session_id,occurred_at_ms FROM usage_events WHERE source_instance_id=?1 AND source_record_key=?2 AND parser_version=?3 AND attribution_status='verified'",params![e.source_instance_id,e.source_record_key,PARSER],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let Some((session, ms)) = accepted else {
                continue;
            };
            let Some(session) = session.filter(|s| !s.is_empty() && s != "unknown-session") else {
                tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='qwen_sdk_session_unverified' WHERE source_instance_id=?1 AND source_record_key=?2",params![e.source_instance_id,e.source_record_key])?;
                affected.insert(calendar.local_day_of(ms)?);
                continue;
            };
            let day = calendar.local_day_of(ms)?.to_string();
            // A sealed SDK source/day may have lost its trace/span details.
            // Its aggregate cannot be split to prove another export disjoint.
            let sdk_sealed: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM daily_usage d JOIN source_instances s ON s.instance_id=d.instance_id WHERE d.agent='qwen-code' AND s.agent='otel' AND s.origin_host_id=?1 AND s.user_id=?2 AND d.tz_version=?3 AND d.local_day=?4 AND d.sealed=1)",params![host,user,calendar.tz_name(),day],|r|r.get(0))?;
            if sdk_sealed {
                tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='qwen_sdk_partition_sealed' WHERE source_instance_id=?1 AND source_record_key=?2",params![e.source_instance_id,e.source_record_key])?;
                affected.insert(calendar.local_day_of(ms)?);
                continue;
            }
            // Archived details may already be gone. A sealed native source/day
            // is indivisible, so it blocks overlapping SDK contributions.
            let sealed: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM daily_usage d JOIN source_instances s ON s.instance_id=d.instance_id WHERE s.agent='qwen-code' AND s.parser_version LIKE 'qwen-chatrecord-%' AND s.origin_host_id=?1 AND s.user_id=?2 AND d.tz_version=?3 AND d.local_day=?4 AND d.sealed=1)",params![host,user,calendar.tz_name(),day],|r|r.get(0))?;
            if sealed {
                tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='qwen_native_partition_sealed' WHERE source_instance_id=?1 AND source_record_key=?2",params![e.source_instance_id,e.source_record_key])?;
                affected.insert(calendar.local_day_of(ms)?);
                continue;
            }
            scopes.insert((
                host.clone(),
                user.clone(),
                session,
                day,
                calendar.tz_name().into(),
            ));
        }
    }
    if scopes != before {
        tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",params![KEY,serde_json::to_string(&scopes)?,batch.now_ms])?;
    }
    let native = {
        let mut stmt=tx.prepare("SELECT e.event_id,e.session_id,e.occurred_at_ms,s.origin_host_id,s.user_id FROM usage_events e JOIN source_instances s ON s.instance_id=e.source_instance_id WHERE e.agent='qwen-code' AND e.parser_version LIKE 'qwen-chatrecord-%' AND e.attribution_status='verified' AND e.session_id IS NOT NULL")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (id, session, ms, host, user) in native {
        crate::adapters::run_policy::check()?;
        if !scopes.iter().any(|(h, u, s, d, tz)| {
            h == &host
                && u == &user
                && s == &session
                && Calendar::new(tz)
                    .and_then(|c| c.local_day_of(ms))
                    .is_ok_and(|day| day.to_string() == *d)
        }) {
            continue;
        }
        let day = calendar.local_day_of(ms)?;
        let sealed: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM daily_usage d JOIN usage_events e ON e.source_instance_id=d.instance_id WHERE e.event_id=?1 AND d.tz_version=?2 AND d.local_day=?3 AND d.sealed=1)",params![id,calendar.tz_name(),day.to_string()],|r|r.get(0))?;
        if !sealed {
            tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='qwen_sdk_session_authority' WHERE event_id=?1",[id])?;
            affected.insert(day);
        }
    }
    Ok(())
}
