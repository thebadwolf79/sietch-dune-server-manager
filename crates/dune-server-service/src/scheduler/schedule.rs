use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule as CronSchedule;

use super::timezone;

#[derive(Debug, Clone)]
pub enum Schedule {
    /// Fire every `every` from the moment the scheduler starts.
    Interval { every: Duration },
    /// Fire daily at `hour:minute` in the configured IANA timezone.
    Daily { hour: u32, minute: u32 },
    /// Fire on the cadence described by a (5-, 6-, or 7-field) cron
    /// expression, evaluated in the operator's TZ.
    Cron(Box<CronSchedule>),
    /// Fire `lead_time` before each fire of `inner`. Used by the restart-notice
    /// task so the pre-restart warning tracks the real restart cadence (daily
    /// or cron) without duplicating its schedule logic.
    RestartNotice {
        inner: Box<Schedule>,
        lead_time: Duration,
    },
    /// Never fire automatically. Manual triggers still work.
    Disabled,
}

/// Parses a user-supplied cron expression. Only the standard 5-field form
/// (`min hour dom mon dow`) is accepted; the underlying parser wants 6 fields
/// (seconds first) so we prepend `0` seconds before handing it off. Anything
/// other than exactly 5 fields is rejected with a clear error.
pub fn parse_cron(expr: &str) -> Result<CronSchedule> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty cron expression"));
    }
    let field_count = trimmed.split_whitespace().count();
    if field_count != 5 {
        return Err(anyhow!(
            "cron must have exactly 5 fields (min hour dom mon dow); got {field_count}"
        ));
    }
    let normalized = format!("0 {trimmed}");
    CronSchedule::from_str(&normalized).map_err(|err| anyhow!("invalid cron expression: {err}"))
}

impl Schedule {
    pub fn interval_secs(secs: u64) -> Self {
        if secs == 0 {
            Self::Disabled
        } else {
            Self::Interval {
                every: Duration::from_secs(secs),
            }
        }
    }

    pub fn daily(hour: u32, minute: u32) -> Self {
        Self::Daily { hour, minute }
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn next_fire(&self, tz: Tz, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Interval { every } => {
                now + chrono::Duration::from_std(*every).expect("interval fits in chrono::Duration")
            }
            Self::Daily { hour, minute } => timezone::next_daily_at(tz, *hour, *minute, now),
            Self::Cron(schedule) => {
                let now_tz = now.with_timezone(&tz);
                schedule
                    .after(&now_tz)
                    .next()
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|| now + chrono::Duration::days(365 * 100))
            }
            Self::RestartNotice { inner, lead_time } => {
                // Find the first inner fire that is at least `lead_time` away,
                // then back off by `lead_time`. Anchoring the search at
                // `now + lead` keeps the result strictly in the future even
                // when `now` already sits inside the warning window of the
                // upcoming restart (we simply warn for the following one).
                let lead = chrono::Duration::from_std(*lead_time)
                    .unwrap_or_else(|_| chrono::Duration::days(365 * 100));
                inner.next_fire(tz, now + lead) - lead
            }
            // Sentinel "very far future" so the loop sleeps until cancellation
            // even if a caller forgets to check `is_disabled`.
            Self::Disabled => now + chrono::Duration::days(365 * 100),
        }
    }

    pub fn describe(&self, tz: Tz) -> String {
        match self {
            Self::Interval { every } => format!("every {}s", every.as_secs()),
            Self::Daily { hour, minute } => {
                format!("daily {:02}:{:02} {}", hour, minute, tz.name())
            }
            Self::Cron(schedule) => format!("cron `{schedule}` {}", tz.name()),
            Self::RestartNotice { inner, lead_time } => {
                format!("{}s before [{}]", lead_time.as_secs(), inner.describe(tz))
            }
            Self::Disabled => "disabled (manual-only)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cron_accepts_5_fields() {
        let s = parse_cron("0 4 * * *").expect("parse");
        // Sanity: should produce a valid CronSchedule
        let next: Vec<_> = s.upcoming(chrono::Utc).take(2).collect();
        assert_eq!(next.len(), 2);
    }

    #[test]
    fn parse_cron_rejects_bad_field_count() {
        assert!(parse_cron("1 2 3 4").is_err());
        assert!(parse_cron("").is_err());
        assert!(parse_cron("not cron at all").is_err());
        // 6-field with seconds is also rejected; we keep the surface 5-only.
        assert!(parse_cron("0 0 4 * * *").is_err());
        // 7-field with year is also rejected.
        assert!(parse_cron("0 0 4 * * * 2026").is_err());
    }

    #[test]
    fn parse_cron_rejects_invalid_field() {
        assert!(parse_cron("99 4 * * *").is_err());
    }

    #[test]
    fn restart_notice_fires_lead_time_before_inner() {
        use chrono::TimeZone;
        let tz = chrono_tz::Europe::Amsterdam;
        let notice = Schedule::RestartNotice {
            inner: Box::new(Schedule::daily(5, 0)),
            lead_time: Duration::from_secs(1800),
        };
        // 01:00 UTC = 03:00 CEST; restart is 05:00 local, notice fires 04:30.
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 1, 0, 0).unwrap();
        let next = notice.next_fire(tz, now);
        let restart = Schedule::daily(5, 0).next_fire(tz, now);
        assert_eq!(next, restart - chrono::Duration::seconds(1800));
        assert!(next > now);
    }

    #[test]
    fn restart_notice_skips_to_next_when_inside_warning_window() {
        use chrono::TimeZone;
        let tz = chrono_tz::Europe::Amsterdam;
        let notice = Schedule::RestartNotice {
            inner: Box::new(Schedule::daily(5, 0)),
            lead_time: Duration::from_secs(1800),
        };
        // 02:45 UTC = 04:45 CEST: past today's 04:30 notice but before 05:00.
        // Must roll to tomorrow's 04:30, never produce a past time.
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 2, 45, 0).unwrap();
        let next = notice.next_fire(tz, now);
        assert!(next > now);
        let local = next.with_timezone(&tz);
        use chrono::Timelike;
        assert_eq!((local.time().hour(), local.time().minute()), (4, 30));
    }
}
