use super::common::*;

/// `--json`: a child process that writes to stdout (here a fake `claude` for
/// `theme create`) must not corrupt the JSON document; its output goes to
/// stderr instead.
#[cfg(unix)]
#[test]
fn test_theme_create_json_keeps_child_stdout_off_stdout() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Child Stdout", "posts");
    let site = tmp.path().join("site");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let fake = bin.join("claude");
    fs::write(
        &fake,
        "#!/bin/sh\necho \"noise from child stdout\"\necho '<html></html>' > templates/base.html\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = page_cmd()
        .args(["--json", "theme", "create", "minimal"])
        .env("PATH", path)
        .current_dir(&site)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let doc = json_stdout(&output);
    assert_eq!(doc["ok"], true);
    assert!(String::from_utf8_lossy(&output.stderr).contains("noise from child stdout"));
    assert!(site.join("templates/base.html").exists());
}

// --- theme command ---

#[test]
fn test_theme_list() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("brutalist"))
        .stdout(predicate::str::contains("bento"));
}

#[test]
fn test_theme_apply_brutalist() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Brutalist Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "brutalist"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("fffef0"),
        "brutalist theme should have cream background"
    );
    assert!(
        base.contains("ffe600"),
        "brutalist theme should have yellow accent"
    );
}

#[test]
fn test_theme_apply_bento() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Bento Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "bento"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("border-radius: 20px"),
        "bento theme should have rounded cards"
    );
    assert!(
        base.contains("5046e5"),
        "bento theme should have indigo accent"
    );
}

#[test]
fn test_theme_apply_dark_revised() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Dark Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("0a0a0a"),
        "dark theme should use true black background"
    );
    assert!(
        base.contains("8b5cf6"),
        "dark theme should use violet accent"
    );
}

#[test]
fn test_theme_create_requires_page_toml() {
    // `seite theme create` without a seite.toml in the directory should fail gracefully
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["theme", "create", "dark glassmorphism"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_theme_list_shows_bundled() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bundled themes"))
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("minimal"))
        .stdout(predicate::str::contains("dark"))
        .stdout(predicate::str::contains("docs"))
        .stdout(predicate::str::contains("brutalist"))
        .stdout(predicate::str::contains("bento"));
}

#[test]
fn test_theme_apply_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Installed Theme Test", "posts");
    let site_dir = tmp.path().join("site");

    // Create a fake installed theme
    let themes_dir = site_dir.join("templates").join("themes");
    std::fs::create_dir_all(&themes_dir).unwrap();
    std::fs::write(
        themes_dir.join("custom-test.tera"),
        "{#- theme-description: A test installed theme -#}\n<!DOCTYPE html>\n<html><head><title>Custom</title></head><body>custom-test-marker</body></html>",
    ).unwrap();

    // Apply the installed theme
    page_cmd()
        .args(["theme", "apply", "custom-test"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied installed theme"));

    // Verify base.html was updated
    let base = std::fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("custom-test-marker"),
        "installed theme should be applied"
    );
}

#[test]
fn test_theme_apply_unknown_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Unknown Theme Test", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "apply", "nonexistent-theme-xyz"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown theme"));
}

#[test]
fn test_theme_export() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Export Test", "posts");
    let site_dir = tmp.path().join("site");

    // Apply a theme first so templates/base.html exists
    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export it
    page_cmd()
        .args([
            "theme",
            "export",
            "my-dark",
            "--description",
            "My custom dark theme",
        ])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported theme 'my-dark'"));

    // Verify the exported file exists and has metadata
    let exported = std::fs::read_to_string(site_dir.join("templates/themes/my-dark.tera")).unwrap();
    assert!(
        exported.contains("theme-description: My custom dark theme"),
        "exported theme should have description"
    );
    assert!(
        exported.contains("0a0a0a"),
        "exported theme should contain dark theme content"
    );
}

#[test]
fn test_theme_export_no_base_html() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Export No Base Test", "posts");
    let site_dir = tmp.path().join("site");

    // Remove base.html to test error case
    let _ = std::fs::remove_file(site_dir.join("templates/base.html"));

    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no templates/base.html found"));
}

