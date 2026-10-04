//! Zed threads.db 格式实现（`threads_db_v1`，文档级 zed-threads-db-1）。
//!
//! 格式依据（Zed 官方源码 bd747337d7be138834e20972b9e203c7b239cc47，A38；
//! 本机 2026-09-29 只读核对 threads 表 schema 一致、0 行）：
//! - 库布局：`<data_dir>/threads/threads.db`；data_dir = Windows
//!   `%LOCALAPPDATA%\Zed`、macOS `~/Library/Application Support/Zed`、
//!   Linux `~/.local/share/zed`（crates/paths/src/paths.rs:143-169）。
//!   主程序无 `ZED_DATA_DIR` 环境变量（覆盖途径是 CLI `--user-data-dir`，
//!   采集器不可见；`ZED_STATELESS` 为真时不落盘）。
//! - threads 表列：id/summary/updated_at(RFC3339)/data_type(json|zstd)/data BLOB
//!   + 迁移列 parent_id/folder_paths/folder_paths_order/created_at(回填=updated_at)。
//! - data blob JSON（DbThread）：`model{provider,model}`、
//!   `cumulative_token_usage`（TokenUsage 四 u64，0 值序列化缺省=报告 0）、
//!   `request_token_usage`（HashMap<用户消息 UUID, TokenUsage>，**turn 内多请求
//!   后写覆盖前写**，thread.rs:2893；求和会漏计，仅作对账）、顶层 `version`。
//! - 仅 provider=="zed.dev" 的 hosted 调用计入；分享导入线程（SharedThread
//!   version "1.0.0"，db.rs:156-174）用量置零，非 zed.dev/导入线程跳过。
//!
//! 映射约定（M8 会话级聚合边界）：每线程一条 `SourceAggregateInput`
//! （scope=Session，总量以 cumulative_token_usage 为准，不展开伪造逐次事件）；
//! request_token_usage 桶数作 `reported_call_count`（=turn 数下界，覆盖语义
//! 如实标注）、桶合计作 Reconciliation 对照，不入账；逐次 usage 无时间戳
//! （官方源码未见），区间用 created_at..updated_at，time_basis=Uncertain。
//!
//! 增量约定（SQLite 行）：线程 id 分页，末页后从头复查可变累计行；
//! offset 恒 0（WAL 下字节长度不能作无变化判定）；聚合按 scope_key
//! upsert 幂等，source_revision = updated_at 毫秒；单轮行数上限 50,000。

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

pub const ZED_PARSER_VERSION: &str = "zed-threads-db-1";
/// 单轮行数上限。
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;
/// 单线程 data blob 解压上限（64 MiB；超限跳过该行并记诊断）。
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;

/// 游标（offset 恒 0：DB 不用字节偏移做无变化判定）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ZedCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
    #[serde(default)]
    last_thread_id: String,
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

