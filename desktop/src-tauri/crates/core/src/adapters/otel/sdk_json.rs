//! Bounded framing for concatenated SDK JSON objects, including pretty printing.
//! No raw object is kept in a checkpoint; incomplete objects restart at their boundary.
use crate::adapters::{framework::ScanStatus, jsonl::JsonlLimits, run_policy};
use crate::error::CoreError;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

pub(super) struct ReadResult {
    pub objects: Vec<(u64, Vec<u8>)>,
    pub offset: u64,
    pub status: ScanStatus,
}

pub(super) fn read(
    path: &Path,
    offset: u64,
    limits: &JsonlLimits,
    byte_budget: u64,
) -> Result<ReadResult, CoreError> {
    let mut file = run_policy::checked_file(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut reader = BufReader::with_capacity(limits.chunk_bytes.clamp(1, 64 * 1024), file);
    let deadline = limits.time_budget.map(|d| std::time::Instant::now() + d);
    let mut result = ReadResult {
        objects: Vec::new(),
        offset,
        status: ScanStatus::Complete,
    };
    let mut object = Vec::new();
    let mut position = offset;
    let mut start = offset;
    let mut depth = 0_i64;
    let mut in_string = false;
    let mut escaped = false;
    loop {
        run_policy::check()?;
        if result.objects.len() as u64 >= limits.max_lines.unwrap_or(50_000).min(50_000)
            || position.saturating_sub(offset) >= byte_budget
            || deadline.is_some_and(|d| std::time::Instant::now() >= d)
        {
            result.status = ScanStatus::BudgetExhausted;
            return Ok(result);
        }
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if !object.is_empty() {
                result.status = ScanStatus::Pending;
            }
            return Ok(result);
        }
        let mut consumed = 0;
        for &byte in buffer {
            consumed += 1;
            position += 1;
            if object.is_empty() {
                if byte.is_ascii_whitespace() {
                    result.offset = position;
                    continue;
                }
                start = position - 1;
                depth = 0;
                in_string = false;
                escaped = false;
            }
            object.push(byte);
            if object.len() > limits.max_line_bytes {
                result.offset = start;
                result.status = ScanStatus::LineTooLong;
                return Ok(result);
            }
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
            } else {
                match byte {
                    b'"' => in_string = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => depth -= 1,
                    _ => {}
                }
            }
            // A non-container malformed prefix is bounded to its physical line.
            let malformed_line =
                !matches!(object.first(), Some(b'{') | Some(b'[')) && byte == b'\n';
            if (depth == 0 && !in_string && matches!(byte, b'}' | b']')) || malformed_line {
                result.objects.push((start, std::mem::take(&mut object)));
                result.offset = position;
                if limits
                    .max_lines
                    .is_some_and(|n| result.objects.len() as u64 >= n)
                {
                    break;
                }
            }
        }
        reader.consume(consumed);
    }
}
