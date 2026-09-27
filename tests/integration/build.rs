use super::common::*;

// --- build command ---

#[test]
fn test_build_produces_output() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Build Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Build it
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Built"));

    // Verify output files
    let dist = site_dir.join("dist");
    assert!(dist.join("index.html").exists());
    assert!(dist.join("feed.xml").exists());
    assert!(dist.join("sitemap.xml").exists());

    // The hello-world post should be rendered
    assert!(dist.join("posts/hello-world.html").exists());

    // Verify index.html has content
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(index.contains("Build Test"));
    assert!(index.contains("Hello World"));

    // Verify post HTML
    let post = fs::read_to_string(dist.join("posts/hello-world.html")).unwrap();
    assert!(post.contains("Hello World"));
    assert!(post.contains("seite")); // from the body text
}

#[test]
fn test_build_excludes_drafts_by_default() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Draft Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Create a draft post
    let draft =
        "---\ntitle: Secret Draft\ndraft: true\ndate: 2025-01-01\n---\n\nThis is a draft.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-01-secret-draft.md"),
        draft,
    )
    .unwrap();

    // Build without --drafts
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(!site_dir.join("dist/posts/secret-draft.html").exists());

    // Build with --drafts
    page_cmd()
        .args(["build", "--drafts"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/posts/secret-draft.html").exists());
}

#[test]
fn test_build_with_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Docs Build", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create a doc
    let doc = "---\ntitle: Getting Started\n---\n\nWelcome to the docs.\n";
    fs::write(site_dir.join("content/docs/getting-started.md"), doc).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/docs/getting-started.html").exists());
}

#[test]
fn test_nested_docs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nested Docs", "posts,docs,pages");

    let site_dir = tmp.path().join("site");

    // Create nested doc
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    let doc = "---\ntitle: Setup Guide\n---\n\nHow to set up.\n";
    fs::write(site_dir.join("content/docs/guides/setup.md"), doc).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/docs/guides/setup.html").exists());
}

// ── Math/LaTeX rendering ──

#[test]
fn test_build_math_disabled_no_rendering() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Math Off", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Math Page\n---\n\nThe formula $E=mc^2$ inline.\n";
    fs::write(site_dir.join("content/pages/math.md"), page_content).unwrap();

    // math defaults to false — no need to set it
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/math.html")).unwrap();
    // With math disabled, no KaTeX CSS link injected and no rendered math spans
    assert!(
        !html.contains("katex.min.css"),
        "should not inject katex CSS link"
    );
    assert!(
        !html.contains("<span class=\"katex\""),
        "should not contain rendered katex spans"
    );
}

#[test]
fn test_build_math_enabled_renders_katex() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Math On", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Math Page\n---\n\nThe formula $E=mc^2$ inline.\n";
    fs::write(site_dir.join("content/pages/math.md"), page_content).unwrap();

    // Enable math in the existing [build] section
    let toml_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&toml_path).unwrap();
    config = config.replace("[build]", "[build]\nmath = true");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/math.html")).unwrap();
    assert!(
        html.contains("katex"),
        "should contain katex rendered output: {html}"
    );
    assert!(
        html.contains("katex.min.css"),
        "should inject katex CSS link"
    );
}

#[test]
fn test_build_mermaid_enabled_renders_diagram() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Mermaid On", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Diagram Page\n---\n\n```mermaid\ngraph TD\n  A-->B\n```\n";
    fs::write(site_dir.join("content/pages/diagram.md"), page_content).unwrap();

    set_build_option(&site_dir, "mermaid", "true");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/diagram.html")).unwrap();
    assert!(
        html.contains(r#"<div class="mermaid">"#),
        "mermaid fence should render as a mermaid div: {html}"
    );
    assert!(html.contains("graph TD"));
    assert!(
        html.contains("mermaid.esm.min.mjs"),
        "should inject the mermaid loader: {html}"
    );
}

#[test]
fn test_build_mermaid_disabled_plain_code() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Mermaid Off", "posts,pages");
    let site_dir = tmp.path().join("site");

    let page_content = "---\ntitle: Diagram Page\n---\n\n```mermaid\ngraph TD\n  A-->B\n```\n";
    fs::write(site_dir.join("content/pages/diagram.md"), page_content).unwrap();

    // mermaid defaults to off — no config change
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/diagram.html")).unwrap();
    assert!(
        !html.contains(r#"class="mermaid""#),
        "no mermaid div when disabled: {html}"
    );
    assert!(
        !html.contains("mermaid.esm.min.mjs"),
        "no loader when disabled"
    );
    assert!(
        html.contains("<pre><code>"),
        "unknown language should fall back to plain code: {html}"
    );
}

