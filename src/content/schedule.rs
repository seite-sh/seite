//! Scheduled publishing: future-dated content and `expires:`.
//!
//! # Design note
//!
//! - **Opt-in.** Scheduling applies only when `[build] schedule = true`
//!   (written by `seite init` for new sites; absent = off, so existing sites
//!   build exactly as before) and only to collections with `has_date = true`.
//!   `seite upgrade` never turns it on; it shows a one-time notice instead.
//! - **What is scheduled.** A page whose `date` (frontmatter, or the
//!   `YYYY-MM-DD-` filename prefix) is after *today* is "future": it is left
//!   out of the build exactly like a draft (no HTML, `.md`, feeds, sitemap,
//!   search index, llms files, tag pages, listings). It goes live on the
//!   first build on or after its date, which is why the generated GitHub
//!   Actions workflow also runs on a daily cron.
//! - **Expiry.** `expires: YYYY-MM-DD` keeps the page up through that day and
//!   removes it from builds from the next day on (same switch).
//! - **Including future content.** `seite build --future`, `[build] future =
//!   true`, and `seite serve` (which previews drafts too) include future
//!   content. `seite check` resolves links exactly like the build (links from
//!   live pages to hidden pages are `link-to-scheduled` warnings) but still
//!   renders scheduled pages, so a post that will publish on its own is
//!   validated today. Expired pages are never built.
//! - **Today.** Dates are plain dates, so "today" is the calendar date in
//!   `[site] timezone`: an IANA name (`Europe/Berlin`, via chrono-tz, which
//!   tera already compiles in) or a fixed offset (`+02:00`, `-0500`, `UTC`).
//!   Without it, the machine's local date — the same clock `seite new`,
//!   `seite init` and MCP create use to date new files, so a post written
//!   today is never hidden as "future". CI runners run in UTC: set
//!   `timezone` when the cron build must agree with your local date.

use std::str::FromStr;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

use std::path::{Path, PathBuf};

use super::Frontmatter;
use crate::build::links;
use crate::config::{CollectionConfig, ResolvedPaths, SiteConfig};
use crate::diagnostics::Diagnostic;

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

/// The machine's IANA time zone, for `seite init` (and `workspace add`) to
/// write as `[site] timezone`; `None` when it can't be named. See
/// [`timezone_from`].
pub fn detect_system_timezone() -> Option<String> {
    timezone_from(std::env::var("TZ").ok().as_deref(), || {
        iana_time_zone::get_timezone().ok()
    })
}

/// The IANA zone the local clock follows, given `TZ` and the OS setting.
///
/// `TZ` wins when set, with libc semantics: empty means UTC; an IANA name,
/// optionally `:`-prefixed or given as a `.../zoneinfo/<name>` path, is that
/// zone; anything else (a POSIX rule such as `CET-1CEST`, an unknown name)
/// gives `None` rather than the OS zone, which the local clock is then not
/// following. Without `TZ`, the OS setting (via `iana-time-zone`).
pub fn timezone_from(
    tz_env: Option<&str>,
    system: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let valid = |name: &str| {
        chrono_tz::Tz::from_str(name)
            .is_ok()
            .then(|| name.to_string())
    };
    match tz_env {
        Some(tz) if tz.trim().is_empty() => Some("UTC".to_string()),
        Some(tz) => {
            let tz = tz.trim().trim_start_matches(':');
            valid(tz.rsplit_once("zoneinfo/").map_or(tz, |(_, name)| name))
        }
        None => system().and_then(|name| valid(name.trim())),
    }
}

/// Today's date in the site's time zone, or the machine's local date when
/// `[site] timezone` is unset (or invalid). Used both to date new files
/// (`seite new`, `seite init`, MCP create, the serve REPL) and to decide what
/// is scheduled, so the two always agree.
pub fn today(config: &SiteConfig) -> NaiveDate {
    match config.site.timezone.as_deref().and_then(SiteTz::parse) {
        Some(tz) => tz.date_at(Utc::now()),
        None => chrono::Local::now().date_naive(),
    }
}

/// [`today`] at instant `now` for a site with a timezone (UTC otherwise).
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

/// The build's view: whether scheduling is on, today's date, and whether
/// future content is included.
#[derive(Debug, Clone, Copy)]
pub struct Schedule {
    /// `[build] schedule = true`.
    pub enabled: bool,
    pub today: NaiveDate,
    pub include_future: bool,
}

impl Schedule {
    pub fn new(config: &SiteConfig) -> Self {
        Self {
            enabled: config.build.schedule,
            today: today(config),
            include_future: config.build.future,
        }
    }

    /// Whether a page of `collection` (with its date already resolved into
    /// `fm.date`) is built. Always true when scheduling is off or the
    /// collection is not dated.
    pub fn is_published(&self, collection: &CollectionConfig, fm: &Frontmatter) -> bool {
        if !self.enabled || !collection.has_date {
            return true;
        }
        match status(fm.date, fm.expires, self.today) {
            Status::Live => true,
            Status::Future => self.include_future,
            Status::Expired => false,
        }
    }
}

/// A page the build leaves out because of scheduling.
#[derive(Debug, Clone)]
pub struct HiddenPage {
    pub source: PathBuf,
    pub url: String,
    pub title: String,
    pub status: Status,
    /// Go-live date (future) or expiry date (expired).
    pub date: NaiveDate,
}