#[test]
fn test_theme_export_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Export Dup Test", "posts");
    let site_dir = tmp.path().join("site");

    // Apply a theme
    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export once
    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export again with same name should fail
    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_theme_install_requires_page_toml() {
    let tmp = TempDir::new().unwrap();
    page_cmd()
        .args(["theme", "install", "https://example.com/theme.tera"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_theme_list_shows_installed() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Installed List Test", "posts");
    let site_dir = tmp.path().join("site");

    // Create an installed theme
    let themes_dir = site_dir.join("templates").join("themes");
    std::fs::create_dir_all(&themes_dir).unwrap();
    std::fs::write(
        themes_dir.join("my-custom.tera"),
        "{#- theme-description: A custom community theme -#}\n<!DOCTYPE html><html></html>",
    )
    .unwrap();

    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed themes"))
        .stdout(predicate::str::contains("my-custom"))
        .stdout(predicate::str::contains("A custom community theme"));
}

// --- additional theme tests ---

#[test]
fn test_theme_apply_all_bundled() {
    // Verify every bundled theme can be applied
    let themes = [
        "default",
        "minimal",
        "dark",
        "docs",
        "brutalist",
        "bento",
        "landing",
        "terminal",
        "magazine",
        "academic",
    ];
    for theme_name in &themes {
        let tmp = TempDir::new().unwrap();
        init_site(&tmp, "thm", "Theme Test", "posts");
        let site_dir = tmp.path().join("thm");

        page_cmd()
            .args(["theme", "apply", theme_name])
            .current_dir(&site_dir)
            .assert()
            .success();

        // Verify base.html was written
        let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
        assert!(!base.is_empty());
    }
}

// --- theme export with description verification ---

#[test]
fn test_theme_export_with_description_metadata() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "texps2", "Theme Export Desc", "posts");
    let site_dir = tmp.path().join("texps2");

    page_cmd()
        .args(["theme", "apply", "minimal"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args([
            "theme",
            "export",
            "my-custom2",
            "--description",
            "My custom theme",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    let content = fs::read_to_string(site_dir.join("templates/themes/my-custom2.tera")).unwrap();
    assert!(content.contains("theme-description: My custom theme"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Additional theme CLI tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_theme_list_shows_all_bundled() {
    page_cmd()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bundled themes"))
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("minimal"))
        .stdout(predicate::str::contains("dark"))
        .stdout(predicate::str::contains("docs"))
        .stdout(predicate::str::contains("brutalist"))
        .stdout(predicate::str::contains("bento"))
        .stdout(predicate::str::contains("landing"))
        .stdout(predicate::str::contains("terminal"))
        .stdout(predicate::str::contains("magazine"))
        .stdout(predicate::str::contains("academic"));
}

#[test]
fn test_theme_apply_dark_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thdkb", "Dark Build", "posts");
    let site_dir = tmp.path().join("thdkb");

    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Build should succeed with dark theme applied
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify the output uses dark theme styles
    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index.contains("0a0a0a"), "dark theme should use true black");
}

#[test]
fn test_theme_apply_minimal_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thmnb", "Minimal Build", "posts");
    let site_dir = tmp.path().join("thmnb");

    page_cmd()
        .args(["theme", "apply", "minimal"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Build should succeed with minimal theme applied
    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify the output uses minimal theme styles
    let index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        index.contains("Georgia"),
        "minimal theme should use Georgia serif"
    );
}

#[test]
fn test_theme_apply_docs_then_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thdob", "Docs Build", "posts,docs");
    let site_dir = tmp.path().join("thdob");

    page_cmd()
        .args(["theme", "apply", "docs"])
        .current_dir(&site_dir)
        .assert()
        .success();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    assert!(site_dir.join("dist/index.html").exists());
}

#[test]
fn test_theme_apply_nonexistent_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thfail", "Fail Theme", "posts");
    let site_dir = tmp.path().join("thfail");

    page_cmd()
        .args(["theme", "apply", "totally-fake-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown theme"));
}

#[test]
fn test_theme_install_from_local_file_url() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thinst", "Install Test", "posts");
    let site_dir = tmp.path().join("thinst");

    // Create a temporary theme file to serve via file:// won't work (ureq http-only),
    // so create it directly in the installed themes dir to test the apply path
    let themes_dir = site_dir.join("templates/themes");
    fs::create_dir_all(&themes_dir).unwrap();
    fs::write(
        themes_dir.join("local-test.tera"),
        "{#- theme-description: A local test theme -#}\n<!DOCTYPE html>\n<html lang=\"{{ lang }}\"><head><title>{% block title %}{{ site.title }}{% endblock %}</title></head><body>local-test-marker{% block content %}{% endblock %}</body></html>",
    ).unwrap();

    // Verify it shows up in theme list
    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed themes"))
        .stdout(predicate::str::contains("local-test"));

    // Apply the installed theme
    page_cmd()
        .args(["theme", "apply", "local-test"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied installed theme"));

    let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("local-test-marker"));
}

#[test]
fn test_theme_export_then_apply() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thexp2", "Export Apply", "posts");
    let site_dir = tmp.path().join("thexp2");

    // Apply brutalist first
    page_cmd()
        .args(["theme", "apply", "brutalist"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export it
    page_cmd()
        .args([
            "theme",
            "export",
            "my-brutalist",
            "--description",
            "My modified brutalist",
        ])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Verify exported file exists
    let exported = site_dir.join("templates/themes/my-brutalist.tera");
    assert!(exported.exists());
    let content = fs::read_to_string(&exported).unwrap();
    assert!(content.contains("theme-description: My modified brutalist"));

    // Now apply a different theme
    page_cmd()
        .args(["theme", "apply", "minimal"])
        .current_dir(&site_dir)
        .assert()
        .success();

    let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(base.contains("Georgia"), "should now be minimal theme");

    // Apply the exported theme
    page_cmd()
        .args(["theme", "apply", "my-brutalist"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Applied installed theme"));

    let base = fs::read_to_string(site_dir.join("templates/base.html")).unwrap();
    assert!(
        base.contains("fffef0"),
        "should be back to brutalist cream background"
    );
}

#[test]
fn test_theme_list_shows_installed_themes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thlst2", "List Installed", "posts");
    let site_dir = tmp.path().join("thlst2");

    // No installed themes initially — should only show bundled
    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Install a theme manually
    let themes_dir = site_dir.join("templates/themes");
    fs::create_dir_all(&themes_dir).unwrap();
    fs::write(
        themes_dir.join("my-fancy.tera"),
        "{#- theme-description: A fancy theme -#}\n<!DOCTYPE html><html><body>fancy</body></html>",
    )
    .unwrap();

    // Now list should show installed section
    page_cmd()
        .args(["theme", "list"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed themes"))
        .stdout(predicate::str::contains("my-fancy"))
        .stdout(predicate::str::contains("A fancy theme"));
}

#[test]
fn test_theme_apply_outside_project_fails() {
    let tmp = TempDir::new().unwrap();

    page_cmd()
        .args(["theme", "apply", "dark"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_theme_export_without_base_html_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thnobase", "No Base", "posts");
    let site_dir = tmp.path().join("thnobase");

    // Remove base.html
    fs::remove_file(site_dir.join("templates/base.html")).unwrap();

    page_cmd()
        .args(["theme", "export", "my-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no templates/base.html"));
}

#[test]
fn test_theme_export_duplicate_name_fails() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "thdup2", "Dup Theme2", "posts");
    let site_dir = tmp.path().join("thdup2");

    // Export once
    page_cmd()
        .args(["theme", "export", "dup-theme"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Export again with same name should fail
    page_cmd()
        .args(["theme", "export", "dup-theme"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_theme_apply_then_build_with_all_themes() {
    // Test that every bundled theme produces a valid build
    let themes = [
        "default",
        "minimal",
        "dark",
        "docs",
        "brutalist",
        "bento",
        "landing",
        "terminal",
        "magazine",
        "academic",
    ];
    for theme_name in &themes {
        let tmp = TempDir::new().unwrap();
        init_site(&tmp, "thall", "All Themes Build", "posts,docs,pages");
        let site_dir = tmp.path().join("thall");

        page_cmd()
            .args(["theme", "apply", theme_name])
            .current_dir(&site_dir)
            .assert()
            .success();

        page_cmd()
            .args(["build"])
            .current_dir(&site_dir)
            .assert()
            .success();

        assert!(
            site_dir.join("dist/index.html").exists(),
            "build with {} theme should produce index.html",
            theme_name
        );
    }
}
