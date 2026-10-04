//! opencode SQLite 家族共享解析件（OpenCode A17 / MiMo Code A14，M3）。
//!
//! 两产品是同一 opencode 内核家族的不同产品身份（adapters.md「分家族复用的
//! 范围」：数据根、实例身份与注册表独立，不因内核同名合并）；本模块是类比
//! kimi_wire.rs 的**根级家族共享件**，只放两产品 pinned 源码逐一证实同形的
//! wire 级逻辑，产品身份（ns/agent/解析器版本）由各产品目录注入：
//!
//! - `part` 表逐 step usage：`data.type="step-finish"` 且 `cost`+`tokens`
//!   在场时携带 `tokens{input, output, reasoning, cache{read, write}}`
//!   （+可选 `total`）。依据：OpenCode pinned 0027387
//!   `packages/core/src/session/projector.ts` 的 `usage()` 提取规则与
//!   `applyUsage` 计数维护、迁移 `20260510033149_session_usage.ts` 的
//!   json_extract 查询规则、以及 pinned 仓库内 vendored client
//!   `packages/app/vendor/opencode-ai-client-1.17.13-v2.tgz` 的
//!   `StepFinishPart`/`AssistantMessage` 类型（tokens 同形，`total` 可选）；
//!   MiMo pinned 456678b `packages/opencode/src/session/message-v2.ts` 的
//!   `StepFinishPart` zod schema（total 可选、五数字段必需）。
//! - **字段语义（固定源码依据，不跨产品移植）**：OpenCode
//!   `packages/core/src/session/runner/publish-llm-event.ts` 的 `tokens()`
//!   逐字映射：`input = usage.nonCachedInputTokens`（未缓存输入，不含缓存）、
//!   `output = usage.visibleOutputTokens`、`reasoning/cache{read,write}` 独立；
//!   `packages/llm/src/protocols/anthropic-messages.ts` mapUsage 证实
//!   inclusive `inputTokens = nonCached + cacheRead + cacheWrite`（sumTokens）
//!   且 anthropic 的 reasoning 不拆分（计入 outputTokens、reasoning 保持
//!   undefined→safe()=0）；`packages/llm/src/protocols/shared.ts` totalTokens
//!   政策 = `inputTokens + outputTokens`（无 provider total 时）。⇒
//!   input_total = input+cache.read+cache.write（派生，上游自算 inclusive
//!   字段语义）、output_total = output+reasoning（派生，两种 provider 模式均
//!   成立：anthropic reasoning=0 已含于 output；拆分 provider 为补和）、
//!   total = 五字段之和（与 kilo 同血统实读结论一致）；`total` 在场时按
//!   source_total 对照，不一致记诊断。
//! - 时间列 epoch 毫秒（两产品 `Timestamps` 均 `Date.now()` 默认）。
//! - 表形状（两产品 pinned sql.ts 逐字列名）：
//!   `part(id, message_id, session_id, time_created, time_updated, data)`、
//!   `session(id, parent_id, version, …)`、`message(id, …, data)`；MiMo 的
//!   `message` 另有 `agent_id` 列、`session` 无 tokens_* 累计列，OpenCode
//!   相反——探测指纹因此互斥，见各产品 common.rs。
//!
//! 家族约定（adapters.md A14/A17）：
//! - **step 与 message 汇总不双计**：assistant `message.data.tokens` 是 turn 级
//!   聚合（OpenCode 迁移把 message.data 逐字段 SUM 进 session 累计列），本实现
//!   只读 `part`（step-finish）逐次入账；message 仅取 `modelID`/`providerID`
//!   归属，不读其 tokens/cost；
//! - **不把 session 累计列逐次相加**：OpenCode `session.tokens_*` 五列只作对账
//!   （projector applyUsage = Σ 当前 step-finish 部件，含删行补偿）；MiMo 无
//!   该五列，不对账；
//! - 模型归属：`message.data.modelID/providerID`（request_field；vendored
//!   client AssistantMessage + MiMo zod 均为必需字段）；**逐调用的精确时间
//!   仍待上游 event 层**（A17：part 行时间是派生视图写入时刻，非逐调用完成时间），
//!   时间依据如实标 observed_at；
//! - step-finish 部件无独立调用 ID：事件键 = `part.id`（两产品均为主键）。
//!
//! 两产品本机均未安装（2026-09-25 盘点 not_found），全部版本 latest_fallback，
//! 能力声明与合成 fixtures 均标注"文档级证据、待真实样本"。