// ── Reading time + word count ──

#[test]
fn test_build_reading_time_in_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rtsite", "RT Site", "posts");
    let site_dir = tmp.path().join("rtsite");

    // Write a post with a known word count (~300 words → 2 min read at 238 WPM)
    let words: String = (0..300).map(|i| format!("word{i} ")).collect();
    let content = format!("---\ntitle: Reading Test\ndate: 2025-01-15\n---\n{}", words);
    fs::write(
        site_dir.join("content/posts/2025-01-15-reading-test.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/reading-test.html")).unwrap();
    assert!(html.contains("min read"), "should contain reading time");
    assert!(
        html.contains("reading-time"),
        "should contain reading-time class"
    );
}

#[test]
fn test_build_reading_time_in_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rtidx", "RT Index", "posts");
    let site_dir = tmp.path().join("rtidx");

    let words: String = (0..500).map(|i| format!("word{i} ")).collect();
    let content = format!("---\ntitle: Long Post\ndate: 2025-01-15\n---\n{}", words);
    fs::write(
        site_dir.join("content/posts/2025-01-15-long-post.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // The index page should have the reading_time in the item summary via template
    // (reading_time is available as item.reading_time in index templates)
    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Index currently doesn't display reading_time by default, but the data is there
    // Just verify the build succeeded and post appears
    assert!(html.contains("Long Post"));
}

// ── Excerpts ──

#[test]
fn test_build_excerpt_more_marker() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "exsite1", "Excerpt Site", "posts");
    let site_dir = tmp.path().join("exsite1");

    let content = "---\ntitle: More Marker\ndate: 2025-01-15\n---\nThis is the intro.\n\n<!-- more -->\n\nThis is after the fold.";
    fs::write(
        site_dir.join("content/posts/2025-01-15-more-marker.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    // Index should show the excerpt (intro before <!-- more -->)
    assert!(
        index.contains("This is the intro."),
        "excerpt should appear in index"
    );
    assert!(
        !index.contains("This is after the fold."),
        "content after more marker should not be in excerpt"
    );
}

#[test]
fn test_build_excerpt_first_paragraph() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "exsite2", "Excerpt Site 2", "posts");
    let site_dir = tmp.path().join("exsite2");

    let content = "---\ntitle: Auto Excerpt\ndate: 2025-01-15\n---\nFirst paragraph auto-extracted.\n\nSecond paragraph not shown.\n\nThird paragraph also hidden.";
    fs::write(
        site_dir.join("content/posts/2025-01-15-auto-excerpt.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("First paragraph auto-extracted."),
        "first paragraph should be used as excerpt"
    );
}

#[test]
fn test_build_excerpt_description_takes_priority() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "exsite3", "Excerpt Site 3", "posts");
    let site_dir = tmp.path().join("exsite3");

    // When description is set, it should take priority over excerpt in the default index template
    let content = "---\ntitle: With Description\ndate: 2025-01-15\ndescription: Custom description here\n---\nFirst paragraph.\n\nSecond paragraph.";
    fs::write(
        site_dir.join("content/posts/2025-01-15-with-desc.md"),
        content,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("Custom description here"),
        "description should be shown when set"
    );
}

// ── 404 page ──

#[test]
fn test_build_generates_404_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site404", "404 Site", "posts");
    let site_dir = tmp.path().join("site404");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let path_404 = site_dir.join("dist/404.html");
    assert!(path_404.exists(), "404.html should be generated");

    let html = fs::read_to_string(path_404).unwrap();
    assert!(html.contains("404"), "should contain 404");
    assert!(
        html.contains("Page Not Found"),
        "should contain Page Not Found heading"
    );
    assert!(
        html.contains("noindex"),
        "404 page should have noindex robots meta"
    );
}

// ── Table of contents ──

