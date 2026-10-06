//! Official VS Code 4.1.22 SDK messages writer, f58bc118 (A03).
//! Session origin is mutable metadata, not per-message version evidence.
//! Metrics may describe a whole run/retry group: observations, never inferred calls.
use std::{io::Read, path::Path};

use crate::{
    adapters::{framework::*, usage_map::finish},
    domain::*,
    error::CoreError,
    ingest::DiagnosticInput,
};

pub const FORMAT: &str = "cline-sdk-messages-json";
pub const PARSER_VERSION: &str = "cline-sdk-messages-v1";
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

pub fn is_sdk_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".messages.json"))
}

fn read_document(path: &Path) -> Result<Option<(serde_json::Value, u64)>, CoreError> {
    let mut bytes = Vec::new();
    crate::adapters::run_policy::checked_file(path)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Ok(None);
    }
    let parsed = crate::adapters::run_policy::json_from_slice(super::super::strip_bom(&bytes));
    crate::adapters::run_policy::check()?;
    Ok(parsed.ok().map(|value| (value, bytes.len() as u64)))
}

fn envelope(doc: &serde_json::Value) -> Option<&str> {
    let session = doc["sessionId"].as_str().filter(|s| !s.is_empty())?;
    (doc["version"].as_u64() == Some(1)
        && doc["agent"] == "lead"
        && doc["origin"]["source"] == "vscode"
        && doc["origin"]["mode"] == "user"
        && doc["origin"]["sessionId"].as_str() == Some(session)
        && doc["messages"].is_array())
    .then_some(session)
}

pub fn detect(path: &Path) -> Result<DetectOutcome, CoreError> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "Cline SDK messages exceed the 32 MiB read cap; no cursor advanced".into(),
        });
    }
    let Some((doc, _)) = read_document(path)? else {
        return Ok(DetectOutcome::Pending);
    };
    if envelope(&doc).is_none() {
        return Ok(DetectOutcome::UnknownFormat {
            reason: "unsupported Cline SDK schema, source, mode, agent or session identity".into(),
        });
    }
    if !doc["messages"].as_array().unwrap().iter().any(|m| {
        m["role"] == "assistant" && m["metrics"].is_object() && m["metadata"]["displayOnly"] != true
    }) {
        return Ok(DetectOutcome::Pending);
    }
    Ok(DetectOutcome::Supported {
        format: FORMAT.into(),
        format_version: doc["origin"]["version"].as_str().map(str::to_string),
        basis: VersionBasis::LatestFallback,
    })
}

fn diag(code: &str, field: Option<&str>, position: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: field.map(str::to_string),
        position: Some(position.into()),
        message:
            "Cline SDK usage field or envelope failed its validated contract; no content retained"
                .into(),
    }
}

