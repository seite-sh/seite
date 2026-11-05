//! Generated social cards (`[og]`, `src/build/og/`): PNGs per page, head
//! tags, exclusions, the cache, determinism, the CJK fallback, config
//! validation, `seite og preview`, audit and upgrade.

use super::common::*;
use std::path::{Path, PathBuf};

fn site(tmp: &TempDir) -> PathBuf {
    init_site(tmp, "site", "Card Site", "posts,docs,pages");
    tmp.path().join("site")
}

fn post(site: &Path, file: &str, frontmatter: &str) {
    write_site_file(
        site,
        &format!("content/posts/{file}"),
        &format!("---\n{frontmatter}\n---\n\nSome words in the body of this post.\n"),
    );
}

fn read(site: &Path, rel: &str) -> String {
    fs::read_to_string(site.join("dist").join(rel)).unwrap()
}

/// Width and height from a PNG's IHDR chunk.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{}", path.display());
    assert_eq!(&bytes[12..16], b"IHDR");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

fn og_image(html: &str) -> Option<String> {
    let start = html.find("<meta property=\"og:image\" content=\"")?;
    let rest = &html[start + "<meta property=\"og:image\" content=\"".len()..];
    Some(rest[..rest.find('"').unwrap()].to_string())
}

fn files_under(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    files.sort();
    files
}

#[test]
fn test_og_init_turns_cards_on() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(toml.contains("[og]\ngenerate = true\n"), "{toml}");
    let gitignore = fs::read_to_string(site.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/.seite/cache/"), "{gitignore}");
}

#[test]
fn test_og_cards_generated_with_head_tags() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(
        &site,
        "2025-01-01-first.md",
        "title: First Post\ndate: 2025-01-01",
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    for card in ["og/index.png", "og/posts/first.png"] {
        assert_eq!(
            png_size(&site.join("dist").join(card)),
            (1200, 630),
            "{card}"
        );
    }
    let html = read(&site, "posts/first.html");
    assert!(html.contains(
        "<meta property=\"og:image\" content=\"http://localhost:3000/og/posts/first.png\">"
    ));
    assert!(html.contains("<meta property=\"og:image:width\" content=\"1200\">"));
    assert!(html.contains("<meta property=\"og:image:height\" content=\"630\">"));
    assert!(html.contains("<meta property=\"og:image:alt\" content=\"First Post\">"));
    assert!(html.contains("<meta name=\"twitter:card\" content=\"summary_large_image\">"));
    assert!(html.contains(
        "<meta name=\"twitter:image\" content=\"http://localhost:3000/og/posts/first.png\">"
    ));
    // Pages without a card of their own use the site card, described by
    // the site title.
    for page in ["index.html", "posts/index.html", "404.html"] {
        let html = read(&site, page);
        assert_eq!(
            og_image(&html).as_deref(),
            Some("http://localhost:3000/og/index.png"),
            "{page}"
        );
        assert!(
            html.contains("<meta property=\"og:image:alt\" content=\"Card Site\">"),
            "{page}"
        );
    }
    let stats = &doc["data"]["og_cards"];
    assert!(stats["rendered"].as_u64().unwrap() >= 2, "{stats}");
    assert_eq!(stats["cached"], 0, "{stats}");
}

#[test]
fn test_og_urls_absolutized_under_base_path() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .replace("http://localhost:3000", "https://example.github.io/repo");
    fs::write(site.join("seite.toml"), toml).unwrap();
    post(
        &site,
        "2025-01-01-first.md",
        "title: First\ndate: 2025-01-01",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(
        og_image(&read(&site, "posts/first.html")).as_deref(),
        Some("https://example.github.io/repo/og/posts/first.png")
    );
    // The card itself is at the output root, served under the base path.
    assert!(site.join("dist/og/posts/first.png").exists());
}

#[test]
fn test_og_frontmatter_image_wins() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(
        &site,
        "2025-01-01-own.md",
        "title: Own Image\ndate: 2025-01-01\nimage: /static/own.png",
    );
    write_test_image(&site, "own.png");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert!(!site.join("dist/og/posts/own.png").exists());
    let html = read(&site, "posts/own.html");
    assert_eq!(
        og_image(&html).as_deref(),
        Some("http://localhost:3000/static/own.png")
    );
    // Unchanged from before generated cards: no alt for authored images.
    assert!(!html.contains("og:image:alt"));
    assert!(html.contains("<meta name=\"twitter:card\" content=\"summary_large_image\">"));
}

