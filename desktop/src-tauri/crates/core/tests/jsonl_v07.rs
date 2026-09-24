//! V07：JSONL 读取器边界 —— 半行、跨块 UTF-8、超长行、BOM、坏行、
//! 文件轮转/截断/同长替换/改名重探测。期望逐条对应验收表。

mod common;

use common::TempDir;
use llm_usage_core::adapters::jsonl::{
    decide_generation, probe_file, read_jsonl, GenerationDecision, JsonlLimits, StopReason,
    StoredFileState,
};

fn limits(chunk: usize, max_line: usize) -> JsonlLimits {
    JsonlLimits {
        chunk_bytes: chunk,
        max_line_bytes: max_line,
        max_lines: None,
        time_budget: None,
    }
}

fn write(dir: &TempDir, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn half_line_waits_and_cursor_does_not_advance() {
    let dir = TempDir::new("v07-half");
    let path = write(&dir, "a.jsonl", b"{\"n\":1}\n{\"n\":2");
    let first = read_jsonl(&path, 0, 1, &limits(4096, 1024)).unwrap();
    assert_eq!(first.lines.len(), 1);
    assert_eq!(first.lines[0].text, "{\"n\":1}");
    assert_eq!(first.next_offset, 8, "cursor stays at the half-line start");
    assert_eq!(first.pending_bytes, 6);
    assert_eq!(first.stop, StopReason::Eof);

    // 追加完成半行后，从同一游标继续读到完整行。
    std::fs::write(&path, b"{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n").unwrap();
    let second = read_jsonl(
        &path,
        first.next_offset,
        first.next_line_number,
        &limits(4096, 1024),
    )
    .unwrap();
    assert_eq!(second.lines.len(), 2);
    assert_eq!(second.lines[0].text, "{\"n\":2}");
    assert_eq!(second.lines[0].number, 2);
    assert_eq!(second.lines[1].text, "{\"n\":3}");
    assert_eq!(second.pending_bytes, 0);
}

#[test]
fn multibyte_utf8_across_chunk_boundary() {
    let dir = TempDir::new("v07-utf8");
    // 构造跨块多字节字符：小块大小必然切开 UTF-8 序列。
    let mut bytes = Vec::new();
    bytes.extend_from_slice("{\"s\":\"中文🦀\"}\n".as_bytes());
    bytes.extend_from_slice("{\"s\":\"é\"}\n".as_bytes());
    let path = write(&dir, "u.jsonl", &bytes);
    let out = read_jsonl(&path, 0, 1, &limits(4, 1024)).unwrap();
    assert_eq!(out.lines.len(), 2);
    assert_eq!(out.lines[0].text, "{\"s\":\"中文🦀\"}");
    assert_eq!(out.lines[1].text, "{\"s\":\"é\"}");
    assert!(out.bad_lines.is_empty());
}

#[test]
fn oversized_line_is_limited_error_not_silent_drop() {
    let dir = TempDir::new("v07-long");
    let long_line = format!("{{\"pad\":\"{}\"}}\n", "x".repeat(100));
    let mut bytes = b"{\"n\":1}\n".to_vec();
    bytes.extend_from_slice(long_line.as_bytes());
    bytes.extend_from_slice(b"{\"n\":3}\n");
    let path = write(&dir, "l.jsonl", &bytes);

    let out = read_jsonl(&path, 0, 1, &limits(4096, 64)).unwrap();
    assert_eq!(
        out.lines.len(),
        1,
        "complete lines before the oversized one are kept"
    );
    match out.stop {
        StopReason::LineTooLong { number, offset } => {
            assert_eq!(number, 2);
            assert_eq!(offset, 8);
        }
        other => panic!("expected LineTooLong, got {other:?}"),
    }
    assert_eq!(
        out.next_offset, 8,
        "cursor held at the oversized line start"
    );

    // 受控重试：提高上限后同一游标可读完全部。
    let retry = read_jsonl(
        &path,
        out.next_offset,
        out.next_line_number,
        &limits(4096, 1024),
    )
    .unwrap();
    assert_eq!(retry.lines.len(), 2);
    assert!(retry.lines[0].text.contains("xxx"));
    assert_eq!(retry.lines[1].text, "{\"n\":3}");
}

#[test]
fn utf8_bom_is_skipped_with_consistent_offsets() {
    let dir = TempDir::new("v07-bom");
    let path = write(&dir, "b.jsonl", b"\xEF\xBB\xBF{\"a\":1}\n{\"b\":2}\n");
    let out = read_jsonl(&path, 0, 1, &limits(4096, 1024)).unwrap();
    assert_eq!(out.lines.len(), 2);
    assert_eq!(out.lines[0].text, "{\"a\":1}");
    assert_eq!(out.lines[0].start, 3, "BOM bytes count toward offsets");
    // 从首行之后续读不重复 BOM 处理。
    let cont = read_jsonl(&path, out.lines[0].end, 2, &limits(4096, 1024)).unwrap();
    assert_eq!(cont.lines.len(), 1);
    assert_eq!(cont.lines[0].text, "{\"b\":2}");
}

#[test]
fn invalid_utf8_line_is_isolated_without_content() {
    let dir = TempDir::new("v07-bad");
    let mut bytes = b"{\"n\":1}\n".to_vec();
    bytes.extend_from_slice(b"\xFF\xFE invalid \x80\n");
    bytes.extend_from_slice(b"{\"n\":3}\n");
    let path = write(&dir, "bad.jsonl", &bytes);
    let out = read_jsonl(&path, 0, 1, &limits(4096, 1024)).unwrap();
    assert_eq!(out.lines.len(), 2, "good lines on both sides survive");
    assert_eq!(out.bad_lines.len(), 1);
    let bad = &out.bad_lines[0];
    assert_eq!(bad.code, "invalid_utf8_line");
    assert_eq!(bad.number, 2);
    // 诊断只有错误码与位置；BadLine 类型本身不携带正文。
    let debug = format!("{bad:?}");
    assert!(!debug.contains("invalid "));
}

#[test]
fn crlf_and_empty_lines() {
    let dir = TempDir::new("v07-crlf");
    let path = write(&dir, "c.jsonl", b"{\"a\":1}\r\n\r\n{\"b\":2}\r\n");
    let out = read_jsonl(&path, 0, 1, &limits(4096, 1024)).unwrap();
    assert_eq!(out.lines.len(), 2);
    assert_eq!(out.lines[0].text, "{\"a\":1}");
    assert_eq!(out.lines[1].text, "{\"b\":2}");
    assert_eq!(
        out.lines[1].number, 3,
        "empty line still consumes a line number"
    );
}

#[test]
fn line_budget_stops_and_resumes() {
    let dir = TempDir::new("v07-budget");
    let path = write(&dir, "n.jsonl", b"1\n2\n3\n4\n");
    let mut capped = limits(4096, 1024);
    capped.max_lines = Some(2);
    let first = read_jsonl(&path, 0, 1, &capped).unwrap();
    assert_eq!(first.lines.len(), 2);
    assert_eq!(first.stop, StopReason::LineBudget);
    let second = read_jsonl(&path, first.next_offset, first.next_line_number, &capped).unwrap();
    assert_eq!(second.lines.len(), 2);
    assert_eq!(second.lines[0].text, "3");
}

#[test]
fn probe_detects_truncation_and_same_size_replacement() {
    let dir = TempDir::new("v07-probe");
    let path = write(&dir, "p.jsonl", b"aaaaaaaaaaaaaaaa\n");
    let probe1 = probe_file(&path).unwrap();
    let stored = StoredFileState {
        generation: 0,
        len: probe1.len,
        created_ms: probe1.created_ms,
        head_hash: probe1.head_hash,
        head_len: probe1.head_len,
        tail_hash: probe1.tail_hash,
        cursor_offset: probe1.len,
    };
    // 纯追加 → Continue。
    std::fs::write(&path, b"aaaaaaaaaaaaaaaa\nbbbb\n").unwrap();
    let probe2 = probe_file(&path).unwrap();
    assert_eq!(
        decide_generation(&stored, &probe2),
        GenerationDecision::Continue
    );
    // 截断（短于游标）→ Rescan。
    std::fs::write(&path, b"aaaa\n").unwrap();
    let probe3 = probe_file(&path).unwrap();
    assert_eq!(
        decide_generation(&stored, &probe3),
        GenerationDecision::Rescan("truncated")
    );
}

#[test]
fn same_size_replacement_is_detected_by_content_not_length() {
    let dir = TempDir::new("v07-replace");
    let path = write(&dir, "r.jsonl", b"{\"session\":\"one\",\"usage\":100}\n");
    let probe1 = probe_file(&path).unwrap();
    let stored = StoredFileState {
        generation: 0,
        len: probe1.len,
        created_ms: probe1.created_ms,
        head_hash: probe1.head_hash,
        head_len: probe1.head_len,
        tail_hash: probe1.tail_hash,
        cursor_offset: probe1.len,
    };
    // 同长替换（内容不同、长度一致）。
    std::fs::write(&path, b"{\"session\":\"two\",\"usage\":200}\n").unwrap();
    let probe2 = probe_file(&path).unwrap();
    assert_eq!(probe2.len, probe1.len, "test precondition: same length");
    let decision = decide_generation(&stored, &probe2);
    assert!(
        matches!(decision, GenerationDecision::Rescan(_)),
        "same-size replacement must trigger re-detection, got {decision:?}"
    );
}
