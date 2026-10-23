//! The numbers on the Stats screen, worked out from sessions and slips.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate};

use crate::store::Outcome;

/// A session that has ended, as the Stats screen needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndedSession {
    pub day: NaiveDate,
    pub outcome: Outcome,
    pub planned_minutes: u32,
    /// How long it actually ran before it ended.
    pub focused_seconds: u32,
}

impl EndedSession {
    /// The time it counts for on the Stats screen. A grown session counts in
    /// full; a withered one still counts for the time that was focused before
    /// it ended.
    pub fn minutes(&self) -> u32 {
        match self.outcome {
            Outcome::Grown => self.planned_minutes,
            Outcome::Withered => (self.focused_seconds / 60).min(self.planned_minutes),
        }
    }
}

/// A slip, as the Stats screen needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlipOn {
    pub day: NaiveDate,
    pub app: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Week {
    /// Minutes focused on each day, Monday first, withered sessions included.
    pub days: [u32; 7],
    /// Which of `days` is today (0 is Monday).
    pub today: usize,
    pub minutes: u32,
    /// The change from last week, as a whole percentage. `None` when there
    /// was nothing last week to compare with.
    pub change_percent: Option<i32>,
    pub month_minutes: u32,
    /// Finished sessions as a share of all sessions this month. `None` when
    /// nothing has ended this month.
    pub completion_percent: Option<u32>,
    /// The apps launched most often during sessions this week, most first.
    pub tempting: Vec<(String, u32)>,
}

/// The Monday of the week `day` falls in.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(i64::from(day.weekday().num_days_from_monday()))
}