#[test]
fn test_og_private_and_draft_pages_get_no_card() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    add_collection_line(&site, "docs", "private = true");
    write_site_file(
        &site,
        "content/docs/secret-plan.md",
        "---\ntitle: Secret Plan\n---\n\nhidden\n",
    );
    post(
        &site,
        "2025-01-01-wip.md",
        "title: Work In Progress\ndate: 2025-01-01\ndraft: true",
    );
    page_cmd()
        .args(["build", "--drafts"])
        .current_dir(&site)
        .assert()
        .success();
    assert!(site.join("dist/docs/secret-plan.html").exists());
    let cards = files_under(&site.join("dist/og"));
    assert!(
        !cards
            .iter()
            .any(|c| c.contains("secret") || c.contains("wip")),
        "{cards:?}"
    );
    // Both fall back to the site card, which reveals nothing.
    for page in ["docs/secret-plan.html", "posts/wip.html"] {
        assert_eq!(
            og_image(&read(&site, page)).as_deref(),
            Some("http://localhost:3000/og/index.png"),
            "{page}"
        );
    }
}

#[test]
fn test_og_absent_config_writes_nothing_new() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    remove_og_section(&site);
    post(
        &site,
        "2025-01-01-first.md",
        "title: First\ndate: 2025-01-01",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert!(!site.join("dist/og").exists());
    let snapshot = |site: &Path| -> Vec<(String, Vec<u8>)> {
        files_under(&site.join("dist"))
            .into_iter()
            .map(|f| {
                let bytes = fs::read(site.join("dist").join(&f)).unwrap();
                (f, bytes)
            })
            .collect()
    };
    let without = snapshot(&site);
    for (file, bytes) in &without {
        if file.ends_with(".html") {
            let html = String::from_utf8_lossy(bytes);
            assert!(!html.contains("/og/"), "{file}");
            assert!(!html.contains("og:image"), "{file}");
            assert!(
                html.contains("<meta name=\"twitter:card\" content=\"summary\">")
                    || !html.contains("twitter:card"),
                "{file}"
            );
        }
    }
    // `generate = false` is the same as no section.
    let mut toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    toml.push_str("\n[og]\ngenerate = false\n");
    fs::write(site.join("seite.toml"), toml).unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(without, snapshot(&site));
}

#[test]
fn test_og_cache_reused_and_output_deterministic() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(&site, "2025-01-01-a.md", "title: Alpha\ndate: 2025-01-01");
    post(&site, "2025-01-02-b.md", "title: Beta\ndate: 2025-01-02");
    let (ok, first) = run_json(&site, &["build"]);
    assert!(ok, "{first}");
    let rendered = first["data"]["og_cards"]["rendered"].as_u64().unwrap();
    assert!(rendered >= 3, "{first}");
    let cache = site.join(".seite/cache/og");
    assert_eq!(files_under(&cache).len() as u64, rendered);
    let card = fs::read(site.join("dist/og/posts/a.png")).unwrap();

    // Second build: every card comes from the cache.
    let (ok, second) = run_json(&site, &["build"]);
    assert!(ok, "{second}");
    assert_eq!(second["data"]["og_cards"]["rendered"], 0, "{second}");
    assert_eq!(
        second["data"]["og_cards"]["cached"].as_u64().unwrap(),
        rendered
    );
    assert_eq!(fs::read(site.join("dist/og/posts/a.png")).unwrap(), card);

    // A changed title renders just that card.
    post(
        &site,
        "2025-01-01-a.md",
        "title: Alpha Two\ndate: 2025-01-01",
    );
    let (_, third) = run_json(&site, &["build"]);
    assert_eq!(third["data"]["og_cards"]["rendered"], 1, "{third}");

    // Determinism: rendering from scratch gives identical bytes.
    post(&site, "2025-01-01-a.md", "title: Alpha\ndate: 2025-01-01");
    fs::remove_dir_all(&cache).unwrap();
    let (_, fresh) = run_json(&site, &["build"]);
    assert_eq!(fresh["data"]["og_cards"]["cached"], 0, "{fresh}");
    assert_eq!(fs::read(site.join("dist/og/posts/a.png")).unwrap(), card);
}

