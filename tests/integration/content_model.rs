//! Content model: taxonomies, related posts and series
//! (`src/build/{taxonomy,related,series}.rs`); typed frontmatter schemas,
//! scheduled publishing, and `seite check --external`.

use super::common::*;
use std::path::Path;

fn append_config(site: &Path, extra: &str) {
    let path = site.join("seite.toml");
    let mut config = fs::read_to_string(&path).unwrap();
    config.push_str(extra);
    fs::write(path, config).unwrap();
}

fn write_post(site: &Path, file: &str, frontmatter: &str) {
    fs::write(
        site.join("content/posts").join(file),
        format!("---\n{frontmatter}\n---\n\nBody of {file}.\n"),
    )
    .unwrap();
}

fn build(site: &Path) {
    page_cmd().arg("build").current_dir(site).assert().success();
}

fn read(site: &Path, rel: &str) -> String {
    fs::read_to_string(site.join("dist").join(rel))
        .unwrap_or_else(|e| panic!("cannot read dist/{rel}: {e}"))
}

/// A posts site whose `post.html` is the embedded default (init writes a copy;
/// sites created before 0.21 keep their copy, which is why it is removed here).
fn posts_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Model", "posts");
    let site = tmp.path().join("site");
    let _ = fs::remove_file(site.join("templates/post.html"));
    for entry in fs::read_dir(site.join("content/posts")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    site
}

fn write_series_content(site: &Path) {
    write_post(
        site,
        "2024-01-01-part-one.md",
        "title: Part One\ndate: 2024-01-01\ntags: [rust]\ncategories: [Systems]\nseries: Rust from scratch\nseries_order: 1",
    );
    write_post(
        site,
        "2024-02-01-part-two.md",
        "title: Part Two\ndate: 2024-02-01\ntags: [rust, web]\ncategories: [Systems, Web]\nseries: Rust from scratch",
    );
    write_post(
        site,
        "2024-03-01-part-three.md",
        "title: Part Three\ndate: 2024-03-01\ntags: [web]\nseries: rust from scratch",
    );
    write_post(
        site,
        "2024-04-01-other.md",
        "title: Other\ndate: 2024-04-01\ntags: [cooking]",
    );
}

#[test]
fn test_build_taxonomy_pages_feeds_and_sitemap() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(
        &site,
        "\n[[taxonomies]]\nname = \"categories\"\npaginate = 1\n",
    );
    write_series_content(&site);
    build(&site);

    let index = read(&site, "categories/index.html");
    assert!(index.contains("<h1>Categories</h1>"), "{index}");
    assert!(index.contains("href=\"/categories/systems/\""));
    assert!(index.contains("(2)"), "term counts: {index}");

    // Paginated term pages, newest first.
    let p1 = read(&site, "categories/systems/index.html");
    assert!(p1.contains("Part Two") && !p1.contains("Part One"), "{p1}");
    assert!(p1.contains("/categories/systems/page/2/"), "{p1}");
    let p2 = read(&site, "categories/systems/page/2/index.html");
    assert!(p2.contains("Part One"), "{p2}");
    assert!(!site.join("dist/categories/systems/page/3").exists());

    // Feeds + discovery link on the term page.
    let rss = read(&site, "categories/systems/feed.xml");
    assert!(rss.contains("Model — Categories: Systems"), "{rss}");
    assert!(rss.contains("/posts/part-one") && rss.contains("/posts/part-two"));
    let atom = read(&site, "categories/systems/atom.xml");
    assert!(atom.contains("/categories/systems/atom.xml"), "{atom}");
    assert!(p1.contains(
        "<link rel=\"alternate\" type=\"application/rss+xml\" title=\"Model — Systems\" href=\"/categories/systems/feed.xml\">"
    ));

    // Implicit tags: unchanged, no per-tag feeds.
    assert!(site.join("dist/tags/rust/index.html").exists());
    assert!(!site.join("dist/tags/rust/feed.xml").exists());

    let sitemap = read(&site, "sitemap.xml");
    for url in [
        "/categories/",
        "/categories/systems/",
        "/categories/systems/page/2/",
        "/categories/web/",
        "/series/rust-from-scratch/",
        "/tags/rust/",
    ] {
        assert!(sitemap.contains(url), "sitemap lacks {url}");
    }

    // Categories render on the post page next to tags.
    let post = read(&site, "posts/part-two.html");
    assert!(
        post.contains("<div class=\"tags terms-categories\">"),
        "{post}"
    );
    assert!(
        post.contains("<a href=\"/categories/web/\">Web</a>"),
        "{post}"
    );
}

#[test]
fn test_build_taxonomy_i18n_and_custom_prefix() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    add_language(&site, "es", "Modelo");
    append_config(
        &site,
        "\n[[taxonomies]]\nname = \"topics\"\nlabel = \"Topics\"\nurl_prefix = \"/topic\"\nfeed = false\n",
    );
    write_post(
        &site,
        "2024-01-01-hello.md",
        "title: Hello\ndate: 2024-01-01\ntopics: [Rust]",
    );
    write_post(
        &site,
        "2024-01-01-hello.es.md",
        "title: Hola\ndate: 2024-01-01\ntopics: Rust",
    );
    build(&site);

    assert!(site.join("dist/topic/rust/index.html").exists());
    let es = read(&site, "es/topic/rust/index.html");
    assert!(es.contains("Hola") && !es.contains(">Hello<"), "{es}");
    assert!(
        !site.join("dist/topic/rust/feed.xml").exists(),
        "feed = false"
    );
    let post = read(&site, "es/posts/hello.html");
    assert!(post.contains("href=\"/es/topic/rust/\""), "{post}");
}

#[test]
fn test_build_tag_feeds_opt_in() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(&site, "\n[[taxonomies]]\nname = \"tags\"\n");
    write_series_content(&site);
    build(&site);

    let rss = read(&site, "tags/rust/feed.xml");
    assert!(rss.contains("Model — Tags: rust"), "{rss}");
    assert!(site.join("dist/tags/rust/atom.xml").exists());
    let tag_page = read(&site, "tags/rust/index.html");
    assert!(
        tag_page.contains("href=\"/tags/rust/feed.xml\""),
        "{tag_page}"
    );
    // No `categories` taxonomy configured → no pages for it.
    assert!(!site.join("dist/categories").exists());
}

#[test]
fn test_build_related_posts_render_and_disable() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(&site, "\n[[taxonomies]]\nname = \"categories\"\n");
    write_series_content(&site);
    build(&site);

    let post = read(&site, "posts/part-one.html");
    let aside = post
        .split("<aside class=\"related\"")
        .nth(1)
        .unwrap_or_else(|| panic!("no related list: {post}"));
    // Part Two shares a category (2) and a tag (1); Part Three shares nothing.
    assert!(aside.contains("Part Two"), "{aside}");
    assert!(
        !aside.contains("Part Three") && !aside.contains("Other"),
        "{aside}"
    );
    let other = read(&site, "posts/other.html");
    assert!(
        !other.contains("class=\"related\""),
        "no shared terms: {other}"
    );

    // [build] related = 0 disables it everywhere.
    append_config(&site, "\n[build]\nrelated = 0\n");
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    // `[build]` already exists in init's config: merge into it instead.
    let config = config.replacen("\n[build]\nrelated = 0\n", "", 1).replacen(
        "[build]\n",
        "[build]\nrelated = 0\n",
        1,
    );
    fs::write(site.join("seite.toml"), config).unwrap();
    build(&site);
    assert!(!read(&site, "posts/part-one.html").contains("class=\"related\""));
}

