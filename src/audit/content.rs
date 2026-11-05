//! Content checks (need the sources, so only when the audit builds the
//! site): links to drafts and to scheduled/expired pages, and dated
//! collections' pages without a date.

use super::{Finding, Needle, Site};
use crate::content::schedule::Status;

pub fn check(site: &Site, out: &mut Vec<Finding>) {
    if !site.build_mode {
        return;
    }
    for page in site.pages.iter().filter(|p| !p.is_auxiliary()) {
        for link in &page.facts.links {
            let Some((path, _)) = site.internal_target(page, &link.href) else {
                continue;
            };
            if site.page_at(&path).is_some() {
                continue;
            }
            let key = super::page_key(&path);
            let loc = || site.locate(page, &[Needle::Url(&link.href)], Some(link.offset));
            if let Some((_, source)) = site.drafts.iter().find(|(u, _)| super::page_key(u) == key) {
                out.push(
                    Finding::new(
                        "content-link-to-draft",
                        Some(&page.url),
                        format!(
                            "link `{}` points at a draft ({}), which is not published",
                            link.href,
                            crate::build::links::relative_display(&site.root, source)
                        ),
                    )
                    .at(loc()),
                );
            } else if let Some(hidden) = site.hidden.iter().find(|h| super::page_key(&h.url) == key)
            {
                let what = match hidden.status {
                    Status::Future => format!("a scheduled page that goes live on {}", hidden.date),
                    _ => format!("a page that expired after {}", hidden.date),
                };
                out.push(
                    Finding::new(
                        "content-link-to-scheduled",
                        Some(&page.url),
                        format!("link `{}` points at {what}", link.href),
                    )
                    .at(loc()),
                );
            }
        }
        if let Some(item) = &page.item {
            if item.dated && item.date.is_none() && !item.is_index {
                out.push(
                    Finding::new(
                        "content-missing-date",
                        Some(&page.url),
                        format!(
                            "no date in the dated `{}` collection (feeds and sorting need one)",
                            item.collection
                        ),
                    )
                    .at(site.locate_page(page, Some("title"))),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{codes, page, site};
    use super::super::ItemInfo;
    use super::*;
    use crate::content::schedule::HiddenPage;

    #[test]
    fn test_content_checks_need_build_mode() {
        let s = site(&[("a.html", &page("", "<a href=\"/draft\">d</a>"))]);
        assert!(codes(&s, check).is_empty());
    }

    #[test]
    fn test_links_to_drafts_and_scheduled_pages_and_missing_dates() {
        let mut s = site(&[
            (
                "a.html",
                &page("", "<a href=\"/posts/draft\">d</a><a href=\"/posts/later/\">l</a><a href=\"/nowhere\">n</a>"),
            ),
            ("posts/undated.html", &page("", "")),
        ]);
        s.build_mode = true;
        s.drafts = vec![("/posts/draft".into(), "/site/content/posts/draft.md".into())];
        s.hidden = vec![HiddenPage {
            source: "/site/content/posts/later.md".into(),
            url: "/posts/later".into(),
            title: "Later".into(),
            status: Status::Future,
            date: chrono::NaiveDate::from_ymd_opt(2999, 1, 1).unwrap(),
        }];
        s.pages[1].item = Some(ItemInfo {
            collection: "posts".into(),
            word_count: 10,
            dated: true,
            date: None,
            is_index: false,
            description: None,
            lang: "en".into(),
        });
        let mut out = Vec::new();
        check(&s, &mut out);
        let c: Vec<_> = out.iter().map(|f| f.code).collect();
        assert_eq!(
            c,
            [
                "content-link-to-draft",
                "content-link-to-scheduled",
                "content-missing-date"
            ]
        );
        assert!(
            out[0].message.contains("content/posts/draft.md"),
            "{}",
            out[0].message
        );
        assert!(out[1].message.contains("2999-01-01"));
    }
}
