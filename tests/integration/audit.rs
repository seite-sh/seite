//! `seite audit`: fresh sites score high, seeded problems are reported per
//! category with source attribution, stable JSON, `--fail-on`, `--category`
//! and `--dist`.

use super::common::*;

fn fresh_site(tmp: &TempDir) -> std::path::PathBuf {
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
    tmp.path().join("site")
}

fn findings(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    doc["data"]["findings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn find<'a>(all: &'a [serde_json::Value], code: &str) -> Vec<&'a serde_json::Value> {
    all.iter().filter(|f| f["code"] == code).collect()
}

/// A noisy PNG (doesn't compress) of roughly `side * side * 3` bytes.
fn noise_png(path: &std::path::Path, side: u32) {
    let mut seed: u32 = 12345;
    let img = image::RgbImage::from_fn(side, side, |_, _| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let b = seed.to_be_bytes();
        image::Rgb([b[0], b[1], b[2]])
    });
    img.save(path).unwrap();
}

/// A site with one seeded problem (at least) per category.
fn bad_site(tmp: &TempDir) -> std::path::PathBuf {
    let site = fresh_site(tmp);
    let w = |rel: &str, body: &str| {
        let path = site.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    };
    w(
        "content/posts/2025-01-01-bad.md",
        "---\ntitle: Bad\ndate: 2025-01-01\n---\n\nShort body.\n\n\
         See the [secret plan](/posts/secret) and [this section](/posts/hello-world#nope).\n\n\
         For the terms, [click here](/terms).\n\n\
         <img src=\"/static/noalt.png\">\n\n<img src=\"/static/big.png\" alt=\"Noise\">\n",
    );
    w(
        "content/posts/secret.md",
        "---\ntitle: Secret\ndate: 2025-01-02\ndraft: true\n---\nNot yet.\n",
    );
    w(
        "content/posts/undated.md",
        "---\ntitle: Undated\ndescription: A post without any date at all, which dated collections need.\n---\nNo date.\n",
    );
    w(
        "content/pages/terms.md",
        "---\ntitle: Terms\ndescription: The terms of service for using this fresh test site, in plain words.\n---\nTerms.\n",
    );
    image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3]))
        .save(site.join("static/noalt.png"))
        .unwrap();
    noise_png(&site.join("static/big.png"), 400);
    // A render-blocking script in the layout: a template finding.
    let base = fs::read_to_string(site.join("templates/base.html")).unwrap();
    let base = base.replacen(
        "</head>",
        "<script src=\"/static/app.js\"></script>\n</head>",
        1,
    );
    fs::write(site.join("templates/base.html"), base).unwrap();
    fs::write(site.join("static/app.js"), "console.log(1)\n").unwrap();
    site
}

#[test]
fn test_audit_fresh_site_scores_high_without_errors() {
    let tmp = TempDir::new().unwrap();
    let site = fresh_site(&tmp);
    let (ok, doc) = run_json(&site, &["audit"]);
    assert!(ok, "{doc}");
    let summary = &doc["data"]["summary"];
    assert_eq!(summary["errors"], 0, "{doc:#}");
    // Starter content and the default theme have no findings above notice
    // level (`og:image` suggestions remain notices).
    assert_eq!(summary["warnings"], 0, "{doc:#}");
    assert!(summary["score"].as_u64().unwrap() >= 95, "{doc:#}");
    for f in findings(&doc) {
        assert_eq!(f["severity"], "notice", "{f}");
    }
    assert!(summary["pages"].as_u64().unwrap() >= 3, "{doc:#}");
    for category in ["seo", "geo", "a11y", "content", "perf"] {
        assert!(summary["by_category"][category].is_object(), "{category}");
    }
    // Built in a scratch dir: no dist/.
    assert!(!site.join("dist").exists());
    // Human report.
    page_cmd()
        .arg("audit")
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Audit: score"))
        .stdout(predicate::str::contains("Accessibility"));
}