#[test]
fn test_build_related_collection_override() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    let config = config.replacen("name = \"posts\"\n", "name = \"posts\"\nrelated = 0\n", 1);
    fs::write(site.join("seite.toml"), config).unwrap();
    write_series_content(&site);
    build(&site);
    assert!(!read(&site, "posts/part-two.html").contains("class=\"related\""));
}

#[test]
fn test_build_series_pages_and_box() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    write_series_content(&site);
    build(&site);

    let index = read(&site, "series/rust-from-scratch/index.html");
    let one = index.find("Part One").unwrap();
    let two = index.find("Part Two").unwrap();
    let three = index.find("Part Three").unwrap();
    assert!(one < two && two < three, "parts in order: {index}");
    assert!(!index.contains(">Other<"));

    let post = read(&site, "posts/part-two.html");
    assert!(post.contains("Part 2 of 3"), "{post}");
    assert!(
        post.contains("href=\"/series/rust-from-scratch/\""),
        "{post}"
    );
    assert!(
        post.contains("<a href=\"/posts/part-one\" rel=\"prev\">"),
        "{post}"
    );
    assert!(
        post.contains("<a href=\"/posts/part-three\" rel=\"next\">"),
        "{post}"
    );
    assert!(!read(&site, "posts/other.html").contains("series-nav"));
}

#[test]
fn test_build_no_new_output_without_new_config() {
    // A site with tags but no taxonomies/series config, and posts sharing no
    // tags: the output file set is exactly what earlier versions produced, and
    // no new markup appears.
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Compat", "posts");
    let site = tmp.path().join("site");
    // Configs from before full-text search have no `search` key (= "basic").
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace("search = \"full\"\n", ""),
    )
    .unwrap();
    remove_og_section(&site);
    for entry in fs::read_dir(site.join("content/posts")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    write_post(
        &site,
        "2024-01-01-a.md",
        "title: A\ndate: 2024-01-01\ntags: [alpha]\nunknown_key: kept",
    );
    write_post(
        &site,
        "2024-01-02-b.md",
        "title: B\ndate: 2024-01-02\ntags: [beta]",
    );
    build(&site);

    let mut files: Vec<String> = walkdir::WalkDir::new(site.join("dist"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(site.join("dist"))
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    files.sort();
    let expected = [
        "404.html",
        "atom.xml",
        "feed.xml",
        "index.html",
        "index.md",
        "llms-full.txt",
        "llms.txt",
        "posts/a.html",
        "posts/a.md",
        "posts/b.html",
        "posts/b.md",
        "posts/index.html",
        "posts/index.md",
        "robots.txt",
        "search-index.json",
        // Every site gets `sitemap.md` (0.21.0), with or without new config.
        "sitemap.md",
        "sitemap.xml",
        "tags/alpha/index.html",
        "tags/beta/index.html",
        "tags/index.html",
    ];
    let unexpected: Vec<&String> = files
        .iter()
        .filter(|f| !expected.contains(&f.as_str()) && !f.starts_with("static/"))
        .collect();
    assert!(unexpected.is_empty(), "new output files: {unexpected:?}");

    for page in ["posts/a.html", "posts/b.html", "tags/alpha/index.html"] {
        let html = read(&site, page);
        for marker in [
            "class=\"related\"",
            "series-nav",
            "terms-",
            "application/rss+xml\" title=\"Compat —",
        ] {
            assert!(!html.contains(marker), "{page} contains {marker}");
        }
    }
}

#[test]
fn test_build_content_model_renders_in_every_bundled_theme() {
    let themes = [
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
    for theme in themes {
        let tmp = TempDir::new().unwrap();
        let site = posts_site(&tmp);
        append_config(&site, "\n[[taxonomies]]\nname = \"categories\"\n");
        write_series_content(&site);
        page_cmd()
            .args(["theme", "apply", theme])
            .current_dir(&site)
            .assert()
            .success();
        build(&site);
        let post = read(&site, "posts/part-two.html");
        for marker in [
            "class=\"related\"",
            "series-nav",
            "terms-categories",
            "Part 2 of 3",
        ] {
            assert!(post.contains(marker), "{theme}: post lacks {marker}");
        }
        page_cmd()
            .args(["check", "--strict"])
            .current_dir(&site)
            .assert()
            .success();
    }
}

#[test]
fn test_stats_and_search_expose_taxonomies() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(&site, "\n[[taxonomies]]\nname = \"categories\"\n");
    write_series_content(&site);

    let (ok, doc) = run_json(&site, &["stats"]);
    assert!(ok, "{doc}");
    let taxes = doc["data"]["taxonomies"].as_array().unwrap();
    let cats = taxes.iter().find(|t| t["name"] == "categories").unwrap();
    assert_eq!(cats["terms"][0]["name"], "Systems");
    assert_eq!(cats["terms"][0]["count"], 2);
    assert_eq!(doc["data"]["series"][0]["parts"], 3);

    let (ok, doc) = run_json(&site, &["search", "Part Two"]);
    assert!(ok, "{doc}");
    let hit = &doc["data"]["results"][0];
    assert_eq!(hit["taxonomies"]["categories"][1], "Web", "{hit}");
    assert_eq!(hit["series"], "Rust from scratch");
}

#[test]
fn test_check_warns_on_unknown_taxonomy_key() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(
        &site,
        "\n[[taxonomies]]\nname = \"categories\"\nfeeds = true\n",
    );
    let (_, doc) = run_json(&site, &["check"]);
    let text = doc.to_string();
    assert!(text.contains("config-unknown-key"), "{text}");
    assert!(text.contains("feeds"), "{text}");
}

#[test]
fn test_build_lenient_series_frontmatter_and_frontmatter_set() {
    // Hugo-style list and string order must not fail the build.
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    write_post(
        &site,
        "2024-01-01-one.md",
        "title: One\ndate: 2024-01-01\nseries: [\"Rust intro\", \"Other\"]\nseries_order: \"2\"",
    );
    write_post(
        &site,
        "2024-01-02-two.md",
        "title: Two\ndate: 2024-01-02\nseries: rust intro\nseries_order: 1",
    );
    write_post(
        &site,
        "2024-01-03-odd.md",
        "title: Odd\ndate: 2024-01-03\nseries: {name: x}\nseries_order: soon",
    );
    build(&site);
    let one = read(&site, "posts/one.html");
    assert!(one.contains("Part 2 of 2"), "{one}");
    assert!(site.join("dist/series/rust-intro/index.html").exists());
    assert!(!read(&site, "posts/odd.html").contains("series-nav"));

    // Editing frontmatter of such a page works and keeps the list as written.
    page_cmd()
        .args([
            "frontmatter",
            "set",
            "content/posts/2024-01-01-one.md",
            "description=Hi",
        ])
        .current_dir(&site)
        .assert()
        .success();
    let source = fs::read_to_string(site.join("content/posts/2024-01-01-one.md")).unwrap();
    assert!(source.contains("description: Hi"), "{source}");
    assert!(source.contains("Rust intro"), "{source}");
}

#[test]
fn test_build_published_markdown_omits_unknown_keys() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(&site, "\n[[taxonomies]]\nname = \"categories\"\n");
    write_post(
        &site,
        "2024-01-01-a.md",
        "title: A\ndate: 2024-01-01\ncategories: [Web]\ninternal_note: secret",
    );
    build(&site);
    let md = read(&site, "posts/a.md");
    assert!(!md.contains("internal_note"), "{md}");
    assert!(
        md.contains("categories"),
        "configured taxonomy published: {md}"
    );

    // The source keeps unknown keys through a frontmatter edit.
    page_cmd()
        .args([
            "frontmatter",
            "set",
            "content/posts/2024-01-01-a.md",
            "description=x",
        ])
        .current_dir(&site)
        .assert()
        .success();
    let source = fs::read_to_string(site.join("content/posts/2024-01-01-a.md")).unwrap();
    assert!(source.contains("internal_note: secret"), "{source}");
    // `seite page` shows them (agents need to see the terms).
    let (ok, doc) = run_json(&site, &["page", "content/posts/2024-01-01-a.md"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["frontmatter"]["categories"][0], "Web");
}

#[test]
fn test_build_term_feeds_have_own_identity() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(&site, "\n[[taxonomies]]\nname = \"categories\"\n");
    write_series_content(&site);
    build(&site);
    let base = "http://localhost:3000";
    let systems = read(&site, "categories/systems/atom.xml");
    let web = read(&site, "categories/web/atom.xml");
    let site_atom = read(&site, "atom.xml");
    let id = |xml: &str| {
        xml.split("<id>")
            .nth(1)
            .and_then(|r| r.split("</id>").next())
            .unwrap()
            .to_string()
    };
    assert_eq!(id(&systems), format!("{base}/categories/systems/"));
    assert_ne!(id(&systems), id(&web));
    assert_ne!(id(&systems), id(&site_atom));
    assert!(systems.contains(&format!(
        "<link href=\"{base}/categories/systems/\" rel=\"alternate\"/>"
    )));
    let rss = read(&site, "categories/systems/feed.xml");
    assert!(
        rss.contains(&format!("<link>{base}/categories/systems/</link>")),
        "{rss}"
    );
}

