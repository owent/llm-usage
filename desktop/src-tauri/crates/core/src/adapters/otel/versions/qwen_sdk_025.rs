//! Qwen 0.25.0 file exporters / llm_request spans, pinned to 6788c035.
//! SDK INTERNAL kind is intentional. Logs and metrics do not add model calls.
use crate::adapters::{
    framework::{DetectOutcome, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState},
    jsonl::JsonlCursor,
    run_policy,
};
use crate::domain::*;
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde_json::Value;

pub const FORMAT: &str = "qwen-code-sdk-json-stream";
pub const PARSER: &str = "qwen-sdk-file-0.25.0-1";

fn resource<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .pointer("/resource/_rawAttributes")?
        .as_array()?
        .iter()
        .find_map(|pair| {
            let a = pair.as_array()?;
            (a.len() == 2 && a[0].as_str() == Some(key))
                .then(|| a[1].as_str())
                .flatten()
        })
}

pub(crate) fn detect_head(bytes: &[u8]) -> Option<DetectOutcome> {
    let first = serde_json::Deserializer::from_slice(bytes)
        .into_iter::<Value>()
        .next()?
        .ok()?;
    if resource(&first, "service.name") != Some("qwen-code") {
        return None;
    }
    let version = resource(&first, "service.version");
    Some(DetectOutcome::Supported {
        format: FORMAT.into(),
        format_version: version.map(str::to_string),
        basis: if version == Some("0.25.0") {
            VersionBasis::KnownVersion
        } else {
            VersionBasis::LatestFallback
        },
    })
}

fn diag(code: &str, offset: u64, field: Option<&str>) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.into(),
        field: field.map(str::to_string),
        position: Some(format!("byte:{offset}")),
        message: format!("Qwen SDK object: {code}; raw content not stored"),
    }
}

fn timestamp(value: &Value) -> Option<i64> {
    let pair = value.as_array()?;
    if pair.len() != 2 {
        return None;
    }
    let seconds = pair[0].as_i64()?;
    let nanos = pair[1].as_i64()?;
    if !(0..1_000_000_000).contains(&nanos) {
        return None;
    }
    seconds
        .checked_mul(1000)?
        .checked_add(nanos / 1_000_000)
        .filter(|n| *n >= MIN_PLAUSIBLE_MS)
}

fn hex_id(value: Option<&Value>, length: usize) -> Option<&str> {
    value?.as_str().filter(|id| {
        id.len() == length
            && id.bytes().all(|c| c.is_ascii_hexdigit())
            && id.bytes().any(|c| c != b'0')
    })
}

pub(crate) fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now: i64,
) -> Result<ScanOutcome, CoreError> {
    scan_with_byte_budget(target, stored, limits, now, 16 * 1024 * 1024)
}

