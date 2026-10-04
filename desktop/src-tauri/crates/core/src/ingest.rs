//! ingest 批次：事件 upsert + 游标 + 解析上下文 + 受影响日汇总在同一事务提交。
//! 中断前未提交的批次可重放；重放幂等（事件按键 upsert、日汇总按日分区重算）。

use crate::calendar::Calendar;
use crate::domain::{EventInput, QualityBucket};
use crate::error::CoreError;
use crate::identity::{
    arbitrate, content_hash, event_content_hash, event_id, Arbitration, ExistingMeta,
};
use crate::jobs::{self, RunStats};
use crate::metrics::detect_contradictions;
use crate::storage::Storage;
use jiff::civil::Date;
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::BTreeSet;

/// usage_events 中已存在记录的去重判定视图。
type ExistingRow = (
    ExistingMeta,
    i64,
    String,
    Option<i64>,
    Option<String>,
    String,
);

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
    let tx = storage.conn().unchecked_transaction()?;
    let outcome = commit_batch_tx(storage, &tx, batch, fault, false)?;
    tx.commit()?;
    Ok(outcome)
}

pub(crate) fn commit_batch_tx(
    storage: &Storage,
    tx: &Transaction<'_>,
    batch: &IngestBatch,
    fault: Option<FaultPoint>,
    archive_snapshot: bool,
) -> Result<BatchOutcome, CoreError> {
    let calendar = Calendar::new(&batch.timezone)?;
    // schema < 3（测试钩子冻结的旧库）没有 parse_basis 列：按旧 schema 降级写入，
    // 新字段不持久化（等价历史行为）。正常运行总是先迁移到 SCHEMA_VERSION。
    let parse_basis_column = storage.schema_version().unwrap_or(u32::MAX) >= 3;
    if batch
        .events
        .iter()
        .any(|e| e.source_instance_id != batch.instance_id)
    {
        return Err(CoreError::Validation(
            "batch events must belong to the batch source instance".into(),
        ));
    }
    if let Some(run_id) = &batch.run_id {
        let running: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM ingest_runs WHERE run_id = ?1 AND instance_id = ?2 AND status = 'running')",
            params![run_id, batch.instance_id], |r| r.get(0),
        )?;
        if !running {
            return Err(CoreError::JobState(
                "batch requires a running job for the same source".into(),
            ));
        }
    }
    let persisted_cutoff: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key = 'detail_retention_floor_ms'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let persisted_cutoff = persisted_cutoff
        .map(|v| {
            v.parse::<i64>()
                .map_err(|_| CoreError::Validation("invalid retention floor".into()))
        })
        .transpose()?;
    let cutoff = batch
        .retention_cutoff_ms
        .into_iter()
        .chain(persisted_cutoff)
        .max();
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
    let mut content_conflicts = BTreeSet::new();

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
        if let Some(cutoff) = cutoff.filter(|_| !archive_snapshot) {
            if event.occurred_at_ms < cutoff {
                outcome.skipped += 1;
                pending_diagnostics.push((
                    Some(eid),
                    DiagnosticInput {
                        event_id: None,
                        code: "expired_by_retention".to_string(),
                        field: Some("occurred_at_ms".to_string()),
                        position: None,
                        message: "event is older than the retention cutoff; not restored"
                            .to_string(),
                    },
                ));
                continue;
            }
        }
        let hash = event_content_hash(event);
        let mut existing: Option<ExistingRow> = tx
            .query_row(
                "SELECT lifecycle, source_revision, content_hash, occurred_at_ms, event_id, observed_at_ms, source_time, parser_version
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
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        // v1 的内容摘要包含 observed_at；仅观察时间改变仍视为同一内容。
        // 同键同内容的重复 final（仅发生/观察/源时间文本不同）是重报而非冲突，同样视为同一内容。
        if let Some((meta, old_ms, _, observed, old_source_time, _)) = &mut existing {
            let mut legacy = event.clone();
            legacy.observed_at_ms = *observed;
            if meta.content_hash == content_hash(&legacy) {
                meta.content_hash = hash.clone();
            }
            legacy.occurred_at_ms = *old_ms;
            legacy.source_time = old_source_time.clone();
            if meta.content_hash == event_content_hash(&legacy) {
                meta.content_hash = hash.clone();
            }
        }
        let eid = existing
            .as_ref()
            .map(|(_, _, id, _, _, _)| id.clone())
            .unwrap_or(eid);
        let arbitration = arbitrate(existing.as_ref().map(|(m, _, _, _, _, _)| m), event, &hash);
        if arbitration == Arbitration::Conflict {
            if let Some(old) = existing
                .as_ref()
                .filter(|old| parser_metadata_upgrade(old, event))
            {
                // 完整旧摘要只允许解析器依据变化；保留原时间、用量和源修订。
                // 同批次已观测的实际冲突不能被后续的元数据更新清除。
                let mut current = event.clone();
                current.occurred_at_ms = old.1;
                current.source_time = old.4.clone();
                let new_hash = event_content_hash(&current);
                let basis_sql = if parse_basis_column {
                    ", parse_basis=?5"
                } else {
                    ""
                };
                let sql = format!(
                    "UPDATE usage_events SET parser_version=?1,content_hash=?2,updated_at_ms=?3,
                     conflict=CASE WHEN ?4 THEN conflict ELSE 0 END{basis_sql} WHERE event_id=?{}",
                    if parse_basis_column { 6 } else { 5 }
                );
                let mut values = vec![
                    rusqlite::types::Value::Text(event.parser_version.clone()),
                    rusqlite::types::Value::Text(new_hash),
                    rusqlite::types::Value::Integer(batch.now_ms),
                    rusqlite::types::Value::Integer(i64::from(content_conflicts.contains(&eid))),
                ];
                if parse_basis_column {
                    values.push(opt_text(&event.parse_basis.map(|b| b.as_str().to_string())));
                }
                values.push(rusqlite::types::Value::Text(eid.clone()));
                tx.execute(&sql, rusqlite::params_from_iter(values))?;
                outcome.updated += 1;
                affected.insert(calendar.local_day_of(old.1)?);
                pending_diagnostics.push((Some(eid), DiagnosticInput {
                    event_id: None,
                    code: "parser_metadata_updated".into(),
                    field: Some("parser_version".into()),
                    position: None,
                    message: "full stored content matched after normalizing parser metadata; usage preserved".into(),
                }));
                continue;
            }
        }
        let arbitration = if arbitration == Arbitration::Conflict
            && existing
                .as_ref()
                .is_some_and(|(meta, _, _, _, _, _)| vs_copilot_policy_upgrade(meta, event))
        {
            Arbitration::Replace
        } else {
            arbitration
        };
        match arbitration {
            Arbitration::Insert => {
                insert_event(tx, event, &eid, &hash, batch.now_ms, parse_basis_column)?;
                outcome.added += 1;
                affected.insert(calendar.local_day_of(event.occurred_at_ms)?);
            }
            Arbitration::Replace => {
                update_event(tx, event, &eid, &hash, batch.now_ms, parse_basis_column)?;
                outcome.updated += 1;
                affected.insert(calendar.local_day_of(event.occurred_at_ms)?);
                if let Some((_, old_ms, _, _, _, _)) = existing {
                    affected.insert(calendar.local_day_of(old_ms)?);
                }
            }
            Arbitration::Keep => {
                outcome.unchanged += 1;
            }
            Arbitration::Conflict => {
                content_conflicts.insert(eid.clone());
                // 不任意择大：保留现存，标 conflict 并记诊断；所在日重算以反映冲突计数。
                tx.execute(
                    "UPDATE usage_events SET conflict = 1 WHERE event_id = ?1",
                    params![eid],
                )?;
                outcome.conflicts += 1;
                if let Some((_, old_ms, _, _, _, _)) = existing {
                    affected.insert(calendar.local_day_of(old_ms)?);
                }
                pending_diagnostics.push((
                    Some(eid.clone()),
                    DiagnosticInput {
                        event_id: None,
                        code: "update_conflict".to_string(),
                        field: None,
                        position: None,
                        message: "incoming record conflicts with existing; kept existing, not MAX"
                            .to_string(),
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
    crate::copilot_carriers::upgrade_otel_identity(tx, batch, &calendar, &mut affected)?;
    crate::copilot_carriers::select(tx, batch, &calendar, &mut affected)?;
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
    let next_revision = if affected.is_empty() {
        crate::storage::data_revision(tx)?
    } else {
        Storage::bump_data_revision_tx(tx, batch.now_ms)?
    };
    outcome.data_revision = next_revision;
    check_fault(fault, FaultPoint::BeforeAggregates)?;

    // 4. 受影响日按（时区, 日）分区重算；封存日不追加。小时分桶同事务持久化
    //    （分级归档：明细删除后小时层仍有数据）。
    for day in &affected {
        recompute_day(tx, &calendar, *day, next_revision)?;
        persist_hourly_day(tx, &calendar, *day, next_revision)?;
    }
    outcome.affected_days = affected.iter().map(Date::to_string).collect();
    check_fault(fault, FaultPoint::AfterAggregates)?;

    // 5. 诊断（脱敏）。
    for (event_id, diag) in &pending_diagnostics {
        insert_diagnostic(tx, batch, event_id.as_deref(), diag)?;
    }
    for diag in &batch.diagnostics {
        insert_diagnostic(tx, batch, diag.event_id.as_deref(), diag)?;
    }

    // 6. 作业进度同事务提交。
    if let Some(run_id) = &batch.run_id {
        jobs::merge_run_stats_tx(
            tx,
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

    Ok(outcome)
}

/// 比较完整旧事件摘要，不把解析器更名当作源修订或允许其他字段变化。
fn parser_metadata_upgrade(old: &ExistingRow, event: &EventInput) -> bool {
    if old.5 == event.parser_version {
        return false;
    }
    let mut legacy = event.clone();
    legacy.parser_version = old.5.clone();
    // 同键重复 final 的时间兼容与普通仲裁一致；元数据更新仍保留原时间。
    for preserve_time in [false, true] {
        if preserve_time {
            legacy.occurred_at_ms = old.1;
            legacy.source_time = old.4.clone();
        }
        if event_content_hash(&legacy) == old.0.content_hash {
            return true;
        }
        legacy.observed_at_ms = old.3;
        if content_hash(&legacy) == old.0.content_hash {
            return true;
        }
    }
    false
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
    parse_basis_column: bool,
) -> Result<(), CoreError> {
    let sql = if parse_basis_column {
        "INSERT INTO usage_events (
           event_id, source_instance_id, source_record_key, record_kind, schema_version, parser_version,
           origin_call_id, attempt_id, session_id, parent_session_id, host_application, agent, call_category,
           occurred_at_ms, observed_at_ms, source_time, time_basis, interval_start_ms, interval_end_ms,
           provider_id, model_raw, model_canonical, model_attribution,
           input_uncached, input_cache_read, input_cache_write, input_total, output_total, output_reasoning,
           total_tokens, source_total, quality_json, quality_bucket, lifecycle, source_revision,
           error_status, duration_ms, ttft_ms, attribution_status, exclusion_reason, conflict, content_hash,
           cost_amount_minor, cost_currency, cost_kind, price_version, billing_scope,
           created_at_ms, updated_at_ms, parse_basis
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
           ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36,
           ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50
         )"
    } else {
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
         )"
    };
    let mut params = event_params(event, event_id, hash, now_ms, now_ms);
    if !parse_basis_column {
        params.pop();
    }
    tx.execute(sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

fn update_event(
    tx: &Transaction<'_>,
    event: &EventInput,
    event_id: &str,
    hash: &str,
    now_ms: i64,
    parse_basis_column: bool,
) -> Result<(), CoreError> {
    let sql = if parse_basis_column {
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
           billing_scope = ?47, updated_at_ms = ?49, parse_basis = ?50
         WHERE event_id = ?1"
    } else {
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
         WHERE event_id = ?1"
    };
    let mut params = event_params(event, event_id, hash, 0, now_ms);
    if !parse_basis_column {
        params.pop();
    }
    tx.execute(sql, rusqlite::params_from_iter(params))?;
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
        Value::Text(event_id.to_string()),                            // 1
        Value::Text(event.source_instance_id.clone()),                // 2
        Value::Text(event.source_record_key.clone()),                 // 3
        Value::Text(event.record_kind.as_str().to_string()),          // 4
        Value::Text(event.schema_version.clone()),                    // 5
        Value::Text(event.parser_version.clone()),                    // 6
        opt_text(&event.origin_call_id),                              // 7
        opt_text(&event.attempt_id),                                  // 8
        opt_text(&event.session_id),                                  // 9
        opt_text(&event.parent_session_id),                           // 10
        opt_text(&event.host_application),                            // 11
        Value::Text(event.agent.clone()),                             // 12
        Value::Text(event.call_category.as_str().to_string()),        // 13
        Value::Integer(event.occurred_at_ms),                         // 14
        opt_int(event.observed_at_ms),                                // 15
        opt_text(&event.source_time),                                 // 16
        Value::Text(event.time_basis.as_str().to_string()),           // 17
        opt_int(event.interval_start_ms),                             // 18
        opt_int(event.interval_end_ms),                               // 19
        opt_text(&event.provider_id),                                 // 20
        opt_text(&event.model_raw),                                   // 21
        opt_text(&event.model_canonical),                             // 22
        Value::Text(event.model_attribution.as_str().to_string()),    // 23
        opt_int(event.usage.input_uncached),                          // 24
        opt_int(event.usage.input_cache_read),                        // 25
        opt_int(event.usage.input_cache_write),                       // 26
        opt_int(event.usage.input_total),                             // 27
        opt_int(event.usage.output_total),                            // 28
        opt_int(event.usage.output_reasoning),                        // 29
        opt_int(event.usage.total_tokens),                            // 30
        opt_int(event.usage.source_total),                            // 31
        Value::Text(quality_json),                                    // 32
        Value::Text(bucket.as_str().to_string()),                     // 33
        Value::Text(event.lifecycle.as_str().to_string()),            // 34
        opt_int(event.source_revision),                               // 35
        opt_text(&event.error_status),                                // 36
        opt_int(event.duration_ms),                                   // 37
        opt_int(event.ttft_ms),                                       // 38
        Value::Text(event.attribution_status.as_str().to_string()),   // 39
        opt_text(&event.exclusion_reason),                            // 40
        Value::Integer(0),                                            // 41 conflict 占位
        Value::Text(hash.to_string()),                                // 42
        opt_int(cost_minor),                                          // 43
        opt_text(&cost_currency),                                     // 44
        opt_text(&cost_kind),                                         // 45
        opt_text(&price_version),                                     // 46
        opt_text(&billing_scope),                                     // 47
        Value::Integer(now_ms),                                       // 48 created（INSERT）
        Value::Integer(now_ms),                                       // 49 updated
        opt_text(&event.parse_basis.map(|b| b.as_str().to_string())), // 50 版本选择依据
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
    tx.execute(
        "DELETE FROM daily_usage WHERE tz_version = ?1 AND local_day = ?2 AND sealed = 0",
        params![calendar.tz_name(), day_str],
    )?;
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    // v4 起 daily_usage 按来源实例分区（M1a：查询跨来源求和，落盘保留每来源贡献）。
    // v2 迁移的旧库重算发生在 v4 分区之前，按旧形状（无 instance_id 列）写入。
    let partitioned: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('daily_usage') WHERE name = 'instance_id')",
        [],
        |r| r.get(0),
    )?;
    let insert_head = if partitioned {
        "INSERT INTO daily_usage (
           tz_version, local_day, instance_id, agent, provider_id, model_raw, call_category, quality_bucket,
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
           ?1, ?2, source_instance_id, agent,"
    } else {
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
           ?1, ?2, agent,"
    };
    let group_by = if partitioned {
        "GROUP BY source_instance_id, agent, COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category, quality_bucket"
    } else {
        "GROUP BY agent, COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category, quality_bucket"
    };
    let unsealed = if partitioned {
        "AND NOT EXISTS (SELECT 1 FROM daily_usage d WHERE d.tz_version=?1
         AND d.local_day=?2 AND d.instance_id=source_instance_id AND d.sealed=1)"
    } else {
        "AND NOT EXISTS (SELECT 1 FROM daily_usage d WHERE d.tz_version=?1 AND d.local_day=?2 AND d.sealed=1)"
    };
    // 未知字段计数（input/output/total_unknown_count）排除 quality_bucket='unknown'
    // 的记录：没有任何已知 token 字段的记录是“无用量调用/观测”（失败调用、
    // Copilot 工具循环 round 等计调用但用量由 turn observation 承载），计入
    // call/event，但不算作观测缺字段的未知字段（数据规范「请求、消息与累计值」）。
    let sql = format!(
        "{insert_head}
           COALESCE(provider_id, ''), COALESCE(model_raw, ''),
           call_category, quality_bucket,
           COUNT(*),
           SUM(CASE WHEN record_kind = 'model_call' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind = 'transport_attempt' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind = 'usage_observation' THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_input END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND quality_bucket != 'unknown' AND known_input IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_uncached END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_uncached IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_read END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_read IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_write END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_write IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_output END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_output IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND quality_bucket != 'unknown' AND known_output IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' THEN known_total END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_total IS NOT NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND quality_bucket != 'unknown' AND known_total IS NULL THEN 1 ELSE 0 END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL AND known_read IS NOT NULL THEN known_input END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL AND known_read IS NOT NULL THEN known_read END),
           SUM(CASE WHEN record_kind != 'transport_attempt' AND known_input IS NOT NULL AND known_read IS NOT NULL THEN 1 ELSE 0 END),
           SUM(conflict),
           0, ?3
         FROM (
           SELECT *,
             CASE WHEN json_extract(quality_json, '$.input_total') IN ('reported', 'derived') THEN input_total END AS known_input,
             CASE WHEN json_extract(quality_json, '$.input_uncached') IN ('reported', 'derived') THEN input_uncached END AS known_uncached,
             CASE WHEN json_extract(quality_json, '$.input_cache_read') IN ('reported', 'derived') THEN input_cache_read END AS known_read,
             CASE WHEN json_extract(quality_json, '$.input_cache_write') IN ('reported', 'derived') THEN input_cache_write END AS known_write,
             CASE WHEN json_extract(quality_json, '$.output_total') IN ('reported', 'derived') THEN output_total END AS known_output,
             CASE WHEN json_extract(quality_json, '$.total_tokens') IN ('reported', 'derived') THEN total_tokens END AS known_total
           FROM usage_events
         )
         WHERE occurred_at_ms >= ?4 AND occurred_at_ms < ?5
           AND attribution_status = 'verified'
           AND record_kind IN ('model_call', 'transport_attempt', 'usage_observation')
         {unsealed} {group_by}"
    );
    tx.execute(
        &sql,
        params![calendar.tz_name(), day_str, data_revision, start_ms, end_ms],
    )?;
    Ok(())
}

/// Only the verified v1/v2 omission is repairable: every other source field must match.
fn vs_copilot_policy_upgrade(existing: &crate::identity::ExistingMeta, event: &EventInput) -> bool {
    let expected_total = event
        .usage
        .input_total
        .zip(event.usage.output_total)
        .and_then(|(i, o)| i.checked_add(o))
        .filter(|v| *v <= crate::domain::MAX_TOKEN_VALUE);
    let expected_quality = if expected_total.is_some() {
        crate::domain::FieldQuality::Derived
    } else {
        crate::domain::FieldQuality::Unknown
    };
    if event.agent != "vs-copilot"
        || event.parser_version != "vs-copilot-otlp-traces-3"
        || event.quality.total_tokens != expected_quality
        || event.usage.total_tokens != expected_total
    {
        return false;
    }
    let mut legacy = event.clone();
    legacy.usage.total_tokens = None;
    legacy.quality.total_tokens = crate::domain::FieldQuality::Unknown;
    ["vs-copilot-otlp-traces-1", "vs-copilot-otlp-traces-2"]
        .into_iter()
        .any(|parser| {
            legacy.parser_version = parser.into();
            existing.content_hash == event_content_hash(&legacy)
        })
}

/// 按目标时区重算 [from_ms, to_ms] 覆盖的本地日（维护/修复路径：时区分区修复）。
/// 事件仍在 ⇒ 重算是推导不是猜测；该时区下已封存的日跳过；单事务 + 修订号。
/// 返回新数据修订号；范围内无未封存重算时返回当前修订号。
pub fn recompute_days_in_tz(
    storage: &Storage,
    timezone: &str,
    from_ms: i64,
    to_ms: i64,
    now_ms: i64,
) -> Result<i64, CoreError> {
    let calendar = Calendar::new(timezone)?;
    let mut day = calendar.local_day_of(from_ms)?;
    let last = calendar.local_day_of(to_ms)?;
    // 有界防护：最多重算 750 天（超出报错，由调用方分批）。
    let mut guard = 0;
    let conn = storage.conn();
    let tx = conn.unchecked_transaction()?;
    let revision = Storage::bump_data_revision_tx(&tx, now_ms)?;
    while day <= last {
        guard += 1;
        if guard > 750 {
            return Err(CoreError::Validation(
                "recompute_days_in_tz: range exceeds 750 days; batch the repair".into(),
            ));
        }
        recompute_day(&tx, &calendar, day, revision)?;
        persist_hourly_day(&tx, &calendar, day, revision)?;
        day = day
            .checked_add(jiff::Span::new().days(1))
            .map_err(|e| CoreError::Validation(format!("day advance: {e}")))?;
    }
    tx.commit()?;
    Ok(revision)
}

/// 小时分桶持久化（分级归档的 30 天层）：按本地日重算该日各小时分桶行，
/// 维度与日汇总一致（实例/Agent/provider/模型/类别/质量桶）。
/// 与 recompute_day 同事务调用；封存日的小时层同样冻结（不重算）。
/// 小时换算按事件时刻的本地偏移（DST 日 23/25 小时自然正确）。
pub(crate) fn persist_hourly_day(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
    data_revision: i64,
) -> Result<(), CoreError> {
    // schema < 5（测试钩子冻结的旧库）没有 hourly_usage 表：跳过
    //（迁移到当前版本后自然生效；不影响日/明细层语义）。
    let has_table: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('hourly_usage'))",
        [],
        |r| r.get(0),
    )?;
    if !has_table {
        return Ok(());
    }
    let tz = calendar.tz_name().to_string();
    let day_str = day.to_string();
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    tx.execute(
        "DELETE FROM hourly_usage WHERE tz_version = ?1 AND local_day = ?2
         AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.tz_version=?1 AND d.local_day=?2
           AND d.instance_id=hourly_usage.instance_id AND d.sealed=1)",
        params![tz, day_str],
    )?;
    let mut stmt = tx.prepare(
        "SELECT source_instance_id, occurred_at_ms, record_kind, agent,
                COALESCE(provider_id, ''), COALESCE(model_raw, ''), call_category,
                quality_json, input_total, input_cache_read, input_cache_write,
                output_total, total_tokens, quality_bucket, conflict
         FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
           AND attribution_status = 'verified'
           AND NOT EXISTS(SELECT 1 FROM daily_usage d WHERE d.tz_version=?3 AND d.local_day=?4
             AND d.instance_id=source_instance_id AND d.sealed=1)
           AND record_kind IN ('model_call', 'transport_attempt', 'usage_observation')",
    )?;
    #[derive(Default)]
    struct Bucket {
        event_count: i64,
        call_count: i64,
        conflicts: i64,
        input: Option<i64>,
        cache_read: Option<i64>,
        cache_write: Option<i64>,
        output: Option<i64>,
        total: Option<i64>,
    }
    type HourKey = (u32, String, String, String, String, String, String);
    let mut buckets: std::collections::BTreeMap<HourKey, Bucket> = Default::default();
    let mut rows = stmt.query(params![start_ms, end_ms, tz, day_str])?;
    while let Some(row) = rows.next()? {
        let hour = calendar.local_hour_of(row.get(1)?)?;
        let kind: String = row.get(2)?;
        let key = (
            hour,
            row.get(0)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(13)?,
        );
        let bucket = buckets.entry(key).or_default();
        bucket.event_count += 1;
        bucket.call_count += i64::from(kind == "model_call");
        bucket.conflicts += row.get::<_, i64>(14)?;
        if kind == "transport_attempt" {
            continue;
        }
        let quality: serde_json::Value = serde_json::from_str(&row.get::<_, String>(7)?)?;
        for (sum, field, index) in [
            (&mut bucket.input, "input_total", 8),
            (&mut bucket.cache_read, "input_cache_read", 9),
            (&mut bucket.cache_write, "input_cache_write", 10),
            (&mut bucket.output, "output_total", 11),
            (&mut bucket.total, "total_tokens", 12),
        ] {
            if matches!(quality[field].as_str(), Some("reported" | "derived")) {
                if let Some(value) = row.get::<_, Option<i64>>(index)? {
                    *sum = Some(
                        sum.unwrap_or(0)
                            .checked_add(value)
                            .ok_or(CoreError::Overflow("hourly tokens"))?,
                    );
                }
            }
        }
    }
    drop(rows);
    drop(stmt);
    let mut insert = tx.prepare(
        "INSERT INTO hourly_usage (
           tz_version, local_day, hour, instance_id, agent, provider_id, model_raw,
           call_category, quality_bucket, event_count, call_count,
           input_known_sum, cache_read_known_sum, cache_write_known_sum,
           output_known_sum, total_known_sum, conflict_count, data_revision
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
    )?;
    for ((hour, instance, agent, provider, model, category, quality), bucket) in buckets {
        insert.execute(params![
            tz,
            day_str,
            hour,
            instance,
            agent,
            provider,
            model,
            category,
            quality,
            bucket.event_count,
            bucket.call_count,
            bucket.input,
            bucket.cache_read,
            bucket.cache_write,
            bucket.output,
            bucket.total,
            bucket.conflicts,
            data_revision
        ])?;
    }
    Ok(())
}
