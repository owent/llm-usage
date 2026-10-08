//! Bounded JSONL reader (V07).
//!
//! Incremental input rules in architecture.md:
//! - cursors keep file identity, generation, complete-line byte offset and parse context;
//! - default line limit 8 MiB and chunk size 4 MiB; assemble lines across chunks;
//!   report oversized positions/reasons without silent dropping, and permit controlled retries;
//! - leave incomplete lines for the next run without advancing cursors; assemble UTF-8 and skip its BOM;
//! - isolate invalid lines with error codes/line numbers/byte offsets, without copying raw content;
//! - detect truncation, same-size replacement and rename; file length alone does not establish continuity.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Default line limit: 8 MiB.
pub const DEFAULT_MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
/// Default chunk size: 4 MiB.
pub const DEFAULT_CHUNK_BYTES: usize = 4 * 1024 * 1024;
/// Bytes sampled at each end for content fingerprints.
const SAMPLE_BYTES: usize = 4096;
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// max_lines is a deterministic line limit for tests and controlled retries;
/// time_budget is a wall-clock timeout, defaulting to 30 seconds per source run.
#[derive(Debug, Clone)]
pub struct JsonlLimits {
    pub chunk_bytes: usize,
    pub max_line_bytes: usize,
    pub max_lines: Option<u64>,
    pub time_budget: Option<std::time::Duration>,
}

impl Default for JsonlLimits {
    fn default() -> Self {
        JsonlLimits {
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            max_lines: None,
            time_budget: Some(std::time::Duration::from_secs(30)),
        }
    }
}

/// Complete line with byte positions; end is the offset after its terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLine {
    pub number: u64,
    pub start: u64,
    pub end: u64,
    pub text: String,
}

/// Invalid-line diagnostic without message content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadLine {
    pub code: &'static str,
    pub number: u64,
    pub offset: u64,
}

/// Why reading stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Reached the current end of file.
    Eof,
    /// Oversized complete line: keep its start as the cursor and permit retry with a higher limit.
    LineTooLong { number: u64, offset: u64 },
    /// Reached the configured line/byte limit.
    LineBudget,
    /// Reached the wall-clock timeout.
    TimeBudget,
}

/// Incremental read result.
#[derive(Debug, Clone)]
pub struct ReadOutcome {
    pub lines: Vec<RawLine>,
    pub bad_lines: Vec<BadLine>,
    /// Next unread offset, after the last complete line; incomplete lines do not advance it.
    pub next_offset: u64,
    /// Next line number, starting at one.
    pub next_line_number: u64,
    /// Incomplete bytes retained for the next read.
    pub pending_bytes: u64,
    pub stop: StopReason,
}

/// Read from a complete-line start_offset; first_line_number identifies that position.
pub fn read_jsonl(
    path: &Path,
    start_offset: u64,
    first_line_number: u64,
    limits: &JsonlLimits,
) -> std::io::Result<ReadOutcome> {
    read_jsonl_with_byte_budget(path, start_offset, first_line_number, limits, None)
}

