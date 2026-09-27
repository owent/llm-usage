//! ZCode cli/db/db.sqlite 历史回填（2026-09-27 清空重采数据丢失恢复）。
//!
//! 背景：model-io JSONL 是滚动窗口——子代理会话文件在会话结束后被 ZCode 删除、
//! 主文件反复压实（generation 递增）丢弃旧记录。「清空全部数据重新采集」只能
//! 重采磁盘上仍存在的文件，被清理的历史永久缺失；cli/db/db.sqlite 的
//! model_usage 逐次行是唯一完整存档。
//!
//! 本机实证（2026-09-27，只读对照）：
//! - token 逐位一致：db.input_tokens == JSONL input_total（含缓存读）、
//!   db.output_tokens == JSONL output_total、input+output == computed_total；
//! - db.completed_at 比 JSONL 完成时间晚 3–32ms（两条写入路径的时钟差），
//!   按（session + |Δt| ≤ 2s + input/output 相等）可稳定行对行匹配；
//! - 值域：query_source ∈ {main_turn, subagent, session_title, compact}、
//!   status ∈ {completed, cancelled, error}（全部行均有 usage，无一 null）。
//!
//! 回填口径与 modelio_v1（AI SDK 主口径）对齐：
//! - 已入库 JSONL 事件先按上述规则匹配剔除（不双计）；重跑幂等——回填事件
//!   同样参与匹配；
//! - 身份 `zcodedb:{logical_request_id}:{attempt_index+1}`：与 JSONL 的
//!   `zcode:{requestId}:{attempt}` 不同源，永不互撞；同一实例下重复回填
//!   走既有事件去重（同键更新，不重复计数）；
//! - cancelled/error 行也回填（token 已上报，不丢弃；error_status 标注），
//!   compact 等未证 querySource → unknown 分类 + 一次诊断（不猜）。

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::{commit_batch, IngestBatch};
use crate::storage::Storage;
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::Path;

/// 回填结果计数（写入操作日志与 UI）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZcodeDbBackfillOutcome {
    /// db 读到的 model_usage 行数（有 usage 的行）。
    pub db_rows: usize,
    /// 与已入库事件行对行匹配剔除的行数（含 JSONL 与此前回填）。
    pub matched_existing: usize,
    /// 新插入事件数。
    pub added: usize,
    /// 同键更新（重复回填）事件数。
    pub updated: usize,
}

