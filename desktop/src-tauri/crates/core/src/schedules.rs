//! 逐源提取计划（extraction_schedules/schedule_state 接线，M6 余项）。
//!
//! 约定（scheduling.md）：
//! - 逐源频率覆盖全局：固定间隔 15 秒–24 小时，或每日/每周指定时间
//!   （time_of_day "HH:MM"、weekday ISO 1–7 周一=1）；首版不开放任意 cron/shell；
//! - 启用的自定义计划覆盖全局自动节奏；手动刷新仍包含全部启用来源；
//! - 同源不并发（应用层 refresh 单飞合并）；禁用后无自动读取；
//! - 错过时点（休眠/关机）醒来后只补扫一次（next_due 在运行后推进）；
//! - 时区固定保存；DST 重复时刻取第一次，缺失时刻取跳变后的首个有效时刻。

use crate::error::CoreError;
use crate::storage::Storage;
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeSet;

/// 逐源间隔允许范围（秒）。
pub const MIN_INTERVAL_SECS: i64 = 15;
pub const MAX_INTERVAL_SECS: i64 = 24 * 60 * 60;

/// 一条逐源计划规则（scope 恒为 "source"，schedule_id = "source:<instance_id>"）。
#[derive(Debug, Clone, PartialEq)]
pub struct SourceScheduleRule {
    pub instance_id: String,
    /// "interval" | "daily" | "weekly"。
    pub rule_kind: String,
    /// interval 专用：15..=86400 秒。
    pub interval_seconds: Option<i64>,
    /// daily/weekly 专用："HH:MM"（24h 制，本地时区语义）。
    pub time_of_day: Option<String>,
    /// weekly 专用：ISO 1..=7（周一=1，周日=7）。
    pub weekday: Option<i64>,
    /// 计划时区（IANA 名；空串在首次保存时固定为用户统计时区）。
    pub tz: String,
    pub enabled: bool,
}

