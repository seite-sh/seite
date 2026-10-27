//! `[build.tailwind]` with a fake Tailwind CLI named by `SEITE_TAILWIND_BIN`
//! (never the network, never a program chosen by seite.toml). Every command
//! runs with a private HOME so the per-user trust list is the test's own.
use super::common::*;

const BIN_ENV: &str = "SEITE_TAILWIND_BIN";

/// A stand-in for the Tailwind CLI: writes CSS (plus the `--cwd` it got) to
/// the `-o` path, and drops a marker file in that `--cwd` (the site root).
#[cfg(unix)]
const FAKE_OK: &str = r#"#!/bin/sh
out=""
root=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    --cwd) root="$2"; shift 2 ;;
    *) shift ;;
  esac
done
touch "$root/tailwind-ran"
printf '/*! fake tailwind */.from-fake{color:red}/* root=%s */' "$(basename "$root")" > "$out"
"#;

#[cfg(unix)]
const FAKE_FAIL: &str = r#"#!/bin/sh
while [ $# -gt 0 ]; do
  case "$1" in
    --cwd) touch "$2/tailwind-ran"; shift 2 ;;
    *) shift ;;
  esac
done
echo 'Error: Cannot apply unknown utility class `nope`' >&2
exit 1
"#;

/// Write the fake CLI outside the site; returns its absolute path.
#[cfg(unix)]
fn write_fake(tmp: &TempDir, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = tmp.path().join("fake-tailwind.sh");
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// `seite` with a private HOME (trust list, caches) under `tmp`.
fn cmd(tmp: &TempDir) -> Command {
    let mut cmd = page_cmd();
    cmd.env("HOME", tmp.path().join("home"))
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_CACHE_HOME")
        .env_remove("SEITE_TRUST_PROJECT");
    cmd
}

/// `seite theme new --tailwind` (which trusts the project) in a new site;
/// returns (site dir, fake CLI).
#[cfg(unix)]
fn tailwind_site(tmp: &TempDir, fake: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    init_site(tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    cmd(tmp)
        .args(["theme", "new", "--tailwind"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(toml.contains("[build.tailwind]"), "{toml}");
    // Not a terminal: `theme new` never trusts on its own.
    assert!(!trust_file(tmp).exists(), "theme new trusted without a TTY");
    cmd(tmp)
        .arg("trust")
        .current_dir(&site_dir)
        .assert()
        .success();
    (site_dir, write_fake(tmp, fake))
}

/// The private HOME's trust list (macOS or XDG layout).
fn trust_file(tmp: &TempDir) -> std::path::PathBuf {
    let home = tmp.path().join("home");
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/seite/trusted-projects.json")
    } else {
        home.join(".config/seite/trusted-projects.json")
    }
}

#[cfg(unix)]
#[test]
fn test_theme_new_tailwind_never_trusts_in_agent_session_or_without_tty() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    cmd(&tmp)
        .args(["theme", "new", "--tailwind"])
        .env("CLAUDECODE", "1")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("run `seite trust` yourself"));
    assert!(!trust_file(&tmp).exists());
    // The generated GitHub Pages workflow gets the CI opt-in.
    let workflow = fs::read_to_string(site_dir.join(".github/workflows/deploy.yml")).unwrap();
    assert!(
        workflow.contains(
            "        run: seite build\n        env:\n          SEITE_TRUST_PROJECT: \"1\"\n"
        ),
        "{workflow}"
    );
}

#[cfg(unix)]
#[test]
fn test_trust_project_env_is_the_ci_escape_hatch() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    cmd(&tmp)
        .args(["theme", "new", "--tailwind"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let fake = write_fake(&tmp, FAKE_OK);
    let (ok, json) = run_json_with_bin(&tmp, &site_dir, &fake, &["build"]);
    assert!(!ok, "{json}");
    let output = cmd(&tmp)
        .args(["build"])
        .env(BIN_ENV, &fake)
        .env("SEITE_TRUST_PROJECT", "1")
        .current_dir(&site_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(site_dir.join("tailwind-ran").exists());
}

/// `seite --json <args>` with `SEITE_TAILWIND_BIN=bin` and the private HOME.
fn run_json_with_bin(
    tmp: &TempDir,
    dir: &std::path::Path,
    bin: &std::path::Path,
    args: &[&str],
) -> (bool, serde_json::Value) {
    let output = cmd(tmp)
        .arg("--json")
        .args(args)
        .env(BIN_ENV, bin)
        .current_dir(dir)
        .output()
        .unwrap();
    let doc = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}):\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.success(), doc)
}

