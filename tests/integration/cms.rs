//! `seite cms setup <pages-cms|decap>`.

use super::common::*;

/// A site with posts, docs and pages, plus a custom schema on posts.
fn cms_site(tmp: &TempDir) -> std::path::PathBuf {
    init_site(tmp, "site", "CMS", "posts,docs,pages");
    let site = tmp.path().join("site");
    let path = site.join("seite.toml");
    let config = fs::read_to_string(&path).unwrap();
    // Fields for posts (the first [[collections]]): insert the tables right
    // before the second [[collections]].
    assert!(config.contains("name = \"posts\""));
    let first = config.find("[[collections]]").unwrap();
    let second = config[first + 1..]
        .find("[[collections]]")
        .map(|i| i + first + 1)
        .unwrap();
    let fields = "[collections.fields.status]\ntype = \"enum\"\nenum = [\"draft\", \"published\"]\n\n[collections.fields.author]\ntype = \"string\"\ndescription = \"Who wrote it\"\n\n";
    let updated = format!("{}{}{}", &config[..second], fields, &config[second..]);
    fs::write(&path, updated).unwrap();
    site
}

fn yaml(path: &std::path::Path) -> serde_yaml_ng::Value {
    serde_yaml_ng::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn test_cms_setup_pages_cms_writes_config() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    let (ok, doc) = run_json(&site, &["cms", "setup", "pages-cms"]);
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["files"][0], ".pages.yml");
    let config = yaml(&site.join(".pages.yml"));
    assert_eq!(config["media"]["input"].as_str(), Some("static/images"));
    let content = config["content"].as_sequence().unwrap();
    let names: Vec<_> = content
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["posts", "docs", "pages"]);
    let posts = &content[0];
    assert_eq!(posts["path"].as_str(), Some("content/posts"));
    assert_eq!(
        posts["filename"].as_str(),
        Some("{year}-{month}-{day}-{primary}.md")
    );
    let fields = posts["fields"].as_sequence().unwrap();
    let status = fields
        .iter()
        .find(|f| f["name"].as_str() == Some("status"))
        .unwrap();
    assert_eq!(status["type"].as_str(), Some("select"));
    let author = fields
        .iter()
        .find(|f| f["name"].as_str() == Some("author"))
        .unwrap();
    assert_eq!(author["description"].as_str(), Some("Who wrote it"));
    // The CMS config passes `seite check` and does not change the build.
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "{doc:#}");
    assert!(!doc.to_string().contains("cms-config-stale"));
}

#[test]
fn test_cms_setup_decap_writes_admin() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    let (ok, doc) = run_json(&site, &["cms", "setup", "decap", "--branch", "main"]);
    assert!(ok, "{doc:#}");
    // Not a GitHub remote (no git repo in the temp dir): placeholder + note.
    assert_eq!(doc["data"]["repo"], "OWNER/REPO");
    assert!(doc["data"]["notes"][0]
        .as_str()
        .unwrap()
        .contains("backend.repo"));
    let config = yaml(&site.join("static/admin/config.yml"));
    assert_eq!(config["backend"]["name"].as_str(), Some("github"));
    assert_eq!(config["backend"]["branch"].as_str(), Some("main"));
    assert_eq!(config["media_folder"].as_str(), Some("static/images"));
    assert_eq!(config["public_folder"].as_str(), Some("/static/images"));
    let posts = &config["collections"][0];
    assert_eq!(posts["folder"].as_str(), Some("content/posts"));
    assert_eq!(posts["create"].as_bool(), Some(true));
    let index = fs::read_to_string(site.join("static/admin/index.html")).unwrap();
    assert!(index.contains("integrity=\"sha384-"));
    assert!(index.contains("content=\"noindex, nofollow\""));

    // The admin page ships as a static file but stays out of every
    // discovery surface and the audit.
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    assert!(site.join("dist/static/admin/index.html").exists());
    assert!(site.join("dist/static/admin/config.yml").exists());
    for file in [
        "sitemap.xml",
        "llms.txt",
        "llms-full.txt",
        "search-index.json",
    ] {
        let text = fs::read_to_string(site.join("dist").join(file)).unwrap_or_default();
        assert!(!text.contains("admin"), "{file} mentions the admin page");
    }
    let (_, audit) = run_json(&site, &["audit"]);
    for finding in audit["data"]["findings"].as_array().unwrap() {
        assert!(
            !finding.to_string().contains("static/admin"),
            "audit flags the CMS admin page: {finding:#}"
        );
    }
}

