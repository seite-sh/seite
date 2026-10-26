//! Full-text search (`[build] search = "full"`): the index under `search/`,
//! what it leaves out, the config modes, the size budget on a 1,000-page
//! site, and the theme's search script run under Node (when installed)
//! against a built index.

use super::common::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use seite::build::search::{query_bytes, BuiltIndex, IndexStats, SHARD_BYTES};

/// Every file under `<dir>/search/` as `search/...` → content.
fn index_files(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(dir.join("search")) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(dir).unwrap();
            out.insert(
                rel.to_string_lossy().replace('\\', "/"),
                fs::read_to_string(entry.path()).unwrap(),
            );
        }
    }
    out
}

fn all_index_text(dir: &Path) -> String {
    index_files(dir)
        .into_values()
        .collect::<Vec<_>>()
        .join("\n")
}

fn build(site: &Path, args: &[&str]) {
    let mut cmd = page_cmd();
    cmd.arg("build").args(args).current_dir(site);
    cmd.assert().success();
}

/// A site with posts in English (diacritics, stemming, headings, CJK), a
/// draft, a `noindex` post, a scheduled post and a private collection.
fn search_site(tmp: &TempDir) -> PathBuf {
    init_site(tmp, "site", "Search", "posts,docs,pages");
    let site = tmp.path().join("site");
    for f in fs::read_dir(site.join("content/posts")).unwrap() {
        fs::remove_file(f.unwrap().path()).unwrap();
    }
    write_site_file(
        &site,
        "content/posts/2025-01-02-cafe.md",
        "---\ntitle: Café culture\ndate: 2025-01-02\ndescription: About coffee shops.\ntags: [drinks]\n---\n\n\
         We love running to the café every morning. Naïve baristas brew espresso.\n\n\
         ## Brewing methods\n\nPour over and French press. We visit Tokyo rarely.\n",
    );
    write_site_file(
        &site,
        "content/posts/2025-01-03-tokyo.md",
        "---\ntitle: Tokyo notes\ndate: 2025-01-03\n---\n\n東京都に住んでいます。Running shoes are great.\n",
    );
    write_site_file(
        &site,
        "content/posts/2025-01-04-secret.md",
        "---\ntitle: Secret\ndate: 2025-01-04\ndraft: true\n---\n\nespresso zzdraftword\n",
    );
    write_site_file(
        &site,
        "content/posts/2025-01-05-hidden.md",
        "---\ntitle: Hidden\ndate: 2025-01-05\nrobots: noindex\n---\n\nespresso zznoindexword\n",
    );
    write_site_file(
        &site,
        "content/posts/2999-01-01-future.md",
        "---\ntitle: Future\ndate: 2999-01-01\n---\n\nespresso zzfutureword\n",
    );
    add_collection_line(&site, "docs", "private = true");
    write_site_file(
        &site,
        "content/docs/internal.md",
        "---\ntitle: Internal\n---\n\nespresso zzprivateword\n",
    );
    site
}

