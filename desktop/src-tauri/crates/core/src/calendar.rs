//! IANA 时区日历：日/周/月边界（含 DST 23/25 小时日）、ISO 周、保留截止。
//! 时间区间统一半开 `[start, end)`；存 UTC 毫秒整数。

use crate::error::CoreError;
use jiff::civil::{date, Date};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};

/// 一周起始日。默认周一（ISO）；周日起始时周标签必须使用开始日期。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekStart {
    Monday,
    Sunday,
}

impl WeekStart {
    pub fn as_str(self) -> &'static str {
        match self {
            WeekStart::Monday => "monday",
            WeekStart::Sunday => "sunday",
        }
    }

    pub fn parse(s: &str) -> Result<Self, CoreError> {
        match s {
            "monday" => Ok(WeekStart::Monday),
            "sunday" => Ok(WeekStart::Sunday),
            other => Err(CoreError::Calendar(format!("unknown week start: {other}"))),
        }
    }
}

/// 绑定一个固定 IANA 时区的日历。保存后不随系统时区变化悄悄重分桶。
#[derive(Debug, Clone)]
pub struct Calendar {
    tz: TimeZone,
    tz_name: String,
}

impl Calendar {
    pub fn new(iana_name: &str) -> Result<Self, CoreError> {
        let tz = TimeZone::get(iana_name)
            .map_err(|e| CoreError::Calendar(format!("unknown IANA time zone {iana_name}: {e}")))?;
        Ok(Calendar { tz, tz_name: iana_name.to_string() })
    }

    pub fn utc() -> Self {
        Calendar { tz: TimeZone::UTC, tz_name: "UTC".to_string() }
    }

    pub fn tz_name(&self) -> &str {
        &self.tz_name
    }

    /// UTC 毫秒 → 本地日。
    pub fn local_day_of(&self, ms: i64) -> Result<Date, CoreError> {
        let ts = Timestamp::from_millisecond(ms)
            .map_err(|e| CoreError::Calendar(format!("timestamp {ms}ms out of range: {e}")))?;
        Ok(ts.to_zoned(self.tz.clone()).date())
    }

    /// UTC 毫秒 → 该时刻的 UTC offset 秒（分辨 DST 重复小时）。
    pub fn offset_seconds_at(&self, ms: i64) -> Result<i32, CoreError> {
        let ts = Timestamp::from_millisecond(ms)
            .map_err(|e| CoreError::Calendar(format!("timestamp {ms}ms out of range: {e}")))?;
        Ok(ts.to_zoned(self.tz.clone()).offset().seconds())
    }

    /// 本地日的 UTC 半开区间 `[start_ms, end_ms)`。DST 日长度可为 23/25 小时；
    /// 不存在的本地午夜由 jiff 顺延到首个有效时刻。
    pub fn day_range_ms(&self, day: Date) -> Result<(i64, i64), CoreError> {
        let start = day.to_zoned(self.tz.clone())?;
        let end = day
            .checked_add(Span::new().days(1))
            .and_then(|d| d.to_zoned(self.tz.clone()))?;
        Ok((start.timestamp().as_millisecond(), end.timestamp().as_millisecond()))
    }

    /// now 所在本地日。
    pub fn today(&self, now_ms: i64) -> Result<Date, CoreError> {
        self.local_day_of(now_ms)
    }

    /// 有限保留 D 天的截止本地日：今天起点往前 D−1 天；今天与前 D−1 个本地日保留。
    pub fn retention_cutoff_day(&self, today: Date, days: u32) -> Result<Date, CoreError> {
        if days == 0 {
            return Err(CoreError::Calendar("retention days must be >= 1".to_string()));
        }
        Ok(today.checked_sub(Span::new().days(i64::from(days) - 1))?)
    }

    /// 本地日所在周的起始日（按 week_start）。
    pub fn week_start_of(&self, day: Date, week_start: WeekStart) -> Date {
        let offset = match week_start {
            WeekStart::Monday => i64::from(day.weekday().to_monday_zero_offset()),
            WeekStart::Sunday => i64::from(day.weekday().to_sunday_zero_offset()),
        };
        day.checked_sub(Span::new().days(offset))
            .expect("week start underflow is unreachable for civil dates")
    }

