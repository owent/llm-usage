//! state.db `session_model_usage` 模型/任务累计行 → 来源原生区间汇总
//! （`session_model_usage_v1`）。
//!
//! 格式依据（A24 固定源码 ef70b3661cbfcf57e583008ad91dd04d8ba46070，
//! hermes_state_common.py SCHEMA_SQL / hermes_state_usage.py / agent/turn_usage.py；
//! 官方存储文档 website/docs/developer-guide/session-storage.md）：
//! - 库布局：`get_hermes_home()/state.db`（HERMES_HOME → Windows
//!   %LOCALAPPDATA%/hermes → 其他 ~/.hermes；命名 profile
//!   `<root>/profiles/<name>/state.db`），WAL 模式，state.db/-wal/-shm 同目录。
//! - `session_model_usage` 按六列组合键（session/model/billing_provider/
//!   billing_base_url/billing_mode/task）累计：api_call_count 与五个 token
//!   计数列 ADD 式增长；first_seen 仅插入时写，last_seen 每次冲突更新推进；
//!   task != '' 的行来自 record_auxiliary_usage（辅助任务累计，不进主会话
//!   总量），task = '' 是主会话模型行。绝对路径写 sessions 总量、不写模型行。
//! - 时间列 REAL Unix epoch 秒（time.time()）；first_seen/last_seen 是聚合
//!   写入边界，不是逐请求时间。
//! - v20 把历史 sessions 汇总回填成模型行（INSERT OR IGNORE ... SELECT ...
//!   FROM sessions）；v22 重建表把 task 纳入主键。回填行不证明历史全部
//!   调用使用该模型。
//!
//! 映射约定（adapters.md「Hermes Agent 本地约定」/ V03 固定数学样本）：
//! - 每行 → 一条 `SourceAggregateInput`（interval_aggregate，保留原生区间），
//!   **不**产生 model_call/usage_observation 事件：api_call_count 只作为
//!   `reported_call_count` 汇总，绝不拆成伪造调用事件；
//! - 跨日累计行按 first_seen..last_seen 区间保存，不把全部 token 记入某一天
//!   （日汇总无详单不落账，不按时长摊分）；
//! - 主模型行与 task 辅助行互斥（源 upsert 键互斥、辅助不进主总量），
//!   coverage=Exclusive：100 + 20 = 120，不是 220；
//! - 主/辅助行归属同一来源表，不读 sessions 累计列（避免 absolute 覆盖与
//!   v20 回填值叠加而双计）；子 Agent/压缩子会话是独立 session 行、各有
//!   自己的模型行，无跨行继承双计；未解释残差保持未知，不强归当前模型。
//! - billing_base_url 只在内存规范化（scheme/host[:port] 或本地摘要），
//!   不持久化完整 URL/查询参数；scope_key 用六键摘要。
//!
//! 增量约定（SQLite 行）：
//! - schema 指纹持久化于解析上下文；指纹变化 ⇒ 已处理位置重置全量重读
//!   （scope_key upsert 幂等）；
//! - 已处理位置 = 行的有效结束毫秒（last_seen，回退 session ended_at → started_at
//!   → first_seen）+ 60s 有界重叠窗；source_revision = 有效结束毫秒
//!   （last_seen 单调推进）；
//! - 单轮行数上限 50,000：触顶 BudgetExhausted；
//! - 游标 `offset` 恒 0（WAL 下文件字节长度不能作为无变化短路依据）。

use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::hermes::common::{
    db_schema_version, map_hermes, normalize_base_url, open_source_db, schema_probe, seconds_to_ms,
    short_probe, HermesUsage, StagingLimits,
};
use crate::aggregates::{AggregateScope, Coverage, SourceAggregateInput};
use crate::domain::TimeBasis;
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use serde::{Deserialize, Serialize};

pub const HERMES_PARSER_VERSION: &str = "hermes-session-model-usage-1";
/// 已处理时间的回看窗（毫秒）：覆盖同秒乱序的聚合写入。
pub const WATERMARK_OVERLAP_MS: i64 = 60_000;
/// 单轮行数上限。
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

/// 行的有效结束秒表达式（NULL 全空时取哨兵，交由 Rust 层判定跳过）。
const EFFECTIVE_END_S: &str =
    "COALESCE(u.last_seen, s.ended_at, s.started_at, u.first_seen, -1.0e18)";