fn parse_hhmm(value: Option<&str>) -> Option<(u8, u8)> {
    let raw = value?.trim();
    let (h, m) = raw.split_once(':')?;
    if h.len() != 2 || m.len() != 2 || !h.bytes().chain(m.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let h: u8 = h.parse().ok()?;
    let m: u8 = m.parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

/// 规则校验（拒绝任意 cron/越界）。
pub fn validate_rule(rule: &SourceScheduleRule) -> Result<(), CoreError> {
    let bad = |msg: &str| Err(CoreError::Validation(msg.to_string()));
    if rule.instance_id.trim().is_empty() {
        return bad("instance_id must not be empty");
    }
    match rule.rule_kind.as_str() {
        "interval" => {
            let secs = rule.interval_seconds.ok_or_else(|| {
                CoreError::Validation("interval rule needs interval_seconds".into())
            })?;
            if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&secs) {
                return bad("interval_seconds out of the 15s..=24h range");
            }
        }
        "daily" => {
            if parse_hhmm(rule.time_of_day.as_deref()).is_none() {
                return bad("daily rule needs time_of_day as HH:MM");
            }
        }
        "weekly" => {
            if parse_hhmm(rule.time_of_day.as_deref()).is_none() {
                return bad("weekly rule needs time_of_day as HH:MM");
            }
            let weekday = rule
                .weekday
                .ok_or_else(|| CoreError::Validation("weekly rule needs weekday".into()))?;
            if !(1..=7).contains(&weekday) {
                return bad("weekday must be ISO 1..=7 (Mon=1)");
            }
        }
        other => return bad(&format!("unknown rule_kind {other:?}")),
    }
    // tz 非空时必须可解析为 IANA 时区：否则 next_due_ms 静默返回 None，
    // 计划永不触发且无任何报错（fail-closed 到 Validation，而非静默失效）。
    let tz = rule.tz.trim();
    if !tz.is_empty() && jiff::tz::TimeZone::get(tz).is_err() {
        return bad(&format!("unknown IANA time zone {tz:?}"));
    }
    Ok(())
}

/// 计算下一次到期毫秒（纯函数）。
/// None 只在：规则未启用、tz 名无法解析（validate_rule 已拦截非空 tz；
/// 空 tz 继承的 fallback_tz 由设置层校验）、或 now 越出时间戳范围。
/// daily/weekly 的重复时刻只取第一次；缺失时刻顺延到跳变后的首个有效时刻。
pub fn next_due_ms(rule: &SourceScheduleRule, now_ms: i64, fallback_tz: &str) -> Option<i64> {
    if !rule.enabled {
        return None;
    }
    let tz_name = if rule.tz.trim().is_empty() {
        fallback_tz
    } else {
        rule.tz.as_str()
    };
    let tz = jiff::tz::TimeZone::get(tz_name).ok()?;
    let now = jiff::Timestamp::from_millisecond(now_ms).ok()?;
    let fallback = now
        .checked_add(jiff::Span::new().hours(24))
        .ok()?
        .as_millisecond();
    let now_zoned = now.to_zoned(tz.clone());
    match rule.rule_kind.as_str() {
        "interval" => {
            let secs = rule.interval_seconds?;
            now.checked_add(jiff::Span::new().seconds(secs))
                .ok()
                .map(|t| t.as_millisecond())
        }
        "daily" | "weekly" => {
            let (hour, minute) = parse_hhmm(rule.time_of_day.as_deref())?;
            let today = now_zoned.date();
            let mut candidate_date = today;
            for _ in 0..8 {
                let matches_day = if rule.rule_kind == "daily" {
                    true
                } else {
                    let weekday = candidate_date.weekday();
                    let iso = weekday.to_monday_zero_offset() as i64 + 1;
                    Some(iso) == rule.weekday
                };
                let ambiguous =
                    tz.to_ambiguous_timestamp(candidate_date.at(hour as i8, minute as i8, 0, 0));
                let candidate = match ambiguous.offset() {
                    jiff::tz::AmbiguousOffset::Gap { .. } => {
                        let before = ambiguous.earlier().ok()?;
                        tz.following(before)
                            .next()
                            .map(|transition| transition.timestamp())
                    }
                    _ => ambiguous.earlier().ok(),
                };
                if let (true, Some(timestamp)) = (matches_day, candidate) {
                    if timestamp > now {
                        return Some(timestamp.as_millisecond());
                    }
                }
                candidate_date = candidate_date.tomorrow().ok()?;
            }
            // 八天内找不到（理论不可达）：保守回退。
            Some(fallback)
        }
        _ => None,
    }
}

/// Preview the next three planned instants; overdue execution is still merged by
/// due_instances rather than replaying every missed occurrence.
pub fn preview_due_ms(rule: &SourceScheduleRule, now_ms: i64, fallback_tz: &str) -> Vec<i64> {
    let mut result = Vec::new();
    let mut cursor = now_ms;
    for _ in 0..3 {
        let Some(next) = next_due_ms(rule, cursor, fallback_tz) else {
            break;
        };
        result.push(next);
        cursor = next;
    }
    result
}

/// Old rules inherited the statistics timezone dynamically. Pin their current
/// effective timezone once, before users can change statistics settings again.
pub fn pin_legacy_timezones(storage: &Storage, timezone: &str) -> Result<(), CoreError> {
    jiff::tz::TimeZone::get(timezone).map_err(|e| CoreError::Validation(e.to_string()))?;
    storage.conn().execute(
        "UPDATE extraction_schedules SET tz=?1 WHERE scope='source' AND trim(tz)=''",
        [timezone],
    )?;
    Ok(())
}

/// upsert 一条逐源计划（含 next_due 重算；config_version 递增）。
pub fn upsert_source_schedule(
    storage: &Storage,
    rule: &SourceScheduleRule,
    now_ms: i64,
    fallback_tz: &str,
) -> Result<(), CoreError> {
    // Persist the initial fallback instead of following later statistics changes.
    let mut rule = rule.clone();
    rule.tz = if rule.tz.trim().is_empty() {
        fallback_tz.trim()
    } else {
        rule.tz.trim()
    }
    .to_string();
    validate_rule(&rule)?;
    let schedule_id = format!("source:{}", rule.instance_id);
    let due = next_due_ms(&rule, now_ms, fallback_tz);
    let existing: Option<i64> = storage
        .conn()
        .query_row(
            "SELECT config_version FROM extraction_schedules WHERE schedule_id = ?1",
            [&schedule_id],
            |r| r.get(0),
        )
        .optional()?;
    let config_version = existing.map(|v| v + 1).unwrap_or(1);
    storage.conn().execute(
        "INSERT INTO extraction_schedules (
           schedule_id, scope, instance_id, rule_kind, interval_seconds,
           time_of_day, weekday, tz, enabled, file_trigger_enabled,
           config_version, next_due_at_ms, created_at_ms, updated_at_ms
         ) VALUES (?1,'source',?2,?3,?4,?5,?6,?7,?8,0,?9,?10,?11,?11)
         ON CONFLICT(schedule_id) DO UPDATE SET
           rule_kind = excluded.rule_kind,
           interval_seconds = excluded.interval_seconds,
           time_of_day = excluded.time_of_day,
           weekday = excluded.weekday,
           tz = excluded.tz,
           enabled = excluded.enabled,
           config_version = excluded.config_version,
           next_due_at_ms = excluded.next_due_at_ms,
           updated_at_ms = excluded.updated_at_ms",
        params![
            schedule_id,
            rule.instance_id,
            rule.rule_kind,
            rule.interval_seconds,
            rule.time_of_day,
            rule.weekday,
            rule.tz,
            rule.enabled as i64,
            config_version,
            due,
            now_ms
        ],
    )?;
    Ok(())
}