#[test]
fn test_build_tag_links_follow_tags_url_prefix() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    append_config(
        &site,
        "\n[[taxonomies]]\nname = \"tags\"\nurl_prefix = \"topics\"\n",
    );
    write_series_content(&site);
    build(&site);
    let post = read(&site, "posts/part-one.html");
    assert!(
        post.contains("<a href=\"/topics/rust/\">rust</a>"),
        "{post}"
    );
    assert!(!post.contains("/tags/rust/"), "{post}");
    assert!(site.join("dist/topics/rust/index.html").exists());
}

#[test]
fn test_check_reports_taxonomy_url_collision() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    write_series_content(&site);
    append_config(
        &site,
        "\n[[taxonomies]]\nname = \"categories\"\nurl_prefix = \"posts\"\n[[taxonomies]]\nname = \"extra\"\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok, "{doc}");
    let text = doc.to_string();
    assert!(text.contains("taxonomy-url-collision"), "{text}");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("taxonomy-url-collision"));

    // A reserved name alone is a warning.
    let config = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        config.replace("url_prefix = \"posts\"\n", ""),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["check"]);
    let text = doc.to_string();
    assert!(text.contains("taxonomy-reserved-name"), "{text}");
    assert!(ok, "{doc}");
}

#[test]
fn test_check_warns_on_series_url_collision() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Model", "posts,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/pages/clash.md"),
        "---\ntitle: Clash\nslug: series/rust\n---\n\nPage.\n",
    )
    .unwrap();
    write_post(
        &site,
        "2024-01-01-a.md",
        "title: A\ndate: 2024-01-01\nseries: Rust",
    );
    let (_, doc) = run_json(&site, &["check"]);
    let text = doc.to_string();
    assert!(text.contains("series-url-collision"), "{text}");
}

#[test]
fn test_upgrade_refreshes_untouched_post_template() {
    let legacy = include_str!("../fixtures/legacy-post-0.20.html");
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Old", "posts");
    let site = tmp.path().join("site");
    fs::write(site.join("templates/post.html"), legacy).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("templates/post.html"));
    let updated = fs::read_to_string(site.join("templates/post.html")).unwrap();
    assert!(updated.contains("page.related"), "{updated}");

    // An edited copy is kept, with a notice.
    let edited = format!("{legacy}\n<!-- my footer -->\n");
    fs::write(site.join("templates/post.html"), &edited).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Kept templates/post.html"));
    assert_eq!(
        fs::read_to_string(site.join("templates/post.html")).unwrap(),
        edited
    );
}

// --- typed frontmatter schemas ---

/// A posts site whose schema requires `author`, bounds `rating`, enumerates
/// `status` (custom fields: top level or under `extra:`) and caps
/// `description`.
fn schema_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Schema", "posts");
    let site = tmp.path().join("site");
    let mut toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    // Replace the preset schema with a stricter one.
    let start = toml.find("[collections.fields.description]").unwrap();
    let end = toml.find("[build]").unwrap();
    toml.replace_range(
        start..end,
        r#"[collections.fields.description]
type = "string"
max = 20
description = "Short summary"

[collections.fields.author]
type = "string"
required = true
default = "Ada"
description = "Who wrote it"

[collections.fields.rating]
type = "integer"
min = 1
max = 5

[collections.fields.status]
type = "enum"
enum = ["draft", "published"]

"#,
    );
    fs::write(site.join("seite.toml"), toml).unwrap();
    // The sample post predates this schema.
    for entry in fs::read_dir(site.join("content/posts")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    site
}

const BAD_POST: &str = "---\ntitle: Bad\ndescription: this description is far too long\nextra:\n  rating: 9\n  status: pending\n---\nBody\n";

fn diagnostics_of(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    doc["data"]["diagnostics"]
        .as_array()
        .or_else(|| doc["error"]["diagnostics"].as_array())
        .cloned()
        .unwrap_or_default()
}

fn schema_diags(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    diagnostics_of(doc)
        .into_iter()
        .filter(|d| d["code"] == "frontmatter-schema")
        .collect()
}

