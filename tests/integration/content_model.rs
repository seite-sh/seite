//! Taxonomies, related posts and series (`src/build/{taxonomy,related,series}.rs`).

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