#[test]
fn test_cms_setup_reads_github_origin() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&site)
            .output()
            .unwrap()
    };
    if !git(&["init", "-q", "-b", "trunk"]).status.success() {
        return; // no git available
    }
    git(&["remote", "add", "origin", "git@github.com:acme/site.git"]);
    let (ok, doc) = run_json(&site, &["cms", "setup", "decap"]);
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["repo"], "acme/site");
    assert_eq!(doc["data"]["branch"], "trunk");
    let config = yaml(&site.join("static/admin/config.yml"));
    assert_eq!(config["backend"]["repo"].as_str(), Some("acme/site"));
}

#[test]
fn test_cms_setup_refuses_overwrite_and_force_backs_up() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    fs::write(site.join(".pages.yml"), "# mine\n").unwrap();
    page_cmd()
        .args(["cms", "setup", "pages-cms"])
        .current_dir(&site)
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    assert_eq!(
        fs::read_to_string(site.join(".pages.yml")).unwrap(),
        "# mine\n"
    );

    let (ok, doc) = run_json(&site, &["cms", "setup", "pages-cms", "--force"]);
    assert!(ok, "{doc:#}");
    assert_eq!(
        doc["data"]["backups"][0],
        ".seite/cache/backups/.pages.yml.bak"
    );
    assert_eq!(
        fs::read_to_string(site.join(".seite/cache/backups/.pages.yml.bak")).unwrap(),
        "# mine\n"
    );
    assert!(!site.join(".pages.yml.bak").exists());
    assert!(fs::read_to_string(site.join(".pages.yml"))
        .unwrap()
        .contains("content:"));
    // A second --force keeps the first backup.
    let (ok, doc) = run_json(&site, &["cms", "setup", "pages-cms", "--force"]);
    assert!(ok, "{doc:#}");
    assert_eq!(
        doc["data"]["backups"][0],
        ".seite/cache/backups/.pages.yml.1.bak"
    );
}

#[test]
fn test_cms_setup_decap_force_backups_are_not_deployed() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    let (ok, doc) = run_json(&site, &["cms", "setup", "decap", "--branch", "main"]);
    assert!(ok, "{doc:#}");
    let (ok, doc) = run_json(
        &site,
        &["cms", "setup", "decap", "--force", "--branch", "main"],
    );
    assert!(ok, "{doc:#}");
    assert_eq!(
        doc["data"]["backups"][1],
        ".seite/cache/backups/static/admin/config.yml.bak"
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    for entry in walkdir::WalkDir::new(site.join("dist")) {
        let entry = entry.unwrap();
        assert!(
            !entry.path().to_string_lossy().ends_with(".bak"),
            "{} deployed",
            entry.path().display()
        );
    }
    assert!(!site.join("static/admin/config.yml.bak").exists());
}

