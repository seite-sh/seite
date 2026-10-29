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

    // GitHub Pages has no previews: a "preview" would replace the live site.
    page_cmd()
        .args(["deploy", "--dry-run", "--preview"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no preview deploys"));
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
fn test_deploy_without_tty_refuses_failed_preflight_before_any_side_effect() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Repo", "posts,pages");
    let site = tmp.path().join("site");
    fs::write(
        site.join("seite.toml"),
        fs::read_to_string(site.join("seite.toml"))
            .unwrap()
            .replace("http://localhost:3000", "https://example.com"),
    )
    .unwrap();
    assert!(!site.join(".git").exists());

    // stdin is not a terminal and there is no --yes: the "Git repository"
    // check can't be auto-fixed and deploying anyway must not be assumed.
    let output = page_cmd()
        .arg("deploy")
        .current_dir(&site)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("confirmation required"), "{stderr}");
    assert!(stderr.contains("--yes"), "{stderr}");
    assert!(stderr.contains("--skip-checks"), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Git repository"), "{stdout}");
    // No `git init`, no commit, no build output.
    assert!(!site.join(".git").exists(), "deploy must not init a repo");
    assert!(!site.join("dist").exists(), "deploy must not build");
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

/// A localhost base_url is the only failing pre-flight check: without a TTY
/// and without --yes, deploy must refuse before committing or pushing (it
/// used to warn and carry on).
#[test]
fn test_deploy_without_tty_refuses_localhost_base_url_before_side_effects() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Localhost", "posts,pages");
    let site = tmp.path().join("site");
    let remote = tmp.path().join("remote.git");
    let git = |dir: &std::path::Path, args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    git(tmp.path(), &["init", "--bare", "-q", "remote.git"]);
    git(&site, &["init", "-q", "-b", "main"]);
    git(
        &site,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    page_cmd()
        .arg("build")
        .current_dir(&site)
        .assert()
        .success();
    git(&site, &["add", "-A"]);
    git(
        &site,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"],
    );
    let head = git(&site, &["rev-parse", "HEAD"]);
    // Leave a change behind that auto-commit would pick up.
    fs::write(
        site.join("content/pages/new.md"),
        "---\ntitle: New\n---\nx\n",
    )
    .unwrap();

    let output = page_cmd()
        .arg("deploy")
        .current_dir(&site)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("confirmation required"), "{stderr}");
    assert!(stderr.contains("--base-url"), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Base URL"), "{stdout}");
    assert_eq!(git(&site, &["rev-parse", "HEAD"]), head, "no auto-commit");
    assert_eq!(git(&remote, &["for-each-ref"]), "", "nothing pushed");
}

/// A git repository on `branch` with one commit (rev-parse fails before it).
fn git_repo_on_branch(dir: &std::path::Path, branch: &str) {
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=seite test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?}: {status:?}");
    };
    git(&["init", "-q", "-b", branch]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
}

fn git_commit_count(dir: &std::path::Path) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn test_deploy_dry_run_reports_branch_preview_and_push() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Branch Plan", "posts");
    let site_dir = tmp.path().join("site");
    git_repo_on_branch(&site_dir, "feature");

    // Cloudflare from a non-main branch: the same auto-preview as a real
    // deploy, and it says it would commit and push.
    page_cmd()
        .args(["deploy", "--dry-run", "--target", "cloudflare"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Source branch: feature"))
        .stdout(predicate::str::contains("Mode: preview"))
        .stdout(predicate::str::contains(
            "would commit all changes and push feature",
        ));

    let out = page_cmd()
        .args(["--json", "deploy", "--dry-run", "--target", "netlify"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["data"]["preview"], true, "{doc}");
    assert_eq!(doc["data"]["mode"], "preview");
    assert_eq!(doc["data"]["auto_preview"], true);
    assert_eq!(doc["data"]["branch"], "feature");
    assert_eq!(doc["data"]["commit_and_push"], true);

    // --no-commit: no push, no auto-preview.
    page_cmd()
        .args([
            "deploy",
            "--dry-run",
            "--target",
            "cloudflare",
            "--no-commit",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Mode: production"))
        .stdout(predicate::str::contains("no commit or push"));

    // GitHub Pages from a non-main branch: the dry run reports the refusal
    // the real deploy would give.
    let out = page_cmd()
        .args(["--json", "deploy", "--dry-run", "--target", "github-pages"])
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        doc["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("GitHub Pages has no preview deploys"),
        "{doc}"
    );
    assert_eq!(doc["data"]["preview"], true, "{doc}");
}

#[test]
fn test_deploy_github_pages_refuses_branch_before_committing() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Push", "posts");
    let site_dir = tmp.path().join("site");
    git_repo_on_branch(&site_dir, "feature");
    fs::write(
        site_dir.join("content/posts/new.md"),
        "---\ntitle: New\n---\n",
    )
    .unwrap();

    page_cmd()
        .args(["deploy", "--skip-checks", "--target", "github-pages"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "GitHub Pages has no preview deploys",
        ));
    // Refused before auto-commit: nothing committed (or pushed).
    assert_eq!(git_commit_count(&site_dir), "1");
}

#[cfg(unix)]
#[test]
fn test_deploy_yes_never_creates_public_github_repo() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Repo", "posts");
    let site_dir = tmp.path().join("site");
    // A fake `gh` that records every call.
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let log = tmp.path().join("gh.log");
    let gh = bin.join("gh");
    fs::write(
        &gh,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display()),
    )
    .unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let out = page_cmd()
        .args(["deploy", "--yes", "--target", "github-pages"])
        .env("PATH", path)
        .current_dir(&site_dir)
        .output()
        .unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let calls = fs::read_to_string(&log).unwrap_or_default();
    assert!(!calls.contains("repo create"), "gh called: {calls}\n{all}");
    assert!(all.contains("it would be public"), "{all}");
}

/// Without --yes, a run that can't ask (stdin not a terminal) never creates
/// the public repository either.
#[cfg(unix)]
#[test]
fn test_deploy_without_tty_never_creates_public_github_repo() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Repo", "posts");
    let site_dir = tmp.path().join("site");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let log = tmp.path().join("gh.log");
    let gh = bin.join("gh");
    fs::write(
        &gh,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display()),
    )
    .unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    git_repo_on_branch(&site_dir, "main");

    let out = page_cmd()
        .args(["deploy", "--target", "github-pages"])
        .env("PATH", path)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .current_dir(&site_dir)
        .output()
        .unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let calls = fs::read_to_string(&log).unwrap_or_default();
    assert!(!calls.contains("repo create"), "gh called: {calls}\n{all}");
    assert!(all.contains("it would be public"), "{all}");
}

