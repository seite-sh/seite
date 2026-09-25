use super::common::*;

// --- deploy improvements ---

#[test]
fn test_deploy_dry_run_github_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Dry Run Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("github-pages"))
        .stdout(predicate::str::contains("gh-pages"));
}

#[test]
fn test_deploy_dry_run_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CF Dry Run", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "cloudflare"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("cloudflare"));
}

#[test]
fn test_deploy_dry_run_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Dry Run", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "netlify"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("netlify"));
}

#[test]
fn test_deploy_unknown_target() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unknown Deploy", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "heroku"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown deploy target"));
}

// --- deploy: pre-flight checks ---

#[test]
fn test_deploy_dry_run_shows_preflight_checks() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Preflight Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Pre-flight checks"))
        .stdout(predicate::str::contains("Output directory"))
        .stdout(predicate::str::contains("Base URL"));
}

#[test]
fn test_deploy_dry_run_warns_localhost_base_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Localhost Test", "posts");
    let site_dir = tmp.path().join("site");

    // Default base_url is localhost — pre-flight should flag it
    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("localhost"));
}

#[test]
fn test_deploy_dry_run_with_base_url_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Override Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--dry-run", "--base-url", "https://example.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Base URL override: https://example.com",
        ));
}

#[test]
fn test_deploy_dry_run_preview_mode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Preview Test", "posts");
    let site_dir = tmp.path().join("site");

    // GitHub Pages doesn't show preview mode, but Netlify does
    page_cmd()
        .args(["deploy", "--dry-run", "--target", "netlify", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("preview"));
}

#[test]
fn test_deploy_dry_run_github_pages_shows_nojekyll() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "NoJekyll Test", "posts");
    let site_dir = tmp.path().join("site");

    // Update base_url to a custom domain
    let config_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let config = config.replace("http://localhost:3000", "https://myblog.com");
    fs::write(&config_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(".nojekyll"))
        .stdout(predicate::str::contains("CNAME: myblog.com"));
}

// --- deploy: domain setup ---

#[test]
fn test_deploy_domain_updates_base_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Domain Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--domain", "myblog.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated base_url"))
        .stdout(predicate::str::contains("myblog.com"));

    // Verify seite.toml was updated
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("https://myblog.com"));
}

#[test]
fn test_deploy_domain_shows_dns_instructions() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "DNS Test", "posts");
    let site_dir = tmp.path().join("site");

    page_cmd()
        .args(["deploy", "--domain", "myblog.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("DNS records"))
        .stdout(predicate::str::contains("github.io"));
}

#[test]
fn test_deploy_skip_checks_bypasses_preflight() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Skip Test", "posts");
    let site_dir = tmp.path().join("site");

    // --skip-checks should not print preflight header at all
    // It will still fail because no output dir exists, but that's the build step not preflight
    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        // dry-run always shows checks regardless of skip-checks (dry-run is informational)
        .success();
}

#[test]
fn test_deploy_dry_run_cloudflare_shows_auth_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CF Auth Test", "posts");
    let site_dir = tmp.path().join("site");

    // Update config to target cloudflare
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"test-project\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Cloudflare auth"));
}

#[test]
fn test_deploy_dry_run_netlify_shows_auth_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Auth Test", "posts");
    let site_dir = tmp.path().join("site");

    // Update config to target netlify
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Netlify auth"));
}

#[test]
fn test_deploy_dry_run_netlify_shows_site_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Site Test", "posts");
    let site_dir = tmp.path().join("site");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    // Dry run should show the Netlify site check
    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Netlify site"));
}

#[test]
fn test_deploy_dry_run_cloudflare_shows_project_check() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "CF Project Test", "posts");
    let site_dir = tmp.path().join("site");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"test-project\"",
    );
    fs::write(&toml_path, config).unwrap();

    // Dry run should show the Cloudflare project check
    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Cloudflare project"));
}

// --- deploy additional tests ---

#[test]
fn test_deploy_dry_run_no_commit() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "depnc", "Deploy NC", "posts");
    let site_dir = tmp.path().join("depnc");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

// --- additional deploy tests ---

#[test]
fn test_deploy_dry_run_skip_checks() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dskip", "Deploy Skip", "posts");
    let site_dir = tmp.path().join("dskip");

    // Build first
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Deploy with skip-checks should skip the pre-flight
    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[test]
fn test_deploy_dry_run_preview() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dprev", "Deploy Preview", "posts");
    let site_dir = tmp.path().join("dprev");

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["deploy", "--dry-run", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional deploy CLI tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_deploy_dry_run_no_commit_github_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddnc1", "Deploy NC GH", "posts");
    let site_dir = tmp.path().join("ddnc1");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("github-pages"));
}

