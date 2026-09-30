//! F2 价格快照持久化、日成本回填与费用汇总查询。
//!
//! 维护模型（对齐 daily_usage 的封存语义）：
//! - 事件批次提交后按受影响日回填 `daily_cost_usage`（只重写未封存日）；
//! - 明细事件过期删除时，保留层在日成本行上封存（sealed=1），历史金额不再改写；
//! - 价格快照导入不触发既有估算重算；重算仅在数据修订（事件批次）或用户显式
//!   请求（`recompute_unsealed_cost_days`）时进行并更新引用。

use crate::calendar::{parse_date, Calendar};
use crate::error::CoreError;
use crate::pricing::{
    EstimateOptions, EventEstimate, PriceBook, PriceRow, PriceSnapshot, SEED_SNAPSHOT_JSON,
};
use crate::storage::Storage;
use jiff::civil::Date;
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// 费用行 kind：本程序按发生时价的估算。
pub const KIND_ESTIMATE_AT_TIME: &str = "estimate_at_time";
/// 费用行 kind：来源记录金额（EventInput.cost，Reported）。
pub const KIND_SOURCE_REPORTED: &str = "reported";
/// 费用行 kind：来源自己给出的估算金额（EventInput.cost，Estimated）。
pub const KIND_SOURCE_ESTIMATE: &str = "source_estimate";

/// 快照导入结果（A10：同 ID 同内容幂等跳过）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotImportOutcome {
    pub snapshot_id: String,
    pub inserted_rows: usize,
    pub already_present: bool,
}

/// 价格快照摘要（界面展示：来源、年龄、行数）。
#[derive(Debug, Clone, Serialize)]
pub struct PriceSnapshotInfo {
    pub snapshot_id: String,
    pub source_type: String,
    pub source_urls: Vec<String>,
    pub fetched_at_ms: i64,
    pub verified_at_ms: Option<i64>,
    pub license: Option<String>,
    pub verified_by: Option<String>,
    pub note: Option<String>,
    pub row_count: i64,
}

/// 日成本回填结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CostDayOutcome {
    pub day: String,
    pub rebuilt: bool,
    pub estimated_rows: usize,
    pub source_cost_rows: usize,
}

/// 按当前价格模拟的结果：分币种行 + 未计价原因 + 明细受限标记。
type CurrentSimOutcome = (Vec<CostCurrencyRow>, BTreeMap<String, i64>, bool);

impl Storage {
    /// 导入价格快照（同 ID 同内容幂等；同 ID 异内容报错——修正须换快照 ID）。
    pub fn import_price_snapshot(
        &self,
        snapshot: &PriceSnapshot,
        now_ms: i64,
    ) -> Result<SnapshotImportOutcome, CoreError> {
        let tx = self.conn().unchecked_transaction()?;
        let outcome = import_price_snapshot_tx(&tx, snapshot, now_ms)?;
        tx.commit()?;
        Ok(outcome)
    }

    /// 幂等导入仓库种子快照（应用启动时调用；不触发任何估算重算）。
    pub fn ensure_seed_price_snapshot(
        &self,
        now_ms: i64,
    ) -> Result<SnapshotImportOutcome, CoreError> {
        let snapshot = crate::pricing::parse_snapshot_json(SEED_SNAPSHOT_JSON)?;
        self.import_price_snapshot(&snapshot, now_ms)
    }

    /// 载入全部价格行（估算用价格簿）。
    pub fn load_price_book(&self) -> Result<PriceBook, CoreError> {
        load_price_book_conn(self.conn())
    }