use crate::adapters::framework::{
    Reconciliation, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::adapters::usage_map::{finish, MappedUsage};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::domain::{FieldQuality as Q, TokenQuality, TokenUsage};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const MAX_REASONABLE_TOKEN: i64 = crate::domain::MAX_TOKEN_VALUE;
/// 已处理时间的回看窗（毫秒）：覆盖并发会话的同毫秒乱序写（同 kilo/kimi 约定）。
pub(crate) const WATERMARK_OVERLAP_MS: i64 = 60_000;
/// 单轮行数上限：触顶停在该毫秒边界，下轮续读。
pub(crate) const MAX_ROWS_PER_ROUND: i64 = 50_000;

/// opencode 家族 step-finish `tokens` 五数字段 + 可选 `total`。
/// 语义（固定源码依据，见模块头）：input 是**未缓存**输入（不含缓存），
/// cache 独立桶；reasoning 与 output 拆分与否随 provider（anthropic 不拆分
/// ⇒ reasoning=0 已含于 output）。派生计算：input_total = input+cr+cw、
/// output_total = output+reasoning、total = 五字段之和；`total` 在场时按
/// source_total 对照（不一致记诊断）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct OpencodeFamilyUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// 源直报总量（可选字段）；缺失 = 未知，不补零。
    pub total: Option<i64>,
}

/// 桶间包含关系按固定源码推导（上游自算 inclusive 输入）；溢出字段
/// 保持 None（未知），不 panic、不截断数值。与 kilo 源码同源，计算规则相同。
pub(crate) fn map_opencode_family_usage(raw: &OpencodeFamilyUsage) -> MappedUsage {
    let mut diagnostics = Vec::new();
    let input_total = raw
        .input
        .checked_add(raw.cache_read)
        .and_then(|v| v.checked_add(raw.cache_write));
    let output_total = raw.output.checked_add(raw.reasoning);
    let derived_total = input_total.and_then(|i| output_total.and_then(|o| i.checked_add(o)));
    if let (Some(dt), Some(total)) = (derived_total, raw.total) {
        if dt != total {
            diagnostics.push(crate::metrics::Contradiction {
                code: "source_total_mismatch",
                field: "total_tokens",
                detail: format!("opencode-family total {total} != derived sum {dt}"),
            });
        }
    }
    let usage = TokenUsage {
        input_uncached: Some(raw.input),
        input_cache_read: Some(raw.cache_read),
        input_cache_write: Some(raw.cache_write),
        input_total,
        output_total,
        output_reasoning: Some(raw.reasoning),
        total_tokens: derived_total.or(raw.total),
        source_total: raw.total,
    };
    let quality = TokenQuality {
        input_uncached: Q::Reported,
        input_cache_read: Q::Reported,
        input_cache_write: Q::Reported,
        input_total: if input_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_total: if output_total.is_some() {
            Q::Derived
        } else {
            Q::Unknown
        },
        output_reasoning: Q::Reported,
        total_tokens: if derived_total.is_some() {
            Q::Derived
        } else {
            Q::Reported
        },
        source_total: if raw.total.is_some() {
            Q::Reported
        } else {
            Q::Unknown
        },
    };
    finish(usage, quality, diagnostics)
}

/// cost 浮点美元 → micro-USD（estimated；上游自行估算，与 cline/zoo 规则相同）。
pub(crate) fn map_family_cost(cost: Option<f64>) -> Option<CostAmount> {
    let total = cost?;
    if !total.is_finite() || total < 0.0 {
        return None;
    }
    let micros = total * 1_000_000.0;
    if micros > i64::MAX as f64 {
        return None;
    }
    Some(CostAmount {
        amount_minor: micros.round() as i64,
        currency: "USD".to_string(),
        kind: CostKind::Estimated,
        price_version: None,
        billing_scope: None,
    })
}

