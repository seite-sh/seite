//! `seite import <wordpress|ghost|jekyll|hugo>` end to end (fixtures in
//! `tests/fixtures/import/`; images from a local tiny_http server).

use super::common::*;
use std::path::{Path, PathBuf};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/import")
        .join(rel)
}

fn copy_dir(src: &Path, dst: &Path) {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.unwrap();
        let target = dst.join(entry.path().strip_prefix(src).unwrap());
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).unwrap();
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Serves the fixture images; `/uploads/huge.png` is 21 MB and
/// `/uploads/redirect.png` redirects to another host.
fn image_server() -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let photo = fs::read(fixture("images/photo.png")).unwrap();
    let featured = fs::read(fixture("images/featured.png")).unwrap();
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            let png = tiny_http::Header::from_bytes("Content-Type", "image/png").unwrap();
            let url = req.url().to_string();
            let response = match url.as_str() {
                "/uploads/photo.png" | "/content/images/2021/01/photo.png" => {
                    tiny_http::Response::from_data(photo.clone()).with_header(png)
                }
                "/uploads/featured.png" | "/content/images/2021/01/featured.png" => {
                    tiny_http::Response::from_data(featured.clone()).with_header(png)
                }
                "/uploads/huge.png" => {
                    tiny_http::Response::from_data(vec![0u8; 21 * 1024 * 1024]).with_header(png)
                }
                // Claims PNG by name and type, but is SVG with script.
                "/uploads/logo.png" | "/uploads/logo.svg" => tiny_http::Response::from_data(
                    b"<svg xmlns=\"http://www.w3.org/2000/svg\" onload=\"alert(1)\"/>".to_vec(),
                )
                .with_header(png),
                "/uploads/redirect.png" => tiny_http::Response::from_data(Vec::new())
                    .with_status_code(302)
                    .with_header(
                        tiny_http::Header::from_bytes("Location", "http://evil.example/x.png")
                            .unwrap(),
                    ),
                _ => tiny_http::Response::from_data(b"nope".to_vec()).with_status_code(404),
            };
            let _ = req.respond(response);
        }
    });
    format!("127.0.0.1:{port}")
}

fn new_site(tmp: &TempDir) -> PathBuf {
    init_site(tmp, "site", "Imported", "posts,pages");
    tmp.path().join("site")
}

fn wxr(tmp: &TempDir, host: &str) -> PathBuf {
    let text = fs::read_to_string(fixture("wordpress.xml"))
        .unwrap()
        .replace("__IMG__", &format!("http://{host}"));
    let path = tmp.path().join("export.xml");
    fs::write(&path, text).unwrap();
    path
}

