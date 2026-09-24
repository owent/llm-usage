//! ingest 批次：事件 upsert + 游标 + 解析上下文 + 受影响日汇总在同一事务提交。
//! 中断前未提交的批次可重放；重放幂等（事件按键 upsert、日汇总按日分区重算）。

use crate::calendar::Calendar;
use crate::domain::{EventInput, QualityBucket};
use crate::error::CoreError;
use crate::identity::{arbitrate, content_hash, event_id, Arbitration, ExistingMeta};
use crate::jobs::{self, RunStats};
use crate::metrics::detect_contradictions;
use crate::storage::Storage;
use jiff::civil::Date;
use rusqlite::{params, Transaction};
use std::collections::BTreeSet;

/// 游标与版本化解析上下文更新（模型状态、累计基线、未完成请求）。
#[derive(Debug, Clone)]
pub struct CheckpointUpdate {
    pub scope_key: String,
    pub cursor_value: Option<serde_json::Value>,
    pub parse_context: Option<serde_json::Value>,
    pub source_revision: Option<i64>,
}

/// 批次内诊断（脱敏：字段名/错误码/位置，不复制原始行内容）。
#[derive(Debug, Clone)]
pub struct DiagnosticInput {
    pub event_id: Option<String>,
    pub code: String,
    pub field: Option<String>,
    pub position: Option<String>,
    pub message: String,
}

/// 一个 ingest 批次。M1 批次内事件属于同一来源实例（跨源并发合入在 M6 接）。
#[derive(Debug, Clone)]
pub struct IngestBatch {
    pub batch_id: String,
    /// 本批次来源实例。
    pub instance_id: String,
    /// 分桶用固定 IANA 时区（保存后不随系统变化）。
    pub timezone: String,
    pub now_ms: i64,
    pub events: Vec<EventInput>,
    pub checkpoints: Vec<CheckpointUpdate>,
    pub diagnostics: Vec<DiagnosticInput>,
    /// 关联的运行中作业：批次统计与其同事务提交。
    pub run_id: Option<String>,
    /// 保留截止（UTC 毫秒）：更早的事件被跳过并记诊断，不允许复活已清理明细。
    pub retention_cutoff_ms: Option<i64>,
}

/// 故障注入点（V09）。在任何定义点失败都必须整体回滚，重放结果相同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    AfterEvents,
    AfterCheckpoint,
    AfterParseContext,
    BeforeAggregates,
    AfterAggregates,
    BeforeCommit,
}