    /// 价格快照列表（含行数）。
    pub fn list_price_snapshots(&self) -> Result<Vec<PriceSnapshotInfo>, CoreError> {
        let mut stmt = self.conn().prepare(
            "SELECT s.snapshot_id, s.source_type, s.source_urls, s.fetched_at_ms,
                    s.verified_at_ms, s.license, s.verified_by, s.note,
                    (SELECT COUNT(*) FROM price_versions v WHERE v.snapshot_id = s.snapshot_id)
             FROM price_snapshots s ORDER BY s.fetched_at_ms DESC, s.snapshot_id",
        )?;
        let rows = stmt.query_map([], |r| {
            let urls: String = r.get(2)?;
            Ok(PriceSnapshotInfo {
                snapshot_id: r.get(0)?,
                source_type: r.get(1)?,
                source_urls: serde_json::from_str(&urls).unwrap_or_default(),
                fetched_at_ms: r.get(3)?,
                verified_at_ms: r.get(4)?,
                license: r.get(5)?,
                verified_by: r.get(6)?,
                note: r.get(7)?,
                row_count: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// 采集后费用回填的选日：返回时区下"修订号大于 `revision_before` 的
    /// 未封存日"（本轮事件批次重写过的日；封存日不动）。调用方在刷新起点
    /// 捕获 revision_before——分级保留在扫描后还会 bump 修订号，不能用
    /// "当前修订号相等"过滤。
    pub fn cost_backfill_days_since(
        &self,
        timezone: &str,
        revision_before: i64,
    ) -> Result<Vec<String>, CoreError> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT local_day FROM daily_usage
             WHERE tz_version = ?1 AND sealed = 0 AND data_revision > ?2
             ORDER BY local_day",
        )?;
        let rows = stmt.query_map(rusqlite::params![timezone, revision_before], |r| r.get(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// 回填单日成本（事件批次提交后/显式重算调用；只重写未封存日）。
    /// 该日无明细事件时不重建（历史行可能来自已过期的明细，保持不动）。
    pub fn recompute_cost_day(
        &self,
        timezone: &str,
        day: &str,
        now_ms: i64,
        options: &EstimateOptions,
    ) -> Result<CostDayOutcome, CoreError> {
        let calendar = Calendar::new(timezone)?;
        let day = parse_date(day)?;
        let tx = self.conn().unchecked_transaction()?;
        let revision = crate::storage::data_revision(&tx)?;
        let outcome = recompute_cost_day_tx(&tx, &calendar, day, now_ms, options, revision)?;
        tx.commit()?;
        Ok(outcome)
    }

    /// 显式重算：回填时区下所有"仍有明细事件的未封存日"。
    pub fn recompute_unsealed_cost_days(
        &self,
        timezone: &str,
        now_ms: i64,
        options: &EstimateOptions,
    ) -> Result<Vec<CostDayOutcome>, CoreError> {
        let calendar = Calendar::new(timezone)?;
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT local_day FROM daily_usage
             WHERE tz_version = ?1 AND sealed = 0 ORDER BY local_day",
        )?;
        let days: Vec<String> = stmt
            .query_map([timezone], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        let mut out = Vec::with_capacity(days.len());
        let tx = conn.unchecked_transaction()?;
        let revision = crate::storage::data_revision(&tx)?;
        for day_str in days {
            let day = parse_date(&day_str)?;
            out.push(recompute_cost_day_tx(
                &tx, &calendar, day, now_ms, options, revision,
            )?);
        }
        tx.commit()?;
        Ok(out)
    }

    /// 费用汇总：按发生时价估算与来源金额来自日成本行；按当前价格模拟即时
    /// 计算仍保留的明细事件（明细过期后无法模拟，标 detail_limited）。
    pub fn cost_summary(&self, request: &CostSummaryRequest) -> Result<CostSummary, CoreError> {
        let calendar = Calendar::new(&request.timezone)?;
        let first_day = parse_date(&request.first_day)?;
        let last_day = parse_date(&request.last_day)?;
        if last_day < first_day {
            return Err(CoreError::Validation(
                "cost summary range: last_day before first_day".into(),
            ));
        }
        let (range_start, range_end) = (
            calendar.day_range_ms(first_day)?.0,
            calendar.day_range_ms(last_day)?.1,
        );

        // 1) 日成本行（按发生时价 + 来源金额）。
        let mut filter_sql = String::new();
        let filter_params: Vec<rusqlite::types::Value> = vec![
            request.timezone.clone().into(),
            first_day.to_string().into(),
            last_day.to_string().into(),
        ];
        if !request.filters.agents.is_empty() {
            filter_sql.push_str(&format!(
                " AND {}",
                fold_filter_condition("agent", &request.filters.agents)
            ));
        }
        if !request.filters.providers.is_empty() {
            filter_sql.push_str(&format!(
                " AND {}",
                fold_filter_condition("provider_id", &request.filters.providers)
            ));
        }
        if !request.filters.models.is_empty() {
            filter_sql.push_str(&format!(
                " AND {}",
                fold_filter_condition("model_raw", &request.filters.models)
            ));
        }
        if let Some(instances) = &request.filters.instances {
            let list = instances
                .iter()
                .map(|i| format!("'{}'", i.replace('\'', "")))
                .collect::<Vec<_>>()
                .join(",");
            if list.is_empty() {
                return Ok(CostSummary::empty());
            }
            filter_sql.push_str(&format!(" AND instance_id IN ({list})"));
        }
        let sql = format!(
            "SELECT currency, kind,
                    SUM(input_amount_minor), SUM(cache_read_amount_minor),
                    SUM(cache_write_amount_minor), SUM(output_amount_minor),
                    SUM(total_amount_minor), SUM(priced_tokens), SUM(known_tokens),
                    SUM(priced_event_count), SUM(unpriced_event_count),
                    SUM(partial_event_count), SUM(ttl_defaulted_events)
             FROM daily_cost_usage
             WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3{filter_sql}
             GROUP BY currency, kind"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(filter_params.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, i64>(12)?,
            ))
        })?;
        let mut at_time: BTreeMap<String, CostCurrencyRow> = BTreeMap::new();
        let mut source_amounts: BTreeMap<String, CostCurrencyRow> = BTreeMap::new();
        let mut unpriced_reasons: BTreeMap<String, i64> = BTreeMap::new();
        for row in rows {
            let (
                currency,
                kind,
                input_amount,
                cache_read_amount,
                cache_write_amount,
                output_amount,
                total_amount,
                priced_tokens,
                known_tokens,
                priced_events,
                unpriced_events,
                partial_events,
                ttl_defaulted,
            ) = row?;
            let entry = CostCurrencyRow {
                currency: currency.clone(),
                total_amount_minor: total_amount,
                input_amount_minor: input_amount,
                cache_read_amount_minor: cache_read_amount,
                cache_write_amount_minor: cache_write_amount,
                output_amount_minor: output_amount,
                priced_tokens,
                known_tokens,
                priced_event_count: priced_events,
                unpriced_event_count: unpriced_events,
                partial_event_count: partial_events,
                ttl_defaulted_events: ttl_defaulted,
            };
            match kind.as_str() {
                KIND_ESTIMATE_AT_TIME => {
                    if currency.is_empty() {
                        // currency='' 行只携带未计价计数。
                        if let Some(row) = at_time.get_mut("") {
                            row.unpriced_event_count += unpriced_events;
                        } else {
                            at_time.insert(
                                String::new(),
                                CostCurrencyRow {
                                    currency: String::new(),
                                    total_amount_minor: 0,
                                    input_amount_minor: None,
                                    cache_read_amount_minor: None,
                                    cache_write_amount_minor: None,
                                    output_amount_minor: None,
                                    priced_tokens: 0,
                                    known_tokens: 0,
                                    priced_event_count: 0,
                                    unpriced_event_count: unpriced_events,
                                    partial_event_count: 0,
                                    ttl_defaulted_events: 0,
                                },
                            );
                        }
                    } else {
                        merge_currency_row(&mut at_time, entry);
                    }
                }
                KIND_SOURCE_REPORTED | KIND_SOURCE_ESTIMATE => {
                    merge_currency_row(&mut source_amounts, entry);
                }
                _ => {}
            }
        }
        drop(stmt);

        // 未计价原因直方图（currency='' 行聚合）。
        {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT unpriced_reasons FROM daily_cost_usage
                 WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
                   AND kind = '{KIND_ESTIMATE_AT_TIME}' AND currency = ''{filter_sql}
                   AND unpriced_reasons IS NOT NULL"
            ))?;
            let reasons: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(filter_params.iter()), |r| {
                    r.get(0)
                })?
                .filter_map(Result::ok)
                .collect();
            for json in reasons {
                if let Ok(map) = serde_json::from_str::<BTreeMap<String, i64>>(&json) {
                    for (reason, count) in map {
                        *unpriced_reasons.entry(reason).or_insert(0) += count;
                    }
                }
            }
        }
        // 数据修订引用（该范围成本行的最大修订号）。
        let data_revision: i64 = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT MAX(data_revision) FROM daily_cost_usage
                 WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3{filter_sql}"
            ))?;
            stmt.query_row(rusqlite::params_from_iter(filter_params.iter()), |r| {
                r.get(0)
            })
            .unwrap_or(0)
        };

        // 2) 按当前价格模拟（即时，仅明细仍在的事件）。
        let (current_sim_rows, sim_unpriced, detail_limited) =
            self.cost_summary_current_sim(range_start, range_end, request)?;

        let mut current_sim = CostModeSummary {
            rows: current_sim_rows,
            unpriced_reasons: sim_unpriced,
            ..Default::default()
        };
        current_sim.as_of_ms = request.now_ms;
        current_sim.detail_limited = detail_limited;

        let mut price_basis = BTreeSet::new();
        {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT price_basis FROM daily_cost_usage
                 WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3
                   AND kind = '{KIND_ESTIMATE_AT_TIME}' AND currency != ''{filter_sql}"
            ))?;
            let bases: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(filter_params.iter()), |r| {
                    r.get(0)
                })?
                .filter_map(Result::ok)
                .collect();
            for json in bases {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&json) {
                    price_basis.extend(list);
                }
            }
        }

        Ok(CostSummary {
            at_time: CostModeSummary {
                rows: at_time.into_values().collect(),
                unpriced_reasons,
                ..Default::default()
            },
            source_amounts: source_amounts.into_values().collect(),
            current_sim,
            price_basis: price_basis.into_iter().collect(),
            data_revision,
        })
    }

    /// 按当前价格模拟：遍历区间内仍保留的 model_call 事件即时计价。
    fn cost_summary_current_sim(
        &self,
        range_start: i64,
        range_end: i64,
        request: &CostSummaryRequest,
    ) -> Result<CurrentSimOutcome, CoreError> {
        let book = self.load_price_book()?;
        let rows =
            collect_events_for_pricing(self.conn(), range_start, range_end, &request.filters)?;
        let mut by_currency: BTreeMap<String, CostCurrencyRow> = BTreeMap::new();
        let mut unpriced: BTreeMap<String, i64> = BTreeMap::new();
        let mut unpriced_events_total = 0i64;
        for row in &rows {
            match book.estimate(&row.event, &request.options, request.now_ms) {
                EventEstimate::Priced(amounts) => {
                    let entry = by_currency
                        .entry(amounts.currency.clone())
                        .or_insert_with(|| CostCurrencyRow::empty(amounts.currency.clone()));
                    accumulate_amounts(entry, &amounts);
                }
                EventEstimate::Unpriced(reason) => {
                    unpriced_events_total += 1;
                    *unpriced.entry(reason.as_str().to_string()).or_insert(0) += 1;
                }
            }
        }
        if unpriced_events_total > 0 {
            by_currency.insert(
                String::new(),
                CostCurrencyRow {
                    currency: String::new(),
                    unpriced_event_count: unpriced_events_total,
                    ..CostCurrencyRow::empty(String::new())
                },
            );
        }
        // 明细覆盖检查：区间内有日汇总行但无任何明细事件 ⇒ 模拟受限（历史明细已过期）。
        let has_daily_rows: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM daily_usage WHERE tz_version = ?1 AND local_day >= ?2 AND local_day <= ?3)",
            [&request.timezone, &request.first_day, &request.last_day],
            |r| r.get(0),
        )?;
        let mut probe = self.conn().prepare(
            "SELECT EXISTS(SELECT 1 FROM usage_events WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2)",
        )?;
        let has_events: bool = probe.query_row([range_start, range_end], |r| r.get(0))?;
        let detail_limited = has_daily_rows && !has_events;
        Ok((
            by_currency.into_values().collect(),
            unpriced,
            detail_limited,
        ))
    }
}

