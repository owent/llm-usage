//! Antigravity conversations/&lt;uuid&gt;.db parser (gen_metadata_v1,
//! parser version antigravity-gen-metadata-1).
//!
//! Format reference: third-party reverse engineering in pinned tokscale commit 1d9a939,
//! sessions/antigravity_cli.rs:20-60,313-321,466-556. Google's closed-source product has
//! no local installation or native samples here; the protobuf layout is reverse-engineered.
//! - CLI path: ~/.gemini/antigravity-cli/conversations/<uuid>.db; environment variable
//!   GEMINI_CLI_HOME redirects the Gemini root. Extensions use
//!   ~/.gemini/antigravity/conversations/*.db. They share Gemini CLI's root
//!   (~/.gemini), with separate subdirectories and discovery rules.
//! - SQLite tables: gen_metadata(idx, data BLOB protobuf),
//!   trajectory_metadata_blob, and steps.
//! - Reverse-engineered protobuf fields: gen_metadata #1 is the chatModel message
//!   (#19 responseModel machine ID, #21 display name, #9 time container);
//!   #4 is usage: #1 fixed system prompt varint, #2 new input varint,
//!   #5 cacheRead, #9 output, #10 thinking, #11 responseId string
//!   used for deduplication. Input=#1+#2, including the fixed system prompt.
//! - Timestamp: agy ≤1.1.17 uses #9.#4 (protobuf Timestamp: #1 seconds varint,
//!   #2 nanoseconds). That field disappears in 1.1.18. The issue's suggested 8-byte #10 encoding
//!   and the complex steps-table fallback are not implemented. Rows without #9.#4
//!   are skipped with diagnostics; do not invent timestamps.
//! - gemini-default is a routing label; do not use it as a model ID.
//! - Unverified cache_write remains Unknown; unverified cost remains None.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::ANTIGRAVITY_FORMAT_VERSION;

pub const ANTIGRAVITY_PARSER_VERSION: &str = "antigravity-gen-metadata-1";
/// Maximum size of one protobuf blob in the data column.
pub const ANTIGRAVITY_MAX_BLOB_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct AntigravityCursor {
    generation: i64,
    /// Highest processed idx; responseId/idx event keys make repeated reads idempotent.
    last_idx: i64,
}