/// Read-only callers can inspect a bounded prefix; offsets remain object boundaries.
pub fn scan_with_byte_budget(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now: i64,
    byte_budget: u64,
) -> Result<ScanOutcome, CoreError> {
    let cursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value::<JsonlCursor>(v.clone()).ok())
        .filter(|c| c.generation == target.generation);
    let previous_errors = stored
        .parse_context
        .as_ref()
        .is_some_and(|c| c["had_errors"] == true);
    let restart = previous_errors
        && cursor
            .as_ref()
            .is_some_and(|c| c.offset == target.probe.len);
    let offset = if restart {
        0
    } else {
        cursor.as_ref().map_or(0, |c| c.offset)
    };
    let read = super::super::sdk_json::read(
        &target.path,
        offset,
        &limits.jsonl,
        byte_budget.min(16 * 1024 * 1024),
    )?;
    let mut diagnostics = Vec::new();
    let mut events = Vec::new();
    let mut records_seen = 0;
    for (position, bytes) in &read.objects {
        run_policy::check()?;
        let value: Value = match run_policy::json_from_slice(bytes) {
            Ok(v) => v,
            Err(_) => {
                diagnostics.push(diag("sdk_object_invalid", *position, None));
                continue;
            }
        };
        if value.get("name").and_then(Value::as_str) != Some("qwen-code.llm_request") {
            continue;
        }
        records_seen += 1;
        if resource(&value, "service.name") != Some("qwen-code")
            || value["kind"].as_u64() != Some(0)
        {
            diagnostics.push(diag("qwen_sdk_product_or_kind_invalid", *position, None));
            continue;
        }
        let (Some(trace), Some(span), Some(time)) = (
            hex_id(value.pointer("/_spanContext/traceId"), 32),
            hex_id(value.pointer("/_spanContext/spanId"), 16),
            timestamp(&value["startTime"]),
        ) else {
            diagnostics.push(diag("qwen_sdk_identity_or_time_invalid", *position, None));
            continue;
        };
        let Some(attrs) = value["attributes"].as_object() else {
            diagnostics.push(diag("qwen_sdk_attributes_invalid", *position, None));
            continue;
        };
        if attrs.get("gen_ai.operation.name").and_then(Value::as_str) != Some("chat") {
            diagnostics.push(diag("qwen_sdk_operation_invalid", *position, None));
            continue;
        }
        let mut token = |key: &str| -> Option<i64> {
            match attrs.get(key) {
                None | Some(Value::Null) => None,
                Some(v) => match v.as_i64().filter(|n| (0..=MAX_TOKEN_VALUE).contains(n)) {
                    Some(n) => Some(n),
                    None => {
                        diagnostics.push(diag("qwen_sdk_token_invalid", *position, Some(key)));
                        None
                    }
                },
            }
        };
        let input = token("gen_ai.usage.input_tokens");
        let output = token("gen_ai.usage.output_tokens");
        let cache_read = token("gen_ai.usage.cache_read.input_tokens");
        let cache_write = token("gen_ai.usage.cache_creation.input_tokens");
        let reasoning = token("thoughts_token_count");
        // candidates/thoughts containment varies by provider; this SDK format
        // does not retain provider-specific definitions. The native sample reports thoughts=0.
        let total = if reasoning == Some(0) {
            input.zip(output).and_then(|(i, o)| i.checked_add(o))
        } else {
            None
        };
        let usage = TokenUsage {
            input_total: input,
            output_total: output,
            input_cache_read: cache_read,
            input_cache_write: cache_write,
            output_reasoning: reasoning,
            total_tokens: total,
            ..Default::default()
        };
        let quality = TokenQuality {
            input_total: FieldQuality::Reported,
            output_total: FieldQuality::Reported,
            input_cache_read: FieldQuality::Reported,
            input_cache_write: FieldQuality::Reported,
            output_reasoning: FieldQuality::Reported,
            total_tokens: FieldQuality::Derived,
            ..Default::default()
        };
        let mapped = crate::adapters::usage_map::finish(usage, quality, Vec::new());
        for error in &mapped.diagnostics {
            diagnostics.push(diag(error.code, *position, Some(error.field)));
        }
        let version = resource(&value, "service.version");
        let known = version == Some("0.25.0");
        let session = attrs
            .get("session.id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let category = match attrs.get("llm_request.context").and_then(Value::as_str) {
            Some("interaction") => CallCategory::Primary,
            Some("subagent") => CallCategory::SubAgent,
            // Standalone auto-memory work does not establish a parent session.
            Some("standalone")
                if attrs.get("subagent_name").and_then(Value::as_str)
                    == Some("managed-auto-memory-extractor") =>
            {
                CallCategory::Auxiliary
            }
            _ => CallCategory::Unknown,
        };
        let identity = serde_json::to_string(&(trace, span))?;
        let model = attrs
            .get("gen_ai.request.model")
            .and_then(Value::as_str)
            .map(str::to_string);
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("qwen-sdk:{identity}"),
            record_kind: RecordKind::ModelCall,
            schema_version: version.unwrap_or("unknown").into(),
            parser_version: PARSER.into(),
            parse_basis: Some(if known {
                VersionBasis::KnownVersion
            } else {
                VersionBasis::LatestFallback
            }),
            origin_call_id: Some(format!("otel-span:{identity}")),
            attempt_id: attrs
                .get("attempt")
                .and_then(Value::as_i64)
                .filter(|n| *n >= 1)
                .map(|n| n.to_string()),
            session_id: session,
            parent_session_id: None,
            host_application: None,
            agent: "qwen-code".into(),
            call_category: category,
            occurred_at_ms: time,
            observed_at_ms: Some(now),
            source_time: Some(time.to_string()),
            time_basis: TimeBasis::SourceStart,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_attribution: if model.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            model_raw: model,
            model_canonical: None,
            usage: mapped.usage,
            quality: mapped.quality,
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: if attrs.get("success").and_then(Value::as_bool) == Some(false)
                || value.pointer("/status/code").and_then(Value::as_u64) == Some(2)
            {
                Some("error".into())
            } else {
                None
            },
            duration_ms: timestamp(&value["endTime"])
                .and_then(|end| end.checked_sub(time))
                .filter(|n| *n >= 0),
            ttft_ms: attrs
                .get("ttft_ms")
                .and_then(Value::as_i64)
                .filter(|n| *n >= 0),
            attribution_status: if known {
                AttributionStatus::Verified
            } else {
                AttributionStatus::Excluded
            },
            exclusion_reason: (!known).then(|| "qwen_sdk_version_unverified".into()),
            cost: None,
        });
    }
    if read.status == ScanStatus::LineTooLong {
        diagnostics.push(diag("sdk_object_exceeds_cap", read.offset, None));
    }
    let had_errors = (!restart && previous_errors) || !diagnostics.is_empty();
    let completed = read.status == ScanStatus::Complete && !had_errors;
    Ok(ScanOutcome {
        status: read.status,
        cursor: Some(serde_json::to_value(JsonlCursor {
            generation: target.generation,
            offset: read.offset,
            line_number: cursor.map_or(1, |c| c.line_number) + read.objects.len() as u64,
        })?),
        parse_context: Some(
            serde_json::json!({"qwen_sdk_file":true,"had_errors":had_errors,"policy_version":if completed {1}else{0}}),
        ),
        events,
        aggregates: Vec::new(),
        health: if had_errors { "degraded" } else { "active" }.into(),
        diagnostics,
        lines_read: read.objects.len() as u64,
        records_seen,
        reconciliations: Vec::new(),
    })
}
