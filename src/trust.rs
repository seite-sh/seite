//! Per-user project trust (`seite trust`).
//!
//! Some features run code that comes from the project itself: Tailwind CSS
//! (`[build.tailwind]`) executes the JavaScript of any `@plugin`/`@config`
//! the project's CSS names, like an npm build script. seite only runs them
//! for projects the user trusted explicitly (`seite trust`, or
//! `seite theme new --tailwind` for a project they just set up).
//!
//! The list lives in the user's config dir
//! ([`crate::platform::config_dir`]`/trusted-projects.json`), never in a
//! repository, keyed by the canonical site root, and is written atomically.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// File name of the trust list inside the user config dir.
pub const TRUST_FILE: &str = "trusted-projects.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrustList {
    /// Canonical site roots, sorted.
    #[serde(default)]
    projects: Vec<String>,
}

/// The trust list for this user, or `None` without a config dir.
pub fn trust_file() -> Option<PathBuf> {
    crate::platform::config_dir().map(|dir| dir.join(TRUST_FILE))
}

/// The key a site root is recorded under (its canonical path).
fn key(root: &Path) -> std::io::Result<String> {
    Ok(root.canonicalize()?.to_string_lossy().into_owned())
}

fn load(file: &Path) -> TrustList {
    std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Environment variable that trusts the project for this run only: the CI
/// escape hatch (runners have no trust list). Read from the process
/// environment only, never from seite.toml or `.env` files, so a repository
/// cannot set it for a local `seite build`.
pub const TRUST_PROJECT_ENV: &str = "SEITE_TRUST_PROJECT";

/// Whether [`TRUST_PROJECT_ENV`] is `1` (or `true`).
pub fn env_trusts_project() -> bool {
    std::env::var(TRUST_PROJECT_ENV).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// Whether the user trusts the project at `root`.
pub fn is_trusted(root: &Path) -> bool {
    trust_file().is_some_and(|file| is_trusted_in(&file, root))
}

/// [`is_trusted`] against an explicit trust list file.
pub fn is_trusted_in(file: &Path, root: &Path) -> bool {
    let Ok(key) = key(root) else {
        return false;
    };
    load(file).projects.contains(&key)
}

/// Trusted project roots in the list file.
pub fn list_in(file: &Path) -> Vec<String> {
    load(file).projects
}

/// Trust (`trusted = true`) or revoke `root` in `file`; returns whether the
/// list changed. Written atomically (temp file + rename).
pub fn set_trusted_in(file: &Path, root: &Path, trusted: bool) -> anyhow::Result<bool> {
    let key = key(root)?;
    let mut list = load(file);
    let present = list.projects.contains(&key);
    if present == trusted {
        return Ok(false);
    }
    if trusted {
        list.projects.push(key);
        list.projects.sort();
    } else {
        list.projects.retain(|p| p != &key);
    }
    let dir = file
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent directory for {}", file.display()))?;
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".trusted-projects-")
        .tempfile_in(dir)?;
    tmp.write_all(serde_json::to_string_pretty(&list)?.as_bytes())?;
    tmp.write_all(b"\n")?;
    tmp.flush()?;
    tmp.persist(file)
        .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", file.display()))?;
    Ok(true)
}

/// [`set_trusted_in`] on this user's trust list.
pub fn set_trusted(root: &Path, trusted: bool) -> anyhow::Result<bool> {
    let file = trust_file().ok_or_else(|| {
        anyhow::anyhow!("no user config directory (HOME/APPDATA is not set); cannot record trust")
    })?;
    set_trusted_in(&file, root, trusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("cfg/trusted-projects.json");
        let site = tmp.path().join("site");
        std::fs::create_dir_all(&site).unwrap();
        assert!(!is_trusted_in(&file, &site));
        assert!(set_trusted_in(&file, &site, true).unwrap());
        assert!(!set_trusted_in(&file, &site, true).unwrap());
        assert!(is_trusted_in(&file, &site));
        // Keyed by canonical path: a `..` spelling is the same project.
        let other = tmp.path().join("cfg/../site");
        assert!(is_trusted_in(&file, &other));
        assert_eq!(list_in(&file).len(), 1);
        assert!(set_trusted_in(&file, &site, false).unwrap());
        assert!(!is_trusted_in(&file, &site));
        assert!(list_in(&file).is_empty());
        // No temp files left behind.
        assert_eq!(
            std::fs::read_dir(tmp.path().join("cfg")).unwrap().count(),
            1
        );
    }

    #[test]
    fn test_missing_or_corrupt_list_trusts_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("trusted-projects.json");
        assert!(!is_trusted_in(&file, tmp.path()));
        std::fs::write(&file, "not json").unwrap();
        assert!(!is_trusted_in(&file, tmp.path()));
        assert!(!is_trusted_in(&file, &tmp.path().join("missing")));
    }
}