fn read(site: &Path, rel: &str) -> String {
    fs::read_to_string(site.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn build_ok(site: &Path) {
    let (ok, doc) = run_json(site, &["build"]);
    assert!(ok, "build failed: {doc:#}");
}

fn warning_codes(doc: &serde_json::Value) -> Vec<String> {
    doc["data"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            format!(
                "{} {}",
                w["code"].as_str().unwrap(),
                w["message"].as_str().unwrap()
            )
        })
        .collect()
}

#[test]
fn test_import_wordpress_end_to_end() {
    let host = image_server();
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    // Without `seite init`'s starter post (see the test below for clashes).
    for entry in fs::read_dir(site.join("content/posts")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    let export = wxr(&tmp, &host);
    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "wordpress",
            export.to_str().unwrap(),
            "--allow-host",
            &host,
        ],
    );
    assert!(ok, "{doc:#}");
    let data = &doc["data"];
    assert_eq!(data["summary"]["posts"]["imported"], 3, "{data:#}");
    // The starter post `hello-world` from `seite init` keeps its URL.
    assert_eq!(data["summary"]["pages"]["imported"], 2, "{data:#}");
    // Draft (no --drafts) and trash.
    assert_eq!(data["summary"]["posts"]["skipped"], 2, "{data:#}");
    assert_eq!(data["images"]["downloaded"], 2, "{data:#}");

    let hello = read(&site, "content/posts/2020-01-02-hello-world.md");
    assert!(hello.contains("title: Hello World & More"), "{hello}");
    assert!(hello.contains("date: 2020-01-02"), "{hello}");
    assert!(hello.contains("description: The first post."), "{hello}");
    assert!(
        hello.contains("image: /static/images/imported/hello-world/featured.png"),
        "{hello}"
    );
    assert!(
        hello.contains("- Rust\n- News"),
        "categories become tags: {hello}"
    );
    assert!(!hello.contains("Uncategorized"), "{hello}");
    assert!(
        hello.contains("- /2020/01/02/hello-world/\n- /2020/01/02/hello/\n"),
        "{hello}"
    );
    assert!(hello.contains("extra:\n  author: Ada Admin"), "{hello}");
    assert!(
        hello.contains("Welcome to the **new** blog.\\\nThis line continues"),
        "{hello}"
    );
    assert!(
        hello.contains(r#"{{< figure(src="/static/images/imported/hello-world/photo.png", alt="A red square", caption="A red square", width="300") >}}"#),
        "{hello}"
    );
    assert!(
        hello.contains(r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#),
        "{hello}"
    );
    assert!(hello.contains(r#"{{< vimeo(id="76979871") >}}"#), "{hello}");
    assert!(hello.contains("[gallery ids=\"11,12\"]"), "{hello}");
    assert!(
        hello.contains("https://evil.example/tracker.png"),
        "{hello}"
    );
    assert!(hello.contains("```\nkeep\n  this\n```"), "{hello}");
    assert_eq!(
        fs::read(site.join("static/images/imported/hello-world/photo.png")).unwrap(),
        fs::read(fixture("images/photo.png")).unwrap()
    );

    let blocks = read(&site, "content/posts/2021-03-04-blocks.md");
    assert!(blocks.contains("Written with *blocks*"), "{blocks}");
    assert!(blocks.contains("{{\\< literal >}}"), "{blocks}");
    // Same bytes → stored once.
    assert!(
        blocks.contains("/static/images/imported/hello-world/photo.png"),
        "{blocks}"
    );
    assert!(blocks.contains("```rust\nfn main() {}\n```"), "{blocks}");
    assert!(!blocks.contains("wp:"), "{blocks}");

    // Slug collision resolved deterministically.
    let again = read(&site, "content/posts/2022-05-06-hello-world-2.md");
    assert!(again.contains("title: Hello World again"), "{again}");
    assert!(again.contains("- /2022/05/06/hello-world-2/"), "{again}");

    let about = read(&site, "content/pages/about.md");
    assert!(!about.contains("aliases"), "{about}");
    let team = read(&site, "content/pages/team.md");
    assert!(team.contains("- /company/team/"), "{team}");

    let warnings = warning_codes(&doc);
    let has = |needle: &str| warnings.iter().any(|w| w.contains(needle));
    assert!(has("wp-shortcode gallery"), "{warnings:#?}");
    assert!(
        has("wp-shortcode WordPress shortcode [contact-form-7]"),
        "{warnings:#?}"
    );
    assert!(has("slug-collision"), "{warnings:#?}");
    assert!(
        has("evil.example is not the export's site"),
        "{warnings:#?}"
    );
    assert!(has("larger than 20 MB"), "{warnings:#?}");
    assert!(has("redirected to evil.example"), "{warnings:#?}");
    assert!(has("categories-as-tags"), "{warnings:#?}");
    // Warnings carry the export line.
    let gallery = data["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["message"].as_str().unwrap().starts_with("gallery"))
        .unwrap();
    assert!(
        gallery["source"]
            .as_str()
            .unwrap()
            .ends_with("export.xml:45"),
        "{gallery}"
    );
    assert_eq!(gallery["path"], "content/posts/2020-01-02-hello-world.md");

    let report = read(&site, ".seite/cache/import-report.md");
    assert!(report.contains("# Import report"), "{report}");
    assert!(report.contains("- [ ] `"), "{report}");
    assert!(
        !report.contains(tmp.path().to_str().unwrap()),
        "no absolute paths: {report}"
    );

    build_ok(&site);
    let redirect = read(&site, "dist/2020/01/02/hello-world.html");
    assert!(redirect.contains("/posts/hello-world"), "{redirect}");
    assert!(site.join("dist/2020/01/02/hello.html").exists());
    assert!(site.join("dist/company/team.html").exists());
}

/// An export that claims a private address as its own site gets no
/// automatic download permission; naming the host on the command line does.
#[test]
fn test_import_refuses_private_site_host_unless_allowed() {
    let host = image_server();
    let tmp = TempDir::new().unwrap();
    let text = fs::read_to_string(fixture("wordpress.xml"))
        .unwrap()
        .replace("https://blog.example.com", &format!("http://{host}"))
        .replace("__IMG__", &format!("http://{host}"));
    let export = tmp.path().join("private.xml");
    fs::write(&export, text).unwrap();

    let site = new_site(&tmp);
    let (ok, doc) = run_json(&site, &["import", "wordpress", export.to_str().unwrap()]);
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["images"]["downloaded"], 0, "{doc:#}");
    assert!(
        warning_codes(&doc)
            .iter()
            .any(|w| w.contains("private/internal address")),
        "{doc:#}"
    );
    assert!(!site.join("static/images/imported").exists());

    let tmp2 = TempDir::new().unwrap();
    let site2 = new_site(&tmp2);
    let (ok, doc) = run_json(
        &site2,
        &[
            "import",
            "wordpress",
            export.to_str().unwrap(),
            "--allow-host",
            &host,
        ],
    );
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["images"]["downloaded"], 2, "{doc:#}");
}

#[test]
fn test_import_hugo_refuses_content_dir_outside_the_source() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let src = tmp.path().join("hugo");
    copy_dir(&fixture("hugo"), &src);
    fs::write(src.join("hugo.toml"), "contentDir = \"../site/content\"\n").unwrap();
    let (ok, doc) = run_json(&site, &["import", "hugo", src.to_str().unwrap()]);
    assert!(!ok);
    assert!(
        doc["error"]["message"]
            .as_str()
            .unwrap()
            .contains("outside the site directory"),
        "{doc:#}"
    );
}

/// A hostile export: aliases aimed at real output, script in HTML, SVG
/// served as PNG, reserved slugs, control characters in titles.
#[test]
fn test_import_wordpress_hostile_export() {
    let host = image_server();
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let item = |id: u32, slug: &str, link: &str, title: &str, body: &str, ptype: &str| {
        format!(
            "<item><title><![CDATA[{title}]]></title><link>{link}</link>\
             <content:encoded><![CDATA[{body}]]></content:encoded>\
             <wp:post_id>{id}</wp:post_id><wp:post_date>2020-01-0{id} 10:00:00</wp:post_date>\
             <wp:post_name>{slug}</wp:post_name><wp:status>publish</wp:status>\
             <wp:post_type>{ptype}</wp:post_type></item>"
        )
    };
    let hostile_body = format!(
        "<p><a href=\"java&#x09;script:alert(1)\" onclick=\"steal()\">click</a></p>\
         <iframe src=\"https://evil.example/x\"></iframe>\
         <p><img src=\"http://{host}/uploads/logo.png\" onerror=\"alert(1)\"></p>\
         <video/onerror=alert(1) src=x.mp4></video>\
         <iframe/src=javascript:alert(1)></iframe>\
         <iframe src=\"https://player.example.com/e\" onload=alert(1)></iframe>\
         <audio src=x onerror\n=alert(1)></audio>\
         <p><a href=\"&#106;avascript:alert(1)\">j</a> <img src=\"JaVa&#x09;Script:alert(1)\"></p>"
    );
    let items = [
        // Its old URL is the starter post's URL; an alias there would replace it.
        item(
            1,
            "attack",
            "https://blog.example.com/posts/hello-world/",
            "Attack\u{1b}[31m",
            &hostile_body,
            "post",
        ),
        item(
            2,
            "static",
            "https://blog.example.com/static/style.css",
            "Static",
            "<p>x</p>",
            "post",
        ),
        item(
            3,
            "index",
            "https://blog.example.com/tags/rust/",
            "Index",
            "<p>x</p>",
            "page",
        ),
        item(
            4,
            "404",
            "https://blog.example.com/feed.xml",
            "Not found",
            "<p>x</p>",
            "page",
        ),
    ];
    let wxr = format!(
        "<?xml version=\"1.0\"?><rss xmlns:content=\"http://purl.org/rss/1.0/modules/content/\" \
         xmlns:wp=\"http://wordpress.org/export/1.2/\"><channel><link>https://blog.example.com</link>{}</channel></rss>",
        items.join("")
    );
    let export = tmp.path().join("hostile.xml");
    fs::write(&export, wxr).unwrap();
    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "wordpress",
            export.to_str().unwrap(),
            "--allow-host",
            &host,
        ],
    );
    assert!(ok, "{doc:#}");
    let warnings = warning_codes(&doc);
    let count = |code: &str| warnings.iter().filter(|w| w.starts_with(code)).count();
    assert_eq!(count("alias-rejected"), 4, "{warnings:#?}");
    assert!(count("untrusted-html") >= 1, "{warnings:#?}");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("not a PNG, JPEG, GIF, WebP or AVIF")),
        "{warnings:#?}"
    );
    assert!(
        !site.join("static/images/imported").exists(),
        "SVG must not be stored"
    );

    let attack = read(&site, "content/posts/2020-01-01-attack.md");
    let lower = attack.to_ascii_lowercase();
    assert!(
        !lower.contains("javascript") && !lower.contains("onclick") && !lower.contains("onerror"),
        "{attack}"
    );
    assert!(!attack.contains("aliases"), "{attack}");
    assert!(
        !attack.contains('\u{1b}') || attack.contains("title:"),
        "{attack}"
    );
    // Reserved slugs.
    assert!(site.join("content/pages/index-2.md").exists());
    assert!(site.join("content/pages/404-2.md").exists());
    let report = read(&site, ".seite/cache/import-report.md");
    assert!(
        report.starts_with(
            "<!-- Titles, paths, URLs and messages quoted below come from the imported export"
        ),
        "{report}"
    );
    assert!(
        !report.contains('\u{1b}'),
        "control characters stripped from the report"
    );

    build_ok(&site);
    assert!(
        read(&site, "dist/posts/hello-world.html").contains("<html"),
        "the starter post is intact"
    );
    // The built page carries none of the payloads.
    let page = read(&site, "dist/posts/attack.html").to_ascii_lowercase();
    let article = page
        .split_once("<article")
        .map_or(page.as_str(), |(_, a)| a)
        .split_once("</article>")
        .map_or(page.as_str(), |(a, _)| a);
    // (Inert payload text may remain; no attribute or link may carry it.)
    for bad in [
        "javascript:",
        "onerror",
        "onclick",
        "onload",
        "href=\"java",
        "src=\"java",
    ] {
        assert!(!article.contains(bad), "{bad} in built page:\n{article}");
    }
    let md = read(&site, "dist/posts/attack.md").to_ascii_lowercase();
    assert!(
        !md.contains("javascript:") && !md.contains("onerror"),
        "{md}"
    );
}