fn assert_bad_post_diagnostics(diags: &[serde_json::Value], doc: &serde_json::Value) {
    let find = |needle: &str| {
        diags
            .iter()
            .find(|d| d["message"].as_str().unwrap().contains(needle))
            .unwrap_or_else(|| panic!("no diagnostic mentioning {needle}: {doc}"))
    };
    let desc = find("`description`");
    assert_eq!(desc["line"], 3, "{desc}");
    assert_eq!(desc["severity"], "error");
    assert!(desc["message"].as_str().unwrap().contains("too long"));
    assert!(desc["hint"].as_str().unwrap().starts_with("Short summary"));
    assert!(desc["file"]
        .as_str()
        .unwrap()
        .ends_with("content/posts/2024-01-01-bad.md"));
    let author = find("required field `author`");
    assert_eq!(author["line"], 4, "points at `extra:` {author}");
    assert!(author["hint"].as_str().unwrap().contains("Who wrote it"));
    assert_eq!(find("`extra.rating`")["line"], 5);
    let status = find("`extra.status`");
    assert_eq!(status["line"], 6);
    assert!(status["message"]
        .as_str()
        .unwrap()
        .contains("must be one of `draft`, `published`"));
    assert_eq!(diags.len(), 4, "{doc}");
}

#[test]
fn test_check_reports_frontmatter_schema_violations_with_lines() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    assert_bad_post_diagnostics(&schema_diags(&doc), &doc);

    // Human output is compiler-style with the hint.
    let output = page_cmd().arg("check").current_dir(&site).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("content/posts/2024-01-01-bad.md:5: error[frontmatter-schema]: field `extra.rating` is too large"),
        "{stderr}"
    );
}

#[test]
fn test_build_fails_on_frontmatter_schema_violations() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(!ok, "{doc}");
    assert_bad_post_diagnostics(&schema_diags(&doc), &doc);
}

#[test]
fn test_build_skips_schema_of_drafts_unless_included() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(
        &site,
        "content/posts/2024-01-01-bad.md",
        &BAD_POST.replace("title: Bad", "title: Bad\ndraft: true"),
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let (ok, doc) = run_json(&site, &["build", "--drafts"]);
    assert!(!ok, "{doc}");
}

#[test]
fn test_check_file_reports_frontmatter_schema() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let (ok, doc) = run_json(
        &site,
        &["check", "--file", "content/posts/2024-01-01-bad.md"],
    );
    assert!(!ok);
    assert_bad_post_diagnostics(&schema_diags(&doc), &doc);
}

#[test]
fn test_check_post_edit_hook_reports_frontmatter_schema() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(&site, "content/posts/2024-01-01-bad.md", BAD_POST);
    let path = site
        .join("content/posts/2024-01-01-bad.md")
        .to_string_lossy()
        .to_string();
    let output = page_cmd()
        .args(["check", "--hook", "claude", "--post-edit"])
        .current_dir(&site)
        .env_remove("CLAUDE_PROJECT_DIR")
        .write_stdin(
            serde_json::json!({ "tool_name": "Edit", "tool_input": { "file_path": path } })
                .to_string(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let context = doc["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        context.contains("2024-01-01-bad.md:6: error[frontmatter-schema]"),
        "{context}"
    );
}

#[test]
fn test_check_strict_fields_warns_about_unknown_keys() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replacen(
            "default_template = \"post.html\"\n",
            "default_template = \"post.html\"\nstrict_fields = true\n",
            1,
        ),
    )
    .unwrap();
    write_site_file(
        &site,
        "content/posts/2024-01-01-ok.md",
        "---\ntitle: Ok\ncolour: red\nextra:\n  author: Me\n  athor: typo\n---\nBody\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "warnings only: {doc}");
    let unknown: Vec<_> = diagnostics_of(&doc)
        .into_iter()
        .filter(|d| d["code"] == "frontmatter-schema-unknown")
        .collect();
    assert_eq!(unknown.len(), 2, "{doc}");
    assert_eq!(unknown[0]["line"], 3);
    assert_eq!(unknown[1]["line"], 6);
    assert_eq!(unknown[1]["hint"], "did you mean `author`?");
    let (ok, _) = run_json(&site, &["check", "--strict"]);
    assert!(!ok);
}

#[test]
fn test_check_config_rejects_invalid_field_schema_with_line() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace("enum = [\"draft\", \"published\"]\n", "requird = true\n"),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    let d = &diagnostics_of(&doc)[0];
    assert_eq!(d["code"], "config-invalid", "{doc}");
    assert!(d["message"]
        .as_str()
        .unwrap()
        .contains("[collections.fields.status]"));
    let line = d["line"].as_u64().unwrap() as usize;
    let text = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert_eq!(
        text.lines().nth(line - 1).unwrap(),
        "[collections.fields.status]"
    );

    // With the enum fixed, the typo is an unknown-key warning.
    fs::write(
        site.join("seite.toml"),
        text.replace("requird = true\n", "enum = [\"a\"]\nrequird = true\n"),
    )
    .unwrap();
    let (_, doc) = run_json(&site, &["check"]);
    let unknown: Vec<_> = diagnostics_of(&doc)
        .into_iter()
        .filter(|d| d["code"] == "config-unknown-key")
        .collect();
    assert_eq!(unknown.len(), 1, "{doc}");
    assert_eq!(unknown[0]["hint"], "did you mean `required`?");
}

#[test]
fn test_new_writes_schema_defaults_and_placeholders() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace(
            "[collections.fields.rating]\ntype = \"integer\"\n",
            "[collections.fields.rating]\ntype = \"integer\"\nrequired = true\n",
        ),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["new", "post", "Hello Schema"]);
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["fill_in"], serde_json::json!(["rating"]));
    let path = doc["data"]["path"].as_str().unwrap();
    let text = fs::read_to_string(path).unwrap();
    assert!(
        text.contains("extra:\n  author: Ada\n  rating: null\n"),
        "{text}"
    );
    let problems = doc["data"]["schema_problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{doc}");
    assert!(problems[0]["message"]
        .as_str()
        .unwrap()
        .contains("missing required field `rating`"));
    // `seite check` points at the same field until it is filled in.
    let (ok, _) = run_json(&site, &["check"]);
    assert!(!ok);
}

#[test]
fn test_mcp_create_content_validates_schema() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_create_content",
        serde_json::json!({
            "collection": "posts",
            "title": "Rated",
            "extra": { "rating": 11 }
        }),
    );
    assert!(is_error, "{text}");
    assert!(text.contains("field `extra.rating` is too large"), "{text}");
    assert!(text.contains("nothing was written"), "{text}");
    assert!(!site.join("content/posts").read_dir().unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("rated")));

    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_create_content",
        serde_json::json!({
            "collection": "posts",
            "title": "Rated",
            "extra": { "rating": 4, "status": "draft" }
        }),
    );
    assert!(!is_error, "{text}");
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    let file = fs::read_to_string(site.join(result["path"].as_str().unwrap())).unwrap();
    assert!(file.contains("author: Ada"), "default applied: {file}");
}

#[test]
fn test_mcp_update_frontmatter_validates_schema_and_routes_custom_fields() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    write_site_file(
        &site,
        "content/posts/2024-01-01-p.md",
        "---\ntitle: P\nextra:\n  author: Me\n---\nBody\n",
    );
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "set": { "status": "pending" } }),
    );
    assert!(is_error, "{text}");
    assert!(
        text.contains("must be one of `draft`, `published`"),
        "{text}"
    );

    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "set": { "status": "published" } }),
    );
    assert!(!is_error, "{text}");
    assert!(text.contains("stored as `extra.status`"), "{text}");
    let file = fs::read_to_string(site.join("content/posts/2024-01-01-p.md")).unwrap();
    assert!(file.contains("status: published"), "{file}");
    assert!(!file.contains("\nstatus:"), "not at the top level: {file}");

    // Removing a required field is refused.
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "unset": ["author"] }),
    );
    assert!(is_error, "{text}");
    assert!(text.contains("missing required field `author`"), "{text}");
}