/// 游标（持久化在 ingestion_checkpoints.cursor_value）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HermesCursor {
    generation: i64,
    /// 恒为 0：DB 不用字节偏移做无变化判定（WAL，同 kilo 约定）。
    #[allow(dead_code)]
    offset: u64,
    /// 已处理到的有效结束时间（毫秒，含该值）；None = 从头全量。
    watermark_ms: Option<i64>,
}

/// 解析上下文：schema 指纹 + 库版本快照。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HermesParseContext {
    schema_fingerprint: Option<String>,
    db_schema_version: Option<i64>,
}

fn diag(code: &str, field: Option<&str>, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        // 位置只存组合键摘要（稳定身份，非路径/正文/URL）。
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

struct SmuRow {
    session_id: String,
    model: String,
    billing_provider: String,
    billing_base_url: String,
    billing_mode: String,
    task: String,
    api_call_count: i64,
    usage: HermesUsage,
    first_seen: Option<f64>,
    last_seen: Option<f64>,
    session_started: Option<f64>,
    session_ended: Option<f64>,
}

impl SmuRow {
    fn effective_end_ms(&self) -> Option<i64> {
        let end_s = self
            .last_seen
            .or(self.session_ended)
            .or(self.session_started)
            .or(self.first_seen);
        seconds_to_ms(end_s.unwrap_or(f64::NAN))
    }

    /// 区间汇总稳定身份：六键摘要（base_url 已规范化，不落原文）。
    fn scope_key(&self) -> String {
        let route = normalize_base_url(&self.billing_base_url);
        let raw = [
            self.session_id.as_str(),
            self.model.as_str(),
            self.billing_provider.as_str(),
            route.as_str(),
            self.billing_mode.as_str(),
            self.task.as_str(),
        ]
        .join("\u{1}");
        format!("smu:{}", crate::identity::content_hash(&raw))
    }
}

fn load_window(conn: &rusqlite::Connection, since_s: f64) -> Result<Vec<SmuRow>, CoreError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT u.session_id, u.model, u.billing_provider, u.billing_base_url, \
                 u.billing_mode, u.task, u.api_call_count, \
                 u.input_tokens, u.output_tokens, u.cache_read_tokens, \
                 u.cache_write_tokens, u.reasoning_tokens, \
                 u.first_seen, u.last_seen, s.started_at, s.ended_at \
             FROM session_model_usage u LEFT JOIN sessions s ON s.id = u.session_id \
             WHERE {EFFECTIVE_END_S} >= ?1 \
             ORDER BY {EFFECTIVE_END_S}, u.session_id, u.model, u.billing_provider, \
                 u.billing_base_url, u.billing_mode, u.task \
             LIMIT ?2"
        ))
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![since_s, MAX_ROWS_PER_ROUND], |r| {
            Ok(SmuRow {
                session_id: r.get(0)?,
                model: r.get(1)?,
                billing_provider: r.get(2)?,
                billing_base_url: r.get(3)?,
                billing_mode: r.get(4)?,
                task: r.get(5)?,
                api_call_count: r.get(6)?,
                usage: HermesUsage {
                    input_tokens: r.get(7)?,
                    output_tokens: r.get(8)?,
                    cache_read_tokens: r.get(9)?,
                    cache_write_tokens: r.get(10)?,
                    reasoning_tokens: r.get(11)?,
                },
                first_seen: r.get(12)?,
                last_seen: r.get(13)?,
                session_started: r.get(14)?,
                session_ended: r.get(15)?,
            })
        })
        .map_err(CoreError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)
}

