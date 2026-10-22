//! Validate oversized non-usage records without retaining their bodies.

use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

use serde::Deserialize;

use super::super::jsonl::{self, JsonlLimits, RawLine, ReadOutcome, StopReason};
use super::super::run_policy;

const MAX_NON_USAGE_LINE_BYTES: u64 = 64 * 1024 * 1024;

struct NonUsageEnvelope {
    kind: NonUsageKind,
}

impl<'de> Deserialize<'de> for NonUsageEnvelope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = NonUsageEnvelope;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a Codex record object")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                #[derive(Deserialize)]
                struct Fields {
                    #[serde(rename = "type")]
                    kind: NonUsageKind,
                }
                let fields =
                    Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(NonUsageEnvelope { kind: fields.kind })
            }
        }
        // Derived structs also accept positional arrays; only native objects may be skipped.
        deserializer.deserialize_map(ObjectVisitor)
    }
}

#[derive(Deserialize)]
enum NonUsageKind {
    #[serde(rename = "response_item")]
    ResponseItem,
    #[serde(rename = "world_state")]
    WorldState,
}

/// Read at most one line, checking UTF-8 even in JSON values ignored by serde.
struct SingleLine<R> {
    inner: R,
    bytes: u64,
    complete: bool,
    utf8_tail: Vec<u8>,
    deadline: Option<Instant>,
    timed_out: bool,
    max_bytes: u64,
}

impl<R: Read> Read for SingleLine<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        run_policy::check_io()?;
        if self.complete || buf.is_empty() {
            return Ok(0);
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            self.timed_out = true;
            return Err(std::io::Error::other("non-usage line read timed out"));
        }
        let take = buf.len().min((self.max_bytes - self.bytes) as usize);
        if take == 0 {
            return Ok(0);
        }
        let n = self.inner.read(&mut buf[..take])?;
        let n = match buf[..n].iter().position(|b| *b == b'\n') {
            Some(end) => {
                self.complete = true;
                end + 1
            }
            None => n,
        };
        self.bytes += n as u64;
        self.utf8_tail.extend_from_slice(&buf[..n]);
        match std::str::from_utf8(&self.utf8_tail) {
            Ok(_) => self.utf8_tail.clear(),
            Err(error) if error.error_len().is_none() => {
                let valid = error.valid_up_to();
                self.utf8_tail.drain(..valid);
            }
            Err(_) => return Err(std::io::Error::from(std::io::ErrorKind::InvalidData)),
        }
        Ok(n)
    }
}

/// Keep the ordinary limit for usage, unknown types and malformed records. A complete,
/// valid response_item/world_state may advance one line using bounded streaming reads.
pub(super) fn read_jsonl(
    path: &Path,
    offset: u64,
    number: u64,
    limits: &JsonlLimits,
) -> std::io::Result<ReadOutcome> {
    let mut outcome = jsonl::read_jsonl(path, offset, number, limits)?;
    let StopReason::LineTooLong { number, offset } = outcome.stop else {
        return Ok(outcome);
    };
    let mut file = run_policy::checked_file(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut line = SingleLine {
        inner: file,
        bytes: 0,
        complete: false,
        utf8_tail: Vec::new(),
        deadline: limits.time_budget.map(|budget| Instant::now() + budget / 2),
        timed_out: false,
        max_bytes: MAX_NON_USAGE_LINE_BYTES,
    };
    let parsed = serde_json::from_reader::<_, NonUsageEnvelope>(BufReader::with_capacity(
        32 * 1024,
        &mut line,
    ));
    // Cancellation and source deadlines must reach the framework, including when
    // serde wraps the IO error as a JSON error.
    run_policy::check_io()?;
    if line.timed_out {
        outcome.stop = StopReason::TimeBudget;
    } else if let Ok(envelope) = parsed {
        if line.complete && line.utf8_tail.is_empty() {
            outcome.lines.push(RawLine {
                number,
                start: offset,
                end: offset + line.bytes,
                text: match envelope.kind {
                    NonUsageKind::ResponseItem => r#"{"type":"response_item"}"#,
                    NonUsageKind::WorldState => r#"{"type":"world_state"}"#,
                }
                .to_string(),
            });
            outcome.next_offset = offset + line.bytes;
            outcome.next_line_number = number + 1;
            outcome.pending_bytes = 0;
            // Resume after this complete record in the next bounded scan window.
            outcome.stop = StopReason::LineBudget;
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(bytes: &[u8], buffer: usize, max_bytes: u64) -> bool {
        let mut line = SingleLine {
            inner: std::io::Cursor::new(bytes),
            bytes: 0,
            complete: false,
            utf8_tail: Vec::new(),
            deadline: None,
            timed_out: false,
            max_bytes,
        };
        let parsed = serde_json::from_reader::<_, NonUsageEnvelope>(BufReader::with_capacity(
            buffer, &mut line,
        ));
        parsed.is_ok() && line.complete && line.utf8_tail.is_empty()
    }

    #[test]
    fn validates_full_envelopes_without_retaining_ignored_values() {
        for kind in ["response_item", "world_state"] {
            let text = format!(
                "{{\"payload\":{{\"output\":\"{}😀\\\"\\n\"}},\"type\":\"{kind}\"}}\r\n",
                "x".repeat(32700)
            );
            for buffer in [1, 7, 32 * 1024] {
                assert!(validate(text.as_bytes(), buffer, 64 * 1024));
            }
            let with_next = format!("{text}invalid next line\n");
            assert!(validate(with_next.as_bytes(), 32 * 1024, 64 * 1024));
        }
    }

    #[test]
    fn usage_unknown_malformed_duplicate_and_unterminated_records_stay_blocked() {
        for kind in [
            "token_usage_record",
            "event_msg",
            "compacted",
            "turn_context",
            "session_meta",
            "future_type",
        ] {
            let text = format!("{{\"type\":\"{kind}\",\"payload\":{{}}}}\n");
            assert!(!validate(text.as_bytes(), 7, 1024));
        }
        for text in [
            "{\"type\":\"response_item\",\"payload\":{broken}}\n",
            "{\"type\":\"response_item\",\"payload\":{}} trailing\n",
            "{\"type\":\"response_item\",\"type\":\"token_usage_record\"}\n",
            "{\"type\":\"response_item\"}",
            "[]\n",
            "[\"response_item\"]\n",
            "[\"world_state\"]\n",
        ] {
            assert!(!validate(text.as_bytes(), 7, 1024), "{text}");
        }
        let invalid_utf8 = b"{\"type\":\"response_item\",\"payload\":\"\xff\"}\n";
        assert!(!validate(invalid_utf8, 1, 1024));
        let text = b"{\"type\":\"response_item\",\"payload\":{}}\n";
        assert!(!validate(text, 7, text.len() as u64 - 1));
        assert!(validate(text, 7, text.len() as u64));
    }
}
