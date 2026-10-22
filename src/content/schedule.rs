//! Scheduled publishing: future-dated content and `expires:`.
//!
//! # Design note
//!
//! - **What is scheduled.** A page whose `date` (frontmatter, or the
//!   `YYYY-MM-DD-` filename prefix in dated collections) is after *today* is
//!   "future": it is left out of the build exactly like a draft (no HTML,
//!   `.md`, feeds, sitemap, search index, llms files, tag pages, listings).
//!   It goes live on the first build on or after its date, which is why the
//!   generated GitHub Actions workflow also runs on a daily cron.
//! - **Expiry.** `expires: YYYY-MM-DD` keeps the page up through that day and
//!   removes it from builds from the next day on.
//! - **Including future content.** `seite build --future`, `[build] future =
//!   true`, and `seite serve` (which previews drafts too) include future
//!   content. `seite check` always renders it, so a scheduled post that will
//!   publish on its own is validated today. Expired pages are never built
//!   (delete `expires:` to bring one back).
//! - **Today.** Dates are plain dates, so "today" is the calendar date in
//!   `[site] timezone`: an IANA name (`Europe/Berlin`, via chrono-tz, which
//!   tera already compiles in) or a fixed offset (`+02:00`, `-0500`, `UTC`).
//!   Without it, UTC — the clock of CI runners, so a local build and the
//!   cron build agree.

use std::str::FromStr;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

use super::Frontmatter;
use crate::config::SiteConfig;

/// A parsed `[site] timezone`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SiteTz {
    Named(chrono_tz::Tz),
    Fixed(FixedOffset),
}

impl SiteTz {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("utc") || s == "Z" {
            return FixedOffset::east_opt(0).map(SiteTz::Fixed);
        }
        if let Some(rest) = s.strip_prefix(['+', '-']) {
            let sign = if s.starts_with('-') { -1 } else { 1 };
            let digits: String = rest.chars().filter(|c| *c != ':').collect();
            if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let hours: i32 = digits[..2].parse().ok()?;
            let minutes: i32 = digits[2..].parse().ok()?;
            if hours > 14 || minutes > 59 {
                return None;
            }
            return FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60)).map(SiteTz::Fixed);
        }
        chrono_tz::Tz::from_str(s).ok().map(SiteTz::Named)
    }

    /// The calendar date at instant `now` in this zone.
    pub fn date_at(self, now: DateTime<Utc>) -> NaiveDate {
        match self {
            SiteTz::Named(tz) => now.with_timezone(&tz).date_naive(),
            SiteTz::Fixed(offset) => now.with_timezone(&offset).date_naive(),
        }
    }
}

/// `Err(message)` when `[site] timezone` is set but not understood.
pub fn validate_timezone(tz: Option<&str>) -> Result<(), String> {
    match tz {
        Some(name) if SiteTz::parse(name).is_none() => Err(format!(
            "[site] timezone = \"{name}\" is not a time zone: use an IANA name such as \"Europe/Berlin\" or \"America/New_York\", or a fixed offset such as \"+02:00\" or \"UTC\""
        )),
        _ => Ok(()),
    }
}

/// Today's date in the site's time zone (UTC when unset or invalid).
pub fn today(config: &SiteConfig) -> NaiveDate {
    today_at(config, Utc::now())
}

pub fn today_at(config: &SiteConfig, now: DateTime<Utc>) -> NaiveDate {
    config
        .site
        .timezone
        .as_deref()
        .and_then(SiteTz::parse)
        .map_or_else(|| now.date_naive(), |tz| tz.date_at(now))
}

/// Where a page stands on `today`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Live,
    /// `date` is after today.
    Future,
    /// `expires` is before today.
    Expired,
}

/// Status of a page with resolved `date` (frontmatter or filename).
pub fn status(date: Option<NaiveDate>, expires: Option<NaiveDate>, today: NaiveDate) -> Status {
    if expires.is_some_and(|e| e < today) {
        Status::Expired
    } else if date.is_some_and(|d| d > today) {
        Status::Future
    } else {
        Status::Live
    }
}

/// The build's view: today's date and whether future content is included.
#[derive(Debug, Clone, Copy)]
pub struct Schedule {
    pub today: NaiveDate,
    pub include_future: bool,
}

impl Schedule {
    pub fn new(config: &SiteConfig) -> Self {
        Self {
            today: today(config),
            include_future: config.build.future,
        }
    }

    /// Whether a page (with its date already resolved into `fm.date`) is
    /// built.
    pub fn is_published(&self, fm: &Frontmatter) -> bool {
        match status(fm.date, fm.expires, self.today) {
            Status::Live => true,
            Status::Future => self.include_future,
            Status::Expired => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn test_parse_timezones() {
        assert!(matches!(
            SiteTz::parse("Europe/Berlin"),
            Some(SiteTz::Named(_))
        ));
        assert!(matches!(SiteTz::parse("+02:00"), Some(SiteTz::Fixed(_))));
        assert!(matches!(SiteTz::parse("-0530"), Some(SiteTz::Fixed(_))));
        assert!(SiteTz::parse("UTC").is_some());
        for bad in ["Mars/Base", "+2", "+25:00", "+02:60", ""] {
            assert!(SiteTz::parse(bad).is_none(), "{bad}");
        }
        assert!(validate_timezone(None).is_ok());
        assert!(validate_timezone(Some("Nope/Zone"))
            .unwrap_err()
            .contains("IANA"));
    }

    #[test]
    fn test_date_at_crosses_midnight() {
        // 23:30 UTC on Jan 1 is already Jan 2 in Berlin (UTC+1) and still Jan 1 in New York.
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 23, 30, 0).unwrap();
        assert_eq!(
            SiteTz::parse("Europe/Berlin").unwrap().date_at(now),
            d("2026-01-02")
        );
        assert_eq!(
            SiteTz::parse("America/New_York").unwrap().date_at(now),
            d("2026-01-01")
        );
        assert_eq!(
            SiteTz::parse("+01:00").unwrap().date_at(now),
            d("2026-01-02")
        );
    }

    #[test]
    fn test_status() {
        let today = d("2026-05-10");
        assert_eq!(status(Some(d("2026-05-10")), None, today), Status::Live);
        assert_eq!(status(Some(d("2026-05-11")), None, today), Status::Future);
        assert_eq!(status(None, Some(d("2026-05-10")), today), Status::Live);
        assert_eq!(status(None, Some(d("2026-05-09")), today), Status::Expired);
        // An expired page is expired even if it was scheduled.
        assert_eq!(
            status(Some(d("2026-06-01")), Some(d("2026-05-01")), today),
            Status::Expired
        );
        let mut fm = Frontmatter {
            date: Some(d("2026-05-11")),
            ..Default::default()
        };
        let mut s = Schedule {
            today,
            include_future: false,
        };
        assert!(!s.is_published(&fm));
        s.include_future = true;
        assert!(s.is_published(&fm));
        fm.expires = Some(d("2026-05-01"));
        assert!(!s.is_published(&fm));
    }
}