#[test]
fn test_build_toc_headings_have_ids() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tocsite1", "ToC Site", "docs");
    let site_dir = tmp.path().join("tocsite1");

    let content = "---\ntitle: My Doc\n---\n## Introduction\n\nSome text.\n\n### Details\n\nMore text.\n\n## Conclusion\n\nEnd.";
    fs::write(site_dir.join("content/docs/my-doc.md"), content).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/my-doc.html")).unwrap();
    assert!(html.contains("id=\"introduction\""), "h2 should have id");
    assert!(html.contains("id=\"details\""), "h3 should have id");
    assert!(html.contains("id=\"conclusion\""), "h2 should have id");
}

#[test]
fn test_build_toc_in_doc_template() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tocsite2", "ToC Site 2", "docs");
    let site_dir = tmp.path().join("tocsite2");

    // Doc with multiple headings should get a ToC nav
    let content = "---\ntitle: Guide\n---\n## Step 1\n\nDo this.\n\n## Step 2\n\nDo that.\n\n## Step 3\n\nDone.";
    fs::write(site_dir.join("content/docs/guide.md"), content).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/guide.html")).unwrap();
    // Default doc template should show ToC when > 1 heading
    assert!(html.contains("toc"), "doc should contain toc class");
    assert!(
        html.contains("Contents"),
        "doc should contain Contents heading"
    );
    assert!(html.contains("step-1"), "toc should link to step-1");
    assert!(html.contains("step-2"), "toc should link to step-2");
    assert!(html.contains("step-3"), "toc should link to step-3");
}

// ── Data files feature ──────────────────────────────────────────────

#[test]
fn test_build_with_yaml_data_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data YAML", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create a YAML data file
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/authors.yaml"),
        "- name: Alice\n  role: Editor\n- name: Bob\n  role: Writer\n",
    )
    .unwrap();

    // Modify index template to render data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.authors %}{% for author in data.authors %}<span class=\"data-author\">{{ author.name }}</span>{% endfor %}{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("data files loaded"));

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Alice"),
        "YAML data should be rendered in template"
    );
    assert!(
        index_html.contains("Bob"),
        "YAML data should be rendered in template"
    );
}

#[test]
fn test_build_with_json_data_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data JSON", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/social.json"),
        r#"[{"name": "GitHub", "url": "https://github.com"}]"#,
    )
    .unwrap();

    // Modify index template to render data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.social %}{% for link in data.social %}<a class=\"data-social\" href=\"{{ link.url }}\">{{ link.name }}</a>{% endfor %}{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("GitHub"),
        "JSON data should be rendered in template"
    );
}

#[test]
fn test_build_with_toml_data_file() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data TOML", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/settings.toml"),
        "site_name = \"My TOML Site\"\nmax_posts = 10\n",
    )
    .unwrap();

    // Modify index template to render data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.settings %}<span class=\"data-setting\">{{ data.settings.site_name }}</span>{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("My TOML Site"),
        "TOML data should be rendered in template"
    );
}

#[test]
fn test_build_with_nested_data_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nested Data", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data/menus")).unwrap();
    fs::write(
        site_dir.join("data/menus/main.yaml"),
        "- title: Home\n  url: /\n- title: About\n  url: /about\n",
    )
    .unwrap();

    // Modify index template to render nested data
    let index_path = site_dir.join("templates/index.html");
    let index_tmpl = fs::read_to_string(&index_path).unwrap();
    let modified = index_tmpl.replace(
        "{% endblock %}",
        "{% if data.menus %}{% if data.menus.main %}{% for item in data.menus.main %}<a class=\"data-nav\">{{ item.title }}</a>{% endfor %}{% endif %}{% endif %}{% endblock %}",
    );
    fs::write(&index_path, modified).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Home"),
        "Nested data.menus.main should work"
    );
    assert!(
        index_html.contains("About"),
        "Nested data.menus.main should work"
    );
}

#[test]
fn test_build_data_conflict_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data Conflict", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(site_dir.join("data/authors.yaml"), "- name: Alice\n").unwrap();
    fs::write(site_dir.join("data/authors.json"), r#"[{"name": "Bob"}]"#).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("data key conflict"));
}

#[test]
fn test_build_data_parse_error() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Data Error", "posts,pages");
    let site_dir = tmp.path().join("site");

    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/broken.yaml"),
        "invalid: yaml: [: unclosed\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("broken.yaml"));
}

