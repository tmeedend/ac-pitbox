//! Story time (SPEC-story-mode, Story time): a calendar date that only moves
//! when a Beat (or a successful event) says so, and an in-story clock time.
//!
//! The engine always counts in a proleptic Gregorian calendar. How the date is
//! *shown* - a real date, "year N of the story", seasons, nothing - is a story
//! setting the frame applies; a relative story simply starts on 0001-01-01.
//! Written by hand rather than pulling `chrono`: three operations are needed.

use std::fmt;
use std::str::FromStr;

/// A calendar date. Field order makes the derived ordering chronological.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoryDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

/// An in-story time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Clock {
    pub hour: u8,
    pub minute: u8,
}

/// A length of story time: "2 days", "1 week", "3 months", "1 year".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Days(u32),
    Weeks(u32),
    Months(u32),
    Years(u32),
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

impl StoryDate {
    pub fn new(year: i32, month: u8, day: u8) -> Option<StoryDate> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)).then_some(StoryDate {
            year,
            month,
            day,
        })
    }

    /// The start of a story with a relative date ("year 1").
    pub const RELATIVE_START: StoryDate = StoryDate {
        year: 1,
        month: 1,
        day: 1,
    };

    // Days since 1970-01-01 and back: Howard Hinnant's `days_from_civil`.
    fn to_days(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn from_days(z: i64) -> StoryDate {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        StoryDate { year, month, day }
    }

    /// Adds `months`, keeping the day of the month when it exists and
    /// clamping it otherwise: one month after January 31st is the last day of
    /// February, never March 3rd - "advance one month" must land in the next
    /// month, which is what an author means.
    fn add_months(self, months: u32) -> StoryDate {
        let total = i64::from(self.month) - 1 + i64::from(months);
        let year = self.year + total.div_euclid(12) as i32;
        let month = (total.rem_euclid(12) + 1) as u8;
        let day = self.day.min(days_in_month(year, month));
        StoryDate { year, month, day }
    }

    pub fn advance(self, span: Span) -> StoryDate {
        match span {
            Span::Days(n) => StoryDate::from_days(self.to_days() + i64::from(n)),
            Span::Weeks(n) => StoryDate::from_days(self.to_days() + 7 * i64::from(n)),
            Span::Months(n) => self.add_months(n),
            Span::Years(n) => self.add_months(n.saturating_mul(12)),
        }
    }
}

impl fmt::Display for StoryDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for StoryDate {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("invalid date `{s}` (expected YYYY-MM-DD)");
        let mut parts = s.trim().split('-');
        let (Some(y), Some(m), Some(d), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err(bad());
        };
        if y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return Err(bad());
        }
        let (y, m, d) = (
            y.parse().map_err(|_| bad())?,
            m.parse().map_err(|_| bad())?,
            d.parse().map_err(|_| bad())?,
        );
        StoryDate::new(y, m, d).ok_or_else(bad)
    }
}

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl FromStr for Clock {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("invalid time `{s}` (expected HH:MM)");
        let (h, m) = s.trim().split_once(':').ok_or_else(bad)?;
        let (hour, minute): (u8, u8) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
        if hour > 23 || minute > 59 || m.len() != 2 {
            return Err(bad());
        }
        Ok(Clock { hour, minute })
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (n, unit) = match *self {
            Span::Days(n) => (n, "day"),
            Span::Weeks(n) => (n, "week"),
            Span::Months(n) => (n, "month"),
            Span::Years(n) => (n, "year"),
        };
        write!(f, "{n} {unit}{}", if n == 1 { "" } else { "s" })
    }
}

impl FromStr for Span {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("invalid duration `{s}` (expected e.g. `2 days`, `1 week`, `3 months`, `1 year`)");
        let mut words = s.split_whitespace();
        let (Some(n), Some(unit), None) = (words.next(), words.next(), words.next()) else {
            return Err(bad());
        };
        let n: u32 = n.parse().map_err(|_| bad())?;
        match unit.trim_end_matches('s') {
            "day" => Ok(Span::Days(n)),
            "week" => Ok(Span::Weeks(n)),
            "month" => Ok(Span::Months(n)),
            "year" => Ok(Span::Years(n)),
            _ => Err(bad()),
        }
    }
}

serde_as_text!(StoryDate);
serde_as_text!(Clock);
serde_as_text!(Span);

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> StoryDate {
        s.parse().unwrap()
    }

    // Story time: "advance one month" lands in the next month, whatever the
    // day - a clamped day, never an overflow into the month after.
    #[test]
    fn a_month_after_the_31st_is_the_end_of_the_next_month() {
        assert_eq!(
            date("1999-01-31").advance(Span::Months(1)),
            date("1999-02-28"),
            "non-leap February"
        );
        assert_eq!(
            date("2000-01-31").advance(Span::Months(1)),
            date("2000-02-29"),
            "leap February"
        );
        assert_eq!(
            date("1999-11-15").advance(Span::Months(3)),
            date("2000-02-15"),
            "across a year"
        );
        assert_eq!(
            date("2000-02-29").advance(Span::Years(1)),
            date("2001-02-28"),
            "leap day + 1 year"
        );
    }

    #[test]
    fn days_and_weeks_cross_months_and_years() {
        assert_eq!(date("1999-12-30").advance(Span::Days(3)), date("2000-01-02"));
        assert_eq!(
            date("2000-02-25").advance(Span::Weeks(1)),
            date("2000-03-03"),
            "leap February has 29 days"
        );
        assert_eq!(
            StoryDate::RELATIVE_START.advance(Span::Days(365)),
            date("0002-01-01"),
            "year 1 is not leap"
        );
    }

    #[test]
    fn text_forms_round_trip_and_reject_nonsense() {
        assert_eq!(date("0001-01-01"), StoryDate::RELATIVE_START);
        assert_eq!(date("1998-03-01").to_string(), "1998-03-01");
        assert!("1999-02-29".parse::<StoryDate>().is_err(), "no Feb 29th in 1999");
        assert!("1999-3-01".parse::<StoryDate>().is_err(), "two-digit month");
        assert_eq!("18:40".parse::<Clock>().unwrap().to_string(), "18:40");
        assert!("24:00".parse::<Clock>().is_err());
        assert_eq!("1 month".parse::<Span>().unwrap(), Span::Months(1));
        assert_eq!("3 weeks".parse::<Span>().unwrap().to_string(), "3 weeks");
        assert!("one month".parse::<Span>().is_err());
    }
}