/// 删除逐源计划（恢复继承全局）。
pub fn delete_source_schedule(storage: &Storage, instance_id: &str) -> Result<(), CoreError> {
    let schedule_id = format!("source:{instance_id}");
    storage.conn().execute(
        "DELETE FROM schedule_state WHERE schedule_id = ?1",
        [&schedule_id],
    )?;
    storage.conn().execute(
        "DELETE FROM extraction_schedules WHERE schedule_id = ?1",
        [&schedule_id],
    )?;
    Ok(())
}

/// 读取一条逐源计划（UI 回显）。
pub fn source_schedule(
    storage: &Storage,
    instance_id: &str,
) -> Result<Option<SourceScheduleRule>, CoreError> {
    type RuleRow = (
        String,
        Option<i64>,
        Option<String>,
        Option<i64>,
        String,
        bool,
    );
    let row: Option<RuleRow> = storage
        .conn()
        .query_row(
            "SELECT rule_kind, interval_seconds, time_of_day, weekday, tz, enabled
             FROM extraction_schedules WHERE schedule_id = ?1",
            [format!("source:{instance_id}")],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get::<_, i64>(5)? != 0,
                ))
            },
        )
        .optional()?;
    Ok(row.map(
        |(kind, interval, tod, weekday, tz, enabled)| SourceScheduleRule {
            instance_id: instance_id.to_string(),
            rule_kind: kind,
            interval_seconds: interval,
            time_of_day: tod,
            weekday,
            tz,
            enabled,
        },
    ))
}

/// 到期（计划启用且来源实例也启用）的实例集合：next_due_at_ms <= now。
/// 联 source_instances.enabled：停用来源不被计划触发——否则到期实例每轮
/// 被采集层跳过又按"无报告=失败"推进 next_due，error_summary 留误导性
/// 失败记录（"禁用后无自动读取"约定）。INNER JOIN：无实例行的悬空计划
/// 无从扫描，一并排除。
pub fn due_instances(storage: &Storage, now_ms: i64) -> Result<BTreeSet<String>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT e.instance_id FROM extraction_schedules e
         JOIN source_instances s ON s.instance_id = e.instance_id
         WHERE e.scope='source' AND e.enabled=1 AND s.enabled=1
           AND e.next_due_at_ms IS NOT NULL
           AND e.next_due_at_ms <= ?1",
    )?;
    let rows = stmt.query_map([now_ms], |r| r.get::<_, String>(0))?;
    let mut out = BTreeSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}