#[test]
fn test_build_custom_schema_fields_at_top_level_or_under_extra() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let _ = fs::remove_file(site.join("templates/post.html"));
    write_site_file(
        &site,
        "templates/post.html",
        "{% extends \"base.html\" %}{% block content %}<p id=\"by\">{{ page.extra.author }}|{{ page.extra.rating | default(value='-') }}|{{ page.extra.mood | default(value='none') }}</p>{% endblock %}",
    );
    // Custom fields at the top level: validated there, readable as page.extra.
    write_site_file(
        &site,
        "content/posts/2024-01-01-top.md",
        "---\ntitle: Top\nauthor: Ada\nrating: 4\nmood: sunny\n---\nBody\n",
    );
    write_site_file(
        &site,
        "content/posts/2024-01-02-nested.md",
        "---\ntitle: Nested\nextra:\n  author: Bob\n---\nBody\n",
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let html = fs::read_to_string(site.join("dist/posts/top.html")).unwrap();
    assert!(html.contains("<p id=\"by\">Ada|4|none</p>"), "{html}");
    let html = fs::read_to_string(site.join("dist/posts/nested.html")).unwrap();
    assert!(html.contains("<p id=\"by\">Bob|-|none</p>"), "{html}");
    // Published markdown keeps declared custom fields where they were
    // written, never undeclared keys.
    let md = fs::read_to_string(site.join("dist/posts/top.md")).unwrap();
    assert!(
        md.contains("\nauthor: Ada\n") && md.contains("\nrating: 4\n"),
        "{md}"
    );
    assert!(!md.contains("mood"), "{md}");
    let md = fs::read_to_string(site.join("dist/posts/nested.md")).unwrap();
    assert!(md.contains("extra:\n  author: Bob"), "{md}");

    // Invalid at the top level: reported at that line.
    write_site_file(
        &site,
        "content/posts/2024-01-01-top.md",
        "---\ntitle: Top\nauthor: Ada\nrating: 9\n---\nBody\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    let diags = schema_diags(&doc);
    assert_eq!(diags.len(), 1, "{doc}");
    assert_eq!(diags[0]["line"], 4, "{doc}");
    assert!(diags[0]["message"]
        .as_str()
        .unwrap()
        .contains("field `rating` is too large"));

    // In both places: one error naming both lines.
    write_site_file(
        &site,
        "content/posts/2024-01-01-top.md",
        "---\ntitle: Top\nauthor: Ada\nextra:\n  author: Bob\n---\nBody\n",
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(!ok);
    let diags = schema_diags(&doc);
    assert_eq!(diags.len(), 1, "{doc}");
    let message = diags[0]["message"].as_str().unwrap();
    assert!(
        message.contains("set twice") && message.contains("line 3") && message.contains("line 5"),
        "{message}"
    );

    // MCP keeps a top-level custom key at the top level.
    write_site_file(
        &site,
        "content/posts/2024-01-01-top.md",
        "---\ntitle: Top\nauthor: Ada\n---\nBody\n",
    );
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-top.md", "set": { "author": "Grace", "rating": 2 } }),
    );
    assert!(!is_error, "{text}");
    let file = fs::read_to_string(site.join("content/posts/2024-01-01-top.md")).unwrap();
    assert!(
        file.contains("\nauthor: Grace\n"),
        "edited in place: {file}"
    );
    assert!(
        file.contains("extra:\n  rating: 2"),
        "new key under extra: {file}"
    );
}

#[test]
fn test_templates_and_context_include_collection_schemas() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let (ok, doc) = run_json(&site, &["templates"]);
    assert!(ok, "{doc}");
    let posts = &doc["data"]["collection_schemas"][0];
    assert_eq!(posts["collection"], "posts");
    let author = posts["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "author")
        .unwrap();
    assert_eq!(author["location"], "extra.author");
    assert_eq!(author["required"], true);
    let (ok, doc) = run_json(&site, &["context"]);
    assert!(ok, "{doc}");
    assert_eq!(
        doc["data"]["collections"][0]["schema"]["fields"][1]["name"],
        "author"
    );
}

#[test]
fn test_init_writes_preset_schemas_and_site_checks_clean() {
    let tmp = TempDir::new().unwrap();
    init_site(
        &tmp,
        "site",
        "Presets",
        "posts,docs,pages,changelog,roadmap",
    );
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(toml.contains("[collections.fields.description]\ntype = \"string\"\ndescription ="));
    assert!(
        !toml.contains("max ="),
        "presets have no length bounds: {toml}"
    );
    assert!(toml.contains("\nschedule = true\n"), "{toml}");
    // Scalars in tags and long descriptions pass the preset schema.
    write_site_file(
        &site,
        "content/posts/2024-01-01-scalars.md",
        &format!(
            "---\ntitle: Scalars\ndescription: \"{}\"\ntags: [2024, rust, true]\n---\nBody\n",
            "x".repeat(400)
        ),
    );
    assert!(toml.contains("[collections.fields.weight]"));
    assert!(
        !toml.contains("required"),
        "presets require nothing: {toml}"
    );
    let (ok, doc) = run_json(&site, &["check", "--strict"]);
    assert!(ok, "{doc}");
}

#[test]
fn test_upgrade_does_not_add_schemas() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Old", "posts");
    let site = tmp.path().join("site");
    // A site from before schemas: no [collections.fields.*] tables.
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    let start = toml.find("[collections.fields.").unwrap();
    let end = toml.find("[build]").unwrap();
    let mut old = toml.clone();
    old.replace_range(start..end, "");
    fs::write(site.join("seite.toml"), &old).unwrap();
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success();
    let after = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(!after.contains("[collections.fields."), "{after}");
}

// --- scheduled publishing ---

fn scheduled_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "Sched", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2999-01-01-futurepost.md",
        "---\ntitle: Futurepost\ndescription: Futuredesc\ntags: [futuretag]\n---\nFuturebody\n",
    );
    write_site_file(
        &site,
        "content/posts/oldpost.md",
        "---\ntitle: Oldpost\ndate: 2001-01-01\nexpires: 2002-01-01\ndescription: Olddesc\ntags: [oldtag]\n---\nOldbody\n",
    );
    write_site_file(
        &site,
        "content/posts/2020-01-01-livepost.md",
        "---\ntitle: Livepost\ndate: 2020-01-01\nexpires: 2999-01-01\ntags: [livetag]\n---\nLivebody\n",
    );
    site
}

/// Every file under `dist` whose contents mention `needle`.
fn files_mentioning(dist: &std::path::Path, needle: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dist.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if fs::read(&path)
                .map(|b| String::from_utf8_lossy(&b).contains(needle))
                .unwrap_or(false)
                || path.to_string_lossy().contains(needle)
            {
                found.push(path.strip_prefix(dist).unwrap().display().to_string());
            }
        }
    }
    found
}

