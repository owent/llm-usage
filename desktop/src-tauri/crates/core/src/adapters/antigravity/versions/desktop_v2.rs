//! Antigravity desktop 2.34.0 generation and completed planner response fields.

use super::gen_metadata_v1::{bounded, iter_fields, WireValue, ANTIGRAVITY_MAX_BLOB_BYTES};
use crate::adapters::{run_policy, usage_map};
use crate::domain::{FieldQuality, TokenQuality, TokenUsage, MIN_PLAUSIBLE_MS};
use crate::error::CoreError;
use rusqlite::Connection;
use std::collections::BTreeMap;

pub const FORMAT_VERSION: &str = "antigravity-desktop-generation-2";

pub(super) fn is_generation_uuid(bytes: &[u8]) -> bool {
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &byte)| match i {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

pub(super) fn response_id(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes)
        .ok()
        .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_whitespace))
}

pub(super) fn usage(bytes: &[u8]) -> Option<(&str, usage_map::MappedUsage)> {
    let mut tokens = TokenUsage::default();
    let mut id = None;
    iter_fields(bytes, |field, value| {
        match (field, value) {
            // Field 1 is the planner's model enum. Field 10 is text output within field 3.
            (2, WireValue::Varint(n)) => tokens.input_uncached = positive(n),
            (3, WireValue::Varint(n)) => tokens.output_total = positive(n),
            (4, WireValue::Varint(n)) => tokens.input_cache_write = positive(n),
            (5, WireValue::Varint(n)) => tokens.input_cache_read = positive(n),
            (9, WireValue::Varint(n)) => tokens.output_reasoning = positive(n),
            (11, WireValue::Bytes(bytes)) => id = response_id(bytes),
            _ => {}
        }
        Some(())
    })?;
    tokens.input_total = tokens
        .input_uncached
        .zip(tokens.input_cache_read)
        .zip(tokens.input_cache_write)
        .and_then(|((input, read), write)| input.checked_add(read)?.checked_add(write))
        .filter(|&sum| sum <= crate::domain::MAX_TOKEN_VALUE);
    let quality = TokenQuality {
        input_uncached: FieldQuality::Reported,
        input_cache_read: FieldQuality::Reported,
        input_cache_write: FieldQuality::Reported,
        input_total: FieldQuality::Derived,
        output_total: FieldQuality::Reported,
        output_reasoning: FieldQuality::Reported,
        ..Default::default()
    };
    Some((id?, usage_map::finish(tokens, quality, Vec::new())))
}

fn positive(value: u64) -> Option<i64> {
    bounded(value).filter(|&n| n > 0)
}

pub(super) type CompletionTimes = BTreeMap<i64, BTreeMap<String, Option<i64>>>;

pub(super) fn completion_times(
    conn: &Connection,
    after_idx: i64,
) -> Result<CompletionTimes, CoreError> {
    let has_steps: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='steps')",
        [],
        |r| r.get(0),
    )?;
    let mut times = CompletionTimes::new();
    if !has_steps {
        return Ok(times);
    }
    let mut stmt = conn.prepare(
        "SELECT metadata FROM steps WHERE step_type=15 AND status=3
         AND typeof(metadata)='blob' AND length(metadata)<=?1",
    )?;
    let mut rows = stmt.query([ANTIGRAVITY_MAX_BLOB_BYTES as i64])?;
    while let Some(row) = rows.next()? {
        run_policy::check()?;
        let rusqlite::types::ValueRef::Blob(blob) = row.get_ref(0)? else {
            continue;
        };
        if let Some((index, id, ms)) = completion(blob).filter(|(index, _, _)| *index > after_idx) {
            times
                .entry(index)
                .or_default()
                .entry(id.to_owned())
                .and_modify(|existing| *existing = None)
                .or_insert(Some(ms));
        }
    }
    Ok(times)
}

fn completion(blob: &[u8]) -> Option<(i64, &str, i64)> {
    let (mut started_at, mut completed_at, mut id, mut index) = (None, None, None, None);
    iter_fields(blob, |field, value| {
        match (field, value) {
            (1, WireValue::Bytes(bytes)) => started_at = timestamp_ms(bytes),
            (7, WireValue::Bytes(bytes)) => completed_at = timestamp_ms(bytes),
            (9, WireValue::Bytes(bytes)) => iter_fields(bytes, |field, value| {
                if let (11, WireValue::Bytes(bytes)) = (field, value) {
                    id = response_id(bytes);
                }
                Some(())
            })?,
            (20, WireValue::Bytes(bytes)) => {
                // The generation index is a protobuf scalar. Zero is omitted from this message.
                let mut generation_index = 0;
                iter_fields(bytes, |field, value| {
                    if let (3, WireValue::Varint(n)) = (field, value) {
                        generation_index = i64::try_from(n).ok()?;
                    }
                    Some(())
                })?;
                index = Some(generation_index);
            }
            _ => {}
        }
        Some(())
    })?;
    let completed_at = completed_at?;
    (started_at? <= completed_at).then_some((index?, id?, completed_at))
}

fn timestamp_ms(blob: &[u8]) -> Option<i64> {
    let (mut seconds, mut nanos) = (None, 0);
    iter_fields(blob, |field, value| {
        match (field, value) {
            (1, WireValue::Varint(n)) => seconds = i64::try_from(n).ok(),
            (2, WireValue::Varint(n)) if n < 1_000_000_000 => nanos = n as i64,
            (2, _) => return None,
            _ => {}
        }
        Some(())
    })?;
    let ms = seconds?.checked_mul(1000)?.checked_add(nanos / 1_000_000)?;
    (MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}