/// step-finish 部件 usage 提取（两产品 pinned 提取规则的共同内核）：
/// - `type == "step-finish"`；
/// - `cost` 与 `tokens` 同时在场（OpenCode projector `usage()` 规则）；
/// - `tokens{input, output, reasoning, cache{read, write}}` 五数字段必需
///   （MiMo zod required；vendored client 同形），`total` 可选；
/// - 数值非负有界，越界/类型错误返回 None（调用方计调用、token 未知）。
pub(crate) fn parse_step_finish(value: &serde_json::Value) -> Option<OpencodeFamilyUsage> {
    let obj = value.as_object()?;
    if obj.get("type").and_then(|t| t.as_str()) != Some("step-finish") {
        return None;
    }
    if !obj.contains_key("cost") || !obj.contains_key("tokens") {
        return None;
    }
    let tokens = obj.get("tokens")?.as_object()?;
    let cache = tokens.get("cache").and_then(|c| c.as_object());
    let num = |v: Option<&serde_json::Value>| {
        v.and_then(|x| x.as_i64())
            .filter(|n| (0..=MAX_REASONABLE_TOKEN).contains(n))
    };
    Some(OpencodeFamilyUsage {
        input: num(tokens.get("input"))?,
        output: num(tokens.get("output"))?,
        reasoning: num(tokens.get("reasoning"))?,
        cache_read: num(cache.and_then(|c| c.get("read")))?,
        cache_write: num(cache.and_then(|c| c.get("write")))?,
        total: num(tokens.get("total")),
    })
}

/// 扫描时注入的产品身份（家族模块不持有产品状态）。
pub(crate) struct PartProduct {
    /// 事件键命名空间（= adapter_id：opencode / mimo-code）。
    pub ns: &'static str,
    /// 统计归属 Agent 名。
    pub agent: &'static str,
    /// 本版本实现的解析器版本串。
    pub parser_version: &'static str,
    /// OpenCode：session.tokens_* 五列对账；MiMo 无累计列不对账。
    pub reconcile_session_counters: bool,
}

/// 游标（持久化在 ingestion_checkpoints.cursor_value）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PartCursor {
    pub generation: i64,
    /// 恒为 0：WAL 活库不用字节偏移做无变化判定（同 kilo 约定）。
    #[allow(dead_code)]
    pub offset: u64,
    /// 已处理到的 part.time_updated（含该值）；None = 从头全量。
    pub watermark_ms: Option<i64>,
}

/// 解析上下文：schema 指纹 + 版本分派结论快照。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PartParseContext {
    pub schema_fingerprint: Option<String>,
    pub version_basis: Option<VersionBasis>,
    pub db_version: Option<String>,
}

fn diag(code: &str, field: Option<&str>, id_pos: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: field.map(str::to_string),
        // 位置只存 part.id（稳定身份，非路径/正文）。
        position: Some(id_pos.to_string()),
        message: message.to_string(),
    }
}

/// 窗口行：step-finish 部件 + 会话/消息归属列。
pub(crate) struct PartRow {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub time_created: i64,
    pub time_updated: i64,
    pub data: String,
    pub parent_id: Option<String>,
    pub session_version: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
}

/// 按已处理时间窗口查询（两产品同形：part LEFT JOIN session/message；
/// message.data 经 json_valid 守卫后取 modelID/providerID，消息正文不入内存）。
fn load_window(conn: &Connection, since_ms: i64) -> Result<Vec<PartRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.message_id, p.session_id, p.time_created, p.time_updated, p.data, \
                 s.parent_id, s.version, \
                 CASE WHEN m.data IS NOT NULL AND json_valid(m.data) \
                      THEN json_extract(m.data, '$.modelID') END, \
                 CASE WHEN m.data IS NOT NULL AND json_valid(m.data) \
                      THEN json_extract(m.data, '$.providerID') END \
             FROM part p \
             LEFT JOIN session s ON s.id = p.session_id \
             LEFT JOIN message m ON m.id = p.message_id \
             WHERE p.time_updated >= ?1 AND json_valid(p.data) \
               AND json_extract(p.data, '$.type') = 'step-finish' \
             ORDER BY p.time_updated, p.id \
             LIMIT ?2",
        )
        .map_err(CoreError::Sqlite)?;
    let rows = stmt
        .query_map(rusqlite::params![since_ms, MAX_ROWS_PER_ROUND], |r| {
            Ok(PartRow {
                id: r.get(0)?,
                message_id: r.get(1)?,
                session_id: r.get(2)?,
                time_created: r.get(3)?,
                time_updated: r.get(4)?,
                data: r.get(5)?,
                parent_id: r.get(6)?,
                session_version: r.get(7)?,
                model_id: r.get(8)?,
                provider_id: r.get(9)?,
            })
        })
        .map_err(CoreError::Sqlite)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)
}

