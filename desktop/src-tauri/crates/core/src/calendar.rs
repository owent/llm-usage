//! IANA calendars: day/week/month boundaries, DST 23/25-hour days, ISO weeks and retention cutoffs.
//! UTC milliseconds represent half-open time intervals [start, end).

use crate::error::CoreError;
use jiff::civil::{date, Date};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};

/// Week start defaults to ISO Monday; Sunday-start week labels use the start date.
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

/// A fixed IANA timezone keeps this Calendar independent of later system timezone changes.
#[derive(Debug, Clone)]
pub struct Calendar {
    tz: TimeZone,
    tz_name: String,
}

impl Calendar {
    pub fn new(iana_name: &str) -> Result<Self, CoreError> {
        let tz = TimeZone::get(iana_name)
            .map_err(|e| CoreError::Calendar(format!("unknown IANA time zone {iana_name}: {e}")))?;
        Ok(Calendar {
            tz,
            tz_name: iana_name.to_string(),
        })
    }

    pub fn utc() -> Self {
        Calendar {
            tz: TimeZone::UTC,
            tz_name: "UTC".to_string(),
        }
    }

    pub fn tz_name(&self) -> &str {
        &self.tz_name
    }

    /// UTC milliseconds to local date.
    pub fn local_day_of(&self, ms: i64) -> Result<Date, CoreError> {
        let ts = Timestamp::from_millisecond(ms)
            .map_err(|e| CoreError::Calendar(format!("timestamp {ms}ms out of range: {e}")))?;
        Ok(ts.to_zoned(self.tz.clone()).date())
    }

    /// UTC milliseconds to local hour 0-23; both occurrences of a repeated DST hour share a label.
    /// Use [`Calendar::offset_seconds_at`] to distinguish their offsets.
    pub fn local_hour_of(&self, ms: i64) -> Result<u32, CoreError> {
        let ts = Timestamp::from_millisecond(ms)
            .map_err(|e| CoreError::Calendar(format!("timestamp {ms}ms out of range: {e}")))?;
        let hour = ts.to_zoned(self.tz.clone()).hour();
        Ok(if hour < 0 { 0 } else { hour as u32 })
    }

    /// UTC milliseconds to ISO weekday, Monday=1 through Sunday=7.
    pub fn local_weekday_of(&self, ms: i64) -> Result<u8, CoreError> {
        let ts = Timestamp::from_millisecond(ms)
            .map_err(|e| CoreError::Calendar(format!("timestamp {ms}ms out of range: {e}")))?;
        let wd = ts.to_zoned(self.tz.clone()).weekday();
        let n = match wd {
            jiff::civil::Weekday::Monday => 1,
            jiff::civil::Weekday::Tuesday => 2,
            jiff::civil::Weekday::Wednesday => 3,
            jiff::civil::Weekday::Thursday => 4,
            jiff::civil::Weekday::Friday => 5,
            jiff::civil::Weekday::Saturday => 6,
            jiff::civil::Weekday::Sunday => 7,
        };
        Ok(n)
    }

    /// UTC offset seconds at this instant, distinguishing repeated DST hours.
    pub fn offset_seconds_at(&self, ms: i64) -> Result<i32, CoreError> {
        let ts = Timestamp::from_millisecond(ms)
            .map_err(|e| CoreError::Calendar(format!("timestamp {ms}ms out of range: {e}")))?;
        Ok(ts.to_zoned(self.tz.clone()).offset().seconds())
    }

    /// Local date as UTC [start_ms, end_ms); DST days may contain 23 or 25 hours.
    /// jiff resolves a nonexistent local midnight to the next valid time.
    pub fn day_range_ms(&self, day: Date) -> Result<(i64, i64), CoreError> {
        let start = day.to_zoned(self.tz.clone())?;
        let end = day
            .checked_add(Span::new().days(1))
            .and_then(|d| d.to_zoned(self.tz.clone()))?;
        Ok((
            start.timestamp().as_millisecond(),
            end.timestamp().as_millisecond(),
        ))
    }

    /// Local date containing now_ms.
    pub fn today(&self, now_ms: i64) -> Result<Date, CoreError> {
        self.local_day_of(now_ms)
    }

    /// Retain today and the preceding D-1 local dates; cutoff is today minus D-1 days.
    pub fn retention_cutoff_day(&self, today: Date, days: u32) -> Result<Date, CoreError> {
        if days == 0 {
            return Err(CoreError::Calendar(
                "retention days must be >= 1".to_string(),
            ));
        }
        Ok(today.checked_sub(Span::new().days(i64::from(days) - 1))?)
    }

