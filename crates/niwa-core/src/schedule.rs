//! Schedules: focus times that start a session by themselves (added Oct 17,
//! 2026).
//!
//! A schedule is a start and an end time on some days of the week. When one
//! of its windows comes round, Niwa starts a session with the block list for
//! the whole window, up to `MAX_SCHEDULED_MINUTES`. A window that ends past
//! midnight belongs to the day it starts on. If Niwa wasn't running when a
//! window began, it joins late for the time that's left.

use chrono::{Datelike, Duration, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::species::MIN_SESSION_MINUTES;

/// The longest a scheduled session runs. Sessions started by hand stop at
/// `MAX_SESSION_MINUTES`.
pub const MAX_SCHEDULED_MINUTES: u32 = 12 * 60;
const DAY_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    /// 0 for one not saved yet.
    pub id: i64,
    /// One bit per day, Monday first: bit 0 is Monday, bit 6 is Sunday.
    pub days: u8,
    /// Minutes after midnight.
    pub start: u16,
    pub end: u16,
    pub enabled: bool,
}

/// One time a schedule comes round, in local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub starts: NaiveDateTime,
    pub ends: NaiveDateTime,
}

impl Schedule {
    /// How long each window lasts. An end at or before the start is the next day.
    pub fn minutes(&self) -> u32 {
        let (start, end) = (u32::from(self.start), u32::from(self.end));
        if end > start {
            end - start
        } else {
            DAY_MINUTES - start + end
        }
    }

    /// Why this schedule can't be saved, if it can't.
    pub fn validate(&self) -> Result<(), String> {
        if self.days & 0x7F == 0 {
            return Err("Pick at least one day.".into());
        }
        if u32::from(self.start) >= DAY_MINUTES || u32::from(self.end) >= DAY_MINUTES {
            return Err("That isn't a time of day.".into());
        }
        let minutes = self.minutes();
        if minutes < MIN_SESSION_MINUTES {
            return Err(format!("A focus time needs at least {MIN_SESSION_MINUTES} minutes."));
        }
        if minutes > MAX_SCHEDULED_MINUTES {
            return Err(format!("A focus time can be up to {} hours.", MAX_SCHEDULED_MINUTES / 60));
        }
        Ok(())
    }

    fn runs_on(&self, day: chrono::NaiveDate) -> bool {
        self.days & (1 << day.weekday().num_days_from_monday()) != 0
    }

    fn window_starting_on(&self, day: chrono::NaiveDate) -> Option<Window> {
        if !self.runs_on(day) {
            return None;
        }
        let start = NaiveTime::from_num_seconds_from_midnight_opt(u32::from(self.start) * 60, 0)?;
        let starts = day.and_time(start);
        Some(Window {
            starts,
            ends: starts + Duration::minutes(i64::from(self.minutes())),
        })
    }

    /// The window going on at `now`, if there is one. Disabled schedules have none.
    pub fn window_at(&self, now: NaiveDateTime) -> Option<Window> {
        if !self.enabled {
            return None;
        }
        // Today's, or yesterday's if it runs past midnight.
        [now.date(), now.date() - Duration::days(1)]
            .into_iter()
            .filter_map(|day| self.window_starting_on(day))
            .find(|window| window.starts <= now && now < window.ends)
    }

    /// The next window to start after `now`.
    pub fn next_window(&self, now: NaiveDateTime) -> Option<Window> {
        if !self.enabled {
            return None;
        }
        (0..=7)
            .filter_map(|ahead| self.window_starting_on(now.date() + Duration::days(ahead)))
            .find(|window| window.starts > now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const WEEKDAYS: u8 = 0b001_1111;

    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        // October 2026: the 12th is a Monday.
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(hour, minute, 0).unwrap()
    }

    fn schedule(days: u8, start: (u16, u16), end: (u16, u16)) -> Schedule {
        Schedule { id: 1, days, start: start.0 * 60 + start.1, end: end.0 * 60 + end.1, enabled: true }
    }

    #[test]
    fn a_window_is_on_between_its_times_on_its_days() {
        let mornings = schedule(WEEKDAYS, (9, 0), (12, 30));
        let window = mornings.window_at(at(12, 10, 15)).unwrap();
        assert_eq!((window.starts, window.ends), (at(12, 9, 0), at(12, 12, 30)));
        assert_eq!(mornings.window_at(at(12, 8, 59)), None);
        assert_eq!(mornings.window_at(at(12, 12, 30)), None);
        // Saturday isn't a weekday.
        assert_eq!(mornings.window_at(at(17, 10, 0)), None);
    }

    #[test]
    fn a_window_past_midnight_belongs_to_the_day_it_starts() {
        let late = schedule(1 << 4, (22, 0), (1, 30)); // Fridays
        assert_eq!(late.minutes(), 210);
        let window = late.window_at(at(17, 0, 45)).unwrap(); // early Saturday
        assert_eq!((window.starts, window.ends), (at(16, 22, 0), at(17, 1, 30)));
        // Thursday night isn't Friday.
        assert_eq!(late.window_at(at(16, 0, 45)), None);
    }

    #[test]
    fn the_next_window_skips_days_off() {
        let mornings = schedule(WEEKDAYS, (9, 0), (12, 0));
        assert_eq!(mornings.next_window(at(12, 8, 0)).unwrap().starts, at(12, 9, 0));
        assert_eq!(mornings.next_window(at(12, 9, 0)).unwrap().starts, at(13, 9, 0));
        // From Friday afternoon, the next is Monday.
        assert_eq!(mornings.next_window(at(16, 15, 0)).unwrap().starts, at(19, 9, 0));
    }

    #[test]
    fn a_disabled_schedule_never_runs() {
        let off = Schedule { enabled: false, ..schedule(0x7F, (9, 0), (10, 0)) };
        assert_eq!(off.window_at(at(12, 9, 30)), None);
        assert_eq!(off.next_window(at(12, 8, 0)), None);
    }

    #[test]
    fn only_sensible_schedules_can_be_saved() {
        assert!(schedule(WEEKDAYS, (9, 0), (17, 0)).validate().is_ok());
        assert!(schedule(0, (9, 0), (10, 0)).validate().is_err());
        assert!(schedule(WEEKDAYS, (9, 0), (9, 5)).validate().is_err());
        // 12 hours is the most.
        assert!(schedule(WEEKDAYS, (8, 0), (20, 0)).validate().is_ok());
        assert!(schedule(WEEKDAYS, (8, 0), (20, 1)).validate().is_err());
        // The same start and end would be a whole day.
        assert!(schedule(WEEKDAYS, (9, 0), (9, 0)).validate().is_err());
        assert!(Schedule { start: 24 * 60, ..schedule(WEEKDAYS, (9, 0), (10, 0)) }.validate().is_err());
    }
}
