//! Schema 24 hot transcript reader. See openclaw-runtime.md for evidence/scope.
use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::openclaw::{
    common::{open_source_db, schema_probe, short_probe, StagingLimits},
    detect::agent_dir,
};
use crate::domain::*;
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

pub const OPENCLAW_PARSER_VERSION: &str = "openclaw-runtime-schema24-1";
pub const MAX_ROWS_PER_ROUND: usize = 50_000;
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
struct Cursor {
    generation: i64,
    offset: u64,
    after_session: String,
    after_seq: i64,
}

fn diag(code: &str, position: &str, field: Option<&str>, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: field.map(str::to_string),
        position: Some(position.into()),
        message: message.into(),
    }
}

fn positive(
    value: &Value,
    name: &str,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<i64> {
    match value.get(name) {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_i64() {
            Some(number) if (0..=MAX_TOKEN_VALUE).contains(&number) => {
                (number > 0).then_some(number)
            }
            _ => {
                diagnostics.push(diag(
                    "invalid_usage_field",
                    position,
                    Some(name),
                    "Invalid native token type/range; field remains unknown",
                ));
                None
            }
        },
    }
}

fn message_event(
    value: &Value,
    session: &str,
    instance: &str,
    now: i64,
    position: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<EventInput> {
    if value.get("type").and_then(Value::as_str) != Some("message") {
        return None;
    }
    let message = value.get("message")?;
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    if message.get("api").and_then(Value::as_str) != Some("openai-completions") {
        diagnostics.push(diag(
            "unverified_transport",
            position,
            Some("api"),
            "Assistant transport not verified; usage isolated",
        ));
        return None;
    }
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= 512);
    let timestamp = message
        .get("timestamp")
        .and_then(Value::as_i64)
        .filter(|time| (MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(time));
    let (Some(id), Some(timestamp)) = (id, timestamp) else {
        diagnostics.push(diag(
            "invalid_transcript_identity_time",
            position,
            None,
            "Assistant entry has no valid stable id/start timestamp; skipped",
        ));
        return None;
    };
    let Some(native) = message.get("usage").filter(|v| v.is_object()) else {
        diagnostics.push(diag(
            "missing_usage",
            position,
            Some("usage"),
            "Assistant has no verified usage object; no zero record",
        ));
        return None;
    };
    let mut usage = TokenUsage {
        input_uncached: positive(native, "input", position, diagnostics),
        input_cache_read: positive(native, "cacheRead", position, diagnostics),
        input_cache_write: positive(native, "cacheWrite", position, diagnostics),
        output_total: positive(native, "output", position, diagnostics),
        output_reasoning: positive(native, "reasoningTokens", position, diagnostics),
        ..Default::default()
    };
    if let (Some(reasoning), Some(output)) = (usage.output_reasoning, usage.output_total) {
        if reasoning > output {
            diagnostics.push(diag(
                "usage_contradiction",
                position,
                Some("reasoningTokens"),
                "Reasoning exceeds total output; reasoning kept unknown",
            ));
            usage.output_reasoning = None;
        }
    }
    // totalTokens is a calculation over default-zero buckets in the distributed
    // transport. It is neither independent raw total evidence nor completeness.
    let _ = positive(native, "totalTokens", position, diagnostics);
    if let Some(total) = native
        .get("totalTokens")
        .and_then(Value::as_i64)
        .filter(|v| *v >= 0)
    {
        let minimum = [
            usage.input_uncached,
            usage.input_cache_read,
            usage.input_cache_write,
            usage.output_total,
        ]
        .into_iter()
        .flatten()
        .try_fold(0i64, |sum, value| sum.checked_add(value));
        if minimum.map_or(true, |minimum| minimum > total) {
            diagnostics.push(diag(
                "usage_contradiction",
                position,
                Some("totalTokens"),
                "Known token buckets exceed native calculated total; completeness remains unknown",
            ));
        }
    }
    if [
        &usage.input_uncached,
        &usage.input_cache_read,
        &usage.input_cache_write,
        &usage.output_total,
        &usage.output_reasoning,
    ]
    .iter()
    .all(|value| value.is_none())
    {
        diagnostics.push(diag(
            "usage_unreported",
            position,
            Some("usage"),
            "Only missing/default-zero buckets are present; no known usage or call inferred",
        ));
        return None;
    }
    let quality = TokenQuality {
        input_uncached: if usage.input_uncached.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        },
        input_cache_read: if usage.input_cache_read.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        },
        input_cache_write: if usage.input_cache_write.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        },
        output_total: if usage.output_total.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        },
        output_reasoning: if usage.output_reasoning.is_some() {
            FieldQuality::Reported
        } else {
            FieldQuality::Unknown
        },
        ..Default::default()
    };
    let text_field = |field| {
        message
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 512)
            .map(str::to_string)
    };
    let model = text_field("model");
    Some(EventInput {
        source_instance_id: instance.into(),
        source_record_key: serde_json::to_string(&(session, id)).ok()?,
        record_kind: RecordKind::UsageObservation,
        schema_version: "openclaw-runtime-schema24".into(),
        parser_version: OPENCLAW_PARSER_VERSION.into(),
        parse_basis: Some(VersionBasis::LatestFallback),
        origin_call_id: None,
        attempt_id: None,
        session_id: Some(session.into()),
        parent_session_id: None,
        host_application: None,
        agent: "openclaw".into(),
        call_category: CallCategory::Unknown,
        occurred_at_ms: timestamp,
        observed_at_ms: Some(now),
        source_time: Some(timestamp.to_string()),
        time_basis: TimeBasis::SourceStart,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: text_field("provider"),
        model_raw: model.clone(),
        model_canonical: None,
        model_attribution: if model.is_some() {
            ModelAttribution::RequestField
        } else {
            ModelAttribution::Unknown
        },
        usage,
        quality,
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: match message.get("stopReason").and_then(Value::as_str) {
            Some("error" | "aborted") => Some("source_failed".into()),
            Some("length") => Some("source_length_limited".into()),
            _ => None,
        },
        duration_ms: None,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    })
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    crate::adapters::run_policy::check()?;
    let owner_dir = agent_dir(&target.path)
        .ok_or_else(|| CoreError::Validation("OpenClaw path ownership changed".into()))?;
    let owner = owner_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let snapshot = source.conn().unchecked_transaction()?;
    if schema_probe(&snapshot, owner)? != Some(24) {
        return Err(CoreError::Validation(
            "OpenClaw schema/owner changed; cursor not advanced".into(),
        ));
    }
    let mut cursor: Cursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if target.rescan
        || cursor.generation != target.generation
        || stored
            .parse_context
            .as_ref()
            .and_then(|v| v.get("parser_version"))
            .and_then(Value::as_str)
            != Some(OPENCLAW_PARSER_VERSION)
    {
        cursor = Cursor {
            generation: target.generation,
            ..Default::default()
        };
    }
    let cap = limits
        .jsonl
        .max_lines
        .map(|v| v as usize)
        .unwrap_or(MAX_ROWS_PER_ROUND)
        .clamp(1, MAX_ROWS_PER_ROUND);
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen = 0;
    let mut more = false;
    let cold: bool = snapshot.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_transcript_cold_archives)",
        [],
        |r| r.get(0),
    )?;
    if cold {
        diagnostics.push(diag("cold_archive_coverage_incomplete","cold-archive",None,"Native cold archives exist and are not yet verified; hot records do not certify complete history"));
    }
    let invalid_keys:bool=snapshot.query_row("SELECT EXISTS(SELECT 1 FROM transcript_events WHERE typeof(session_id)!='text' OR typeof(seq)!='integer' OR seq<0)",[],|r|r.get(0))?;
    if invalid_keys {
        diagnostics.push(diag(
            "invalid_transcript_key",
            "transcript-key",
            None,
            "Invalid transcript key type/range; affected rows isolated",
        ));
    }
    let mut stmt=snapshot.prepare("SELECT e.session_id,e.seq,
      CASE WHEN length(CAST(e.event_json AS BLOB))<=?3 THEN e.event_json ELSE NULL END,
      CASE WHEN length(e.event_zstd)<=?3 THEN e.event_zstd ELSE NULL END,
      e.event_utf8_bytes,length(CAST(e.event_json AS BLOB)),length(e.event_zstd),
      w.session_entry_provenance,w.acp_owned,w.plugin_owner_id,w.hook_external_content_source,w.agent_harness_id,w.session_key
      FROM transcript_events e LEFT JOIN session_windows w ON w.session_id=e.session_id
      WHERE typeof(e.session_id)='text' AND typeof(e.seq)='integer' AND e.seq>=0 AND (e.session_id>?1 OR (e.session_id=?1 AND e.seq>?2))
      ORDER BY e.session_id,e.seq LIMIT ?4")?;
    let mut rows = stmt.query(rusqlite::params![
        cursor.after_session,
        cursor.after_seq,
        MAX_BODY_BYTES as i64,
        cap as i64 + 1
    ])?;
    while let Some(row) = rows.next()? {
        crate::adapters::run_policy::check()?;
        if seen >= cap {
            more = true;
            break;
        }
        seen += 1;
        let session: String = row.get(0)?;
        let seq: i64 = row.get(1)?;
        cursor.after_session = session.clone();
        cursor.after_seq = seq;
        let position = crate::identity::content_hash(&(session.as_str(), seq));
        let data = (|| -> Result<_, rusqlite::Error> {
            Ok((
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })();
        let (
            text,
            compressed,
            raw_size,
            text_size,
            zstd_size,
            provenance,
            acp,
            plugin,
            hook,
            harness,
            key,
        ) = match data {
            Ok(data) => data,
            Err(_) => {
                diagnostics.push(diag(
                    "invalid_row_type",
                    &position,
                    None,
                    "Invalid native payload/provenance SQL type; row isolated",
                ));
                continue;
            }
        };
        if provenance != Some(1)
            || acp != Some(0)
            || plugin.is_some()
            || hook.is_some()
            || harness.as_deref().is_some_and(|h| h != "openclaw")
            || !key
                .as_deref()
                .is_some_and(|key| key.starts_with(&format!("agent:{owner}:")))
        {
            diagnostics.push(diag(
                "source_attribution_unverified",
                &position,
                None,
                "Migrated/external/other harness provenance isolated; no token inference",
            ));
            continue;
        }
        if text_size.is_some_and(|size| size > MAX_BODY_BYTES as i64)
            || zstd_size.is_some_and(|size| size > MAX_BODY_BYTES as i64)
        {
            diagnostics.push(diag(
                "transcript_body_too_large",
                &position,
                None,
                "Native transcript body exceeds 4 MiB; skipped without loading oversized bytes",
            ));
            continue;
        }
        let bytes = match (text, compressed) {
            (Some(text), None) => text.into_bytes(),
            (None, Some(data))
                if raw_size.is_some_and(|size| (1..=MAX_BODY_BYTES as i64).contains(&size)) =>
            {
                let decoded = (|| -> std::io::Result<Vec<u8>> {
                    let decoder = zstd::stream::read::Decoder::new(data.as_slice())?;
                    let mut bytes = Vec::new();
                    crate::adapters::run_policy::checked_reader(decoder)
                        .take(MAX_BODY_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)?;
                    Ok(bytes)
                })();
                crate::adapters::run_policy::check()?;
                match decoded {
                    Ok(bytes) if Some(bytes.len() as i64) == raw_size => bytes,
                    _ => {
                        diagnostics.push(diag(
                            "invalid_transcript_encoding",
                            &position,
                            None,
                            "Invalid zstd payload or exact UTF-8 length; row isolated",
                        ));
                        continue;
                    }
                }
            }
            _ => {
                diagnostics.push(diag(
                    "invalid_transcript_encoding",
                    &position,
                    None,
                    "Expected exactly one native TEXT/zstd body with bounded metadata",
                ));
                continue;
            }
        };
        let value = match crate::adapters::run_policy::json_from_slice::<Value>(&bytes) {
            Ok(value) => value,
            Err(_) => {
                crate::adapters::run_policy::check()?;
                diagnostics.push(diag(
                    "invalid_transcript_json",
                    &position,
                    None,
                    "Invalid native JSON body; row isolated",
                ));
                continue;
            }
        };
        if let Some(event) = message_event(
            &value,
            &session,
            &target.instance_id,
            now_ms,
            &position,
            &mut diagnostics,
        ) {
            events.push(event);
        }
    }
    if !more {
        cursor.after_session.clear();
        cursor.after_seq = 0;
    }
    Ok(ScanOutcome {
        status: if more {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(cursor)?),
        parse_context: Some(
            serde_json::json!({"parser_version":OPENCLAW_PARSER_VERSION,"schema_version":24}),
        ),
        events,
        aggregates: Vec::new(),
        health: if diagnostics.is_empty() {
            "active"
        } else {
            "degraded"
        }
        .into(),
        diagnostics,
        lines_read: seen as u64,
        records_seen: seen as u64,
        reconciliations: Vec::new(),
    })
}