fn merge_currency_row(map: &mut BTreeMap<String, CostCurrencyRow>, entry: CostCurrencyRow) {
    let row = map
        .entry(entry.currency.clone())
        .or_insert_with(|| CostCurrencyRow::empty(entry.currency.clone()));
    row.total_amount_minor += entry.total_amount_minor;
    row.input_amount_minor = sum_opt(row.input_amount_minor, entry.input_amount_minor);
    row.cache_read_amount_minor =
        sum_opt(row.cache_read_amount_minor, entry.cache_read_amount_minor);
    row.cache_write_amount_minor =
        sum_opt(row.cache_write_amount_minor, entry.cache_write_amount_minor);
    row.output_amount_minor = sum_opt(row.output_amount_minor, entry.output_amount_minor);
    row.priced_tokens += entry.priced_tokens;
    row.known_tokens += entry.known_tokens;
    row.priced_event_count += entry.priced_event_count;
    row.unpriced_event_count += entry.unpriced_event_count;
    row.partial_event_count += entry.partial_event_count;
    row.ttl_defaulted_events += entry.ttl_defaulted_events;
}

fn sum_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (None, None) => None,
        (Some(v), None) | (None, Some(v)) => Some(v),
        (Some(x), Some(y)) => x.checked_add(y),
    }
}