/// 库内数值最大 session.version（两产品 session 表均有该列；空表 → None）。
pub(crate) fn max_session_version(conn: &Connection) -> Result<Option<String>, CoreError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT version FROM session")
        .map_err(CoreError::Sqlite)?;
    let versions = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(CoreError::Sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CoreError::Sqlite)?;
    // 语义最大按数值序比较（同 kilo version_max 约定）。
    Ok(versions
        .into_iter()
        .reduce(|a, b| version_max(&a, &b).to_string()))
}

/// 数值序版本比较（kilo versions::version_max 同语义，家族内复制）。
fn version_max<'a>(a: &'a str, b: &'a str) -> &'a str {
    let parse = |s: &str| -> Vec<i64> {
        s.split('.')
            .map(|p| p.parse::<i64>().unwrap_or(0))
            .collect()
    };
    let (va, vb) = (parse(a), parse(b));
    if va >= vb {
        a
    } else {
        b
    }
}

/// OpenCode 对账：逐次 step-finish 五字段合计 vs session.tokens_* 五列合计
/// （projector applyUsage 语义：五列 = Σ 当前部件，含删行补偿）。
fn reconcile_session_counters(
    conn: &Connection,
    session_id: &str,
) -> Result<Reconciliation, CoreError> {
    let detail: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(json_extract(data,'$.tokens.input')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.output')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.reasoning')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.cache.read')),0) \
                 + COALESCE(SUM(json_extract(data,'$.tokens.cache.write')),0) \
             FROM part WHERE session_id = ?1 AND json_valid(data) \
               AND json_extract(data,'$.type') = 'step-finish'",
            [session_id],
            |r| r.get(0),
        )
        .map_err(CoreError::Sqlite)?;
    let snapshot: Option<i64> = conn
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
    let (difference, verdict) = match snapshot {
        Some(snap) => {
            let diff = detail - snap;
            (Some(diff), if diff == 0 { "matched" } else { "mismatch" })
        }
        None => (None, "no_snapshot"),
    };
    Ok(Reconciliation {
        series: "session_cumulative_snapshot".to_string(),
        detail_sum: detail,
        snapshot_final: snapshot,
        carried_sum: 0,
        difference,
        verdict: verdict.to_string(),
    })
}

/// 家族共享扫描核心：step-finish 部件 → 逐次 model_call 事件 + 已处理位置游标。
/// 调用方（各产品 versions 实现）负责打开只读连接、计算 schema 指纹与版本
/// 分派结论后传入；本函数完成游标/指纹重置/窗口/事件/对账/更新已处理位置。
/// 扫描上下文：探测/分派结论（产品身份、schema 指纹、原始版本与选择依据）。
pub(crate) struct PartScanContext {
    pub product: PartProduct,
    pub fingerprint: String,
    pub db_version: Option<String>,
    pub basis: VersionBasis,
}

