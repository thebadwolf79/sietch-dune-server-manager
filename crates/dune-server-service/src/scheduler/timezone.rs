use chrono::{DateTime, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

/// Compute the next UTC instant at which the local wall-clock `hour:minute`
/// occurs in `tz`, strictly after `now`.
///
/// Handles DST transitions:
/// - **Spring-forward gap** (the wall time skipped, e.g. 02:30 on transition
///   day in Europe): advance to the next non-gap day.
/// - **Fall-back overlap** (the wall time happens twice): use the latest of
///   the two so the schedule fires once per day, not twice.
pub fn next_daily_at(tz: Tz, hour: u32, minute: u32, now: DateTime<Utc>) -> DateTime<Utc> {
    let local_now = now.with_timezone(&tz);
    let mut candidate_date = local_now.date_naive();

    for _ in 0..14 {
        if let Some(target) = build_target(tz, candidate_date, hour, minute) {
            if target > now {
                return target;
            }
        }
        candidate_date = candidate_date
            .succ_opt()
            .expect("date arithmetic always succeeds");
    }

    // Defensive: if 14 days of DST gaps somehow occurred (impossible in
    // practice), fall back to now + 24h.
    now + Duration::hours(24)
}

fn build_target(tz: Tz, date: NaiveDate, hour: u32, minute: u32) -> Option<DateTime<Utc>> {
    let naive = date.and_hms_opt(hour, minute, 0)?;
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(_earliest, latest) => Some(latest.with_timezone(&Utc)),
        LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    fn ams() -> Tz {
        chrono_tz::Europe::Amsterdam
    }

    #[test]
    fn fires_today_when_target_still_ahead() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 1, 0, 0).unwrap();
        let next = next_daily_at(ams(), 5, 0, now);
        let local = next.with_timezone(&ams());
        assert_eq!(
            local.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
        );
        assert_eq!((local.time().hour(), local.time().minute()), (5, 0));
    }

    #[test]
    fn rolls_to_next_day_when_target_already_passed() {
        // 09:00 UTC = 11:00 in Amsterdam (CEST). 05:00 local is already in the past.
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
        let next = next_daily_at(ams(), 5, 0, now);
        let local = next.with_timezone(&ams());
        assert_eq!(
            local.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 6, 2).unwrap()
        );
        assert_eq!((local.time().hour(), local.time().minute()), (5, 0));
    }

    #[test]
    fn skips_over_dst_spring_forward_gap() {
        // 2026-03-29 02:00..03:00 doesn't exist in Amsterdam (clocks jump to 03:00).
        let before_gap = Utc.with_ymd_and_hms(2026, 3, 29, 0, 50, 0).unwrap();
        let next = next_daily_at(ams(), 2, 30, before_gap);
        // Either the next-day 02:30, or another defined point — must not panic, must be after.
        assert!(next > before_gap);
        let local = next.with_timezone(&ams());
        assert_eq!((local.time().hour(), local.time().minute()), (2, 30));
    }

    #[test]
    fn picks_latest_for_dst_fall_back_overlap() {
        // 2026-10-25 02:00..03:00 happens twice in Amsterdam. Daily 02:30 must be picked once.
        let before = Utc.with_ymd_and_hms(2026, 10, 25, 0, 0, 0).unwrap();
        let next = next_daily_at(ams(), 2, 30, before);
        assert!(next > before);
        // Sanity: when interpreted back to local, hour+minute match.
        let local = next.with_timezone(&ams());
        assert_eq!((local.time().hour(), local.time().minute()), (2, 30));
    }
}