/// 事务内导入快照（幂等：同 ID 同内容跳过；同 ID 异内容拒绝）。
pub(crate) fn import_price_snapshot_tx(
    tx: &Transaction<'_>,
    snapshot: &PriceSnapshot,
    now_ms: i64,
) -> Result<SnapshotImportOutcome, CoreError> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT content_hash FROM price_snapshots WHERE snapshot_id = ?1",
            [&snapshot.snapshot_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(hash) = existing {
        if hash == format!("{:016x}", snapshot.content_hash) {
            return Ok(SnapshotImportOutcome {
                snapshot_id: snapshot.snapshot_id.clone(),
                inserted_rows: 0,
                already_present: true,
            });
        }
        return Err(CoreError::Validation(format!(
            "price snapshot {} exists with different content; corrections require a new snapshot id",
            snapshot.snapshot_id
        )));
    }
    tx.execute(
        "INSERT INTO price_snapshots (
           snapshot_id, source_type, source_urls, fetched_at_ms, verified_at_ms,
           content_hash, license, verified_by, note, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            snapshot.snapshot_id,
            snapshot.source_type,
            serde_json::to_string(&snapshot.source_urls)
                .map_err(|e| CoreError::Validation(format!("source urls: {e}")))?,
            snapshot.fetched_at_ms,
            snapshot.verified_at_ms,
            snapshot.content_hash_hex(),
            snapshot.license,
            snapshot.verified_by,
            snapshot.note,
            now_ms,
        ],
    )?;
    let mut insert = tx.prepare(
        "INSERT INTO price_versions (
           price_id, snapshot_id, provider_id, model, region, channel, service_tier,
           context_threshold_tokens, effective_from_ms, effective_to_ms,
           input_per_mtok_hundredths, cache_read_per_mtok_hundredths,
           cache_write_5m_per_mtok_hundredths, cache_write_1h_per_mtok_hundredths,
           output_per_mtok_hundredths, cache_storage_per_mtok_hour_hundredths,
           currency, note, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                   ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
    )?;
    for row in &snapshot.rows {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM price_versions WHERE price_id = ?1)",
            [&row.price_id],
            |r| r.get(0),
        )?;
        if exists {
            return Err(CoreError::Validation(format!(
                "price row id {} already exists in another snapshot",
                row.price_id
            )));
        }
        insert.execute(rusqlite::params![
            row.price_id,
            row.snapshot_id,
            row.provider_id,
            row.model,
            row.region,
            row.channel,
            row.service_tier,
            row.context_threshold_tokens,
            row.effective_from_ms,
            row.effective_to_ms,
            row.input_per_mtok_hundredths,
            row.cache_read_per_mtok_hundredths,
            row.cache_write_5m_per_mtok_hundredths,
            row.cache_write_1h_per_mtok_hundredths,
            row.output_per_mtok_hundredths,
            row.cache_storage_per_mtok_hour_hundredths,
            row.currency,
            row.note,
            now_ms,
        ])?;
    }
    Ok(SnapshotImportOutcome {
        snapshot_id: snapshot.snapshot_id.clone(),
        inserted_rows: snapshot.rows.len(),
        already_present: false,
    })
}