#[test]
fn test_full_search_index_files_and_exclusions() {
    let tmp = TempDir::new().unwrap();
    let site = search_site(&tmp);
    build(&site, &["--drafts"]);
    let dist = site.join("dist");
    let meta = read_json_file(&dist.join("search/meta.json"));
    assert_eq!(meta["version"], 1);
    assert_eq!(meta["lang"], "en");
    assert_eq!(meta["stem"], true);
    assert_eq!(meta["pages"], 2, "{meta}");
    assert_eq!(meta["collections"], serde_json::json!(["posts"]));
    let text = all_index_text(&dist);
    for word in [
        "zzdraftword",
        "zznoindexword",
        "zzfutureword",
        "zzprivateword",
    ] {
        assert!(!text.contains(word), "{word} leaked into the search index");
    }
    // Stemmed, folded terms and CJK bigrams are in the shards.
    for term in ["\"cafe\"", "\"run\"", "\"東京\"", "\"京都\"", "\"brew\""] {
        assert!(text.contains(term), "{term} missing");
    }
    // The basic index keeps its exact shape.
    let basic = read_json_file(&dist.join("search-index.json"));
    let first = basic.as_array().unwrap()[0].as_object().unwrap();
    let keys: Vec<&str> = first.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "title",
            "description",
            "url",
            "collection",
            "tags",
            "date",
            "lang"
        ]
    );
    // The page carries the full-mode script and the search box.
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(index.contains(r#"id="search-input""#));
    assert!(index.contains("search/meta.json"));
    assert!(!index.contains("innerHTML"));
}

#[test]
fn test_full_search_index_per_language() {
    let tmp = TempDir::new().unwrap();
    let site = search_site(&tmp);
    add_language(&site, "es", "Búsqueda");
    write_site_file(
        &site,
        "content/posts/2025-01-02-cafe.es.md",
        "---\ntitle: Cultura del café\ndate: 2025-01-02\n---\n\nCorriendo al café.\n",
    );
    build(&site, &[]);
    let dist = site.join("dist");
    let es = read_json_file(&dist.join("es/search/meta.json"));
    assert_eq!(es["lang"], "es");
    assert_eq!(es["stem"], false, "no stemming outside English");
    assert_eq!(es["pages"], 1);
    assert!(all_index_text(&dist.join("es")).contains("\"corriendo\""));
    assert!(!all_index_text(&dist).contains("corriendo"));
    let es_page = fs::read_to_string(dist.join("es/posts/cafe.html")).unwrap();
    assert!(
        es_page.contains(r#"'/'+"es"+'/'"#),
        "per-language index dir"
    );
}

#[test]
fn test_search_modes_basic_absent_and_off() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Modes", "posts");
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(toml.contains("search = \"full\""), "init writes full");

    // Absent key = basic: exactly the pre-0.21 output.
    let without: String = toml
        .lines()
        .filter(|l| !l.starts_with("search = ") && !l.contains("search over page text"))
        .filter(|l| !l.contains("searches titles, descriptions"))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(site.join("seite.toml"), &without).unwrap();
    build(&site, &[]);
    assert!(site.join("dist/search-index.json").exists());
    assert!(!site.join("dist/search").exists());
    let index = fs::read_to_string(site.join("dist/index.html")).unwrap();
    assert!(index.contains(r#"mode="basic""#), "basic mode script");

    // search = false: no index, no box, no script.
    set_build_option(&site, "search", "false");
    build(&site, &[]);
    assert!(!site.join("dist/search-index.json").exists());
    assert!(!site.join("dist/search").exists());
    let index = fs::read_to_string(site.join("dist/index.html")).unwrap();
    assert!(!index.contains("search-input"), "no search box");

    // An invalid value is a config error.
    set_build_option(&site, "search", "\"fancy\"");
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid search mode"));
}

#[test]
fn test_build_json_reports_search_index_stats() {
    let tmp = TempDir::new().unwrap();
    let site = search_site(&tmp);
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let stats = &doc["data"]["search_index"];
    assert_eq!(stats["pages"], 2, "{doc}");
    assert!(stats["bytes"].as_u64().unwrap() > 0);
    assert!(stats["files"].as_u64().unwrap() >= 4);
    assert!(stats["largest_shard_bytes"].as_u64().unwrap() > 0);
    // Verbose human output prints the summary line.
    page_cmd()
        .args(["build", "--verbose"])
        .current_dir(&site)
        .assert()
        .success()
        .stdout(predicate::str::contains("Search index: 2 pages"));
}

#[test]
fn test_upgrade_suggests_full_search_once() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Upgrade", "posts");
    let site = tmp.path().join("site");
    let toml = fs::read_to_string(site.join("seite.toml")).unwrap();
    let old: String = toml
        .lines()
        .filter(|l| !l.starts_with("search = "))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(site.join("seite.toml"), old).unwrap();
    let run = || {
        let out = page_cmd()
            .args(["upgrade", "--force"])
            .current_dir(&site)
            .output()
            .unwrap();
        assert!(out.status.success());
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    assert!(run().contains("Full-text search is available"));
    assert!(!fs::read_to_string(site.join("seite.toml"))
        .unwrap()
        .contains("search = \"full\""));
    assert!(!run().contains("Full-text search is available"), "once");
}

// --- 1,000-page size budget ---

/// Deterministic pseudo-random generator (LCG) for the synthetic corpus.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

/// A 5,000-word vocabulary drawn with a Zipf-like distribution: a few very
/// common words, a long tail of rare ones (like real prose).
fn corpus_word(rng: &mut Lcg, vocab: &[String]) -> String {
    // Inverse-power sampling: index ~ n^(u^2) skews toward small indexes.
    let u = (rng.next() % 1_000_000) as f64 / 1_000_000.0;
    let idx = ((vocab.len() as f64).powf(u * u) as usize).saturating_sub(1);
    vocab[idx.min(vocab.len() - 1)].clone()
}

fn vocabulary() -> Vec<String> {
    let syll = [
        "ka", "lo", "mi", "ner", "sto", "ber", "ga", "ti", "pre", "con", "ul", "ra", "ven", "dor",
        "si", "pha", "lu", "mon", "tre", "qua",
    ];
    let mut rng = Lcg(7);
    (0..5000)
        .map(|_| {
            let n = 2 + (rng.next() % 3) as usize;
            (0..n)
                .map(|_| syll[(rng.next() % syll.len() as u64) as usize])
                .collect::<String>()
        })
        .collect()
}

fn thousand_page_site(tmp: &TempDir) -> PathBuf {
    init_site(tmp, "big", "Big", "posts");
    let site = tmp.path().join("big");
    let vocab = vocabulary();
    let mut rng = Lcg(42);
    for i in 0..1000 {
        let title: Vec<String> = (0..4).map(|_| corpus_word(&mut rng, &vocab)).collect();
        let mut body = String::new();
        for section in 0..3 {
            let heading: Vec<String> = (0..3).map(|_| corpus_word(&mut rng, &vocab)).collect();
            body.push_str(&format!("\n## {} {section}\n\n", heading.join(" ")));
            let words: Vec<String> = (0..120).map(|_| corpus_word(&mut rng, &vocab)).collect();
            body.push_str(&words.join(" "));
            body.push_str(".\n");
        }
        write_site_file(
            &site,
            &format!("content/posts/2024-01-01-p{i:04}.md"),
            &format!(
                "---\ntitle: {}\ndate: 2024-01-01\ndescription: Page {i} about {}.\n---\n{body}",
                title.join(" "),
                title[0]
            ),
        );
    }
    site
}

/// Budget on a generated 1,000-page site: every shard within the cap (a
/// single huge posting list may not be split), and a typical query
/// (meta + the shards of its terms + the 8 largest page fragments) under
/// 100 KB. With Node installed, the real script's downloads are measured too.
#[test]
fn test_full_search_size_budget_on_1000_pages() {
    let tmp = TempDir::new().unwrap();
    let site = thousand_page_site(&tmp);
    let (ok, doc) = run_json(&site, &["build"]);
    assert!(ok, "{doc}");
    let dist = site.join("dist");
    let files = index_files(&dist);
    let meta = read_json_file(&dist.join("search/meta.json"));
    assert_eq!(meta["pages"], 1001, "1,000 pages plus the starter post");
    let shards: Vec<&String> = files
        .iter()
        .filter(|(p, _)| p.starts_with("search/terms/"))
        .map(|(_, b)| b)
        .collect();
    let largest = shards.iter().map(|b| b.len()).max().unwrap();
    for shard in &shards {
        let parsed: serde_json::Value = serde_json::from_str(shard).unwrap();
        let terms = parsed.as_object().unwrap().len();
        assert!(
            shard.len() <= SHARD_BYTES || terms == 1,
            "shard of {} bytes holds {terms} terms",
            shard.len()
        );
    }
    let total: usize = files.values().map(String::len).sum();
    let stats = &doc["data"]["search_index"];
    assert_eq!(stats["bytes"].as_u64().unwrap() as usize, total);

    let index = BuiltIndex {
        files: files.clone(),
        stats: IndexStats::default(),
    };
    let vocab = vocabulary();
    let mut worst = 0;
    for q in [
        vocab[0].clone(),
        format!("{} {}", vocab[3], vocab[40]),
        format!("{} {}", vocab[10], &vocab[200][..3]),
        vocab[2500].clone(),
    ] {
        worst = worst.max(query_bytes(&index, &q, 8, "en"));
    }
    eprintln!(
        "1,000 pages: index {total} bytes in {} files, {} shards, largest shard {largest} bytes, meta {} bytes, worst query {worst} bytes",
        files.len(),
        shards.len(),
        files["search/meta.json"].len()
    );
    assert!(worst < 100 * 1024, "typical query downloads {worst} bytes");

    if let Some(node) = node() {
        let queries = serde_json::json!([
            {"q": vocab[0]},
            {"q": format!("{} {}", vocab[3], vocab[40])},
            {"q": format!("{} {}", vocab[10], &vocab[200][..3])},
        ]);
        let out = run_script(&node, &dist, &dist.join("index.html"), &queries, None);
        for r in out.as_array().unwrap() {
            let bytes = r["bytes"].as_u64().unwrap();
            eprintln!("node query {}: {bytes} bytes", r["q"]);
            assert!(bytes < 100 * 1024, "{}: {bytes} bytes", r["q"]);
            assert!(!r["results"].as_array().unwrap().is_empty(), "{r}");
        }
    }
}

// --- the search script under Node ---

fn node() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(if cfg!(windows) { "node.exe" } else { "node" }))
        .find(|p| p.is_file())
}

fn run_script(
    node: &Path,
    dist: &Path,
    page: &Path,
    queries: &serde_json::Value,
    collections: Option<&str>,
) -> serde_json::Value {
    let qfile = dist.parent().unwrap().join("queries.json");
    fs::write(&qfile, queries.to_string()).unwrap();
    let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/search-js/run.js");
    let mut cmd = std::process::Command::new(node);
    cmd.arg(runner).arg(dist).arg(page).arg(&qfile);
    if let Some(c) = collections {
        cmd.env("SEARCH_COLLECTIONS", c);
    }
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!("{e}: {}", String::from_utf8_lossy(&out.stdout));
    })
}

