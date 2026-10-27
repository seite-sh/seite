//! `[build.tailwind]`: compile a Tailwind CSS v4 entry file with the
//! standalone CLI (a single executable, no Node).
//!
//! Design:
//! - **One pinned release.** [`PINNED_VERSION`] and the SHA-256 of every
//!   per-platform asset are embedded ([`ASSETS`]). The binary is downloaded
//!   on first use from the GitHub release, size-capped, hashed while it
//!   streams to a temp file in the cache directory, and only made
//!   executable and renamed into place (atomically) once the hash matches.
//! - **seite.toml never picks the program.** The only executable seite runs
//!   is that verified download, or the one named by the user-level
//!   [`BIN_ENV`] environment variable. `binary`, or a `version` other than
//!   the pinned one, in seite.toml is a `config-invalid` error: a cloned
//!   repository must not be able to make `seite check` (which agent hooks
//!   run automatically) execute something of its choosing.
//! - **Cache.** Per user ([`crate::platform::cache_dir`]`/tailwind/<ver>/`),
//!   falling back to `.seite/cache/tailwind/<ver>/` (gitignored, written
//!   only through [`crate::safe_fs`] checks) when there is no usable user
//!   cache dir. The cached file is re-hashed before its first use in each
//!   process (and again if its size or mtime changes); a mismatch means it
//!   is downloaded again.
//! - **Who downloads.** Only `seite build`/`seite serve` (and other real
//!   builds). `seite check`/`audit` scratch builds set
//!   [`TailwindSection::offline`]: they run a cached binary, and without one
//!   copy the input as-is after one info note. Agent hooks and the MCP
//!   `seite_check` tool drop `[build.tailwind]` entirely (never run it).
//! - **Confinement.** `input` must resolve inside the site root with no
//!   symlinked component ([`crate::safe_fs::guard`]) before it is compiled
//!   or copied; `input`/`output` are plain relative paths. The CLI runs with
//!   a minimal environment (PATH, HOME, temp dirs) and a timeout.
//! - **Build.** `build_site_inner` calls [`run`] right after the data files
//!   load (before any page renders, so the staging output holds only
//!   `public/` copies while Tailwind scans the site), as
//!   `<bin> -i <input> -o <scratch> --minify` with the site root as cwd, so
//!   v4's automatic source detection sees templates, content and data.
//!   Failures are `tailwind-failed` / `tailwind-download-failed` errors that
//!   join the build's fatal diagnostics. Step 10 writes the CSS to
//!   `static/<output>` through the same fingerprint/manifest path as every
//!   other static file and skips the raw input.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use crate::config::{SiteConfig, TailwindSection};
use crate::diagnostics::Diagnostic;

/// The Tailwind CSS release seite downloads and verifies.
pub const PINNED_VERSION: &str = "4.3.3";

/// One prebuilt standalone CLI of [`PINNED_VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    /// `std::env::consts::OS` value.
    pub os: &'static str,
    /// Normalized architecture: `arm64` or `x64`.
    pub arch: &'static str,
    /// Linked against musl libc (Alpine and friends).
    pub musl: bool,
    /// Release asset file name.
    pub name: &'static str,
    /// Lowercase hex SHA-256 of the asset.
    pub sha256: &'static str,
}

