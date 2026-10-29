//! Zed threads.db parser: threads_db_v1, documented zed-threads-db-1/2 formats.
//! Native 1.22.0 / 76659a55 also supports llm-usage-zhipu with DbThread 0.3.0:
//! input is uncached; positive buckets are reported and default zeros remain unknown.
//!
//! Historical hosted reference: Zed bd747337d7be138834e20972b9e203c7b239cc47 (A38).
//! Local 2026-09-29 inspection found matching table schema but zero rows, without usage acceptance.
//! - Database: <data_dir>/threads/threads.db. Data directories:
//!   Windows %LOCALAPPDATA%/Zed; macOS ~/Library/Application Support/Zed;
//!   Linux ~/.local/share/zed (crates/paths/src/paths.rs:143-169).
//!   Upstream uses --user-data-dir, not ZED_DATA_DIR; the collector cannot observe that CLI option.
//!   ZED_STATELESS disables persistence.
//! - threads columns: id/summary/updated_at RFC3339/data_type json|zstd/data BLOB,
//!   plus parent_id/folder_paths/folder_paths_order/created_at, initially backfilled from updated_at.
//! - DbThread JSON contains model{provider,model}, cumulative_token_usage,
//!   four u64 buckets omitting zeros in the historical serializer; its mapper reports those zeros.
//!   request_token_usage maps user UUIDs to usage; multiple requests in a turn
//!   overwrite earlier values (thread.rs:2893). Use those sums only for comparison.
//! - Native 1.26.2 / c4273271 DbThread 0.3.0 output/cache buckets accept custom provider IDs.
//!   OpenAI chat input excludes cache; DeepSeek input includes it and drops cache details.
//!   Latest model identity cannot determine historical input inclusion in a mixed thread.
//!   Keep older hosted/llm-usage-zhipu rules separate; skip empty imported threads.
//!
//! Each thread contributes a session SourceAggregateInput from cumulative usage,
//! without constructing individual call tokens, models, or timestamps.
//! request_token_usage bucket count is a turn-count lower bound stored as reported_call_count.
//! Bucket sums reconcile only; source records do not provide individual usage timestamps.
//! Use created_at..updated_at as the uncertain aggregate interval.
//!
//! Page by thread ID and restart after the last page to revisit mutable cumulative rows.
//! Keep offset=0 because database file size under WAL cannot identify unchanged rows.
//! Upsert scope_key at updated_at revision; read up to 50,001 rows including a continuation check.

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::zed::common::{
    map_zed, open_source_db, short_probe, SourceDb, StagingLimits, ZedUsage,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::TimeBasis;
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};
use std::io::Read;

pub const ZED_PARSER_VERSION: &str = "zed-threads-db-2";
/// Independent policy marker replays old cursors without deleting checkpoints before a scan.
pub const NATIVE_RULES_VERSION: u8 = 3;
/// Nominal row limit per scan; one extra row detects continuation.
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;
/// Decompressed thread data limit: 64 MiB; oversized blobs are skipped with diagnostics.
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;

/// Database cursor keeps offset=0 rather than using byte positions to skip changed rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ZedCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
    #[serde(default)]
    last_thread_id: String,
}

#[derive(Default, Serialize, Deserialize)]
struct ZedPolicy {
    #[serde(default)]
    version: Option<u8>,
    #[serde(default)]
    in_progress: bool,
    #[serde(default)]
    invalid: bool,
}

pub fn needs_policy_check(stored: &StoredScanState) -> bool {
    stored
        .parse_context
        .as_ref()
        .and_then(|v| v.get("version"))
        .and_then(serde_json::Value::as_u64)
        != Some(u64::from(NATIVE_RULES_VERSION))
}