/// Defence in depth: a hand-written alias that names another page's output
/// is skipped with an `alias-collision` warning, never written over it.
#[test]
fn test_build_alias_collision_keeps_the_page() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    write_site_file(
        &site,
        "content/pages/about.md",
        "---\ntitle: About\n---\nThe real about page.\n",
    );
    write_site_file(
        &site,
        "content/pages/contact.md",
        "---\ntitle: Contact\naliases:\n- /about\n- /old-contact\n---\nContact.\n",
    );
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc:#}");
    assert!(read(&site, "dist/about.html").contains("The real about page."));
    assert!(read(&site, "dist/old-contact.html").contains("/contact"));
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "{doc:#}");
    let diags = doc["data"]["diagnostics"].as_array().unwrap();
    let d = diags
        .iter()
        .find(|d| d["code"] == "alias-collision")
        .unwrap_or_else(|| panic!("{doc:#}"));
    assert_eq!(d["file"], "content/pages/contact.md", "{d}");
    assert_eq!(d["line"], 4, "{d}");
}

#[test]
fn test_import_avoids_urls_of_existing_content() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let starter: Vec<_> = fs::read_dir(site.join("content/posts"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        starter.iter().any(|n| n.ends_with("-hello-world.md")),
        "{starter:?}"
    );
    let export = wxr(&tmp, "127.0.0.1:9");
    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "wordpress",
            export.to_str().unwrap(),
            "--no-images",
        ],
    );
    assert!(ok, "{doc:#}");
    assert!(site
        .join("content/posts/2020-01-02-hello-world-2.md")
        .exists());
    assert!(site
        .join("content/posts/2022-05-06-hello-world-3.md")
        .exists());
    assert!(
        warning_codes(&doc)
            .iter()
            .any(|w| w.contains("slug-collision") && w.contains("existing content")),
        "{doc:#}"
    );
    build_ok(&site);
}