/// Assets of Tailwind CSS v4.3.3 (released 2026-07-16). Checksums from the
/// release's `sha256sums.txt`, cross-checked against the `digest` GitHub's
/// release API reports for each asset (both agree).
pub const ASSETS: &[Asset] = &[
    Asset {
        os: "macos",
        arch: "arm64",
        musl: false,
        name: "tailwindcss-macos-arm64",
        sha256: "cdf646702987a743464dff4d9c60fd4480d1c1e73dd819a9a67f1078815dce9d",
    },
    Asset {
        os: "macos",
        arch: "x64",
        musl: false,
        name: "tailwindcss-macos-x64",
        sha256: "7922e0953f2110c05976e3bf58f14e643d90427575e766b7d433f5f80cbee7e1",
    },
    Asset {
        os: "linux",
        arch: "arm64",
        musl: false,
        name: "tailwindcss-linux-arm64",
        sha256: "55fd0b241214eff3de1e8ee4f22796662f2d2e7a49bcfca7477cfd0bac398195",
    },
    Asset {
        os: "linux",
        arch: "arm64",
        musl: true,
        name: "tailwindcss-linux-arm64-musl",
        sha256: "71ea4be79c9de9827545682df3e040053fb535d37c71ed2cfdedf9385a0868e0",
    },
    Asset {
        os: "linux",
        arch: "x64",
        musl: false,
        name: "tailwindcss-linux-x64",
        sha256: "dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a",
    },
    Asset {
        os: "linux",
        arch: "x64",
        musl: true,
        name: "tailwindcss-linux-x64-musl",
        sha256: "a04d34ceacc8f52cbe8920ad846cdeb61d3d0021dba32db0d1f77c9d9fad7a6c",
    },
    Asset {
        os: "windows",
        arch: "x64",
        musl: false,
        name: "tailwindcss-windows-x64.exe",
        sha256: "e0e260ce048014e9268f6237ff18f8ccf02cef521cbd0ae04e82c2cdf7aa3955",
    },
];

/// Downloads larger than this are refused (the assets are 80–115 MB).
pub const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

/// Whole-download timeout.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// Opens a URL for reading (injected so tests never touch the network).
pub type Fetch<'a> = &'a dyn Fn(&str) -> Result<Box<dyn Read>, String>;

/// The asset for an OS (`std::env::consts::OS`), an architecture
/// (`aarch64`/`arm64` or `x86_64`/`x64`) and libc.
pub fn asset_for(os: &str, arch: &str, musl: bool) -> Option<&'static Asset> {
    let arch = match arch {
        "aarch64" | "arm64" => "arm64",
        "x86_64" | "x64" | "amd64" => "x64",
        _ => return None,
    };
    let musl = musl && os == "linux";
    ASSETS
        .iter()
        .find(|a| a.os == os && a.arch == arch && a.musl == musl)
}

/// The asset for the machine seite runs on.
pub fn host_asset() -> Option<&'static Asset> {
    asset_for(std::env::consts::OS, std::env::consts::ARCH, host_is_musl())
}

fn host_is_musl() -> bool {
    if cfg!(target_env = "musl") {
        return true;
    }
    if std::env::consts::OS != "linux" {
        return false;
    }
    // A glibc build of seite can still run on a musl system (gcompat);
    // the musl loader's presence decides.
    std::fs::read_dir("/lib").is_ok_and(|entries| {
        entries
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("ld-musl-"))
    })
}

/// The release download URL of an asset.
pub fn asset_url(asset: &Asset) -> String {
    format!(
        "https://github.com/tailwindlabs/tailwindcss/releases/download/v{PINNED_VERSION}/{}",
        asset.name
    )
}

/// Validate `[build.tailwind]`: `Err((message, key))` where `key` locates
/// the offending line for `schema::config_line`.
pub fn validate_config(config: &SiteConfig) -> Result<(), (String, String)> {
    let Some(tw) = &config.build.tailwind else {
        return Ok(());
    };
    let key = |name: &str| format!("[build.tailwind]{name}");
    if tw.binary.is_some() {
        return Err((
            format!(
                "[build.tailwind] binary is not supported: seite.toml cannot choose a program for seite to run (a cloned repository could run anything through `seite check`). Remove it; to use your own tailwindcss, set the {BIN_ENV} environment variable (e.g. {BIN_ENV}=/usr/local/bin/tailwindcss)"
            ),
            key("binary"),
        ));
    }
    if let Some(version) = &tw.version {
        let wanted = version.trim().trim_start_matches('v');
        if wanted != PINNED_VERSION {
            return Err((
                format!(
                    "[build.tailwind] version = \"{version}\": seite only runs Tailwind {PINNED_VERSION}, the release whose SHA-256 checksums it ships. Remove `version`; to use tailwindcss {wanted}, set the {BIN_ENV} environment variable to your own executable"
                ),
                key("version"),
            ));
        }
    }
    for (name, value) in [("input", &tw.input), ("output", &tw.output)] {
        if !is_plain_relative(value) {
            return Err((
                format!(
                    "[build.tailwind] {name} = \"{value}\" must be a relative path inside the site without `..`"
                ),
                key(name),
            ));
        }
    }
    Ok(())
}