pub fn scan(
    target: &ScanTarget,
    _stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let mut out = ScanOutcome {
        status: ScanStatus::Pending,
        cursor: None,
        parse_context: None,
        events: vec![],
        aggregates: vec![],
        diagnostics: vec![],
        lines_read: 0,
        records_seen: 0,
        reconciliations: vec![],
        health: "active".into(),
    };
    if target.probe.len > MAX_FILE_BYTES {
        out.status = ScanStatus::LineTooLong;
        out.health = "degraded".into();
        out.diagnostics
            .push(diag("file_exceeds_size_cap", None, "document"));
        return Ok(out);
    }
    let Some((doc, consumed)) = read_document(&target.path)? else {
        out.diagnostics
            .push(diag("sdk_messages_unparseable", None, "document"));
        return Ok(out);
    };
    let Some(session) = envelope(&doc) else {
        out.health = "degraded".into();
        out.diagnostics
            .push(diag("sdk_envelope_unverified", None, "document"));
        return Ok(out);
    };
    for (index, message) in doc["messages"].as_array().unwrap().iter().enumerate() {
        crate::adapters::run_policy::check()?;
        out.records_seen += 1;
        if message["role"] != "assistant"
            || message["metadata"]["displayOnly"] == true
            || message.get("metrics").is_none()
        {
            continue;
        }
        let position = format!("messages[{index}]");
        let Some(metrics) = message["metrics"].as_object() else {
            out.diagnostics
                .push(diag("usage_shape_deviation", Some("metrics"), &position));
            continue;
        };
        let Some(id) = message["id"].as_str().filter(|s| !s.is_empty()) else {
            out.diagnostics
                .push(diag("message_id_missing", Some("id"), &position));
            continue;
        };
        let Some(ts) = message["ts"]
            .as_i64()
            .filter(|v| (MIN_PLAUSIBLE_MS..=4_102_444_800_000).contains(v))
        else {
            out.diagnostics
                .push(diag("timestamp_unparseable", Some("ts"), &position));
            continue;
        };
        let mut counts = [None; 4];
        let mut valid = true;
        for (slot, name) in counts.iter_mut().zip([
            "inputTokens",
            "outputTokens",
            "cacheReadTokens",
            "cacheWriteTokens",
        ]) {
            if let Some(value) = metrics.get(name) {
                match value.as_i64().filter(|v| (0..=MAX_TOKEN_VALUE).contains(v)) {
                    Some(v) => *slot = (v > 0).then_some(v),
                    None => {
                        valid = false;
                        out.diagnostics
                            .push(diag("usage_shape_deviation", Some(name), &position));
                    }
                }
            }
        }
        if !valid {
            continue;
        }
        let [input, output, read, write] = counts;
        let uncached = match (input, read, write) {
            (Some(i), Some(r), Some(w)) => i
                .checked_sub(r)
                .and_then(|v| v.checked_sub(w))
                .filter(|v| *v >= 0),
            _ => None,
        };
        let total = input.and_then(|i| output.and_then(|o| i.checked_add(o)));
        let quality = |value: Option<i64>| {
            if value.is_some() {
                FieldQuality::Reported
            } else {
                FieldQuality::Unknown
            }
        };
        let derived = |value: Option<i64>| {
            if value.is_some() {
                FieldQuality::Derived
            } else {
                FieldQuality::Unknown
            }
        };
        let mapped = finish(
            TokenUsage {
                input_uncached: uncached,
                input_cache_read: read,
                input_cache_write: write,
                input_total: input,
                output_total: output,
                output_reasoning: None,
                total_tokens: total,
                source_total: None,
            },
            TokenQuality {
                input_uncached: derived(uncached),
                input_cache_read: quality(read),
                input_cache_write: quality(write),
                input_total: quality(input),
                output_total: quality(output),
                output_reasoning: FieldQuality::Unknown,
                total_tokens: derived(total),
                source_total: FieldQuality::Unknown,
            },
            vec![],
        );
        for issue in &mapped.diagnostics {
            out.diagnostics
                .push(diag(issue.code, Some(issue.field), &position));
        }
        let model = message["modelInfo"]["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let provider = message["modelInfo"]["provider"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        out.events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("sdk:{}", serde_json::to_string(&[session, id])?),
            record_kind: RecordKind::UsageObservation,
            schema_version: "cline-sdk-messages-1".into(),
            parser_version: PARSER_VERSION.into(),
            parse_basis: Some(VersionBasis::LatestFallback),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(session.into()),
            parent_session_id: None,
            host_application: Some("vscode".into()),
            agent: "cline".into(),
            call_category: CallCategory::Primary,
            occurred_at_ms: ts,
            observed_at_ms: Some(now_ms),
            source_time: Some(ts.to_string()),
            time_basis: TimeBasis::Uncertain,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: provider,
            model_raw: model.clone(),
            model_canonical: None,
            model_attribution: if model.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
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
        });
    }
    out.status = ScanStatus::Complete;
    out.cursor = Some(
        serde_json::json!({ "generation": target.generation, "offset": consumed, "line_number": 1 }),
    );
    out.parse_context = Some(
        serde_json::json!({ "version_basis": "latest_fallback", "parser_version": PARSER_VERSION }),
    );
    out.lines_read = 1;
    if !out.diagnostics.is_empty() {
        out.health = "degraded".into();
    }
    Ok(out)
}
