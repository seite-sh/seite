use super::common::*;

#[test]
fn test_index_shows_listed_only() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Index Test", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc and a page
    let doc = "---\ntitle: A Doc\n---\n\nDoc.\n";
    fs::write(site_dir.join("content/docs/a-doc.md"), doc).unwrap();

    let pg = "---\ntitle: About\n---\n\nAbout page.\n";
    fs::write(site_dir.join("content/pages/about.md"), pg).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Posts and docs are listed
    assert!(index.contains("Hello World"));
    assert!(index.contains("A Doc"));
    // Pages collection is not listed (listed: false)
    assert!(!index.contains("About"));
}

// --- homepage as special page ---

#[test]
fn test_build_homepage_with_index_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Home Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create content/pages/index.md as the homepage
    let homepage = "---\ntitle: Welcome Home\n---\n\nThis is **hero content** for the homepage.\n";
    fs::write(site_dir.join("content/pages/index.md"), homepage).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Homepage content should appear
    assert!(index.contains("hero content"));
    assert!(index.contains("homepage-content"));
    // Collection listing should still work
    assert!(index.contains("Hello World"));

    // Homepage markdown should also be output
    assert!(site_dir.join("dist/index.md").exists());
}

#[test]
fn test_build_homepage_without_index_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Home Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Collection listing should work as before
    assert!(index.contains("Hello World"));
    // No homepage-content div since there's no index.md
    assert!(!index.contains("class=\"homepage-content\""));
}

// --- tag pages ---

#[test]
fn test_build_generates_tag_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tagsite", "Tag Site", "posts");
    let site_dir = tmp.path().join("tagsite");

    // Create posts with tags
    fs::write(
        site_dir.join("content/posts/2025-01-15-alpha.md"),
        "---\ntitle: Alpha Post\ndate: 2025-01-15\ntags:\n  - rust\n  - web\n---\nAlpha content.",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2025-01-16-beta.md"),
        "---\ntitle: Beta Post\ndate: 2025-01-16\ntags:\n  - rust\n  - cli\n---\nBeta content.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Tag index page should exist
    let tags_index = site_dir.join("dist/tags/index.html");
    assert!(tags_index.exists(), "tags index page should exist");
    let tags_html = fs::read_to_string(&tags_index).unwrap();
    assert!(tags_html.contains("rust"), "tags index should list 'rust'");
    assert!(tags_html.contains("web"), "tags index should list 'web'");
    assert!(tags_html.contains("cli"), "tags index should list 'cli'");

    // Individual tag pages should exist
    let rust_tag = site_dir.join("dist/tags/rust/index.html");
    assert!(rust_tag.exists(), "rust tag page should exist");
    let rust_html = fs::read_to_string(&rust_tag).unwrap();
    assert!(
        rust_html.contains("Alpha Post"),
        "rust tag should list Alpha Post"
    );
    assert!(
        rust_html.contains("Beta Post"),
        "rust tag should list Beta Post"
    );

    let web_tag = site_dir.join("dist/tags/web/index.html");
    assert!(web_tag.exists(), "web tag page should exist");
    let web_html = fs::read_to_string(&web_tag).unwrap();
    assert!(
        web_html.contains("Alpha Post"),
        "web tag should list Alpha Post"
    );
    assert!(
        !web_html.contains("Beta Post"),
        "web tag should NOT list Beta Post"
    );

    let cli_tag = site_dir.join("dist/tags/cli/index.html");
    assert!(cli_tag.exists(), "cli tag page should exist");
}

#[test]
fn test_build_tag_index_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tagidx", "Tag Index", "posts");
    let site_dir = tmp.path().join("tagidx");

    fs::write(
        site_dir.join("content/posts/2025-01-15-post.md"),
        "---\ntitle: Tagged Post\ndate: 2025-01-15\ntags:\n  - alpha\n  - beta\n---\nContent.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let tags_html = fs::read_to_string(site_dir.join("dist/tags/index.html")).unwrap();
    // Should show tag counts
    assert!(tags_html.contains("(1)"), "tag count should be shown");
    // Should link to tag pages (Tera auto-escapes / as &#x2F;)
    assert!(
        tags_html.contains("tags") && tags_html.contains("alpha"),
        "should link to alpha tag page"
    );
    assert!(
        tags_html.contains("tags") && tags_html.contains("beta"),
        "should link to beta tag page"
    );

    // Sitemap should include tag URLs
    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
    assert!(
        sitemap.contains("/tags/"),
        "sitemap should include tag page URLs"
    );
}

