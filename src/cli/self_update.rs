//! `seite self-update` — update the seite binary to the latest release.
//!
//! Fetches the latest release version from seite.sh (falling back to the
//! GitHub API), downloads the binary through seite.sh/download/ (which
//! 302-redirects to GitHub Releases), verifies the checksum, and replaces
//! the running binary.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::Args;

use crate::output::human;

const REPO: &str = "seite-sh/seite";
const DOWNLOAD_BASE: &str = "https://seite.sh/download";

#[derive(Args)]
pub struct SelfUpdateArgs {
    /// Update to a specific version (e.g., "0.2.0" or "v0.2.0")
    #[arg(long = "target-version")]
    pub target_version: Option<String>,

    /// Just check for updates without installing
    #[arg(long)]
    pub check: bool,
}

pub fn run(args: &SelfUpdateArgs) -> anyhow::Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");

    // 1. Resolve the target version
    let target_tag = match &args.target_version {
        Some(v) => {
            let tag = if v.starts_with('v') {
                v.clone()
            } else {
                format!("v{v}")
            };
            human::info(&format!("Targeting version {tag}..."));
            tag
        }
        None => {
            human::info("Checking for updates...");
            fetch_latest_tag()?
        }
    };

    let target_version = target_tag.trim_start_matches('v');

    // 2. Compare versions
    if target_version == current_version {
        human::success(&format!("Already up to date (seite {current_version})."));
        return Ok(());
    }

    // Show what we'd do
    let is_upgrade = version_cmp(target_version, current_version) == std::cmp::Ordering::Greater;
    let direction = if is_upgrade { "Upgrade" } else { "Downgrade" };
    human::info(&format!(
        "{direction}: {current_version} → {target_version}"
    ));

    if args.check {
        if is_upgrade {
            human::info(&format!(
                "Run `seite self-update` to install seite {target_version}."
            ));
            std::process::exit(1); // exit 1 = update available (useful for CI)
        }
        return Ok(());
    }

    // 3. Detect platform
    let target_triple = detect_target_triple()?;
    let archive_name = format!("seite-{target_triple}.tar.gz");
    let download_url = format!("{DOWNLOAD_BASE}/{target_tag}/{archive_name}");
    let checksums_url = format!("{DOWNLOAD_BASE}/{target_tag}/checksums-sha256.txt");

    // 4. Download to a temp directory
    human::info(&format!("Downloading {archive_name}..."));

    let tmp_dir = tempfile::TempDir::new()?;
    let archive_path = tmp_dir.path().join(&archive_name);
    let checksums_path = tmp_dir.path().join("checksums-sha256.txt");

    download_file(&download_url, &archive_path)?;
    download_file(&checksums_url, &checksums_path)?;

    // 5. Verify checksum
    human::info("Verifying checksum...");
    verify_checksum(&archive_path, &checksums_path, &archive_name)?;
    human::success("Checksum verified.");

    // 6. Extract binary from tar.gz
    let binary_path = extract_binary(&archive_path, tmp_dir.path())?;

    // 7. Replace the running binary
    let current_exe = env::current_exe()?;
    replace_binary(&binary_path, &current_exe)?;

    human::success(&format!(
        "Updated seite {current_version} → {target_version}"
    ));
    human::info("Run `seite upgrade` in your projects to update their config files.");

    Ok(())
}

/// Fetch the latest release tag, trying seite.sh first then GitHub API.
fn fetch_latest_tag() -> anyhow::Result<String> {
    // Try seite.sh/version.txt first (fast, no API rate limits)
    if let Ok(mut response) = ureq::get("https://seite.sh/version.txt")
        .header("User-Agent", "seite-self-update")
        .call()
    {
        if let Ok(body) = response.body_mut().read_to_string() {
            let tag = body.trim().to_string();
            if !tag.is_empty() {
                return Ok(tag);
            }
        }
    }

    // Fallback to GitHub API
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let mut response = ureq::get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "seite-self-update")
        .call()
        .map_err(|e| anyhow::anyhow!("Failed to check for updates: {e}"))?;

    let body: serde_json::Value = response.body_mut().read_json()?;
    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Could not parse latest release tag from GitHub"))?;

    Ok(tag.to_string())
}

