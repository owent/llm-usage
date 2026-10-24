//! Source-derived CLI 1.1.76 transcript reader; native-client acceptance remains pending.
//! KLo/bxu/kU persist inclusive input, default/redacted zeros and per-record versions.
//! SessionTranscriptWriter buffers partials; fork copies retain forkedFrom ownership.
use crate::{
    adapters::{framework::*, jsonl::*, usage_map::finish},
    domain::*,
    error::CoreError,
    ingest::DiagnosticInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PARSER_VERSION: &str = "qoder-transcript-2";

#[derive(Default, Serialize, Deserialize)]
pub struct Context {
    pub parser_version: String,
    pub validated: bool,
    pub fallback: bool,
    pub has_errors: bool,
    pub native_envelope: bool,
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().filter(|s| !s.trim().is_empty())
}

fn diag(code: &str, field: &str, line: u64) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: Some(field.into()),
        position: Some(format!("line {line}")),
        message:
            "Qoder transcript field failed the source-derived CLI contract; no content retained"
                .into(),
    }
}

fn positive(
    value: &Value,
    field: &str,
    line: u64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<i64> {
    let raw = value.get(field)?;
    if raw.is_null() {
        return None;
    }
    match raw.as_i64().filter(|n| (0..=MAX_TOKEN_VALUE).contains(n)) {
        Some(0) => None,
        Some(n) => Some(n),
        None => {
            diagnostics.push(diag("usage_shape_deviation", field, line));
            None
        }
    }
}

fn reported(value: Option<i64>) -> FieldQuality {
    value.map_or(FieldQuality::Unknown, |_| FieldQuality::Reported)
}

fn derived(value: Option<i64>) -> FieldQuality {
    value.map_or(FieldQuality::Unknown, |_| FieldQuality::Derived)
}

fn event(
    row: &Value,
    target: &ScanTarget,
    line: u64,
    now_ms: i64,
    context: &mut Context,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Option<EventInput> {
    if row["type"] != "assistant" {
        return None;
    }
    let message = &row["message"];
    if row["entrypoint"] != "cli" || row["userType"] != "external" {
        diagnostics.push(diag("usage_fields_unverified", "entrypoint/userType", line));
        return None;
    }
    if text(row, "cwd").is_none() {
        diagnostics.push(diag("assistant_metadata_missing", "cwd", line));
        return None;
    }
    let ids = (
        text(row, "sessionId"),
        text(row, "uuid"),
        text(message, "id"),
    );
    let (Some(session), Some(uuid), Some(id)) = ids else {
        diagnostics.push(diag(
            "message_identity_missing",
            "sessionId/uuid/message.id",
            line,
        ));
        return None;
    };
    let Some(sidechain) = row["isSidechain"].as_bool() else {
        diagnostics.push(diag("ownership_unverified", "isSidechain", line));
        return None;
    };
    if sidechain && text(row, "agentId").is_none() {
        diagnostics.push(diag("ownership_unverified", "agentId", line));
        return None;
    }
    if let Some(fork) = row.get("forkedFrom") {
        if text(fork, "sessionId").is_none_or(|owner| owner == session)
            || text(fork, "messageUuid") != Some(uuid)
        {
            diagnostics.push(diag("ownership_unverified", "forkedFrom", line));
        }
        return None;
    }
    if row
        .get("isApiErrorMessage")
        .is_some_and(|v| !v.is_boolean())
    {
        diagnostics.push(diag(
            "assistant_shape_unverified",
            "isApiErrorMessage",
            line,
        ));
        return None;
    }
    if message["model"] == "<synthetic>" || row["isApiErrorMessage"] == true {
        return None;
    }
    if message.get("stop_reason").is_some_and(Value::is_null) {
        return None;
    }
    if message["role"] != "assistant"
        || text(message, "stop_reason").is_none()
        || !message["content"].is_array()
    {
        diagnostics.push(diag("assistant_shape_unverified", "message", line));
        return None;
    }
    let (Some(model), Some(version), Some(source_time)) = (
        text(message, "model"),
        text(row, "version"),
        text(row, "timestamp"),
    ) else {
        diagnostics.push(diag(
            "assistant_metadata_missing",
            "model/version/timestamp",
            line,
        ));
        return None;
    };
    let Some(occurred) = source_time
        .parse::<jiff::Timestamp>()
        .ok()
        .map(|t| t.as_millisecond())
        .filter(|t| *t >= MIN_PLAUSIBLE_MS)
    else {
        diagnostics.push(diag("timestamp_unparseable", "timestamp", line));
        return None;
    };
    let basis = super::select(Some(version)).basis;
    context.validated = true;
    context.fallback |= basis == VersionBasis::LatestFallback;
    let usage = &message["usage"];
    if !usage.is_null() && !usage.is_object() {
        diagnostics.push(diag("usage_shape_deviation", "message.usage", line));
    }
    let input = positive(usage, "input_tokens", line, diagnostics);
    let output = positive(usage, "output_tokens", line, diagnostics);
    let read = positive(usage, "cache_read_input_tokens", line, diagnostics);
    let write = positive(usage, "cache_creation_input_tokens", line, diagnostics);
    let uncached = input.zip(read).zip(write).and_then(|((i, r), w)| {
        i.checked_sub(r)
            .and_then(|v| v.checked_sub(w))
            .filter(|v| *v >= 0)
    });
    let total = input
        .zip(output)
        .and_then(|(i, o)| i.checked_add(o))
        .filter(|n| *n <= MAX_TOKEN_VALUE);
    if input.is_some() && output.is_some() && total.is_none() {
        diagnostics.push(diag("derived_total_out_of_range", "total_tokens", line));
    }
    let mapped = finish(
        TokenUsage {
            input_uncached: uncached,
            input_total: input,
            input_cache_read: read,
            input_cache_write: write,
            output_total: output,
            total_tokens: total,
            ..Default::default()
        },
        TokenQuality {
            input_uncached: derived(uncached),
            input_total: reported(input),
            input_cache_read: reported(read),
            input_cache_write: reported(write),
            output_total: reported(output),
            total_tokens: derived(total),
            ..Default::default()
        },
        vec![],
    );
    for issue in &mapped.diagnostics {
        diagnostics.push(diag(issue.code, issue.field, line));
    }
    Some(EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("response:{}", serde_json::to_string(&[session, id]).ok()?),
        record_kind: RecordKind::ModelCall,
        schema_version: version.into(),
        parser_version: PARSER_VERSION.into(),
        parse_basis: Some(basis),
        origin_call_id: Some(id.into()),
        attempt_id: None,
        session_id: Some(session.into()),
        parent_session_id: None,
        host_application: None,
        agent: "qoder".into(),
        call_category: if sidechain {
            CallCategory::SubAgent
        } else {
            CallCategory::Primary
        },
        occurred_at_ms: occurred,
        observed_at_ms: Some(now_ms),
        source_time: Some(source_time.into()),
        time_basis: TimeBasis::ObservedAt,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: None,
        model_raw: Some(model.into()),
        model_canonical: None,
        model_attribution: ModelAttribution::RequestField,
        usage: mapped.usage,
        quality: mapped.quality,
        lifecycle: Lifecycle::Final,
        source_revision: None,
        error_status: None,
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
    if !super::super::detect::native_record(&target.path) {
        return super::probe_only::scan(target, stored, limits, now_ms);
    }
    let saved = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value::<Context>(v.clone()).ok());
    let replay = target.rescan
        || saved
            .as_ref()
            .is_none_or(|c| c.parser_version != PARSER_VERSION);
    let mut context = if replay {
        Context::default()
    } else {
        saved.unwrap_or_default()
    };
    context.parser_version = PARSER_VERSION.into();
    // The independent file probe verified a native CLI envelope, even when this
    // window precedes it. This permits progress without certifying usage fields.
    context.native_envelope = true;
    let cursor = (!replay)
        .then_some(stored.cursor.as_ref())
        .flatten()
        .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
        .filter(|c| c.generation == target.generation)
        .unwrap_or(JsonlCursor {
            generation: target.generation,
            offset: 0,
            line_number: 1,
        });
    let read = read_jsonl(
        &target.path,
        cursor.offset,
        cursor.line_number,
        &limits.jsonl,
    )?;
    let mut diagnostics = Vec::new();
    let mut events = Vec::new();
    for bad in &read.bad_lines {
        diagnostics.push(diag(bad.code, "record", bad.number));
    }
    for raw in &read.lines {
        crate::adapters::run_policy::check()?;
        match crate::adapters::run_policy::json_from_str::<Value>(&raw.text) {
            Ok(row) => {
                if let Some(event) = event(
                    &row,
                    target,
                    raw.number,
                    now_ms,
                    &mut context,
                    &mut diagnostics,
                ) {
                    events.push(event);
                }
            }
            Err(_) => diagnostics.push(diag("invalid_json", "record", raw.number)),
        }
    }
    context.has_errors |= !diagnostics.is_empty();
    let status = match read.stop {
        StopReason::Eof => ScanStatus::Complete,
        StopReason::LineTooLong { number, .. } => {
            diagnostics.push(diag("line_too_long", "record", number));
            ScanStatus::LineTooLong
        }
        _ => ScanStatus::BudgetExhausted,
    };
    let health = if context.has_errors || status == ScanStatus::LineTooLong {
        "degraded"
    } else if context.fallback {
        "active_compat"
    } else {
        "active"
    };
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(JsonlCursor {
            generation: target.generation,
            offset: read.next_offset,
            line_number: read.next_line_number,
        })?),
        parse_context: Some(serde_json::to_value(&context)?),
        events,
        aggregates: vec![],
        diagnostics,
        lines_read: read.lines.len() as u64,
        records_seen: read.lines.len() as u64,
        reconciliations: vec![],
        health: health.into(),
    })
}