fn diag(code: &str, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

/// Convert RFC3339 to UTC milliseconds.
fn rfc3339_ms(value: &str) -> Option<i64> {
    let ts: jiff::Timestamp = value.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

/// Decode bounded zstd data; oversized/invalid blobs return None and skip the row.
/// Plain JSON shares the 64 MiB decoded-data limit.
/// Encoding does not change the field rules recorded in adapter capabilities.
fn decode_blob(data_type: &str, data: &[u8]) -> Option<Vec<u8>> {
    if data_type.eq_ignore_ascii_case("json") {
        return (data.len() <= MAX_BLOB_BYTES).then(|| data.to_vec());
    }
    if !data_type.eq_ignore_ascii_case("zstd") {
        return None;
    }
    let decoder = zstd::stream::read::Decoder::new(data).ok()?;
    let mut out = Vec::new();
    crate::adapters::run_policy::checked_reader(decoder)
        .take(MAX_BLOB_BYTES as u64 + 1)
        .read_to_end(&mut out)
        .ok()?;
    (out.len() <= MAX_BLOB_BYTES).then_some(out)
}

/// Decode historical TokenUsage defaults as zero; the native external mapper handles validity separately.
fn parse_token_usage(value: Option<&serde_json::Value>) -> Option<ZedUsage> {
    let Some(obj) = value else {
        // Missing objects decode to default zeros; this alone does not verify reported native usage.
        return Some(ZedUsage::default());
    };
    if !obj.is_object() {
        return None;
    }
    let get = |key: &str| -> Option<i64> {
        match obj.get(key) {
            None => Some(0),
            Some(v) => {
                let n = v.as_i64()?;
                (0..=crate::domain::MAX_TOKEN_VALUE)
                    .contains(&n)
                    .then_some(n)
            }
        }
    };
    Some(ZedUsage {
        input_tokens: get("input_tokens")?,
        output_tokens: get("output_tokens")?,
        cache_read_input_tokens: get("cache_read_input_tokens")?,
        cache_creation_input_tokens: get("cache_creation_input_tokens")?,
    })
}

/// Count and sum request_token_usage buckets from the current map or historical array.
/// Older vectors align with messages; upstream upgrades assign user-message keys.
fn request_usage_summary(
    value: Option<&serde_json::Value>,
    include_total: bool,
) -> Option<(usize, Option<i64>, Option<i64>)> {
    let Some(value) = value else {
        return Some((0, include_total.then_some(0), None));
    };
    let buckets: Vec<&serde_json::Value> = match value {
        serde_json::Value::Object(map) => map.values().collect(),
        serde_json::Value::Array(items) => items.iter().collect(),
        _ => return None,
    };
    let mut sum = include_total.then_some(0i64);
    let mut output_sum = (!buckets.is_empty()).then_some(0i64);
    for bucket in &buckets {
        let usage = parse_token_usage(Some(bucket))?;
        if let Some(current) = sum {
            sum = Some(current.checked_add(usage.total()?)?);
        }
        output_sum = output_sum
            .filter(|_| usage.output_tokens > 0)
            .and_then(|sum| sum.checked_add(usage.output_tokens));
    }
    Some((buckets.len(), sum, output_sum))
}

type ThreadRow = (String, String, String, Vec<u8>, Option<String>);

fn load_rows(
    db: &SourceDb,
    has_created_at: bool,
    after_id: &str,
    row_limit: i64,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Result<Vec<ThreadRow>, CoreError> {
    // Read one extra row to distinguish an exact limit from a remaining page.
    let sql = if has_created_at {
        "SELECT id, updated_at, data_type, data, created_at FROM threads
         WHERE id > ?1 ORDER BY id LIMIT ?2"
    } else {
        "SELECT id, updated_at, data_type, data, NULL FROM threads
         WHERE id > ?1 ORDER BY id LIMIT ?2"
    };
    let mut stmt = db.conn().prepare(sql).map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![after_id, row_limit + 1], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(CoreError::Sqlite)?;
    // Skip and diagnose SQLite row type errors without stopping other valid rows.
    let mut out = Vec::new();
    for row in rows {
        match row {
            Ok(r) => out.push(r),
            Err(e) => diagnostics.push(diag(
                "row_read_failed",
                "threads",
                &format!("row read failed: {e}; row skipped"),
            )),
        }
    }
    Ok(out)
}

/// Scan threads.db, dispatched by ZedAdapter::scan.
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    scan_with_row_limit(target, stored, limits, now_ms, MAX_ROWS_PER_ROUND)
}

fn scan_with_row_limit(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
    row_limit: i64,
) -> Result<ScanOutcome, CoreError> {
    let db = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let has_created_at: bool = {
        let mut stmt = db
            .conn()
            .prepare("PRAGMA table_info(threads)")
            .map_err(CoreError::Sqlite)?;
        let mut rows = stmt.query([]).map_err(CoreError::Sqlite)?;
        let mut found = false;
        while let Ok(Some(row)) = rows.next() {
            if row.get::<_, String>(1).is_ok_and(|c| c == "created_at") {
                found = true;
            }
        }
        found
    };
    let mut policy: ZedPolicy = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let restart = target.rescan || needs_policy_check(stored) && !policy.in_progress;
    if restart {
        policy = ZedPolicy::default();
    }
    let mut aggregates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut reconciliations = Vec::new();
    let mut external_reported = false;
    let mut imported_reported = false;
    let mut blob_failures: u64 = 0;
    let after_id = if restart {
        String::new()
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<ZedCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .map(|c| c.last_thread_id)
            .unwrap_or_default()
    };
    if after_id.is_empty() {
        policy = ZedPolicy::default();
    }
    let rows = load_rows(&db, has_created_at, &after_id, row_limit, &mut diagnostics)?;
    let hit_cap = rows.len() as i64 > row_limit;
    let last_thread_id = rows.last().map(|r| r.0.clone()).unwrap_or(after_id);
    for (thread_id, updated_at, data_type, data, created_at) in &rows {
        crate::adapters::run_policy::check()?;
        let position = format!("thread:{thread_id}");
        let Some(blob) = decode_blob(data_type, data) else {
            blob_failures += 1;
            diagnostics.push(diag(
                "blob_decode_failed",
                &position,
                "data_type not json/zstd or blob exceeds the 64 MiB cap; thread skipped",
            ));
            continue;
        };
        let document: serde_json::Value = match crate::adapters::run_policy::json_from_slice(&blob)
        {
            Ok(v) => v,
            Err(_) => {
                blob_failures += 1;
                diagnostics.push(diag(
                    "blob_json_unparseable",
                    &position,
                    "data blob does not parse as JSON; thread skipped",
                ));
                continue;
            }
        };
        let provider = document
            .pointer("/model/provider")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let verified_external = provider == "llm-usage-zhipu"
            && document.get("version").and_then(|v| v.as_str()) == Some("0.3.0");
        let native = document["version"] == "0.3.0"
            && document
                .get("messages")
                .is_some_and(serde_json::Value::is_array)
            && document
                .pointer("/model/provider")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
            && document
                .pointer("/model/model")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
            && document
                .get("cumulative_token_usage")
                .is_some_and(serde_json::Value::is_object)
            && document
                .get("request_token_usage")
                .is_some_and(|v| v.is_object() || v.is_array());
        if provider != "zed.dev" && !verified_external && !native {
            if document["version"] == "0.3.0" {
                blob_failures += 1;
                diagnostics.push(diag("native_thread_shape_deviation",&position,"DbThread 0.3.0 requires model identity, messages and cumulative/request usage objects"));
                continue;
            }
            if !external_reported {
                external_reported = true;
                diagnostics.push(diag(
                    "non_zed_dev_thread_skipped",
                    &position,
                    "thread lacks a verified hosted or native DbThread structure; provider name alone cannot establish its format",
                ));
            }
            continue;
        }
        let version = document
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let imported = version == "1.0.0"
            && !document
                .get("cumulative_token_usage")
                .is_some_and(|v| v.as_object().is_some_and(|m| !m.is_empty()));
        if imported {
            if !imported_reported {
                imported_reported = true;
                diagnostics.push(diag(
                    "imported_thread_skipped",
                    &position,
                    "shared/imported thread carries no usage (official SharedThread conversion); skipped",
                ));
            }
            continue;
        }
        let Some(cumulative) = parse_token_usage(document.get("cumulative_token_usage")) else {
            blob_failures += 1;
            diagnostics.push(diag(
                "token_usage_shape_deviation",
                &position,
                "cumulative_token_usage carries a negative/non-integer/out-of-range value; thread skipped",
            ));
            continue;
        };
        let cumulative_total = if native {
            None
        } else {
            match cumulative.total() {
                Some(total) => Some(total),
                None => {
                    blob_failures += 1;
                    diagnostics.push(diag(
                        "token_usage_overflow",
                        &position,
                        "cumulative token buckets overflow i64; thread skipped",
                    ));
                    continue;
                }
            }
        };
        let Some((bucket_count, bucket_sum, bucket_output)) =
            request_usage_summary(document.get("request_token_usage"), !native)
        else {
            blob_failures += 1;
            diagnostics.push(diag(
                "request_usage_shape_deviation",
                &position,
                "request_token_usage is neither an object nor an array, or its bucket sum overflowed; treated as unusable",
            ));
            continue;
        };
        let updated_ms = rfc3339_ms(updated_at);
        let start_ms = created_at.as_deref().and_then(rfc3339_ms).or(updated_ms);
        let Some(end_ms) = updated_ms else {
            blob_failures += 1;
            diagnostics.push(diag(
                "timestamp_unparseable",
                &position,
                "threads.updated_at not a plausible RFC3339 timestamp; thread skipped",
            ));
            continue;
        };
        let mapped = if native {
            if cumulative.input_tokens > 0 {
                diagnostics.push(diag("thread_input_inclusion_unknown",&position,"native input bucket spans mutable provider/model selections; OpenAI chat is uncached and DeepSeek is inclusive; input and complete totals remain unknown"));
            }
            crate::adapters::zed::common::map_native_mixed(&cumulative)
        } else if verified_external {
            crate::adapters::zed::common::map_verified_openai(&cumulative)
        } else {
            map_zed(&cumulative)
        };
        let empty = if native {
            [
                cumulative.input_tokens,
                cumulative.output_tokens,
                cumulative.cache_read_input_tokens,
                cumulative.cache_creation_input_tokens,
            ]
            .into_iter()
            .all(|value| value == 0)
        } else {
            cumulative_total == Some(0)
        };
        if (verified_external || native) && empty {
            continue;
        }
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: format!("zed:thread:{thread_id}"),
            interval_start_ms: start_ms,
            interval_end_ms: end_ms,
            interval_end_inclusive: false,
            usage: mapped.usage,
            quality: mapped.quality,
            reported_call_count: Some(bucket_count as i64),
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        });
        let comparison = if native {
            bucket_output
                .filter(|_| cumulative.output_tokens > 0)
                .map(|sum| {
                    (
                        "request_output_tokens_vs_cumulative",
                        sum,
                        cumulative.output_tokens,
                    )
                })
        } else {
            bucket_sum
                .zip(cumulative_total)
                .filter(|_| bucket_count > 0)
                .map(|(sum, total)| ("request_token_usage_vs_cumulative", sum, total))
        };
        if let Some((series, detail_sum, snapshot_final)) = comparison {
            reconciliations.push(crate::adapters::framework::Reconciliation {
                series: series.to_string(),
                detail_sum,
                snapshot_final: Some(snapshot_final),
                carried_sum: 0,
                difference: Some(detail_sum - snapshot_final),
                verdict: if detail_sum == snapshot_final {
                    "matched".to_string()
                } else {
                    // Overwritten request buckets can sum below the cumulative value (thread.rs:2893).
                    "mismatch".to_string()
                },
            });
        }
    }
    policy.invalid |= blob_failures > 0 || diagnostics.iter().any(|d| d.code == "row_read_failed");
    policy.in_progress = hit_cap;
    if !hit_cap && !policy.invalid {
        policy.version = Some(NATIVE_RULES_VERSION);
    }
    let degraded = policy.invalid;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(ZedCursor {
            generation: target.generation,
            offset: 0,
            last_thread_id: if hit_cap {
                last_thread_id
            } else {
                String::new()
            },
        })?),
        parse_context: Some(serde_json::to_value(policy)?),
        events: Vec::new(),
        aggregates,
        diagnostics,
        lines_read: rows.len() as u64,
        records_seen: rows.len() as u64,
        reconciliations,
        health: if degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_and_bounds() {
        assert_eq!(rfc3339_ms("2026-08-05T06:38:00Z"), Some(1_785_911_880_000));
        assert_eq!(rfc3339_ms("not a time"), None);
        assert_eq!(rfc3339_ms("1970-01-01T00:00:00Z"), None);
    }

    #[test]
    fn token_usage_defaults_to_reported_zero() {
        // Test historical decoding defaults only; native default-zero validity is checked separately.
        let usage = parse_token_usage(None).unwrap();
        assert_eq!(usage.total(), Some(0));
        let usage = parse_token_usage(Some(&serde_json::json!({
            "input_tokens": 120, "output_tokens": 34
        })))
        .unwrap();
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.cache_read_input_tokens, 0);
        assert!(parse_token_usage(Some(&serde_json::json!({"input_tokens": -1}))).is_none());
        let overflowing = parse_token_usage(Some(&serde_json::json!({
            "input_tokens": crate::domain::MAX_TOKEN_VALUE,
            "output_tokens": crate::domain::MAX_TOKEN_VALUE
        })))
        .unwrap();
        assert_eq!(overflowing.total(), None);
    }

    #[test]
    fn request_usage_accepts_map_and_array() {
        let (_, sum, _) = request_usage_summary(
            Some(&serde_json::json!({
                "a": {"input_tokens": 10, "output_tokens": 5},
                "b": {"cache_read_input_tokens": 7}
            })),
            true,
        )
        .unwrap();
        assert_eq!(sum, Some(22));
        let (_, sum, _) = request_usage_summary(
            Some(&serde_json::json!([
                {"input_tokens": 3}
            ])),
            true,
        )
        .unwrap();
        assert_eq!(sum, Some(3));
        let (_, sum, output) = request_usage_summary(
            Some(&serde_json::json!({
                "a": {
                    "input_tokens": crate::domain::MAX_TOKEN_VALUE,
                    "cache_read_input_tokens": crate::domain::MAX_TOKEN_VALUE,
                    "output_tokens": 5
                }
            })),
            false,
        )
        .unwrap();
        assert_eq!(sum, None);
        assert_eq!(output, Some(5));
        assert!(request_usage_summary(Some(&serde_json::json!("x")), true).is_none());
    }

    #[test]
    fn actual_capped_page_completes_policy_only_on_the_final_page() {
        let path = std::env::temp_dir().join(format!(
            "llm-usage-zed-page-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute("CREATE TABLE threads(id TEXT PRIMARY KEY,summary TEXT NOT NULL,updated_at TEXT NOT NULL,data_type TEXT NOT NULL,data BLOB NOT NULL,created_at TEXT)", []).unwrap();
        for id in ["a", "b", "c", "d"] {
            let document = serde_json::json!({
                "version":"0.3.0",
                "messages":[],
                "model":{"provider":"deepseek","model":"native"},
                "cumulative_token_usage":{"output_tokens":1},
                "request_token_usage":{"request":{"output_tokens":1}}
            });
            db.execute(
                "INSERT INTO threads VALUES(?1,'redacted','2026-10-29T00:00:00Z','json',?2,'2026-10-29T00:00:00Z')",
                rusqlite::params![id, serde_json::to_vec(&document).unwrap()],
            )
            .unwrap();
        }
        drop(db);
        let target = ScanTarget {
            instance_id: "zed-page-unit".into(),
            path: path.clone(),
            file_id: "file".into(),
            file_identity: "identity".into(),
            probe: crate::adapters::jsonl::probe_file(&path).unwrap(),
            generation: 0,
            rescan: false,
        };
        let first = scan_with_row_limit(
            &target,
            &StoredScanState::default(),
            &ScanLimits::default(),
            0,
            2,
        )
        .unwrap();
        assert_eq!(first.status, ScanStatus::BudgetExhausted);
        assert_eq!(first.records_seen, 3);
        assert_eq!(first.aggregates.len(), 3);
        assert_eq!(
            first
                .parse_context
                .as_ref()
                .unwrap()
                .get("version")
                .and_then(serde_json::Value::as_u64),
            None
        );
        assert_eq!(
            first
                .parse_context
                .as_ref()
                .unwrap()
                .get("in_progress")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        let state = StoredScanState {
            cursor: first.cursor,
            parse_context: first.parse_context,
        };
        let second = scan_with_row_limit(&target, &state, &ScanLimits::default(), 0, 2).unwrap();
        assert_eq!(second.status, ScanStatus::Complete);
        assert_eq!(second.records_seen, 1);
        assert_eq!(second.aggregates.len(), 1);
        assert_eq!(
            second
                .parse_context
                .as_ref()
                .unwrap()
                .get("version")
                .and_then(serde_json::Value::as_u64),
            Some(u64::from(NATIVE_RULES_VERSION))
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn zstd_roundtrip_within_cap() {
        let raw =
            br#"{"model":{"provider":"zed.dev"},"cumulative_token_usage":{"input_tokens":1}}"#;
        let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 3).unwrap();
        use std::io::Write;
        encoder.write_all(raw).unwrap();
        let compressed = encoder.finish().unwrap();
        let decoded = decode_blob("zstd", &compressed).unwrap();
        assert_eq!(decoded, raw);
        assert!(decode_blob("other", &compressed).is_none());
    }
}
