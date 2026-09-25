//! kilo.db `message` 表逐次 usage 格式实现（`message_tokens_v1`）。
//!
//! 格式证据（真实脱敏 fixture + 本机只读 SELECT 探查，2026-09-25）：
//! - 库布局：`<kilo home>/kilo.db`（本机 `~/.local/share/kilo/kilo.db`，WAL 模式，
//!   另有 kilo.db-wal/-shm；上游另有 opencode-rc.db 不在范围）。
//! - 表：`message(id, session_id, time_created, time_updated, data)`，
//!   `session(id, project_id, parent_id, …, version, …, tokens_input/output/
//!   reasoning/cache_read/cache_write, …)`（DDL 见 fixture schema 节，实读原文）。
//! - 逐次 usage 载体：message.data（JSON 文本）中 `role="assistant"` 的
//!   `tokens{input, output, reasoning?, cache{read, write}, total?}`；
//!   `total = input+output+reasoning+cache.read+cache.write` 全互斥。
//!   本机实读库 13,342 条 assistant 全带 tokens，其中 42 条缺 `total`
//!   （未完成/出错）；`finish` 取值 tool-calls/stop/length/other/error/null。
//! - 模型归属：assistant 消息自带 `modelID`/`providerID`（request_field）；
//!   `time{created, completed?}` 为 epoch 毫秒。
//! - 子 Agent：session.parent_id 非空 ⇒ 该会话消息记 sub_agent（子会话是
//!   独立 session 行，与主会话同库，无跨表双计）。
//! - session 行 tokens_* 五列是累计快照：仅五列合计可与逐次对账
//!   （实读 276 会话 275 对上；7.4.8/7.4.9 fixture 期列名与值错位，本机新版本
//!   列名一一对应——列级语义随版本不稳定，五列合计稳定）。快照列绝不映射为
//!   request 事件（A11：session 累计表不混入 request 计数）。
//! - 新 core 数据层（session_message 表）当前 0 行；仅存 session_message 而无
//!   message 表的库按未知格式 fail closed，待专用实现取证。
//!
//! 增量合同（architecture.md「各输入的增量策略」SQLite 行）：
//! - schema 指纹（表/关键列存在性）持久化于解析上下文；指纹变化 ⇒ 水位重置
//!   全量重读（id 键 upsert 幂等，不双计）；
//! - 稳定键 = message.id，更新序号 = message.time_updated（事件 source_revision）；
//! - 水位 + 60s 有界重叠窗（并发会话同毫秒写放大兜底，重复行按
//!   同键同内容幂等）；
//! - message 表同时有 time_created/time_updated，无需 created_at-only 有界重扫；
//! - 每轮行数上限 50,000：触顶 ⇒ BudgetExhausted，水位停在最后一个完整毫秒；
//! - 游标 `offset` 恒为 0：WAL 下主库文件长度不变不代表内容未变，
//!   字节长度不能作为无变化短路依据（框架短路与代数裁决仍生效）。
//!   in-place 页重写会改变文件头（change counter）触发框架 Rescan 标记，
//!   本实现不因 rescan 重置水位（同一逻辑库的 id/更新序号仍有效）。

use crate::adapters::framework::{
    Reconciliation, ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::kilo::common::{
    map_kilo, open_source_db, schema_fingerprint, KiloUsage, SourceDb, StagingLimits,
};
use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

pub const KILO_PARSER_VERSION: &str = "kilo-message-tokens-1";
const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// 水位回看窗（毫秒）：覆盖并发子会话的同毫秒乱序写。
pub const WATERMARK_OVERLAP_MS: i64 = 60_000;
/// 单轮行数上限：触顶停在该毫秒边界，下轮续读。
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

/// 游标（持久化在 ingestion_checkpoints.cursor_value）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KiloCursor {
    generation: i64,
    /// 恒为 0：DB 不用字节偏移做无变化判定（WAL 见模块头）。
    #[allow(dead_code)]
    offset: u64,
    /// 已处理到的 message.time_updated 水位（含）；None = 从头全量。
    watermark_ms: Option<i64>,
}

/// 解析上下文：schema 指纹 + 版本分派结论（游标期间库结构/版本的持久快照）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct KiloParseContext {
    schema_fingerprint: Option<String>,
    version_basis: Option<VersionBasis>,
    db_version: Option<String>,
}