pub fn read_jsonl_with_byte_budget(
    path: &Path,
    start_offset: u64,
    first_line_number: u64,
    limits: &JsonlLimits,
    max_bytes: Option<u64>,
) -> std::io::Result<ReadOutcome> {
    let mut file = super::run_policy::checked_file(path)?;
    file.seek(SeekFrom::Start(start_offset))?;
    let started = std::time::Instant::now();
    let read_budget = limits.time_budget.map(|budget| budget / 2);
    let max_bytes = max_bytes.or(Some(
        (32 * 1024 * 1024).max(limits.max_line_bytes as u64 + 1),
    ));
    let mut outcome = ReadOutcome {
        lines: Vec::new(),
        bad_lines: Vec::new(),
        next_offset: start_offset,
        next_line_number: first_line_number,
        pending_bytes: 0,
        stop: StopReason::Eof,
    };
    // At offset zero, skip the BOM as line content while retaining it in byte offsets.
    let mut absolute = start_offset;
    if start_offset == 0 {
        let mut bom = [0u8; 3];
        let n = file.read(&mut bom)?;
        if n == 3 && bom == UTF8_BOM {
            absolute = 3;
            outcome.next_offset = 3;
        } else {
            file.seek(SeekFrom::Start(0))?;
        }
    }
    let mut carry: Vec<u8> = Vec::new();
    let mut carry_start = absolute;
    let mut chunk = vec![0u8; limits.chunk_bytes.max(1)];
    loop {
        if let Some(budget) = read_budget {
            if started.elapsed() >= budget {
                outcome.stop = StopReason::TimeBudget;
                break;
            }
        }
        if let Some(max_lines) = limits.max_lines {
            if outcome.lines.len() as u64 >= max_lines {
                outcome.stop = StopReason::LineBudget;
                break;
            }
        }
        let remaining = if let Some(budget) = max_bytes {
            Some(budget.saturating_sub(file.stream_position()?.saturating_sub(start_offset)))
        } else {
            None
        };
        if remaining == Some(0) {
            outcome.stop = StopReason::LineBudget;
            break;
        }
        let take = remaining.map_or(chunk.len(), |n| chunk.len().min(n as usize));
        let n = file.read(&mut chunk[..take])?;
        if n == 0 {
            // EOF leaves carry as an incomplete line for the next run.
            outcome.pending_bytes = carry.len() as u64;
            outcome.stop = StopReason::Eof;
            break;
        }
        let mut search_from = carry.len();
        carry.extend_from_slice(&chunk[..n]);
        // Split complete lines by byte: 0x0A cannot occur within a UTF-8 multibyte sequence.
        let mut consumed = 0usize;
        while let Some(pos) = carry[search_from..].iter().position(|b| *b == b'\n') {
            super::run_policy::check_io()?;
            let line_end = search_from + pos + 1;
            let line_bytes = &carry[consumed..line_end];
            let line_number = outcome.next_line_number;
            if line_bytes.len() > limits.max_line_bytes {
                outcome.stop = StopReason::LineTooLong {
                    number: line_number,
                    offset: carry_start + consumed as u64,
                };
                carry.drain(..consumed);
                return Ok(finish_carry(outcome, carry_start + consumed as u64, &carry));
            }
            let mut text_bytes = &line_bytes[..line_bytes.len() - 1];
            if text_bytes.last() == Some(&b'\r') {
                text_bytes = &text_bytes[..text_bytes.len() - 1];
            }
            let start = carry_start + consumed as u64;
            let end = carry_start + line_end as u64;
            consumed = line_end;
            search_from = consumed;
            outcome.next_offset = end;
            outcome.next_line_number += 1;
            if text_bytes.is_empty() {
                continue;
            }
            match std::str::from_utf8(text_bytes) {
                Ok(text) => outcome.lines.push(RawLine {
                    number: line_number,
                    start,
                    end,
                    text: text.to_string(),
                }),
                Err(_) => outcome.bad_lines.push(BadLine {
                    code: "invalid_utf8_line",
                    number: line_number,
                    offset: start,
                }),
            }
            if let Some(max_lines) = limits.max_lines {
                if outcome.lines.len() as u64 >= max_lines {
                    carry.drain(..consumed);
                    outcome.stop = StopReason::LineBudget;
                    return Ok(finish_carry(outcome, carry_start + consumed as u64, &carry));
                }
            }
        }
        // Incomplete data above the line limit is already oversized; do not wait for further growth.
        if carry.len() - consumed > limits.max_line_bytes {
            let line_number = outcome.next_line_number;
            outcome.stop = StopReason::LineTooLong {
                number: line_number,
                offset: carry_start + consumed as u64,
            };
            carry.drain(..consumed);
            return Ok(finish_carry(outcome, carry_start + consumed as u64, &carry));
        }
        carry.drain(..consumed);
        carry_start = outcome.next_offset;
    }
    Ok(finish_carry(outcome, carry_start, &carry))
}

fn finish_carry(mut outcome: ReadOutcome, carry_start: u64, carry: &[u8]) -> ReadOutcome {
    outcome.pending_bytes = carry.len() as u64;
    let _ = carry_start;
    outcome
}

