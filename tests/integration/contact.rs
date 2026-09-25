use super::common::*;

#[test]
fn test_contact_status_no_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "stsite", "Status", "posts,pages");

    page_cmd()
        .args(["contact", "status"])
        .current_dir(tmp.path().join("stsite"))
        .assert()
        .success()
        .stdout(predicate::str::contains("seite contact setup"));
}

#[test]
fn test_contact_status_with_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "stsite2", "Status", "posts,pages");
    let site_dir = tmp.path().join("stsite2");

    add_contact_config(&site_dir, "formspree", "xtest789");

    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"))
        .stdout(predicate::str::contains("xtest789"));
}

#[test]
fn test_contact_setup_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "setupsite", "Setup", "posts,pages");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "mykey123",
        ])
        .current_dir(tmp.path().join("setupsite"))
        .assert()
        .success();

    let config = fs::read_to_string(tmp.path().join("setupsite/seite.toml")).unwrap();
    assert!(config.contains("[contact]"));
    assert!(config.contains("web3forms"));
    assert!(config.contains("mykey123"));
}

#[test]
fn test_contact_remove() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "rmsite", "Remove", "posts,pages");
    let site_dir = tmp.path().join("rmsite");

    add_contact_config(&site_dir, "formspree", "xremove");

    // Verify config has [contact]
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("[contact]"));

    page_cmd()
        .args(["contact", "remove"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify [contact] was removed
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(!config.contains("[contact]"));
}

// --- contact form additions ---

#[test]
fn test_contact_setup_web3forms() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctweb3", "Web3 Contact", "posts,pages");
    let site_dir = tmp.path().join("ctweb3");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "abc123",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("web3forms"));
    assert!(config.contains("abc123"));
}

#[test]
fn test_contact_setup_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctnet", "Netlify Contact", "posts,pages");
    let site_dir = tmp.path().join("ctnet");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "netlify",
            "--endpoint",
            "contact",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("netlify"));
}

#[test]
fn test_contact_setup_creates_contact_page() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctpage", "Contact Page", "posts,pages");
    let site_dir = tmp.path().join("ctpage");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xtest",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Should create a contact page when pages collection exists
    assert!(site_dir.join("content/pages/contact.md").exists());
    let contact = fs::read_to_string(site_dir.join("content/pages/contact.md")).unwrap();
    assert!(contact.contains("contact_form"));
}

// --- contact remove when not configured ---

#[test]
fn test_contact_remove_when_not_configured() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "cr2", "Contact Remove 2", "posts");
    let site_dir = tmp.path().join("cr2");

    page_cmd()
        .args(["contact", "remove"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("No contact form"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional contact CLI tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_contact_setup_formspree_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctfsp", "Formspree Setup", "posts,pages");
    let site_dir = tmp.path().join("ctfsp");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xpznqkdl",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("[contact]"));
    assert!(config.contains("formspree"));
    assert!(config.contains("xpznqkdl"));
}

#[test]
fn test_contact_setup_formspree_then_status() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctfst", "FS Status", "posts,pages");
    let site_dir = tmp.path().join("ctfst");

    // Setup formspree
    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xtest456",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Check status
    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"))
        .stdout(predicate::str::contains("xtest456"));
}

#[test]
fn test_contact_setup_then_remove_then_status() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctrms", "Remove Status", "posts,pages");
    let site_dir = tmp.path().join("ctrms");

    // Setup
    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xrm123",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify config present
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("[contact]"));

    // Remove
    page_cmd()
        .args(["contact", "remove"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    // Verify config gone
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(!config.contains("[contact]"));

    // Status should show not configured
    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("seite contact setup"));
}

#[test]
fn test_contact_setup_web3forms_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctw3f2", "Web3 Setup2", "posts,pages");
    let site_dir = tmp.path().join("ctw3f2");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "w3key789",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Web3Forms"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("web3forms"));
    assert!(config.contains("w3key789"));
}

#[test]
fn test_contact_setup_netlify_noninteractive() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctnet2", "Netlify Setup2", "posts,pages");
    let site_dir = tmp.path().join("ctnet2");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "netlify",
            "--endpoint",
            "myform",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Netlify"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("netlify"));
    assert!(config.contains("myform"));
}

#[test]
fn test_contact_setup_with_redirect_and_subject() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctopt", "Contact Opts", "posts,pages");
    let site_dir = tmp.path().join("ctopt");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xopts",
            "--redirect",
            "/thank-you",
            "--subject",
            "New inquiry",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("formspree"));
    assert!(config.contains("xopts"));
    assert!(config.contains("/thank-you"));
    assert!(config.contains("New inquiry"));
}

#[test]
fn test_contact_setup_invalid_provider() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctinv", "Invalid Provider", "posts,pages");
    let site_dir = tmp.path().join("ctinv");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "invalid_provider",
            "--endpoint",
            "x123",
        ])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown contact provider"));
}

#[test]
fn test_contact_status_shows_redirect_and_subject() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctstat2", "Status Detail", "posts,pages");
    let site_dir = tmp.path().join("ctstat2");

    // Add contact config with redirect and subject
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[contact]\nprovider = \"formspree\"\nendpoint = \"xdetail\"\nredirect = \"/thanks\"\nsubject = \"Contact Form\"\n",
    );
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["contact", "status"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Formspree"))
        .stdout(predicate::str::contains("xdetail"))
        .stdout(predicate::str::contains("/thanks"))
        .stdout(predicate::str::contains("Contact Form"));
}

#[test]
fn test_contact_setup_over_existing_config_needs_yes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Replace", "posts,pages");
    let site = tmp.path().join("site");
    add_contact_config(&site, "formspree", "xoriginal");
    let before = fs::read_to_string(site.join("seite.toml")).unwrap();
    let setup = [
        "contact",
        "setup",
        "--provider",
        "web3forms",
        "--endpoint",
        "wnew",
    ];

    page_cmd()
        .args(setup)
        .current_dir(&site)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("confirmation required"))
        .stderr(predicate::str::contains("--yes"));
    assert_eq!(fs::read_to_string(site.join("seite.toml")).unwrap(), before);
    assert!(!site.join("content/pages/contact.md").exists());

    page_cmd()
        .arg("--yes")
        .args(setup)
        .current_dir(&site)
        .assert()
        .success();
    let after = fs::read_to_string(site.join("seite.toml")).unwrap();
    assert!(after.contains("wnew"), "{after}");
    assert!(!after.contains("xoriginal"), "{after}");
}

#[test]
fn test_contact_setup_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "x123",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_contact_setup_creates_contact_page_with_shortcode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctpg2", "Contact Page2", "posts,pages");
    let site_dir = tmp.path().join("ctpg2");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "web3forms",
            "--endpoint",
            "w3key",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify contact page was created
    let contact_page = site_dir.join("content/pages/contact.md");
    assert!(contact_page.exists());
    let content = fs::read_to_string(&contact_page).unwrap();
    assert!(content.contains("contact_form"));
    assert!(content.contains("title: Contact"));
}

#[test]
fn test_contact_setup_no_contact_page_without_pages_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ctnopg", "No Pages", "posts");
    let site_dir = tmp.path().join("ctnopg");

    page_cmd()
        .args([
            "contact",
            "setup",
            "--provider",
            "formspree",
            "--endpoint",
            "xnopg",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // No contact page should be created when no pages collection
    assert!(!site_dir.join("content/pages/contact.md").exists());
}