/// User-level override for the Tailwind executable (never read from
/// seite.toml).
pub const BIN_ENV: &str = "SEITE_TAILWIND_BIN";

/// A non-empty relative path with only normal components.
fn is_plain_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.trim().is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
        && p.components().next().is_some()
}

/// Hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Files already verified in this process: (path, size, mtime).
static VERIFIED: Mutex<Vec<(PathBuf, u64, Option<SystemTime>)>> = Mutex::new(Vec::new());

fn stamp(path: &Path) -> Option<(u64, Option<SystemTime>)> {
    let meta = std::fs::metadata(path).ok()?;
    meta.is_file().then(|| (meta.len(), meta.modified().ok()))
}

/// Whether `path` exists and hashes to `sha256` (re-hashed once per
/// process, and whenever its size or mtime changed).
pub fn is_verified(path: &Path, sha256: &str) -> bool {
    let Some((len, mtime)) = stamp(path) else {
        return false;
    };
    if let Ok(seen) = VERIFIED.lock() {
        if seen
            .iter()
            .any(|(p, l, m)| p == path && *l == len && *m == mtime)
        {
            return true;
        }
    }
    let ok = sha256_file(path).is_ok_and(|h| h == sha256);
    if ok {
        if let Ok(mut seen) = VERIFIED.lock() {
            seen.retain(|(p, _, _)| p != path);
            seen.push((path.to_path_buf(), len, mtime));
        }
    }
    ok
}

/// Make sure `dir/<asset name>` is the verified asset, downloading it with
/// `fetch` when it is missing or corrupt. The download streams into a temp
/// file in `dir` (size-capped, hashed on the way); only a matching file is
/// made executable and renamed into place.
pub fn ensure_cached(dir: &Path, asset: &Asset, fetch: Fetch) -> Result<PathBuf, String> {
    ensure_cached_capped(dir, asset, fetch, MAX_DOWNLOAD_BYTES)
}

fn ensure_cached_capped(
    dir: &Path,
    asset: &Asset,
    fetch: Fetch,
    cap: u64,
) -> Result<PathBuf, String> {
    let dest = dir.join(asset.name);
    if is_verified(&dest, asset.sha256) {
        return Ok(dest);
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let url = asset_url(asset);
    let mut reader = fetch(&url)?.take(cap + 1);
    let mut tmp = tempfile::Builder::new()
        .prefix(".download-")
        .tempfile_in(dir)
        .map_err(|e| format!("cannot write to {}: {e}", dir.display()))?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("download of {url} failed: {e}"))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > cap {
            return Err(format!(
                "download of {url} is larger than {} MB; refusing it",
                cap / (1024 * 1024)
            ));
        }
        hasher.update(&buf[..n]);
        tmp.write_all(&buf[..n])
            .map_err(|e| format!("cannot write the download: {e}"))?;
    }
    tmp.flush()
        .map_err(|e| format!("cannot write the download: {e}"))?;
    let got = hex(&hasher.finalize());
    if got != asset.sha256 {
        // The temp file is deleted when `tmp` drops.
        return Err(format!(
            "checksum mismatch for {url}: expected {}, got {got}; the download was discarded",
            asset.sha256
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("cannot make the download executable: {e}"))?;
    }
    tmp.persist(&dest)
        .map_err(|e| format!("cannot move the download to {}: {e}", dest.display()))?;
    // Remember it as verified (hashed above).
    is_verified(&dest, asset.sha256);
    Ok(dest)
}