/// Probe length, mtime, creation time and prefix/suffix fingerprints, not length alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileProbe {
    pub len: u64,
    pub mtime_ms: i64,
    pub created_ms: Option<i64>,
    /// Prefix fingerprint for head_len sampled bytes.
    pub head_hash: u64,
    /// Prefix length: 4096 for larger files, otherwise current length; changed lengths are not comparable.
    pub head_len: u64,
    pub tail_hash: u64,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn probe_file(path: &Path) -> std::io::Result<FileProbe> {
    let meta = std::fs::metadata(path)?;
    let len = meta.len();
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let created_ms = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    let mut file = super::run_policy::checked_file(path)?;
    let mut head = vec![0u8; SAMPLE_BYTES.min(len as usize)];
    file.read_exact(&mut head)?;
    let head_len = head.len() as u64;
    let head_hash = fnv1a(&head);
    let tail_hash = if len > SAMPLE_BYTES as u64 {
        let mut tail = vec![0u8; SAMPLE_BYTES];
        file.seek(SeekFrom::Start(len - SAMPLE_BYTES as u64))?;
        file.read_exact(&mut tail)?;
        fnv1a(&tail)
    } else {
        head_hash
    };
    Ok(FileProbe {
        len,
        mtime_ms,
        created_ms,
        head_hash,
        head_len,
        tail_hash,
    })
}

/// Stored source_files state and cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredFileState {
    pub generation: i64,
    pub len: u64,
    pub created_ms: Option<i64>,
    pub head_hash: u64,
    /// Prefix hashes are not comparable when sample lengths differ, such as a growing small file.
    pub head_len: u64,
    pub tail_hash: u64,
    pub cursor_offset: u64,
}

/// Decide whether file continuity permits resuming or requires a new generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationDecision {
    /// Same generation: resume from the cursor for unchanged or appended content.
    Continue,
    /// Replacement/truncation/rebuilding requires generation+1, a full reread and discarded parse context.
    Rescan(&'static str),
}

/// Compare stored/probed state: changed creation time indicates a recreated file;
/// length below the cursor means truncation; comparable changed prefix or same-size suffix means replacement.
/// When growing small files change sample length, rely on creation time and monotonic length instead.
pub fn decide_generation(stored: &StoredFileState, probe: &FileProbe) -> GenerationDecision {
    if let (Some(old), Some(new)) = (stored.created_ms, probe.created_ms) {
        if old != new {
            return GenerationDecision::Rescan("recreated");
        }
    }
    if probe.len < stored.cursor_offset {
        return GenerationDecision::Rescan("truncated");
    }
    if stored.head_len == probe.head_len && probe.head_hash != stored.head_hash {
        return GenerationDecision::Rescan("prefix_changed");
    }
    if probe.len == stored.len && probe.tail_hash != stored.tail_hash {
        return GenerationDecision::Rescan("same_size_replaced");
    }
    GenerationDecision::Continue
}

/// JSONL cursor persisted in ingestion_checkpoints.cursor_value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JsonlCursor {
    pub generation: i64,
    pub offset: u64,
    pub line_number: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_decisions() {
        let stored = StoredFileState {
            generation: 0,
            len: 100,
            created_ms: Some(10),
            head_hash: 1,
            head_len: 100,
            tail_hash: 2,
            cursor_offset: 100,
        };
        let same = FileProbe {
            len: 150,
            mtime_ms: 20,
            created_ms: Some(10),
            head_hash: 1,
            head_len: 150,
            tail_hash: 3,
        };
        // A growing small file changes prefix length; unchanged creation time and increasing length permit append.
        assert_eq!(
            decide_generation(&stored, &same),
            GenerationDecision::Continue
        );
        let grown_stable_sample = FileProbe {
            head_len: 100,
            ..same
        };
        assert_eq!(
            decide_generation(&stored, &grown_stable_sample),
            GenerationDecision::Continue
        );
        let truncated = FileProbe { len: 50, ..same };
        assert_eq!(
            decide_generation(&stored, &truncated),
            GenerationDecision::Rescan("truncated")
        );
        let recreated = FileProbe {
            created_ms: Some(11),
            ..same
        };
        assert_eq!(
            decide_generation(&stored, &recreated),
            GenerationDecision::Rescan("recreated")
        );
        let prefix = FileProbe {
            head_hash: 9,
            head_len: 100,
            ..same
        };
        assert_eq!(
            decide_generation(&stored, &prefix),
            GenerationDecision::Rescan("prefix_changed")
        );
        let same_size = FileProbe {
            len: 100,
            head_len: 100,
            tail_hash: 8,
            ..same
        };
        assert_eq!(
            decide_generation(&stored, &same_size),
            GenerationDecision::Rescan("same_size_replaced")
        );
    }
}
