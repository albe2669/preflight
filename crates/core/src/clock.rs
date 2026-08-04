//! The logical clock — the only place date logic lives.
//!
//! A "logical day" starts at a configurable hour (e.g. 04:00) in a configured
//! timezone, not at midnight UTC. The date is computed once, here, in Rust,
//! and stored as a plain `DATE` column downstream. Nothing else in the codebase
//! does date arithmetic: queries scan `logical_date` directly.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Tz;

/// The timestamp type SeaORM uses for `DateTimeWithTimeZone` columns.
/// `Utc::now()` is `DateTime<Utc>`; columns want `DateTime<FixedOffset>`.
pub fn now_tz() -> chrono::DateTime<chrono::FixedOffset> {
    Utc::now().into()
}

/// A clock that maps an instant to a logical date.
///
/// `day_start_hour` is in [0, 23]. With `day_start_hour = 4`, work done at 02:00
/// local counts as *yesterday* — late-night effort stays off the morning plan.
#[derive(Debug, Clone)]
pub struct Clock {
    pub tz: Tz,
    pub day_start_hour: i64,
}

impl Clock {
    pub fn new(tz: Tz, day_start_hour: i64) -> Self {
        Self {
            tz,
            day_start_hour: day_start_hour.clamp(0, 23),
        }
    }

    /// The logical date for an instant.
    ///
    /// Convert to the configured zone, subtract `day_start_hour` hours so the
    /// boundary rolls back into the previous calendar day, then take the date.
    /// 02:00 local with day_start_hour=4 -> (02:00 - 4h) = 22:00 the prior day.
    pub fn logical_date(&self, at: DateTime<Utc>) -> NaiveDate {
        (at.with_timezone(&self.tz) - Duration::hours(self.day_start_hour)).date_naive()
    }

    /// The logical date for *now*.
    pub fn now_logical(&self) -> NaiveDate {
        self.logical_date(Utc::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn la() -> Clock {
        Clock::new(chrono_tz::America::Los_Angeles, 4)
    }

    #[test]
    fn late_night_is_yesterday() {
        // 2026-08-04 02:00 PT (09:00 UTC) with day_start_hour=4 -> 2026-08-03.
        let at = Tz::America__Los_Angeles
            .with_ymd_and_hms(2026, 8, 4, 2, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            la().logical_date(at),
            NaiveDate::from_ymd_opt(2026, 8, 3).unwrap()
        );
    }

    #[test]
    fn after_boundary_is_today() {
        // 2026-08-04 10:00 PT (17:00 UTC) -> 2026-08-04.
        let at = Tz::America__Los_Angeles
            .with_ymd_and_hms(2026, 8, 4, 10, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            la().logical_date(at),
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap()
        );
    }

    #[test]
    fn exactly_at_boundary_is_today() {
        // 2026-08-04 04:00 PT exactly -> the new day begins.
        let at = Tz::America__Los_Angeles
            .with_ymd_and_hms(2026, 8, 4, 4, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            la().logical_date(at),
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap()
        );
    }
}