/// The first day of the month `day` falls in.
pub fn month_start(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

/// How many apps "Most tempting" lists.
const TEMPTING: usize = 3;

/// Works out this week's numbers. `sessions` and `slips` need cover at least
/// last week and this month; anything older is ignored.
pub fn week(sessions: &[EndedSession], slips: &[SlipOn], today: NaiveDate) -> Week {
    let monday = week_start(today);
    let last_monday = monday - Duration::days(7);
    let first_of_month = month_start(today);

    let mut days = [0u32; 7];
    let mut last_week = 0u32;
    let mut month_minutes = 0u32;
    let (mut month_grown, mut month_ended) = (0u32, 0u32);

    for session in sessions.iter().filter(|session| session.day <= today) {
        let minutes = session.minutes();
        if session.day >= first_of_month {
            month_ended += 1;
            month_minutes += minutes;
            if session.outcome == Outcome::Grown {
                month_grown += 1;
            }
        }
        if session.day >= monday {
            days[(session.day - monday).num_days() as usize] += minutes;
        } else if session.day >= last_monday {
            last_week += minutes;
        }
    }

    let minutes = days.iter().sum();
    let change_percent = (last_week > 0).then(|| {
        ((f64::from(minutes) - f64::from(last_week)) / f64::from(last_week) * 100.0).round() as i32
    });

    let mut counts: HashMap<&str, u32> = HashMap::new();
    for slip in slips.iter().filter(|slip| slip.day >= monday && slip.day <= today) {
        *counts.entry(slip.app.as_str()).or_default() += 1;
    }
    let mut tempting: Vec<(String, u32)> = counts
        .into_iter()
        .map(|(app, count)| (app.to_owned(), count))
        .collect();
    // Most first; ties in name order so the list doesn't shuffle.
    tempting.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    tempting.truncate(TEMPTING);

    Week {
        days,
        today: (today - monday).num_days() as usize,
        minutes,
        change_percent,
        month_minutes,
        completion_percent: (month_ended > 0)
            .then(|| (f64::from(month_grown) / f64::from(month_ended) * 100.0).round() as u32),
        tempting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Thursday 15 October 2026.
    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
    }

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    fn grown(day: NaiveDate, minutes: u32) -> EndedSession {
        EndedSession {
            day,
            outcome: Outcome::Grown,
            planned_minutes: minutes,
            focused_seconds: minutes * 60,
        }
    }

    /// A 50-minute session given up after `focused` minutes.
    fn withered(day: NaiveDate, focused: u32) -> EndedSession {
        EndedSession {
            day,
            outcome: Outcome::Withered,
            planned_minutes: 50,
            focused_seconds: focused * 60,
        }
    }

    fn slip(day: NaiveDate, app: &str) -> SlipOn {
        SlipOn {
            day,
            app: app.into(),
        }
    }

    #[test]
    fn weeks_start_on_monday() {
        assert_eq!(week_start(today()), day(12));
        assert_eq!(week_start(day(12)), day(12));
        assert_eq!(week_start(day(18)), day(12));
        assert_eq!(month_start(today()), day(1));
    }

    #[test]
    fn minutes_land_on_their_day() {
        let sessions = [grown(day(12), 45), grown(day(13), 90), grown(day(13), 30), grown(day(15), 25)];
        let week = week(&sessions, &[], today());
        assert_eq!(week.days, [45, 120, 0, 25, 0, 0, 0]);
        assert_eq!(week.minutes, 190);
        assert_eq!(week.today, 3);
    }

    #[test]
    fn withered_sessions_count_the_time_they_ran() {
        let week = week(&[grown(day(14), 50), withered(day(14), 12)], &[], today());
        assert_eq!(week.days[2], 62);
        assert_eq!(week.minutes, 62);
        assert_eq!(week.month_minutes, 62);
    }

    #[test]
    fn withered_time_rounds_down_and_never_passes_the_plan() {
        let mut short = withered(day(14), 0);
        short.focused_seconds = 59;
        assert_eq!(short.minutes(), 0);
        let mut long = withered(day(14), 0);
        // A session's time can run on through its wait to end early.
        long.focused_seconds = 65 * 60;
        assert_eq!(long.minutes(), 50);
    }

    #[test]
    fn withered_time_counts_toward_last_week() {
        let sessions = [withered(day(6), 40), grown(day(13), 50)];
        assert_eq!(week(&sessions, &[], today()).change_percent, Some(25));
    }

    #[test]
    fn change_is_against_last_week() {
        let sessions = [grown(day(6), 100), grown(day(13), 122)];
        assert_eq!(week(&sessions, &[], today()).change_percent, Some(22));
        let worse = [grown(day(6), 100), grown(day(13), 50)];
        assert_eq!(week(&worse, &[], today()).change_percent, Some(-50));
    }

    #[test]
    fn no_change_without_a_last_week() {
        assert_eq!(week(&[grown(day(13), 50)], &[], today()).change_percent, None);
        // Two weeks ago doesn't count as last week.
        assert_eq!(week(&[grown(day(2), 50)], &[], today()).change_percent, None);
    }

    #[test]
    fn month_totals_and_completion() {
        let september = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let sessions = [
            grown(september, 300),
            grown(day(2), 60),
            grown(day(9), 90),
            grown(day(13), 50),
            withered(day(14), 20),
        ];
        let week = week(&sessions, &[], today());
        assert_eq!(week.month_minutes, 220);
        assert_eq!(week.completion_percent, Some(75));
    }

    #[test]
    fn no_completion_rate_before_anything_ends() {
        assert_eq!(week(&[], &[], today()).completion_percent, None);
    }

    #[test]
    fn most_tempting_counts_this_weeks_slips() {
        let slips = [
            slip(day(12), "Steam games"),
            slip(day(13), "Discord"),
            slip(day(13), "Steam games"),
            slip(day(14), "Riot Client"),
            slip(day(14), "Steam games"),
            slip(day(15), "Discord"),
            slip(day(15), "Spotify"),
            // Last week's slip isn't counted.
            slip(day(8), "Spotify"),
        ];
        assert_eq!(
            week(&[], &slips, today()).tempting,
            vec![
                ("Steam games".to_owned(), 3),
                ("Discord".to_owned(), 2),
                ("Riot Client".to_owned(), 1),
            ]
        );
    }
}