#[test]
fn test_build_excludes_future_and_expired_content_from_every_output() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let dist = site.join("dist");
    for needle in [
        "futurepost",
        "Futurepost",
        "futuretag",
        "Oldpost",
        "oldpost",
        "oldtag",
    ] {
        let found = files_mentioning(&dist, needle);
        assert!(found.is_empty(), "{needle} leaked into {found:?}");
    }
    // The live post is everywhere: page, markdown, feeds, sitemap, search, llms, tags.
    let live = files_mentioning(&dist, "livepost");
    for expected in [
        "posts/livepost.html",
        "posts/livepost.md",
        "feed.xml",
        "atom.xml",
        "sitemap.xml",
        "search-index.json",
        "llms.txt",
        "llms-full.txt",
    ] {
        assert!(
            live.iter().any(|f| f == expected),
            "{expected} missing from {live:?}"
        );
    }
    assert!(!files_mentioning(&dist, "livetag").is_empty());
}

#[test]
fn test_build_future_flag_and_config_include_scheduled_content() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let (ok, doc) = run_json(&site, &["build", "--future"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/futurepost.html").exists());
    assert!(!files_mentioning(&site.join("dist"), "futuretag").is_empty());
    assert!(
        !site.join("dist/posts/oldpost.html").exists(),
        "expired stays out"
    );

    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        toml.replace("[build]\n", "[build]\nfuture = true\n"),
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/futurepost.html").exists());
}

#[test]
fn test_build_timezone_decides_what_today_is() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tz", "posts");
    let site = tmp.path().join("site");
    // Without the zone `seite init` detected.
    let toml: String = fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("timezone = "))
        .map(|l| format!("{l}\n"))
        .collect();
    let with_tz = |tz: &str| {
        fs::write(
            site.join("seite.toml"),
            toml.replace(
                "author = \"\"\n",
                &format!("author = \"\"\ntimezone = \"{tz}\"\n"),
            ),
        )
        .unwrap();
    };
    // Today at UTC+14 is always a later date than today at UTC-12.
    with_tz("+14:00");
    let (_, stats) = run_json(&site, &["stats"]);
    let east_today = stats["data"]["today"].as_str().unwrap().to_string();
    write_site_file(
        &site,
        "content/posts/tzpost.md",
        &format!("---\ntitle: Tzpost\ndate: {east_today}\n---\nx\n"),
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/tzpost.html").exists());
    with_tz("-12:00");
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(!site.join("dist/posts/tzpost.html").exists());
    // IANA names work; nonsense is a located config error.
    with_tz("Pacific/Kiritimati");
    assert!(run_json(&site, &["build"]).0);
    with_tz("Mars/Olympus");
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    let d = &diagnostics_of(&doc)[0];
    assert_eq!(d["code"], "config-invalid");
    assert!(d["message"].as_str().unwrap().contains("IANA"), "{d}");
    assert_eq!(d["line"], 7, "{d}");
}

#[test]
fn test_check_renders_scheduled_content() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Sched", "posts");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/posts/2999-01-01-later.md",
        "---\ntitle: Later\n---\n\n{{< youtub(id=\"x\") >}}\n",
    );
    // The build leaves the scheduled post out, so it passes...
    assert!(run_json(&site, &["build"]).0);
    // ...but check validates it before the cron publishes it.
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    assert!(diagnostics_of(&doc)
        .iter()
        .any(|d| d["code"] == "shortcode-unknown"));
}

#[test]
fn test_stats_and_context_report_expired_and_next_scheduled() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let (ok, doc) = run_json(&site, &["stats"]);
    assert!(ok, "{doc}");
    let data = &doc["data"];
    assert_eq!(data["totals"]["future_dated"], 1);
    assert_eq!(data["totals"]["expired"], 1);
    assert_eq!(data["collections"][0]["expired"][0]["date"], "2002-01-01");
    assert_eq!(data["next_scheduled"]["title"], "Futurepost");
    assert_eq!(data["next_scheduled"]["date"], "2999-01-01");
    let output = page_cmd()
        .arg("context")
        .current_dir(&site)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("next scheduled: 2999-01-01 (Futurepost)"),
        "{stdout}"
    );
}

// --- the daily cron in the generated workflow ---

#[test]
fn test_init_workflow_has_daily_cron() {
    // Netlify gets no workflow (netlify.toml only; Netlify builds on push).
    for target in ["github-pages", "cloudflare"] {
        let tmp = TempDir::new().unwrap();
        page_cmd()
            .args([
                "init",
                "site",
                "--title",
                "T",
                "--description",
                "",
                "--deploy-target",
                target,
                "--collections",
                "posts",
            ])
            .current_dir(tmp.path())
            .assert()
            .success();
        let wf = fs::read_to_string(tmp.path().join("site/.github/workflows/deploy.yml")).unwrap();
        assert!(wf.contains("  schedule:\n"), "{target}: {wf}");
        assert!(wf.contains("- cron: '17 5 * * *'"), "{target}");
    }
}

fn set_project_version(site: &std::path::Path, version: &str) {
    let path = site.join(".seite/config.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    meta["version"] = version.into();
    fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

const CRON_BLOCK: &str = "  schedule:\n    # Daily rebuild: publishes scheduled posts (future `date:`) and removes expired ones\n    - cron: '17 5 * * *'\n";

#[test]
fn test_upgrade_adds_cron_to_unmodified_workflow() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "T", "posts");
    let site = tmp.path().join("site");
    let wf_path = site.join(".github/workflows/deploy.yml");
    let generated = fs::read_to_string(&wf_path).unwrap();
    assert!(generated.contains(CRON_BLOCK));
    fs::write(&wf_path, generated.replace(CRON_BLOCK, "")).unwrap();
    set_project_version(&site, "0.20.1");
    page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&wf_path).unwrap(), generated);
}

#[test]
fn test_upgrade_leaves_modified_workflow_and_explains_the_cron() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "T", "posts");
    let site = tmp.path().join("site");
    let wf_path = site.join(".github/workflows/deploy.yml");
    let edited = fs::read_to_string(&wf_path)
        .unwrap()
        .replace(CRON_BLOCK, "")
        .replace("Build site", "Build my site");
    fs::write(&wf_path, &edited).unwrap();
    set_project_version(&site, "0.20.1");
    let output = page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&wf_path).unwrap(), edited);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains("- cron: '17 5 * * *'"), "{all}");
}

// --- seite check --external ---