fn diag(code: &str, field: Option<&str>, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        // 位置只存 message.id（稳定身份，非路径/正文）。
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

struct MessageRow {
    id: String,
    session_id: String,
    time_created: i64,
    time_updated: i64,
    data: String,
    parent_id: Option<String>,
    session_version: Option<String>,
}

fn load_window(conn: &rusqlite::Connection, since_ms: i64) -> Result<Vec<MessageRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.session_id, m.time_created, m.time_updated, m.data, \
             s.parent_id, s.version \
             FROM message m LEFT JOIN session s ON s.id = m.session_id \
             WHERE m.time_updated >= ?1 \
             ORDER BY m.time_updated, m.id \
             LIMIT ?2",
        )
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![since_ms, MAX_ROWS_PER_ROUND], |r| {
            Ok(MessageRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                time_created: r.get(2)?,
                time_updated: r.get(3)?,
                data: r.get(4)?,
                parent_id: r.get(5)?,
                session_version: r.get(6)?,
            })
        })
        .map_err(CoreError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)
}

/// 库内数值最大 session.version（一个库可混存多版本会话）。
fn max_session_version(conn: &rusqlite::Connection) -> Result<Option<String>, CoreError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT version FROM session")
        .map_err(CoreError::Sqlite)?;
    let versions = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(CoreError::Sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)?;
    Ok(versions
        .into_iter()
        .reduce(|a, b| super::version_max(&a, &b).to_string()))
}

/// 解析 message.data.tokens（五互斥字段）。
/// input/output/cache.read/cache.write 四者必需（本机实读 13,342/13,342 成立）；
/// reasoning/total 可缺失（缺失是未知，不是 0）。越界/类型错误返回 None。
fn parse_tokens(tokens: &serde_json::Value) -> Option<KiloUsage> {
    let obj = tokens.as_object()?;
    let get = |parent: Option<&serde_json::Map<String, serde_json::Value>>, key: &str| {
        parent?
            .get(key)?
            .as_i64()
            .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v))
    };
    let cache = obj.get("cache").and_then(|c| c.as_object());
    Some(KiloUsage {
        input: get(Some(obj), "input")?,
        output: get(Some(obj), "output")?,
        reasoning: obj
            .get("reasoning")
            .and_then(|v| v.as_i64())
            .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v)),
        cache_read: get(cache, "read")?,
        cache_write: get(cache, "write")?,
        total: obj
            .get("total")
            .and_then(|v| v.as_i64())
            .filter(|v| (0..=MAX_REASONABLE_TOKEN).contains(v)),
    })
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    target: &ScanTarget,
    row: &MessageRow,
    data: &serde_json::Value,
    parse_basis: VersionBasis,
    now_ms: i64,
) -> EventInput {
    let json_i64 = |path: &[&str]| -> Option<i64> {
        let mut node = data;
        for key in path {
            node = node.get(key)?;
        }
        node.as_i64()
    };
    let json_str = |key: &str| data.get(key).and_then(|v| v.as_str());
    // 时间：优先完成时间（source_completion），未完成退开始时间（source_start），
    // 再退行级 time_created（uncertain）。
    let (occurred_ms, time_basis) = match (
        json_i64(&["time", "completed"]),
        json_i64(&["time", "created"]),
    ) {
        (Some(completed), _) => (completed, TimeBasis::SourceCompletion),
        (None, Some(created)) => (created, TimeBasis::SourceStart),
        (None, None) => (row.time_created, TimeBasis::Uncertain),
    };
    let source_time = json_i64(&["time", "completed"])
        .or_else(|| json_i64(&["time", "created"]))
        .map(|ms| ms.to_string());
    let duration_ms = match (
        json_i64(&["time", "created"]),
        json_i64(&["time", "completed"]),
    ) {
        (Some(created), Some(completed)) if completed >= created => Some(completed - created),
        _ => None,
    };
    let finish = json_str("finish");
    let has_error =
        data.get("error").map(|e| e.is_object()).unwrap_or(false) || finish == Some("error");
    // 未完成（无 finish、无 error、无 total）记 partial；完成后 time_updated 提升、
    // 同键按 source_revision 替换为 final。
    let lifecycle = if finish.is_some() || has_error {
        Lifecycle::Final
    } else {
        Lifecycle::Partial
    };
    EventInput {
        source_instance_id: target.instance_id.clone(),
        source_record_key: format!("kilo:msg:{}", row.id),
        record_kind: RecordKind::ModelCall,
        schema_version: row
            .session_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        parser_version: KILO_PARSER_VERSION.to_string(),
        parse_basis: Some(parse_basis),
        origin_call_id: json_str("parentID").map(str::to_string),
        attempt_id: None,
        session_id: Some(row.session_id.clone()),
        parent_session_id: row.parent_id.clone(),
        host_application: None,
        agent: "kilo-code".to_string(),
        call_category: if row.parent_id.is_some() {
            CallCategory::SubAgent
        } else {
            CallCategory::Primary
        },
        occurred_at_ms: occurred_ms,
        observed_at_ms: Some(now_ms),
        source_time,
        time_basis,
        interval_start_ms: None,
        interval_end_ms: None,
        provider_id: json_str("providerID").map(str::to_string),
        model_raw: json_str("modelID").map(str::to_string),
        model_canonical: None,
        model_attribution: if data.get("modelID").is_some() {
            ModelAttribution::RequestField
        } else {
            ModelAttribution::Unknown
        },
        usage: crate::domain::TokenUsage::default(),
        quality: crate::domain::TokenQuality::default(),
        lifecycle,
        source_revision: Some(row.time_updated),
        error_status: if has_error {
            Some("error".to_string())
        } else {
            None
        },
        duration_ms,
        ttft_ms: None,
        attribution_status: AttributionStatus::Verified,
        exclusion_reason: None,
        cost: None,
    }
}