/// Every non-draft page scheduling hides from a build of `config` (future
/// pages only when `config.build.future` is off), sorted by date.
pub fn hidden_pages(config: &SiteConfig, paths: &ResolvedPaths) -> Vec<HiddenPage> {
    let schedule = Schedule::new(config);
    if !schedule.enabled {
        return Vec::new();
    }
    let mut out = Vec::new();
    for collection in config.collections.iter().filter(|c| c.has_date) {
        let dir = paths.content.join(&collection.directory);
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        {
            let path = entry.path();
            let Ok((fm, _)) = super::parse_content_file(path) else {
                continue;
            };
            if fm.draft {
                continue;
            }
            let date = crate::build::resolve_item_date(&fm, path, collection);
            let (status, when) = match status(date, fm.expires, schedule.today) {
                Status::Future if !schedule.include_future => (Status::Future, date),
                Status::Expired => (Status::Expired, fm.expires),
                _ => continue,
            };
            let Some(when) = when else { continue };
            let rel = path.strip_prefix(&dir).unwrap_or(path);
            let url = crate::build::resolve_item_location(config, collection, path, rel, &fm).url;
            out.push(HiddenPage {
                source: path.to_path_buf(),
                url,
                title: fm.title.clone(),
                status,
                date: when,
            });
        }
    }
    out.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.title.cmp(&b.title)));
    out
}

/// One line for `seite build`/`serve`/`check`: "N scheduled post(s) hidden;
/// next goes live <date> (<title>)", plus expired pages. `None` when
/// nothing is hidden.
pub fn hidden_summary(hidden: &[HiddenPage]) -> Option<String> {
    let future: Vec<&HiddenPage> = hidden
        .iter()
        .filter(|h| h.status == Status::Future)
        .collect();
    let expired = hidden
        .iter()
        .filter(|h| h.status == Status::Expired)
        .count();
    let mut parts = Vec::new();
    if let Some(next) = future.first() {
        parts.push(format!(
            "{} scheduled post{} hidden; next goes live {} ({})",
            future.len(),
            if future.len() == 1 { "" } else { "s" },
            next.date,
            next.title
        ));
    }
    if expired > 0 {
        parts.push(format!(
            "{expired} expired post{} hidden",
            if expired == 1 { "" } else { "s" }
        ));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// [`links::link_diagnostics`], except that broken links whose target is a
/// page hidden by scheduling become `link-to-scheduled` warnings (errors
/// with `strict`) naming the go-live (or expiry) date.
pub fn link_diagnostics(
    check: &links::LinkCheckResult,
    hidden: &[HiddenPage],
    strict: bool,
    root: &Path,
) -> Vec<Diagnostic> {
    let norm = |u: &str| u.trim_end_matches('/').to_string();
    let target = |href: &str| hidden.iter().find(|h| norm(&h.url) == norm(href));
    let mut rest = check.clone();
    rest.broken_links.retain(|l| target(&l.href).is_none());
    let mut out = links::link_diagnostics(&rest, strict);
    let mut scheduled = check.clone();
    scheduled.broken_links.retain(|l| target(&l.href).is_some());
    scheduled.missing_assets.clear();
    for link in links::distinct_links(&scheduled.broken_links) {
        let Some(page) = target(&link.href) else {
            continue;
        };
        let source = links::relative_display(root, &page.source);
        let what = match page.status {
            Status::Future => format!("a scheduled page that goes live on {}", page.date),
            _ => format!("a page that expired after {}", page.date),
        };
        let message = format!("link to `{}` points at {what} ({source})", link.href);
        let mut d = if strict {
            Diagnostic::error("link-to-scheduled", message)
        } else {
            Diagnostic::warning("link-to-scheduled", message)
        }
        .with_file(PathBuf::from(link.file()))
        .with_hint(
            "the link is broken until the target is published: remove it for now, give this page the same or a later `date:`, or build with --future to preview",
        );
        if let Some(line) = link.line {
            d = d.with_line(line);
        }
        out.push(d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn test_timezone_from_tz_env_and_system() {
        let none = || -> Option<String> { panic!("TZ set: the OS zone is not consulted") };
        assert_eq!(
            timezone_from(Some("Asia/Tokyo"), none).as_deref(),
            Some("Asia/Tokyo")
        );
        assert_eq!(
            timezone_from(Some(":America/New_York"), none).as_deref(),
            Some("America/New_York")
        );
        assert_eq!(
            timezone_from(Some("/usr/share/zoneinfo/Europe/Paris"), none).as_deref(),
            Some("Europe/Paris")
        );
        assert_eq!(timezone_from(Some(""), none).as_deref(), Some("UTC"));
        assert_eq!(timezone_from(Some("CET-1CEST"), none), None);
        assert_eq!(timezone_from(Some("Not/AZone"), none), None);
        assert_eq!(
            timezone_from(None, || Some("Europe/Berlin\n".into())).as_deref(),
            Some("Europe/Berlin")
        );
        assert_eq!(timezone_from(None, || Some("Bogus".into())), None);
        assert_eq!(timezone_from(None, || None), None);
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
        let posts = CollectionConfig::preset_posts();
        let pages = CollectionConfig::preset_pages();
        let mut s = Schedule {
            enabled: true,
            today,
            include_future: false,
        };
        assert!(!s.is_published(&posts, &fm));
        assert!(
            s.is_published(&pages, &fm),
            "undated collections are never scheduled"
        );
        s.include_future = true;
        assert!(s.is_published(&posts, &fm));
        fm.expires = Some(d("2026-05-01"));
        assert!(!s.is_published(&posts, &fm));
        s.enabled = false;
        assert!(
            s.is_published(&posts, &fm),
            "off unless [build] schedule = true"
        );
    }
}