/// 增量扫描一个 state.db（统一入口 `HermesAdapter::scan` 分派到本实现）。
pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    _now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let cursor: HermesCursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(HermesCursor {
            generation: target.generation,
            offset: 0,
            watermark_ms: None,
        });
    let context: HermesParseContext = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let source = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let conn = source.conn();
    let probe = schema_probe(conn)?;
    let Some(fingerprint) = probe.fingerprint else {
        // 探测层保证不会走到这里（指纹不符在 detect fail closed）；
        // 游标期间库被换掉时按未知格式拒绝，保留旧结果。
        return Err(CoreError::Validation(
            "hermes state.db schema fingerprint no longer matches; fail closed".to_string(),
        ));
    };
    // schema 指纹变化 ⇒ 旧的已处理位置不可信，全量重读（scope_key 幂等，不双计）。
    let fingerprint_reset = context.schema_fingerprint.is_some()
        && context.schema_fingerprint.as_deref() != Some(fingerprint.as_str());
    let watermark = if fingerprint_reset {
        None
    } else {
        cursor.watermark_ms
    };
    let since_s = watermark
        .map(|w| ((w - WATERMARK_OVERLAP_MS) as f64) / 1000.0)
        .unwrap_or(-1.0e19);

    let mut rows = load_window(conn, since_s)?;
    let hit_cap = rows.len() as i64 >= MAX_ROWS_PER_ROUND;
    let db_version = db_schema_version(conn)?;

    let mut aggregates: Vec<SourceAggregateInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    for row in rows.iter_mut() {
        records_seen += 1;
        let scope_key = row.scope_key();
        let Some(end_ms) = row.effective_end_ms() else {
            diagnostics.push(diag(
                "implausible_or_missing_time",
                Some("last_seen"),
                &scope_key,
                "session_model_usage row without plausible end time \
                 (last_seen/session window all missing or pre-2000); row skipped, no zero fill",
            ));
            continue;
        };
        let start_ms = row.first_seen.and_then(seconds_to_ms);
        if let Some(start) = start_ms {
            if end_ms < start {
                diagnostics.push(diag(
                    "inconsistent_interval",
                    Some("first_seen"),
                    &scope_key,
                    "last_seen before first_seen; row skipped, not clamped",
                ));
                continue;
            }
        }
        let counters = [
            row.usage.input_tokens,
            row.usage.output_tokens,
            row.usage.cache_read_tokens,
            row.usage.cache_write_tokens,
            row.usage.reasoning_tokens,
        ];
        if counters.iter().any(|v| *v < 0) || row.api_call_count < 0 {
            diagnostics.push(diag(
                "negative_counter",
                Some("tokens"),
                &scope_key,
                "negative token/call counter; row skipped, not zeroed",
            ));
            continue;
        }
        let mapped = map_hermes(&row.usage);
        for contradiction in &mapped.diagnostics {
            diagnostics.push(diag(
                contradiction.code,
                Some(contradiction.field),
                &scope_key,
                &contradiction.detail,
            ));
        }
        aggregates.push(SourceAggregateInput {
            instance_id: target.instance_id.clone(),
            scope: AggregateScope::Session,
            scope_key: scope_key.clone(),
            interval_start_ms: start_ms,
            interval_end_ms: end_ms,
            // last_seen 是瞬时边界（闭区间语义）。
            interval_end_inclusive: true,
            usage: mapped.usage,
            quality: mapped.quality,
            // 来源报告调用汇总；不伪造逐次 model_call。
            reported_call_count: Some(row.api_call_count),
            // 源组合键互斥（主/task 辅助行分别累计，辅助不进主总量）。
            coverage: Coverage::Exclusive,
            duplicate_of: None,
            // first_seen/last_seen 是聚合写入边界，不是逐请求时间。
            time_basis: TimeBasis::Uncertain,
            source_revision: Some(end_ms),
        });
    }

    // 更新已处理位置：触顶停在最后一个完整毫秒（重叠窗下轮重读，幂等）。
    let last_end = rows.last().and_then(|r| r.effective_end_ms());
    let new_watermark = if hit_cap {
        last_end.map(|w| w - 1)
    } else {
        last_end
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
    let new_cursor = HermesCursor {
        generation: target.generation,
        offset: 0,
        watermark_ms: next_watermark,
    };
    let new_context = HermesParseContext {
        schema_fingerprint: Some(fingerprint),
        db_schema_version: db_version,
    };
    let degraded = !diagnostics.is_empty();
    Ok(ScanOutcome {
        status,
        cursor: Some(serde_json::to_value(new_cursor)?),
        parse_context: Some(serde_json::to_value(new_context)?),
        events: Vec::new(),
        aggregates,
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: if degraded {
            "degraded".to_string()
        } else {
            "active".to_string()
        },
    })
}