/// 单会话对账：逐次五字段合计（assistant）vs session 行五列合计。
fn reconcile_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Reconciliation, CoreError> {
    let detail: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(json_extract(data,'$.tokens.input')),0) \
               + COALESCE(SUM(json_extract(data,'$.tokens.output')),0) \
               + COALESCE(SUM(json_extract(data,'$.tokens.reasoning')),0) \
               + COALESCE(SUM(json_extract(data,'$.tokens.cache.read')),0) \
               + COALESCE(SUM(json_extract(data,'$.tokens.cache.write')),0) \
             FROM message \
             WHERE session_id = ?1 AND json_valid(data) \
               AND json_extract(data,'$.role') = 'assistant'",
            [session_id],
            |r| r.get(0),
        )
        .map_err(CoreError::Sqlite)?;
    let snapshot: Option<f64> = conn
        .query_row(
            "SELECT tokens_input + tokens_output + tokens_reasoning \
               + tokens_cache_read + tokens_cache_write \
             FROM session WHERE id = ?1",
            [session_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(CoreError::Sqlite)?;
    let snapshot_final = snapshot.map(|v| v.round() as i64);
    let (difference, verdict) = match snapshot_final {
        Some(snap) => {
            let diff = detail - snap;
            (Some(diff), if diff == 0 { "matched" } else { "mismatch" })
        }
        None => (None, "no_snapshot"),
    };
    Ok(Reconciliation {
        series: "session_cumulative_snapshot".to_string(),
        detail_sum: detail,
        snapshot_final,
        carried_sum: 0,
        difference,
        verdict: verdict.to_string(),
    })
}

/// 增量扫描一个 kilo.db（统一入口 `KiloAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    // 游标不做 generation 过滤：同一逻辑库的 in-place 重写会让框架标 Rescan，
    // 但 id/更新序号水位仍有效；换了逻辑库（身份变化）时 stored 为空自然全量。
    let cursor: KiloCursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(KiloCursor {
            generation: target.generation,
            offset: 0,
            watermark_ms: None,
        });
    let context: KiloParseContext = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let fingerprint = schema_fingerprint(conn)?;
    // schema 指纹变化 ⇒ 旧水位不可信，全量重读（id 幂等，不双计）。
    let fingerprint_reset =
        context.schema_fingerprint.is_some() && context.schema_fingerprint != fingerprint;
    let watermark = if fingerprint_reset {
        None
    } else {
        cursor.watermark_ms
    };
    let since_ms = watermark
        .map(|w| w.saturating_sub(WATERMARK_OVERLAP_MS))
        .unwrap_or(i64::MIN);

    let mut rows = load_window(conn, since_ms)?;
    let hit_cap = rows.len() as i64 >= MAX_ROWS_PER_ROUND;
    let version = max_session_version(conn)?;
    let selection = super::select(version.as_deref());

    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut touched_sessions: Vec<String> = Vec::new();
    for row in rows.iter_mut() {
        records_seen += 1;
        if !touched_sessions.iter().any(|s| s == &row.session_id) {
            touched_sessions.push(row.session_id.clone());
        }
        let data: serde_json::Value = match serde_json::from_str(&row.data) {
            Ok(v) => v,
            Err(_) => {
                diagnostics.push(diag(
                    "bad_data_json",
                    Some("data"),
                    &format!("kilo:msg:{}", row.id),
                    "message.data is not valid JSON; row isolated, content not stored",
                ));
                continue;
            }
        };
        // role 过滤：只 assistant 的 usage 记 model_call（A11）。
        let role = data.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "assistant" {
            if role.is_empty() {
                diagnostics.push(diag(
                    "missing_role",
                    Some("role"),
                    &format!("kilo:msg:{}", row.id),
                    "message.data without role; skipped, not counted as a call",
                ));
            }
            continue;
        }
        let mut event = build_event(target, row, &data, selection.basis, now_ms);
        match data.get("tokens") {
            Some(tokens) => match parse_tokens(tokens) {
                Some(usage) => {
                    let mapped = map_kilo(&usage);
                    event.usage = mapped.usage;
                    event.quality = mapped.quality;
                    for contradiction in &mapped.diagnostics {
                        diagnostics.push(diag(
                            contradiction.code,
                            Some(contradiction.field),
                            &event.source_record_key,
                            &contradiction.detail,
                        ));
                    }
                }
                None => {
                    diagnostics.push(diag(
                        "usage_shape_deviation",
                        Some("tokens"),
                        &format!("kilo:msg:{}", row.id),
                        "assistant tokens missing required numeric fields; call counted, tokens unknown",
                    ));
                }
            },
            None => {
                // 无 usage 的 assistant 消息仍是一次调用的证据：
                // 计调用数，token 全未知（不补零）。
                diagnostics.push(diag(
                    "usage_shape_deviation",
                    Some("tokens"),
                    &format!("kilo:msg:{}", row.id),
                    "assistant message without tokens; call counted, tokens unknown",
                ));
            }
        }
        events.push(event);
    }

    // 水位推进：触顶时停在最后一个完整毫秒（该毫秒下轮重读，幂等）。
    let new_watermark = if hit_cap {
        rows.last().map(|r| r.time_updated - 1)
    } else {
        rows.last().map(|r| r.time_updated)
    };
    let next_watermark = match (watermark, new_watermark) {
        (_, Some(w)) => Some(w.max(watermark.unwrap_or(i64::MIN))),
        (None, None) => None,
        (Some(w), None) => Some(w),
    };
    let status = if hit_cap {
        ScanStatus::BudgetExhausted
    } else {
        ScanStatus::Complete
    };

    let mut reconciliations: Vec<Reconciliation> = Vec::new();
    if status == ScanStatus::Complete {
        for session_id in &touched_sessions {
            let rec = reconcile_session(conn, session_id)?;
            if rec.verdict == "mismatch" {
                diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "reconcile_mismatch".to_string(),
                    field: Some("total_tokens".to_string()),
                    position: None,
                    message: format!(
                        "session assistant detail sum {} != snapshot five-column sum {} (diff {})",
                        rec.detail_sum,
                        rec.snapshot_final.unwrap_or(0),
                        rec.difference.unwrap_or(0)
                    ),
                });
            }
            reconciliations.push(rec);
        }
    }

    let new_cursor = KiloCursor {
        generation: target.generation,
        offset: 0,
        watermark_ms: next_watermark,
    };
    let new_context = KiloParseContext {
        schema_fingerprint: fingerprint,
        version_basis: Some(selection.basis),
        db_version: version,
    };
    let degraded = diagnostics.iter().any(|d| {
        matches!(
            d.code.as_str(),
            "bad_data_json" | "missing_role" | "usage_shape_deviation" | "reconcile_mismatch"
        )
    });
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(new_context)?),
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations,
        health: if degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}