/// 有自定义启用计划的实例集合（全局刷新时排除）。
pub fn custom_scheduled_instances(storage: &Storage) -> Result<BTreeSet<String>, CoreError> {
    let mut stmt = storage.conn().prepare(
        "SELECT instance_id FROM extraction_schedules
         WHERE scope='source' AND enabled=1",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = BTreeSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}

/// 运行后推进：next_due = 以当前时刻重算；schedule_state 记录成败。
pub fn mark_source_run(
    storage: &Storage,
    instance_id: &str,
    now_ms: i64,
    success: bool,
    fallback_tz: &str,
    stats_json: Option<&str>,
) -> Result<(), CoreError> {
    let schedule_id = format!("source:{instance_id}");
    let Some(rule) = source_schedule(storage, instance_id)? else {
        return Ok(());
    };
    let due = next_due_ms(&rule, now_ms, fallback_tz);
    storage.conn().execute(
        "UPDATE extraction_schedules SET next_due_at_ms = ?2, updated_at_ms = ?3
         WHERE schedule_id = ?1",
        params![schedule_id, due, now_ms],
    )?;
    storage.conn().execute(
        "INSERT INTO schedule_state (
           schedule_id, desired_state, applied_state, last_started_ms,
           last_success_ms, last_duration_ms, last_run_stats, error_summary,
           next_run_ms, updated_ms
         ) VALUES (?1,'enabled',?2,?3,?4,NULL,?5,?6,?7,?3)
         ON CONFLICT(schedule_id) DO UPDATE SET
           applied_state = excluded.applied_state,
           last_started_ms = excluded.last_started_ms,
           last_success_ms = COALESCE(excluded.last_success_ms, schedule_state.last_success_ms),
           last_run_stats = excluded.last_run_stats,
           error_summary = excluded.error_summary,
           next_run_ms = excluded.next_run_ms,
           updated_ms = excluded.updated_ms",
        params![
            schedule_id,
            if success {
                "running_ok"
            } else {
                "running_error"
            },
            now_ms,
            success.then_some(now_ms),
            stats_json,
            (!success).then_some("per-source run failed"),
            due,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daily(tz: &str, time: &str) -> SourceScheduleRule {
        SourceScheduleRule {
            instance_id: "test".into(),
            rule_kind: "daily".into(),
            interval_seconds: None,
            time_of_day: Some(time.into()),
            weekday: None,
            tz: tz.into(),
            enabled: true,
        }
    }
    fn ms(time: &str) -> i64 {
        time.parse::<jiff::Timestamp>().unwrap().as_millisecond()
    }

    #[test]
    fn dst_gap_runs_at_first_valid_instant_and_fold_only_once() {
        let rule = daily("America/New_York", "02:30");
        assert_eq!(
            next_due_ms(&rule, ms("2026-03-08T06:00:00Z"), "UTC"),
            Some(ms("2026-03-08T07:00:00Z"))
        );
        let rule = daily("America/New_York", "01:30");
        assert_eq!(
            next_due_ms(&rule, ms("2026-11-01T04:00:00Z"), "UTC"),
            Some(ms("2026-11-01T05:30:00Z"))
        );
        assert_eq!(
            next_due_ms(&rule, ms("2026-11-01T05:31:00Z"), "UTC"),
            Some(ms("2026-11-02T06:30:00Z"))
        );
        // Lord Howe advances only 30 minutes; adding a fixed hour is incorrect.
        let rule = daily("Australia/Lord_Howe", "02:15");
        assert_eq!(
            next_due_ms(&rule, ms("2026-10-03T14:00:00Z"), "UTC"),
            Some(ms("2026-10-03T15:30:00Z"))
        );
    }

    #[test]
    fn timezone_is_persisted_and_preview_follows_calendar_days() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../build/plan-completion");
        std::fs::create_dir_all(&root).unwrap();
        let storage =
            Storage::open(&root.join(format!("schedule-timezone-{}.sqlite", std::process::id())))
                .unwrap();
        let now = ms("2026-10-03T00:00:00Z");
        upsert_source_schedule(&storage, &daily("", "09:00"), now, "Asia/Shanghai").unwrap();
        let rule = source_schedule(&storage, "test").unwrap().unwrap();
        assert_eq!(rule.tz, "Asia/Shanghai");
        assert_eq!(
            preview_due_ms(&rule, now, "America/New_York"),
            vec![
                ms("2026-10-03T01:00:00Z"),
                ms("2026-10-04T01:00:00Z"),
                ms("2026-10-05T01:00:00Z")
            ]
        );
        storage
            .conn()
            .execute("UPDATE extraction_schedules SET tz=''", [])
            .unwrap();
        pin_legacy_timezones(&storage, "UTC").unwrap();
        pin_legacy_timezones(&storage, "Asia/Shanghai").unwrap();
        assert_eq!(
            source_schedule(&storage, "test").unwrap().unwrap().tz,
            "UTC"
        );
    }

    #[test]
    fn rule_validation_bounds() {
        assert!(validate_rule(&daily("UTC", "9:5")).is_err());
        assert!(validate_rule(&daily("UTC", "+9:05")).is_err());
        assert!(validate_rule(&SourceScheduleRule {
            instance_id: "a@b".into(),
            rule_kind: "interval".into(),
            interval_seconds: Some(15),
            time_of_day: None,
            weekday: None,
            tz: String::new(),
            enabled: true,
        })
        .is_ok());
        assert!(validate_rule(&SourceScheduleRule {
            instance_id: "a@b".into(),
            rule_kind: "interval".into(),
            interval_seconds: Some(14),
            time_of_day: None,
            weekday: None,
            tz: String::new(),
            enabled: true,
        })
        .is_err());
        assert!(validate_rule(&SourceScheduleRule {
            instance_id: "a@b".into(),
            rule_kind: "weekly".into(),
            interval_seconds: None,
            time_of_day: Some("09:30".into()),
            weekday: Some(8),
            tz: String::new(),
            enabled: true,
        })
        .is_err());
        assert!(validate_rule(&SourceScheduleRule {
            instance_id: "a@b".into(),
            rule_kind: "cron".into(),
            interval_seconds: None,
            time_of_day: None,
            weekday: None,
            tz: String::new(),
            enabled: true,
        })
        .is_err());
    }

    #[test]
    fn next_due_interval_and_daily() {
        let now = jiff::Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let interval = SourceScheduleRule {
            instance_id: "x".into(),
            rule_kind: "interval".into(),
            interval_seconds: Some(3600),
            time_of_day: None,
            weekday: None,
            tz: String::new(),
            enabled: true,
        };
        assert_eq!(
            next_due_ms(&interval, now.as_millisecond(), "UTC"),
            Some(now.as_millisecond() + 3_600_000)
        );
        // daily 09:00 UTC：now=2027-01-15T02:13:20Z ⇒ 当日 09:00。
        let daily = SourceScheduleRule {
            instance_id: "x".into(),
            rule_kind: "daily".into(),
            interval_seconds: None,
            time_of_day: Some("09:00".into()),
            weekday: None,
            tz: "UTC".into(),
            enabled: true,
        };
        let due = next_due_ms(&daily, now.as_millisecond(), "UTC").unwrap();
        let due_zoned = jiff::Timestamp::from_millisecond(due)
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::UTC);
        assert_eq!(due_zoned.hour(), 9);
        assert_eq!(due_zoned.date(), jiff::civil::date(2027, 1, 15));
    }

    #[test]
    fn next_due_weekly_skips_to_weekday() {
        // 2027-01-15 是周五（ISO 5）。周一 09:00 的 weekly ⇒ 下周一 2027-01-18。
        let now = jiff::Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        assert_eq!(
            now.to_zoned(jiff::tz::TimeZone::UTC)
                .date()
                .weekday()
                .to_monday_zero_offset(),
            4
        );
        let weekly = SourceScheduleRule {
            instance_id: "x".into(),
            rule_kind: "weekly".into(),
            interval_seconds: None,
            time_of_day: Some("09:00".into()),
            weekday: Some(1),
            tz: "UTC".into(),
            enabled: true,
        };
        let due = next_due_ms(&weekly, now.as_millisecond(), "UTC").unwrap();
        let due_zoned = jiff::Timestamp::from_millisecond(due)
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::UTC);
        assert_eq!(due_zoned.date(), jiff::civil::date(2027, 1, 18));
        assert_eq!(due_zoned.hour(), 9);
    }

    #[test]
    fn disabled_rule_never_due() {
        let rule = SourceScheduleRule {
            instance_id: "x".into(),
            rule_kind: "interval".into(),
            interval_seconds: Some(60),
            time_of_day: None,
            weekday: None,
            tz: String::new(),
            enabled: false,
        };
        assert_eq!(next_due_ms(&rule, 1_800_000_000_000, "UTC"), None);
    }

    #[test]
    fn store_roundtrip_due_and_advance() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-sched-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Storage::open(&dir.join("s.sqlite")).unwrap();
        let now = 1_800_000_000_000i64;
        // due_instances 联 source_instances.enabled：先注册实例行（采集层 upsert）。
        storage
            .conn()
            .execute(
                "INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,health,created_at_ms,updated_at_ms) VALUES ('inst-1','codex','local_filesystem','verified',1,'ok',?1,?1)",
                [now],
            )
            .unwrap();
        upsert_source_schedule(
            &storage,
            &SourceScheduleRule {
                instance_id: "inst-1".into(),
                rule_kind: "interval".into(),
                interval_seconds: Some(60),
                time_of_day: None,
                weekday: None,
                tz: String::new(),
                enabled: true,
            },
            now,
            "UTC",
        )
        .unwrap();
        // 未到期。
        assert!(due_instances(&storage, now + 30_000).unwrap().is_empty());
        // 到期。
        assert_eq!(
            due_instances(&storage, now + 61_000).unwrap(),
            BTreeSet::from(["inst-1".to_string()])
        );
        // 推进后不再到期；custom 集合包含。
        mark_source_run(&storage, "inst-1", now + 61_000, true, "UTC", None).unwrap();
        assert!(due_instances(&storage, now + 61_000).unwrap().is_empty());
        assert!(custom_scheduled_instances(&storage)
            .unwrap()
            .contains("inst-1"));
        // 删除恢复继承。
        delete_source_schedule(&storage, "inst-1").unwrap();
        assert!(custom_scheduled_instances(&storage).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_instance_not_due_and_reenable_recovers() {
        let dir = std::env::temp_dir().join(format!(
            "llm-usage-sched-dis-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Storage::open(&dir.join("s.sqlite")).unwrap();
        let now = 1_800_000_000_000i64;
        storage
            .conn()
            .execute(
                "INSERT INTO source_instances(instance_id,agent,locality_basis,attribution_status,enabled,health,created_at_ms,updated_at_ms) VALUES ('inst-2','codex','local_filesystem','verified',1,'ok',?1,?1)",
                [now],
            )
            .unwrap();
        upsert_source_schedule(
            &storage,
            &SourceScheduleRule {
                instance_id: "inst-2".into(),
                rule_kind: "interval".into(),
                interval_seconds: Some(60),
                time_of_day: None,
                weekday: None,
                tz: String::new(),
                enabled: true,
            },
            now,
            "UTC",
        )
        .unwrap();
        // 停用来源：到期也不返回（禁用后无自动读取）。
        storage
            .conn()
            .execute(
                "UPDATE source_instances SET enabled=0 WHERE instance_id='inst-2'",
                [],
            )
            .unwrap();
        assert!(due_instances(&storage, now + 120_000).unwrap().is_empty());
        // 重新启用：计划恢复生效。
        storage
            .conn()
            .execute(
                "UPDATE source_instances SET enabled=1 WHERE instance_id='inst-2'",
                [],
            )
            .unwrap();
        assert_eq!(
            due_instances(&storage, now + 120_000).unwrap(),
            BTreeSet::from(["inst-2".to_string()])
        );
        // 停用计划本身（rule.enabled=false）同样不到期。
        storage
            .conn()
            .execute(
                "UPDATE extraction_schedules SET enabled=0 WHERE schedule_id='source:inst-2'",
                [],
            )
            .unwrap();
        assert!(due_instances(&storage, now + 120_000).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_tz_rejected_at_validation() {
        let rule = SourceScheduleRule {
            instance_id: "x".into(),
            rule_kind: "daily".into(),
            interval_seconds: None,
            time_of_day: Some("09:00".into()),
            weekday: None,
            tz: "Not/AZone".into(),
            enabled: true,
        };
        assert!(validate_rule(&rule).is_err());
        // 合法 IANA 名通过。
        let ok = SourceScheduleRule {
            tz: "Asia/Shanghai".into(),
            ..rule
        };
        assert!(validate_rule(&ok).is_ok());
    }
}
