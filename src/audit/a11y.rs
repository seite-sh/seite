//! Static accessibility checks: `<html lang>`, image alt text, link and
//! button names, form labels, a skip link, duplicate ids.

use std::collections::BTreeMap;

use super::{Finding, Needle, Page, Site};

/// Link texts that don't say where the link goes (compared lowercased,
/// without trailing punctuation and arrows).
pub const AMBIGUOUS_LINK_TEXT: &[&str] = &[
    "click here",
    "here",
    "read more",
    "more",
    "learn more",
    "link",
    "this",
    "continue",
    "continue reading",
    "details",
    "more info",
];

pub fn check(site: &Site, out: &mut Vec<Finding>) {
    for page in site.pages.iter().filter(|p| !p.is_auxiliary()) {
        lang(site, page, out);
        images(site, page, out);
        links(site, page, out);
        controls(site, page, out);
        buttons(site, page, out);
        skip_link(site, page, out);
        duplicate_ids(site, page, out);
    }
}

fn lang(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    if page
        .facts
        .lang
        .as_deref()
        .is_none_or(|l| l.trim().is_empty())
    {
        out.push(
            Finding::new(
                "a11y-html-lang",
                Some(&page.url),
                "<html> has no lang attribute (screen readers guess the language)",
            )
            .at(site.locate_layout(page, "<html", Some(page.facts.html_offset))),
        );
    }
}

fn images(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for img in &page.facts.imgs {
        if img.alt.is_none() && !img.role_presentation {
            let src = img.src.as_deref().unwrap_or("");
            out.push(
                Finding::new(
                    "a11y-img-alt",
                    Some(&page.url),
                    format!("<img src=\"{src}\"> has no alt attribute"),
                )
                .at(site.locate(page, &[Needle::Url(src)], Some(img.offset))),
            );
        }
    }
}

/// `text` lowercased without surrounding punctuation, arrows and spaces.
fn bare(text: &str) -> String {
    text.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn is_ambiguous(text: &str) -> bool {
    AMBIGUOUS_LINK_TEXT.contains(&bare(text).as_str())
}

fn links(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for link in &page.facts.links {
        let name = link.accessible_name();
        let loc = || site.locate(page, &[Needle::Url(&link.href)], Some(link.offset));
        if name.trim().is_empty() {
            out.push(
                Finding::new(
                    "a11y-link-empty",
                    Some(&page.url),
                    format!("link to `{}` has no text or aria-label", link.href),
                )
                .at(loc()),
            );
            continue;
        }
        if link.aria_label.is_none() && is_ambiguous(&name) {
            // Fine when the same target is also linked with a clear name
            // (e.g. a card's title link plus its "Read more").
            let described =
                page.facts.links.iter().any(|other| {
                    other.href == link.href && !is_ambiguous(&other.accessible_name())
                });
            if !described {
                out.push(
                    Finding::new(
                        "a11y-link-ambiguous",
                        Some(&page.url),
                        format!(
                            "link text \"{name}\" doesn't say where `{}` goes",
                            link.href
                        ),
                    )
                    .at(site.locate(
                        page,
                        &[Needle::Url(&link.href), Needle::Text(&name)],
                        Some(link.offset),
                    )),
                );
            }
        }
    }
}

fn controls(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for control in &page.facts.controls {
        let by_for = control
            .id
            .as_deref()
            .is_some_and(|id| page.facts.label_for.iter().any(|f| f == id));
        if control.labelled || by_for {
            continue;
        }
        let ident = control
            .id
            .as_deref()
            .map(|id| format!("id=\"{id}\""))
            .or_else(|| control.name.as_deref().map(|n| format!("name=\"{n}\"")))
            .unwrap_or_default();
        out.push(
            Finding::new(
                "a11y-input-label",
                Some(&page.url),
                format!(
                    "<{}{}{}> has no label",
                    control.tag,
                    if control.kind.is_empty() {
                        String::new()
                    } else {
                        format!(" type=\"{}\"", control.kind)
                    },
                    if ident.is_empty() {
                        String::new()
                    } else {
                        format!(" {ident}")
                    }
                ),
            )
            .at(site.locate(page, &[Needle::Text(&ident)], Some(control.offset))),
        );
    }
}

fn buttons(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for button in &page.facts.buttons {
        if button.name.trim().is_empty() {
            out.push(
                Finding::new(
                    "a11y-button-name",
                    Some(&page.url),
                    "<button> has no text or aria-label",
                )
                .at(site.output_loc(page, Some(button.offset))),
            );
        }
    }
}

fn skip_link(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    if !page.facts.has_skip_link() {
        out.push(
            Finding::new(
                "a11y-skip-link",
                Some(&page.url),
                "no skip link to the main content (keyboard users tab through the whole header)",
            )
            .at(site.locate_layout(page, "<body", None)),
        );
    }
}

fn duplicate_ids(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    let mut seen: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (id, offset) in &page.facts.ids {
        seen.entry(id.as_str()).or_default().push(*offset);
    }
    for (id, offsets) in seen.into_iter().filter(|(_, o)| o.len() > 1) {
        let attr = format!("id=\"{id}\"");
        out.push(
            Finding::new(
                "a11y-duplicate-id",
                Some(&page.url),
                format!("id \"{id}\" is used {} times", offsets.len()),
            )
            .at(site.locate(page, &[Needle::Text(&attr)], Some(offsets[1]))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{codes, page, site};
    use super::*;

    #[test]
    fn test_clean_page_has_no_findings() {
        let s = site(&[(
            "a.html",
            &page(
                "",
                "<img src=\"/a.png\" alt=\"\"><a href=\"/x\">The install guide</a><label for=\"q\">Q</label><input id=\"q\"><button>Go</button>",
            ),
        )]);
        assert!(codes(&s, check).is_empty(), "{:?}", codes(&s, check));
    }

    #[test]
    fn test_each_a11y_problem() {
        let s = site(&[(
            "a.html",
            "<html><body><img src=\"/a.png\"><img src=\"/d.png\" role=\"presentation\"><a href=\"/x\"></a><a href=\"/y\">Read more →</a>\
             <a href=\"/z\">Z guide</a><a href=\"/z\">read more</a><a href=\"/w\" aria-label=\"Read the W guide\">Read more</a>\
             <input name=\"q\"><button></button><p id=\"d\"></p><p id=\"d\"></p></body></html>",
        )]);
        assert_eq!(
            codes(&s, check),
            [
                "a11y-button-name",
                "a11y-duplicate-id",
                "a11y-html-lang",
                "a11y-img-alt",
                "a11y-input-label",
                "a11y-link-ambiguous",
                "a11y-link-empty",
                "a11y-skip-link",
            ]
        );
        let mut out = Vec::new();
        links(&s, &s.pages[0], &mut out);
        let ambiguous: Vec<_> = out
            .iter()
            .filter(|f| f.code == "a11y-link-ambiguous")
            .collect();
        assert_eq!(
            ambiguous.len(),
            1,
            "only /y: /z has a clear twin, /w an aria-label"
        );
        assert!(ambiguous[0].message.contains("/y"));
    }

    #[test]
    fn test_is_ambiguous() {
        assert!(is_ambiguous("Read more →"));
        assert!(is_ambiguous("  Click HERE! "));
        assert!(!is_ambiguous("Read more about Rust"));
    }
}