/// Open `url` with ureq (follows GitHub's redirect to its download host).
fn http_fetch(url: &str) -> Result<Box<dyn Read>, String> {
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .build(),
    );
    let response = agent
        .get(url)
        .header("User-Agent", concat!("seite/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("download of {url} failed: {e}"))?;
    Ok(Box::new(response.into_body().into_reader()))
}

/// Cache directories for the pinned binary, preferred first: the user
/// cache, then the project's `.seite/cache/tailwind/<ver>/`.
fn cache_dirs(root: &Path) -> Vec<(PathBuf, bool)> {
    let mut dirs = Vec::new();
    if let Some(user) = crate::platform::cache_dir() {
        dirs.push((user.join("tailwind").join(PINNED_VERSION), false));
    }
    dirs.push((
        root.join(".seite")
            .join("cache")
            .join("tailwind")
            .join(PINNED_VERSION),
        true,
    ));
    dirs
}

/// The [`BIN_ENV`] override, when set and non-empty.
fn env_binary() -> Option<PathBuf> {
    std::env::var_os(BIN_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Where the Tailwind executable is: `env_bin` ([`BIN_ENV`]), or the
/// verified pinned download (fetched with `fetch` when not cached).
/// `Ok(None)`: offline and not cached (the caller copies the input as-is).
pub fn resolve_binary(
    tw: &TailwindSection,
    root: &Path,
    env_bin: Option<&Path>,
    fetch: Fetch,
) -> Result<Option<PathBuf>, Box<Diagnostic>> {
    if let Some(path) = env_bin {
        if !path.is_file() {
            return Err(Box::new(
                Diagnostic::error(
                    "tailwind-failed",
                    format!("{BIN_ENV}={} does not exist", path.display()),
                )
                .with_file("seite.toml")
                .with_hint(format!(
                    "point {BIN_ENV} at a tailwindcss v4 executable, or unset it to use the pinned download"
                )),
            ));
        }
        return Ok(Some(path.to_path_buf()));
    }
    let Some(asset) = host_asset() else {
        return Err(Box::new(
            Diagnostic::error(
                "tailwind-download-failed",
                format!(
                    "Tailwind publishes no standalone CLI for {}/{}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ),
            )
            .with_file("seite.toml")
            .with_hint(format!(
                "build or install tailwindcss for this machine and set the {BIN_ENV} environment variable to it"
            )),
        ));
    };
    let dirs = cache_dirs(root);
    if let Some(found) = dirs
        .iter()
        .map(|(dir, _)| dir.join(asset.name))
        .find(|p| is_verified(p, asset.sha256))
    {
        return Ok(Some(found));
    }
    if tw.offline {
        info_stderr(&format!(
            "Tailwind CSS v{PINNED_VERSION} is not downloaded yet, so this check skips it (`seite build` downloads it once)"
        ));
        return Ok(None);
    }
    let mut errors = Vec::new();
    for (dir, in_project) in &dirs {
        if *in_project {
            if let Err(e) = crate::safe_fs::create_dir_all(root, dir)
                .and_then(|()| crate::safe_fs::guard(root, &dir.join(asset.name)))
            {
                errors.push(e.to_string());
                continue;
            }
        }
        info_stderr(&format!(
            "Downloading Tailwind CSS v{PINNED_VERSION} ({}) to {} (once; verified by SHA-256)",
            asset.name,
            dir.display()
        ));
        match ensure_cached(dir, asset, fetch) {
            Ok(path) => return Ok(Some(path)),
            Err(e) => {
                let checksum = e.contains("checksum mismatch");
                errors.push(e);
                // A bad download is not the directory's fault: don't retry.
                if checksum {
                    break;
                }
            }
        }
    }
    Err(Box::new(Diagnostic::error(
        "tailwind-download-failed",
        format!(
            "could not download Tailwind CSS v{PINNED_VERSION}: {}",
            errors.join("; ")
        ),
    )
    .with_file("seite.toml")
    .with_hint(format!(
        "check the network and retry, or download {} yourself, verify it, and set the {BIN_ENV} environment variable to it",
        asset_url(asset)
    ))))
}

/// An info line on stderr (library code: stdout may carry JSON or MCP).
fn info_stderr(msg: &str) {
    if !crate::output::is_json() || crate::output::is_verbose() {
        eprintln!("{} {msg}", console::style("ℹ").blue().bold());
    }
}

/// Run the CLI: `<bin> -i <input> -o <scratch> --minify` in `root`.
pub fn compile(bin: &Path, tw: &TailwindSection, root: &Path) -> Result<Vec<u8>, Box<Diagnostic>> {
    let fail = |message: String, hint: &str| {
        Box::new(
            Diagnostic::error("tailwind-failed", message)
                .with_file(&tw.input)
                .with_hint(hint.to_string()),
        )
    };
    let input = confined_input(tw, root)?;
    if !input.is_file() {
        return Err(fail(
            format!("Tailwind input `{}` does not exist", tw.input),
            "create it with `@import \"tailwindcss\";` (or `seite theme new --tailwind`), or point [build.tailwind] input at your entry CSS",
        ));
    }
    let scratch = tempfile::Builder::new()
        .prefix("seite-tailwind-")
        .tempdir()
        .map_err(|e| fail(format!("cannot create a temp dir: {e}"), ""))?;
    let out = scratch.path().join("tailwind.css");
    let mut command = Command::new(bin);
    command
        .current_dir(root)
        .arg("-i")
        .arg(&tw.input)
        .arg("-o")
        .arg(&out)
        .arg("--minify")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // A minimal environment: no tokens or other secrets from the caller's.
    for var in KEPT_ENV {
        if let Some(value) = std::env::var_os(var) {
            command.env(var, value);
        }
    }
    let (status, stderr) = run_with_timeout(command, COMPILE_TIMEOUT).map_err(|e| {
        fail(
            format!("could not run {}: {e}", bin.display()),
            &format!("check that the Tailwind CLI ({BIN_ENV} or the cached download) is a tailwindcss executable"),
        )
    })?;
    if !status.success() {
        let stderr = strip_ansi(&String::from_utf8_lossy(&stderr));
        let tail: Vec<&str> = stderr
            .lines()
            // Drop the CLI's banner and box drawing around its messages.
            .map(|l| l.trim_start_matches(['│', '┌', '└', '├', ' ']).trim_end())
            .filter(|l| !l.is_empty() && !l.starts_with('≈') && *l != "Error:")
            .collect();
        let tail = tail[tail.len().saturating_sub(15)..].join("\n");
        let status = status
            .code()
            .map_or("a signal".to_string(), |c| format!("status {c}"));
        return Err(fail(
            format!(
                "tailwindcss exited with {status}{}{tail}",
                if tail.is_empty() { "" } else { ":\n" }
            ),
            "fix the CSS entry file (see Tailwind's message); run the same command by hand from the site root: tailwindcss -i <input> -o /tmp/out.css",
        ));
    }
    std::fs::read(&out).map_err(|e| {
        fail(
            format!("tailwindcss wrote no output ({e})"),
            &format!(
                "check that the Tailwind CLI ({BIN_ENV} or the cached download) is Tailwind v4"
            ),
        )
    })
}

/// Environment variables the Tailwind CLI keeps (everything else is
/// cleared).
const KEPT_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "TMPDIR",
    "TMP",
    "TEMP",
    "SYSTEMROOT",
    "LANG",
];

/// Longest a Tailwind compile may take before it is killed.
const COMPILE_TIMEOUT: Duration = Duration::from_secs(120);

/// Run `command` (stderr piped) and wait at most `timeout`; returns the
/// exit status and stderr. A process still running at the deadline is
/// killed.
fn run_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> std::io::Result<(std::process::ExitStatus, Vec<u8>)> {
    let mut child = command.spawn()?;
    let mut stderr_pipe = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("timed out after {} s", timeout.as_secs()),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stderr = reader.join().unwrap_or_default();
    Ok((status, stderr))
}

/// `root/<input>`, refused (as `tailwind-failed`) when a component is a
/// symlink or it resolves outside the site root.
pub fn confined_input(tw: &TailwindSection, root: &Path) -> Result<PathBuf, Box<Diagnostic>> {
    let input = root.join(&tw.input);
    crate::safe_fs::guard(root, &input).map_err(|e| {
        Box::new(
            Diagnostic::error("tailwind-failed", format!("Tailwind input: {e}"))
                .with_file("seite.toml")
                .with_hint(
                    "[build.tailwind] input must be a real file inside the site (no symlinks)",
                ),
        )
    })?;
    Ok(input)
}

/// Drop ANSI escape sequences (`ESC [ ... letter`).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// The compiled stylesheet for the site, or `None` without
/// `[build.tailwind]`. Offline with no cached binary: the raw input.
pub fn run(config: &SiteConfig, root: &Path) -> Result<Option<Vec<u8>>, Box<Diagnostic>> {
    run_with(config, root, env_binary().as_deref(), &http_fetch)
}

/// [`run`] with an injected [`BIN_ENV`] value and download function.
pub fn run_with(
    config: &SiteConfig,
    root: &Path,
    env_bin: Option<&Path>,
    fetch: Fetch,
) -> Result<Option<Vec<u8>>, Box<Diagnostic>> {
    let Some(tw) = &config.build.tailwind else {
        return Ok(None);
    };
    if tw.never_run {
        // As if Tailwind were absent: the input as-is, nothing reported.
        return Ok(confined_input(tw, root)
            .ok()
            .and_then(|input| std::fs::read(input).ok()));
    }
    match resolve_binary(tw, root, env_bin, fetch)? {
        Some(bin) => compile(&bin, tw, root).map(Some),
        None => {
            let input = confined_input(tw, root)?;
            Ok(Some(std::fs::read(input).unwrap_or_default()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn fake_asset(content: &[u8]) -> Asset {
        let digest = hex(&Sha256::digest(content));
        Asset {
            os: "linux",
            arch: "x64",
            musl: false,
            name: "tailwindcss-test",
            sha256: Box::leak(digest.into_boxed_str()),
        }
    }

    #[test]
    fn test_asset_mapping() {
        let name = |os, arch, musl| asset_for(os, arch, musl).map(|a| a.name);
        assert_eq!(
            name("macos", "aarch64", false),
            Some("tailwindcss-macos-arm64")
        );
        assert_eq!(
            name("macos", "x86_64", false),
            Some("tailwindcss-macos-x64")
        );
        // musl only matters on Linux.
        assert_eq!(name("macos", "x86_64", true), Some("tailwindcss-macos-x64"));
        assert_eq!(
            name("linux", "aarch64", false),
            Some("tailwindcss-linux-arm64")
        );
        assert_eq!(
            name("linux", "aarch64", true),
            Some("tailwindcss-linux-arm64-musl")
        );
        assert_eq!(
            name("linux", "x86_64", false),
            Some("tailwindcss-linux-x64")
        );
        assert_eq!(
            name("linux", "x86_64", true),
            Some("tailwindcss-linux-x64-musl")
        );
        assert_eq!(
            name("windows", "x86_64", false),
            Some("tailwindcss-windows-x64.exe")
        );
        assert_eq!(name("windows", "aarch64", false), None);
        assert_eq!(name("freebsd", "x86_64", false), None);
        assert_eq!(name("linux", "riscv64", false), None);
        for a in ASSETS {
            assert_eq!(a.sha256.len(), 64);
            assert!(a.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
        assert!(asset_url(&ASSETS[0])
            .starts_with("https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/"));
    }

    #[test]
    fn test_download_verified_and_cached() {
        let tmp = tempfile::tempdir().unwrap();
        let asset = fake_asset(b"#!/bin/sh\necho hi\n");
        let calls = Cell::new(0);
        let fetch = |_: &str| -> Result<Box<dyn Read>, String> {
            calls.set(calls.get() + 1);
            Ok(Box::new(std::io::Cursor::new(
                b"#!/bin/sh\necho hi\n".to_vec(),
            )))
        };
        let path = ensure_cached(tmp.path(), &asset, &fetch).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"#!/bin/sh\necho hi\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        // Cache hit: no second download.
        ensure_cached(tmp.path(), &asset, &fetch).unwrap();
        assert_eq!(calls.get(), 1);
        // No temp files left behind.
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_download_bad_checksum_is_discarded() {
        let tmp = tempfile::tempdir().unwrap();
        let asset = fake_asset(b"good");
        let fetch = |_: &str| -> Result<Box<dyn Read>, String> {
            Ok(Box::new(std::io::Cursor::new(b"evil".to_vec())))
        };
        let err = ensure_cached(tmp.path(), &asset, &fetch).unwrap_err();
        assert!(err.contains("checksum mismatch"), "{err}");
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn test_corrupt_cache_is_downloaded_again() {
        let tmp = tempfile::tempdir().unwrap();
        let asset = fake_asset(b"good binary");
        std::fs::write(tmp.path().join(asset.name), b"tampered").unwrap();
        assert!(!is_verified(&tmp.path().join(asset.name), asset.sha256));
        let calls = Cell::new(0);
        let fetch = |_: &str| -> Result<Box<dyn Read>, String> {
            calls.set(calls.get() + 1);
            Ok(Box::new(std::io::Cursor::new(b"good binary".to_vec())))
        };
        let path = ensure_cached(tmp.path(), &asset, &fetch).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(std::fs::read(path).unwrap(), b"good binary");
    }

    #[test]
    fn test_download_fetch_error_and_size_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let asset = fake_asset(b"x");
        let failing =
            |_: &str| -> Result<Box<dyn Read>, String> { Err("connection refused".into()) };
        assert_eq!(
            ensure_cached(tmp.path(), &asset, &failing).unwrap_err(),
            "connection refused"
        );
        let huge = |_: &str| -> Result<Box<dyn Read>, String> {
            Ok(Box::new(std::io::repeat(0).take(3 * 1024 * 1024)))
        };
        let err = ensure_cached_capped(tmp.path(), &asset, &huge, 1024 * 1024).unwrap_err();
        assert!(err.contains("larger than"), "{err}");
    }

    fn config_with(tw: TailwindSection) -> SiteConfig {
        let mut config: SiteConfig =
            toml::from_str("collections = []\n[site]\ntitle = \"t\"\n").unwrap();
        config.build.tailwind = Some(tw);
        config
    }

    #[test]
    fn test_validate_config() {
        assert!(validate_config(&config_with(TailwindSection::default())).is_ok());
        let pinned = TailwindSection {
            version: Some(format!("v{PINNED_VERSION}")),
            ..Default::default()
        };
        assert!(validate_config(&config_with(pinned)).is_ok());
        let other = TailwindSection {
            version: Some("4.0.0".into()),
            ..Default::default()
        };
        let (msg, key) = validate_config(&config_with(other)).unwrap_err();
        assert!(msg.contains(BIN_ENV), "{msg}");
        assert_eq!(key, "[build.tailwind]version");
        // seite.toml never names the program to run.
        let with_binary = TailwindSection {
            binary: Some("bin/tailwindcss".into()),
            ..Default::default()
        };
        let (msg, key) = validate_config(&config_with(with_binary)).unwrap_err();
        assert!(msg.contains(BIN_ENV), "{msg}");
        assert_eq!(key, "[build.tailwind]binary");
        for bad in ["../x.css", "/abs/x.css", ""] {
            let tw = TailwindSection {
                input: bad.into(),
                ..Default::default()
            };
            assert!(validate_config(&config_with(tw)).is_err(), "{bad}");
            let tw = TailwindSection {
                output: bad.into(),
                ..Default::default()
            };
            assert!(validate_config(&config_with(tw)).is_err(), "{bad}");
        }
    }

    #[test]
    fn test_config_error_is_located() {
        let tmp = tempfile::tempdir().unwrap();
        let toml = "collections = []\n\n[site]\ntitle = \"t\"\n\n[build.tailwind]\ninput = \"static/css/tw.css\"\nversion = \"4.0.0\"\n";
        std::fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        let err = SiteConfig::load_with_diagnostics(&tmp.path().join("seite.toml")).unwrap_err();
        let crate::error::PageError::Diagnostics(d) = err else {
            panic!("expected diagnostics");
        };
        let d = d.iter().next().unwrap();
        assert_eq!(d.code, "config-invalid");
        assert_eq!(d.line, Some(8));
    }

    #[test]
    fn test_unknown_tailwind_key_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let toml =
            "collections = []\n\n[site]\ntitle = \"t\"\n\n[build.tailwind]\ninptu = \"x.css\"\n";
        std::fs::write(tmp.path().join("seite.toml"), toml).unwrap();
        let (_, warnings) =
            SiteConfig::load_with_diagnostics(&tmp.path().join("seite.toml")).unwrap();
        assert!(
            warnings
                .iter()
                .any(|w| w.code == "config-unknown-key" && w.message.contains("inptu")),
            "{warnings:?}"
        );
    }

    #[test]
    fn test_offline_without_cache_copies_input() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("static/css")).unwrap();
        std::fs::write(
            tmp.path().join("static/css/tailwind.css"),
            "@import \"tailwindcss\";",
        )
        .unwrap();
        let tw = TailwindSection {
            offline: true,
            ..Default::default()
        };
        let never = |_: &str| -> Result<Box<dyn Read>, String> { panic!("no download offline") };
        // The pinned binary is not in this fresh project's cache; the user
        // cache may hold it on a developer machine, so accept either.
        match resolve_binary(&tw, tmp.path(), None, &never).unwrap() {
            None => {
                let css = run_with(&config_with(tw), tmp.path(), None, &never).unwrap();
                assert_eq!(css.unwrap(), b"@import \"tailwindcss\";");
            }
            Some(path) => assert!(path.is_file()),
        }
    }

    #[test]
    fn test_missing_binary_and_input() {
        let tmp = tempfile::tempdir().unwrap();
        let never = |_: &str| -> Result<Box<dyn Read>, String> { panic!("no download") };
        let missing = tmp.path().join("nope/tailwindcss");
        let d = resolve_binary(
            &TailwindSection::default(),
            tmp.path(),
            Some(&missing),
            &never,
        )
        .unwrap_err();
        assert_eq!(d.code, "tailwind-failed");
        assert!(d.message.contains(BIN_ENV));
        let d = compile(
            Path::new("/bin/true"),
            &TailwindSection::default(),
            tmp.path(),
        )
        .unwrap_err();
        assert_eq!(d.code, "tailwind-failed");
        assert!(d.message.contains("does not exist"));
    }

    #[cfg(unix)]
    #[test]
    fn test_input_symlink_or_outside_root_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("site");
        std::fs::create_dir_all(root.join("static/css")).unwrap();
        std::fs::write(tmp.path().join("secret.css"), "secret").unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join("secret.css"),
            root.join("static/css/tailwind.css"),
        )
        .unwrap();
        let tw = TailwindSection {
            offline: true,
            ..Default::default()
        };
        // Neither compiled nor copied raw.
        let d = confined_input(&tw, &root).unwrap_err();
        assert_eq!(d.code, "tailwind-failed");
        let d = compile(Path::new("/bin/true"), &tw, &root).unwrap_err();
        assert!(d.message.contains("symlink"), "{}", d.message);
    }

    #[cfg(unix)]
    #[test]
    fn test_compile_runs_with_minimal_env_and_timeout() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("static/css")).unwrap();
        std::fs::write(tmp.path().join("static/css/tailwind.css"), "x").unwrap();
        // Writes the environment it sees to the -o path.
        let script = tmp.path().join("tw.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && out=\"$2\"; shift; done\nenv > \"$out\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let css = compile(&script, &TailwindSection::default(), tmp.path()).unwrap();
        let env = String::from_utf8(css).unwrap();
        for line in env.lines() {
            let name = line.split('=').next().unwrap_or("");
            assert!(
                KEPT_ENV.contains(&name) || ["PWD", "SHLVL", "_", "OLDPWD"].contains(&name),
                "leaked env var {name}"
            );
        }
        let mut sleeper = Command::new("/bin/sh");
        sleeper.args(["-c", "sleep 5"]).stderr(Stdio::piped());
        let err = run_with_timeout(sleeper, Duration::from_millis(100)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\u{1b}[31mError\u{1b}[0m: x"), "Error: x");
    }
}