#[test]
fn test_audit_reports_seeded_problems_with_sources() {
    let tmp = TempDir::new().unwrap();
    let site = bad_site(&tmp);
    let (ok, doc) = run_json(&site, &["audit"]);
    assert!(ok, "report only without --fail-on: {doc}");
    let all = findings(&doc);
    let bad_md = "content/posts/2025-01-01-bad.md";

    // seo: from the page's own frontmatter/body.
    let missing = find(&all, "seo-description-missing");
    let f = missing
        .iter()
        .find(|f| f["page_url"] == "/posts/bad")
        .unwrap_or_else(|| panic!("{all:#?}"));
    assert_eq!(f["file"], bad_md);
    let thin = find(&all, "seo-thin-content");
    assert!(thin.iter().any(|f| f["file"] == bad_md), "{thin:#?}");
    let anchor = find(&all, "seo-broken-anchor");
    assert_eq!(anchor.len(), 1, "{all:#?}");
    assert_eq!(anchor[0]["file"], bad_md);
    assert_eq!(anchor[0]["line"], 8);
    // geo: no summary in llms.txt for the page without a description —
    // reported once, as seo-description-missing, unless SEO is filtered out.
    assert!(!find(&all, "geo-llms-no-summary")
        .iter()
        .any(|f| f["file"] == bad_md));
    let (_, geo) = run_json(&site, &["audit", "--category", "geo"]);
    assert!(find(&findings(&geo), "geo-llms-no-summary")
        .iter()
        .any(|f| f["file"] == bad_md));
    // a11y: image without alt, ambiguous link text, both at their line.
    let alt = find(&all, "a11y-img-alt");
    assert_eq!(alt.len(), 1, "{alt:#?}");
    assert_eq!(alt[0]["file"], bad_md);
    assert_eq!(alt[0]["line"], 12);
    let ambiguous = find(&all, "a11y-link-ambiguous");
    assert_eq!(ambiguous.len(), 1, "{ambiguous:#?}");
    assert_eq!(ambiguous[0]["line"], 10);
    // content: link to a draft, undated post.
    let draft = find(&all, "content-link-to-draft");
    assert_eq!(draft.len(), 1, "{all:#?}");
    assert_eq!(draft[0]["file"], bad_md);
    assert!(draft[0]["message"]
        .as_str()
        .unwrap()
        .contains("content/posts/secret.md"));
    let undated = find(&all, "content-missing-date");
    assert_eq!(undated.len(), 1, "{all:#?}");
    assert_eq!(undated[0]["file"], "content/posts/undated.md");
    // perf: the template script, once for all pages, at its line.
    let blocking = find(&all, "perf-render-blocking-script");
    assert_eq!(blocking.len(), 1, "collapsed: {blocking:#?}");
    assert_eq!(blocking[0]["file"], "templates/base.html");
    assert!(blocking[0]["line"].as_u64().is_some());
    assert!(
        blocking[0]["message"]
            .as_str()
            .unwrap()
            .contains("more page"),
        "{blocking:#?}"
    );
    let large = find(&all, "perf-image-large");
    assert_eq!(large.len(), 1, "{all:#?}");
    assert_eq!(large[0]["file"], bad_md);

    // Errors lower the score; every finding documents itself.
    let summary = &doc["data"]["summary"];
    assert!(summary["errors"].as_u64().unwrap() >= 2);
    assert!(summary["score"].as_u64().unwrap() < 80, "{summary}");
    for f in &all {
        let code = f["code"].as_str().unwrap();
        assert_eq!(
            f["docs_url"],
            format!("https://seite.sh/docs/errors#{code}"),
            "{f}"
        );
        assert!(!f["hint"].as_str().unwrap().is_empty(), "{f}");
    }

    // Deterministic: a second run gives the same document.
    let (_, again) = run_json(&site, &["audit"]);
    assert_eq!(doc["data"], again["data"]);
}

#[test]
fn test_audit_json_shape() {
    let tmp = TempDir::new().unwrap();
    let site = bad_site(&tmp);
    let (_, doc) = run_json(&site, &["audit"]);
    assert_eq!(doc["command"], "audit");
    let data = doc["data"].as_object().unwrap();
    let keys: Vec<&str> = data.keys().map(String::as_str).collect();
    assert_eq!(keys, ["summary", "findings", "omitted"]);
    assert_eq!(data["omitted"], 0);
    let summary: Vec<&str> = data["summary"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        summary,
        [
            "score",
            "errors",
            "warnings",
            "notices",
            "pages",
            "by_category"
        ]
    );
    for f in findings(&doc) {
        let keys: Vec<&str> = f.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "code", "severity", "category", "page_url", "file", "line", "message", "hint",
                "docs_url"
            ]
        );
        assert!(["error", "warning", "notice"].contains(&f["severity"].as_str().unwrap()));
    }
}

