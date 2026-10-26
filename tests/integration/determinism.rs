//! Builds are byte-identical run to run: no output depends on hash-map
//! iteration order (collections, `extra:` keys, asset manifest, ...) or on
//! the build time.

use super::common::*;

/// A site exercising every output that collects pages: several collections,
/// two extra languages, tags plus a `categories` taxonomy, a series,
/// aliases, `extra:` keys in non-alphabetical order, data, a processed
/// image and fingerprinted assets.
fn rich_site(tmp: &TempDir) -> std::path::PathBuf {
    page_cmd()
        .args([
            "init",
            "rich",
            "--title",
            "Rich",
            "--description",
            "A rich site for determinism",
            "--deploy-target",
            "cloudflare",
            "--collections",
            "posts,docs,pages,changelog,roadmap,trust",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let site = tmp.path().join("rich");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    let toml = toml
        .replace(
            "[languages]\n",
            "[languages.es]\ntitle = \"Rico\"\n\n[languages.de]\ntitle = \"Reich\"\n",
        )
        .replace(
            "[deploy]",
            "fingerprint = true\nrelated = 3\n\n[[taxonomies]]\nname = \"categories\"\n\n[deploy]",
        );
    fs::write(site.join("seite.toml"), toml).unwrap();
    for i in 1..=8 {
        fs::write(
            site.join(format!("content/posts/2025-01-0{i}-post-{i}.md")),
            format!(
                "---\ntitle: Post {i}\ndate: 2025-01-0{i}\ndescription: Description of post {i}, long enough for search results.\n\
                 tags: [alpha, beta, gamma{g}, delta{d}]\ncategories: [cat{g}, news]\nseries: my-series\n\
                 aliases: [/old/post-{i}/, /legacy/p{i}.html]\n\
                 extra:\n  zeta: {i}\n  alpha: a{i}\n  mid: [1, 2]\n  nested:\n    z: 1\n    a: 2\n---\n\n\
                 Body {i} with a [link](/posts/post-1) and [docs](/docs/intro).\n\n## Heading {i}\n\n![a pic](/static/pic.png)\n",
                g = i % 3,
                d = i % 2,
            ),
        )
        .unwrap();
        fs::write(
            site.join(format!("content/posts/2025-01-0{i}-post-{i}.es.md")),
            format!(
                "---\ntitle: Entrada {i}\ndate: 2025-01-0{i}\ntags: [uno, dos]\n---\nHola {i}.\n"
            ),
        )
        .unwrap();
    }
    fs::write(
        site.join("content/docs/intro.md"),
        "---\ntitle: Intro\nweight: 1\n---\nIntro.\n",
    )
    .unwrap();
    fs::write(
        site.join("content/docs/setup.md"),
        "---\ntitle: Setup\nweight: 2\n---\nSetup.\n",
    )
    .unwrap();
    fs::write(
        site.join("content/pages/about.md"),
        "---\ntitle: About\naliases: [/about-us/]\n---\nAbout.\n",
    )
    .unwrap();
    fs::write(
        site.join("content/pages/about.de.md"),
        "---\ntitle: Über\n---\nÜber.\n",
    )
    .unwrap();
    fs::write(
        site.join("data/team.yaml"),
        "- {name: Zed, role: b}\n- {name: Amy, role: a}\n",
    )
    .unwrap();
    image::RgbImage::from_fn(900, 600, |x, y| image::Rgb([x as u8, y as u8, 128]))
        .save(site.join("static/pic.png"))
        .unwrap();
    fs::write(site.join("static/a.css"), "body { color: red }\n").unwrap();
    fs::write(site.join("static/b.js"), "console.log(1)\n").unwrap();
    site
}

fn files(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let rel = e
                .path()
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, fs::read(e.path()).unwrap())
        })
        .collect()
}

fn build_into(site: &std::path::Path, out: &str) -> std::collections::BTreeMap<String, Vec<u8>> {
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    let start = toml.find("output_dir = ").unwrap();
    let end = start + toml[start..].find('\n').unwrap();
    let toml = format!("{}output_dir = \"{out}\"{}", &toml[..start], &toml[end..]);
    fs::write(site.join("seite.toml"), toml).unwrap();
    page_cmd().arg("build").current_dir(site).assert().success();
    files(&site.join(out))
}

#[test]
fn test_build_is_byte_identical_across_runs() {
    let tmp = TempDir::new().unwrap();
    let site = rich_site(&tmp);
    let first = build_into(&site, "out1");
    // Sanity: the outputs that used to vary are there.
    for f in [
        "sitemap.xml",
        "search-index.json",
        "_redirects",
        "asset-manifest.json",
        "llms.txt",
        "llms-full.txt",
        "posts/post-1.md",
        "categories/index.html",
        "es/posts/post-1.html",
    ] {
        assert!(first.contains_key(f), "missing {f}: {:?}", first.keys());
    }
    // Hash-map order differs per process, so a few runs catch it reliably.
    for run in 2..=3 {
        let next = build_into(&site, &format!("out{run}"));
        assert_eq!(
            first.keys().collect::<Vec<_>>(),
            next.keys().collect::<Vec<_>>(),
            "file sets differ"
        );
        let differing: Vec<&String> = first
            .iter()
            .filter(|(k, v)| next.get(*k) != Some(v))
            .map(|(k, _)| k)
            .collect();
        assert!(differing.is_empty(), "run {run} differs in {differing:?}");
    }
    // `extra:` keys keep their source order in the published `.md` copy.
    let md = String::from_utf8(first["posts/post-1.md"].clone()).unwrap();
    let zeta = md.find("zeta:").unwrap();
    let alpha = md.find("alpha: a1").unwrap();
    assert!(zeta < alpha, "{md}");
}