    /// First date of this local week, using week_start.
    pub fn week_start_of(&self, day: Date, week_start: WeekStart) -> Date {
        let offset = match week_start {
            WeekStart::Monday => i64::from(day.weekday().to_monday_zero_offset()),
            WeekStart::Sunday => i64::from(day.weekday().to_sunday_zero_offset()),
        };
        day.checked_sub(Span::new().days(offset))
            .expect("week start underflow is unreachable for civil dates")
    }

    /// Monday-start labels use ISO week/year, such as 2026-W01;
    /// Sunday-start labels use their start date, such as 2025-12-28.
    pub fn week_label(&self, day: Date, week_start: WeekStart) -> String {
        match week_start {
            WeekStart::Monday => {
                let (year, week) = iso_week(day);
                format!("{year:04}-W{week:02}")
            }
            WeekStart::Sunday => self.week_start_of(day, week_start).to_string(),
        }
    }

    /// First local date of the calendar month.
    pub fn month_start_of(&self, day: Date) -> Date {
        day.first_of_month()
    }

    /// Month label, such as 2026-09.
    pub fn month_label(&self, day: Date) -> String {
        format!("{:04}-{:02}", day.year(), day.month())
    }

    /// Calendar month as a half-open UTC interval.
    pub fn month_range_ms(&self, day: Date) -> Result<(i64, i64), CoreError> {
        let first = day.first_of_month();
        let next = first
            .checked_add(Span::new().months(1))
            .map_err(CoreError::from)?;
        Ok((
            first
                .to_zoned(self.tz.clone())?
                .timestamp()
                .as_millisecond(),
            next.to_zoned(self.tz.clone())?.timestamp().as_millisecond(),
        ))
    }
}

/// ISO 8601 (week-year, week-number): the week's Thursday determines its year.
/// Week number = (Thursday day-of-year - 1) / 7 + 1.
pub fn iso_week(day: Date) -> (i16, i16) {
    let dow = i64::from(day.weekday().to_monday_zero_offset());
    let thursday = day
        .checked_add(Span::new().days(3 - dow))
        .expect("iso week overflow is unreachable for civil dates");
    let year = thursday.year();
    let week = (thursday.day_of_year() - 1) / 7 + 1;
    (year, week)
}

/// Parse YYYY-MM-DD.
pub fn parse_date(s: &str) -> Result<Date, CoreError> {
    s.parse::<Date>()
        .map_err(|e| CoreError::Calendar(format!("bad date {s:?}: {e}")))
}

/// Last date of the month for displayed end labels.
pub fn month_end(day: Date) -> Date {
    day.last_of_month()
}

/// Date constructor for tests and callers.
pub fn ymd(y: i16, m: i8, d: i8) -> Date {
    date(y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_week_cross_year() {
        // Monday 2025-12-29 belongs to ISO 2026 week 1.
        assert_eq!(iso_week(ymd(2025, 12, 29)), (2026, 1));
        assert_eq!(iso_week(ymd(2026, 1, 4)), (2026, 1));
        assert_eq!(iso_week(ymd(2026, 1, 5)), (2026, 2));
        // 2024-12-30 through 2025-01-05 belongs to 2025-W01.
        assert_eq!(iso_week(ymd(2024, 12, 31)), (2025, 1));
        // The first ISO week of 2027 starts on 2027-01-04.
        assert_eq!(iso_week(ymd(2027, 1, 3)), (2026, 53));
        assert_eq!(iso_week(ymd(2027, 1, 4)), (2027, 1));
    }

    #[test]
    fn dst_day_lengths() {
        let cal = Calendar::new("America/New_York").unwrap();
        // Spring-forward 2026-03-08 has 23 hours.
        let (s, e) = cal.day_range_ms(ymd(2026, 3, 8)).unwrap();
        assert_eq!((e - s) / 3_600_000, 23);
        // Fall-back 2026-11-01 has 25 hours.
        let (s, e) = cal.day_range_ms(ymd(2026, 11, 1)).unwrap();
        assert_eq!((e - s) / 3_600_000, 25);
        // An ordinary day has 24 hours.
        let (s, e) = cal.day_range_ms(ymd(2026, 3, 9)).unwrap();
        assert_eq!((e - s) / 3_600_000, 24);
    }
}