#[test]
fn test_audit_fail_on_exit_codes() {
    let tmp = TempDir::new().unwrap();
    let site = bad_site(&tmp);
    let (ok, doc) = run_json(&site, &["audit", "--fail-on", "error"]);
    assert!(!ok);
    assert_eq!(doc["ok"], false);
    assert!(doc["data"]["findings"].is_array(), "report kept: {doc}");
    page_cmd()
        .args(["audit", "--fail-on", "error"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("--fail-on error"));

    // Only warnings and notices in these categories.
    let (ok, doc) = run_json(&site, &["audit", "--category", "geo", "--fail-on", "error"]);
    assert!(ok, "{doc}");
    let (ok, _) = run_json(
        &site,
        &["audit", "--category", "geo", "--fail-on", "warning"],
    );
    let warnings = doc["data"]["summary"]["warnings"].as_u64().unwrap();
    assert_eq!(ok, warnings == 0);
    let (ok, _) = run_json(
        &site,
        &["audit", "--category", "perf", "--fail-on", "warning"],
    );
    assert!(!ok, "the render-blocking script is a warning");
}

#[test]
fn test_audit_category_filter() {
    let tmp = TempDir::new().unwrap();
    let site = bad_site(&tmp);
    let (ok, doc) = run_json(&site, &["audit", "--category", "a11y,content"]);
    assert!(ok, "{doc}");
    let all = findings(&doc);
    assert!(!all.is_empty());
    for f in &all {
        assert!(f["category"] == "a11y" || f["category"] == "content", "{f}");
    }
    let by_category = doc["data"]["summary"]["by_category"].as_object().unwrap();
    let keys: Vec<&str> = by_category.keys().map(String::as_str).collect();
    assert_eq!(keys, ["a11y", "content"]);
    page_cmd()
        .args(["audit", "--category", "speed"])
        .current_dir(&site)
        .assert()
        .failure();
}

#[test]
fn test_audit_dist_mode_reads_existing_output() {
    let tmp = TempDir::new().unwrap();
    let site = bad_site(&tmp);
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    // A robots.txt from elsewhere that shuts out an AI search crawler.
    fs::write(
        site.join("dist/robots.txt"),
        "User-agent: *\nAllow: /\n\nUser-agent: PerplexityBot\nDisallow: /\n",
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["audit", "--dist", "dist"]);
    assert!(ok, "{doc}");
    let all = findings(&doc);
    let robots = find(&all, "geo-robots-blocks-ai-search");
    assert_eq!(robots.len(), 1, "{all:#?}");
    assert_eq!(robots[0]["file"], "dist/robots.txt");
    assert!(robots[0]["page_url"].is_null());
    assert!(robots[0]["message"]
        .as_str()
        .unwrap()
        .contains("PerplexityBot"));
    assert!(!find(&all, "geo-content-signal-missing").is_empty());
    // Generated files, with lines; no content checks without sources.
    let alt = find(&all, "a11y-img-alt");
    assert_eq!(alt.len(), 1, "{all:#?}");
    assert_eq!(alt[0]["file"], "dist/posts/bad.html");
    assert!(alt[0]["line"].as_u64().is_some());
    assert!(all.iter().all(|f| f["category"] != "content"));
    // The output dir is only read.
    assert!(site.join("dist/posts/bad.html").exists());

    page_cmd()
        .args(["audit", "--dist", "nope"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a directory"));
}

#[test]
fn test_audit_outside_a_site_fails_clearly() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .arg("audit")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("seite.toml"));
}

/// Fresh sites have no warnings at all: every preset together with each
/// starter theme, and each preset on its own (default starter).
#[test]
fn test_audit_fresh_sites_have_no_warnings() {
    let all = "posts,docs,pages,changelog,roadmap,trust";
    let mut cases: Vec<(&str, &str)> = ["default", "docs", "landing", "blank"]
        .into_iter()
        .map(|theme| (theme, all))
        .collect();
    for preset in all.split(',') {
        cases.push(("default", preset));
    }
    for (i, (theme, collections)) in cases.into_iter().enumerate() {
        let tmp = TempDir::new().unwrap();
        let name = format!("site{i}");
        page_cmd()
            .args([
                "init",
                &name,
                "--title",
                "Fresh Site",
                "--description",
                "A fresh site made to test the audit command end to end",
                "--deploy-target",
                "github-pages",
                "--collections",
                collections,
                "--theme",
                theme,
            ])
            .current_dir(tmp.path())
            .assert()
            .success();
        let (ok, doc) = run_json(&tmp.path().join(&name), &["audit", "--fail-on", "warning"]);
        assert!(ok, "{theme} / {collections}: {doc:#}");
        let summary = &doc["data"]["summary"];
        assert_eq!(summary["warnings"], 0, "{theme} / {collections}: {doc:#}");
        assert_eq!(summary["errors"], 0, "{theme} / {collections}: {doc:#}");
        assert_eq!(summary["score"], 100, "{theme} / {collections}: {doc:#}");
    }
}

#[test]
fn test_audit_min_severity_filters_findings_not_summary() {
    let tmp = TempDir::new().unwrap();
    let site = bad_site(&tmp);
    let (_, all) = run_json(&site, &["audit"]);
    let (ok, errors_only) = run_json(&site, &["audit", "--min-severity", "error"]);
    assert!(ok);
    assert_eq!(all["data"]["summary"], errors_only["data"]["summary"]);
    let kept = findings(&errors_only);
    assert!(!kept.is_empty());
    assert!(kept.iter().all(|f| f["severity"] == "error"));
    assert_eq!(
        errors_only["data"]["omitted"].as_u64().unwrap() as usize,
        findings(&all).len() - kept.len()
    );
}

/// The same site gives the same findings whether the audit builds it or
/// reads the built output (`--dist`): only the files differ (sources vs
/// generated pages), and `content` checks need the sources.
#[test]
fn test_audit_build_and_dist_modes_agree() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args([
            "init",
            "site",
            "--title",
            "Parity",
            "--description",
            "A site to compare the build and dist modes of the audit",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,docs,pages,changelog",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2025-01-01-short.md"),
        "---\ntitle: A short post with a title that is clearly much longer than sixty characters\ndate: 2025-01-01\n---\n\nToo short.\n",
    )
    .unwrap();
    fs::write(
        site.join("content/docs/nodesc.md"),
        "---\ntitle: No description\n---\n\nA doc without a description.\n",
    )
    .unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let pairs = |args: &[&str]| -> Vec<(String, String)> {
        let (ok, doc) = run_json(&site, args);
        assert!(ok, "{doc}");
        let mut v: Vec<(String, String)> = findings(&doc)
            .iter()
            .filter(|f| f["category"] != "content")
            .map(|f| {
                (
                    f["code"].as_str().unwrap().to_string(),
                    f["page_url"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        v.sort();
        v
    };
    let built = pairs(&["audit"]);
    let dist = pairs(&["audit", "--dist", "dist"]);
    assert_eq!(built, dist);
    // The changelog entry is exempt from thin content in both modes.
    assert!(!built
        .iter()
        .any(|(c, u)| c == "seo-thin-content" && u.starts_with("/changelog")));
    assert!(built.contains(&("seo-title-long".to_string(), "/posts/short".to_string())));
    assert!(built.contains(&(
        "seo-description-missing".to_string(),
        "/docs/nodesc".to_string()
    )));
}

#[test]
fn test_audit_skips_static_copies_but_not_pages_under_static() {
    let tmp = TempDir::new().unwrap();
    let site = fresh_site(&tmp);
    // A pass-through HTML file (e.g. a CMS admin app): not audited.
    fs::create_dir_all(site.join("static/admin")).unwrap();
    fs::write(
        site.join("static/admin/index.html"),
        "<!doctype html><html><head></head><body><img src=x></body></html>",
    )
    .unwrap();
    // A collection rendered under /static/: audited like any page.
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        format!(
            "{config}\n[[collections]]\nname = \"notes\"\nlabel = \"Notes\"\ndirectory = \"notes\"\nhas_date = false\nhas_rss = false\nlisted = false\nurl_prefix = \"/static/notes\"\nnested = false\ndefault_template = \"page.html\"\n"
        ),
    )
    .unwrap();
    fs::create_dir_all(site.join("content/notes")).unwrap();
    fs::write(
        site.join("content/notes/thin.md"),
        "---\ntitle: Thin\n---\n\nShort.\n",
    )
    .unwrap();
    let (_, doc) = run_json(&site, &["audit"]);
    let all = findings(&doc);
    assert!(
        all.iter().all(|f| !f.to_string().contains("static/admin")),
        "{doc:#}"
    );
    assert!(
        all.iter().any(|f| f["page_url"]
            .as_str()
            .is_some_and(|u| u.starts_with("/static/notes/thin"))),
        "pages under /static/ are still audited: {doc:#}"
    );
}