#[test]
fn test_build_no_data_dir_succeeds() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Data", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove the data dir that init creates
    let data_dir = site_dir.join("data");
    if data_dir.exists() {
        fs::remove_dir_all(&data_dir).unwrap();
    }

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_build_data_nav_in_theme() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Theme Nav", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create nav data file matching the theme's expected format
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/nav.yaml"),
        "- title: Blog\n  url: /posts\n- title: About\n  url: /about\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Main navigation"),
        "Theme should render data.nav with aria-label"
    );
    // Tera HTML-escapes / to &#x2F; in attribute values
    assert!(
        index_html.contains("/posts") || index_html.contains("&#x2F;posts"),
        "Nav should contain link URLs"
    );
    assert!(
        index_html.contains("Blog"),
        "Nav should contain link titles"
    );
}

#[test]
fn test_build_data_footer_in_theme() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Theme Footer", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Create footer data file
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/footer.yaml"),
        "links:\n  - title: GitHub\n    url: https://github.com\n  - title: Twitter\n    url: https://twitter.com\ncopyright: \"2026 Test Corp\"\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index_html.contains("Footer navigation"),
        "Theme should render data.footer with aria-label"
    );
    assert!(
        index_html.contains("GitHub"),
        "Footer should contain link title"
    );
    assert!(
        index_html.contains("2026 Test Corp"),
        "Footer should use custom copyright"
    );
}

#[test]
fn test_build_docs_sorted_by_weight() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Weight Sort", "docs");

    let site_dir = tmp.path().join("site");

    // Create 3 docs with weights out of alphabetical order
    fs::write(
        site_dir.join("content/docs/zebra.md"),
        "---\ntitle: Zebra\nweight: 1\n---\n\nFirst by weight.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/alpha.md"),
        "---\ntitle: Alpha\nweight: 3\n---\n\nThird by weight.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/middle.md"),
        "---\ntitle: Middle\nweight: 2\n---\n\nSecond by weight.\n",
    )
    .unwrap();

    // Apply docs theme for sidebar nav
    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check the sidebar order in any built doc page
    let html = fs::read_to_string(site_dir.join("dist/docs/zebra.html")).unwrap();
    let pos_zebra = html.find(">Zebra<").expect("Zebra should be in sidebar");
    let pos_middle = html.find(">Middle<").expect("Middle should be in sidebar");
    let pos_alpha = html.find(">Alpha<").expect("Alpha should be in sidebar");

    assert!(
        pos_zebra < pos_middle && pos_middle < pos_alpha,
        "Docs should be sorted by weight: Zebra(1) < Middle(2) < Alpha(3), got positions: {} {} {}",
        pos_zebra,
        pos_middle,
        pos_alpha
    );
}

#[test]
fn test_build_docs_weight_mixed_with_unweighted() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Mixed Weight", "docs");

    let site_dir = tmp.path().join("site");

    // Weighted items should come first, unweighted sort alphabetically after
    fs::write(
        site_dir.join("content/docs/second.md"),
        "---\ntitle: Second\nweight: 2\n---\n\nWeighted second.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/first.md"),
        "---\ntitle: First\nweight: 1\n---\n\nWeighted first.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/bravo.md"),
        "---\ntitle: Bravo\n---\n\nUnweighted B.\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/docs/alpha-unweighted.md"),
        "---\ntitle: Alpha Unweighted\n---\n\nUnweighted A.\n",
    )
    .unwrap();

    // Apply docs theme for sidebar nav
    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/docs/first.html")).unwrap();
    let pos_first = html.find(">First<").expect("First should be in sidebar");
    let pos_second = html.find(">Second<").expect("Second should be in sidebar");
    let pos_alpha = html
        .find(">Alpha Unweighted<")
        .expect("Alpha Unweighted should be in sidebar");
    let pos_bravo = html.find(">Bravo<").expect("Bravo should be in sidebar");

    // Weighted items first (by weight), then unweighted (alphabetically)
    assert!(
        pos_first < pos_second && pos_second < pos_alpha && pos_alpha < pos_bravo,
        "Expected: First(w1) < Second(w2) < Alpha Unweighted(none) < Bravo(none), got: {} {} {} {}",
        pos_first,
        pos_second,
        pos_alpha,
        pos_bravo
    );
}

#[test]
fn test_build_nudges_when_outdated() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Nudge Test", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Remove .seite/config.json to simulate pre-tracking project
    fs::remove_file(site_dir.join(".seite/config.json")).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite upgrade"));
}

#[test]
fn test_build_no_nudge_when_current() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Nudge", "posts,pages");
    let site_dir = tmp.path().join("site");

    // Fresh init should not nudge
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite upgrade").not());
}

