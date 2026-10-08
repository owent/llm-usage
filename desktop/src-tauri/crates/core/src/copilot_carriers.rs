//! Select a record format per host/user/session/local day, without guessing
//! call identity from time/token equality; native records remain stored.
use crate::{calendar::Calendar, error::CoreError, ingest::IngestBatch};
use jiff::civil::Date;
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::BTreeSet;

const KEY: &str = "copilot_otel_scopes_v1";
type Scope = (String, String, String, String, String);

pub(crate) fn upgrade_otel_identity(
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    calendar: &Calendar,
    affected: &mut BTreeSet<Date>,
) -> Result<(), CoreError> {
    for e in batch
        .events
        .iter()
        .filter(|e| e.parser_version == "otel-spans-file-2")
    {
        let Some((_, span)) = e
            .origin_call_id
            .as_deref()
            .and_then(|s| s.strip_prefix("otel-span:"))
            .and_then(|s| serde_json::from_str::<(String, String)>(s).ok())
        else {
            continue;
        };
        let old:Option<(String,i64)>=tx.query_row("SELECT event_id,occurred_at_ms FROM usage_events WHERE
            source_instance_id=?1 AND source_record_key=?2 AND parser_version IN ('otel-spans-doc1','otel-spans-file-2')
            AND attribution_status='verified' AND EXISTS(SELECT 1 FROM usage_events accepted
                WHERE accepted.source_instance_id=?1 AND accepted.source_record_key=?3 AND accepted.attribution_status='verified')",
            params![e.source_instance_id,format!("otel:{span}"),e.source_record_key],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((id, ms)) = old {
            // Accepted trace+span records replace legacy span-only identities;
            // old audit records remain stored without inflating aggregates.
            tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='otel_trace_identity_upgrade' WHERE event_id=?1",[id])?;
            affected.insert(calendar.local_day_of(ms)?);
        }
    }
    Ok(())
}

pub(crate) fn select(
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    calendar: &Calendar,
    affected: &mut BTreeSet<Date>,
) -> Result<(), CoreError> {
    if !batch
        .events
        .iter()
        .any(|e| e.agent == "vscode-copilot-chat")
    {
        return Ok(());
    }
    // Repeated trace+span identifies the same call. Keep one stable file
    // per local host/user regardless of collection enable state.
    let duplicates = {
        let mut stmt=tx.prepare("SELECT e.event_id,e.occurred_at_ms,e.origin_call_id,s.origin_host_id,s.user_id
            FROM usage_events e JOIN source_instances s ON s.instance_id=e.source_instance_id
            WHERE e.agent='vscode-copilot-chat' AND e.parser_version='otel-spans-file-2' AND e.attribution_status='verified'
            AND e.origin_call_id LIKE 'otel-span:[%' AND s.origin_host_id<>'legacy_unknown'
            ORDER BY s.created_at_ms,s.instance_id,e.event_id")?;
        let rows = stmt
            .query_map([], |r| {
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
        if !identities.insert((origin, host, user)) {
            tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='copilot_otel_span_duplicate' WHERE event_id=?1",[id])?;
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
    let owner: Option<(String, String)> = tx
        .query_row(
            "SELECT origin_host_id,user_id FROM source_instances WHERE instance_id=?1",
            [&batch.instance_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((host, user)) = owner.filter(|(h, _)| !h.is_empty() && h != "legacy_unknown") {
        for e in &batch.events {
            if e.agent == "vscode-copilot-chat"
                && e.parser_version == "otel-spans-file-2"
                && e.attribution_status == crate::domain::AttributionStatus::Verified
            {
                // Rejected/expired input cannot select the contributing format. Read the
                // accepted version and its session/time from this transaction.
                let accepted: Option<(Option<String>, Option<String>, i64)> = tx
                    .query_row(
                        "SELECT session_id,parent_session_id,occurred_at_ms FROM usage_events
                     WHERE source_instance_id=?1 AND source_record_key=?2
                       AND parser_version='otel-spans-file-2' AND attribution_status='verified'",
                        params![e.source_instance_id, e.source_record_key],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()?;
                let Some((session, parent, ms)) = accepted else {
                    continue;
                };
                let day = calendar.local_day_of(ms)?.to_string();
                let mut frozen = false;
                for s in [&session, &parent].into_iter().flatten() {
                    frozen |= tx.query_row("SELECT EXISTS(SELECT 1 FROM usage_events n
                        JOIN source_instances src ON src.instance_id=n.source_instance_id
                        JOIN daily_usage d ON d.instance_id=src.instance_id
                        WHERE n.agent='vscode-copilot-chat' AND n.parser_version LIKE 'vscode-copilot-chat-session-log-%'
                        AND n.session_id=?1 AND src.origin_host_id=?2 AND src.user_id=?3
                        AND d.tz_version=?4 AND d.local_day=?5 AND d.sealed=1)",
                        params![s,host,user,calendar.tz_name(),day],|r| r.get::<_,bool>(0))?;
                }
                if frozen {
                    tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='copilot_native_partition_sealed'
                        WHERE source_instance_id=?1 AND source_record_key=?2",params![e.source_instance_id,e.source_record_key])?;
                    affected.insert(calendar.local_day_of(ms)?);
                    continue;
                }
                for session in [&session, &parent].into_iter().flatten() {
                    if !session.is_empty() {
                        scopes.insert((
                            host.clone(),
                            user.clone(),
                            session.clone(),
                            day.clone(),
                            calendar.tz_name().to_string(),
                        ));
                    }
                }
            }
        }
    }
    if before != scopes {
        tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES(?1,?2,1,?3)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
            params![KEY,serde_json::to_string(&scopes)?,batch.now_ms])?;
    }
    if scopes.is_empty() {
        return Ok(());
    }
    let native = {
        let mut stmt = tx.prepare("SELECT e.event_id,e.session_id,e.occurred_at_ms,s.origin_host_id,s.user_id
            FROM usage_events e JOIN source_instances s ON s.instance_id=e.source_instance_id
            WHERE e.agent='vscode-copilot-chat' AND e.parser_version LIKE 'vscode-copilot-chat-session-log-%'
              AND e.attribution_status='verified' AND e.session_id IS NOT NULL")?;
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
        let day = calendar.local_day_of(ms)?;
        let selected = scopes.iter().any(|(h, u, s, d, tz)| {
            h == &host
                && u == &user
                && s == &session
                && Calendar::new(tz)
                    .and_then(|c| c.local_day_of(ms))
                    .is_ok_and(|date| date.to_string() == *d)
        });
        if selected {
            // Preserve sealed partitions; their historical contributions are fixed.
            let sealed: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM daily_usage d
                JOIN usage_events e ON e.source_instance_id=d.instance_id WHERE e.event_id=?1
                AND d.tz_version=?2 AND d.local_day=?3 AND d.sealed=1)",
                params![id, calendar.tz_name(), day.to_string()],
                |r| r.get(0),
            )?;
            if sealed {
                continue;
            }
            tx.execute("UPDATE usage_events SET attribution_status='excluded',exclusion_reason='copilot_otel_session_authority' WHERE event_id=?1",[id])?;
            affected.insert(day);
        }
    }
    Ok(())
}