/// 只读打开 cli/db/db.sqlite 并回填缺失历史。`instance_id` 须与 zcode JSONL
/// 适配器的实例 ID 一致（事件归属同一来源实例，多用户分派沿用）。
pub fn zcode_db_backfill(
    storage: &Storage,
    zcode_db_path: &Path,
    instance_id: &str,
    timezone: &str,
    now_ms: i64,
) -> Result<ZcodeDbBackfillOutcome, CoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let db = Connection::open_with_flags(zcode_db_path, flags)
        .map_err(|e| CoreError::Query(format!("open zcode db: {e}")))?;

    // 已入库事件索引：(session, input_total, output_total) → 发生时间列表。
    // 匹配一次消耗一个时间戳（同键多事件不重复占用）。
    let mut existing: HashMap<(String, i64, i64), Vec<i64>> = HashMap::new();
    {
        let mut stmt = storage.conn().prepare(
            "SELECT session_id, occurred_at_ms, input_total, output_total
             FROM usage_events
             WHERE agent = 'zcode' AND record_kind = 'model_call'
               AND session_id IS NOT NULL AND occurred_at_ms IS NOT NULL
               AND input_total IS NOT NULL AND output_total IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows {
            let (session, occurred, input, output) = row?;
            existing
                .entry((session, input, output))
                .or_default()
                .push(occurred);
        }
    }

    let mut stmt = db.prepare(
        "SELECT logical_request_id, attempt_index, session_id, provider_id, model_id,
                query_source, completed_at, duration_ms,
                input_tokens, cache_read_input_tokens, cache_creation_input_tokens,
                output_tokens, reasoning_tokens, provider_total_tokens, status
         FROM model_usage
         WHERE input_tokens IS NOT NULL AND output_tokens IS NOT NULL
           AND completed_at IS NOT NULL AND session_id IS NOT NULL
         ORDER BY completed_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ZcodeDbRow {
            logical_request_id: r.get(0)?,
            attempt_index: r.get::<_, i64>(1)?,
            session_id: r.get(2)?,
            provider_id: r.get(3)?,
            model_id: r.get(4)?,
            query_source: r.get(5)?,
            completed_at: r.get(6)?,
            duration_ms: r.get(7)?,
            input_tokens: r.get(8)?,
            cache_read: r.get(9)?,
            cache_write: r.get(10)?,
            output_tokens: r.get(11)?,
            reasoning_tokens: r.get(12)?,
            provider_total: r.get(13)?,
            status: r.get(14)?,
        })
    })?;

    let mut batch = IngestBatch {
        batch_id: format!("zcode-db-backfill-{now_ms}"),
        instance_id: instance_id.to_string(),
        timezone: timezone.to_string(),
        now_ms,
        events: Vec::new(),
        checkpoints: Vec::new(),
        diagnostics: Vec::new(),
        run_id: None,
        retention_cutoff_ms: None,
    };
    let mut outcome = ZcodeDbBackfillOutcome::default();
    let mut unmapped_reported = false;

    for row in rows {
        let row = row?;
        outcome.db_rows += 1;
        // 行对行匹配剔除（±2s + token 相等）；匹配一次消耗一个时间戳。
        let key = (
            row.session_id.clone(),
            row.input_tokens,
            row.output_tokens,
        );
        let matched = existing
            .get_mut(&key)
            .and_then(|times| {
                let idx = times
                    .iter()
                    .position(|t| (row.completed_at - t).abs() <= 2_000)?;
                Some(times.remove(idx))
            })
            .is_some();
        if matched {
            outcome.matched_existing += 1;
            continue;
        }
        let (category, unmapped) = map_db_query_source(row.query_source.as_deref());
        if unmapped && !unmapped_reported {
            unmapped_reported = true;
            batch.diagnostics.push(crate::ingest::DiagnosticInput {
                event_id: None,
                code: "unmapped_query_source".to_string(),
                field: Some("query_source".to_string()),
                position: row.query_source.clone(),
                message: format!(
                    "querySource {:?} not in verified value set; category=unknown (no guess)",
                    row.query_source
                ),
            });
        }
        // input_uncached = input − (cache_read + cache_write)；部分和超过总量时
        // 置 None（未知不补零，与 AI SDK 口径同规则）。
        let uncached = row
            .cache_read
            .zip(row.cache_write)
            .and_then(|(r, w)| r.checked_add(w))
            .and_then(|parts| row.input_tokens.checked_sub(parts))
            .filter(|v| *v >= 0);
        let key = format!(
            "zcodedb:{}:{}",
            row.logical_request_id,
            row.attempt_index + 1
        );
        batch.events.push(EventInput {
            source_instance_id: instance_id.to_string(),
            source_record_key: key,
            record_kind: RecordKind::ModelCall,
            schema_version: "1".to_string(),
            parser_version: ZCODE_DB_BACKFILL_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: Some((row.attempt_index + 1).to_string()),
            session_id: Some(row.session_id),
            parent_session_id: None,
            host_application: None,
            agent: "zcode".to_string(),
            call_category: category,
            occurred_at_ms: row.completed_at,
            observed_at_ms: None,
            source_time: None,
            time_basis: TimeBasis::SourceCompletion,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: Some(row.provider_id),
            model_raw: Some(row.model_id),
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: crate::domain::TokenUsage {
                input_uncached: uncached,
                input_cache_read: row.cache_read,
                input_cache_write: row.cache_write,
                input_total: Some(row.input_tokens),
                output_total: Some(row.output_tokens),
                output_reasoning: row.reasoning_tokens,
                total_tokens: row
                    .input_tokens
                    .checked_add(row.output_tokens)
                    .or(row.provider_total),
                source_total: row.provider_total,
            },
            quality: crate::domain::TokenQuality {
                input_uncached: crate::domain::FieldQuality::Derived,
                input_cache_read: crate::domain::FieldQuality::Reported,
                input_cache_write: crate::domain::FieldQuality::Reported,
                input_total: crate::domain::FieldQuality::Reported,
                output_total: crate::domain::FieldQuality::Reported,
                output_reasoning: if row.reasoning_tokens.is_some() {
                    crate::domain::FieldQuality::Reported
                } else {
                    crate::domain::FieldQuality::Unknown
                },
                total_tokens: crate::domain::FieldQuality::Derived,
                source_total: if row.provider_total.is_some() {
                    crate::domain::FieldQuality::Reported
                } else {
                    crate::domain::FieldQuality::Unknown
                },
            },
            lifecycle: Lifecycle::Final,
            source_revision: None,
            error_status: if row.status == "completed" {
                None
            } else {
                Some(row.status)
            },
            duration_ms: row.duration_ms,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: None,
        });
    }

    if !batch.events.is_empty() {
        let committed = commit_batch(storage, &batch, None)?;
        outcome.added = committed.added.max(0) as usize;
        outcome.updated = committed.updated.max(0) as usize;
    }
    Ok(outcome)
}

/// db.query_source → 调用分类（与 modelio_v1 同值域判定；compact 未证实 → unknown）。
fn map_db_query_source(source: Option<&str>) -> (CallCategory, bool) {
    match source {
        Some("main_turn") => (CallCategory::Primary, false),
        Some("subagent") => (CallCategory::SubAgent, false),
        Some("session_title") => (CallCategory::Auxiliary, false),
        _ => (CallCategory::Unknown, true),
    }
}

/// db 行（model_usage 读取形状；usage 均为非空——查询已过滤）。
struct ZcodeDbRow {
    logical_request_id: String,
    attempt_index: i64,
    session_id: String,
    provider_id: String,
    model_id: String,
    query_source: Option<String>,
    completed_at: i64,
    duration_ms: Option<i64>,
    input_tokens: i64,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
    output_tokens: i64,
    reasoning_tokens: Option<i64>,
    provider_total: Option<i64>,
    status: String,
}

/// 回填实现的解析器版本标识（事件可追溯）。
pub const ZCODE_DB_BACKFILL_VERSION: &str = "zcode-db-backfill-1";