fn diag(code: &str, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

// Protobuf wire reader for varints and length-delimited fields.

#[derive(Debug, Clone, Copy, PartialEq)]
enum WireValue<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

/// Visit fields in one message, skipping fixed-width values. Unsupported wire types, including groups,
/// return None, rejecting the message.
fn iter_fields<'a>(
    data: &'a [u8],
    mut visit: impl FnMut(u64, WireValue<'a>) -> Option<()>,
) -> Option<()> {
    let mut pos = 0usize;
    while pos < data.len() {
        let (tag, consumed) = read_varint(&data[pos..])?;
        pos += consumed;
        let field = tag >> 3;
        let wire = (tag & 0x7) as u8;
        match wire {
            0 => {
                let (value, consumed) = read_varint(&data[pos..])?;
                pos += consumed;
                visit(field, WireValue::Varint(value))?;
            }
            2 => {
                let (len, consumed) = read_varint(&data[pos..])?;
                pos += consumed;
                let len = usize::try_from(len).ok()?;
                let end = pos.checked_add(len)?;
                if end > data.len() {
                    return None;
                }
                visit(field, WireValue::Bytes(&data[pos..end]))?;
                pos = end;
            }
            // Skip unused 64/32-bit fixed-width fields after checking bounds.
            1 => {
                let end = pos.checked_add(8)?;
                if end > data.len() {
                    return None;
                }
                pos = end;
            }
            5 => {
                let end = pos.checked_add(4)?;
                if end > data.len() {
                    return None;
                }
                pos = end;
            }
            _ => return None,
        }
    }
    Some(())
}

fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    for (i, byte) in data.iter().enumerate().take(10) {
        // The tenth byte must be 0 or 1 for a protobuf u64 varint. Reject larger bytes
        // without truncating high bits and accepting corrupted data.
        if i == 9 && byte > &1 {
            return None;
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

/// Parsed fields from one gen_metadata.data row.
struct GenRow {
    timestamp_ms: Option<i64>,
    fixed_system: Option<i64>,
    new_input: Option<i64>,
    cache_read: Option<i64>,
    output: Option<i64>,
    thinking: Option<i64>,
    response_id: Option<String>,
    response_model: Option<String>,
}

fn bounded(v: u64) -> Option<i64> {
    i64::try_from(v)
        .ok()
        .filter(|n| (0..=crate::domain::MAX_TOKEN_VALUE).contains(n))
}

fn parse_gen_row(data: &[u8]) -> Option<GenRow> {
    let mut row = GenRow {
        timestamp_ms: None,
        fixed_system: None,
        new_input: None,
        cache_read: None,
        output: None,
        thinking: None,
        response_id: None,
        response_model: None,
    };
    iter_fields(data, |field, value| {
        match (field, value) {
            // #1 chatModel message.
            (1, WireValue::Bytes(chat_model)) => {
                iter_fields(chat_model, |sub, sub_value| {
                    match (sub, sub_value) {
                        (19, WireValue::Bytes(model)) => {
                            row.response_model = Some(String::from_utf8_lossy(model).to_string());
                            Some(())
                        }
                        // #9 time container (agy ≤1.1.17).
                        (9, WireValue::Bytes(time_container)) => {
                            iter_fields(time_container, |t, tv| match (t, tv) {
                                (4, WireValue::Bytes(ts)) => {
                                    // protobuf Timestamp: #1 seconds, #2 nanoseconds; this parser reads seconds.
                                    let mut seconds: Option<u64> = None;
                                    iter_fields(ts, |s, sv| {
                                        if let (1, WireValue::Varint(v)) = (s, sv) {
                                            seconds = Some(v);
                                        }
                                        Some(())
                                    })?;
                                    if let Some(secs) = seconds {
                                        // Overflowing or out-of-range seconds produce no usable timestamp,
                                        // as with the 1.1.18+ layout; they do not invalidate the protobuf structure.
                                        if let Some(ms) = i64::try_from(secs)
                                            .ok()
                                            .and_then(|s| s.checked_mul(1000))
                                            .filter(|ms| {
                                                (crate::domain::MIN_PLAUSIBLE_MS
                                                    ..=4_102_444_800_000)
                                                    .contains(ms)
                                            })
                                        {
                                            row.timestamp_ms = Some(ms);
                                        }
                                    }
                                    Some(())
                                }
                                _ => Some(()),
                            })?;
                            Some(())
                        }
                        _ => Some(()),
                    }
                })?;
                Some(())
            }
            // #4 usage message.
            (4, WireValue::Bytes(usage)) => {
                iter_fields(usage, |u, uv| {
                    match (u, uv) {
                        (1, WireValue::Varint(v)) => row.fixed_system = bounded(v),
                        (2, WireValue::Varint(v)) => row.new_input = bounded(v),
                        (5, WireValue::Varint(v)) => row.cache_read = bounded(v),
                        (9, WireValue::Varint(v)) => row.output = bounded(v),
                        (10, WireValue::Varint(v)) => row.thinking = bounded(v),
                        (11, WireValue::Bytes(id)) => {
                            row.response_id = Some(String::from_utf8_lossy(id).to_string());
                        }
                        _ => {}
                    }
                    Some(())
                })?;
                Some(())
            }
            _ => Some(()),
        }
    })?;
    Some(row)
}

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // Resume by idx after the per-round limit. The old cursor stayed at zero, so repeated
    // BudgetExhausted scans never reached later rows. Event keys include responseId/idx;
    // upserts remain idempotent for replayed rows before cursor advancement.
    let last_idx = if target.rescan {
        0
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<AntigravityCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .map(|c| c.last_idx)
            .unwrap_or(0)
    };
    let conn = rusqlite::Connection::open_with_flags(
        &target.path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(CoreError::Sqlite)?;
    crate::adapters::run_policy::install_sqlite_control(&conn)?;
    conn.busy_timeout(std::time::Duration::from_millis(150))
        .map_err(CoreError::Sqlite)?;
    let mut stmt = conn
        .prepare("SELECT idx, data FROM gen_metadata WHERE idx > ?1 ORDER BY idx LIMIT ?2")
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![last_idx, MAX_ROWS_PER_ROUND + 1], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })
        .map_err(CoreError::Sqlite)?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    let mut no_timestamp: u64 = 0;
    let mut max_idx = last_idx;
    let mut seen_response_ids: std::collections::BTreeSet<String> = Default::default();
    for row in rows {
        crate::adapters::run_policy::check()?;
        // Skip individual SQLite dynamic-type errors without stopping the scan.
        let (idx, data) = match row {
            Ok(r) => r,
            Err(e) => {
                records_seen += 1;
                diagnostics.push(diag(
                    "row_read_failed",
                    "gen_metadata",
                    &format!("row read failed: {e}; row skipped"),
                ));
                continue;
            }
        };
        records_seen += 1;
        max_idx = max_idx.max(idx);
        if data.len() > ANTIGRAVITY_MAX_BLOB_BYTES {
            diagnostics.push(diag(
                "blob_exceeds_cap",
                &format!("gen_metadata:{idx}"),
                "protobuf blob exceeds the 4 MiB cap; row skipped",
            ));
            continue;
        }
        let Some(parsed) = parse_gen_row(&data) else {
            diagnostics.push(diag(
                "protobuf_unparseable",
                &format!("gen_metadata:{idx}"),
                "protobuf wire data malformed; row skipped",
            ));
            continue;
        };
        // No #9.#4 timestamp, including the agy 1.1.18+ layout: skip without inventing a time.
        let Some(occurred_ms) = parsed.timestamp_ms else {
            no_timestamp += 1;
            continue;
        };
        let input_total = parsed
            .fixed_system
            .zip(parsed.new_input)
            .and_then(|(a, b)| a.checked_add(b));
        if input_total.is_none()
            && parsed.output.is_none()
            && parsed.cache_read.is_none()
            && parsed.thinking.is_none()
        {
            continue;
        }
        let response_key = parsed
            .response_id
            .clone()
            .unwrap_or_else(|| format!("idx-{idx}"));
        // Deduplicate responseId within this scan, matching the third-party reader's approach.
        if parsed.response_id.is_some()
            && !seen_response_ids.insert(parsed.response_id.clone().unwrap())
        {
            continue;
        }
        // gemini-default is a routing label; missing responseModel leaves the model unknown.
        let model_raw = parsed
            .response_model
            .filter(|m| !m.is_empty() && m != "gemini-default");
        let mapped = crate::adapters::usage_map::finish(
            crate::domain::TokenUsage {
                input_uncached: None,
                input_cache_read: parsed.cache_read,
                input_cache_write: None,
                input_total,
                output_total: parsed.output,
                output_reasoning: parsed.thinking,
                total_tokens: None,
                source_total: None,
            },
            // Present values need appropriate quality markers. All-Unknown quality would fail ingest's
            // value/quality consistency check in domain.rs and prevent import.
            crate::domain::TokenQuality {
                input_cache_read: crate::domain::FieldQuality::Reported,
                input_total: crate::domain::FieldQuality::Reported,
                output_total: crate::domain::FieldQuality::Reported,
                output_reasoning: crate::domain::FieldQuality::Reported,
                ..Default::default()
            },
            Vec::new(),
        );
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("antigravity:{}:{response_key}", target.file_identity),
            record_kind: RecordKind::ModelCall,
            schema_version: ANTIGRAVITY_FORMAT_VERSION.to_string(),
            parser_version: ANTIGRAVITY_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: parsed
                .response_id
                .as_deref()
                .map(|id| format!("antigravity-response:{id}")),
            attempt_id: None,
            session_id: None,
            parent_session_id: None,
            host_application: None,
            agent: "antigravity".to_string(),
            call_category: CallCategory::Primary,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: Some(occurred_ms.to_string()),
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: None,
            model_raw,
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
        });
    }
    if no_timestamp > 0 {
        diagnostics.push(diag(
            "rows_without_verifiable_timestamp",
            "gen_metadata",
            &format!(
                "{no_timestamp} rows lack the ≤1.1.17 #9.#4 timestamp (1.1.18+ layout); skipped, no fabricated timing"
            ),
        ));
    }
    let hit_cap = records_seen > MAX_ROWS_PER_ROUND as u64;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(AntigravityCursor {
            generation: target.generation,
            last_idx: max_idx,
        })?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn field_bytes(field: u64, payload: &[u8]) -> Vec<u8> {
        let mut out = encode_varint((field << 3) | 2);
        out.extend(encode_varint(payload.len() as u64));
        out.extend_from_slice(payload);
        out
    }

    fn field_varint(field: u64, v: u64) -> Vec<u8> {
        let mut out = encode_varint(field << 3);
        out.extend(encode_varint(v));
        out
    }

    #[test]
    fn parses_usage_and_timestamp() {
        // Build Timestamp(seconds=1_790_000_000) as #4 bytes{#1 varint}.
        // #9 time container { #4 Timestamp { #1 seconds } }.
        let ts = field_varint(1, 1_790_000_000);
        let ts_container = field_bytes(9, &field_bytes(4, &ts));
        // responseModel #19 is a string encoded as bytes.
        let mut chat_model = field_bytes(19, b"gemini-2.6-pro");
        chat_model.extend_from_slice(&ts_container);
        // usage #4: #1 fixed prompt 1132, #2 new input 100, #5 cacheRead 40, #9 output 20,
        // #10 thinking 5 and #11 responseId "resp-1".
        let usage = [
            field_varint(1, 1132),
            field_varint(2, 100),
            field_varint(5, 40),
            field_varint(9, 20),
            field_varint(10, 5),
            field_bytes(11, b"resp-1"),
        ]
        .concat();
        let row = [field_bytes(1, &chat_model), field_bytes(4, &usage)].concat();
        let parsed = parse_gen_row(&row).unwrap();
        assert_eq!(parsed.timestamp_ms, Some(1_790_000_000_000));
        assert_eq!(parsed.fixed_system, Some(1132));
        assert_eq!(parsed.new_input, Some(100));
        assert_eq!(parsed.cache_read, Some(40));
        assert_eq!(parsed.response_id.as_deref(), Some("resp-1"));
        assert_eq!(parsed.response_model.as_deref(), Some("gemini-2.6-pro"));
    }

    #[test]
    fn rejects_truncated() {
        assert!(parse_gen_row(&[0x0a, 0xff]).is_none());
    }

    #[test]
    fn varint_tenth_byte_high_bits_rejected() {
        // Reject ten continuation bytes.
        assert_eq!(read_varint(&[0xff; 10]), None);
        // Reject a tenth byte >1 (u64 overflow), without truncating high bits.
        let bad = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02];
        assert_eq!(read_varint(&bad), None);
        // A tenth byte of 1 is valid for u64::MAX.
        let max = encode_varint(u64::MAX);
        assert_eq!(read_varint(&max), Some((u64::MAX, 10)));
        // Ordinary values remain readable.
        assert_eq!(read_varint(&encode_varint(300)), Some((300, 2)));
    }

    #[test]
    fn timestamp_seconds_overflow_is_no_timestamp_not_malformed() {
        // Seconds that overflow when multiplied by 1000 leave timestamp_ms=None,
        // while the protobuf structure remains valid and parsing succeeds.
        let ts = field_varint(1, u64::MAX);
        let ts_container = field_bytes(9, &field_bytes(4, &ts));
        let chat_model = ts_container;
        let usage = [field_varint(1, 10), field_varint(9, 20)].concat();
        let row = [field_bytes(1, &chat_model), field_bytes(4, &usage)].concat();
        let parsed = parse_gen_row(&row).expect("row structure is valid");
        assert_eq!(parsed.timestamp_ms, None);
        assert_eq!(parsed.output, Some(20));
    }
}