#[test]
fn test_import_ghost_end_to_end() {
    let host = image_server();
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let site_url = format!("http://{host}");
    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "ghost",
            fixture("ghost.json").to_str().unwrap(),
            "--site-url",
            &site_url,
            "--drafts",
        ],
    );
    assert!(ok, "{doc:#}");
    let story = read(&site, "content/posts/2021-01-05-ghost-story.md");
    assert!(
        story.contains("description: A story told by Ghost."),
        "{story}"
    );
    assert!(story.contains("updated: 2021-02-01"), "{story}");
    assert!(
        story.contains("image: /static/images/imported/ghost-story/featured.png"),
        "{story}"
    );
    assert!(story.contains("tags:\n- Stories\n- Ghosts\n"), "{story}");
    assert!(!story.contains("internal"), "{story}");
    assert!(story.contains("- /ghost-story/"), "{story}");
    assert!(story.contains("author: Casper Writer"), "{story}");
    assert!(
        story.contains(&format!("[Ghost]({site_url}/about/)")),
        "{story}"
    );
    assert!(
        story.contains(r#"{{< figure(src="/static/images/imported/ghost-story/photo.png", alt="Photo", caption="A caption") >}}"#),
        "{story}"
    );
    assert!(
        story.contains(r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#),
        "{story}"
    );
    assert!(story.contains("Markdown card"), "{story}");

    let draft = read(&site, "content/posts/2021-03-01-draft-idea.md");
    assert!(
        draft.contains("draft: true") && draft.contains("Only plaintext here."),
        "{draft}"
    );
    let paid = read(&site, "content/posts/2021-04-01-paid-insights.md");
    assert!(paid.contains("draft: true"), "{paid}");
    let about = read(&site, "content/pages/about.md");
    assert!(!about.contains("aliases"), "{about}");
    assert!(warning_codes(&doc)
        .iter()
        .any(|w| w.starts_with("members-only")));

    build_ok(&site);
    assert!(read(&site, "dist/ghost-story.html").contains("/posts/ghost-story"));
}

#[test]
fn test_import_ghost_without_drafts_skips_them() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "ghost",
            fixture("ghost.json").to_str().unwrap(),
            "--no-images",
        ],
    );
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["summary"]["posts"]["imported"], 1, "{doc:#}");
    assert_eq!(doc["data"]["summary"]["posts"]["skipped"], 2, "{doc:#}");
    assert!(!site.join("content/posts/2021-03-01-draft-idea.md").exists());
    // No URL in the export: __GHOST_URL__ links become root-relative.
    let story = read(&site, "content/posts/2021-01-05-ghost-story.md");
    assert!(story.contains("[Ghost](/about/)"), "{story}");
    assert!(
        story.contains("image: /content/images/2021/01/featured.png"),
        "{story}"
    );
    assert!(warning_codes(&doc)
        .iter()
        .any(|w| w.starts_with("ghost-url")));
}