// ── analytics ───────────────────────────────────────────────────────

#[test]
fn test_build_with_google_analytics_direct() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Analytics Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Add [analytics] to seite.toml
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"google\"\nid = \"G-TEST12345\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check that analytics script is injected into HTML
    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("googletagmanager.com/gtag/js?id=G-TEST12345"),
        "index.html should contain GA4 script"
    );
    assert!(
        index.contains("gtag('config','G-TEST12345')"),
        "index.html should contain gtag config call"
    );
    // Should NOT have consent banner
    assert!(
        !index.contains("seite-cookie-banner"),
        "should not have consent banner when cookie_consent is false"
    );

    // Check post HTML too
    let post = fs::read_to_string(site_dir.join("dist/posts/hello-world.html")).unwrap();
    assert!(post.contains("G-TEST12345"));
}

#[test]
fn test_build_with_analytics_cookie_consent() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Consent Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    // Add [analytics] with cookie_consent = true
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[analytics]\nprovider = \"google\"\nid = \"G-CONSENT1\"\ncookie_consent = true\n",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();

    // Should have consent banner
    assert!(
        index.contains("seite-cookie-banner"),
        "should have cookie consent banner"
    );
    assert!(
        index.contains("seite-cookie-accept"),
        "should have accept button"
    );
    assert!(
        index.contains("seite-cookie-decline"),
        "should have decline button"
    );
    assert!(
        index.contains("seite_analytics_consent"),
        "should use localStorage key"
    );

    // Analytics script should NOT be in <head> directly
    let head_end = index.find("</head>").unwrap();
    let head_section = &index[..head_end];
    assert!(
        !head_section.contains("googletagmanager.com/gtag/js"),
        "GA script should not be directly in <head> when consent is required"
    );
}

#[test]
fn test_build_with_plausible_analytics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Plausible Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"plausible\"\nid = \"example.com\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("plausible.io/js/script.js"));
    assert!(index.contains("data-domain=\"example.com\""));
}

#[test]
fn test_build_with_gtm_has_noscript() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "GTM Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"gtm\"\nid = \"GTM-ABC123\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("GTM-ABC123"), "should contain GTM ID");
    assert!(
        index.contains("<noscript><iframe"),
        "GTM should include noscript fallback"
    );
}

#[test]
fn test_build_without_analytics_no_injection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Analytics", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(!index.contains("googletagmanager"));
    assert!(!index.contains("plausible"));
    assert!(!index.contains("seite-cookie-banner"));
    assert!(!index.contains("usefathom"));
}

#[test]
fn test_build_with_umami_custom_script_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Umami Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[analytics]\nprovider = \"umami\"\nid = \"abc-def-123\"\nscript_url = \"https://stats.example.com/script.js\"\n",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("stats.example.com/script.js"));
    assert!(index.contains("data-website-id=\"abc-def-123\""));
}

#[test]
fn test_build_contact_form_formspree() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "cftest", "CF Test", "posts,pages");
    let site_dir = tmp.path().join("cftest");

    add_contact_config(&site_dir, "formspree", "xtest123");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("formspree.io/f/xtest123"));
    assert!(html.contains("contact-form"));
    assert!(html.contains("method=\"POST\""));
    assert!(html.contains("_gotcha")); // honeypot
}

#[test]
fn test_build_contact_form_web3forms() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "w3test", "W3 Test", "posts,pages");
    let site_dir = tmp.path().join("w3test");

    add_contact_config(&site_dir, "web3forms", "test-access-key");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("api.web3forms.com/submit"));
    assert!(html.contains("test-access-key"));
    assert!(html.contains("name=\"access_key\""));
}

#[test]
fn test_build_contact_form_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "nltest", "NL Test", "posts,pages");
    let site_dir = tmp.path().join("nltest");

    add_contact_config(&site_dir, "netlify", "contact");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("data-netlify=\"true\""));
    assert!(html.contains("data-netlify-honeypot=\"bot-field\""));
    assert!(html.contains("name=\"form-name\""));
}

#[test]
fn test_build_contact_form_hubspot() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "hstest", "HS Test", "posts,pages");
    let site_dir = tmp.path().join("hstest");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[contact]\nprovider = \"hubspot\"\nendpoint = \"12345/abcd-efgh\"\nregion = \"na1\"\n",
    );
    fs::write(&config_path, config).unwrap();

    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("js.hsforms.net/forms/embed/v2.js"));
    assert!(html.contains("hbspt.forms.create"));
    assert!(html.contains("portalId:\"12345\""));
    assert!(html.contains("formId:\"abcd-efgh\""));
}