#[cfg(unix)]
#[test]
fn test_build_tailwind_compiles_with_env_binary() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, fake) = tailwind_site(&tmp, FAKE_OK);
    let input = fs::read_to_string(site_dir.join("static/css/tailwind.css")).unwrap();
    assert!(
        input.contains("@import \"tailwindcss\" source(none);"),
        "{input}"
    );
    assert!(input.contains("@source \"../../templates\";"), "{input}");
    set_build_option(&site_dir, "fingerprint", "true");

    cmd(&tmp)
        .arg("build")
        .env(BIN_ENV, &fake)
        .current_dir(&site_dir)
        .assert()
        .success();

    // Compiled (not the raw input), for the site root, through the same
    // fingerprint/manifest path as other CSS.
    let css = fs::read_to_string(site_dir.join("dist/static/css/tailwind.css")).unwrap();
    assert_eq!(
        css,
        "/*! fake tailwind */.from-fake{color:red}/* root=site */"
    );
    let manifest = read_json_file(&site_dir.join("dist/asset-manifest.json"));
    let fingerprinted = manifest["/static/css/tailwind.css"]
        .as_str()
        .expect("tailwind.css in the asset manifest")
        .to_string();
    assert_ne!(fingerprinted, "/static/css/tailwind.css");
    let fp_file = site_dir
        .join("dist")
        .join(fingerprinted.trim_start_matches('/'));
    assert_eq!(fs::read_to_string(fp_file).unwrap(), css);

    // base.html links the compiled stylesheet, and `seite check` (which
    // never runs Tailwind) is clean.
    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        html.contains("href=\"/static/css/tailwind.css\""),
        "base.html should link the compiled CSS"
    );
    fs::remove_file(site_dir.join("tailwind-ran")).unwrap();
    let (ok, json) = run_json_with_bin(&tmp, &site_dir, &fake, &["check"]);
    assert!(ok, "{json}");
    assert!(
        !site_dir.join("tailwind-ran").exists(),
        "check ran Tailwind"
    );
}

#[cfg(unix)]
#[test]
fn test_build_tailwind_input_is_not_copied_raw() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, fake) = tailwind_site(&tmp, FAKE_OK);
    let toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    fs::write(
        site_dir.join("seite.toml"),
        toml.replace("output = \"css/tailwind.css\"", "output = \"css/app.css\""),
    )
    .unwrap();
    cmd(&tmp)
        .arg("build")
        .env(BIN_ENV, &fake)
        .current_dir(&site_dir)
        .assert()
        .success();
    assert!(!site_dir.join("dist/static/css/tailwind.css").exists());
    let css = fs::read_to_string(site_dir.join("dist/static/css/app.css")).unwrap();
    assert!(css.starts_with("/*! fake tailwind */"), "{css}");
}

#[cfg(unix)]
#[test]
fn test_build_tailwind_failure_is_a_diagnostic() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, fake) = tailwind_site(&tmp, FAKE_FAIL);
    let (ok, json) = run_json_with_bin(&tmp, &site_dir, &fake, &["build"]);
    assert!(!ok, "{json}");
    let text = json.to_string();
    assert!(text.contains("tailwind-failed"), "{text}");
    assert!(
        text.contains("Cannot apply unknown utility class"),
        "{text}"
    );
    assert!(text.contains("static/css/tailwind.css"), "{text}");
    assert!(!site_dir.join("dist/static/css/tailwind.css").exists());
}