impl FaultPoint {
    fn name(self) -> &'static str {
        match self {
            FaultPoint::AfterEvents => "after_events",
            FaultPoint::AfterCheckpoint => "after_checkpoint",
            FaultPoint::AfterParseContext => "after_parse_context",
            FaultPoint::BeforeAggregates => "before_aggregates",
            FaultPoint::AfterAggregates => "after_aggregates",
            FaultPoint::BeforeCommit => "before_commit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome {
    pub added: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub skipped: i64,
    pub errors: i64,
    pub conflicts: i64,
    pub data_revision: i64,
    /// 本次重算影响的本地日（YYYY-MM-DD）。
    pub affected_days: Vec<String>,
}

fn check_fault(fault: Option<FaultPoint>, point: FaultPoint) -> Result<(), CoreError> {
    if fault == Some(point) {
        return Err(CoreError::FaultInjected(point.name()));
    }
    Ok(())
}

/// 提交批次：单一事务。fault 为测试钩子，注入后整体回滚。
pub fn commit_batch(
    storage: &Storage,
    batch: &IngestBatch,
    fault: Option<FaultPoint>,
) -> Result<BatchOutcome, CoreError> {
    let calendar = Calendar::new(&batch.timezone)?;
    let conn = storage.conn();
    let tx = conn.unchecked_transaction()?;
    let mut outcome = BatchOutcome {
        added: 0,
        updated: 0,
        unchanged: 0,
        skipped: 0,
        errors: 0,
        conflicts: 0,
        data_revision: 0,
        affected_days: Vec::new(),
    };
    let mut affected: BTreeSet<Date> = BTreeSet::new();
    let mut pending_diagnostics: Vec<(Option<String>, DiagnosticInput)> = Vec::new();

    // 1. 事件 upsert。
    for event in &batch.events {
        let eid = event_id(&event.source_instance_id, &event.source_record_key);
        if let Err(e) = event.validate() {
            outcome.errors += 1;
            pending_diagnostics.push((
                Some(eid),
                DiagnosticInput {
                    event_id: None,
                    code: "validation_failed".to_string(),
                    field: None,
                    position: None,
                    message: e.to_string(),
                },
            ));
            continue;
        }
        if let Some(cutoff) = batch.retention_cutoff_ms {
            if event.occurred_at_ms < cutoff {
                outcome.skipped += 1;
                pending_diagnostics.push((
                    Some(eid),
                    DiagnosticInput {
                        event_id: None,
                        code: "expired_by_retention".to_string(),
                        field: Some("occurred_at_ms".to_string()),
                        position: None,
                        message: "event is older than the retention cutoff; not restored".to_string(),
                    },
                ));
                continue;
            }
        }
        let hash = content_hash(event);
        let existing: Option<(ExistingMeta, i64)> = tx
            .query_row(
                "SELECT lifecycle, source_revision, content_hash, occurred_at_ms
                 FROM usage_events WHERE source_instance_id = ?1 AND source_record_key = ?2",
                params![event.source_instance_id, event.source_record_key],
                |r| {
                    Ok((
                        ExistingMeta {
                            lifecycle: crate::domain::Lifecycle::parse(&r.get::<_, String>(0)?)
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                            source_revision: r.get(1)?,
                            content_hash: r.get(2)?,
                        },
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .ok();
        match arbitrate(existing.as_ref().map(|(m, _)| m), event, &hash) {
            Arbitration::Insert => {
                insert_event(&tx, event, &eid, &hash, batch.now_ms)?;
                outcome.added += 1;
                affected.insert(calendar.local_day_of(event.occurred_at_ms)?);
            }
            Arbitration::Replace => {
                update_event(&tx, event, &eid, &hash, batch.now_ms)?;
                outcome.updated += 1;
                affected.insert(calendar.local_day_of(event.occurred_at_ms)?);
                if let Some((_, old_ms)) = existing {
                    affected.insert(calendar.local_day_of(old_ms)?);
                }
            }
            Arbitration::Keep => {
                outcome.unchanged += 1;
            }
            Arbitration::Conflict => {
                // 不任意择大：保留现存，标 conflict 并记诊断；所在日重算以反映冲突计数。
                tx.execute(
                    "UPDATE usage_events SET conflict = 1 WHERE event_id = ?1",
                    params![eid],
                )?;
                outcome.conflicts += 1;
                if let Some((_, old_ms)) = existing {
                    affected.insert(calendar.local_day_of(old_ms)?);
                }
                pending_diagnostics.push((
                    Some(eid.clone()),
                    DiagnosticInput {
                        event_id: None,
                        code: "update_conflict".to_string(),
                        field: None,
                        position: None,
                        message: "incoming record conflicts with existing; kept existing, not MAX".to_string(),
                    },
                ));
            }
        }
        // 数学矛盾进入受限诊断（不用 max(0,…) 隐藏）。
        for contradiction in detect_contradictions(&event.usage) {
            pending_diagnostics.push((
                Some(eid.clone()),
                DiagnosticInput {
                    event_id: None,
                    code: contradiction.code.to_string(),
                    field: Some(contradiction.field.to_string()),
                    position: None,
                    message: contradiction.detail,
                },
            ));
        }
    }
    check_fault(fault, FaultPoint::AfterEvents)?;

    // 2. 游标与解析上下文（同事务；分两段以便故障点语义清晰）。
    for checkpoint in &batch.checkpoints {
        tx.execute(
            "INSERT INTO ingestion_checkpoints (instance_id, scope_key, cursor_value, source_revision, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(instance_id, scope_key) DO UPDATE SET
               cursor_value = excluded.cursor_value,
               source_revision = excluded.source_revision,
               updated_at_ms = excluded.updated_at_ms",
            params![
                batch.instance_id,
                checkpoint.scope_key,
                checkpoint.cursor_value.as_ref().map(serde_json::to_string).transpose()?,
                checkpoint.source_revision,
                batch.now_ms
            ],
        )?;
    }
    check_fault(fault, FaultPoint::AfterCheckpoint)?;
    for checkpoint in &batch.checkpoints {
        if let Some(context) = &checkpoint.parse_context {
            tx.execute(
                "UPDATE ingestion_checkpoints SET parse_context = ?1, updated_at_ms = ?2
                 WHERE instance_id = ?3 AND scope_key = ?4",
                params![
                    serde_json::to_string(context)?,
                    batch.now_ms,
                    batch.instance_id,
                    checkpoint.scope_key
                ],
            )?;
        }
    }
    check_fault(fault, FaultPoint::AfterParseContext)?;

    // 3. 数据修订号单调递增；本批次的日汇总行携带新修订。
    let next_revision = Storage::bump_data_revision_tx(&tx, batch.now_ms)?;
    outcome.data_revision = next_revision;
    check_fault(fault, FaultPoint::BeforeAggregates)?;

    // 4. 受影响日按（时区, 日）分区重算；封存日不追加。
    for day in &affected {
        recompute_day(&tx, &calendar, *day, next_revision)?;
    }
    outcome.affected_days = affected.iter().map(Date::to_string).collect();
    check_fault(fault, FaultPoint::AfterAggregates)?;

    // 5. 诊断（脱敏）。
    for (event_id, diag) in &pending_diagnostics {
        insert_diagnostic(&tx, batch, event_id.as_deref(), diag)?;
    }
    for diag in &batch.diagnostics {
        insert_diagnostic(&tx, batch, diag.event_id.as_deref(), diag)?;
    }

    // 6. 作业进度同事务提交。
    if let Some(run_id) = &batch.run_id {
        jobs::merge_run_stats_tx(
            &tx,
            run_id,
            RunStats {
                added: outcome.added,
                updated: outcome.updated,
                unchanged: outcome.unchanged,
                skipped: outcome.skipped,
                errors: outcome.errors,
            },
            next_revision,
        )?;
    }
    check_fault(fault, FaultPoint::BeforeCommit)?;

    tx.commit()?;
    Ok(outcome)
}

fn insert_diagnostic(
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    event_id: Option<&str>,
    diag: &DiagnosticInput,
) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO diagnostics (batch_id, run_id, instance_id, event_id, code, field, position, message, created_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            batch.batch_id,
            batch.run_id,
            batch.instance_id,
            event_id,
            diag.code,
            diag.field,
            diag.position,
            diag.message,
            batch.now_ms
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    tx: &Transaction<'_>,
    event: &EventInput,
    event_id: &str,
    hash: &str,
    now_ms: i64,
) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO usage_events (
           event_id, source_instance_id, source_record_key, record_kind, schema_version, parser_version,
           origin_call_id, attempt_id, session_id, parent_session_id, host_application, agent, call_category,
           occurred_at_ms, observed_at_ms, source_time, time_basis, interval_start_ms, interval_end_ms,
           provider_id, model_raw, model_canonical, model_attribution,
           input_uncached, input_cache_read, input_cache_write, input_total, output_total, output_reasoning,
           total_tokens, source_total, quality_json, quality_bucket, lifecycle, source_revision,
           error_status, duration_ms, ttft_ms, attribution_status, exclusion_reason, conflict, content_hash,
           cost_amount_minor, cost_currency, cost_kind, price_version, billing_scope,
           created_at_ms, updated_at_ms
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
           ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36,
           ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49
         )",
        rusqlite::params_from_iter(event_params(event, event_id, hash, now_ms, now_ms)),
    )?;
    Ok(())
}

fn update_event(
    tx: &Transaction<'_>,
    event: &EventInput,
    event_id: &str,
    hash: &str,
    now_ms: i64,
) -> Result<(), CoreError> {
    tx.execute(
        "UPDATE usage_events SET
           record_kind = ?4, schema_version = ?5, parser_version = ?6,
           origin_call_id = ?7, attempt_id = ?8, session_id = ?9, parent_session_id = ?10,
           host_application = ?11, agent = ?12, call_category = ?13,
           occurred_at_ms = ?14, observed_at_ms = ?15, source_time = ?16, time_basis = ?17,
           interval_start_ms = ?18, interval_end_ms = ?19,
           provider_id = ?20, model_raw = ?21, model_canonical = ?22, model_attribution = ?23,
           input_uncached = ?24, input_cache_read = ?25, input_cache_write = ?26, input_total = ?27,
           output_total = ?28, output_reasoning = ?29, total_tokens = ?30, source_total = ?31,
           quality_json = ?32, quality_bucket = ?33, lifecycle = ?34, source_revision = ?35,
           error_status = ?36, duration_ms = ?37, ttft_ms = ?38,
           attribution_status = ?39, exclusion_reason = ?40, conflict = 0, content_hash = ?42,
           cost_amount_minor = ?43, cost_currency = ?44, cost_kind = ?45, price_version = ?46,
           billing_scope = ?47, updated_at_ms = ?49
         WHERE event_id = ?1",
        rusqlite::params_from_iter(event_params(event, event_id, hash, 0, now_ms)),
    )?;
    Ok(())
}

/// 与 INSERT/UPDATE 共用的参数顺序；?41 为 conflict（仅 INSERT 使用字面 0，占位一致），
/// ?48 created_at_ms 仅 INSERT 有意义（UPDATE 不引用）。
fn event_params(
    event: &EventInput,
    event_id: &str,
    hash: &str,
    _created_ms: i64,
    now_ms: i64,
) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value;
    let quality_json = serde_json::to_string(&event.quality).unwrap_or_else(|_| "{}".to_string());
    let bucket = QualityBucket::of(&event.usage, &event.quality);
    let (cost_minor, cost_currency, cost_kind, price_version, billing_scope) = match &event.cost {
        Some(c) => (
            Some(c.amount_minor),
            Some(c.currency.clone()),
            Some(c.kind.as_str().to_string()),
            c.price_version.clone(),
            c.billing_scope.clone(),
        ),
        None => (None, None, None, None, None),
    };
    vec![
        Value::Text(event_id.to_string()),                       // 1
        Value::Text(event.source_instance_id.clone()),           // 2
        Value::Text(event.source_record_key.clone()),            // 3
        Value::Text(event.record_kind.as_str().to_string()),     // 4
        Value::Text(event.schema_version.clone()),               // 5
        Value::Text(event.parser_version.clone()),               // 6
        opt_text(&event.origin_call_id),                         // 7
        opt_text(&event.attempt_id),                             // 8
        opt_text(&event.session_id),                             // 9
        opt_text(&event.parent_session_id),                      // 10
        opt_text(&event.host_application),                       // 11
        Value::Text(event.agent.clone()),                        // 12
        Value::Text(event.call_category.as_str().to_string()),   // 13
        Value::Integer(event.occurred_at_ms),                    // 14
        opt_int(event.observed_at_ms),                           // 15
        opt_text(&event.source_time),                            // 16
        Value::Text(event.time_basis.as_str().to_string()),      // 17
        opt_int(event.interval_start_ms),                        // 18
        opt_int(event.interval_end_ms),                          // 19
        opt_text(&event.provider_id),                            // 20
        opt_text(&event.model_raw),                              // 21
        opt_text(&event.model_canonical),                        // 22
        Value::Text(event.model_attribution.as_str().to_string()), // 23
        opt_int(event.usage.input_uncached),                     // 24
        opt_int(event.usage.input_cache_read),                   // 25
        opt_int(event.usage.input_cache_write),                  // 26
        opt_int(event.usage.input_total),                        // 27
        opt_int(event.usage.output_total),                       // 28
        opt_int(event.usage.output_reasoning),                   // 29
        opt_int(event.usage.total_tokens),                       // 30
        opt_int(event.usage.source_total),                       // 31
        Value::Text(quality_json),                               // 32
        Value::Text(bucket.as_str().to_string()),                // 33
        Value::Text(event.lifecycle.as_str().to_string()),       // 34
        opt_int(event.source_revision),                          // 35
        opt_text(&event.error_status),                           // 36
        opt_int(event.duration_ms),                              // 37
        opt_int(event.ttft_ms),                                  // 38
        Value::Text(event.attribution_status.as_str().to_string()), // 39
        opt_text(&event.exclusion_reason),                       // 40
        Value::Integer(0),                                       // 41 conflict 占位
        Value::Text(hash.to_string()),                           // 42
        opt_int(cost_minor),                                     // 43
        opt_text(&cost_currency),                                // 44
        opt_text(&cost_kind),                                    // 45
        opt_text(&price_version),                                // 46
        opt_text(&billing_scope),                                // 47
        Value::Integer(now_ms),                                  // 48 created（INSERT）
        Value::Integer(now_ms),                                  // 49 updated
    ]
}

fn opt_text(value: &Option<String>) -> rusqlite::types::Value {
    match value {
        Some(v) => rusqlite::types::Value::Text(v.clone()),
        None => rusqlite::types::Value::Null,
    }
}

fn opt_int(value: Option<i64>) -> rusqlite::types::Value {
    match value {
        Some(v) => rusqlite::types::Value::Integer(v),
        None => rusqlite::types::Value::Null,
    }
}

/// 重算单个本地日：删除未封存行后从 usage_events 重建。
/// 只有归属已核验的事件进入总计；transport_attempt 只计入 attempt_count。
pub(crate) fn recompute_day(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
    data_revision: i64,
) -> Result<(), CoreError> {
    let day_str = day.to_string();
    let sealed: i64 = tx.query_row(
        "SELECT COUNT(*) FROM daily_usage WHERE tz_version = ?1 AND local_day = ?2 AND sealed = 1",
        params![calendar.tz_name(), day_str],
        |r| r.get(0),
    )?;
    if sealed > 0 {
        return Ok(());
    }
    tx.execute(
        "DELETE FROM daily_usage WHERE tz_version = ?1 AND local_day = ?2",
        params![calendar.tz_name(), day_str],
    )?;
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    tx.execute(
        "INSERT INTO daily_usage (
           tz_version, local_day, agent, provider_id, model_raw, call_category, quality_bucket,
           event_count, call_count, attempt_count, observation_count,
           input_known_sum, input_known_count, input_unknown_count,
           uncached_known_sum, uncached_known_count,
           cache_read_known_sum, cache_read_known_count,
           cache_write_known_sum, cache_write_known_count,
           output_known_sum, output_known_count, output_unknown_count,
           total_known_sum, total_known_count, total_unknown_count,
           ratio_input_sum, ratio_cache_read_sum, ratio_sample_count,
           conflict_count, sealed, data_revision
         )
         SELECT
           ?1, ?2, agent,
           COALESCE(provider_id, ''), COALESCE(model_raw, ''),
           call_category, quality_bucket,
           COUNT(*),
           SUM(CASE WHEN record_kind = 'model_call' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind = 'transport_attempt' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind = 'usage_observation' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN input_total END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_total IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_total IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN input_uncached END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_uncached IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN input_cache_read END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_cache_read IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN input_cache_write END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_cache_write IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN output_total END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND output_total IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND output_total IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN total_tokens END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND total_tokens IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND total_tokens IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_total IS NOT NULL AND input_cache_read IS NOT NULL THEN input_total END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_total IS NOT NULL AND input_cache_read IS NOT NULL THEN input_cache_read END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND input_total IS NOT NULL AND input_cache_read IS NOT NULL THEN 1 ELSE 0 END),
           SUM(conflict),
           0, ?3
         FROM usage_events
         WHERE occurred_at_ms >= ?4 AND occurred_at_ms < ?5
           AND attribution_status = 'verified'
         GROUP BY agent, COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category, quality_bucket",
        params![calendar.tz_name(), day_str, data_revision, start_ms, end_ms],
    )?;
    Ok(())
}
