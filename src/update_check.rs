//! Background update check — shows a one-liner when a newer seite version is available.
//!
//! Stores a cache file at `~/.seite/update-cache.json` so we only hit the
//! network at most once every 24 hours. All errors are silently swallowed —
//! a failed check should never break the CLI.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::output::human;
use crate::platform;

/// How often to check for updates (24 hours).
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// HTTP timeout for the version check (3 seconds).
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

/// Cached update check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateCache {
    /// ISO 8601 timestamp of when we last checked.
    last_check: String,
    /// The latest version string from the remote (e.g. "0.2.0").
    latest_version: String,
}

/// Directory name under the user's home for global seite state.
const CACHE_DIR: &str = ".seite";

/// Cache filename.
const CACHE_FILE: &str = "update-cache.json";

/// Run the background update check. Call this from `main()` after command dispatch.
///
/// - Skips if we checked less than 24 hours ago (reads cache).
/// - If the cache is stale, fetches the latest version from seite.sh/version.txt.
/// - If a newer version is available, prints a one-liner info message.
/// - All errors are silently ignored.
pub fn maybe_notify() {
    // Best-effort: never panic or propagate errors
    let _ = check_and_notify();
}

fn check_and_notify() -> Option<()> {
    let cache_path = cache_path()?;
    let current_version = env!("CARGO_PKG_VERSION");

    // Try to read existing cache
    let cache = load_cache(&cache_path);

    let latest_version = if should_check(&cache) {
        // Fetch from network
        let version = fetch_latest_version()?;
        // Write cache regardless of result
        let _ = write_cache(
            &cache_path,
            &UpdateCache {
                last_check: Utc::now().to_rfc3339(),
                latest_version: version.clone(),
            },
        );
        version
    } else {
        // Use cached version
        cache?.latest_version
    };

    // Compare versions
    if version_is_newer(&latest_version, current_version) {
        human::info(&format!(
            "A new version of seite is available: {current_version} → {latest_version} \
             (run `seite self-update`)"
        ));
    }

    Some(())
}

/// Return the path to `~/.seite/update-cache.json`.
fn cache_path() -> Option<PathBuf> {
    platform::home_dir().map(|home| home.join(CACHE_DIR).join(CACHE_FILE))
}

/// Load the cache file, returning `None` if it doesn't exist or is invalid.
fn load_cache(path: &PathBuf) -> Option<UpdateCache> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write the cache file, creating the directory if needed.
fn write_cache(path: &PathBuf, cache: &UpdateCache) -> Option<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    let json = serde_json::to_string_pretty(cache).ok()?;
    fs::write(path, json).ok()
}

/// Determine if we should fetch a new version from the network.
fn should_check(cache: &Option<UpdateCache>) -> bool {
    match cache {
        None => true,
        Some(c) => {
            let Ok(last) = c.last_check.parse::<DateTime<Utc>>() else {
                return true;
            };
            let elapsed = Utc::now().signed_duration_since(last);
            elapsed.num_seconds() < 0
                || elapsed.to_std().unwrap_or(CHECK_INTERVAL) >= CHECK_INTERVAL
        }
    }
}

/// Fetch the latest version string from seite.sh/version.txt with a short timeout.
fn fetch_latest_version() -> Option<String> {
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(HTTP_TIMEOUT))
            .build(),
    );

    let mut response = agent
        .get("https://seite.sh/version.txt")
        .header("User-Agent", "seite-update-check")
        .call()
        .ok()?;

    let body = response.body_mut().read_to_string().ok()?;
    let version = body.trim().trim_start_matches('v').to_string();
    if version.is_empty() {
        return None;
    }
    Some(version)
}

/// Return true if `latest` is a higher semver than `current`.
fn version_is_newer(latest: &str, current: &str) -> bool {
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
    parse(latest) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_newer() {
        assert!(version_is_newer("0.2.0", "0.1.7"));
        assert!(version_is_newer("1.0.0", "0.9.9"));
        assert!(version_is_newer("0.1.8", "0.1.7"));
        assert!(!version_is_newer("0.1.7", "0.1.7"));
        assert!(!version_is_newer("0.1.6", "0.1.7"));
        assert!(!version_is_newer("0.0.1", "0.1.7"));
    }

    #[test]
    fn test_should_check_no_cache() {
        assert!(should_check(&None));
    }

    #[test]
    fn test_should_check_stale_cache() {
        let old = Utc::now() - chrono::Duration::hours(25);
        let cache = UpdateCache {
            last_check: old.to_rfc3339(),
            latest_version: "0.1.7".to_string(),
        };
        assert!(should_check(&Some(cache)));
    }

    #[test]
    fn test_should_check_fresh_cache() {
        let recent = Utc::now() - chrono::Duration::hours(1);
        let cache = UpdateCache {
            last_check: recent.to_rfc3339(),
            latest_version: "0.1.7".to_string(),
        };
        assert!(!should_check(&Some(cache)));
    }

    #[test]
    fn test_should_check_invalid_timestamp() {
        let cache = UpdateCache {
            last_check: "not-a-date".to_string(),
            latest_version: "0.1.7".to_string(),
        };
        assert!(should_check(&Some(cache)));
    }

    #[test]
    fn test_cache_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("update-cache.json");

        let cache = UpdateCache {
            last_check: Utc::now().to_rfc3339(),
            latest_version: "0.2.0".to_string(),
        };

        write_cache(&path, &cache).unwrap();
        let loaded = load_cache(&path).unwrap();
        assert_eq!(loaded.latest_version, "0.2.0");
        assert_eq!(loaded.last_check, cache.last_check);
    }

    #[test]
    fn test_load_cache_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.json");
        assert!(load_cache(&path).is_none());
    }

    #[test]
    fn test_load_cache_invalid_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("bad.json");
        fs::write(&path, "not json").unwrap();
        assert!(load_cache(&path).is_none());
    }
}
