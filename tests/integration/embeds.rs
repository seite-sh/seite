//! `[comments]` / `[newsletter]` with the `comments()` / `newsletter()`
//! shortcodes and the `seite/comments.html` / `seite/newsletter.html` partials.

use super::common::*;

fn site_with_post(tmp: &TempDir, body: &str) -> std::path::PathBuf {
    init_site(tmp, "site", "Embeds", "posts,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("content/posts/2025-01-02-hello.md"),
        format!(
            "---\ntitle: Hello\ndate: 2025-01-02\ndescription: A post\n---\n\nText.\n\n{body}\n"
        ),
    )
    .unwrap();
    site
}

fn append_config(site: &std::path::Path, toml: &str) {
    let path = site.join("seite.toml");
    let mut config = fs::read_to_string(&path).unwrap();
    config.push_str(toml);
    fs::write(path, config).unwrap();
}

fn post_html(site: &std::path::Path) -> String {
    fs::read_to_string(site.join("dist/posts/hello.html")).unwrap()
}

const GISCUS: &str = "\n[comments]\nprovider = \"giscus\"\nrepo = \"owner/repo\"\nrepo_id = \"R_kgDO123\"\ncategory = \"Blog comments\"\ncategory_id = \"DIC_kwDO123\"\nreactions = false\n";

#[test]
fn test_build_comments_giscus_embed() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "{{< comments() >}}");
    append_config(&site, GISCUS);
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let html = post_html(&site);
    assert!(
        html.contains(
            "<script src=\"https://giscus.app/client.js\" data-repo=\"owner&#x2F;repo\" data-repo-id=\"R_kgDO123\" data-category=\"Blog comments\" data-category-id=\"DIC_kwDO123\" data-mapping=\"pathname\" data-strict=\"0\" data-reactions-enabled=\"0\" data-emit-metadata=\"0\" data-input-position=\"bottom\" data-theme=\"preferred_color_scheme\" data-lang=\"en\" data-loading=\"lazy\" crossorigin=\"anonymous\" async></script>"
        ),
        "{html}"
    );
}

#[test]
fn test_build_comments_utterances_via_partial() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "");
    append_config(
        &site,
        "\n[comments]\nprovider = \"utterances\"\nrepo = \"owner/repo\"\nlabel = \"comment\"\ntheme = \"github-dark\"\n",
    );
    // A theme places comments without the shortcode.
    let post = site.join("templates/post.html");
    let template = fs::read_to_string(&post).unwrap();
    let template = template.replacen(
        "{% endblock %}",
        "{% include \"seite/comments.html\" %}{% include \"seite/newsletter.html\" %}{% endblock %}",
        1,
    );
    assert!(template.contains("seite/comments.html"));
    fs::write(&post, template).unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let html = post_html(&site);
    assert!(
        html.contains("<script src=\"https://utteranc.es/client.js\" repo=\"owner&#x2F;repo\" issue-term=\"pathname\" label=\"comment\" theme=\"github-dark\" crossorigin=\"anonymous\" async></script>"),
        "{html}"
    );
    // No [newsletter]: the partial renders nothing.
    assert!(!html.contains("newsletter-form"));
}

#[test]
fn test_build_comments_rejects_hostile_config() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "{{< comments() >}}");
    append_config(
        &site,
        "\n[comments]\nprovider = \"giscus\"\nrepo = \"owner/repo\"\nrepo_id = \"R_1\\\"><script>alert(1)</script>\"\ncategory = \"General\"\ncategory_id = \"DIC_1\"\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok, "{doc}");
    let text = doc.to_string();
    assert!(text.contains("config-invalid"), "{doc:#}");
    assert!(text.contains("repo_id"), "{doc:#}");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure();
    assert!(!site.join("dist/posts/hello.html").exists());
}

#[test]
fn test_build_comments_not_configured_warns() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "{{< comments() >}}\n\n{{< newsletter() >}}");
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "{doc:#}");
    let text = doc.to_string();
    assert!(text.contains("comments-not-configured"), "{doc:#}");
    assert!(text.contains("newsletter-not-configured"), "{doc:#}");
    let output = page_cmd().arg("build").current_dir(&site).output().unwrap();
    assert!(output.status.success());
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains("comments-not-configured"), "{all}");
    assert!(all.contains("2025-01-02-hello.md:9"), "{all}");
    let html = post_html(&site);
    assert!(!html.contains("giscus") && !html.contains("newsletter-form"));
}

#[test]
fn test_build_newsletter_buttondown_form() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(
        &tmp,
        "{{< newsletter(title=\"Get updates\", button=\"Sign me up\") >}}",
    );
    append_config(
        &site,
        "\n[newsletter]\nprovider = \"buttondown\"\nusername = \"my-news\"\n",
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let html = post_html(&site);
    assert!(
        html.contains("action=\"https:&#x2F;&#x2F;buttondown.com&#x2F;api&#x2F;emails&#x2F;embed-subscribe&#x2F;my-news\" method=\"post\""),
        "{html}"
    );
    assert!(html.contains("name=\"email\" required autocomplete=\"email\""));
    assert!(html.contains("<input type=\"hidden\" name=\"embed\" value=\"1\">"));
    assert!(html.contains("Get updates"));
    assert!(html.contains(">Sign me up</button>"));
    assert!(html.contains("<label for=\"newsletter-email\">Email address</label>"));
}

#[test]
fn test_build_newsletter_kit_i18n_labels() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "{{< newsletter() >}}");
    append_config(
        &site,
        "\n[newsletter]\nprovider = \"kit\"\nform_id = \"1234567\"\n",
    );
    fs::create_dir_all(site.join("data/i18n")).unwrap();
    fs::write(
        site.join("data/i18n/en.yaml"),
        "newsletter_email: Your email\nnewsletter_submit: Join\n",
    )
    .unwrap();
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    let html = post_html(&site);
    assert!(
        html.contains(
            "action=\"https:&#x2F;&#x2F;app.kit.com&#x2F;forms&#x2F;1234567&#x2F;subscriptions\""
        ),
        "{html}"
    );
    assert!(html.contains("name=\"email_address\""));
    assert!(html.contains(">Your email</label>"));
    assert!(html.contains(">Join</button>"));
    assert!(!html.contains("name=\"embed\""));
}

#[test]
fn test_build_newsletter_rejects_bad_form_id() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "{{< newsletter() >}}");
    append_config(
        &site,
        "\n[newsletter]\nprovider = \"kit\"\nform_id = \"12\\\" onfocus=\\\"x\"\n",
    );
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(!ok);
    let text = doc.to_string();
    assert!(
        text.contains("config-invalid") && text.contains("form_id"),
        "{doc:#}"
    );
}

#[test]
fn test_comments_unknown_key_warns() {
    let tmp = TempDir::new().unwrap();
    let site = site_with_post(&tmp, "");
    append_config(&site, &format!("{GISCUS}repoo = \"x\"\n"));
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "{doc:#}");
    assert!(doc.to_string().contains("config-unknown-key"), "{doc:#}");
}