/// 维度筛选条件（SQL 片段）：casefold IN 白名单；筛选值含 "unknown" 时
/// 空值/NULL 也命中（与 query::Filters 的 unknown 语义一致），否则不命中。
/// 白名单由构建方（应用命令层）控制，单引号剔除防注入。
fn fold_filter_condition(column: &str, values: &[String]) -> String {
    let folded: Vec<String> = values
        .iter()
        .map(|v| v.to_lowercase().replace('\'', ""))
        .collect();
    let list = folded
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(",");
    let unknown_match = if folded.iter().any(|v| v == "unknown") {
        format!(" OR {column} IS NULL OR {column} = ''")
    } else {
        String::new()
    };
    format!("(fold_name({column}) IN ({list}){unknown_match})")
}

fn load_price_book_conn(conn: &Connection) -> Result<PriceBook, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT price_id, snapshot_id, provider_id, model, region, channel, service_tier,
                context_threshold_tokens, effective_from_ms, effective_to_ms,
                input_per_mtok_hundredths, cache_read_per_mtok_hundredths,
                cache_write_5m_per_mtok_hundredths, cache_write_1h_per_mtok_hundredths,
                output_per_mtok_hundredths, cache_storage_per_mtok_hour_hundredths,
                currency, note
         FROM price_versions",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(PriceRow {
            price_id: r.get(0)?,
            snapshot_id: r.get(1)?,
            provider_id: r.get(2)?,
            model: r.get(3)?,
            region: r.get(4)?,
            channel: r.get(5)?,
            service_tier: r.get(6)?,
            context_threshold_tokens: r.get(7)?,
            effective_from_ms: r.get(8)?,
            effective_to_ms: r.get(9)?,
            input_per_mtok_hundredths: r.get(10)?,
            cache_read_per_mtok_hundredths: r.get(11)?,
            cache_write_5m_per_mtok_hundredths: r.get(12)?,
            cache_write_1h_per_mtok_hundredths: r.get(13)?,
            output_per_mtok_hundredths: r.get(14)?,
            cache_storage_per_mtok_hour_hundredths: r.get(15)?,
            currency: r.get(16)?,
            note: r.get(17)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(PriceBook { rows: out })
}

/// 参与计价的事件（verified model_call，区间内，按筛选）及其维度。
/// quality 非 known（reported/derived）的字段置 None。
pub(crate) struct EventWithDims {
    pub event: crate::pricing::PricingEvent,
    pub instance: String,
    pub agent: String,
    pub provider: String,
    pub model: String,
    /// 来源记录金额（cost_* 列；Reported/Estimated 分别成行）。
    pub source_amount_minor: Option<i64>,
    pub source_currency: Option<String>,
    pub source_kind: Option<String>,
}

pub(crate) fn collect_events_for_pricing(
    conn: &Connection,
    range_start: i64,
    range_end: i64,
    filters: &CostFilters,
) -> Result<Vec<EventWithDims>, CoreError> {
    let mut sql = String::from(
        "SELECT provider_id, model_canonical, model_raw, occurred_at_ms,
                input_uncached, input_cache_read, input_cache_write,
                input_total, output_total, quality_json,
                source_instance_id, agent,
                COALESCE(provider_id, ''), COALESCE(model_raw, ''),
                cost_amount_minor, cost_currency, cost_kind
         FROM usage_events
         WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2
           AND attribution_status = 'verified' AND record_kind = 'model_call'",
    );
    if !filters.agents.is_empty() {
        sql.push_str(&format!(
            " AND {}",
            fold_filter_condition("agent", &filters.agents)
        ));
    }
    if !filters.providers.is_empty() {
        sql.push_str(&format!(
            " AND {}",
            fold_filter_condition("provider_id", &filters.providers)
        ));
    }
    if !filters.models.is_empty() {
        sql.push_str(&format!(
            " AND {}",
            fold_filter_condition("model_raw", &filters.models)
        ));
    }
    if let Some(instances) = &filters.instances {
        if instances.is_empty() {
            return Ok(Vec::new());
        }
        let list: Vec<String> = instances
            .iter()
            .map(|i| format!("'{}'", i.replace('\'', "")))
            .collect();
        sql.push_str(&format!(" AND source_instance_id IN ({})", list.join(",")));
    }
    sql.push_str(" ORDER BY event_id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([range_start, range_end], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<i64>>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, String>(10)?,
            r.get::<_, String>(11)?,
            r.get::<_, String>(12)?,
            r.get::<_, String>(13)?,
            r.get::<_, Option<i64>>(14)?,
            r.get::<_, Option<String>>(15)?,
            r.get::<_, Option<String>>(16)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (
            provider_id,
            model_canonical,
            model_raw,
            occurred_at_ms,
            input_uncached,
            input_cache_read,
            input_cache_write,
            input_total,
            output_total,
            quality_json,
            instance,
            agent,
            provider,
            model,
            source_amount_minor,
            source_currency,
            source_kind,
        ) = row?;
        let quality: Option<serde_json::Value> = serde_json::from_str(&quality_json).ok();
        let known = |field: &str, value: Option<i64>| -> Option<i64> {
            let status = quality
                .as_ref()
                .and_then(|q| q.get(field))
                .and_then(|v| v.as_str());
            match status {
                Some("reported") | Some("derived") => value,
                _ => None,
            }
        };
        out.push(EventWithDims {
            event: crate::pricing::PricingEvent {
                provider_id,
                model_canonical,
                model_raw,
                occurred_at_ms,
                input_uncached: known("input_uncached", input_uncached),
                input_cache_read: known("input_cache_read", input_cache_read),
                input_cache_write: known("input_cache_write", input_cache_write),
                input_total: known("input_total", input_total),
                output_total: known("output_total", output_total),
            },
            instance,
            agent,
            provider,
            model,
            source_amount_minor,
            source_currency,
            source_kind,
        });
    }
    Ok(out)
}

