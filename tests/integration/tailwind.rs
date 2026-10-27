//! `[build.tailwind]` with a fake Tailwind CLI named by `SEITE_TAILWIND_BIN`
//! (never the network, never a program chosen by seite.toml).
use super::common::*;

const BIN_ENV: &str = "SEITE_TAILWIND_BIN";

/// A stand-in for the Tailwind CLI: writes CSS (plus the cwd it ran in) to
/// the `-o` path, and drops a marker file in the cwd (the site root).
#[cfg(unix)]
const FAKE_OK: &str = r#"#!/bin/sh
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
touch tailwind-ran
printf '/*! fake tailwind */.from-fake{color:red}/* cwd=%s */' "$(basename "$(pwd)")" > "$out"
"#;

#[cfg(unix)]
const FAKE_FAIL: &str = "#!/bin/sh\ntouch tailwind-ran\necho 'Error: Cannot apply unknown utility class `nope`' >&2\nexit 1\n";

/// Write the fake CLI outside the site; returns its path.
#[cfg(unix)]
fn write_fake(tmp: &TempDir, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = tmp.path().join("fake-tailwind.sh");
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// `seite theme new --tailwind` in a new site; returns (site dir, fake CLI).
#[cfg(unix)]
fn tailwind_site(tmp: &TempDir, fake: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    init_site(tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "new", "--tailwind"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(toml.contains("[build.tailwind]"), "{toml}");
    (site_dir, write_fake(tmp, fake))
}

/// `seite --json <args>` with `SEITE_TAILWIND_BIN=bin`.
fn run_json_with_bin(
    dir: &std::path::Path,
    bin: &std::path::Path,
    args: &[&str],
) -> (bool, serde_json::Value) {
    let output = page_cmd()
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

    page_cmd()
        .arg("build")
        .env(BIN_ENV, &fake)
        .current_dir(&site_dir)
        .assert()
        .success();

    // Compiled (not the raw input), from the site root, through the same
    // fingerprint/manifest path as other CSS.
    let css = fs::read_to_string(site_dir.join("dist/static/css/tailwind.css")).unwrap();
    assert_eq!(
        css,
        "/*! fake tailwind */.from-fake{color:red}/* cwd=site */"
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

    // base.html links the compiled stylesheet, and the link resolves.
    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        html.contains("href=\"/static/css/tailwind.css\""),
        "base.html should link the compiled CSS"
    );
    let (ok, json) = run_json_with_bin(&site_dir, &fake, &["check"]);
    assert!(ok, "{json}");
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
    page_cmd()
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
    let (ok, json) = run_json_with_bin(&site_dir, &fake, &["build"]);
    assert!(!ok, "{json}");
    let text = json.to_string();
    assert!(text.contains("tailwind-failed"), "{text}");
    assert!(
        text.contains("Cannot apply unknown utility class"),
        "{text}"
    );
    assert!(text.contains("static/css/tailwind.css"), "{text}");
    assert!(!site_dir.join("dist/static/css/tailwind.css").exists());

    // Plain `seite check` runs it (it validates the CSS).
    let (ok, json) = run_json_with_bin(&site_dir, &fake, &["check"]);
    assert!(!ok, "{json}");
    assert!(json.to_string().contains("tailwind-failed"), "{json}");
}

#[cfg(unix)]
#[test]
fn test_check_hook_never_runs_tailwind() {
    let tmp = TempDir::new().unwrap();
    let (site_dir, fake) = tailwind_site(&tmp, FAKE_FAIL);
    let output = page_cmd()
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