#[test]
fn test_import_jekyll_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let (ok, doc) = run_json(
        &site,
        &["import", "jekyll", fixture("jekyll").to_str().unwrap()],
    );
    assert!(ok, "{doc:#}");
    let first = read(&site, "content/posts/2020-05-01-first-post.md");
    assert!(
        first.contains("image: /static/images/imported/first-post/cat.png"),
        "{first}"
    );
    assert!(first.contains("- /2020/05/01/first-post\n"), "{first}");
    assert!(first.contains("[second post](/posts/second)"), "{first}");
    assert!(first.contains("[about](/about)"), "{first}");
    assert!(
        first.contains("![Cat](/static/images/imported/first-post/cat.png)"),
        "{first}"
    );
    assert!(first.contains("```ruby\nputs \"hi\"\n```"), "{first}");
    assert!(first.contains("Literal {{ liquid }} stays."), "{first}");
    assert!(first.contains("{% include note.html %}"), "{first}");
    assert!(first.contains("author: Jay Kyll"), "{first}");
    assert!(site
        .join("static/images/imported/first-post/cat.png")
        .is_file());

    let second = read(&site, "content/posts/2020-06-01-second.md");
    assert!(
        second.contains("- /dev/tools/2020/06/01/second\n"),
        "{second}"
    );
    assert!(second.contains("[first](/posts/first-post)"), "{second}");
    let about = read(&site, "content/pages/about.md");
    assert!(about.contains("- /about-me/"), "{about}");
    assert!(
        !site.join("content/pages/readme.md").exists(),
        "excluded by _config.yml"
    );

    let warnings = warning_codes(&doc);
    let include = doc["data"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["code"] == "liquid")
        .unwrap_or_else(|| panic!("{warnings:#?}"));
    assert!(
        include["source"]
            .as_str()
            .unwrap()
            .ends_with("_posts/2020-05-01-first-post.md:21"),
        "{include}"
    );
    let skipped: Vec<&str> = doc["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["status"] == "skipped")
        .map(|i| i["source_id"].as_str().unwrap())
        .collect();
    assert!(
        skipped.contains(&"_drafts/wip.md") && skipped.contains(&"index.md"),
        "{skipped:?}"
    );

    build_ok(&site);
    assert!(read(&site, "dist/2020/05/01/first-post.html").contains("/posts/first-post"));
    assert!(site.join("dist/about-me.html").exists());
}