/// 单日回填的事务内实现：删除未封存成本行 → 事件计价 + 来源金额 → 重插。
pub(crate) fn recompute_cost_day_tx(
    tx: &Transaction<'_>,
    calendar: &Calendar,
    day: Date,
    now_ms: i64,
    options: &EstimateOptions,
    data_revision: i64,
) -> Result<CostDayOutcome, CoreError> {
    let day_str = day.to_string();
    let tz = calendar.tz_name();
    let (start_ms, end_ms) = calendar.day_range_ms(day)?;
    let has_events: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM usage_events WHERE occurred_at_ms >= ?1 AND occurred_at_ms < ?2)",
        [start_ms, end_ms],
        |r| r.get(0),
    )?;
    if !has_events {
        return Ok(CostDayOutcome {
            day: day_str,
            rebuilt: false,
            estimated_rows: 0,
            source_cost_rows: 0,
        });
    }
    tx.execute(
        "DELETE FROM daily_cost_usage WHERE tz_version = ?1 AND local_day = ?2 AND sealed = 0",
        [tz, day_str.as_str()],
    )?;

    let book = load_price_book_conn(tx)?;
    // 累计键：instance/agent/provider/model/currency/kind。
    struct Acc {
        instance: String,
        agent: String,
        provider: String,
        model: String,
        currency: String,
        kind: String,
        priced_events: i64,
        unpriced_events: i64,
        partial_events: i64,
        ttl_defaulted: i64,
        input_amount: Option<i64>,
        cache_read_amount: Option<i64>,
        cache_write_amount: Option<i64>,
        output_amount: Option<i64>,
        total_amount: i64,
        priced_tokens: i64,
        known_tokens: i64,
        price_basis: BTreeSet<String>,
    }
    impl Acc {
        fn key_matches(
            &self,
            instance: &str,
            agent: &str,
            provider: &str,
            model: &str,
            currency: &str,
            kind: &str,
        ) -> bool {
            self.instance == instance
                && self.agent == agent
                && self.provider == provider
                && self.model == model
                && self.currency == currency
                && self.kind == kind
        }
        fn blank(
            instance: &str,
            agent: &str,
            provider: &str,
            model: &str,
            currency: &str,
            kind: &str,
        ) -> Acc {
            Acc {
                instance: instance.to_string(),
                agent: agent.to_string(),
                provider: provider.to_string(),
                model: model.to_string(),
                currency: currency.to_string(),
                kind: kind.to_string(),
                priced_events: 0,
                unpriced_events: 0,
                partial_events: 0,
                ttl_defaulted: 0,
                input_amount: None,
                cache_read_amount: None,
                cache_write_amount: None,
                output_amount: None,
                total_amount: 0,
                priced_tokens: 0,
                known_tokens: 0,
                price_basis: BTreeSet::new(),
            }
        }
    }
    fn account_mut<'a>(
        accounts: &'a mut Vec<Acc>,
        instance: &str,
        agent: &str,
        provider: &str,
        model: &str,
        currency: &str,
        kind: &str,
    ) -> &'a mut Acc {
        if let Some(idx) = accounts
            .iter()
            .position(|a| a.key_matches(instance, agent, provider, model, currency, kind))
        {
            return &mut accounts[idx];
        }
        accounts.push(Acc::blank(instance, agent, provider, model, currency, kind));
        accounts.last_mut().expect("just pushed")
    }

    let mut accounts: Vec<Acc> = Vec::new();
    let mut unpriced_reasons: BTreeMap<String, i64> = BTreeMap::new();

    let events = collect_events_for_pricing(tx, start_ms, end_ms, &CostFilters::default())?;
    for row in &events {
        // 1) 按发生时价估算。
        match book.estimate_at_time(&row.event, options) {
            EventEstimate::Priced(amounts) => {
                let partial =
                    amounts.priced_tokens < amounts.known_tokens || amounts.has_unknown_components;
                let acc = account_mut(
                    &mut accounts,
                    &row.instance,
                    &row.agent,
                    &row.provider,
                    &row.model,
                    &amounts.currency,
                    KIND_ESTIMATE_AT_TIME,
                );
                acc.priced_events += 1;
                if partial {
                    acc.partial_events += 1;
                }
                if amounts.ttl_defaulted {
                    acc.ttl_defaulted += 1;
                }
                acc.input_amount = sum_opt(acc.input_amount, amounts.input_amount_minor);
                acc.cache_read_amount =
                    sum_opt(acc.cache_read_amount, amounts.cache_read_amount_minor);
                acc.cache_write_amount =
                    sum_opt(acc.cache_write_amount, amounts.cache_write_amount_minor);
                acc.output_amount = sum_opt(acc.output_amount, amounts.output_amount_minor);
                acc.total_amount = acc
                    .total_amount
                    .checked_add(amounts.total_amount_minor)
                    .ok_or(CoreError::Overflow("daily cost total"))?;
                acc.priced_tokens = acc
                    .priced_tokens
                    .checked_add(amounts.priced_tokens)
                    .ok_or(CoreError::Overflow("daily cost priced tokens"))?;
                acc.known_tokens = acc
                    .known_tokens
                    .checked_add(amounts.known_tokens)
                    .ok_or(CoreError::Overflow("daily cost known tokens"))?;
                for id in &amounts.matched_price_ids {
                    if let Some(price_row) = book.rows.iter().find(|r| &r.price_id == id) {
                        acc.price_basis.insert(price_row.snapshot_id.clone());
                    }
                }
            }
            EventEstimate::Unpriced(reason) => {
                *unpriced_reasons
                    .entry(reason.as_str().to_string())
                    .or_insert(0) += 1;
            }
        }
        // 2) 来源记录金额（reported / 来源自身估算，分列）。
        if let (Some(amount), Some(currency), Some(kind_str)) = (
            row.source_amount_minor,
            row.source_currency.clone(),
            row.source_kind.clone(),
        ) {
            let kind = match kind_str.as_str() {
                "reported" => KIND_SOURCE_REPORTED,
                "estimated" => KIND_SOURCE_ESTIMATE,
                _ => continue,
            };
            let acc = account_mut(
                &mut accounts,
                &row.instance,
                &row.agent,
                &row.provider,
                &row.model,
                &currency,
                kind,
            );
            acc.priced_events += 1;
            acc.total_amount = acc
                .total_amount
                .checked_add(amount)
                .ok_or(CoreError::Overflow("daily source cost"))?;
        }
    }

    let mut estimated_rows = 0usize;
    let mut source_cost_rows = 0usize;
    {
        let mut insert = tx.prepare(
            "INSERT INTO daily_cost_usage (
               tz_version, local_day, instance_id, agent, provider_id, model_raw,
               currency, kind, priced_event_count, unpriced_event_count,
               partial_event_count, ttl_defaulted_events,
               input_amount_minor, cache_read_amount_minor, cache_write_amount_minor,
               output_amount_minor, total_amount_minor, priced_tokens, known_tokens,
               unpriced_reasons, price_basis, sealed, data_revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, 0, ?22)",
        )?;
        for acc in &accounts {
            let is_estimate = acc.kind == KIND_ESTIMATE_AT_TIME;
            let reasons_json = if is_estimate && acc.currency.is_empty() {
                serde_json::to_string(&unpriced_reasons).ok()
            } else {
                None
            };
            insert.execute(rusqlite::params![
                tz,
                day_str,
                acc.instance,
                acc.agent,
                acc.provider,
                acc.model,
                acc.currency,
                acc.kind,
                acc.priced_events,
                acc.unpriced_events,
                acc.partial_events,
                acc.ttl_defaulted,
                acc.input_amount,
                acc.cache_read_amount,
                acc.cache_write_amount,
                acc.output_amount,
                acc.total_amount,
                acc.priced_tokens,
                acc.known_tokens,
                reasons_json,
                serde_json::to_string(&acc.price_basis.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_else(|_| "[]".to_string()),
                data_revision,
            ])?;
            if is_estimate {
                estimated_rows += 1;
            } else {
                source_cost_rows += 1;
            }
        }
        // 未计价事件计数行（currency=''，仅估算 kind）。
        if !unpriced_reasons.is_empty() {
            let total_unpriced: i64 = unpriced_reasons.values().sum();
            insert.execute(rusqlite::params![
                tz,
                day_str,
                "legacy_unknown",
                "",
                "",
                "",
                "",
                KIND_ESTIMATE_AT_TIME,
                0,
                total_unpriced,
                0,
                0,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                0,
                0,
                0,
                serde_json::to_string(&unpriced_reasons).unwrap_or_else(|_| "{}".to_string()),
                "[]",
                data_revision,
            ])?;
            estimated_rows += 1;
        }
    }
    let _ = now_ms;
    Ok(CostDayOutcome {
        day: day_str,
        rebuilt: true,
        estimated_rows,
        source_cost_rows,
    })
}