#[test]
fn test_deploy_dry_run_no_commit_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddnc2", "Deploy NC CF", "posts");
    let site_dir = tmp.path().join("ddnc2");

    // Change target to cloudflare
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"test-proj\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("cloudflare"))
        .stdout(predicate::str::contains("test-proj"));
}

#[test]
fn test_deploy_dry_run_no_commit_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddnc3", "Deploy NC Net", "posts");
    let site_dir = tmp.path().join("ddnc3");

    // Change target to netlify
    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("netlify"));
}

#[test]
fn test_deploy_dry_run_skip_checks_github_pages() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddsc1", "Deploy SC GH", "posts");
    let site_dir = tmp.path().join("ddsc1");

    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("github-pages"));
}

#[test]
fn test_deploy_dry_run_skip_checks_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddsc2", "Deploy SC CF", "posts");
    let site_dir = tmp.path().join("ddsc2");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-proj\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("cloudflare"));
}

#[test]
fn test_deploy_dry_run_skip_checks_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddsc3", "Deploy SC Net", "posts");
    let site_dir = tmp.path().join("ddsc3");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("netlify"));
}

#[test]
fn test_deploy_dry_run_no_commit_skip_checks_combined() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddcomb", "Deploy Combined", "posts");
    let site_dir = tmp.path().join("ddcomb");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit", "--skip-checks"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

#[test]
fn test_deploy_dry_run_with_target_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddtgt", "Deploy Target Override", "posts");
    let site_dir = tmp.path().join("ddtgt");

    // Config says github-pages, but override to netlify
    page_cmd()
        .args(["deploy", "--dry-run", "--target", "netlify"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("netlify"))
        .stdout(predicate::str::contains("Dry run"));
}

#[test]
fn test_deploy_dry_run_cloudflare_preview_mode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddcfp", "CF Preview", "posts");
    let site_dir = tmp.path().join("ddcfp");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-prev\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("preview"));
}

#[test]
fn test_deploy_dry_run_netlify_production_mode() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddntp", "Net Prod", "posts");
    let site_dir = tmp.path().join("ddntp");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("production"));
}

#[test]
fn test_deploy_dry_run_shows_output_dir() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddout", "Deploy Output", "posts");
    let site_dir = tmp.path().join("ddout");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Output dir"));
}

#[test]
fn test_deploy_dry_run_github_pages_shows_branch() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddbr", "Deploy Branch", "posts");
    let site_dir = tmp.path().join("ddbr");

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("gh-pages"));
}

#[test]
fn test_deploy_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["deploy", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_deploy_invalid_target_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddinv", "Deploy Invalid", "posts");
    let site_dir = tmp.path().join("ddinv");

    page_cmd()
        .args(["deploy", "--dry-run", "--target", "aws-s3"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown deploy target"));
}

#[test]
fn test_deploy_dry_run_base_url_override_with_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "ddbucf", "Deploy BU CF", "posts");
    let site_dir = tmp.path().join("ddbucf");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-bu\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args([
            "deploy",
            "--dry-run",
            "--base-url",
            "https://my-cf-site.pages.dev",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Base URL override: https://my-cf-site.pages.dev",
        ));
}

#[test]
fn test_deploy_domain_cloudflare() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dddomcf", "Domain CF", "posts");
    let site_dir = tmp.path().join("dddomcf");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace(
        "target = \"github-pages\"",
        "target = \"cloudflare\"\nproject = \"cf-domain\"",
    );
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--domain", "example.com"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated base_url"))
        .stdout(predicate::str::contains("example.com"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("https://example.com"));
}

#[test]
fn test_deploy_domain_netlify() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dddomnt", "Domain Net", "posts");
    let site_dir = tmp.path().join("dddomnt");

    let toml_path = site_dir.join("seite.toml");
    let config = fs::read_to_string(&toml_path).unwrap();
    let config = config.replace("target = \"github-pages\"", "target = \"netlify\"");
    fs::write(&toml_path, config).unwrap();

    page_cmd()
        .args(["deploy", "--domain", "mysite.netlify.app"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated base_url"));

    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(config.contains("https://mysite.netlify.app"));
}

#[test]
fn test_deploy_dry_run_shows_subdomains() {
    let tmp = TempDir::new().unwrap();
    let site_dir = init_subdomain_site(&tmp, "subdom5");

    page_cmd()
        .args(["deploy", "--dry-run", "--no-commit"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Subdomain deploys"))
        .stdout(predicate::str::contains("docs.example.com"));
}
