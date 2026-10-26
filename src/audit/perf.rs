//! Static performance checks (no network): image file size, total page
//! weight, render-blocking scripts in `<head>`, lazy loading.
//!
//! Thresholds: an image over 300 KB; a page whose HTML plus local
//! stylesheets, scripts and images exceeds 1.5 MB. For an `<img>` with a
//! `srcset`, the smallest candidate counts (browsers on small screens fetch
//! it); otherwise its `src`. Remote assets are not fetched or counted.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use super::{Finding, Needle, Page, Site};

pub const IMAGE_MAX_BYTES: u64 = 300 * 1024;
pub const PAGE_MAX_BYTES: u64 = 1536 * 1024;

pub fn check(site: &Site, out: &mut Vec<Finding>) {
    let mut reported: HashSet<PathBuf> = HashSet::new();
    for page in site.pages.iter().filter(|p| !p.is_auxiliary()) {
        images_and_weight(site, page, &mut reported, out);
        render_blocking(site, page, out);
        lazy(site, page, out);
    }
}

/// The output file an internal asset URL on `page` refers to.
fn asset_file(site: &Site, page: &Page, url: &str) -> Option<(PathBuf, u64)> {
    let (path, _) = site.internal_target(page, url)?;
    let decoded = urlencoding::decode(&path)
        .map(|d| d.into_owned())
        .unwrap_or(path);
    let file = site.output.join(decoded.trim_start_matches('/'));
    let len = std::fs::metadata(&file).ok().filter(|m| m.is_file())?.len();
    Some((file, len))
}

fn images_and_weight(
    site: &Site,
    page: &Page,
    reported: &mut HashSet<PathBuf>,
    out: &mut Vec<Finding>,
) {
    let mut assets: BTreeSet<(PathBuf, u64)> = BTreeSet::new();
    for img in &page.facts.imgs {
        let candidates: Vec<&str> = match img.srcset.as_deref() {
            Some(srcset) => crate::build::links::parse_srcset(srcset),
            None => img.src.as_deref().into_iter().collect(),
        };
        let smallest = candidates
            .iter()
            .filter_map(|u| asset_file(site, page, u).map(|f| (f, *u)))
            .min_by_key(|((_, len), _)| *len);
        let Some(((file, len), url)) = smallest else {
            continue;
        };
        if len > IMAGE_MAX_BYTES && reported.insert(file.clone()) {
            let src = img.src.as_deref().unwrap_or(url);
            out.push(
                Finding::new(
                    "perf-image-large",
                    Some(&page.url),
                    format!(
                        "image `{url}` is {} KB (over {} KB)",
                        len / 1024,
                        IMAGE_MAX_BYTES / 1024
                    ),
                )
                .at(site.locate(
                    page,
                    &[Needle::Url(src), Needle::Url(url)],
                    Some(img.offset),
                )),
            );
        }
        assets.insert((file, len));
    }
    let scripts = page.facts.scripts.iter().filter_map(|s| s.src.as_deref());
    for url in page
        .facts
        .stylesheets
        .iter()
        .map(String::as_str)
        .chain(scripts)
    {
        if let Some(asset) = asset_file(site, page, url) {
            assets.insert(asset);
        }
    }
    let total = page.bytes + assets.iter().map(|(_, len)| len).sum::<u64>();
    if total > PAGE_MAX_BYTES {
        out.push(
            Finding::new(
                "perf-page-weight",
                Some(&page.url),
                format!(
                    "page weighs {} KB with its local CSS, JS and images (over {} KB)",
                    total / 1024,
                    PAGE_MAX_BYTES / 1024
                ),
            )
            .at(site.locate_page(page, None)),
        );
    }
}

fn render_blocking(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    for script in &page.facts.scripts {
        let Some(src) = script.src.as_deref() else {
            continue;
        };
        let module = script
            .kind
            .as_deref()
            .is_some_and(|k| k.trim().eq_ignore_ascii_case("module"));
        if script.in_head && !script.is_async && !script.defer && !module {
            out.push(
                Finding::new(
                    "perf-render-blocking-script",
                    Some(&page.url),
                    format!("<script src=\"{src}\"> in <head> blocks rendering"),
                )
                .at(site.locate(page, &[Needle::Url(src)], Some(script.offset))),
            );
        }
    }
}

/// Images in `<main>` (the whole page without one) after the first: header
/// logos and the lead image are above the fold.
fn lazy(site: &Site, page: &Page, out: &mut Vec<Finding>) {
    let has_main = page.facts.has_main;
    for img in page
        .facts
        .imgs
        .iter()
        .filter(|i| !has_main || i.in_main)
        .skip(1)
    {
        let Some(src) = img.src.as_deref() else {
            continue;
        };
        if src.starts_with("data:") {
            continue;
        }
        if img
            .loading
            .as_deref()
            .is_none_or(|l| !l.eq_ignore_ascii_case("lazy"))
        {
            out.push(
                Finding::new(
                    "perf-img-lazy",
                    Some(&page.url),
                    format!("<img src=\"{src}\"> below the first image loads eagerly"),
                )
                .at(site.locate(page, &[Needle::Url(src)], Some(img.offset))),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{codes, page, site};
    use super::*;

    #[test]
    fn test_perf_checks() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("big.png"), vec![0u8; 400 * 1024]).unwrap();
        std::fs::write(tmp.path().join("big-480w.png"), vec![0u8; 100 * 1024]).unwrap();
        std::fs::write(tmp.path().join("huge.js"), vec![b' '; 1600 * 1024]).unwrap();
        let mut s = site(&[
            (
                "a.html",
                &page(
                    "<script src=\"/huge.js\"></script><script src=\"/ok.js\" defer></script><script type=\"module\" src=\"/m.js\"></script>",
                    "<img src=\"/big.png\" alt=\"\"><img src=\"/big.png\" srcset=\"/big-480w.png 480w, /big.png 900w\" alt=\"\" loading=\"lazy\"><img src=\"/x.png\" alt=\"\">",
                ),
            ),
            ("b.html", &page("", "<img src=\"/big.png\" alt=\"\">")),
        ]);
        s.output = tmp.path().to_path_buf();
        let c = codes(&s, check);
        assert_eq!(
            c,
            [
                "perf-image-large",
                "perf-img-lazy",
                "perf-page-weight",
                "perf-render-blocking-script",
            ],
            "the big image is reported once; the srcset image uses its small candidate"
        );
    }
}