/// 费用汇总筛选（与 query::Filters 同语义：不同字段 AND、同字段 OR、
/// "unknown" 匹配空值；instances Some([]) = 无可用来源）。
#[derive(Debug, Clone, Default)]
pub struct CostFilters {
    pub agents: Vec<String>,
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub instances: Option<Vec<String>>,
}

/// 费用汇总请求。
#[derive(Debug, Clone)]
pub struct CostSummaryRequest {
    pub timezone: String,
    /// 本地日 YYYY-MM-DD（含端点）。
    pub first_day: String,
    pub last_day: String,
    pub filters: CostFilters,
    pub now_ms: i64,
    pub options: EstimateOptions,
}

/// 按币种分列的金额行（不同币种不合并；E8）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostCurrencyRow {
    pub currency: String,
    pub total_amount_minor: i64,
    pub input_amount_minor: Option<i64>,
    pub cache_read_amount_minor: Option<i64>,
    pub cache_write_amount_minor: Option<i64>,
    pub output_amount_minor: Option<i64>,
    pub priced_tokens: i64,
    pub known_tokens: i64,
    pub priced_event_count: i64,
    pub unpriced_event_count: i64,
    pub partial_event_count: i64,
    pub ttl_defaulted_events: i64,
}

impl CostCurrencyRow {
    fn empty(currency: String) -> Self {
        CostCurrencyRow {
            currency,
            ..Default::default()
        }
    }
}

