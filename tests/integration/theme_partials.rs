//! Bundled themes built on seite's `seite/*` partials: rendered output
//! unchanged from the pre-partial themes, overrides, the missing-head
//! diagnostic, the upgrade migration, starters vs gallery, and `theme new`.

use crate::common::*;
use std::collections::BTreeMap;
use std::path::Path;

/// The ten themes as they were before the partials (0.20.x), kept as fixtures.
const PRE_PARTIAL_THEMES: &[&str] = &[
    "default",
    "minimal",
    "dark",
    "docs",
    "brutalist",
    "bento",
    "landing",
    "terminal",
    "magazine",
    "academic",
];

fn pre_partial(name: &str) -> String {
    fs::read_to_string(format!(
        "{}/tests/fixtures/themes-pre-partials/{name}.tera",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// A site with posts, docs and pages, a Spanish translation (language
/// switcher, hreflang), a doc with a table of contents and a page with an
/// image (og:image).
fn full_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Partials", "posts,docs,pages");
    let site = tmp.path().join("site");
    add_language(&site, "es", "Parciales");
    let post = fs::read_dir(site.join("content/posts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let es = post.to_string_lossy().replace(".md", ".es.md");
    fs::copy(&post, es).unwrap();
    fs::write(
        site.join("content/docs/intro.md"),
        "---\ntitle: Intro\ndescription: Start here\n---\n\n## One\n\ntext\n\n## Two\n\nmore\n",
    )
    .unwrap();
    fs::write(
        site.join("content/pages/about.md"),
        "---\ntitle: About\ndescription: About us\nimage: /static/card.png\n---\n\nabout\n",
    )
    .unwrap();
    fs::create_dir_all(site.join("static")).unwrap();
    fs::write(site.join("static/card.png"), b"png").unwrap();
    site
}

fn build_html(site: &Path) -> BTreeMap<String, String> {
    page_cmd().arg("build").current_dir(site).assert().success();
    let dist = site.join("dist");
    let mut pages = BTreeMap::new();
    for entry in walkdir::WalkDir::new(&dist) {
        let entry = entry.unwrap();
        if entry.path().extension().is_some_and(|e| e == "html") {
            let rel = entry.path().strip_prefix(&dist).unwrap();
            pages.insert(
                rel.to_string_lossy().into_owned(),
                fs::read_to_string(entry.path()).unwrap(),
            );
        }
    }
    pages
}

/// Replace the inline search script (the one that loads search-index.json)
/// with a marker: the shared partial replaced every theme's copy (landing and
/// terminal had an `innerHTML` variant) and later became the full-text one.
fn without_search_script(html: &str) -> String {
    let Some(index) = html
        .find("var index = null;")
        .or_else(|| html.find("getElementById('search-input')"))
    else {
        return html.to_string();
    };
    let start = html[..index].rfind("<script>").unwrap();
    let end = index + html[index..].find("</script>").unwrap() + "</script>".len();
    format!("{}<script>SEARCH</script>{}", &html[..start], &html[end..])
}

/// docs used `<div class="lang-switcher">`; the partial is a `<nav>` like
/// the other nine themes.
fn docs_switcher_as_nav(html: &str) -> String {
    let Some(start) = html.find(r#"<div class="lang-switcher">"#) else {
        return html.to_string();
    };
    let close = start + html[start..].find("</div>").unwrap();
    format!(
        "{}<nav class=\"lang-switcher\">{}</nav>{}",
        &html[..start],
        &html[start + r#"<div class="lang-switcher">"#.len()..close],
        &html[close + "</div>".len()..]
    )
}

fn without_blank_lines(html: &str) -> String {
    html.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn trimmed_lines(html: &str) -> String {
    html.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// How closely each theme's output must match its pre-partial version.
/// - exact: byte-identical.
/// - blank lines: identical except one whitespace-only line (the empty
///   `{% if math_enabled %}` the theme no longer carries; KaTeX moved into
///   `seite/head.html`).
/// - indentation: the partials' inner lines have the default theme's
///   indentation, so themes that nest the search box or switcher deeper differ
///   in leading whitespace only.
///
/// For every theme, the search script is the shared full-text one
/// (`seite/search-script.html`), compared as a marker, and the theme CSS has
/// one added line styling result snippets, highlights and the active result.
fn normalize(theme: &str, html: &str) -> String {
    let html: String = without_search_script(html)
        .split_inclusive('\n')
        .filter(|l| !l.contains("#search-results .result-snippet"))
        .collect();
    let html = &html;
    match theme {
        "academic" => html.to_string(),
        "default" | "minimal" | "dark" | "brutalist" => without_blank_lines(html),
        "docs" => trimmed_lines(&docs_switcher_as_nav(html)),
        _ => trimmed_lines(html),
    }
}

#[test]
fn test_partial_themes_render_like_the_pre_partial_themes() {
    let tmp = TempDir::new().unwrap();
    let site = full_site(&tmp);
    for theme in PRE_PARTIAL_THEMES {
        fs::write(site.join("templates/base.html"), pre_partial(theme)).unwrap();
        let before = build_html(&site);
        page_cmd()
            .args(["theme", "apply", theme])
            .current_dir(&site)
            .assert()
            .success();
        let after = build_html(&site);
        assert_eq!(
            before.keys().collect::<Vec<_>>(),
            after.keys().collect::<Vec<_>>(),
            "{theme}"
        );
        assert!(before.len() >= 10, "{theme}: {}", before.len());
        for (page, old) in &before {
            let new = &after[page];
            assert_eq!(
                normalize(theme, old),
                normalize(theme, new),
                "{theme}: {page} differs beyond the documented changes"
            );
        }
        // The pages that matter for SEO really carry the head.
        let about = after
            .iter()
            .find(|(k, _)| k.contains("about"))
            .map(|(_, v)| v)
            .unwrap();
        assert!(
            about.contains(
                r#"<meta property="og:image" content="http://localhost:3000/static/card.png">"#
            ),
            "{theme}"
        );
        assert!(
            about.contains(r#"<link rel="canonical" href="http://localhost:3000/about">"#),
            "{theme}"
        );
        let es_post = after.keys().find(|k| k.starts_with("es/posts/")).unwrap();
        assert!(
            after[es_post].contains(r#"class="lang-switcher""#),
            "{theme}"
        );
        assert!(
            after[es_post].contains(r#"hreflang="x-default""#),
            "{theme}"
        );
    }
}

#[test]
fn test_every_theme_gets_katex_from_the_head_partial() {
    let tmp = TempDir::new().unwrap();
    let site = full_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replacen("[build]\n", "[build]\nmath = true\n", 1),
    )
    .unwrap();
    for theme in seite::themes::all() {
        page_cmd()
            .args(["theme", "apply", theme.name])
            .current_dir(&site)
            .assert()
            .success();
        let pages = build_html(&site);
        let doc = &pages["docs/intro.html"];
        let head = &doc[..doc.find("</head>").unwrap()];
        assert_eq!(head.matches("katex.min.css").count(), 1, "{}", theme.name);
    }
}

#[test]
fn test_user_partial_overrides_the_embedded_one() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Override", "posts");
    let site = tmp.path().join("site");
    fs::create_dir_all(site.join("templates/seite")).unwrap();
    fs::write(
        site.join("templates/seite/head.html"),
        r#"<meta name="generator" content="my-own-head">"#,
    )
    .unwrap();
    let pages = build_html(&site);
    let index = &pages["index.html"];
    assert!(index.contains("my-own-head"), "{index}");
    assert!(!index.contains("og:title"), "{index}");
    // Search and scripts still come from the embedded partials.
    assert!(index.contains(r#"id="search-input""#));
    assert!(index.contains("search-index.json"));
}

fn diagnostics(site: &Path, args: &[&str]) -> (bool, Vec<serde_json::Value>) {
    let (ok, doc) = run_json(site, args);
    let list = doc["data"]["diagnostics"]
        .as_array()
        .or_else(|| doc["error"]["diagnostics"].as_array())
        .cloned()
        .unwrap_or_default();
    (ok, list)
}

#[test]
fn test_missing_seite_head_warning_legacy_and_missing() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Head", "posts");
    let site = tmp.path().join("site");

    // Fresh site: no warning, strict check passes.
    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&site)
        .assert()
        .success();

    // A pre-partial theme: legacy message, warning only.
    fs::write(site.join("templates/base.html"), pre_partial("dark")).unwrap();
    let (ok, found) = diagnostics(&site, &["check"]);
    assert!(ok, "a warning must not fail a non-strict check");
    let head: Vec<_> = found
        .iter()
        .filter(|d| d["code"] == "template-missing-seite-head")
        .collect();
    assert_eq!(head.len(), 1, "{found:?}");
    assert_eq!(head[0]["severity"], "warning");
    assert!(head[0]["message"]
        .as_str()
        .unwrap()
        .starts_with("legacy theme"));
    assert!(head[0]["hint"].as_str().unwrap().contains("seite upgrade"));
    assert_eq!(head[0]["file"], "templates/base.html");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&site)
        .assert()
        .failure();

    // A redesign that dropped the head entirely.
    fs::write(
        site.join("templates/base.html"),
        "<!DOCTYPE html>\n<html lang=\"{{ lang }}\">\n<head>\n<title>{% block title %}{% endblock %}</title>\n</head>\n<body><main id=\"main\">{% block content %}{% endblock %}</main></body>\n</html>\n",
    )
    .unwrap();
    let (_, found) = diagnostics(&site, &["check"]);
    let head: Vec<_> = found
        .iter()
        .filter(|d| d["code"] == "template-missing-seite-head")
        .collect();
    assert_eq!(head.len(), 1, "{found:?}");
    assert!(head[0]["message"]
        .as_str()
        .unwrap()
        .starts_with("missing SEO head"));
}

#[test]
fn test_upgrade_replaces_unedited_pre_partial_theme_and_keeps_edited() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade", "posts");
    let site = tmp.path().join("site");
    let legacy = include_str!("../fixtures/legacy-themes/dark-0ef6f5f.tera");
    fs::write(site.join("templates/base.html"), legacy).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("templates/base.html"));
    assert_eq!(
        fs::read_to_string(site.join("templates/base.html")).unwrap(),
        seite::themes::dark().base_html
    );
    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&site)
        .assert()
        .success();

    // An edited copy (here: the 0.20 landing theme with a tweak) is kept.
    let edited = format!("{}\n<!-- my tweak -->\n", pre_partial("landing"));
    fs::write(site.join("templates/base.html"), &edited).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Kept templates/base.html"))
        .stdout(predicate::str::contains(
            r#"{% include "seite/head.html" %}"#,
        ));
    assert_eq!(
        fs::read_to_string(site.join("templates/base.html")).unwrap(),
        edited
    );
}

#[test]
fn test_theme_list_groups_starters_and_gallery() {
    let tmp = TempDir::new().unwrap();
    let (ok, doc) = run_json(tmp.path(), &["theme", "list"]);
    assert!(ok);
    let names = |key: &str| -> Vec<String> {
        doc["data"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names("starters"), ["default", "docs", "landing", "blank"]);
    assert_eq!(
        names("gallery"),
        [
            "minimal",
            "dark",
            "brutalist",
            "bento",
            "terminal",
            "magazine",
            "academic"
        ]
    );
    assert_eq!(names("bundled").len(), 11);
    page_cmd()
        .args(["theme", "list"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Starters"))
        .stdout(predicate::str::contains("Gallery"))
        .stdout(predicate::str::contains("seite theme new"));
}

fn init_with(
    tmp: &TempDir,
    name: &str,
    collections: &str,
    theme: Option<&str>,
) -> assert_cmd::assert::Assert {
    let mut cmd = page_cmd();
    cmd.args([
        "init",
        name,
        "--title",
        "T",
        "--description",
        "",
        "--deploy-target",
        "github-pages",
        "--collections",
        collections,
        "--agents",
        "claude",
    ]);
    if let Some(theme) = theme {
        cmd.args(["--theme", theme]);
    }
    cmd.current_dir(tmp.path()).assert()
}

#[test]
fn test_init_offers_starters_only() {
    let tmp = TempDir::new().unwrap();
    let base =
        |site: &str| fs::read_to_string(tmp.path().join(site).join("templates/base.html")).unwrap();

    init_with(&tmp, "blog", "posts,pages", None)
        .success()
        .stdout(predicate::str::contains("seite theme new"));
    assert_eq!(base("blog"), seite::themes::default().base_html);

    init_with(&tmp, "manual", "docs", None).success();
    assert_eq!(base("manual"), seite::themes::docs().base_html);

    init_with(&tmp, "launch", "pages", Some("landing")).success();
    assert_eq!(base("launch"), seite::themes::landing().base_html);

    init_with(&tmp, "mine", "posts", Some("blank")).success();
    assert_eq!(base("mine"), seite::themes::blank().base_html);
    assert!(tmp.path().join("mine/static/css/site.css").exists());

    init_with(&tmp, "mag", "posts", Some("magazine"))
        .failure()
        .stderr(predicate::str::contains("gallery example"))
        .stderr(predicate::str::contains("seite theme apply magazine"));
    assert!(!tmp.path().join("mag").exists());
}

#[test]
fn test_every_bundled_theme_passes_strict_check() {
    let tmp = TempDir::new().unwrap();
    let site = full_site(&tmp);
    for theme in seite::themes::all() {
        page_cmd()
            .args(["theme", "apply", theme.name])
            .current_dir(&site)
            .assert()
            .success();
        let (ok, doc) = run_json(&site, &["check", "--strict"]);
        assert!(ok, "{}: {doc}", theme.name);
    }
}

#[test]
fn test_theme_new_scaffolds_blank_with_linked_stylesheet() {
    let tmp = TempDir::new().unwrap();
    let site = full_site(&tmp);
    // Fingerprinting and minification on: the plain href must still resolve.
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replacen("[build]\n", "[build]\nfingerprint = true\n", 1),
    )
    .unwrap();

    let (ok, doc) = run_json(&site, &["theme", "new", "coral"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["starter"], "blank");
    assert!(doc["data"]["stylesheet"]
        .as_str()
        .unwrap()
        .ends_with("site.css"));
    assert_eq!(
        fs::read_to_string(site.join("templates/base.html")).unwrap(),
        seite::themes::blank().base_html
    );
    assert!(site.join("static/css/site.css").exists());

    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&site)
        .assert()
        .success();
    let pages = build_html(&site);
    assert!(site.join("dist/static/css/site.css").exists());
    let post = pages
        .iter()
        .find(|(k, _)| k.starts_with("posts/"))
        .unwrap()
        .1;
    assert!(post.contains(r#"<link rel="stylesheet" href="/static/css/site.css">"#));
    // Audit basics: language, skip link to main, one site-wide header that
    // is not an h1 (the page title is the h1), and the SEO head.
    assert!(post.contains(r#"<html lang="en">"#));
    assert!(post.contains(r##"<a href="#main" class="skip-link">"##));
    assert!(post.contains(r#"<main id="main">"#));
    assert_eq!(post.matches("<h1").count(), 1, "{post}");
    assert!(post.contains("og:title") && post.contains("application/ld+json"));
    let doc_page = &pages["docs/intro.html"];
    assert!(doc_page.contains(r#"class="docs-nav""#), "{doc_page}");
    assert!(doc_page.contains(r#"aria-current="page""#), "{doc_page}");

    // A customized base.html is refused without --force, then backed up.
    fs::write(site.join("templates/base.html"), "<html>mine</html>").unwrap();
    page_cmd()
        .args(["theme", "new"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    page_cmd()
        .args(["theme", "new", "--force"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Backed up"));
    assert_eq!(
        fs::read_to_string(site.join("templates/base.html.bak")).unwrap(),
        "<html>mine</html>"
    );
}

#[test]
fn test_theme_apply_blank_writes_stylesheet_once() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Blank", "posts");
    let site = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "blank"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("static/css/site.css"));
    fs::write(site.join("static/css/site.css"), "body{color:red}").unwrap();
    page_cmd()
        .args(["theme", "apply", "blank"])
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(site.join("static/css/site.css")).unwrap(),
        "body{color:red}"
    );
}

#[test]
fn test_theme_create_refuses_nested_session_before_writing() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nested", "posts");
    let site = tmp.path().join("site");
    let before = fs::read_to_string(site.join("templates/base.html")).unwrap();
    page_cmd()
        .args(["theme", "create", "coral brutalist"])
        .env("CLAUDECODE", "1")
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("nested"));
    assert_eq!(
        fs::read_to_string(site.join("templates/base.html")).unwrap(),
        before
    );
    assert!(!site.join("static/css/site.css").exists());
    assert!(!site.join("templates/base.html.bak").exists());
}

#[test]
fn test_theme_export_inlines_stylesheet_and_applies_elsewhere() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "one", "One", "posts");
    init_site(&tmp, "two", "Two", "posts");
    let one = tmp.path().join("one");
    let two = tmp.path().join("two");
    page_cmd()
        .args(["theme", "new"])
        .current_dir(&one)
        .assert()
        .success();
    let mut css = fs::read_to_string(one.join("static/css/site.css")).unwrap();
    css.push_str("\n.coral-marker { color: coral; }\n");
    fs::write(one.join("static/css/site.css"), css).unwrap();
    page_cmd()
        .args(["theme", "export", "coral"])
        .current_dir(&one)
        .assert()
        .success()
        .stdout(predicate::str::contains("Inlined /static/css/site.css"));
    let exported = fs::read_to_string(one.join("templates/themes/coral.tera")).unwrap();
    assert!(!exported.contains(r#"href="/static/css/site.css""#));
    assert!(exported.contains(r#"{% include "seite/head.html" %}"#));

    fs::create_dir_all(two.join("templates/themes")).unwrap();
    fs::write(two.join("templates/themes/coral.tera"), &exported).unwrap();
    page_cmd()
        .args(["theme", "apply", "coral"])
        .current_dir(&two)
        .assert()
        .success();
    let pages = build_html(&two);
    let index = &pages["index.html"];
    assert!(index.contains(".coral-marker { color: coral; }"), "{index}");
    assert!(index.contains("<style>"), "{index}");
    assert!(!two.join("static/css/site.css").exists());
    page_cmd()
        .args(["check", "--strict"])
        .current_dir(&two)
        .assert()
        .success();
}

#[test]
fn test_special_characters_in_titles_cannot_break_the_head() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Esc", "posts");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2026-01-02-esc.md"),
        "---\ntitle: 'A \"quoted\" title'\ndate: 2026-01-02\ndescription: 'Ends </script><script>alert(1)</script>'\n---\n\nbody\n",
    )
    .unwrap();
    let pages = build_html(&site);
    let post = &pages["posts/esc.html"];
    let head = &post[..post.find("</head>").unwrap()];
    assert!(
        head.contains(r#"<meta property="og:title" content="A &quot;quoted&quot; title">"#),
        "{head}"
    );
    assert!(!head.contains("<script>alert(1)"), "{head}");
    assert_eq!(
        head.matches("<script").count(),
        head.matches("</script>").count()
    );
    assert!(head.contains("\\u003c/script>"), "{head}");
}

#[test]
fn test_listing_pages_have_distinct_titles_and_heading_order() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Site", "posts,pages,trust");
    let site = tmp.path().join("site");
    add_language(&site, "es", "Sitio");
    let title = |html: &str| {
        let start = html.find("<title>").unwrap() + "<title>".len();
        html[start..start + html[start..].find("</title>").unwrap()].to_string()
    };
    let pages = build_html(&site);
    // Home keeps the site title; collection indexes get their label.
    assert_eq!(title(&pages["index.html"]), "Site");
    assert_eq!(title(&pages["es/index.html"]), "Sitio");
    assert_eq!(title(&pages["posts/index.html"]), "Posts — Site");
    assert_eq!(title(&pages["es/posts/index.html"]), "Posts — Sitio");

    // A homepage from content/pages/index.md is still the site title.
    fs::write(
        site.join("content/pages/index.md"),
        "---\ntitle: Welcome\n---\n\nHello\n",
    )
    .unwrap();
    let pages = build_html(&site);
    assert_eq!(title(&pages["index.html"]), "Site");

    // Tag pages: item headings are h2 directly under the h1.
    let tag = pages
        .iter()
        .find(|(k, _)| k.starts_with("tags/") && k.as_str() != "tags/index.html")
        .unwrap()
        .1;
    let main = &tag[tag.find("<main").unwrap()..];
    assert!(main.contains("<h2><a href="), "{main}");
    assert!(!main.contains("<h3"), "{main}");

    // Trust certification links name the certification for screen readers.
    let trust = &pages["trust/index.html"];
    assert!(
        trust.contains(r#"aria-label="Learn more: SOC 2 Type II">Learn more &rarr;</a>"#),
        "{trust}"
    );
}
