//! Calendars: a configurable in-world calendar, and a session schedule that converts
//! between session numbers and real-world dates.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Month {
    pub name: String,
    pub days: u32,
}

impl Default for Month {
    fn default() -> Self {
        Month {
            name: "Month".into(),
            days: 30,
        }
    }
}

/// A user-defined fantasy calendar. Years have a fixed number of days (no leap rules yet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameCalendar {
    pub name: String,
    pub months: Vec<Month>,
    pub weekdays: Vec<String>,
    /// Suffix appended to years, e.g. "DR" or "AE".
    pub era: String,
    /// Weekday index (0-based) of day 1, month 1, year 0.
    pub epoch_weekday: u32,
    /// The current in-world date, for "today" in the plot tracker.
    pub current: GameDate,
}

impl Default for GameCalendar {
    fn default() -> Self {
        GameCalendar::simple()
    }
}

impl GameCalendar {
    /// Twelve 30-day months, a 7-day week.
    pub fn simple() -> Self {
        let names = [
            "Deepwinter",
            "Thawmoon",
            "Seedtime",
            "Rainfall",
            "Greening",
            "Highsun",
            "Midsummer",
            "Harvest",
            "Leaffall",
            "Mistmoon",
            "Frostfall",
            "Yearsend",
        ];
        GameCalendar {
            name: "Simple calendar".into(),
            months: names.iter().map(|n| Month { name: n.to_string(), days: 30 }).collect(),
            weekdays: ["Firstday", "Secondday", "Thirdday", "Fourthday", "Fifthday", "Sixthday", "Restday"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            era: String::new(),
            epoch_weekday: 0,
            current: GameDate { year: 1, month: 1, day: 1 },
        }
    }

    /// Gregorian month names and lengths (non-leap).
    pub fn gregorian_like() -> Self {
        let m = [
            ("January", 31),
            ("February", 28),
            ("March", 31),
            ("April", 30),
            ("May", 31),
            ("June", 30),
            ("July", 31),
            ("August", 31),
            ("September", 30),
            ("October", 31),
            ("November", 30),
            ("December", 31),
        ];
        GameCalendar {
            name: "Gregorian-style".into(),
            months: m.iter().map(|(n, d)| Month { name: n.to_string(), days: *d }).collect(),
            weekdays: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            era: String::new(),
            epoch_weekday: 0,
            current: GameDate { year: 1, month: 1, day: 1 },
        }
    }

    pub fn days_in_year(&self) -> i64 {
        self.months.iter().map(|m| m.days as i64).sum::<i64>().max(1)
    }

    pub fn validate(&self, d: &GameDate) -> Result<(), String> {
        if self.months.is_empty() {
            return Err("the calendar has no months".into());
        }
        if d.month < 1 || d.month as usize > self.months.len() {
            return Err(format!("month must be 1–{}", self.months.len()));
        }
        let max = self.months[d.month as usize - 1].days;
        if d.day < 1 || d.day > max {
            return Err(format!("{} has {} days", self.months[d.month as usize - 1].name, max));
        }
        Ok(())
    }

    /// Clamp a date into range (useful after the calendar is edited).
    pub fn clamp(&self, d: GameDate) -> GameDate {
        if self.months.is_empty() {
            return d;
        }
        let month = d.month.clamp(1, self.months.len() as u32);
        let max = self.months[month as usize - 1].days.max(1);
        GameDate {
            year: d.year,
            month,
            day: d.day.clamp(1, max),
        }
    }

    /// Days since year 0, month 1, day 1. Sorts dates chronologically.
    pub fn ordinal(&self, d: &GameDate) -> i64 {
        let d = self.clamp(*d);
        let before_month: i64 = self.months[..(d.month as usize - 1).min(self.months.len())].iter().map(|m| m.days as i64).sum();
        d.year as i64 * self.days_in_year() + before_month + (d.day as i64 - 1)
    }

    pub fn from_ordinal(&self, ord: i64) -> GameDate {
        let diy = self.days_in_year();
        let year = ord.div_euclid(diy);
        let mut rem = ord.rem_euclid(diy);
        for (i, m) in self.months.iter().enumerate() {
            if rem < m.days as i64 {
                return GameDate {
                    year: year as i32,
                    month: i as u32 + 1,
                    day: rem as u32 + 1,
                };
            }
            rem -= m.days as i64;
        }
        GameDate {
            year: year as i32,
            month: 1,
            day: 1,
        }
    }

    pub fn add_days(&self, d: &GameDate, days: i64) -> GameDate {
        self.from_ordinal(self.ordinal(d) + days)
    }

    pub fn days_between(&self, a: &GameDate, b: &GameDate) -> i64 {
        self.ordinal(b) - self.ordinal(a)
    }

    pub fn weekday(&self, d: &GameDate) -> Option<&str> {
        if self.weekdays.is_empty() {
            return None;
        }
        let n = self.weekdays.len() as i64;
        let idx = (self.ordinal(d) + self.epoch_weekday as i64).rem_euclid(n);
        Some(&self.weekdays[idx as usize])
    }

    pub fn format(&self, d: &GameDate) -> String {
        let d = self.clamp(*d);
        let month = self.months.get(d.month as usize - 1).map(|m| m.name.as_str()).unwrap_or("?");
        let era = if self.era.trim().is_empty() {
            String::new()
        } else {
            format!(" {}", self.era.trim())
        };
        format!("{} {}, {}{}", d.day, month, d.year, era)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct GameDate {
    pub year: i32,
    /// 1-based month index
    pub month: u32,
    /// 1-based day of month
    pub day: u32,
}

impl Default for GameDate {
    fn default() -> Self {
        GameDate { year: 1, month: 1, day: 1 }
    }
}

/// Regular play schedule. Lets the DM convert session numbers to real dates and back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionSchedule {
    /// Date of session number `first_number`.
    pub first_date: Option<NaiveDate>,
    pub first_number: u32,
    /// Days between sessions (7 = weekly, 14 = every other week).
    pub interval_days: u32,
    /// Dates on which play is skipped (holidays); later sessions shift back.
    pub skipped_dates: Vec<NaiveDate>,
}

impl Default for SessionSchedule {
    fn default() -> Self {
        SessionSchedule {
            first_date: None,
            first_number: 1,
            interval_days: 7,
            skipped_dates: Vec::new(),
        }
    }
}

impl SessionSchedule {
    /// Expected real-world date for session `n`, honouring skipped dates.
    pub fn date_for_session(&self, n: u32) -> Option<NaiveDate> {
        let start = self.first_date?;
        if n < self.first_number || self.interval_days == 0 {
            return None;
        }
        let step = Duration::days(self.interval_days as i64);
        let mut date = start;
        let mut number = self.first_number;
        // Walk forward slot by slot, skipping cancelled dates. Bounded loop.
        let mut guard = 0;
        while self.skipped_dates.contains(&date) && guard < 10_000 {
            date += step;
            guard += 1;
        }
        while number < n && guard < 100_000 {
            date += step;
            if !self.skipped_dates.contains(&date) {
                number += 1;
            }
            guard += 1;
        }
        Some(date)
    }

    /// The session number expected on or before `date` (the most recent scheduled slot).
    pub fn session_for_date(&self, date: NaiveDate) -> Option<u32> {
        let start = self.first_date?;
        if date < start || self.interval_days == 0 {
            return None;
        }
        let mut n = self.first_number;
        let mut last = None;
        let mut guard = 0;
        loop {
            let d = self.date_for_session(n)?;
            if d > date || guard > 100_000 {
                break;
            }
            last = Some(n);
            n += 1;
            guard += 1;
        }
        last
    }

    /// Number of scheduled sessions between today and `date` (inclusive of the end).
    pub fn sessions_until(&self, today: NaiveDate, target: NaiveDate) -> Option<i64> {
        let a = self.session_for_date(today).unwrap_or(self.first_number.saturating_sub(1));
        let b = self.session_for_date(target)?;
        Some(b as i64 - a as i64)
    }
}

/// First day of the week in calendar grids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeekStart {
    /// US style.
    #[default]
    Sunday,
    Monday,
}

impl WeekStart {
    pub fn first(self) -> chrono::Weekday {
        match self {
            WeekStart::Sunday => chrono::Weekday::Sun,
            WeekStart::Monday => chrono::Weekday::Mon,
        }
    }
    /// Two-letter weekday headers in display order.
    pub fn headers(self) -> [&'static str; 7] {
        match self {
            WeekStart::Sunday => ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
            WeekStart::Monday => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
        }
    }
}

/// How real-world dates are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DateStyle {
    /// "Sat, Oct 31, 2026"
    #[default]
    Us,
    /// "Sat 31 Oct 2026"
    International,
    /// "2026-10-31"
    Iso,
}

