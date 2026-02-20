//! Project metadata stored in `.seite/config.json`.
//!
//! Tracks which version of `page` last scaffolded or upgraded the project.
//! Used by `seite upgrade` to determine which upgrade steps to apply, and
//! by `seite build` to nudge users when new features are available.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The directory where page stores its internal project metadata.
const META_DIR: &str = ".seite";

/// The metadata file within the `.seite/` directory.
const META_FILE: &str = "config.json";

/// Project metadata managed by `page`.
///
/// This is stored in `.seite/config.json` and is fully owned by the tool —
/// users should not need to edit it manually.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    /// The version of `page` that last scaffolded or upgraded this project.
    pub version: String,
    /// ISO 8601 timestamp of when the project was first created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialized_at: Option<String>,
}

impl PageMeta {
    /// Create a new `PageMeta` for the current binary version.
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            initialized_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    /// Create a `PageMeta` stamped with the current version but no init timestamp.
    /// Used when upgrading an existing project (preserves the original init time if present).
    pub fn stamp_current_version(existing: Option<&PageMeta>) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            initialized_at: existing.and_then(|m| m.initialized_at.clone()),
        }
    }
}

/// The path to `.seite/config.json` relative to a project root.
pub fn meta_path(project_root: &Path) -> PathBuf {
    project_root.join(META_DIR).join(META_FILE)
}

/// The path to the `.seite/` directory relative to a project root.
pub fn meta_dir(project_root: &Path) -> PathBuf {
    project_root.join(META_DIR)
}

/// Load project metadata from `.seite/config.json`.
///
/// Returns `None` if the file doesn't exist (pre-upgrade project).
pub fn load(project_root: &Path) -> Option<PageMeta> {
    let path = meta_path(project_root);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write project metadata to `.seite/config.json`.
pub fn write(project_root: &Path, meta: &PageMeta) -> std::io::Result<()> {
    let dir = meta_dir(project_root);
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(meta)?;
    fs::write(meta_path(project_root), json)?;
    Ok(())
}

/// Parse the project version, returning `(0, 0, 0)` for pre-metadata projects.
pub fn project_version(project_root: &Path) -> (u64, u64, u64) {
    match load(project_root) {
        Some(meta) => parse_semver(&meta.version),
        None => (0, 0, 0),
    }
}

/// Parse the binary's own version.
pub fn binary_version() -> (u64, u64, u64) {
    parse_semver(env!("CARGO_PKG_VERSION"))
}

/// Check if the project needs an upgrade (project version < binary version).
pub fn needs_upgrade(project_root: &Path) -> bool {
    project_version(project_root) < binary_version()
}

/// Parse a semver string into a tuple. Returns `(0, 0, 0)` on failure.
fn parse_semver(version: &str) -> (u64, u64, u64) {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse().unwrap_or(0);
        let minor = parts[1].parse().unwrap_or(0);
        let patch = parts[2].parse().unwrap_or(0);
        (major, minor, patch)
    } else {
        (0, 0, 0)
    }
}

/// Format a version tuple as a string.
pub fn format_version(v: (u64, u64, u64)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver() {
        assert_eq!(parse_semver("0.1.0"), (0, 1, 0));
        assert_eq!(parse_semver("1.2.3"), (1, 2, 3));
        assert_eq!(parse_semver("10.20.30"), (10, 20, 30));
    }

    #[test]
    fn test_parse_semver_invalid() {
        assert_eq!(parse_semver(""), (0, 0, 0));
        assert_eq!(parse_semver("abc"), (0, 0, 0));
        assert_eq!(parse_semver("1.2"), (0, 0, 0));
    }

    #[test]
    fn test_version_comparison() {
        assert!((0, 0, 0) < (0, 1, 0));
        assert!((0, 1, 0) < (0, 2, 0));
        assert!((0, 1, 0) < (1, 0, 0));
    }

    #[test]
    fn test_meta_current() {
        let meta = PageMeta::current();
        assert_eq!(meta.version, env!("CARGO_PKG_VERSION"));
        assert!(meta.initialized_at.is_some());
    }

    #[test]
    fn test_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let meta = PageMeta::current();
        write(tmp.path(), &meta).unwrap();

        let loaded = load(tmp.path()).unwrap();
        assert_eq!(loaded.version, meta.version);
        assert_eq!(loaded.initialized_at, meta.initialized_at);
    }

    #[test]
    fn test_load_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(load(tmp.path()).is_none());
    }

    #[test]
    fn test_project_version_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(project_version(tmp.path()), (0, 0, 0));
    }

    #[test]
    fn test_format_version() {
        assert_eq!(format_version((0, 1, 0)), "0.1.0");
        assert_eq!(format_version((1, 2, 3)), "1.2.3");
    }
}