#[test]
fn test_og_cjk_title_skips_card_with_notice() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(
        &site,
        "2025-01-01-cjk.md",
        "title: 你好世界\ndate: 2025-01-01\nslug: cjk",
    );
    post(
        &site,
        "2025-01-02-emoji.md",
        "title: Launch day 🚀\ndate: 2025-01-02\nslug: emoji",
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(!site.join("dist/og/posts/cjk.png").exists());
    assert!(!site.join("dist/og/posts/emoji.png").exists());
    assert_eq!(doc["data"]["og_cards"]["missing_glyphs"], 2, "{doc}");
    let diags = doc["data"]["diagnostics"].as_array().unwrap();
    let notice: Vec<_> = diags
        .iter()
        .filter(|d| d["code"] == "og-missing-glyphs")
        .collect();
    assert_eq!(notice.len(), 1, "{doc}");
    assert!(notice[0]["message"]
        .as_str()
        .unwrap()
        .starts_with("2 pages got no social card"));
    assert_eq!(
        og_image(&read(&site, "posts/cjk.html")).as_deref(),
        Some("http://localhost:3000/og/index.png")
    );
}

#[test]
fn test_og_invalid_config_is_located() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .replace(
            "[og]\n",
            "[og]\naccent = \"blue\"\nlogo = \"static/nope.svg\"\n",
        );
    fs::write(site.join("seite.toml"), &toml).unwrap();
    let og_line = toml.lines().position(|l| l == "[og]").unwrap() + 1;
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok, "{doc}");
    let text = doc.to_string();
    let errors: Vec<&serde_json::Value> = doc["error"]["diagnostics"]
        .as_array()
        .or_else(|| doc["data"]["diagnostics"].as_array())
        .unwrap_or_else(|| panic!("{text}"))
        .iter()
        .filter(|d| d["code"] == "og-config-invalid")
        .collect();
    assert_eq!(errors.len(), 2, "{text}");
    assert_eq!(errors[0]["file"], "seite.toml");
    assert_eq!(errors[0]["line"], og_line + 1);
    assert_eq!(errors[1]["line"], og_line + 2);
    // `seite build` fails on it too.
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure();
}

#[test]
fn test_og_check_renders_no_cards() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    // A scheduled post makes check render a second scratch build too.
    post(
        &site,
        "2099-01-01-later.md",
        "title: Later\ndate: 2099-01-01",
    );
    page_cmd()
        .arg("check")
        .current_dir(&site)
        .assert()
        .success();
    assert!(!site.join(".seite/cache/og").exists());
    assert!(!site.join("dist").exists());
}

#[test]
fn test_og_preview_writes_png() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(
        &site,
        "2025-01-01-first.md",
        "title: First\ndate: 2025-01-01",
    );
    // --out stays inside the site (agents run `og preview` without asking).
    let outside = tmp.path().join("card.png");
    let (ok, _) = run_json(
        &site,
        &["og", "preview", "/", "--out", outside.to_str().unwrap()],
    );
    assert!(!ok);
    assert!(!outside.exists());
    // One clean message, no internal error chain.
    let (ok, doc) = run_json(&site, &["og", "preview", "/", "--out", "../card.png"]);
    assert!(!ok);
    let err = doc["error"].to_string();
    assert!(
        err.contains("--out must be a file inside the site"),
        "{err}"
    );
    assert!(!err.contains("prefix not found"), "{err}");
    assert_eq!(
        doc["error"]["chain"].as_array().map_or(0, |c| c.len()),
        0,
        "{err}"
    );
    assert!(!outside.exists());
    // Inside the site: only .png files, never in dot directories, never
    // replacing a file that isn't a PNG.
    fs::create_dir_all(site.join(".git/hooks")).unwrap();
    let toml_before = fs::read(site.join("seite.toml")).unwrap();
    for bad in [
        ".git/hooks/pre-commit.png",
        "card.txt",
        "seite.toml",
        ".claude/x.png",
    ] {
        let (ok, doc) = run_json(&site, &["og", "preview", "/", "--out", bad]);
        assert!(!ok, "{bad}: {doc}");
        assert!(
            doc["error"]
                .to_string()
                .contains("--out must be a new or existing .png"),
            "{bad}: {doc}"
        );
    }
    fs::write(site.join("fake.png"), "not a png").unwrap();
    let (ok, _) = run_json(&site, &["og", "preview", "/", "--out", "fake.png"]);
    assert!(!ok);
    assert_eq!(fs::read(site.join("fake.png")).unwrap(), b"not a png");
    assert_eq!(fs::read(site.join("seite.toml")).unwrap(), toml_before);
    assert!(!site.join(".git/hooks/pre-commit.png").exists());
    let out = site.join("card.png");
    for target in ["/posts/first", "content/posts/2025-01-01-first.md", "/"] {
        let (ok, doc) = run_json(
            &site,
            &[
                "og",
                "preview",
                target,
                "--template",
                "bold",
                "--out",
                out.to_str().unwrap(),
            ],
        );
        assert!(ok, "{doc}");
        assert_eq!(png_size(&out), (1200, 630));
        fs::remove_file(&out).unwrap();
    }
    // Default output: the gitignored cache dir.
    page_cmd()
        .args(["og", "preview", "/"])
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(
        png_size(&site.join(".seite/cache/og-preview.png")),
        (1200, 630)
    );
}