/// Outside a git repository the dry run says auto-commit would be skipped
/// (and why), and `--json` reports no commit or push.
#[test]
fn test_deploy_dry_run_without_git_repo_skips_auto_commit() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "No Git", "posts");
    let site_dir = tmp.path().join("site");
    assert!(!site_dir.join(".git").exists());
    page_cmd()
        .args(["deploy", "--dry-run", "--target", "cloudflare"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("auto-commit would be skipped"));
    let (ok, doc) = run_json(
        &site_dir,
        &["deploy", "--dry-run", "--target", "cloudflare"],
    );
    assert!(ok, "{doc}");
    assert_eq!(doc["data"]["commit_and_push"], false, "{doc}");
    assert_eq!(doc["data"]["dry_run"], true, "{doc}");
}

/// A directory holding fake CLIs (`name` → exit 0, args logged to
/// `<bin>/<name>.log`), and a PATH with it first.
#[cfg(unix)]
fn fake_clis(tmp: &TempDir, names: &[&str]) -> (std::path::PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    for name in names {
        let exe = bin.join(name);
        fs::write(
            &exe,
            format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n",
                bin.join(format!("{name}.log")).display()
            ),
        )
        .unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (bin, path)
}

/// Netlify builds netlify.toml from git: `--setup` writes no GitHub Actions
/// workflow (it would deploy twice).
#[cfg(unix)]
#[test]
fn test_deploy_setup_netlify_writes_no_github_workflow() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Netlify Setup", "posts");
    let site_dir = tmp.path().join("site");
    // `seite init` wrote a GitHub Pages workflow; setup must not write one.
    let _ = fs::remove_file(site_dir.join(".github/workflows/deploy.yml"));
    let (bin, path) = fake_clis(&tmp, &["netlify"]);
    let out = page_cmd()
        .args(["deploy", "--setup", "--target", "netlify"])
        .env("PATH", path)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .current_dir(&site_dir)
        .output()
        .unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{all}");
    assert!(
        fs::read_to_string(bin.join("netlify.log"))
            .unwrap()
            .contains("sites:create"),
        "{all}"
    );
    assert!(site_dir.join("netlify.toml").is_file(), "{all}");
    assert!(
        !site_dir.join(".github/workflows/deploy.yml").exists(),
        "{all}"
    );
}

/// The Cloudflare workflow `--setup` writes deploys to the project it just
/// set up, never to the `your-project-name` placeholder.
#[cfg(unix)]
#[test]
fn test_deploy_setup_cloudflare_workflow_names_the_project() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "my-cf-site", "Cloudflare Setup", "posts");
    let site_dir = tmp.path().join("my-cf-site");
    let _ = fs::remove_file(site_dir.join(".github/workflows/deploy.yml"));
    let (_bin, path) = fake_clis(&tmp, &["wrangler"]);
    let out = page_cmd()
        .args(["deploy", "--setup", "--target", "cloudflare"])
        .env("PATH", path)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .current_dir(&site_dir)
        .output()
        .unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{all}");
    let workflow = fs::read_to_string(site_dir.join(".github/workflows/deploy.yml")).unwrap();
    assert!(
        workflow.contains("--project-name my-cf-site\n"),
        "{workflow}"
    );
    assert!(!workflow.contains("your-project-name"), "{workflow}");
    assert!(!workflow.contains("TODO"), "{workflow}");
    let toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(toml.contains("project = \"my-cf-site\""), "{toml}");

    // A configured project wins over the directory name.
    fs::write(
        site_dir.join("seite.toml"),
        toml.replace("project = \"my-cf-site\"", "project = \"other-project\""),
    )
    .unwrap();
    let (_bin2, path2) = fake_clis(&tmp, &["wrangler"]);
    let out = page_cmd()
        .args(["deploy", "--setup", "--target", "cloudflare"])
        .env("PATH", path2)
        .env_remove("SEITE_YES")
        .write_stdin("")
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    let workflow = fs::read_to_string(site_dir.join(".github/workflows/deploy.yml")).unwrap();
    assert!(
        workflow.contains("--project-name other-project\n"),
        "{workflow}"
    );
    let toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(toml.contains("project = \"other-project\""), "{toml}");
}

#[cfg(unix)]
#[test]
fn test_deploy_setup_github_pages_yes_never_creates_public_repo() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Setup No Repo", "posts");
    let site_dir = tmp.path().join("site");
    let (bin, path) = fake_clis(&tmp, &["gh"]);
    let out = page_cmd()
        .args(["deploy", "--setup", "--yes", "--target", "github-pages"])
        .env("PATH", path)
        .current_dir(&site_dir)
        .output()
        .unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let calls = fs::read_to_string(bin.join("gh.log")).unwrap_or_default();
    assert!(!calls.contains("repo create"), "gh called: {calls}\n{all}");
    assert!(all.contains("it would be public"), "{all}");
}