#[test]
fn test_cms_setup_site_in_repo_subdirectory() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(repo.join("sites")).unwrap();
    let inited = std::process::Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(&repo)
        .output();
    if !inited.is_ok_and(|o| o.status.success()) {
        return; // no git available
    }
    page_cmd()
        .args([
            "init",
            "blog",
            "--title",
            "Blog",
            "--description",
            "",
            "--deploy-target",
            "github-pages",
            "--collections",
            "posts,pages",
        ])
        .current_dir(repo.join("sites"))
        .assert()
        .success();
    let site = repo.join("sites/blog");

    let (ok, doc) = run_json(&site, &["cms", "setup", "pages-cms"]);
    assert!(ok, "{doc:#}");
    assert_eq!(doc["data"]["repo_prefix"], "sites/blog");
    assert!(!site.join(".pages.yml").exists());
    let config = yaml(&repo.join(".pages.yml"));
    assert_eq!(
        config["media"]["input"].as_str(),
        Some("sites/blog/static/images")
    );
    assert_eq!(
        config["content"][0]["path"].as_str(),
        Some("sites/blog/content/posts")
    );

    let (ok, doc) = run_json(&site, &["cms", "setup", "decap"]);
    assert!(ok, "{doc:#}");
    let config = yaml(&site.join("static/admin/config.yml"));
    assert_eq!(
        config["media_folder"].as_str(),
        Some("sites/blog/static/images")
    );
    assert_eq!(
        config["collections"][0]["folder"].as_str(),
        Some("sites/blog/content/posts")
    );

    // `seite check` resolves the prefixed paths: nothing stale ...
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "{doc:#}");
    assert!(!doc.to_string().contains("cms-config-stale"), "{doc:#}");
    // ... until a folder goes away (reported for both files).
    fs::remove_dir_all(site.join("content/pages")).unwrap();
    let (_, doc) = run_json(&site, &["check"]);
    let stale = doc.to_string().matches("sites/blog/content/pages").count();
    assert!(stale >= 2, "{doc:#}");
}

#[cfg(unix)]
#[test]
fn test_cms_setup_force_refuses_symlinked_target() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    let secret = tmp.path().join("secret.txt");
    fs::write(&secret, "outside the project\n").unwrap();
    std::os::unix::fs::symlink(&secret, site.join(".pages.yml")).unwrap();
    page_cmd()
        .args(["cms", "setup", "pages-cms", "--force"])
        .current_dir(&site)
        .assert()
        .failure();
    assert!(!site.join(".pages.yml.bak").exists());
    assert_eq!(
        fs::read_to_string(&secret).unwrap(),
        "outside the project\n"
    );
    assert!(fs::symlink_metadata(site.join(".pages.yml"))
        .unwrap()
        .file_type()
        .is_symlink());

    // A symlinked static/ directory: nothing is written through it.
    let outside = tmp.path().join("outside-static");
    fs::create_dir_all(outside.join("admin")).unwrap();
    fs::write(outside.join("admin/config.yml"), "mine\n").unwrap();
    fs::remove_dir_all(site.join("static")).unwrap();
    std::os::unix::fs::symlink(&outside, site.join("static")).unwrap();
    page_cmd()
        .args(["cms", "setup", "decap", "--force", "--branch", "main"])
        .current_dir(&site)
        .assert()
        .failure();
    assert!(!outside.join("admin/config.yml.bak").exists());
    assert!(!outside.join("admin/index.html").exists());
    assert_eq!(
        fs::read_to_string(outside.join("admin/config.yml")).unwrap(),
        "mine\n"
    );
}

#[test]
fn test_cms_setup_rejects_bad_branch() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    page_cmd()
        .args(["cms", "setup", "decap", "--branch", "x\"; y"])
        .current_dir(&site)
        .assert()
        .failure();
    assert!(!site.join("static/admin/config.yml").exists());
}

#[test]
fn test_check_reports_stale_cms_config() {
    let tmp = TempDir::new().unwrap();
    let site = cms_site(&tmp);
    let (ok, _) = run_json(&site, &["cms", "setup", "pages-cms"]);
    assert!(ok);
    fs::remove_dir_all(site.join("content/docs")).unwrap();
    let (ok, doc) = run_json(&site, &["check"]);
    assert!(ok, "warnings only: {doc:#}");
    let text = doc.to_string();
    assert!(text.contains("cms-config-stale"), "{doc:#}");
    assert!(text.contains("content/docs"), "{doc:#}");
}