/// Local HTTP server: /ok 200, /missing 404, /slow (sleeps 3 s).
fn link_server() -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            std::thread::spawn(move || {
                let code = match req.url() {
                    "/ok" => 200,
                    "/slow" => {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        200
                    }
                    _ => 404,
                };
                let _ = req.respond(tiny_http::Response::empty(code));
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// `run_json` with `SEITE_EXTERNAL_PRIVATE=1`: the link server is on 127.0.0.1,
/// which `check --external` otherwise never requests.
fn run_json_private(dir: &std::path::Path, args: &[&str]) -> (bool, serde_json::Value) {
    let output = page_cmd()
        .env("SEITE_EXTERNAL_PRIVATE", "1")
        .arg("--json")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    let doc = serde_json::from_slice(&output.stdout).unwrap();
    (output.status.success(), doc)
}

#[test]
fn test_check_external_reports_broken_links_with_source_lines() {
    let base = link_server();
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Ext", "posts");
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    fs::write(
        site.join("seite.toml"),
        format!("{toml}\n[check]\nexternal_timeout = 1\nexternal_skip = [\"skipme.test\", \"fonts.googleapis.com\"]\n"),
    )
    .unwrap();
    write_site_file(
        &site,
        "content/posts/2024-01-01-links.md",
        &format!(
            "---\ntitle: Links\n---\n\nIntro.\n\n[ok]({base}/ok)\n[bad]({base}/missing)\n[slow]({base}/slow)\n[li](https://www.linkedin.com/in/someone)\n[skip](https://docs.skipme.test/x)\n"
        ),
    );
    // Not run by default (no network, no cache).
    let (ok, _) = run_json(&site, &["check"]);
    assert!(ok);
    assert!(!site.join(".seite/cache/links.json").exists());

    let (ok, doc) = run_json_private(&site, &["check", "--external"]);
    assert!(ok, "warnings only: {doc}");
    let diags = diagnostics_of(&doc);
    let broken = diags
        .iter()
        .find(|d| d["code"] == "external-link-broken")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(broken["file"], "content/posts/2024-01-01-links.md");
    assert_eq!(broken["line"], 8);
    assert_eq!(broken["severity"], "warning");
    assert!(broken["message"].as_str().unwrap().contains("404"));
    let slow = diags
        .iter()
        .find(|d| d["code"] == "external-link-unreachable")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(slow["line"], 9);
    assert!(slow["message"].as_str().unwrap().contains("timed out"));
    let ext = &doc["data"]["external"];
    assert!(
        ext["skipped"].as_u64().unwrap() >= 3,
        "linkedin + skip list + own URLs: {ext}"
    );
    assert_eq!(
        ext["checked"], 3,
        "only the local server is requested: {ext}"
    );
    assert_eq!(ext["broken"], 1);
    assert_eq!(ext["unreachable"], 1);

    let cache = fs::read_to_string(site.join(".seite/cache/links.json")).unwrap();
    assert!(cache.contains(&format!("{base}/ok")), "{cache}");
    let gitignore = fs::read_to_string(site.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/.seite/cache/"));

    // Second run: the working link comes from the cache; --strict fails.
    let (ok, doc) = run_json_private(&site, &["check", "--external", "--strict"]);
    assert!(!ok);
    assert_eq!(doc["data"]["external"]["cached"], 1, "{doc}");
    assert!(diagnostics_of(&doc)
        .iter()
        .any(|d| d["code"] == "external-link-broken" && d["severity"] == "error"));
}

// --- review follow-ups: opt-in scheduling, local clock, links, MCP extra ---

#[test]
fn test_build_existing_site_without_schedule_key_publishes_future_posts() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(toml.contains("schedule = true"));
    // A site from before scheduling: no `schedule` key.
    let old: String = toml
        .lines()
        .filter(|l| !l.contains("schedule = true") && !l.starts_with("# "))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(site.join("seite.toml"), old).unwrap();
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    assert!(site.join("dist/posts/futurepost.html").exists());
    assert!(
        site.join("dist/posts/oldpost.html").exists(),
        "expires follows the switch"
    );
    // Upgrade does not turn it on but explains it once.
    let output = page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success());
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains("Scheduled publishing is available"), "{all}");
    assert!(!fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .contains("schedule = true"));
    let output = page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .output()
        .unwrap();
    let again = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !again.contains("Scheduled publishing is available"),
        "once: {again}"
    );
}

#[test]
fn test_build_undated_collections_are_never_scheduled() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Docs", "posts,docs");
    let site = tmp.path().join("site");
    write_site_file(
        &site,
        "content/docs/later.md",
        "---\ntitle: Later\ndate: 2999-01-01\nexpires: 2000-01-01\n---\nx\n",
    );
    assert!(run_json(&site, &["build"]).0);
    assert!(site.join("dist/docs/later.html").exists());
}

#[test]
fn test_init_and_build_publish_starter_content_in_any_local_timezone() {
    for tz in ["Pacific/Kiritimati", "Pacific/Pago_Pago"] {
        let tmp = TempDir::new().unwrap();
        page_cmd()
            .env("TZ", tz)
            .args([
                "init",
                "site",
                "--title",
                "T",
                "--description",
                "",
                "--deploy-target",
                "github-pages",
                "--collections",
                "posts,changelog",
            ])
            .current_dir(tmp.path())
            .assert()
            .success();
        let site = tmp.path().join("site");
        let output = page_cmd()
            .env("TZ", tz)
            .args(["--json", "build"])
            .current_dir(&site)
            .output()
            .unwrap();
        assert!(output.status.success(), "{tz}");
        let posts: Vec<_> = fs::read_dir(site.join("dist/posts")).unwrap().collect();
        assert!(
            site.join("dist/posts/hello-world.html").exists(),
            "{tz}: starter post must be built ({posts:?})"
        );
        let changelog = fs::read_dir(site.join("dist/changelog"))
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name().to_string_lossy().ends_with(".html") && e.file_name() != "index.html"
            });
        assert!(changelog, "{tz}: starter changelog entry must be built");
        // `seite new` dates with the same clock: the new post is live too.
        let output = page_cmd()
            .env("TZ", tz)
            .args(["new", "post", "Fresh"])
            .current_dir(&site)
            .output()
            .unwrap();
        assert!(output.status.success());
        page_cmd()
            .env("TZ", tz)
            .arg("build")
            .current_dir(&site)
            .assert()
            .success();
        assert!(site.join("dist/posts/fresh.html").exists(), "{tz}");
    }
}

