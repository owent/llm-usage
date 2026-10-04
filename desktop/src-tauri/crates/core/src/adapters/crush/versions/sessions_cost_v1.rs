//! Crush crush.db 格式实现（`sessions_cost_v1`，crush-sessions-cost-1）。
//!
//! 格式依据（charmbracelet/crush 固定源码
//! 1f3827bcd2d20f38076b2d46123683271e6ed9ba；本机未安装、无真实样本）：
//! - 每项目一库 `<data_dir>/crush.db`（WAL）；sessions 表
//!   id/parent_session_id/title/message_count/prompt_tokens/completion_tokens/
//!   cost/updated_at/created_at（Unix 秒）。
//! - **token 列不是用量**（官方 agent.go:2060-2086：最近 step 上下文规模快照，
//!   SET 覆盖、摘要后重置、标题请求会额外加一次）⇒ 一律不采
//!   （求和即虚增；官方 stats.sql 的 SUM 是上游的近似计算方式，不沿用）。
//! - **cost 是累计**（agent.go:2065），子会话结束回卷父行
//!   （coordinator.go:1742-1758）⇒ 只取 parent_session_id IS NULL 根行
//!   防双计（官方统计查询规则相同）。
//! - 交付形态：每根会话一条 UsageObservation 事件（cost-only；token 全
//!   Unknown）。cost 由模型费率自算（含 OpenRouter 覆盖价、FlatRate=0、
//!   估算 usage 时 0）⇒ CostKind::Estimated（micro-USD）。
//! - 增量：整表读（≤50k 行），事件键 crush:&lt;db 指纹&gt;:&lt;session id&gt;，
//!   内容哈希幂等（cost 增长时按请求更新语义推进）。

