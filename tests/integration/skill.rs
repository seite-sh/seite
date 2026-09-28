use super::common::*;

// --- skill command ---

#[test]
fn test_skill_help() {
    page_cmd()
        .args(["skill", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("update"));
}

#[test]
fn test_skill_list_shows_bundled() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillsite", "Skill Test", "posts,pages");
    let site_dir = tmp.path().join("skillsite");

    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("theme-builder"))
        .stdout(predicate::str::contains("brand-identity"))
        .stdout(predicate::str::contains("landing-page"));
}

#[test]
fn test_skill_list_shows_available_packs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillpk", "Pack Test", "posts");
    let site_dir = tmp.path().join("skillpk");

    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seomachine"))
        .stdout(predicate::str::contains("Available packs"));
}

#[test]
fn test_skill_remove_nonexistent() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillrm", "Remove Test", "posts");
    let site_dir = tmp.path().join("skillrm");

    page_cmd()
        .args(["skill", "remove", "nonexistent-pack"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no skill or pack named"));
}

#[test]
fn test_skill_install_unknown_name() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillunk", "Unknown Test", "posts");
    let site_dir = tmp.path().join("skillunk");

    page_cmd()
        .args(["skill", "install", "nonexistent-pack-name"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown skill pack"));
}

#[test]
fn test_skill_remove_custom_skill() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillcust", "Custom Test", "posts");
    let site_dir = tmp.path().join("skillcust");

    // Manually create a custom skill
    let skill_dir = site_dir.join(".claude").join("skills").join("my-custom");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# My Custom Skill\n").unwrap();

    // Verify it shows up in list
    page_cmd()
        .args(["skill", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("my-custom"));

    // Remove it
    page_cmd()
        .args(["skill", "remove", "my-custom"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify it's gone
    assert!(!skill_dir.exists());
}

#[test]
fn test_skill_update_nothing_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skillupd", "Update Test", "posts");
    let site_dir = tmp.path().join("skillupd");

    page_cmd()
        .args(["skill", "update"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing to update"));
}

#[test]
fn test_skill_update_regenerates_links_map_without_packs() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "skilllinks", "Links Test", "posts,pages");
    let site_dir = tmp.path().join("skilllinks");

    // Serve posts under /blog, like seite-sh does.
    let config_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let rewritten = config.replace("url_prefix = \"/posts\"", "url_prefix = \"/blog\"");
    assert_ne!(config, rewritten, "expected posts url_prefix in seite.toml");
    fs::write(&config_path, rewritten).unwrap();

    fs::write(
        site_dir.join("content/posts/2026-03-13-hugo-alternative.md"),
        "---\ntitle: \"Hugo Alternative\"\n---\nBody\n",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/pages/about.md"),
        "---\ntitle: About\n---\nBody\n",
    )
    .unwrap();
    fs::create_dir_all(site_dir.join("context")).unwrap();

    page_cmd()
        .args(["skill", "update"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Regenerated internal links map"))
        .stdout(predicate::str::contains("Nothing to update").not());

    let map = fs::read_to_string(site_dir.join("context/internal-links-map.md")).unwrap();
    assert!(map.contains("- [Hugo Alternative](/blog/hugo-alternative)"));
    assert!(map.contains("- [About](/about)"));
    assert!(!map.contains("/posts/hugo-alternative"));
    assert!(!map.contains("/pages/about"));
}