fn accumulate_amounts(row: &mut CostCurrencyRow, amounts: &crate::pricing::EventEstimateAmounts) {
    row.total_amount_minor = row
        .total_amount_minor
        .saturating_add(amounts.total_amount_minor);
    row.input_amount_minor = sum_opt(row.input_amount_minor, amounts.input_amount_minor);
    row.cache_read_amount_minor =
        sum_opt(row.cache_read_amount_minor, amounts.cache_read_amount_minor);
    row.cache_write_amount_minor = sum_opt(
        row.cache_write_amount_minor,
        amounts.cache_write_amount_minor,
    );
    row.output_amount_minor = sum_opt(row.output_amount_minor, amounts.output_amount_minor);
    row.priced_tokens = row.priced_tokens.saturating_add(amounts.priced_tokens);
    row.known_tokens = row.known_tokens.saturating_add(amounts.known_tokens);
    row.priced_event_count += 1;
    if amounts.priced_tokens < amounts.known_tokens || amounts.has_unknown_components {
        row.partial_event_count += 1;
    }
    if amounts.ttl_defaulted {
        row.ttl_defaulted_events += 1;
    }
}

/// 单一估算模式的汇总（按币种行 + 未计价原因）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostModeSummary {
    pub rows: Vec<CostCurrencyRow>,
    /// 未计价原因直方图（reason → 事件数）。
    pub unpriced_reasons: BTreeMap<String, i64>,
    /// 模拟模式的评估时点/明细受限标记（at_time 不用）。
    pub as_of_ms: i64,
    pub detail_limited: bool,
}

/// 费用汇总响应（估算引用 data_revision 与 price_basis 快照集合）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostSummary {
    /// 按发生时价估算（持久化日成本行）。
    pub at_time: CostModeSummary,
    /// 来源记录金额（reported + 来源自身估算，分列币种）。
    pub source_amounts: Vec<CostCurrencyRow>,
    /// 按当前价格模拟（即时计算，明细保留范围内）。
    pub current_sim: CostModeSummary,
    /// at_time 引用的价格快照。
    pub price_basis: Vec<String>,
    pub data_revision: i64,
}

impl CostSummary {
    fn empty() -> Self {
        CostSummary::default()
    }
}

/// 封存日成本行（明细层保留截止时调用；历史金额不再改写）。
pub(crate) fn seal_cost_days_tx(
    tx: &Transaction<'_>,
    tz: &str,
    cutoff_day: &str,
) -> Result<usize, CoreError> {
    Ok(tx.execute(
        "UPDATE daily_cost_usage SET sealed = 1
         WHERE tz_version = ?1 AND local_day < ?2 AND sealed = 0",
        [tz, cutoff_day],
    )?)
}

/// 日层清理时同步清理日成本行（随日层保留期；与 daily_usage 同 cutoff）。
pub(crate) fn prune_cost_days_tx(
    tx: &Transaction<'_>,
    tz: &str,
    cutoff_day: &str,
) -> Result<usize, CoreError> {
    Ok(tx.execute(
        "DELETE FROM daily_cost_usage WHERE tz_version = ?1 AND local_day < ?2",
        [tz, cutoff_day],
    )?)
}