#[cfg(unix)]
#[test]
fn test_untrusted_project_needs_seite_trust() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, fake) = tailwind_site(&tmp, FAKE_OK);
    // A fresh clone elsewhere: same files, not trusted.
    let other = TempDir::new().unwrap();
    let (ok, json) = run_json_with_bin(&other, &site_dir, &fake, &["build"]);
    assert!(!ok, "{json}");
    let text = json.to_string();
    assert!(text.contains("tailwind-untrusted"), "{text}");
    assert!(text.contains("seite trust"), "{text}");
    assert!(!site_dir.join("tailwind-ran").exists());

    let (ok, json) = run_json_with_bin(&other, &site_dir, &fake, &["trust"]);
    assert!(ok, "{json}");
    assert_eq!(json["data"]["trusted"], true);
    let (ok, json) = run_json_with_bin(&other, &site_dir, &fake, &["trust", "--list"]);
    assert!(ok, "{json}");
    assert_eq!(json["data"]["projects"].as_array().unwrap().len(), 1);
    let (ok, json) = run_json_with_bin(&other, &site_dir, &fake, &["build"]);
    assert!(ok, "{json}");
    assert!(site_dir.join("tailwind-ran").exists());

    let (ok, _) = run_json_with_bin(&other, &site_dir, &fake, &["trust", "--revoke"]);
    assert!(ok);
    let (ok, json) = run_json_with_bin(&other, &site_dir, &fake, &["build"]);
    assert!(!ok, "{json}");
}

#[cfg(unix)]
#[test]
fn test_check_hook_never_runs_tailwind() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, fake) = tailwind_site(&tmp, FAKE_FAIL);
    let output = cmd(&tmp)
        .args(["check", "--hook", "claude"])
        .env(BIN_ENV, &fake)
        .env_remove("CLAUDE_PROJECT_DIR")
        .env_remove("CURSOR_PROJECT_DIR")
        .current_dir(&site_dir)
        .write_stdin(r#"{"hook_event_name":"Stop","stop_hook_active":false}"#)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("tailwind"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !site_dir.join("tailwind-ran").exists(),
        "the hook must not run the Tailwind CLI"
    );
    // Neither does `seite check` or `seite audit`.
    let (ok, json) = run_json_with_bin(&tmp, &site_dir, &fake, &["check"]);
    assert!(ok, "{json}");
    run_json_with_bin(&tmp, &site_dir, &fake, &["audit"]);
    assert!(!site_dir.join("tailwind-ran").exists());
}

#[cfg(unix)]
#[test]
fn test_env_binary_must_be_absolute() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, _fake) = tailwind_site(&tmp, FAKE_OK);
    let (ok, json) = run_json_with_bin(
        &tmp,
        &site_dir,
        std::path::Path::new("fake-tailwind.sh"),
        &["build"],
    );
    assert!(!ok, "{json}");
    assert!(json.to_string().contains("absolute"), "{json}");
}

#[test]
fn test_tailwind_binary_in_seite_toml_is_refused() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    let base = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    for table in [
        "[build.tailwind]\nbinary = \"./evil.sh\"\n",
        "[build.tailwind]\nversion = \"4.0.0\"\n",
    ] {
        fs::write(site_dir.join("seite.toml"), format!("{base}\n{table}")).unwrap();
        let (ok, json) = run_json(&site_dir, &["check"]);
        assert!(!ok, "{json}");
        let text = json.to_string();
        assert!(text.contains("config-invalid"), "{text}");
        assert!(text.contains(BIN_ENV), "{text}");
    }
}

#[test]
fn test_agent_permissions_never_allow_seite_trust() {
    // Generated permissions (every agent, MCP on and off, upgrades) never
    // let an agent run `seite trust` on its own.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/harness");
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        for pattern in [
            "Bash(seite trust",
            "Bash(seite:*",
            "Shell(seite:trust",
            "Shell(seite:*",
            "\"seite trust",
            "\"seite *",
            "seite trust*",
        ] {
            assert!(
                !text.contains(pattern),
                "{} allows `{pattern}`",
                path.display()
            );
        }
    }
}

#[test]
fn test_build_without_tailwind_copies_css_raw() {
    // No [build.tailwind]: a file named tailwind.css is an ordinary asset.
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Plain", "posts");
    let site_dir = tmp.path().join("site");
    set_build_option(&site_dir, "minify", "false");
    write_static_css(&site_dir, "tailwind.css", "@import \"tailwindcss\";\n");
    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(site_dir.join("dist/static/tailwind.css")).unwrap(),
        "@import \"tailwindcss\";\n"
    );
}
