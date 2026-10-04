//! 有界 JSONL 读取器（V07）。
//!
//! 约定（architecture.md「各输入的增量策略」）：
//! - 游标 = 文件身份 + generation + 完整行字节偏移 + 解析上下文；
//! - 默认单行上限 8 MiB、单块 4 MiB（允许跨块组装行）；超限不静默丢弃，
//!   状态显示位置与原因，允许受控重试；
//! - 半行留待下次（游标不前移）；跨块 UTF-8 正确组装；跳过 UTF-8 BOM；
//! - 坏行隔离：诊断只存错误码与位置（行号/字节偏移），不复制原始行内容；
//! - 截断/同大小替换/改名时重探测；变更检测不限于文件长度。

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// 单行上限初值 8 MiB。
pub const DEFAULT_MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
/// 单块上限初值 4 MiB。
pub const DEFAULT_CHUNK_BYTES: usize = 4 * 1024 * 1024;
/// 身份采样字节数（首/尾各取这么多做内容指纹）。
const SAMPLE_BYTES: usize = 4096;
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// 读取限制。`max_lines` 是确定性行数上限（测试与受控重试用）；
/// `time_budget` 是墙钟超时（单源每轮 30 秒初值）。
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

/// 一条完整行（含字节位置；`end` 为行终止符之后的偏移）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLine {
    pub number: u64,
    pub start: u64,
    pub end: u64,
    pub text: String,
}

/// 坏行诊断（无正文）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadLine {
    pub code: &'static str,
    pub number: u64,
    pub offset: u64,
}

/// 读取停止原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// 读到当前文件尾。
    Eof,
    /// 某完整行超过单行上限：游标停在该行起点，可受控重试（提高上限）。
    LineTooLong { number: u64, offset: u64 },
    /// 达到确定性行数或字节上限。
    LineBudget,
    /// 达到墙钟超时时间。
    TimeBudget,
}

/// 一次增量读取的结果。
#[derive(Debug, Clone)]
pub struct ReadOutcome {
    pub lines: Vec<RawLine>,
    pub bad_lines: Vec<BadLine>,
    /// 下一 unread 偏移（最后一条完整行之后；半行不前移）。
    pub next_offset: u64,
    /// 下一行号（1 起始）。
    pub next_line_number: u64,
    /// 半行字节数（留待下次）。
    pub pending_bytes: u64,
    pub stop: StopReason,
}

/// 从 `start_offset`（完整行边界）读取 JSONL。`first_line_number` 为该偏移处的行号。
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
    // 起点为 0 时跳过 BOM；BOM 不计入行内容但计入字节偏移。
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
            // EOF：carry 中的残余是半行，留待下次。
            outcome.pending_bytes = carry.len() as u64;
            outcome.stop = StopReason::Eof;
            break;
        }
        let mut search_from = carry.len();
        carry.extend_from_slice(&chunk[..n]);
        // 逐条提取完整行；0x0A 不会出现在 UTF-8 多字节序列内，按字节切分安全。
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
        // 无换行符的残余：若已超过单行上限，同样按超长行处理（不等待它继续增长）。
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

/// 文件身份探测：长度、mtime、创建时间与首/尾采样指纹。变更检测不限于长度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileProbe {
    pub len: u64,
    pub mtime_ms: i64,
    pub created_ms: Option<i64>,
    /// 首采样指纹（采样长度 `head_len`）。
    pub head_hash: u64,
    /// 首采样长度：len >= 4096 时固定 4096；小文件为当前长度（增长后不可比）。
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

/// 已存储的扫描状态（source_files 行 + 游标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredFileState {
    pub generation: i64,
    pub len: u64,
    pub created_ms: Option<i64>,
    pub head_hash: u64,
    /// 首采样长度：与探测值不一致时首指纹不可比（小文件增长跨过采样边界）。
    pub head_len: u64,
    pub tail_hash: u64,
    pub cursor_offset: u64,
}

/// 代数裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationDecision {
    /// 同一文件代：从游标继续（含未变化与纯追加）。
    Continue,
    /// 文件被替换/截断/重建：generation+1 从头重扫，解析上下文作废。
    Rescan(&'static str),
}

/// 比较存储状态与当前探测。身份要点：创建时间变化 → 新文件；
/// 长度小于游标 → 截断；同长但首/尾指纹不同 → 同长替换；首指纹不同 → 原地改写。
/// 首采样长度不一致（小文件增长）时首指纹不可比，依靠创建时间与长度单调性裁决。
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

/// JSONL 游标（持久化在 ingestion_checkpoints.cursor_value）。
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
        // 小文件增长：首采样长度变化，指纹不可比，但创建时间一致且长度递增 → 追加。
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