#[test]
fn test_build_tag_pages_multilingual() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "taglang", "Tag Lang", "posts");
    let site_dir = tmp.path().join("taglang");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Tag Lang ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\ntags:\n  - greetings\n---\nHello content.",
    )
    .unwrap();
    // Spanish post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\ntags:\n  - saludos\n---\nHola contenido.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English tag pages
    assert!(
        site_dir.join("dist/tags/greetings/index.html").exists(),
        "English tag page should exist"
    );
    // Spanish tag pages
    assert!(
        site_dir.join("dist/es/tags/saludos/index.html").exists(),
        "Spanish tag page should exist"
    );
    // Spanish tags index
    assert!(
        site_dir.join("dist/es/tags/index.html").exists(),
        "Spanish tags index should exist"
    );
    // English tag should NOT contain Spanish items
    let en_html = fs::read_to_string(site_dir.join("dist/tags/greetings/index.html")).unwrap();
    assert!(
        en_html.contains("Hello"),
        "English tag page should have English post"
    );
    assert!(
        !en_html.contains("Hola"),
        "English tag page should NOT have Spanish post"
    );
}

// --- custom templates and extra frontmatter ---

#[test]
fn test_build_extra_frontmatter_in_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "extrasite", "Extra Site", "posts");
    let site_dir = tmp.path().join("extrasite");

    // Create a custom template that uses page.extra
    let templates_dir = site_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("post.html"),
        r#"{% extends "base.html" %}
{% block title %}{{ page.title }}{% endblock %}
{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    {% if page.extra.hero_color %}<div class="hero" style="background: {{ page.extra.hero_color }}">Hero</div>{% endif %}
    {% if page.extra.featured %}<span class="featured-badge">Featured</span>{% endif %}
    {{ page.content | safe }}
</article>
{% endblock %}"#,
    )
    .unwrap();

    // Create a post with extra frontmatter
    fs::write(
        site_dir.join("content/posts/2025-01-15-custom.md"),
        "---\ntitle: Custom Post\ndate: 2025-01-15\nextra:\n  hero_color: \"#ff6600\"\n  featured: true\n---\nCustom content here.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/custom.html")).unwrap();
    assert!(
        html.contains("hero"),
        "should render hero div from extra.hero_color"
    );
    assert!(html.contains("#ff6600"), "should include hero_color value");
    assert!(
        html.contains("featured-badge"),
        "should render featured badge from extra.featured"
    );
}

#[test]
fn test_build_custom_template_with_blocks() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "blocksite", "Block Site", "posts");
    let site_dir = tmp.path().join("blocksite");

    // Create a custom template that uses the new blocks
    let templates_dir = site_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("post.html"),
        r#"{% extends "base.html" %}
{% block title %}{{ page.title }}{% endblock %}
{% block extra_css %}<style>.custom-style { color: red; }</style>{% endblock %}
{% block head %}<meta name="custom-meta" content="test-value">{% endblock %}
{% block content %}
<article>{{ page.content | safe }}</article>
{% endblock %}
{% block footer %}<div class="custom-footer">Custom Footer</div>{% endblock %}
{% block extra_js %}<script>console.log("custom js")</script>{% endblock %}"#,
    )
    .unwrap();

    fs::write(
        site_dir.join("content/posts/2025-01-15-block-test.md"),
        "---\ntitle: Block Test\ndate: 2025-01-15\n---\nBlock test content.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/block-test.html")).unwrap();
    assert!(
        html.contains("custom-style"),
        "should include extra_css block"
    );
    assert!(html.contains("custom-meta"), "should include head block");
    assert!(
        html.contains("custom-footer"),
        "should include footer block"
    );
    assert!(html.contains("custom js"), "should include extra_js block");
}

// --- URL collision detection ---

#[test]
fn test_build_detects_url_collision() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "collide", "Collide Site", "posts,pages");
    let site_dir = tmp.path().join("collide");

    // Create two content items that would resolve to the same URL
    // A post with slug override matching a page URL
    fs::write(
        site_dir.join("content/posts/2025-01-15-about.md"),
        "---\ntitle: About Post\ndate: 2025-01-15\nslug: about\n---\nAbout as a post.",
    )
    .unwrap();
    // Another post with the same slug
    fs::write(
        site_dir.join("content/posts/2025-01-16-about.md"),
        "---\ntitle: About Post 2\ndate: 2025-01-16\nslug: about\n---\nAnother about.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("URL collision"));
}

