use super::common::*;

// --- shortcodes ---

#[test]
fn test_build_inline_shortcode_youtube() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest");

    fs::write(
        site_dir.join("content/posts/2025-01-15-video.md"),
        r#"---
title: Video Post
---
Check this out:

{{< youtube(id="dQw4w9WgXcQ") >}}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/video.html")).unwrap();
    assert!(html.contains("youtube.com/embed/dQw4w9WgXcQ"));
    assert!(html.contains("video-embed"));
}

#[test]
fn test_build_body_shortcode_callout() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest2", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest2");

    fs::write(
        site_dir.join("content/posts/2025-01-15-callout.md"),
        "---\ntitle: Callout Post\n---\n\n{{% callout(type=\"warning\") %}}\nThis is **important** stuff.\n{{% end %}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/callout.html")).unwrap();
    assert!(html.contains("callout-warning"));
    assert!(html.contains("<strong>important</strong>"));
}

#[test]
fn test_build_shortcode_in_code_block_not_expanded() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest3", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest3");

    fs::write(
        site_dir.join("content/posts/2025-01-15-code.md"),
        "---\ntitle: Code Post\n---\n\n```\n{{< youtube(id=\"test\") >}}\n```\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/code.html")).unwrap();
    // Should NOT contain youtube embed — shortcode is inside code block
    assert!(!html.contains("youtube.com/embed"));
    // Should contain the literal shortcode text (escaped)
    assert!(html.contains("youtube"));
}

#[test]
fn test_build_shortcode_in_inline_code_not_expanded() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest4", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest4");

    fs::write(
        site_dir.join("content/posts/2025-01-15-inline.md"),
        "---\ntitle: Inline Code\n---\n\nUse `{{< youtube(id=\"test\") >}}` for videos.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/inline.html")).unwrap();
    assert!(!html.contains("youtube.com/embed"));
}

#[test]
fn test_build_unknown_shortcode_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest5", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest5");

    fs::write(
        site_dir.join("content/posts/2025-01-15-unknown.md"),
        "---\ntitle: Unknown\n---\n\n{{< nonexistent(x=\"y\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown shortcode"));
}

#[test]
fn test_build_unclosed_body_shortcode_errors() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest6", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest6");

    fs::write(
        site_dir.join("content/posts/2025-01-15-unclosed.md"),
        "---\ntitle: Unclosed\n---\n\n{{% callout(type=\"info\") %}}\nNo end tag here.\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unclosed body shortcode"));
}

#[test]
fn test_build_user_defined_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest7", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest7");

    // Create custom shortcode
    fs::create_dir_all(site_dir.join("templates/shortcodes")).unwrap();
    fs::write(
        site_dir.join("templates/shortcodes/badge.html"),
        "<span class=\"badge badge-{{ color }}\">{{ text }}</span>",
    )
    .unwrap();

    fs::write(
        site_dir.join("content/posts/2025-01-15-badge.md"),
        "---\ntitle: Badge Post\n---\n\n{{< badge(color=\"green\", text=\"New\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/badge.html")).unwrap();
    assert!(html.contains("badge-green"));
    assert!(html.contains(">New</span>"));
}

#[test]
fn test_build_shortcode_raw_body_preserved_in_md() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest8", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest8");

    fs::write(
        site_dir.join("content/posts/2025-01-15-raw.md"),
        "---\ntitle: Raw MD\n---\n\n{{< youtube(id=\"test123\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // HTML should have the expanded embed
    let html = fs::read_to_string(site_dir.join("dist/posts/raw.html")).unwrap();
    assert!(html.contains("youtube.com/embed/test123"));

    // MD output should have the original unexpanded shortcode
    let md = fs::read_to_string(site_dir.join("dist/posts/raw.md")).unwrap();
    assert!(md.contains("{{< youtube(id=\"test123\") >}}"));
}

#[test]
fn test_build_figure_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest9", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest9");

    fs::write(
        site_dir.join("content/posts/2025-01-15-figure.md"),
        "---\ntitle: Figure Post\n---\n\n{{< figure(src=\"/static/img.jpg\", caption=\"A photo\", alt=\"My photo\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/figure.html")).unwrap();
    assert!(html.contains("<figure"));
    assert!(html.contains("<figcaption>A photo</figcaption>"));
}

#[test]
fn test_build_multiple_shortcodes_in_one_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest10", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest10");

    fs::write(
        site_dir.join("content/posts/2025-01-15-multi.md"),
        r#"---
title: Multi
---
{{< youtube(id="abc") >}}

Some text between.

{{< vimeo(id="123") >}}
"#,
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/multi.html")).unwrap();
    assert!(html.contains("youtube.com/embed/abc"));
    assert!(html.contains("player.vimeo.com/video/123"));
}

#[test]
fn test_build_shortcode_user_overrides_builtin() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest11", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest11");

    // Override youtube shortcode with custom template
    fs::create_dir_all(site_dir.join("templates/shortcodes")).unwrap();
    fs::write(
        site_dir.join("templates/shortcodes/youtube.html"),
        "<div class=\"custom-yt\">{{ id }}</div>",
    )
    .unwrap();

    fs::write(
        site_dir.join("content/posts/2025-01-15-override.md"),
        "---\ntitle: Override\n---\n\n{{< youtube(id=\"custom\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/override.html")).unwrap();
    assert!(html.contains("custom-yt"));
    assert!(!html.contains("youtube.com/embed"));
}

#[test]
fn test_build_gist_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "sctest12", "SC Test", "posts");
    let site_dir = tmp.path().join("sctest12");

    fs::write(
        site_dir.join("content/posts/2025-01-15-gist.md"),
        "---\ntitle: Gist Post\n---\n\n{{< gist(user=\"octocat\", id=\"abc123\") >}}\n",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/posts/gist.html")).unwrap();
    assert!(html.contains("gist.github.com/octocat/abc123.js"));
}