#[cfg(unix)]
#[test]
fn test_import_jekyll_refuses_symlinks_out_of_the_source() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let src = tmp.path().join("jekyll");
    copy_dir(&fixture("jekyll"), &src);
    let secret = tmp.path().join("secret.png");
    fs::copy(fixture("images/photo.png"), &secret).unwrap();
    std::os::unix::fs::symlink(&secret, src.join("assets/img/escape.png")).unwrap();
    fs::write(
        src.join("_posts/2021-01-01-escape.md"),
        "---\ntitle: Escape\n---\n![x](/assets/img/escape.png)\n",
    )
    .unwrap();
    let (ok, doc) = run_json(&site, &["import", "jekyll", src.to_str().unwrap()]);
    assert!(ok, "{doc:#}");
    assert!(
        warning_codes(&doc)
            .iter()
            .any(|w| w.contains("outside the source directory")),
        "{doc:#}"
    );
    assert!(!site.join("static/images/imported/escape").exists());
}

#[test]
fn test_import_hugo_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let (ok, doc) = run_json(
        &site,
        &["import", "hugo", fixture("hugo").to_str().unwrap()],
    );
    assert!(ok, "{doc:#}");
    let post = read(&site, "content/posts/2022-02-03-toml-post.md");
    assert!(post.contains("updated: 2022-03-01"), "{post}");
    assert!(post.contains("series: Migration"), "{post}");
    assert!(post.contains("- hugo\n- toml\n- Guides"), "{post}");
    assert!(
        post.contains("- /2022/02/toml-post/\n- /old/toml-post/\n"),
        "{post}"
    );
    assert!(
        post.contains("![logo](/static/images/imported/toml-post/logo.png)"),
        "{post}"
    );
    assert!(
        post.contains(r#"{{< youtube(id="w7Ft2ymGmfc") >}}"#),
        "{post}"
    );
    assert!(
        post.contains(r#"{{< figure(src="/static/images/imported/toml-post/logo.png", caption="The logo") >}}"#),
        "{post}"
    );
    assert!(post.contains("[about](/about)"), "{post}");
    assert!(post.contains("```go\nfunc main() {}\n```"), "{post}");
    assert!(
        post.contains(r#"`{{< tweet user="hugo" id="1" >}}`"#),
        "{post}"
    );

    let bundle = read(&site, "content/posts/2023-07-08-bundle-post.md");
    assert!(
        bundle.contains("image: /static/images/imported/bundle-post/cover.png"),
        "{bundle}"
    );
    assert!(
        bundle.contains("![Diagram](/static/images/imported/bundle-post/diagram.png)"),
        "{bundle}"
    );
    let about = read(&site, "content/pages/about.md");
    assert!(
        about.contains("weight: 2") && about.contains("- /about-us/"),
        "{about}"
    );
    assert!(!site.join("content/posts/2024-01-01-draft-post.md").exists());
    assert!(warning_codes(&doc)
        .iter()
        .any(|w| w.starts_with("hugo-shortcode")));

    build_ok(&site);
    assert!(read(&site, "dist/old/toml-post.html").contains("/posts/toml-post"));
    assert!(site.join("dist/2022/02/toml-post.html").exists());
}

#[test]
fn test_import_dry_run_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let before = dir_snapshot(&site);
    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "hugo",
            fixture("hugo").to_str().unwrap(),
            "--dry-run",
        ],
    );
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["dry_run"], true);
    assert_eq!(doc["data"]["summary"]["posts"]["imported"], 2, "{doc:#}");
    assert!(
        doc["data"]["images"]["planned"].as_u64().unwrap() >= 3,
        "{doc:#}"
    );
    assert_eq!(before, dir_snapshot(&site));
}