pub(crate) fn scan_step_finish_parts(
    conn: &Connection,
    target: &ScanTarget,
    stored: &StoredScanState,
    now_ms: i64,
    ctx: PartScanContext,
) -> Result<ScanOutcome, CoreError> {
    let PartScanContext {
        product,
        fingerprint,
        db_version,
        basis,
    } = ctx;
    // 游标不做 generation 过滤：同一逻辑库 in-place 重写让框架标 Rescan，
    // 但 part.id/更新序号记录的已处理位置仍有效（同 kilo 约定）。
    let cursor: PartCursor = stored
        .cursor
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(PartCursor {
            generation: target.generation,
            offset: 0,
            watermark_ms: None,
        });
    let context: PartParseContext = stored
        .parse_context
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    // schema 指纹变化 ⇒ 旧的已处理位置不可信，全量重读（id 键 upsert 幂等）。
    let fingerprint_reset = context.schema_fingerprint.is_some()
        && context.schema_fingerprint.as_deref() != Some(fingerprint.as_str());
    let watermark = if fingerprint_reset {
        None
    } else {
        cursor.watermark_ms
    };
    let since_ms = watermark
        .map(|w| w.saturating_sub(WATERMARK_OVERLAP_MS))
        .unwrap_or(i64::MIN);

    let rows = load_window(conn, since_ms)?;
    let hit_cap = rows.len() as i64 >= MAX_ROWS_PER_ROUND;

    let mut events: Vec<EventInput> = Vec::new();
    let mut diagnostics: Vec<DiagnosticInput> = Vec::new();
    let mut records_seen: u64 = 0;
    let mut touched_sessions: Vec<String> = Vec::new();
    for row in rows.iter() {
        records_seen += 1;
        if !touched_sessions.iter().any(|s| s == &row.session_id) {
            touched_sessions.push(row.session_id.clone());
        }
        let record_key = format!("{}:part:{}", product.ns, row.id);
        let data: serde_json::Value = match serde_json::from_str(&row.data) {
            Ok(v) => v,
            Err(_) => {
                // json_valid 已在 SQL 层过滤；此处防御游标期间被改写的行。
                diagnostics.push(diag(
                    "bad_data_json",
                    Some("data"),
                    &record_key,
                    "part.data is not valid JSON; row isolated, content not stored",
                ));
                continue;
            }
        };
        // 行时间是派生视图写入时刻：不可信（早于 2000）时跳过不猜。
        if row.time_created < crate::domain::MIN_PLAUSIBLE_MS {
            diagnostics.push(diag(
                "timestamp_implausible",
                Some("time_created"),
                &record_key,
                "part.time_created before 2000-01-01; row skipped, not zero-filled",
            ));
            continue;
        }
        let (usage, quality, cost) = match parse_step_finish(&data) {
            Some(raw) => {
                let mapped = map_opencode_family_usage(&raw);
                (
                    mapped.usage,
                    mapped.quality,
                    map_family_cost(data.get("cost").and_then(|c| c.as_f64())),
                )
            }
            None => {
                // step-finish 在场即表明该 step 已运行：计调用、token 未知。
                diagnostics.push(diag(
                    "usage_shape_deviation",
                    Some("tokens"),
                    &record_key,
                    "step-finish part without usable cost/tokens numbers; call counted, tokens unknown",
                ));
                (
                    TokenUsage::default(),
                    TokenQuality::default(),
                    map_family_cost(data.get("cost").and_then(|c| c.as_f64())),
                )
            }
        };
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: record_key,
            record_kind: RecordKind::ModelCall,
            schema_version: row
                .session_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            parser_version: product.parser_version.to_string(),
            parse_basis: Some(basis),
            origin_call_id: Some(row.message_id.clone()),
            attempt_id: None,
            session_id: Some(row.session_id.clone()),
            parent_session_id: row.parent_id.clone(),
            host_application: None,
            agent: product.agent.to_string(),
            call_category: if row.parent_id.is_some() {
                CallCategory::SubAgent
            } else {
                CallCategory::Primary
            },
            occurred_at_ms: row.time_created,
            observed_at_ms: Some(now_ms),
            source_time: Some(row.time_created.to_string()),
            // part 行写入时刻（≈该 step 完成），非上游精确逐调用时间（A17）。
            time_basis: TimeBasis::ObservedAt,
            interval_start_ms: None,
            interval_end_ms: None,
            provider_id: row.provider_id.clone(),
            model_raw: row.model_id.clone(),
            model_canonical: None,
            model_attribution: if row.model_id.is_some() {
                ModelAttribution::RequestField
            } else {
                ModelAttribution::Unknown
            },
            usage,
            quality,
            // 行级当前值即该 step 终值；后续改写经 time_updated 修订替换。
            lifecycle: Lifecycle::Final,
            source_revision: Some(row.time_updated),
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost,
        });
    }

    // 更新已处理位置：触顶时停在最后一个完整毫秒（该毫秒下轮重读，幂等）。
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
    if product.reconcile_session_counters && status == ScanStatus::Complete {
        for session_id in &touched_sessions {
            let rec = reconcile_session_counters(conn, session_id)?;
            if rec.verdict == "mismatch" {
                diagnostics.push(DiagnosticInput {
                    event_id: None,
                    code: "reconcile_mismatch".to_string(),
                    field: Some("total_tokens".to_string()),
                    position: None,
                    message: format!(
                        "session step-finish detail sum {} != snapshot five-column sum {} (diff {})",
                        rec.detail_sum,
                        rec.snapshot_final.unwrap_or(0),
                        rec.difference.unwrap_or(0)
                    ),
                });
            }
            reconciliations.push(rec);
        }
    }

    let new_cursor = PartCursor {
        generation: target.generation,
        offset: 0,
        watermark_ms: next_watermark,
    };
    let new_context = PartParseContext {
        schema_fingerprint: Some(fingerprint),
        version_basis: Some(basis),
        db_version,
    };
    let degraded = diagnostics.iter().any(|d| {
        matches!(
            d.code.as_str(),
            "bad_data_json" | "usage_shape_deviation" | "reconcile_mismatch"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_step_finish_requires_projector_rule_and_five_fields() {
        let full = serde_json::json!({
            "type": "step-finish", "reason": "stop",
            "cost": 0.012,
            "tokens": {"total": 1230, "input": 1000, "output": 200,
                       "reasoning": 10, "cache": {"read": 15, "write": 5}}
        });
        let usage = parse_step_finish(&full).unwrap();
        assert_eq!(usage.input, 1000);
        assert_eq!(usage.total, Some(1230));

        // 缺 cost 或 tokens（projector 规则）⇒ None。
        assert!(parse_step_finish(&serde_json::json!({
            "type": "step-finish", "tokens": {"input": 1, "output": 1, "reasoning": 0,
                                              "cache": {"read": 0, "write": 0}}
        }))
        .is_none());
        // 非 step-finish 类型 ⇒ None。
        assert!(parse_step_finish(&serde_json::json!({
            "type": "text", "cost": 0.1,
            "tokens": {"input": 1, "output": 1, "reasoning": 0,
                       "cache": {"read": 0, "write": 0}}
        }))
        .is_none());
        // 五数字段缺一/越界 ⇒ None（按 MiMo zod required 定义）。
        assert!(parse_step_finish(&serde_json::json!({
            "type": "step-finish", "cost": 0.1,
            "tokens": {"input": 1, "output": 1, "reasoning": 0,
                       "cache": {"read": 0}}
        }))
        .is_none());
        assert!(parse_step_finish(&serde_json::json!({
            "type": "step-finish", "cost": 0.1,
            "tokens": {"input": -1, "output": 1, "reasoning": 0,
                       "cache": {"read": 0, "write": 0}}
        }))
        .is_none());
    }

    #[test]
    fn map_derives_inclusive_totals_from_pinned_semantics() {
        let m = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: 100,
            output: 40,
            reasoning: 5,
            cache_read: 50,
            cache_write: 10,
            total: Some(205),
        });
        // input 是未缓存输入（pinned publish-llm-event tokens()）：inclusive 派生。
        assert_eq!(m.usage.input_uncached, Some(100));
        assert_eq!(
            m.usage.input_total,
            Some(160),
            "input+cr+cw（上游 sumTokens）"
        );
        assert_eq!(m.usage.output_total, Some(45), "output+reasoning 派生");
        assert_eq!(m.usage.total_tokens, Some(205), "五字段之和");
        assert_eq!(m.usage.source_total, Some(205));
        assert!(m.diagnostics.is_empty(), "直报 total 与派生一致");

        // 缺 total：source_total None，派生总量仍在。
        let missing = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: 10,
            output: 5,
            reasoning: 2,
            cache_read: 0,
            cache_write: 0,
            total: None,
        });
        assert_eq!(missing.usage.source_total, None);
        assert_eq!(missing.usage.total_tokens, Some(17));

        // 直报 total 与派生值不一致时记诊断（包含关系成立才有可比性）。
        let bad = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: 10,
            output: 5,
            reasoning: 0,
            cache_read: 0,
            cache_write: 0,
            total: Some(99),
        });
        assert_eq!(bad.usage.total_tokens, Some(15), "规范化总量按派生口径");
        assert_eq!(bad.usage.source_total, Some(99), "直报值独立保留");
        assert!(bad
            .diagnostics
            .iter()
            .any(|d| d.code == "source_total_mismatch"));
    }

    #[test]
    fn map_overflow_degrades_to_unknown_not_panic() {
        let m = map_opencode_family_usage(&OpencodeFamilyUsage {
            input: i64::MAX,
            output: 1,
            reasoning: 0,
            cache_read: 1,
            cache_write: 0,
            total: None,
        });
        // 溢出的派生字段保持 None（未知），不 panic、不截断数值。
        assert_eq!(m.usage.input_total, None);
        assert_eq!(m.usage.total_tokens, None);
    }

    #[test]
    fn cost_maps_to_estimated_micro_usd() {
        let cost = map_family_cost(Some(0.0125)).unwrap();
        assert_eq!(cost.amount_minor, 12_500);
        assert_eq!(cost.kind, CostKind::Estimated);
        assert!(map_family_cost(None).is_none());
        assert!(map_family_cost(Some(-1.0)).is_none());
    }

    #[test]
    fn version_max_prefers_numeric_order() {
        assert_eq!(version_max("1.9.0", "1.10.0"), "1.10.0");
        assert_eq!(version_max("0.5.1", "0.5.0"), "0.5.1");
    }
}