#[test]
fn test_og_audit_fresh_site_has_no_og_findings() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--title",
            "Fresh Site",
            "--description",
            "A fresh site made to test the audit command end to end",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,docs,pages",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let site = tmp.path().join("site");
    let (_, doc) = run_json(&site, &["audit"]);
    assert_eq!(doc["data"]["summary"]["warnings"], 0, "{doc:#}");
    let findings = doc["data"]["findings"].as_array().unwrap();
    assert!(
        !findings.iter().any(|f| f["code"] == "seo-og-image-missing"),
        "{doc:#}"
    );
}

#[test]
fn test_og_upgrade_notice_once() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    remove_og_section(&site);
    let run = || {
        let output = page_cmd()
            .args(["upgrade", "--force"])
            .current_dir(&site)
            .output()
            .unwrap();
        assert!(output.status.success());
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    };
    let first = run();
    assert!(
        first.contains("Social preview cards are available"),
        "{first}"
    );
    assert!(!fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .contains("[og]"));
    assert!(!run().contains("Social preview cards are available"));
}

/// Every BreadcrumbList item URL on a site with a custom `url_prefix`, a
/// root-mounted `pages` collection and a second language is a built page.
#[test]
fn test_breadcrumb_urls_resolve_to_built_pages() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .replacen("url_prefix = \"/posts\"", "url_prefix = \"/blog\"", 1)
        // A trailing slash and a base path must not double up in crumbs.
        .replace("http://localhost:3000", "https://ex.com/sub/");
    fs::write(site.join("seite.toml"), toml).unwrap();
    add_language(&site, "es", "Sitio");
    post(
        &site,
        "2025-01-01-first.md",
        "title: First\ndate: 2025-01-01",
    );
    post(
        &site,
        "2025-01-01-first.es.md",
        "title: Primero\ndate: 2025-01-01",
    );
    write_site_file(
        &site,
        "content/pages/about.md",
        "---\ntitle: About\n---\n\nabout\n",
    );
    write_site_file(
        &site,
        "content/pages/about.es.md",
        "---\ntitle: Acerca\n---\n\nacerca\n",
    );
    write_site_file(
        &site,
        "content/docs/intro.md",
        "---\ntitle: Intro\n---\n\nintro\n",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();

    let dist = site.join("dist");
    let resolves = |url: &str| -> bool {
        let path = url
            .strip_prefix("https://ex.com/sub")
            .unwrap_or_else(|| panic!("not absolute: {url}"))
            .trim_matches('/');
        if path.is_empty() {
            return dist.join("index.html").is_file();
        }
        dist.join(format!("{path}.html")).is_file() || dist.join(path).join("index.html").is_file()
    };
    let mut checked = 0;
    let mut crumbs_by_page = std::collections::BTreeMap::new();
    for file in files_under(&dist)
        .into_iter()
        .filter(|f| f.ends_with(".html"))
    {
        let html = read(&site, &file);
        let Some(at) = html.find("\"@type\":\"BreadcrumbList\"") else {
            continue;
        };
        let start = html[..at].rfind('{').unwrap();
        let end = at + html[at..].find("</script>").unwrap();
        let json: serde_json::Value =
            serde_json::from_str(html[start..end].trim()).unwrap_or_else(|e| panic!("{file}: {e}"));
        let mut urls = Vec::new();
        for item in json["itemListElement"].as_array().unwrap() {
            if let Some(url) = item["item"].as_str() {
                assert!(
                    resolves(url),
                    "{file}: breadcrumb {url} is not a built page"
                );
                urls.push(url.to_string());
                checked += 1;
            }
        }
        crumbs_by_page.insert(file, urls);
    }
    assert!(checked >= 8, "{crumbs_by_page:?}");
    let crumbs = |page: &str| crumbs_by_page[page].clone();
    assert_eq!(
        crumbs("blog/first.html"),
        ["https://ex.com/sub", "https://ex.com/sub/blog/"]
    );
    assert_eq!(
        crumbs("es/blog/first.html"),
        ["https://ex.com/sub/es", "https://ex.com/sub/es/blog/"]
    );
    // Root-mounted pages: Home > page, no collection crumb.
    assert_eq!(crumbs("about.html"), ["https://ex.com/sub"]);
    assert_eq!(crumbs("es/about.html"), ["https://ex.com/sub/es"]);
    // A collection index is the last crumb itself (no "Blog > Blog").
    assert_eq!(
        crumbs("blog/index.html"),
        ["https://ex.com/sub", "https://ex.com/sub/blog/"]
    );
    assert_eq!(
        crumbs("es/blog/index.html"),
        ["https://ex.com/sub/es", "https://ex.com/sub/es/blog/"]
    );
    let html = read(&site, "blog/index.html");
    let at = html.find("\"@type\":\"BreadcrumbList\"").unwrap();
    assert_eq!(
        html[at..]
            .split("</script>")
            .next()
            .unwrap()
            .matches("ListItem")
            .count(),
        2
    );
    // No breadcrumb on the homepage.
    assert!(!crumbs_by_page.contains_key("index.html"));
    assert!(!crumbs_by_page.contains_key("es/index.html"));
}

