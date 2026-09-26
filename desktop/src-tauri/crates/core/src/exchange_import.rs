//! 聚合交换包导入（M1a 合同的实施，2026-09-26 用户需求）：
//! 同分区键修订比较——更高修订替换、同修订覆盖（重复导出导入不缺失）、更低修订冲突保留现存
//! \+ 诊断；新键插入；不覆盖已有来源归属；导入主机登记为外部。
//! 单事务；明细 records 导入随后续功能排期（聚合层已可完整重建历史趋势）。

use crate::error::CoreError;
use crate::exchange::{ExchangeExport, ExchangeHost, EXCHANGE_FORMAT_VERSION};
use crate::storage::Storage;
use rusqlite::{params, OptionalExtension, Transaction};

/// 聚合导入结果（计数供 UI 展示）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImportAggregateOutcome {
    pub sources_registered: usize,
    pub daily_inserted: usize,
    pub daily_replaced: usize,
    pub daily_skipped: usize,
    pub daily_conflicts: usize,
    pub hourly_inserted: usize,
    pub hourly_replaced: usize,
    pub hourly_skipped: usize,
}

/// 导入聚合交换包。见模块头合同。
pub fn import_aggregate(
    storage: &Storage,
    export: &ExchangeExport,
    now_ms: i64,
) -> Result<ImportAggregateOutcome, CoreError> {
    if export.format_version != EXCHANGE_FORMAT_VERSION {
        return Err(CoreError::Validation(format!(
            "unsupported exchange format {:?}; expected {EXCHANGE_FORMAT_VERSION}",
            export.format_version
        )));
    }
    let mut outcome = ImportAggregateOutcome::default();
    let conn = storage.conn();
    let tx = conn.unchecked_transaction()?;
    register_host(&tx, &export.host, now_ms)?;
    for source in &export.sources {
        // 已存在的来源不覆盖（本机采集的归属/启用状态优先）。
        tx.execute(
            "INSERT INTO source_instances (
               instance_id, agent, host_application, locality_basis, attribution_status,
               exclusion_reason, enabled, format, location_hint, parser_version,
               capabilities, health, origin_host_id, user_id, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, NULL, 'remote_sync', 'verified', NULL, 0, ?3, NULL, ?4, '{}', 'ok', ?5, 'default', ?6, ?6)
             ON CONFLICT(instance_id) DO NOTHING",
            params![
                source.source_instance_id,
                source.agent,
                source.format,
                source.parser_version,
                export.host.origin_host_id,
                now_ms
            ],
        )?;
        outcome.sources_registered += 1;
    }
    for p in &export.daily_partitions {
        let existing_rev: Option<i64> = tx
            .query_row(
                "SELECT data_revision FROM daily_usage
                 WHERE tz_version = ?1 AND local_day = ?2 AND instance_id = ?3
                   AND agent = ?4 AND provider_id = ?5 AND model_raw = ?6
                   AND call_category = ?7 AND quality_bucket = ?8",
                params![
                    p.tz_version,
                    p.local_day,
                    p.instance_id,
                    p.agent,
                    p.provider_id,
                    p.model_raw,
                    p.call_category,
                    p.quality_bucket
                ],
                |r| r.get(0),
            )
            .optional()?;
        match existing_rev {
            None => {
                tx.execute(
                    "INSERT INTO daily_usage (
                       tz_version, local_day, instance_id, agent, provider_id, model_raw,
                       call_category, quality_bucket, event_count, call_count, attempt_count,
                       observation_count, input_known_sum, input_known_count, input_unknown_count,
                       uncached_known_sum, uncached_known_count,
                       cache_read_known_sum, cache_read_known_count,
                       cache_write_known_sum, cache_write_known_count,
                       output_known_sum, output_known_count, output_unknown_count,
                       total_known_sum, total_known_count, total_unknown_count,
                       ratio_input_sum, ratio_cache_read_sum, ratio_sample_count,
                       conflict_count, sealed, sealed_at_ms, seal_tz, seal_field_version,
                       seal_source_version, data_revision
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0,
                       ?11, CASE WHEN ?11 IS NULL THEN 0 ELSE 1 END, 0,
                       NULL, 0, ?12, 0, ?13, 0, ?14, 0, 0,
                       ?15, CASE WHEN ?15 IS NULL THEN 0 ELSE 1 END, 0,
                       NULL, NULL, 0, ?16, ?17, NULL, NULL, NULL, NULL, ?18)",
                    params![
                        p.tz_version,
                        p.local_day,
                        p.instance_id,
                        p.agent,
                        p.provider_id,
                        p.model_raw,
                        p.call_category,
                        p.quality_bucket,
                        p.event_count,
                        p.call_count,
                        p.input_known_sum,
                        p.cache_read_known_sum,
                        p.cache_write_known_sum,
                        p.output_known_sum,
                        p.total_known_sum,
                        p.conflict_count,
                        if p.sealed { 1 } else { 0 },
                        p.data_revision
                    ],
                )?;
                outcome.daily_inserted += 1;
            }
            Some(existing_rev) if p.data_revision > existing_rev => {
                tx.execute(
                    "UPDATE daily_usage SET
                       event_count = ?9, call_count = ?10,
                       input_known_sum = ?11, cache_read_known_sum = ?12,
                       cache_write_known_sum = ?13, output_known_sum = ?14,
                       total_known_sum = ?15, conflict_count = ?16, data_revision = ?17
                     WHERE tz_version = ?1 AND local_day = ?2 AND instance_id = ?3
                       AND agent = ?4 AND provider_id = ?5 AND model_raw = ?6
                       AND call_category = ?7 AND quality_bucket = ?8",
                    params![
                        p.tz_version,
                        p.local_day,
                        p.instance_id,
                        p.agent,
                        p.provider_id,
                        p.model_raw,
                        p.call_category,
                        p.quality_bucket,
                        p.event_count,
                        p.call_count,
                        p.input_known_sum,
                        p.cache_read_known_sum,
                        p.cache_write_known_sum,
                        p.output_known_sum,
                        p.total_known_sum,
                        p.conflict_count,
                        p.data_revision
                    ],
                )?;
                outcome.daily_replaced += 1;
            }
            Some(existing_rev) if p.data_revision == existing_rev => {
                // 同修订：覆盖（重复导出导入不缺失；内容相同时 SQL 层面幂等）。
                tx.execute(
                    "UPDATE daily_usage SET
                       event_count = ?9, call_count = ?10,
                       input_known_sum = ?11, cache_read_known_sum = ?12,
                       cache_write_known_sum = ?13, output_known_sum = ?14,
                       total_known_sum = ?15, conflict_count = ?16, data_revision = ?17
                     WHERE tz_version = ?1 AND local_day = ?2 AND instance_id = ?3
                       AND agent = ?4 AND provider_id = ?5 AND model_raw = ?6
                       AND call_category = ?7 AND quality_bucket = ?8",
                    params![
                        p.tz_version,
                        p.local_day,
                        p.instance_id,
                        p.agent,
                        p.provider_id,
                        p.model_raw,
                        p.call_category,
                        p.quality_bucket,
                        p.event_count,
                        p.call_count,
                        p.input_known_sum,
                        p.cache_read_known_sum,
                        p.cache_write_known_sum,
                        p.output_known_sum,
                        p.total_known_sum,
                        p.conflict_count,
                        p.data_revision
                    ],
                )?;
                outcome.daily_replaced += 1;
            }
            Some(_) => {
                tx.execute(
                    "INSERT INTO diagnostics (code, field, position, message, created_ms)
                     VALUES ('import_revision_older', 'data_revision', ?1,
                             'imported partition revision older than existing; kept existing', ?2)",
                    params![p.local_day, now_ms],
                )?;
                outcome.daily_conflicts += 1;
            }
        }
    }
    for h in &export.hourly_partitions {
        let existing_rev: Option<i64> = tx
            .query_row(
                "SELECT data_revision FROM hourly_usage
                 WHERE tz_version = ?1 AND local_day = ?2 AND hour = ?3 AND instance_id = ?4
                   AND agent = ?5 AND provider_id = ?6 AND model_raw = ?7
                   AND call_category = ?8 AND quality_bucket = ?9",
                params![
                    h.tz_version,
                    h.local_day,
                    h.hour,
                    h.instance_id,
                    h.agent,
                    h.provider_id,
                    h.model_raw,
                    h.call_category,
                    h.quality_bucket
                ],
                |r| r.get(0),
            )
            .optional()?;
        match existing_rev {
            None => {
                tx.execute(
                    "INSERT INTO hourly_usage (
                       tz_version, local_day, hour, instance_id, agent, provider_id, model_raw,
                       call_category, quality_bucket, event_count, call_count,
                       input_known_sum, cache_read_known_sum, cache_write_known_sum,
                       output_known_sum, total_known_sum, conflict_count, data_revision
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                               ?12, ?13, ?14, ?15, ?16, 0, ?17)",
                    params![
                        h.tz_version,
                        h.local_day,
                        h.hour,
                        h.instance_id,
                        h.agent,
                        h.provider_id,
                        h.model_raw,
                        h.call_category,
                        h.quality_bucket,
                        h.event_count,
                        h.call_count,
                        h.input_known_sum,
                        h.cache_read_known_sum,
                        h.cache_write_known_sum,
                        h.output_known_sum,
                        h.total_known_sum,
                        h.data_revision
                    ],
                )?;
                outcome.hourly_inserted += 1;
            }
            Some(rev) if h.data_revision > rev => {
                tx.execute(
                    "UPDATE hourly_usage SET
                       event_count = ?10, call_count = ?11, input_known_sum = ?12,
                       cache_read_known_sum = ?13, cache_write_known_sum = ?14,
                       output_known_sum = ?15, total_known_sum = ?16, data_revision = ?17
                     WHERE tz_version = ?1 AND local_day = ?2 AND hour = ?3 AND instance_id = ?4
                       AND agent = ?5 AND provider_id = ?6 AND model_raw = ?7
                       AND call_category = ?8 AND quality_bucket = ?9",
                    params![
                        h.tz_version,
                        h.local_day,
                        h.hour,
                        h.instance_id,
                        h.agent,
                        h.provider_id,
                        h.model_raw,
                        h.call_category,
                        h.quality_bucket,
                        h.event_count,
                        h.call_count,
                        h.input_known_sum,
                        h.cache_read_known_sum,
                        h.cache_write_known_sum,
                        h.output_known_sum,
                        h.total_known_sum,
                        h.data_revision
                    ],
                )?;
                outcome.hourly_replaced += 1;
            }
            Some(_) => outcome.hourly_skipped += 1,
        }
    }
    let revision = Storage::bump_data_revision_tx(&tx, now_ms)?;
    tx.execute(
        "INSERT INTO diagnostics (code, message, created_ms)
         VALUES ('import_completed', ?1, ?2)",
        params![
            format!(
                "aggregate import rev={revision}: sources={} daily ins/repl/skip/conf={}/{}/{}/{} hourly ins/repl/skip={}/{}/{}",
                outcome.sources_registered,
                outcome.daily_inserted,
                outcome.daily_replaced,
                outcome.daily_skipped,
                outcome.daily_conflicts,
                outcome.hourly_inserted,
                outcome.hourly_replaced,
                outcome.hourly_skipped
            ),
            now_ms
        ],
    )?;
    tx.commit()?;
    Ok(outcome)
}

/// 注册导出包的主机（外部来源；本机身份不覆盖）。
fn register_host(tx: &Transaction<'_>, host: &ExchangeHost, now_ms: i64) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO origin_hosts (host_id, is_local, note, first_seen_ms, last_seen_ms)
         VALUES (?1, 0, 'imported exchange package', ?2, ?2)
         ON CONFLICT(host_id) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
        params![host.origin_host_id, now_ms],
    )?;
    if let Some(alias) = &host.hostname_alias {
        tx.execute(
            "INSERT INTO origin_host_names (host_id, hostname, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(host_id, hostname) DO UPDATE SET last_seen_ms = excluded.last_seen_ms",
            params![host.origin_host_id, alias, now_ms],
        )?;
    }
    Ok(())
}