fn init_with_tz(tmp: &TempDir, tz: &str) -> std::path::PathBuf {
    page_cmd()
        .env("TZ", tz)
        .args([
            "init",
            "site",
            "--title",
            "T",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp.path().join("site")
}

#[test]
fn test_init_writes_detected_timezone() {
    for (tz, expected) in [
        ("Asia/Tokyo", "Asia/Tokyo"),
        (":America/New_York", "America/New_York"),
        ("/usr/share/zoneinfo/Europe/Paris", "Europe/Paris"),
    ] {
        let tmp = TempDir::new().unwrap();
        let site = init_with_tz(&tmp, tz);
        let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
        let line = format!("\ntimezone = \"{expected}\"\n");
        assert!(toml.contains(&line), "{tz}: {toml}");
        assert!(
            toml.contains("identical on every machine and in CI"),
            "{toml}"
        );
        // Under [site], and the site loads and checks cleanly.
        let site_table = toml.split("[[collections]]").next().unwrap();
        assert!(site_table.contains(&line), "{toml}");
        let (ok, doc) = run_json(&site, &["check"]);
        assert!(ok, "{tz}: {doc}");
    }
}

#[test]
fn test_init_unknown_timezone_writes_commented_line() {
    let tmp = TempDir::new().unwrap();
    let site = init_with_tz(&tmp, "Not/AZone");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(
        toml.contains("\n# timezone = \"Europe/Berlin\"\n"),
        "{toml}"
    );
    assert!(!toml.contains("\ntimezone = "), "{toml}");
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "{doc}");
}

#[test]
fn test_upgrade_recommends_timezone_once_when_scheduling_without_one() {
    let tmp = TempDir::new().unwrap();
    let site = init_with_tz(&tmp, "Not/AZone");
    let upgrade = || {
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
    let first = upgrade();
    assert!(first.contains("without [site] timezone"), "{first}");
    assert!(!fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .contains("\ntimezone = "));
    assert!(!upgrade().contains("without [site] timezone"), "once");

    // With a timezone: no notice.
    let tmp = TempDir::new().unwrap();
    let site = init_with_tz(&tmp, "Asia/Tokyo");
    let output = page_cmd()
        .args(["upgrade", "--force"])
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains("without [site] timezone"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("without [site] timezone"));
}

#[test]
fn test_check_and_build_agree_on_links_to_scheduled_pages() {
    let tmp = TempDir::new().unwrap();
    let site = scheduled_site(&tmp);
    write_site_file(
        &site,
        "content/posts/2020-02-02-teaser.md",
        "---\ntitle: Teaser\n---\nIntro.\n\nComing: [soon](/posts/futurepost)\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "warning only: {doc}");
    let d = diagnostics_of(&doc)
        .into_iter()
        .find(|d| d["code"] == "link-to-scheduled")
        .unwrap_or_else(|| panic!("{doc}"));
    assert_eq!(d["file"], "content/posts/2020-02-02-teaser.md");
    assert_eq!(d["line"], 6);
    assert!(
        d["message"]
            .as_str()
            .unwrap()
            .contains("goes live on 2999-01-01"),
        "{d}"
    );
    // Human output: the info note (not a diagnostic).
    let output = page_cmd().arg("check").current_dir(&site).output().unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        all.contains("1 scheduled post hidden; next goes live 2999-01-01 (Futurepost)"),
        "{all}"
    );
    // --strict: check and build both fail on it.
    let (ok, doc) = run_json(&site, &["check", "--strict"]);
    assert!(!ok);
    assert!(diagnostics_of(&doc)
        .iter()
        .any(|d| d["code"] == "link-to-scheduled"));
    let (ok, doc) = run_json(&site, &["build", "--strict"]);
    assert!(!ok, "{doc}");
    assert!(
        diagnostics_of(&doc)
            .iter()
            .any(|d| d["code"] == "link-to-scheduled"),
        "{doc}"
    );
    // With --future the link resolves in both.
    let (ok, doc) = run_json(&site, &["check", "--strict", "--future"]);
    assert!(ok, "{doc}");
    let (ok, doc) = run_json(&site, &["build", "--strict", "--future"]);
    assert!(ok, "{doc}");
    // Build prints the hidden-post line.
    let output = page_cmd().arg("build").current_dir(&site).output().unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains("1 scheduled post hidden; next goes live 2999-01-01 (Futurepost); 1 expired post hidden"), "{all}");
}

#[test]
fn test_mcp_update_frontmatter_refuses_unconvertible_extra() {
    let tmp = TempDir::new().unwrap();
    let site = schema_site(&tmp);
    let original = "---\ntitle: P\nextra:\n  author: Me\n  1: numeric key\n---\nBody\n";
    write_site_file(&site, "content/posts/2024-01-01-p.md", original);
    let (is_error, text) = mcp_tool_call(
        &site,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "content/posts/2024-01-01-p.md", "set": { "status": "draft" } }),
    );
    assert!(is_error, "{text}");
    assert!(text.contains("nothing was written"), "{text}");
    assert_eq!(
        fs::read_to_string(site.join("content/posts/2024-01-01-p.md")).unwrap(),
        original
    );
}

// --- scheduling × taxonomies, series and related pages ---

#[test]
fn test_build_scheduled_posts_stay_out_of_terms_series_and_related() {
    let tmp = TempDir::new().unwrap();
    let site = posts_site(&tmp);
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    if !toml.contains("schedule = true") {
        fs::write(
            site.join("seite.toml"),
            toml.replacen("[build]\n", "[build]\nschedule = true\n", 1),
        )
        .unwrap();
    }
    append_config(&site, "\n[[taxonomies]]\nname = \"categories\"\n");
    let shared = "tags: [rust]\ncategories: [Systems]\nseries: Rust from scratch";
    write_post(
        &site,
        "2024-01-01-part-one.md",
        &format!("title: Part One\ndate: 2024-01-01\n{shared}\nseries_order: 1"),
    );
    write_post(
        &site,
        "2024-02-01-part-two.md",
        &format!("title: Part Two\ndate: 2024-02-01\n{shared}\nseries_order: 2"),
    );
    // Scheduled (future date) and expired posts sharing every term.
    write_post(
        &site,
        "2099-01-01-future-hidden.md",
        &format!("title: Future Hidden\ndate: 2099-01-01\n{shared}\nseries_order: 3"),
    );
    write_post(
        &site,
        "2024-03-01-expired-hidden.md",
        &format!("title: Expired Hidden\ndate: 2024-03-01\nexpires: 2024-06-01\n{shared}"),
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    // No broken links either (series boxes only link published parts).
    assert!(
        !diagnostics_of(&doc)
            .iter()
            .any(|d| d["code"] == "broken-link" || d["code"] == "link-to-scheduled"),
        "{doc}"
    );

    let dist = site.join("dist");
    // The outputs exist and list the published posts...
    let term = read(&site, "categories/systems/index.html");
    assert!(
        term.contains("Part One") && term.contains("Part Two"),
        "{term}"
    );
    assert!(read(&site, "categories/systems/feed.xml").contains("Part Two"));
    assert!(read(&site, "tags/rust/index.html").contains("Part Two"));
    let series = read(&site, "series/rust-from-scratch/index.html");
    assert!(series.contains("Part Two"), "{series}");
    let one = read(&site, "posts/part-one.html");
    assert!(
        one.contains("Part 1 of 2"),
        "series box counts only published parts: {one}"
    );
    assert!(one.contains("href=\"/posts/part-two\""), "related: {one}");
    // ...and nothing anywhere mentions the hidden ones.
    for needle in ["Hidden", "future-hidden", "expired-hidden"] {
        let found = files_mentioning(&dist, needle);
        assert!(found.is_empty(), "{needle} leaked into {found:?}");
    }

    // `--future` brings the scheduled post back everywhere.
    let (ok, doc) = run_json(&site, &["build", "--future"]);
    assert!(ok, "{doc}");
    assert!(read(&site, "categories/systems/index.html").contains("Future Hidden"));
    assert!(read(&site, "categories/systems/feed.xml").contains("Future Hidden"));
    assert!(read(&site, "series/rust-from-scratch/index.html").contains("Future Hidden"));
    assert!(read(&site, "posts/part-one.html").contains("Part 1 of 3"));
}