#[test]
fn test_og_stale_cards_and_cache_entries_removed() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(&site, "2025-01-01-a.md", "title: Alpha\ndate: 2025-01-01");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let cache = site.join(".seite/cache/og");
    let before = files_under(&cache);
    assert!(site.join("dist/og/posts/a.png").exists());
    // Renamed page and new title: the old card leaves dist/ (full builds
    // start from a clean output) and its cache entry is pruned.
    fs::remove_file(site.join("content/posts/2025-01-01-a.md")).unwrap();
    post(&site, "2025-01-01-b.md", "title: Beta\ndate: 2025-01-01");
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(!site.join("dist/og/posts/a.png").exists());
    assert!(site.join("dist/og/posts/b.png").exists());
    assert_eq!(doc["data"]["og_cards"]["pruned"], 1, "{doc}");
    let after = files_under(&cache);
    assert_eq!(after.len(), before.len());
    assert_ne!(after, before);
}

#[test]
fn test_og_scheduled_page_gets_no_card() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    post(
        &site,
        "2099-01-01-later.md",
        "title: Later\ndate: 2099-01-01",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert!(!site.join("dist/posts/later.html").exists());
    assert!(!site.join("dist/og/posts/later.png").exists());
    // With --future it is built, with its own card.
    page_cmd()
        .args(["build", "--future"])
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(
        og_image(&read(&site, "posts/later.html")).as_deref(),
        Some("http://localhost:3000/og/posts/later.png")
    );
    assert!(site.join("dist/og/posts/later.png").exists());
}

#[test]
fn test_og_multilingual_site_cards() {
    let tmp = TempDir::new().unwrap();
    let site = site(&tmp);
    add_language(&site, "es", "Sitio de Tarjetas");
    post(
        &site,
        "2025-01-01-first.md",
        "title: First\ndate: 2025-01-01",
    );
    post(
        &site,
        "2025-01-01-first.es.md",
        "title: Primero\ndate: 2025-01-01",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    for card in [
        "og/index.png",
        "og/es/index.png",
        "og/posts/first.png",
        "og/es/posts/first.png",
    ] {
        assert_eq!(
            png_size(&site.join("dist").join(card)),
            (1200, 630),
            "{card}"
        );
    }
    assert_ne!(
        fs::read(site.join("dist/og/index.png")).unwrap(),
        fs::read(site.join("dist/og/es/index.png")).unwrap()
    );
    assert_eq!(
        og_image(&read(&site, "es/posts/first.html")).as_deref(),
        Some("http://localhost:3000/og/es/posts/first.png")
    );
    let es_home = read(&site, "es/index.html");
    assert_eq!(
        og_image(&es_home).as_deref(),
        Some("http://localhost:3000/og/es/index.png")
    );
    assert!(es_home.contains("<meta property=\"og:image:alt\" content=\"Sitio de Tarjetas\">"));
}

#[test]
fn test_og_subdomain_cards() {
    let tmp = TempDir::new().unwrap();
    let site = init_subdomain_site(&tmp, "site");
    write_site_file(
        &site,
        "content/docs/intro.md",
        "---\ntitle: Intro\n---\n\nintro\n",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let sub = site.join("dist-subdomains/docs");
    assert_eq!(png_size(&sub.join("og/index.png")), (1200, 630));
    assert_eq!(png_size(&sub.join("og/intro.png")), (1200, 630));
    let html = fs::read_to_string(sub.join("intro.html")).unwrap();
    let url = og_image(&html).unwrap();
    assert!(
        url.ends_with("/og/intro.png") && !url.starts_with("https://example.com/"),
        "{url}"
    );
    // The main site keeps its own cards, and no docs cards.
    assert!(site.join("dist/og/index.png").exists());
    assert!(!site.join("dist/og/docs").exists());
}