#[test]
fn test_import_skips_existing_files_without_force() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let target = site.join("content/pages/about.md");
    fs::write(&target, "---\ntitle: Mine\n---\nKeep me.\n").unwrap();
    let (ok, doc) = run_json(
        &site,
        &["import", "hugo", fixture("hugo").to_str().unwrap()],
    );
    assert!(ok, "{doc:#}");
    assert!(read(&site, "content/pages/about.md").contains("Keep me."));
    let about = doc["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["source_id"] == "about.md")
        .unwrap();
    assert_eq!(about["status"], "skipped");
    assert!(
        about["reason"].as_str().unwrap().contains("--force"),
        "{about}"
    );

    let (ok, _) = run_json(
        &site,
        &[
            "import",
            "hugo",
            fixture("hugo").to_str().unwrap(),
            "--force",
        ],
    );
    assert!(ok);
    assert!(read(&site, "content/pages/about.md").contains("JSON frontmatter"));
}

#[test]
fn test_import_malformed_input_gives_located_errors() {
    let tmp = TempDir::new().unwrap();
    let site = new_site(&tmp);
    let xml = tmp.path().join("bad.xml");
    fs::write(&xml, "<rss>\n<channel>\n<item><wp:post_id>1</item>\n").unwrap();
    let (ok, doc) = run_json(&site, &["import", "wordpress", xml.to_str().unwrap()]);
    assert!(!ok);
    let msg = doc["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("bad.xml:") && msg.contains("malformed XML"),
        "{msg}"
    );

    let json = tmp.path().join("bad.json");
    fs::write(&json, "{\"db\": [\n  {\"data\": }\n]}").unwrap();
    let (ok, doc) = run_json(&site, &["import", "ghost", json.to_str().unwrap()]);
    assert!(!ok);
    let msg = doc["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("bad.json:2:") && msg.contains("invalid JSON"),
        "{msg}"
    );

    let (ok, doc) = run_json(
        &site,
        &[
            "import",
            "ghost",
            fixture("wordpress.xml").to_str().unwrap(),
        ],
    );
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("invalid JSON"));

    let (ok, doc) = run_json(
        &site,
        &["import", "jekyll", fixture("hugo").to_str().unwrap()],
    );
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("_config.yml"));
    assert_eq!(
        fs::read_dir(site.join("content/posts")).unwrap().count(),
        1,
        "nothing imported"
    );
}

#[test]
fn test_import_requires_target_collection_and_site() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Posts only", "posts");
    let site = tmp.path().join("site");
    let (ok, doc) = run_json(
        &site,
        &["import", "hugo", fixture("hugo").to_str().unwrap()],
    );
    assert!(!ok);
    let msg = doc["error"]["message"].as_str().unwrap();
    assert!(msg.contains("seite collection add pages"), "{msg}");

    let (ok, doc) = run_json(
        tmp.path(),
        &["import", "hugo", fixture("hugo").to_str().unwrap()],
    );
    assert!(!ok);
    assert!(doc["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not inside a seite site"));
}

/// Every file under `dirs` of `root` with its bytes.
fn tree(root: &Path, dirs: &[&str]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for d in dirs {
        for e in walkdir::WalkDir::new(root.join(d))
            .into_iter()
            .filter_map(Result::ok)
        {
            if e.file_type().is_file() {
                let rel = e.path().strip_prefix(root).unwrap().display().to_string();
                out.push((rel, fs::read(e.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn test_import_is_deterministic() {
    let host = image_server();
    let mut trees = Vec::new();
    for _ in 0..2 {
        let tmp = TempDir::new().unwrap();
        let site = new_site(&tmp);
        let export = wxr(&tmp, &host);
        for args in [
            vec![
                "import",
                "wordpress",
                export.to_str().unwrap(),
                "--allow-host",
                &host,
            ],
            vec!["import", "hugo", fixture("hugo").to_str().unwrap()],
        ] {
            let (ok, doc) = run_json(&site, &args);
            assert!(ok, "{doc:#}");
        }
        trees.push(tree(
            &site,
            &["content", "static", ".seite/cache/import-report.md"],
        ));
    }
    assert!(!trees[0].is_empty());
    assert_eq!(trees[0], trees[1]);
}