#[test]
fn test_build_warns_missing_content_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "missingdir", "Missing Dir", "posts,docs");
    let site_dir = tmp.path().join("missingdir");

    // Remove the docs content directory
    let docs_dir = site_dir.join("content/docs");
    if docs_dir.exists() {
        fs::remove_dir_all(&docs_dir).unwrap();
    }

    // Build should still succeed (warning, not error)
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

// --- accessibility ---

#[test]
fn test_build_accessibility_skip_link() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "a11y", "A11y Site", "posts");
    let site_dir = tmp.path().join("a11y");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(html.contains("skip-link"), "should have skip link class");
    assert!(
        html.contains("Skip to main content"),
        "should have skip link text"
    );
    assert!(
        html.contains("id=\"main\""),
        "main element should have id=\"main\""
    );
}

#[test]
fn test_build_accessibility_aria_search() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "a11ysearch", "A11y Search", "posts");
    let site_dir = tmp.path().join("a11ysearch");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        html.contains("role=\"search\""),
        "search form should have role=search"
    );
    assert!(
        html.contains("aria-label"),
        "search input should have aria-label"
    );
    assert!(
        html.contains("aria-live=\"polite\""),
        "search results should have aria-live"
    );
}

// --- edge case tests ---

#[test]
fn test_build_malformed_frontmatter_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "malformed", "Malformed", "posts");
    let site_dir = tmp.path().join("malformed");

    // Write a post with invalid YAML frontmatter
    fs::write(
        site_dir.join("content/posts/2025-01-15-bad.md"),
        "---\ntitle: [unclosed bracket\n---\nContent.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure();
}

#[test]
fn test_build_empty_content_body() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "emptybody", "Empty Body", "posts");
    let site_dir = tmp.path().join("emptybody");

    // Post with frontmatter but no body
    fs::write(
        site_dir.join("content/posts/2025-01-15-empty.md"),
        "---\ntitle: Empty Body Post\ndate: 2025-01-15\n---\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/empty.html")).unwrap();
    assert!(
        html.contains("Empty Body Post"),
        "should render title even with empty body"
    );
}

#[test]
fn test_build_special_characters_in_title() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "specialchars", "Special Chars", "posts");
    let site_dir = tmp.path().join("specialchars");

    fs::write(
        site_dir.join("content/posts/2025-01-15-special.md"),
        "---\ntitle: \"Rust & WebAssembly: <Fast> \\\"Quotes\\\"\"\ndate: 2025-01-15\n---\nContent with special chars.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should build without errors — HTML escaping handled by Tera
    assert!(site_dir.join("dist/posts/special.html").exists());
}

#[test]
fn test_build_custom_template_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tmploverride", "Override", "posts");
    let site_dir = tmp.path().join("tmploverride");

    // Create a custom index.html template
    let templates_dir = site_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("index.html"),
        r#"{% extends "base.html" %}
{% block title %}Custom Index{% endblock %}
{% block content %}
<div class="custom-index">Custom index content for {{ site.title }}</div>
{% endblock %}"#,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        html.contains("custom-index"),
        "should use custom index template"
    );
    assert!(
        html.contains("Custom index content"),
        "should render custom template content"
    );
}

#[test]
fn test_build_markdown_output_matches_source() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mdsource", "MD Source", "posts");
    let site_dir = tmp.path().join("mdsource");

    let original_body = "This is the **original** markdown content.\n\n- Item 1\n- Item 2";
    fs::write(
        site_dir.join("content/posts/2025-01-15-source.md"),
        format!("---\ntitle: Source Test\ndate: 2025-01-15\n---\n{original_body}"),
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The .md output should contain the original markdown body
    let md_output = fs::read_to_string(site_dir.join("dist/posts/source.md")).unwrap();
    assert!(
        md_output.contains("**original** markdown content"),
        "md output should preserve original markdown"
    );
    assert!(
        md_output.contains("- Item 1"),
        "md output should preserve list items"
    );
}

#[test]
fn test_build_no_listed_collections() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "unlisted", "Unlisted", "pages");
    let site_dir = tmp.path().join("unlisted");

    // Pages collection is not listed, so index should show no collections
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should still generate a valid index.html
    assert!(site_dir.join("dist/index.html").exists());
}