/// RFC3339 → UTC 毫秒。
fn rfc3339_ms(value: &str) -> Option<i64> {
    let ts: jiff::Timestamp = value.trim().parse().ok()?;
    let ms = ts.as_millisecond();
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

/// 解压 zstd blob（有界：超上限返回 None，调用方记诊断跳行）。
/// json 分支同样限长——64 MiB 上限约束的是"单 blob 解压后大小"，
/// 与存储编码无关（字段语义以能力表声明为准）。
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

/// TokenUsage JSON（0 值缺省=报告 0；负值/类型错误拒绝该线程）。
fn parse_token_usage(value: Option<&serde_json::Value>) -> Option<ZedUsage> {
    let Some(obj) = value else {
        // 缺失对象 = 全零（官方 serde default 语义）。
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

/// request_token_usage 的桶数与合计（map 新格式 / 数组旧格式都接受；
/// 旧 Vec 与 messages 等长按索引对齐，官方升级时已归到用户消息 key）。
fn request_usage_summary(value: Option<&serde_json::Value>) -> Option<(usize, i64)> {
    let Some(value) = value else {
        return Some((0, 0));
    };
    let buckets: Vec<&serde_json::Value> = match value {
        serde_json::Value::Object(map) => map.values().collect(),
        serde_json::Value::Array(items) => items.iter().collect(),
        _ => return None,
    };
    let mut sum = 0i64;
    for bucket in &buckets {
        let usage = parse_token_usage(Some(bucket))?;
        // checked 算术约定：溢出（桶值极大时）拒绝该线程，不饱和隐藏。
        sum = sum.checked_add(usage.total()?)?;
    }
    Some((buckets.len(), sum))
}

type ThreadRow = (String, String, String, Vec<u8>, Option<String>);

fn load_rows(
    db: &SourceDb,
    has_created_at: bool,
    after_id: &str,
    diagnostics: &mut Vec<DiagnosticInput>,
) -> Result<Vec<ThreadRow>, CoreError> {
    // 多取一行判定是否还有更多（恰好 MAX 行不误报 BudgetExhausted）。
    let sql = if has_created_at {
        "SELECT id, updated_at, data_type, data, created_at FROM threads
         WHERE id > ?1 ORDER BY id LIMIT ?2"
    } else {
        "SELECT id, updated_at, data_type, data, NULL FROM threads
         WHERE id > ?1 ORDER BY id LIMIT ?2"
    };
    let mut stmt = db.conn().prepare(sql).map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![after_id, MAX_ROWS_PER_ROUND + 1], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(CoreError::Sqlite)?;
    // 行级容错：单行类型错误（SQLite 动态类型）跳行记诊断，不中止整轮。
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

/// 增量扫描一个 threads.db（统一入口 `ZedAdapter::scan` 分派）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
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
    let mut aggregates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut reconciliations = Vec::new();
    let mut external_reported = false;
    let mut imported_reported = false;
    let mut blob_failures: u64 = 0;
    let after_id = if target.rescan {
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
    let rows = load_rows(&db, has_created_at, &after_id, &mut diagnostics)?;
    let hit_cap = rows.len() as i64 > MAX_ROWS_PER_ROUND;
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
        if provider != "zed.dev" {
            if !external_reported {
                external_reported = true;
                diagnostics.push(diag(
                    "non_zed_dev_thread_skipped",
                    &position,
                    "thread provider is not zed.dev (external/ACP agent); skipped to avoid cross-source double counting",
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
        let Some(cumulative_total) = cumulative.total() else {
            blob_failures += 1;
            diagnostics.push(diag(
                "token_usage_overflow",
                &position,
                "cumulative token buckets overflow i64; thread skipped",
            ));
            continue;
        };
        let Some((bucket_count, bucket_sum)) =
            request_usage_summary(document.get("request_token_usage"))
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
        let mapped = map_zed(&cumulative);
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
        if bucket_count > 0 {
            reconciliations.push(crate::adapters::framework::Reconciliation {
                series: "request_token_usage_vs_cumulative".to_string(),
                detail_sum: bucket_sum,
                snapshot_final: Some(cumulative_total),
                carried_sum: 0,
                difference: Some(bucket_sum - cumulative_total),
                verdict: if bucket_sum == cumulative_total {
                    "matched".to_string()
                } else {
                    // 覆盖语义（官方 thread.rs:2893）下逐桶和可能小于累计值。
                    "mismatch".to_string()
                },
            });
        }
    }
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
        parse_context: None,
        events: Vec::new(),
        aggregates,
        diagnostics,
        lines_read: rows.len() as u64,
        records_seen: rows.len() as u64,
        reconciliations,
        health: if blob_failures > 0 {
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
        // 官方 skip_serializing_if 语义：字段缺失 = 已报告 0，不是未知。
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
        let (_, sum) = request_usage_summary(Some(&serde_json::json!({
            "a": {"input_tokens": 10, "output_tokens": 5},
            "b": {"cache_read_input_tokens": 7}
        })))
        .unwrap();
        assert_eq!(sum, 22);
        let (_, sum) = request_usage_summary(Some(&serde_json::json!([
            {"input_tokens": 3}
        ])))
        .unwrap();
        assert_eq!(sum, 3);
        assert!(request_usage_summary(Some(&serde_json::json!("x"))).is_none());
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
