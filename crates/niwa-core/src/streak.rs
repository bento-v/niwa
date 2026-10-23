//! The daily streak. A day counts when at least one session finished on it.

use chrono::{DateTime, Local, NaiveDate};

/// The local calendar day a Unix timestamp falls on.
pub fn local_day(unix_seconds: i64) -> NaiveDate {
    DateTime::from_timestamp(unix_seconds, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
        .date_naive()
}

/// Counts consecutive days with a finished session, ending today.
///
/// If nothing has finished yet today, the streak through yesterday is still
/// shown: it isn't lost until the day ends.
pub fn streak(finished_days: &[NaiveDate], today: NaiveDate) -> u32 {
    let finished = |day: NaiveDate| finished_days.contains(&day);
    let mut day = match (finished(today), today.pred_opt()) {
        (true, _) => today,
        (false, Some(yesterday)) => yesterday,
        (false, None) => return 0,
    };
    let mut count = 0;
    while finished(day) {
        count += 1;
        match day.pred_opt() {
            Some(previous) => day = previous,
            None => break,
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    #[test]
    fn no_sessions_is_no_streak() {
        assert_eq!(streak(&[], day(10)), 0);
    }

    #[test]
    fn counts_back_from_today() {
        assert_eq!(streak(&[day(8), day(9), day(10)], day(10)), 3);
    }

    #[test]
    fn several_sessions_in_a_day_count_once() {
        assert_eq!(streak(&[day(10), day(10), day(9)], day(10)), 2);
    }

    #[test]
    fn a_gap_ends_the_streak() {
        assert_eq!(streak(&[day(5), day(6), day(8), day(9), day(10)], day(10)), 3);
    }

    #[test]
    fn yesterdays_streak_survives_until_today_ends() {
        assert_eq!(streak(&[day(8), day(9)], day(10)), 2);
    }

    #[test]
    fn missing_yesterday_and_today_resets() {
        assert_eq!(streak(&[day(7), day(8)], day(10)), 0);
    }

    #[test]
    fn crosses_month_boundaries() {
        let september_30 = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        assert_eq!(streak(&[september_30, day(1)], day(1)), 2);
    }
}
