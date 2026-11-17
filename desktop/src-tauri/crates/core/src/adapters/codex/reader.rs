//! Stream oversized records without retaining non-usage bodies.

use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::super::jsonl::{self, JsonlLimits, RawLine, ReadOutcome, StopReason};
use super::super::run_policy;

const MAX_OVERSIZED_LINE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize)]
#[serde(transparent)]
struct JsonObject<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for JsonObject<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
            type Value = T;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a native JSON object")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }
        deserializer
            .deserialize_map(ObjectVisitor::<T>(std::marker::PhantomData))
            .map(JsonObject)
    }
}

#[derive(Default, Deserialize, Serialize)]
struct CarriedUsage {
    #[serde(default)]
    input_tokens: Option<i64>,
    #[serde(default)]
    cached_input_tokens: Option<i64>,
    #[serde(default)]
    cache_write_input_tokens: Option<i64>,
    #[serde(default)]
    output_tokens: Option<i64>,
    #[serde(default)]
    reasoning_output_tokens: Option<i64>,
    #[serde(default)]
    total_tokens: Option<i64>,
}

#[derive(Default, Deserialize, Serialize)]
struct CarriedUsageRecord {
    #[serde(default)]
    response_id: Option<String>,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    usage: Option<JsonObject<CarriedUsage>>,
}

#[derive(Default, Serialize)]
struct OversizedPayload {
    latest_token_usage_record: Option<JsonObject<CarriedUsageRecord>>,
}

#[derive(Serialize)]
struct CompactedLine {
    #[serde(rename = "type")]
    record_type: &'static str,
    timestamp: Option<String>,
    payload: OversizedPayload,
}

/// Ignore oversized payload fields, retaining only compaction's usage-bearing record.
/// Non-object payloads remain valid ignored JSON rather than becoming record errors.
impl<'de> Deserialize<'de> for OversizedPayload {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier)]
        enum Field {
            #[serde(rename = "latest_token_usage_record")]
            LatestTokenUsageRecord,
            #[serde(other)]
            Other,
        }

        struct PayloadVisitor;
        impl<'de> serde::de::Visitor<'de> for PayloadVisitor {
            type Value = OversizedPayload;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an ignored Codex payload")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut latest = None;
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::LatestTokenUsageRecord => {
                            latest = map.next_value()?;
                        }
                        Field::Other => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(OversizedPayload {
                    latest_token_usage_record: latest,
                })
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(OversizedPayload::default())
            }

            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
            fn visit_string<E: serde::de::Error>(self, _: String) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(OversizedPayload::default())
            }
        }
        deserializer.deserialize_any(PayloadVisitor)
    }
}

struct OversizedEnvelope {
    kind: OversizedKind,
    timestamp: Option<String>,
    payload: OversizedPayload,
}

impl<'de> Deserialize<'de> for OversizedEnvelope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = OversizedEnvelope;

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
                    kind: OversizedKind,
                    #[serde(default)]
                    timestamp: Option<String>,
                    #[serde(default)]
                    payload: OversizedPayload,
                }
                let fields =
                    Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(OversizedEnvelope {
                    kind: fields.kind,
                    timestamp: fields.timestamp,
                    payload: fields.payload,
                })
            }
        }
        // Derived structs also accept positional arrays; only native objects may advance.
        deserializer.deserialize_map(ObjectVisitor)
    }
}

#[derive(Deserialize)]
enum OversizedKind {
    #[serde(rename = "response_item")]
    ResponseItem,
    #[serde(rename = "world_state")]
    WorldState,
    #[serde(rename = "compacted")]
    Compacted,
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

/// Keep the ordinary limit for usage, unknown types and malformed records. Complete,
/// valid response_item/world_state records retain only their type. Compacted records
/// retain their timestamp and latest_token_usage_record for boundary usage recovery.
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
        max_bytes: MAX_OVERSIZED_LINE_BYTES,
    };
    let parsed = serde_json::from_reader::<_, OversizedEnvelope>(BufReader::with_capacity(
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
            let text = match envelope.kind {
                OversizedKind::ResponseItem => r#"{"type":"response_item"}"#.to_string(),
                OversizedKind::WorldState => r#"{"type":"world_state"}"#.to_string(),
                OversizedKind::Compacted => serde_json::to_string(&CompactedLine {
                    record_type: "compacted",
                    timestamp: envelope.timestamp,
                    payload: envelope.payload,
                })
                .map_err(std::io::Error::other)?,
            };
            outcome.lines.push(RawLine {
                number,
                start: offset,
                end: offset + line.bytes,
                text,
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
        let parsed = serde_json::from_reader::<_, OversizedEnvelope>(BufReader::with_capacity(
            buffer, &mut line,
        ));
        parsed.is_ok() && line.complete && line.utf8_tail.is_empty()
    }

    #[test]
    fn validates_full_envelopes_without_retaining_ignored_values() {
        for kind in ["response_item", "world_state", "compacted"] {
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
        for payload in ["null", "[]", "\"ignored\"", "1", "true"] {
            let text = format!("{{\"payload\":{payload},\"type\":\"response_item\"}}\n");
            assert!(validate(text.as_bytes(), 7, 1024), "{text}");
        }
    }

    #[test]
    fn compacted_discards_unknown_carried_fields_without_expanding_sanitized_output() {
        let usage = serde_json::json!({"input_tokens":100,"cached_input_tokens":50,
            "cache_write_input_tokens":0,"output_tokens":20,"reasoning_output_tokens":5,
            "total_tokens":120,"ignored_usage_text":"x".repeat(4096)});
        let record = serde_json::json!({"type":"compacted","timestamp":"2026-10-20T00:01:00Z",
            "payload":{"latest_token_usage_record":{"response_id":"carried-call",
            "thread_id":"synthetic-session","session_id":"synthetic-session","usage":usage,
            "ignored_dense":vec![0;4096],"ignored_text":"x".repeat(4096)}}});
        let path = std::env::temp_dir().join(format!(
            "llm-usage-codex-carried-fields-{}.jsonl",
            std::process::id()
        ));
        std::fs::write(&path, format!("{record}\n")).unwrap();
        let limits = JsonlLimits {
            chunk_bytes: 64,
            max_line_bytes: 512,
            max_lines: Some(1),
            time_budget: Some(std::time::Duration::from_secs(5)),
        };
        let outcome = read_jsonl(&path, 0, 1, &limits).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(outcome.lines.len(), 1);
        let text = &outcome.lines[0].text;
        let sanitized: serde_json::Value = serde_json::from_str(text).unwrap();
        let carried = &sanitized["payload"]["latest_token_usage_record"];
        assert_eq!(carried["usage"]["total_tokens"], 120);
        assert!(carried["ignored_dense"].is_null());
        assert!(carried["ignored_text"].is_null());
        assert!(carried["usage"]["ignored_usage_text"].is_null());
        assert!(
            text.len() < 1024,
            "unknown carried fields must not be retained: {} bytes",
            text.len()
        );
    }

    #[test]
    fn usage_unknown_malformed_duplicate_and_unterminated_records_stay_blocked() {
        for kind in [
            "token_usage_record",
            "event_msg",
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
            "{\"type\":\"compacted\",\"payload\":{\"latest_token_usage_record\":[null,null,null,null]}}\n",
            "{\"type\":\"compacted\",\"payload\":{\"latest_token_usage_record\":{\"usage\":[1,2,3,4,5,6]}}}\n",
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