fn titles(r: &serde_json::Value) -> Vec<String> {
    r["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["title"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn test_search_script_under_node() {
    let Some(node) = node() else {
        eprintln!("node not on PATH: skipping the search script test");
        return;
    };
    let tmp = TempDir::new().unwrap();
    let site = search_site(&tmp);
    build(&site, &[]);
    let dist = site.join("dist");
    let queries = serde_json::json!([
        {"q": "espresso"},
        {"q": "CAFE"},
        {"q": "runs"},
        {"q": "brew meth"},
        {"q": "東京"},
        {"q": "京", "keys": ["ArrowDown", "Enter"]},
        {"q": "nothingmatcheshere"},
        {"q": "tokyo"},
        {"q": "esp", "keys": ["ArrowDown", "ArrowDown", "ArrowUp"]},
        {"q": "the "},
    ]);
    let out = run_script(&node, &dist, &dist.join("index.html"), &queries, None);
    let r = out.as_array().unwrap();
    let get = |q: &str| r.iter().find(|x| x["q"] == q).unwrap();

    // Body match, highlighted, drafts/noindex/scheduled/private excluded.
    let espresso = get("espresso");
    assert_eq!(titles(espresso), ["Café culture"], "{espresso}");
    assert_eq!(espresso["status"], "1 result");
    let hit = &espresso["results"][0];
    assert_eq!(hit["href"], "/posts/cafe");
    assert!(hit["marks"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("espresso")));
    assert!(
        hit["snippet"].as_str().unwrap().contains("espresso"),
        "{hit}"
    );
    // Only the meta, the shard and the page fragment are fetched.
    let fetched = espresso["fetched"].as_array().unwrap();
    assert!(
        fetched
            .iter()
            .all(|u| u.as_str().unwrap().starts_with("/search/")),
        "{espresso}"
    );

    // Diacritic folding and case: CAFE finds Café, and marks the original.
    let cafe = get("CAFE");
    assert_eq!(titles(cafe), ["Café culture"]);
    assert!(cafe["results"][0]["marks"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("Café")));

    // Stemming: "runs" matches "running"/"Running" on both posts.
    assert_eq!(get("runs")["results"].as_array().unwrap().len(), 2);

    // AND semantics with a prefix last term: brew + meth(ods).
    assert_eq!(titles(get("brew meth")), ["Café culture"]);

    // CJK bigrams, and a single character as a prefix.
    assert_eq!(titles(get("東京")), ["Tokyo notes"]);
    let kyo = get("京");
    assert_eq!(titles(kyo), ["Tokyo notes"]);
    assert_eq!(
        kyo["location"], "/posts/tokyo",
        "ArrowDown + Enter opens it"
    );

    // No results: message and status.
    let none = get("nothingmatcheshere");
    assert_eq!(none["noResults"], true);
    assert_eq!(none["status"], "No results");

    // Ranking: the title match outranks the body mention.
    assert_eq!(titles(get("tokyo")), ["Tokyo notes", "Café culture"]);

    // Keyboard: down, down, up → the first result is active.
    let esp = get("esp");
    assert_eq!(esp["activedescendant"], "search-result-0");
    assert_eq!(esp["results"][0]["active"], true);

    // Stopwords only: nothing to search for.
    assert_eq!(get("the ")["noResults"], true);

    // data-collections filters results.
    let filtered = run_script(
        &node,
        &dist,
        &dist.join("index.html"),
        &serde_json::json!([{"q": "tokyo"}]),
        Some("pages"),
    );
    assert_eq!(filtered[0]["noResults"], true, "{filtered}");

    // Without search/ the script falls back to search-index.json.
    fs::remove_dir_all(dist.join("search")).unwrap();
    let basic = run_script(
        &node,
        &dist,
        &dist.join("index.html"),
        &serde_json::json!([{"q": "café"}, {"q": "espresso"}]),
        None,
    );
    assert_eq!(titles(&basic[0]), ["Café culture"], "{basic}");
    assert_eq!(basic[1]["noResults"], true, "basic mode has no body text");
    assert!(basic[0]["allFetched"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "/search-index.json"));
}