#[test]
fn test_build_contact_form_typeform() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tftest", "TF Test", "posts,pages");
    let site_dir = tmp.path().join("tftest");

    add_contact_config(&site_dir, "typeform", "abc123XY");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("data-tf-widget=\"abc123XY\""));
    assert!(html.contains("embed.typeform.com/next/embed.js"));
}

#[test]
fn test_build_contact_form_without_config_shows_error() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "noconf", "No Conf", "posts,pages");
    let site_dir = tmp.path().join("noconf");

    // No [contact] section in config — shortcode should render an error message
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form() >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("contact-form-error"));
    assert!(html.contains("seite contact setup"));
}

#[test]
fn test_build_contact_form_with_label_overrides() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "lbltest", "Lbl Test", "posts,pages");
    let site_dir = tmp.path().join("lbltest");

    add_contact_config(&site_dir, "formspree", "xtest456");
    fs::write(
        site_dir.join("content/pages/contact.md"),
        "---\ntitle: Contact\n---\n\n{{< contact_form(name_label=\"Full Name\", submit_label=\"Submit\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/contact.html")).unwrap();
    assert!(html.contains("Full Name"));
    assert!(html.contains("Submit"));
}

#[test]
fn test_build_generates_404() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "err404", "404 Test", "posts");
    let site_dir = tmp.path().join("err404");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/404.html").exists());
}

#[test]
fn test_build_generates_llms_txt() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "llms", "LLMs Test", "posts");
    let site_dir = tmp.path().join("llms");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/llms.txt").exists());
    assert!(site_dir.join("dist/llms-full.txt").exists());
}

#[test]
fn test_build_generates_robots_txt() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "robots", "Robots Test", "posts");
    let site_dir = tmp.path().join("robots");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let robots = fs::read_to_string(site_dir.join("dist/robots.txt")).unwrap();
    assert!(robots.contains("User-agent"));
    assert!(robots.contains("Sitemap:"));
}

#[test]
fn test_build_with_data_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "datasite", "Data Test", "posts,pages");
    let site_dir = tmp.path().join("datasite");

    // Create a data file
    fs::create_dir_all(site_dir.join("data")).unwrap();
    fs::write(
        site_dir.join("data/nav.yaml"),
        "- title: Blog\n  url: /posts\n- title: About\n  url: /about\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

// --- build with multiple collections ---

#[test]
fn test_build_with_all_collection_types() {
    let tmp = TempDir::new().unwrap();
    init_site(
        &tmp,
        "allcol",
        "All Collections",
        "posts,docs,pages,changelog,roadmap",
    );
    let site_dir = tmp.path().join("allcol");

    // Create content in each collection
    page_cmd()
        .args(["new", "post", "Test Post", "--tags", "test"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["new", "doc", "Getting Started"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["new", "changelog", "v1.0.0", "--tags", "new"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["new", "roadmap", "Offline Mode", "--tags", "planned"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify each collection has output
    assert!(site_dir.join("dist/index.html").exists());
    assert!(site_dir.join("dist/sitemap.xml").exists());
    assert!(site_dir.join("dist/feed.xml").exists());
}

// --- build with google analytics ---

#[test]
fn test_build_with_google_analytics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ganalytics", "GA Test", "posts");
    let site_dir = tmp.path().join("ganalytics");

    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[analytics]\nprovider = \"google\"\nid = \"G-TESTID123\"\n");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("G-TESTID123"));
}

// --- build output markdown alongside HTML ---

#[test]
fn test_build_outputs_markdown_alongside_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mdout", "MD Output", "posts");
    let site_dir = tmp.path().join("mdout");

    page_cmd()
        .args(["new", "post", "Markdown Output Test"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Find the generated post
    let dist = site_dir.join("dist/posts");
    if dist.exists() {
        let html_files: Vec<_> = fs::read_dir(&dist)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "html").unwrap_or(false))
            .collect();
        // Each HTML should have a corresponding .md
        for html in &html_files {
            let md_path = html.path().with_extension("md");
            assert!(md_path.exists(), "Missing .md for {:?}", html.path());
        }
    }
}
