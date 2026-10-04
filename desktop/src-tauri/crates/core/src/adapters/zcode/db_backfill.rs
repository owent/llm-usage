//! Authoritative local model_usage snapshots. JSONL is a fallback only when
//! this root has never supplied a database. The source prunes after 30 days;
//! snapshots replace complete source/day partitions, never match time or tokens.

use crate::domain::{
    AttributionStatus, CallCategory, EventInput, Lifecycle, ModelAttribution, RecordKind,
    TimeBasis, VersionBasis,
};
use crate::error::CoreError;
use crate::ingest::IngestBatch;
use crate::storage::Storage;
use rusqlite::OptionalExtension;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// 回填结果计数（写入操作日志与 UI）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZcodeDbBackfillOutcome {
    /// db 读到的 model_usage 行数（有 usage 的行）。
    pub db_rows: usize,
    /// 快照未改变的记录数。
    pub matched_existing: usize,
    /// 新插入事件数。
    pub added: usize,
    /// 同键更新（重复回填）事件数。
    pub updated: usize,
}

/// 只读打开 cli/db/db.sqlite 并同步权威快照。`instance_id` 须与 zcode JSONL
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

    db.busy_timeout(std::time::Duration::from_secs(5))?;
    let snapshot = db.unchecked_transaction()?;

    let mut stmt = snapshot.prepare(
        "SELECT id, attempt_index, session_id, provider_id, model_id,
                query_source, completed_at, duration_ms,
                input_tokens, cache_read_input_tokens, cache_creation_input_tokens,
                output_tokens, reasoning_tokens, provider_total_tokens, status
         FROM model_usage
         WHERE completed_at IS NOT NULL
         ORDER BY completed_at, id LIMIT 500001",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ZcodeDbRow {
            native_id: r.get(0)?,
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
        if outcome.db_rows > 500_000 {
            return Err(CoreError::Validation(
                "ZCode archive exceeds the per-scan row budget".into(),
            ));
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
        // 置 None（未知不补零，与 AI SDK 的规则相同）。
        let uncached = row
            .cache_read
            .zip(row.cache_write)
            .and_then(|(r, w)| r.checked_add(w))
            .and_then(|parts| row.input_tokens.and_then(|input| input.checked_sub(parts)))
            .filter(|v| *v >= 0);
        let key = format!("zcodedb:{}", row.native_id);
        batch.events.push(EventInput {
            source_instance_id: instance_id.to_string(),
            source_record_key: key,
            record_kind: RecordKind::ModelCall,
            schema_version: "1".to_string(),
            parser_version: ZCODE_DB_BACKFILL_VERSION.to_string(),
            parse_basis: Some(VersionBasis::LatestFallback),
            origin_call_id: None,
            attempt_id: Some((row.attempt_index + 1).to_string()),
            session_id: row.session_id,
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
            provider_id: row.provider_id,
            model_raw: row.model_id,
            model_canonical: None,
            model_attribution: ModelAttribution::RequestField,
            usage: crate::domain::TokenUsage {
                input_uncached: uncached,
                input_cache_read: row.cache_read,
                input_cache_write: row.cache_write,
                input_total: row.input_tokens,
                output_total: row.output_tokens,
                output_reasoning: row.reasoning_tokens,
                total_tokens: row
                    .input_tokens
                    .zip(row.output_tokens)
                    .and_then(|(input, output)| input.checked_add(output)),
                source_total: row.provider_total,
            },
            quality: crate::domain::TokenQuality {
                input_uncached: quality(uncached, true),
                input_cache_read: quality(row.cache_read, false),
                input_cache_write: quality(row.cache_write, false),
                input_total: quality(row.input_tokens, false),
                output_total: quality(row.output_tokens, false),
                output_reasoning: if row.reasoning_tokens.is_some() {
                    crate::domain::FieldQuality::Reported
                } else {
                    crate::domain::FieldQuality::Unknown
                },
                total_tokens: quality(
                    row.input_tokens
                        .zip(row.output_tokens)
                        .and_then(|(a, b)| a.checked_add(b)),
                    true,
                ),
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

    let result = commit_snapshot(storage, &batch)?;
    outcome.added = result.added as usize;
    outcome.updated = result.updated as usize;
    outcome.matched_existing = result.unchanged as usize;
    Ok(outcome)
}

fn quality(value: Option<i64>, derived: bool) -> crate::domain::FieldQuality {
    use crate::domain::FieldQuality;
    match (value, derived) {
        (None, _) => FieldQuality::Unknown,
        (Some(_), true) => FieldQuality::Derived,
        _ => FieldQuality::Reported,
    }
}

pub fn authority_key(instance: &str) -> String {
    format!("zcode_archive_authority:{instance}")
}

/// Persist a snapshot and its aggregates in one transaction. Old details are
/// used only inside this transaction, then removed under the existing policy.
fn commit_snapshot(
    storage: &Storage,
    batch: &IngestBatch,
) -> Result<crate::ingest::BatchOutcome, CoreError> {
    use crate::calendar::Calendar;
    use std::collections::BTreeMap;
    let calendar = Calendar::new(&batch.timezone)?;
    // Validate the complete snapshot before deleting any existing contribution.
    let mut days = BTreeMap::<jiff::civil::Date, Vec<_>>::new();
    for event in &batch.events {
        event.validate()?;
        days.entry(calendar.local_day_of(event.occurred_at_ms)?)
            .or_default()
            .push(event.clone());
    }
    let tx = storage.conn().unchecked_transaction()?;
    let floor: Option<i64> = tx
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM settings WHERE key='detail_retention_floor_ms'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let daily_floor: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [format!("daily_retention_floor:{}", batch.timezone)],
            |r| r.get(0),
        )
        .optional()?;
    let boundary = calendar.local_day_of(batch.now_ms.saturating_sub(30 * 86_400_000))?;
    let mut replacement = IngestBatch {
        batch_id: batch.batch_id.clone(),
        instance_id: batch.instance_id.clone(),
        timezone: batch.timezone.clone(),
        now_ms: batch.now_ms,
        events: vec![],
        checkpoints: vec![],
        diagnostics: batch.diagnostics.clone(),
        run_id: None,
        retention_cutoff_ms: None,
    };
    let mut unchanged = 0;
    let mut skipped = 0;
    let mut changed_days = Vec::new();
    for (day, events) in days {
        let day_text = day.to_string();
        if daily_floor.as_ref().is_some_and(|d| &day_text < d) {
            skipped += events.len() as i64;
            continue;
        }
        let key = format!(
            "zcode_archive_day:{}:{}:{day}",
            batch.instance_id, batch.timezone
        );
        let digest = crate::identity::content_hash(
            &events
                .iter()
                .map(crate::identity::event_content_hash)
                .collect::<Vec<_>>(),
        );
        let previous: Option<String> = tx
            .query_row("SELECT value FROM settings WHERE key=?1", [&key], |r| {
                r.get(0)
            })
            .optional()?;
        if previous.as_deref() == Some(digest.as_str()) {
            unchanged += events.len() as i64;
            continue;
        }
        let has_daily: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM daily_usage WHERE tz_version=?1 AND local_day=?2 AND instance_id=?3)",
            rusqlite::params![batch.timezone, day_text, batch.instance_id], |r| r.get(0)
        )?;
        // The source may already have removed the beginning of this day.
        // Keep the previously captured complete contribution; never replace it
        // with a partial snapshot or infer missing calls from token amounts.
        if day <= boundary && has_daily {
            skipped += events.len() as i64;
            if previous.as_deref() != Some("partial") {
                tx.execute("INSERT INTO diagnostics(instance_id, code, message, created_ms) VALUES (?1, 'archive_partial_boundary', 'Source retention may have truncated this day; existing totals were preserved.', ?2)",
                    rusqlite::params![batch.instance_id, batch.now_ms])?;
                tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,'partial',1,?2) ON CONFLICT(key) DO UPDATE SET value='partial',updated_at_ms=excluded.updated_at_ms",
                    rusqlite::params![key, batch.now_ms])?;
            }
            continue;
        }
        let (start, end) = calendar.day_range_ms(day)?;
        tx.execute("DELETE FROM usage_events WHERE source_instance_id=?1 AND occurred_at_ms>=?2 AND occurred_at_ms<?3",
            rusqlite::params![batch.instance_id, start, end])?;
        for table in ["daily_usage", "hourly_usage"] {
            tx.execute(
                &format!(
                    "DELETE FROM {table} WHERE instance_id=?1 AND tz_version=?2 AND local_day=?3"
                ),
                rusqlite::params![batch.instance_id, batch.timezone, day_text],
            )?;
        }
        tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,?2,1,?3) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at_ms=excluded.updated_at_ms",
            rusqlite::params![key, digest, batch.now_ms])?;
        replacement.events.extend(events);
        changed_days.push(day);
    }
    if replacement.events.is_empty() {
        replacement.diagnostics.clear();
    }
    let mut result = crate::ingest::commit_batch_tx(storage, &tx, &replacement, None, true)?;
    result.unchanged += unchanged;
    result.skipped += skipped;
    if let Some(floor) = floor {
        for day in changed_days {
            let (_, end) = calendar.day_range_ms(day)?;
            if end <= floor {
                tx.execute("UPDATE daily_usage SET sealed=1, sealed_at_ms=?1, seal_tz=?2, seal_field_version=?3 WHERE instance_id=?4 AND tz_version=?2 AND local_day=?5",
                    rusqlite::params![batch.now_ms,batch.timezone,crate::retention::SEAL_FIELD_VERSION,batch.instance_id,day.to_string()])?;
            }
        }
        tx.execute(
            "DELETE FROM usage_events WHERE source_instance_id=?1 AND occurred_at_ms<?2",
            rusqlite::params![batch.instance_id, floor],
        )?;
    }
    tx.execute("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES (?1,'1',1,?2) ON CONFLICT(key) DO NOTHING",
        rusqlite::params![authority_key(&batch.instance_id),batch.now_ms])?;
    tx.commit()?;
    Ok(result)
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
    native_id: String,
    attempt_index: i64,
    session_id: Option<String>,
    provider_id: Option<String>,
    model_id: Option<String>,
    query_source: Option<String>,
    completed_at: i64,
    duration_ms: Option<i64>,
    input_tokens: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
    output_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    provider_total: Option<i64>,
    status: String,
}

/// 回填实现的解析器版本标识（事件可追溯）。
pub const ZCODE_DB_BACKFILL_VERSION: &str = "zcode-db-snapshot-2";