/// Detect the target triple for the current platform.
fn detect_target_triple() -> anyhow::Result<String> {
    let os = if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else if cfg!(target_os = "windows") {
        anyhow::bail!(
            "Self-update is not supported on Windows. Use the PowerShell installer:\n  \
             irm https://seite.sh/install.ps1 | iex"
        );
    } else {
        anyhow::bail!("Unsupported operating system. Install from source: cargo install seite");
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        anyhow::bail!("Unsupported architecture. Install from source: cargo install seite");
    };

    Ok(format!("{arch}-{os}"))
}

/// Download a URL to a local file using ureq.
fn download_file(url: &str, dest: &PathBuf) -> anyhow::Result<()> {
    let response = ureq::get(url)
        .header("User-Agent", "seite-self-update")
        .call()
        .map_err(|e| anyhow::anyhow!("Download failed ({url}): {e}"))?;

    let mut reader = response.into_body().into_reader();
    let mut file = fs::File::create(dest)?;
    std::io::copy(&mut reader, &mut file)?;
    file.flush()?;
    Ok(())
}

/// Verify the SHA256 checksum of a downloaded archive.
fn verify_checksum(
    archive: &PathBuf,
    checksums_file: &PathBuf,
    archive_name: &str,
) -> anyhow::Result<()> {
    use std::io::Read;

    // Compute SHA256 of the archive
    let mut file = fs::File::open(archive)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hasher.hex_digest();

    // Find expected checksum in checksums file
    let checksums = fs::read_to_string(checksums_file)?;
    let expected = checksums
        .lines()
        .find(|line| line.ends_with(archive_name) || line.contains(archive_name))
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| anyhow::anyhow!("Archive {archive_name} not found in checksums file"))?;

    if actual != expected {
        anyhow::bail!("Checksum mismatch!\n  Expected: {expected}\n  Actual:   {actual}");
    }

    Ok(())
}

/// Minimal SHA256 implementation (we already have the primitives via image crate deps,
/// but to avoid adding a heavy dependency, use a simple pure-Rust implementation).
///
/// In production, you might replace this with the `sha2` crate.
struct Sha256 {
    data: Vec<u8>,
}

impl Sha256 {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn update(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    /// Compute SHA256 and return hex string.
    /// Uses the system `sha256sum` or `shasum` command (same approach as install.sh).
    fn hex_digest(self) -> String {
        // Write data to a temp file and use system sha256 tool
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file for checksum");
        fs::write(tmp.path(), &self.data).expect("failed to write temp file for checksum");

        // Try sha256sum first, then shasum -a 256
        let output = std::process::Command::new("sha256sum")
            .arg(tmp.path())
            .output()
            .or_else(|_| {
                std::process::Command::new("shasum")
                    .args(["-a", "256"])
                    .arg(tmp.path())
                    .output()
            });

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.split_whitespace().next().unwrap_or("").to_string()
            }
            _ => String::new(),
        }
    }
}

/// Extract the `seite` binary from a tar.gz archive.
fn extract_binary(archive: &PathBuf, dest_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let status = std::process::Command::new("tar")
        .args(["xzf"])
        .arg(archive)
        .arg("-C")
        .arg(dest_dir)
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to extract archive");
    }

    let binary = dest_dir.join("seite");
    if !binary.exists() {
        anyhow::bail!("Binary 'seite' not found in archive");
    }

    Ok(binary)
}

/// Replace the currently running binary with the new one.
///
/// On Unix, this does an atomic rename. The old binary is moved to a backup
/// location first, and if anything fails, we try to restore it.
fn replace_binary(new_binary: &PathBuf, current_exe: &PathBuf) -> anyhow::Result<()> {
    // Resolve symlinks to get the real path
    let real_path = fs::canonicalize(current_exe)?;
    let backup_path = real_path.with_extension("old");

    // Move current → backup
    if let Err(e) = fs::rename(&real_path, &backup_path) {
        anyhow::bail!(
            "Cannot replace binary at {}: {e}\n\
             You may need to run with elevated permissions or update manually.",
            real_path.display()
        );
    }

    // Move new → target
    if let Err(e) = fs::copy(new_binary, &real_path) {
        // Try to restore backup
        let _ = fs::rename(&backup_path, &real_path);
        anyhow::bail!("Failed to install new binary: {e}");
    }

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&real_path, fs::Permissions::from_mode(0o755))?;
    }

    // Clean up backup
    let _ = fs::remove_file(&backup_path);

    Ok(())
}