use crate::adapters::crush::common::{open_source_db, short_probe, StagingLimits};
use crate::adapters::framework::{
    ScanLimits, ScanOutcome, ScanStatus, ScanTarget, StoredScanState,
};
use crate::domain::{
    AttributionStatus, CallCategory, CostAmount, CostKind, EventInput, Lifecycle, ModelAttribution,
    RecordKind, TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::DiagnosticInput;

use super::CRUSH_FORMAT_VERSION;

pub const CRUSH_PARSER_VERSION: &str = "crush-sessions-cost-1";
pub const MAX_ROWS_PER_ROUND: i64 = 50_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CrushCursor {
    generation: i64,
    #[allow(dead_code)]
    offset: u64,
    #[serde(default)]
    last_session_id: String,
}

fn diag(code: &str, position: &str, message: &str) -> DiagnosticInput {
    DiagnosticInput {
        event_id: None,
        code: code.to_string(),
        field: None,
        position: Some(position.to_string()),
        message: message.to_string(),
    }
}

fn seconds_to_ms(secs: i64) -> Option<i64> {
    let ms = secs.checked_mul(1000)?;
    (crate::domain::MIN_PLAUSIBLE_MS..=4_102_444_800_000)
        .contains(&ms)
        .then_some(ms)
}

fn usd_cost(amount: f64) -> Option<CostAmount> {
    // 零 cost（列默认 0.0 / FlatRate 模型）无信息量：不产观测事件。
    if !amount.is_finite() || amount <= 0.0 {
        return None;
    }
    let micros = amount * 1_000_000.0;
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

pub fn scan(
    target: &ScanTarget,
    stored: &StoredScanState,
    _limits: &ScanLimits,
    now_ms: i64,
) -> Result<ScanOutcome, CoreError> {
    let db = open_source_db(&target.path, short_probe, &StagingLimits::default())?;
    let after_id = if target.rescan {
        String::new()
    } else {
        stored
            .cursor
            .as_ref()
            .and_then(|v| serde_json::from_value::<CrushCursor>(v.clone()).ok())
            .filter(|c| c.generation == target.generation)
            .map(|c| c.last_session_id)
            .unwrap_or_default()
    };
    // 多取一行判定是否还有更多（恰好 MAX 行不误报 BudgetExhausted）。
    let mut stmt = db.conn().prepare(
        "SELECT id, title, cost, created_at, updated_at
         FROM sessions WHERE parent_session_id IS NULL AND id > ?1
         ORDER BY id LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![&after_id, MAX_ROWS_PER_ROUND + 1], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<f64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    let mut records_seen: u64 = 0;
    let mut last_session_id = after_id;
    for row in rows {
        crate::adapters::run_policy::check()?;
        // 行级容错：单行类型错误不中止整轮（SQLite 动态类型）。
        let (session_id, _title, cost, created_at, updated_at) = match row {
            Ok(r) => r,
            Err(e) => {
                records_seen += 1;
                diagnostics.push(diag(
                    "row_read_failed",
                    "sessions",
                    &format!("row read failed: {e}; row skipped"),
                ));
                continue;
            }
        };
        records_seen += 1;
        last_session_id = session_id.clone();
        let Some(cost) = cost.and_then(usd_cost) else {
            continue;
        };
        let end_ms = updated_at.and_then(seconds_to_ms);
        let Some(occurred_ms) = end_ms.or(created_at.and_then(seconds_to_ms)) else {
            diagnostics.push(diag(
                "timestamp_unparseable",
                &format!("session:{session_id}"),
                "created_at/updated_at missing/implausible; session skipped",
            ));
            continue;
        };
        events.push(EventInput {
            source_instance_id: target.instance_id.clone(),
            source_record_key: format!("crush:session:{session_id}"),
            // cost-only 观测：不是一次模型调用，token 全 Unknown（快照列不采）。
            record_kind: RecordKind::UsageObservation,
            schema_version: CRUSH_FORMAT_VERSION.to_string(),
            parser_version: CRUSH_PARSER_VERSION.to_string(),
            parse_basis: Some(VersionBasis::KnownVersion),
            origin_call_id: None,
            attempt_id: None,
            session_id: Some(session_id),
            parent_session_id: None,
            host_application: None,
            agent: "crush".to_string(),
            call_category: CallCategory::Unknown,
            occurred_at_ms: occurred_ms,
            observed_at_ms: Some(now_ms),
            source_time: updated_at.map(|s| s.to_string()),
            time_basis: TimeBasis::Uncertain,
            interval_start_ms: created_at.and_then(seconds_to_ms),
            interval_end_ms: end_ms,
            provider_id: None,
            model_raw: None,
            model_canonical: None,
            model_attribution: ModelAttribution::Unknown,
            usage: crate::domain::TokenUsage::default(),
            quality: crate::domain::TokenQuality::default(),
            lifecycle: Lifecycle::Final,
            // cost 是根会话的累计快照；updated_at 为同一 session 的
            // 修订次序，否则第二次扫描的增长会与旧 Final 事件冲突。
            source_revision: end_ms,
            error_status: None,
            duration_ms: None,
            ttft_ms: None,
            attribution_status: AttributionStatus::Verified,
            exclusion_reason: None,
            cost: Some(cost),
        });
    }
    let hit_cap = records_seen > MAX_ROWS_PER_ROUND as u64;
    Ok(ScanOutcome {
        status: if hit_cap {
            ScanStatus::BudgetExhausted
        } else {
            ScanStatus::Complete
        },
        cursor: Some(serde_json::to_value(CrushCursor {
            generation: target.generation,
            offset: 0,
            last_session_id: if hit_cap {
                last_session_id
            } else {
                String::new()
            },
        })?),
        parse_context: None,
        events,
        aggregates: Vec::new(),
        diagnostics,
        lines_read: records_seen,
        records_seen,
        reconciliations: Vec::new(),
        health: "active".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_gate() {
        assert_eq!(usd_cost(0.5).unwrap().amount_minor, 500_000);
        assert!(usd_cost(-0.1).is_none());
        assert!(usd_cost(f64::NAN).is_none());
    }
}