    /// 周标签：周一起始用 ISO 周（标签含周所属年份，如 2026-W01）；
    /// 周日起始用开始日期（如 2025-12-28），不能冒充 ISO 周。
    pub fn week_label(&self, day: Date, week_start: WeekStart) -> String {
        match week_start {
            WeekStart::Monday => {
                let (year, week) = iso_week(day);
                format!("{year:04}-W{week:02}")
            }
            WeekStart::Sunday => self.week_start_of(day, week_start).to_string(),
        }
    }

    /// 本地日所在自然月的第一天。
    pub fn month_start_of(&self, day: Date) -> Date {
        day.first_of_month()
    }

    /// 月标签，如 2026-09。
    pub fn month_label(&self, day: Date) -> String {
        format!("{:04}-{:02}", day.year(), day.month())
    }

    /// 月份 UTC 半开区间。
    pub fn month_range_ms(&self, day: Date) -> Result<(i64, i64), CoreError> {
        let first = day.first_of_month();
        let next = first
            .checked_add(Span::new().months(1))
            .map_err(CoreError::from)?;
        Ok((
            first.to_zoned(self.tz.clone())?.timestamp().as_millisecond(),
            next.to_zoned(self.tz.clone())?.timestamp().as_millisecond(),
        ))
    }
}

/// ISO 8601 周：(周所属年份, 周号)。算法：本周周四所属年即 ISO 年；
/// 周号 = (周四的年内序数 - 1) / 7 + 1。
pub fn iso_week(day: Date) -> (i16, i16) {
    let dow = i64::from(day.weekday().to_monday_zero_offset());
    let thursday = day
        .checked_add(Span::new().days(3 - dow))
        .expect("iso week overflow is unreachable for civil dates");
    let year = thursday.year();
    let week = (thursday.day_of_year() - 1) / 7 + 1;
    (year, week)
}

/// 解析 YYYY-MM-DD。
pub fn parse_date(s: &str) -> Result<Date, CoreError> {
    s.parse::<Date>()
        .map_err(|e| CoreError::Calendar(format!("bad date {s:?}: {e}")))
}

/// 月份最后一天（用于月区间结束标签显示）。
pub fn month_end(day: Date) -> Date {
    day.last_of_month()
}

/// 便捷构造（测试与调用方）。
pub fn ymd(y: i16, m: i8, d: i8) -> Date {
    date(y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_week_cross_year() {
        // 2025-12-29 是周一，属于 ISO 2026 年第 1 周。
        assert_eq!(iso_week(ymd(2025, 12, 29)), (2026, 1));
        assert_eq!(iso_week(ymd(2026, 1, 4)), (2026, 1));
        assert_eq!(iso_week(ymd(2026, 1, 5)), (2026, 2));
        // 2024-12-30..2025-01-05 属 2025-W01。
        assert_eq!(iso_week(ymd(2024, 12, 31)), (2025, 1));
        // 2027 年第一周从 2027-01-04 开始。
        assert_eq!(iso_week(ymd(2027, 1, 3)), (2026, 53));
        assert_eq!(iso_week(ymd(2027, 1, 4)), (2027, 1));
    }

    #[test]
    fn dst_day_lengths() {
        let cal = Calendar::new("America/New_York").unwrap();
        // 2026-03-08 春季拨快：23 小时。
        let (s, e) = cal.day_range_ms(ymd(2026, 3, 8)).unwrap();
        assert_eq!((e - s) / 3_600_000, 23);
        // 2026-11-01 秋季拨回：25 小时。
        let (s, e) = cal.day_range_ms(ymd(2026, 11, 1)).unwrap();
        assert_eq!((e - s) / 3_600_000, 25);
        // 普通日 24 小时。
        let (s, e) = cal.day_range_ms(ymd(2026, 3, 9)).unwrap();
        assert_eq!((e - s) / 3_600_000, 24);
    }
}
