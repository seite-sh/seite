//! `[build.tailwind]` with a fake `binary` (never the network).
use super::common::*;

/// A stand-in for the Tailwind CLI: writes CSS (plus the cwd it ran in) to
/// the `-o` path.
#[cfg(unix)]
const FAKE_OK: &str = r#"#!/bin/sh
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '/*! fake tailwind */.from-fake{color:red}/* cwd=%s */' "$(basename "$(pwd)")" > "$out"
"#;

#[cfg(unix)]
const FAKE_FAIL: &str =
    "#!/bin/sh\necho 'Error: Cannot apply unknown utility class `nope`' >&2\nexit 1\n";

#[cfg(unix)]
fn write_fake(site_dir: &std::path::Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = site_dir.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// `seite theme new --tailwind`, then point `binary` at `fake`.
#[cfg(unix)]
fn tailwind_site(tmp: &TempDir, fake: &str) -> std::path::PathBuf {
    init_site(tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    page_cmd()
        .args(["theme", "new", "--tailwind"])
        .current_dir(&site_dir)
        .assert()
        .success();
    let toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    assert!(toml.contains("[build.tailwind]"), "{toml}");
    let toml = toml.replace(
        "output = \"css/tailwind.css\"\n",
        "output = \"css/tailwind.css\"\nbinary = \"fake-tailwind.sh\"\n",
    );
    fs::write(site_dir.join("seite.toml"), toml).unwrap();
    write_fake(&site_dir, "fake-tailwind.sh", fake);
    site_dir
}

#[cfg(unix)]
#[test]
fn test_build_tailwind_compiles_with_binary() {
    let tmp = TempDir::new().unwrap();
    let site_dir = tailwind_site(&tmp, FAKE_OK);
    let input = fs::read_to_string(site_dir.join("static/css/tailwind.css")).unwrap();
    assert!(
        input.contains("@import \"tailwindcss\" source(none);"),
        "{input}"
    );
    assert!(input.contains("@source \"../../templates\";"), "{input}");
    set_build_option(&site_dir, "fingerprint", "true");

    page_cmd()
        .arg("build")
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
    page_cmd()
        .arg("check")
        .current_dir(&site_dir)
        .assert()
        .success();
}

#[cfg(unix)]
#[test]
fn test_build_tailwind_failure_is_a_diagnostic() {
    let tmp = TempDir::new().unwrap();
    let site_dir = tailwind_site(&tmp, FAKE_FAIL);
    let (ok, json) = run_json(&site_dir, &["build"]);
    assert!(!ok, "{json}");
    let text = json.to_string();
    assert!(text.contains("tailwind-failed"), "{text}");
    assert!(
        text.contains("Cannot apply unknown utility class"),
        "{text}"
    );
    assert!(text.contains("static/css/tailwind.css"), "{text}");
    // The previous output is untouched (no dist/ from a failed first build).
    assert!(!site_dir.join("dist/static/css/tailwind.css").exists());

    let (ok, json) = run_json(&site_dir, &["check"]);
    assert!(!ok, "{json}");
    assert!(json.to_string().contains("tailwind-failed"), "{json}");
}

#[test]
fn test_build_tailwind_other_version_needs_binary() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Tailwind", "posts");
    let site_dir = tmp.path().join("site");
    let mut toml = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    toml.push_str("\n[build.tailwind]\nversion = \"4.0.0\"\n");
    fs::write(site_dir.join("seite.toml"), toml).unwrap();
    let (ok, json) = run_json(&site_dir, &["check"]);
    assert!(!ok, "{json}");
    let text = json.to_string();
    assert!(text.contains("config-invalid"), "{text}");
    assert!(text.contains("binary"), "{text}");
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