/// Simple semver comparison for version strings.
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> (u64, u64, u64) {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() >= 3 {
            (
                parts[0].parse().unwrap_or(0),
                parts[1].parse().unwrap_or(0),
                parts[2].parse().unwrap_or(0),
            )
        } else {
            (0, 0, 0)
        }
    };
    parse(a).cmp(&parse(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    // ── version_cmp tests ──────────────────────────────────────────────

    #[test]
    fn test_version_cmp_equal() {
        assert_eq!(version_cmp("0.2.3", "0.2.3"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_newer_patch() {
        assert_eq!(version_cmp("0.2.4", "0.2.3"), Ordering::Greater);
    }

    #[test]
    fn test_version_cmp_older_patch() {
        assert_eq!(version_cmp("0.2.2", "0.2.3"), Ordering::Less);
    }

    #[test]
    fn test_version_cmp_newer_minor() {
        assert_eq!(version_cmp("0.3.0", "0.2.9"), Ordering::Greater);
    }

    #[test]
    fn test_version_cmp_newer_major() {
        assert_eq!(version_cmp("1.0.0", "0.99.99"), Ordering::Greater);
    }

    #[test]
    fn test_version_cmp_malformed() {
        // Fewer than 3 parts → treated as (0, 0, 0)
        assert_eq!(version_cmp("1.0", "0.0.0"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_two_parts() {
        // Two-part versions like "1.0" are treated as (0, 0, 0) by the parser
        // since it requires >= 3 parts; both map to (0,0,0) so they are Equal
        assert_eq!(version_cmp("1.0", "1.0"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_one_part() {
        // Single-part versions like "5" are treated as (0, 0, 0)
        assert_eq!(version_cmp("5", "5"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_empty_strings() {
        // Empty strings: split produces [""], which has len() == 1 < 3, so (0,0,0)
        assert_eq!(version_cmp("", ""), Ordering::Equal);
        assert_eq!(version_cmp("", "0.0.0"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_non_numeric_parts() {
        // Non-numeric segments parse as 0 via unwrap_or(0)
        assert_eq!(version_cmp("a.b.c", "0.0.0"), Ordering::Equal);
        assert_eq!(version_cmp("1.beta.3", "1.0.3"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_extra_parts_ignored() {
        // Only the first 3 parts are considered; extra parts are ignored
        assert_eq!(version_cmp("1.2.3.4", "1.2.3"), Ordering::Equal);
        assert_eq!(version_cmp("1.2.3.99", "1.2.3.0"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_leading_zeros() {
        // "01" parses to 1 via u64::parse, so "01.02.03" == "1.2.3"
        assert_eq!(version_cmp("01.02.03", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn test_version_cmp_large_numbers() {
        assert_eq!(version_cmp("999.999.999", "999.999.998"), Ordering::Greater);
        assert_eq!(version_cmp("0.0.1", "0.0.0"), Ordering::Greater);
    }

    #[test]
    fn test_version_cmp_zeros() {
        assert_eq!(version_cmp("0.0.0", "0.0.0"), Ordering::Equal);
        assert_eq!(version_cmp("0.0.1", "0.0.0"), Ordering::Greater);
        assert_eq!(version_cmp("0.0.0", "0.0.1"), Ordering::Less);
    }

    #[test]
    fn test_version_cmp_major_dominates() {
        // Major version always wins regardless of minor/patch
        assert_eq!(version_cmp("2.0.0", "1.99.99"), Ordering::Greater);
        assert_eq!(version_cmp("1.0.0", "0.999.999"), Ordering::Greater);
    }

    #[test]
    fn test_version_cmp_minor_dominates_patch() {
        // Minor wins over patch within same major
        assert_eq!(version_cmp("0.2.0", "0.1.999"), Ordering::Greater);
    }

    // ── detect_target_triple tests ─────────────────────────────────────

    #[test]
    fn test_detect_target_triple() {
        let triple = detect_target_triple().unwrap();
        #[cfg(target_os = "macos")]
        assert!(triple.contains("apple-darwin"));
        #[cfg(target_os = "linux")]
        assert!(triple.contains("unknown-linux-gnu"));
        #[cfg(target_arch = "x86_64")]
        assert!(triple.starts_with("x86_64"));
        #[cfg(target_arch = "aarch64")]
        assert!(triple.starts_with("aarch64"));
    }

    #[test]
    fn test_detect_target_triple_format() {
        // The triple must be exactly "{arch}-{os}" with a single hyphen separating them
        let triple = detect_target_triple().unwrap();
        // Must contain a hyphen
        assert!(
            triple.contains('-'),
            "triple should contain a hyphen: {triple}"
        );
        // First segment should be the architecture
        let arch_part = triple.split('-').next().unwrap();
        assert!(
            arch_part == "x86_64" || arch_part == "aarch64",
            "arch should be x86_64 or aarch64, got: {arch_part}"
        );
        // Should end with the OS suffix
        #[cfg(target_os = "macos")]
        assert!(
            triple.ends_with("apple-darwin"),
            "on macOS, triple should end with apple-darwin: {triple}"
        );
        #[cfg(target_os = "linux")]
        assert!(
            triple.ends_with("unknown-linux-gnu"),
            "on Linux, triple should end with unknown-linux-gnu: {triple}"
        );
    }

    #[test]
    fn test_detect_target_triple_deterministic() {
        // Calling twice should return the same result
        let t1 = detect_target_triple().unwrap();
        let t2 = detect_target_triple().unwrap();
        assert_eq!(t1, t2, "detect_target_triple should be deterministic");
    }

    // ── URL and archive name construction tests ────────────────────────

    #[test]
    fn test_archive_name_construction() {
        let triple = detect_target_triple().unwrap();
        let archive_name = format!("seite-{triple}.tar.gz");
        assert!(archive_name.starts_with("seite-"));
        assert!(archive_name.ends_with(".tar.gz"));
        // Should not have double hyphens
        assert!(
            !archive_name.contains("--"),
            "archive name should not have double hyphens: {archive_name}"
        );
    }

    #[test]
    fn test_download_url_construction() {
        let target_tag = "v0.2.3";
        let archive_name = "seite-x86_64-apple-darwin.tar.gz";
        let download_url = format!("{DOWNLOAD_BASE}/{target_tag}/{archive_name}");
        assert_eq!(
            download_url,
            "https://seite.sh/download/v0.2.3/seite-x86_64-apple-darwin.tar.gz"
        );
    }

    #[test]
    fn test_checksums_url_construction() {
        let target_tag = "v1.0.0";
        let checksums_url = format!("{DOWNLOAD_BASE}/{target_tag}/checksums-sha256.txt");
        assert_eq!(
            checksums_url,
            "https://seite.sh/download/v1.0.0/checksums-sha256.txt"
        );
    }

    #[test]
    fn test_url_construction_various_tags() {
        for tag in &["v0.1.0", "v0.2.3", "v1.0.0", "v10.20.30"] {
            let archive = "seite-aarch64-apple-darwin.tar.gz";
            let url = format!("{DOWNLOAD_BASE}/{tag}/{archive}");
            assert!(url.starts_with("https://seite.sh/download/v"));
            assert!(url.ends_with(".tar.gz"));
            assert!(url.contains(tag));
        }
    }

    // ── Version tag normalization tests ────────────────────────────────

    #[test]
    fn test_tag_normalization_with_v_prefix() {
        let input = "v0.2.3";
        let tag = if input.starts_with('v') {
            input.to_string()
        } else {
            format!("v{input}")
        };
        assert_eq!(tag, "v0.2.3");
    }

    #[test]
    fn test_tag_normalization_without_v_prefix() {
        let input = "0.2.3";
        let tag = if input.starts_with('v') {
            input.to_string()
        } else {
            format!("v{input}")
        };
        assert_eq!(tag, "v0.2.3");
    }

    #[test]
    fn test_tag_trim_v_prefix() {
        let tag = "v0.2.3";
        let version = tag.trim_start_matches('v');
        assert_eq!(version, "0.2.3");
    }

    #[test]
    fn test_tag_trim_no_v_prefix() {
        let tag = "0.2.3";
        let version = tag.trim_start_matches('v');
        assert_eq!(version, "0.2.3");
    }

    #[test]
    fn test_tag_trim_multiple_v_prefix() {
        // trim_start_matches strips all leading 'v' characters
        let tag = "vvv0.2.3";
        let version = tag.trim_start_matches('v');
        assert_eq!(version, "0.2.3");
    }

    // ── Upgrade/downgrade direction logic tests ────────────────────────

    #[test]
    fn test_direction_upgrade() {
        let is_upgrade = version_cmp("0.3.0", "0.2.0") == std::cmp::Ordering::Greater;
        assert!(is_upgrade);
        let direction = if is_upgrade { "Upgrade" } else { "Downgrade" };
        assert_eq!(direction, "Upgrade");
    }

    #[test]
    fn test_direction_downgrade() {
        let is_upgrade = version_cmp("0.1.0", "0.2.0") == std::cmp::Ordering::Greater;
        assert!(!is_upgrade);
        let direction = if is_upgrade { "Upgrade" } else { "Downgrade" };
        assert_eq!(direction, "Downgrade");
    }

    // ── Sha256 tests ───────────────────────────────────────────────────

    #[test]
    fn test_sha256_basic() {
        let mut hasher = Sha256::new();
        hasher.update(b"hello");
        let digest = hasher.hex_digest();
        assert!(!digest.is_empty());
        assert_eq!(digest.len(), 64); // SHA-256 is 64 hex chars
    }

    #[test]
    fn test_sha256_deterministic() {
        let mut h1 = Sha256::new();
        h1.update(b"test data");
        let d1 = h1.hex_digest();

        let mut h2 = Sha256::new();
        h2.update(b"test data");
        let d2 = h2.hex_digest();

        assert_eq!(d1, d2);
    }

    #[test]
    fn test_sha256_empty() {
        // SHA256 of empty input should still produce a valid 64-char hex string
        let hasher = Sha256::new();
        let digest = hasher.hex_digest();
        assert_eq!(
            digest.len(),
            64,
            "SHA256 of empty input should be 64 hex chars, got: {digest}"
        );
        // The well-known SHA256 of empty string is:
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            digest, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "SHA256 of empty input should match the known constant"
        );
    }

    #[test]
    fn test_sha256_incremental() {
        // Hashing "ab" all at once should equal hashing "a" then "b" incrementally
        let mut hasher_once = Sha256::new();
        hasher_once.update(b"ab");
        let digest_once = hasher_once.hex_digest();

        let mut hasher_inc = Sha256::new();
        hasher_inc.update(b"a");
        hasher_inc.update(b"b");
        let digest_inc = hasher_inc.hex_digest();

        assert_eq!(
            digest_once, digest_inc,
            "sha256('ab') should equal sha256('a' + 'b') done incrementally"
        );
    }

    #[test]
    fn test_sha256_known_value_hello() {
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let mut hasher = Sha256::new();
        hasher.update(b"hello");
        let digest = hasher.hex_digest();
        assert_eq!(
            digest, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
            "SHA256 of 'hello' should match the known constant"
        );
    }

    #[test]
    fn test_sha256_known_value_hello_world() {
        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let mut hasher = Sha256::new();
        hasher.update(b"hello world");
        let digest = hasher.hex_digest();
        assert_eq!(
            digest, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
            "SHA256 of 'hello world' should match the known constant"
        );
    }

    #[test]
    fn test_sha256_different_inputs_differ() {
        let mut h1 = Sha256::new();
        h1.update(b"input A");
        let d1 = h1.hex_digest();

        let mut h2 = Sha256::new();
        h2.update(b"input B");
        let d2 = h2.hex_digest();

        assert_ne!(d1, d2, "different inputs should produce different hashes");
    }

    #[test]
    fn test_sha256_multiple_incremental_updates() {
        // Hash "abcdefgh" in 4 updates of 2 bytes each
        let mut hasher_once = Sha256::new();
        hasher_once.update(b"abcdefgh");
        let digest_once = hasher_once.hex_digest();

        let mut hasher_inc = Sha256::new();
        hasher_inc.update(b"ab");
        hasher_inc.update(b"cd");
        hasher_inc.update(b"ef");
        hasher_inc.update(b"gh");
        let digest_inc = hasher_inc.hex_digest();

        assert_eq!(digest_once, digest_inc);
    }

    #[test]
    fn test_sha256_single_byte() {
        // sha256("\n") = 01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b
        let mut hasher = Sha256::new();
        hasher.update(b"\n");
        let digest = hasher.hex_digest();
        assert_eq!(digest.len(), 64);
        assert_eq!(
            digest, "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
            "SHA256 of newline should match the known constant"
        );
    }

    #[test]
    fn test_sha256_binary_data() {
        // Ensure binary data (with null bytes) is handled correctly
        let mut hasher = Sha256::new();
        hasher.update(&[0u8, 1, 2, 3, 255, 254, 253]);
        let digest = hasher.hex_digest();
        assert_eq!(digest.len(), 64);
        // All hex chars
        assert!(
            digest.chars().all(|c| c.is_ascii_hexdigit()),
            "digest should be all hex chars: {digest}"
        );
    }

    #[test]
    fn test_sha256_large_input() {
        // Hash 1 MB of data to verify larger inputs work
        let data = vec![0x42u8; 1024 * 1024]; // 1 MB of 'B'
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let digest = hasher.hex_digest();
        assert_eq!(digest.len(), 64);
        // Verify determinism with same large input
        let mut hasher2 = Sha256::new();
        hasher2.update(&data);
        let digest2 = hasher2.hex_digest();
        assert_eq!(digest, digest2);
    }

    // ── verify_checksum tests ──────────────────────────────────────────

    #[test]
    fn test_verify_checksum_success() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create an archive file with known content
        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"fake archive content for testing").unwrap();

        // Compute its sha256
        let mut hasher = Sha256::new();
        hasher.update(b"fake archive content for testing");
        let hash = hasher.hex_digest();
        assert_eq!(hash.len(), 64, "hash should be 64 hex chars");

        // Write checksums file with correct hash and archive name
        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        let checksums_content = format!(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  other-file.tar.gz\n\
             {}  seite-test.tar.gz\n\
             bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  another-file.tar.gz\n",
            hash
        );
        fs::write(&checksums_path, checksums_content).unwrap();

        // verify_checksum should succeed
        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(result.is_ok(), "expected success but got: {:?}", result);
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create an archive file
        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"actual content").unwrap();

        // Write checksums file with a WRONG hash
        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let checksums_content = format!("{}  seite-test.tar.gz\n", wrong_hash);
        fs::write(&checksums_path, checksums_content).unwrap();

        // verify_checksum should fail with "mismatch"
        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("mismatch"),
            "expected 'mismatch' in error, got: {err_msg}"
        );
    }

    #[test]
    fn test_verify_checksum_missing_archive_name() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create an archive file
        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"some content").unwrap();

        // Write checksums file WITHOUT the archive name we look for
        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        let checksums_content =
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890  other-archive.tar.gz\n";
        fs::write(&checksums_path, checksums_content).unwrap();

        // verify_checksum should fail because archive name is not in the file
        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found in checksums"),
            "expected 'not found in checksums' in error, got: {err_msg}"
        );
    }

    #[test]
    fn test_verify_checksum_empty_checksums_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"content").unwrap();

        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        fs::write(&checksums_path, "").unwrap();

        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found in checksums"),
            "empty checksums file should error: {err_msg}"
        );
    }

    #[test]
    fn test_verify_checksum_single_space_separator() {
        // Some checksum tools use a single space instead of double-space
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"test data for single space").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"test data for single space");
        let hash = hasher.hex_digest();

        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        // Single space between hash and filename
        let content = format!("{} seite-test.tar.gz\n", hash);
        fs::write(&checksums_path, content).unwrap();

        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(
            result.is_ok(),
            "single-space separator should work: {:?}",
            result
        );
    }

    #[test]
    fn test_verify_checksum_archive_not_on_disk() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let archive_path = tmp_dir.path().join("nonexistent.tar.gz");
        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        fs::write(&checksums_path, "abc  nonexistent.tar.gz\n").unwrap();

        let result = verify_checksum(&archive_path, &checksums_path, "nonexistent.tar.gz");
        assert!(
            result.is_err(),
            "should fail when archive file doesn't exist"
        );
    }

    #[test]
    fn test_verify_checksum_checksums_file_not_on_disk() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"content").unwrap();

        let checksums_path = tmp_dir.path().join("nonexistent-checksums.txt");

        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(
            result.is_err(),
            "should fail when checksums file doesn't exist"
        );
    }

    #[test]
    fn test_verify_checksum_with_path_prefix_in_checksums() {
        // Some tools produce "./seite-test.tar.gz" in the checksums file
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"path prefix content").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"path prefix content");
        let hash = hasher.hex_digest();

        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        // The line contains() the archive name even with ./ prefix
        let content = format!("{}  ./seite-test.tar.gz\n", hash);
        fs::write(&checksums_path, content).unwrap();

        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(
            result.is_ok(),
            "should match archive name via contains(): {:?}",
            result
        );
    }

    #[test]
    fn test_verify_checksum_first_match_wins() {
        // If multiple lines contain the archive name, the first one is used
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        fs::write(&archive_path, b"first match content").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"first match content");
        let correct_hash = hasher.hex_digest();

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let checksums_path = tmp_dir.path().join("checksums-sha256.txt");
        // First line has correct hash, second has wrong hash
        let content = format!(
            "{}  seite-test.tar.gz\n{}  seite-test.tar.gz\n",
            correct_hash, wrong_hash
        );
        fs::write(&checksums_path, content).unwrap();

        let result = verify_checksum(&archive_path, &checksums_path, "seite-test.tar.gz");
        assert!(
            result.is_ok(),
            "first matching line should be used: {:?}",
            result
        );
    }

    // ── replace_binary tests ───────────────────────────────────────────

    #[test]
    fn test_replace_binary_basic() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create a fake "current" binary
        let current_path = tmp_dir.path().join("seite");
        fs::write(&current_path, b"old binary content").unwrap();

        // Create a fake "new" binary
        let new_path = tmp_dir.path().join("seite-new");
        fs::write(&new_path, b"new binary content").unwrap();

        // Replace the current binary with the new one
        let result = replace_binary(&new_path, &current_path);
        assert!(result.is_ok(), "replace_binary failed: {:?}", result);

        // Verify the content was replaced
        let content = fs::read_to_string(&current_path).unwrap();
        assert_eq!(
            content, "new binary content",
            "binary content should be the new version"
        );

        // Verify the backup was cleaned up
        let backup_path = current_path.with_extension("old");
        assert!(
            !backup_path.exists(),
            "backup file should be cleaned up after successful replace"
        );
    }

    #[test]
    fn test_replace_binary_preserves_new_binary() {
        // The source binary should still exist after replacement (we use copy, not move)
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let current_path = tmp_dir.path().join("seite");
        fs::write(&current_path, b"old").unwrap();

        let new_path = tmp_dir.path().join("seite-new");
        fs::write(&new_path, b"new").unwrap();

        replace_binary(&new_path, &current_path).unwrap();

        assert!(
            new_path.exists(),
            "source binary should still exist after replace"
        );
    }

    #[test]
    fn test_replace_binary_nonexistent_current() {
        // If the current binary doesn't exist, fs::canonicalize should fail
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let new_path = tmp_dir.path().join("seite-new");
        fs::write(&new_path, b"new binary").unwrap();

        let nonexistent = tmp_dir.path().join("does-not-exist");

        let result = replace_binary(&new_path, &nonexistent);
        assert!(
            result.is_err(),
            "should fail when current binary doesn't exist"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_replace_binary_sets_executable_permission() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = tempfile::TempDir::new().unwrap();

        let current_path = tmp_dir.path().join("seite");
        fs::write(&current_path, b"old").unwrap();

        let new_path = tmp_dir.path().join("seite-new");
        fs::write(&new_path, b"new").unwrap();

        replace_binary(&new_path, &current_path).unwrap();

        let perms = fs::metadata(&current_path).unwrap().permissions();
        let mode = perms.mode() & 0o777;
        assert_eq!(
            mode, 0o755,
            "replaced binary should have 0755 permissions, got: {mode:o}"
        );
    }

    // ── extract_binary tests ───────────────────────────────────────────

    #[test]
    fn test_extract_binary_valid_archive() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create a fake "seite" binary to tar up
        let staging_dir = tmp_dir.path().join("staging");
        fs::create_dir(&staging_dir).unwrap();
        let binary_path = staging_dir.join("seite");
        fs::write(&binary_path, b"fake seite binary").unwrap();

        // Create tar.gz archive containing "seite"
        let archive_path = tmp_dir.path().join("seite-test.tar.gz");
        let status = std::process::Command::new("tar")
            .args(["czf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(&staging_dir)
            .arg("seite")
            .status()
            .unwrap();
        assert!(status.success(), "failed to create test archive");

        // Extract and verify
        let extract_dir = tmp_dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_binary(&archive_path, &extract_dir);
        assert!(result.is_ok(), "extract_binary failed: {:?}", result);

        let extracted = result.unwrap();
        assert!(extracted.exists(), "extracted binary should exist");
        let content = fs::read_to_string(&extracted).unwrap();
        assert_eq!(content, "fake seite binary");
    }

    #[test]
    fn test_extract_binary_missing_seite_in_archive() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Create a tar.gz with a different file (not "seite")
        let staging_dir = tmp_dir.path().join("staging");
        fs::create_dir(&staging_dir).unwrap();
        fs::write(staging_dir.join("other-binary"), b"not seite").unwrap();

        let archive_path = tmp_dir.path().join("bad-archive.tar.gz");
        let status = std::process::Command::new("tar")
            .args(["czf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(&staging_dir)
            .arg("other-binary")
            .status()
            .unwrap();
        assert!(status.success());

        let extract_dir = tmp_dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_binary(&archive_path, &extract_dir);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found"),
            "should report binary not found: {err_msg}"
        );
    }

    #[test]
    fn test_extract_binary_invalid_archive() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        // Write garbage data as a .tar.gz
        let archive_path = tmp_dir.path().join("garbage.tar.gz");
        fs::write(&archive_path, b"this is not a tar file").unwrap();

        let extract_dir = tmp_dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();
        let result = extract_binary(&archive_path, &extract_dir);
        assert!(result.is_err(), "should fail on invalid archive");
    }

    // ── Constants tests ────────────────────────────────────────────────

    #[test]
    fn test_repo_constant() {
        assert_eq!(REPO, "seite-sh/seite");
    }

    #[test]
    fn test_download_base_constant() {
        assert_eq!(DOWNLOAD_BASE, "https://seite.sh/download");
        assert!(DOWNLOAD_BASE.starts_with("https://"));
        assert!(!DOWNLOAD_BASE.ends_with('/'));
    }

    // ── SelfUpdateArgs tests ───────────────────────────────────────────

    #[test]
    fn test_self_update_args_default() {
        let args = SelfUpdateArgs {
            target_version: None,
            check: false,
        };
        assert!(args.target_version.is_none());
        assert!(!args.check);
    }

    #[test]
    fn test_self_update_args_with_version() {
        let args = SelfUpdateArgs {
            target_version: Some("0.3.0".to_string()),
            check: false,
        };
        assert_eq!(args.target_version.as_deref(), Some("0.3.0"));
    }

    #[test]
    fn test_self_update_args_check_mode() {
        let args = SelfUpdateArgs {
            target_version: None,
            check: true,
        };
        assert!(args.check);
    }

    // ── GitHub API URL construction test ───────────────────────────────

    #[test]
    fn test_github_api_url() {
        let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
        assert_eq!(
            url,
            "https://api.github.com/repos/seite-sh/seite/releases/latest"
        );
    }

    // ── Backup path extension test ─────────────────────────────────────

    #[test]
    fn test_backup_path_extension() {
        let path = PathBuf::from("/usr/local/bin/seite");
        let backup = path.with_extension("old");
        assert_eq!(backup, PathBuf::from("/usr/local/bin/seite.old"));
    }

    #[test]
    fn test_backup_path_extension_already_has_ext() {
        // with_extension replaces any existing extension
        let path = PathBuf::from("/usr/local/bin/seite.exe");
        let backup = path.with_extension("old");
        assert_eq!(backup, PathBuf::from("/usr/local/bin/seite.old"));
    }

    // ── End-to-end checksum round-trip ─────────────────────────────────

    #[test]
    fn test_checksum_round_trip_various_contents() {
        let tmp_dir = tempfile::TempDir::new().unwrap();

        let test_cases: &[&[u8]] = &[
            b"",
            b"a",
            b"hello world",
            b"binary\x00data\xff\xfe",
            &[0u8; 10000], // 10 KB of zeros
        ];

        for (i, content) in test_cases.iter().enumerate() {
            let archive_name = format!("test-{i}.tar.gz");
            let archive_path = tmp_dir.path().join(&archive_name);
            fs::write(&archive_path, content).unwrap();

            // Compute hash
            let mut hasher = Sha256::new();
            hasher.update(content);
            let hash = hasher.hex_digest();

            // Write checksums file
            let checksums_path = tmp_dir.path().join(format!("checksums-{i}.txt"));
            fs::write(&checksums_path, format!("{}  {}\n", hash, archive_name)).unwrap();

            // Verify
            let result = verify_checksum(&archive_path, &checksums_path, &archive_name);
            assert!(
                result.is_ok(),
                "round-trip failed for test case {i}: {:?}",
                result
            );
        }
    }
}