/// 短查询事务探测：一条廉价只读查询验证本连接当前可一致读取。
fn short_probe(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |_| Ok(()))
}

/// 供测试/示例使用的只读打开（同合同：busy 时暂存副本）。
pub fn open_readonly_source(path: &std::path::Path) -> Result<SourceDb, CoreError> {
    open_source_db(path, short_probe, &StagingLimits::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tokens_requires_core_fields_keeps_optionals_absent() {
        let full = serde_json::json!({
            "input": 20040, "output": 51, "reasoning": 477,
            "cache": {"read": 0, "write": 0}, "total": 20568
        });
        let usage = parse_tokens(&full).unwrap();
        assert_eq!(usage.total, Some(20568));
        let no_total = serde_json::json!({
            "input": 0, "output": 0, "reasoning": 0,
            "cache": {"read": 0, "write": 0}
        });
        let usage = parse_tokens(&no_total).unwrap();
        assert_eq!(usage.total, None, "missing total stays unknown, not zero");
        assert_eq!(
            usage.reasoning,
            Some(0),
            "explicit zero is a reported value"
        );
        // 缺 cache 子对象或任一必需字段 ⇒ 无法按全互斥口径映射。
        assert!(parse_tokens(&serde_json::json!({"input": 1, "output": 2})).is_none());
        assert!(parse_tokens(&serde_json::json!({
            "input": 1, "output": 2, "cache": {"read": 0}
        }))
        .is_none());
        // 负值/超限拒绝。
        assert!(parse_tokens(&serde_json::json!({
            "input": -1, "output": 2, "cache": {"read": 0, "write": 0}
        }))
        .is_none());
    }

    #[test]
    fn version_max_prefers_numeric_order() {
        assert_eq!(super::super::version_max("7.4.9", "7.4.10"), "7.4.10");
        assert_eq!(super::super::version_max("7.7.12", "7.4.9"), "7.7.12");
    }
}