/// Full date in the chosen style.
pub fn format_date(d: NaiveDate, style: DateStyle) -> String {
    match style {
        DateStyle::Us => d.format("%a, %b %-d, %Y").to_string(),
        DateStyle::International => d.format("%a %-d %b %Y").to_string(),
        DateStyle::Iso => d.format("%Y-%m-%d").to_string(),
    }
}

/// Short date without the year ("Oct 31" / "31 Oct" / "10-31").
pub fn format_date_short(d: NaiveDate, style: DateStyle) -> String {
    match style {
        DateStyle::Us => d.format("%b %-d").to_string(),
        DateStyle::International => d.format("%-d %b").to_string(),
        DateStyle::Iso => d.format("%m-%d").to_string(),
    }
}

/// Six weeks of dates covering `year`/`month`, starting on the chosen weekday —
/// the cells of a month calendar.
pub fn month_grid(year: i32, month: u32, start: WeekStart) -> Vec<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default();
    let mut d = first;
    while d.weekday() != start.first() {
        d -= Duration::days(1);
    }
    (0..42).map(|i| d + Duration::days(i)).collect()
}

/// Human-friendly real date, e.g. "Sat 14 Jun 2025" (international style).
pub fn format_real_date(d: NaiveDate) -> String {
    let wd = match d.weekday() {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    };
    format!("{} {}", wd, d.format("%d %b %Y"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_roundtrip() {
        let c = GameCalendar::gregorian_like();
        for ord in [-400, -1, 0, 1, 30, 31, 58, 59, 364, 365, 1000, 99_999] {
            let d = c.from_ordinal(ord);
            assert_eq!(c.ordinal(&d), ord, "{d:?}");
        }
        let d = GameDate { year: 1492, month: 3, day: 1 };
        assert_eq!(c.add_days(&d, -1), GameDate { year: 1492, month: 2, day: 28 });
        assert_eq!(c.add_days(&GameDate { year: 1, month: 12, day: 31 }, 1), GameDate { year: 2, month: 1, day: 1 });
    }

    #[test]
    fn validate_and_format() {
        let mut c = GameCalendar::simple();
        c.era = "DR".into();
        assert!(c.validate(&GameDate { year: 1, month: 13, day: 1 }).is_err());
        assert!(c.validate(&GameDate { year: 1, month: 1, day: 31 }).is_err());
        assert!(c.validate(&GameDate { year: 1, month: 12, day: 30 }).is_ok());
        assert_eq!(c.format(&GameDate { year: 1492, month: 2, day: 5 }), "5 Thawmoon, 1492 DR");
        assert_eq!(
            c.days_between(&GameDate { year: 1, month: 1, day: 1 }, &GameDate { year: 2, month: 1, day: 1 }),
            360
        );
    }

    #[test]
    fn weekdays_cycle() {
        let c = GameCalendar::simple();
        let d = GameDate { year: 0, month: 1, day: 1 };
        assert_eq!(c.weekday(&d), Some("Firstday"));
        assert_eq!(c.weekday(&c.add_days(&d, 7)), Some("Firstday"));
        assert_eq!(c.weekday(&c.add_days(&d, 6)), Some("Restday"));
    }

    #[test]
    fn session_schedule() {
        let s = SessionSchedule {
            first_date: NaiveDate::from_ymd_opt(2026, 1, 3),
            first_number: 1,
            interval_days: 7,
            skipped_dates: vec![NaiveDate::from_ymd_opt(2026, 1, 17).unwrap()],
        };
        assert_eq!(s.date_for_session(1), NaiveDate::from_ymd_opt(2026, 1, 3));
        assert_eq!(s.date_for_session(2), NaiveDate::from_ymd_opt(2026, 1, 10));
        // 17th skipped -> session 3 on the 24th
        assert_eq!(s.date_for_session(3), NaiveDate::from_ymd_opt(2026, 1, 24));
        assert_eq!(s.session_for_date(NaiveDate::from_ymd_opt(2026, 1, 20).unwrap()), Some(2));
        assert_eq!(s.session_for_date(NaiveDate::from_ymd_opt(2026, 1, 24).unwrap()), Some(3));
        assert_eq!(s.session_for_date(NaiveDate::from_ymd_opt(2025, 12, 1).unwrap()), None);
        assert_eq!(s.date_for_session(0), None);
        let empty = SessionSchedule::default();
        assert_eq!(empty.date_for_session(3), None);
    }

    #[test]
    fn date_styles_and_week_start() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        assert_eq!(format_date(d, DateStyle::Us), "Sat, Oct 31, 2026");
        assert_eq!(format_date(d, DateStyle::International), "Sat 31 Oct 2026");
        assert_eq!(format_date(d, DateStyle::Iso), "2026-10-31");
        assert_eq!(format_date_short(NaiveDate::from_ymd_opt(2026, 3, 5).unwrap(), DateStyle::Us), "Mar 5");
        // October 2026 starts on a Thursday.
        let us = month_grid(2026, 10, WeekStart::Sunday);
        assert_eq!(us.len(), 42);
        assert_eq!(us[0], NaiveDate::from_ymd_opt(2026, 9, 27).unwrap());
        assert_eq!(us[0].weekday(), chrono::Weekday::Sun);
        assert_eq!(us[4], NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        let eu = month_grid(2026, 10, WeekStart::Monday);
        assert_eq!(eu[0], NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());
        assert_eq!(eu[3], NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        // a month starting on the week's first day starts the grid on the 1st
        assert_eq!(month_grid(2026, 11, WeekStart::Sunday)[0], NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(WeekStart::default(), WeekStart::Sunday);
        assert_eq!(WeekStart::Sunday.headers()[0], "Su");
    }

    #[test]
    fn real_date_format() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        assert_eq!(format_real_date(d), "Sat 31 Oct 2026");
    }
}
