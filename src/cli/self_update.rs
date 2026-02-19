//! `page self-update` — update the page binary to the latest release.
//!
//! Fetches the latest release from GitHub, downloads the appropriate binary
//! for the current platform, verifies the checksum, and replaces the running
//! binary. Uses the same release infrastructure as install.sh.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::Args;

use crate::output::human;

const REPO: &str = "sanchezomar/page";

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
        human::success(&format!(
            "Already up to date (page {current_version})."
        ));
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
                "Run `page self-update` to install page {target_version}."
            ));
            std::process::exit(1); // exit 1 = update available (useful for CI)
        }
        return Ok(());
    }

    // 3. Detect platform
    let target_triple = detect_target_triple()?;
    let archive_name = format!("page-{target_triple}.tar.gz");
    let download_url = format!(
        "https://github.com/{REPO}/releases/download/{target_tag}/{archive_name}"
    );
    let checksums_url = format!(
        "https://github.com/{REPO}/releases/download/{target_tag}/checksums-sha256.txt"
    );

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
        "Updated page {current_version} → {target_version}"
    ));
    human::info("Run `page upgrade` in your projects to update their config files.");

    Ok(())
}

/// Fetch the latest release tag from GitHub.
fn fetch_latest_tag() -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let response = ureq::get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "page-self-update")
        .call()
        .map_err(|e| anyhow::anyhow!("Failed to check for updates: {e}"))?;

    let body: serde_json::Value = response.into_json()?;
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
             irm https://raw.githubusercontent.com/sanchezomar/page/main/install.ps1 | iex"
        );
    } else {
        anyhow::bail!(
            "Unsupported operating system. Install from source: cargo install page"
        );
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        anyhow::bail!(
            "Unsupported architecture. Install from source: cargo install page"
        );
    };

    Ok(format!("{arch}-{os}"))
}

/// Download a URL to a local file using ureq.
fn download_file(url: &str, dest: &PathBuf) -> anyhow::Result<()> {
    let response = ureq::get(url)
        .set("User-Agent", "page-self-update")
        .call()
        .map_err(|e| anyhow::anyhow!("Download failed ({url}): {e}"))?;

    let mut reader = response.into_reader();
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
        .ok_or_else(|| {
            anyhow::anyhow!("Archive {archive_name} not found in checksums file")
        })?;

    if actual != expected {
        anyhow::bail!(
            "Checksum mismatch!\n  Expected: {expected}\n  Actual:   {actual}"
        );
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
        let tmp = tempfile::NamedTempFile::new().unwrap();
        fs::write(tmp.path(), &self.data).unwrap();

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

/// Extract the `page` binary from a tar.gz archive.
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

    let binary = dest_dir.join("page");
    if !binary.exists() {
        anyhow::bail!("Binary 'page' not found in archive");
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
