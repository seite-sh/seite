//! Build cache for incremental builds.
//!
//! Tracks file modification times and content hashes to determine which
//! content items need rebuilding. The cache is stored as JSON in
//! `.seite/build-cache.json` and persists between builds.
//!
//! **Invalidation rules:**
//! - Content file changed → rebuild only that item (indexes/feeds are always regenerated)
//! - Template file changed → full rebuild
//! - Data file changed → full rebuild
//! - Config changed → full rebuild
//! - Static file changed → copy only that file

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// On-disk cache format stored in `.seite/build-cache.json`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BuildCache {
    /// Content files: source path → entry with mtime + hash.
    pub content: HashMap<String, CacheEntry>,
    /// Template files: path → mtime.
    pub templates: HashMap<String, u64>,
    /// Data files: path → mtime.
    pub data: HashMap<String, u64>,
    /// Static files: path → mtime.
    pub static_files: HashMap<String, u64>,
    /// Config file mtime.
    pub config_mtime: u64,
    /// FNV hash of the config file contents.
    pub config_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheEntry {
    /// File modification time as seconds since epoch.
    pub mtime: u64,
    /// FNV-1a hash of file contents (first 16 hex chars).
    pub hash: String,
}

/// What changed since the last build.
#[derive(Debug)]
pub struct ChangeSet {
    /// Content files that need rebuilding (relative paths within content dir).
    pub changed_content: Vec<PathBuf>,
    /// Content files that were deleted since last build.
    pub deleted_content: Vec<PathBuf>,
    /// Whether a full rebuild is required (template/data/config change).
    pub needs_full_rebuild: bool,
    /// Reason for full rebuild (for display).
    pub full_rebuild_reason: Option<String>,
    /// Static files that changed.
    pub changed_static: Vec<PathBuf>,
    /// Total content files (for stats).
    pub total_content: usize,
}

impl ChangeSet {
    /// True if nothing changed at all.
    pub fn is_empty(&self) -> bool {
        !self.needs_full_rebuild
            && self.changed_content.is_empty()
            && self.deleted_content.is_empty()
            && self.changed_static.is_empty()
    }

    /// Number of content items that need rebuilding.
    pub fn content_rebuild_count(&self) -> usize {
        if self.needs_full_rebuild {
            self.total_content
        } else {
            self.changed_content.len()
        }
    }
}

impl BuildCache {
    /// Cache file path within the project.
    pub fn cache_path(project_root: &Path) -> PathBuf {
        project_root.join(".seite").join("build-cache.json")
    }

    /// Load cache from disk, returning default if missing or corrupt.
    pub fn load(project_root: &Path) -> Self {
        let path = Self::cache_path(project_root);
        match fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save cache to disk.
    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::cache_path(project_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self)?;
        fs::write(&path, json)
    }

    /// Compare current file state against the cache to determine what changed.
    pub fn diff(
        &self,
        config_path: &Path,
        content_dir: &Path,
        template_dir: &Path,
        data_dir: &Path,
        static_dir: &Path,
    ) -> ChangeSet {
        let mut changeset = ChangeSet {
            changed_content: Vec::new(),
            deleted_content: Vec::new(),
            needs_full_rebuild: false,
            full_rebuild_reason: None,
            changed_static: Vec::new(),
            total_content: 0,
        };

        // Check config
        if config_path.exists() {
            let current_mtime = file_mtime(config_path);
            if current_mtime != self.config_mtime {
                // Mtime changed — check content hash to avoid false positives
                if let Ok(content) = fs::read(config_path) {
                    let hash = fnv_hash16(&content);
                    if hash != self.config_hash {
                        changeset.needs_full_rebuild = true;
                        changeset.full_rebuild_reason = Some("config file changed".to_string());
                    }
                }
            }
        }

        // Check templates
        if !changeset.needs_full_rebuild {
            if let Some(reason) = self.check_dir_changed(template_dir, &self.templates) {
                changeset.needs_full_rebuild = true;
                changeset.full_rebuild_reason = Some(format!("template changed: {reason}"));
            }
        }

        // Check data files
        if !changeset.needs_full_rebuild {
            if let Some(reason) = self.check_dir_changed(data_dir, &self.data) {
                changeset.needs_full_rebuild = true;
                changeset.full_rebuild_reason = Some(format!("data file changed: {reason}"));
            }
        }

        // Check content files
        if content_dir.exists() {
            let current_content = scan_md_files(content_dir);
            changeset.total_content = current_content.len();

            if !changeset.needs_full_rebuild {
                // Find changed/new content files
                for (rel_path, mtime) in &current_content {
                    let key = rel_path.to_string_lossy().to_string();
                    match self.content.get(&key) {
                        Some(entry) if entry.mtime == *mtime => {
                            // Unchanged
                        }
                        Some(_) => {
                            // Mtime changed — verify with hash
                            let abs_path = content_dir.join(rel_path);
                            if let Ok(content) = fs::read(&abs_path) {
                                let hash = fnv_hash16(&content);
                                if self.content.get(&key).map(|e| &e.hash) != Some(&hash) {
                                    changeset.changed_content.push(rel_path.clone());
                                }
                            }
                        }
                        None => {
                            // New file
                            changeset.changed_content.push(rel_path.clone());
                        }
                    }
                }

                // Find deleted content files
                for cached_key in self.content.keys() {
                    let rel = PathBuf::from(cached_key);
                    if !current_content.contains_key(&rel) {
                        changeset.deleted_content.push(rel);
                    }
                }
            }
        }

        // Check static files
        if static_dir.exists() {
            let current_static = scan_all_files(static_dir);
            for (rel_path, mtime) in &current_static {
                let key = rel_path.to_string_lossy().to_string();
                match self.static_files.get(&key) {
                    Some(&cached_mtime) if cached_mtime == *mtime => {}
                    _ => {
                        changeset.changed_static.push(rel_path.clone());
                    }
                }
            }
        }

        changeset
    }

    /// Snapshot the current state of all project files into a new cache.
    pub fn snapshot(
        config_path: &Path,
        content_dir: &Path,
        template_dir: &Path,
        data_dir: &Path,
        static_dir: &Path,
    ) -> Self {
        let mut cache = Self::default();

        // Config
        if config_path.exists() {
            cache.config_mtime = file_mtime(config_path);
            if let Ok(content) = fs::read(config_path) {
                cache.config_hash = fnv_hash16(&content);
            }
        }

        // Content
        if content_dir.exists() {
            for (rel_path, mtime) in scan_md_files(content_dir) {
                let abs_path = content_dir.join(&rel_path);
                let hash = fs::read(&abs_path)
                    .map(|c| fnv_hash16(&c))
                    .unwrap_or_default();
                cache.content.insert(
                    rel_path.to_string_lossy().to_string(),
                    CacheEntry { mtime, hash },
                );
            }
        }

        // Templates
        if template_dir.exists() {
            for (rel_path, mtime) in scan_all_files(template_dir) {
                cache
                    .templates
                    .insert(rel_path.to_string_lossy().to_string(), mtime);
            }
        }

        // Data
        if data_dir.exists() {
            for (rel_path, mtime) in scan_all_files(data_dir) {
                cache
                    .data
                    .insert(rel_path.to_string_lossy().to_string(), mtime);
            }
        }

        // Static
        if static_dir.exists() {
            for (rel_path, mtime) in scan_all_files(static_dir) {
                cache
                    .static_files
                    .insert(rel_path.to_string_lossy().to_string(), mtime);
            }
        }

        cache
    }

    /// Check if any files in a directory have changed relative to a cached map.
    /// Returns the first changed file's name if something changed.
    fn check_dir_changed(&self, dir: &Path, cached: &HashMap<String, u64>) -> Option<String> {
        if !dir.exists() {
            return if cached.is_empty() {
                None
            } else {
                Some("directory removed".to_string())
            };
        }

        let current = scan_all_files(dir);

        // Check for new or modified files
        for (rel_path, mtime) in &current {
            let key = rel_path.to_string_lossy().to_string();
            match cached.get(&key) {
                Some(&cached_mtime) if cached_mtime == *mtime => {}
                Some(_) => return Some(key),
                None => return Some(format!("{key} (new)")),
            }
        }

        // Check for deleted files
        for cached_key in cached.keys() {
            let rel = PathBuf::from(cached_key);
            if !current.contains_key(&rel) {
                return Some(format!("{cached_key} (deleted)"));
            }
        }

        None
    }
}

/// Get file mtime as seconds since epoch.
fn file_mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// FNV-1a hash → first 16 hex chars.
fn fnv_hash16(data: &[u8]) -> String {
    let mut hash: u64 = 14695981039346656037;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")
}

/// Scan a directory for `.md` files, returning relative paths and mtimes.
fn scan_md_files(dir: &Path) -> HashMap<PathBuf, u64> {
    let mut files = HashMap::new();
    if !dir.exists() {
        return files;
    }
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        if let Ok(rel) = entry.path().strip_prefix(dir) {
            files.insert(rel.to_path_buf(), file_mtime(entry.path()));
        }
    }
    files
}

/// Scan a directory for all files, returning relative paths and mtimes.
fn scan_all_files(dir: &Path) -> HashMap<PathBuf, u64> {
    let mut files = HashMap::new();
    if !dir.exists() {
        return files;
    }
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Ok(rel) = entry.path().strip_prefix(dir) {
            files.insert(rel.to_path_buf(), file_mtime(entry.path()));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_empty_cache_triggers_full_rebuild() {
        let tmp = TempDir::new().unwrap();
        let cache = BuildCache::default();
        let config_path = tmp.path().join("seite.toml");
        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();

        let changeset = cache.diff(
            &config_path,
            &tmp.path().join("content"),
            &tmp.path().join("templates"),
            &tmp.path().join("data"),
            &tmp.path().join("static"),
        );

        // Config exists but cache has no hash → config changed → full rebuild
        assert!(changeset.needs_full_rebuild);
    }

    #[test]
    fn test_no_changes_detected() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&content_dir).unwrap();
        fs::write(content_dir.join("test.md"), "# Hello").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(!changeset.needs_full_rebuild);
        assert!(changeset.changed_content.is_empty());
        assert!(changeset.deleted_content.is_empty());
        assert!(changeset.is_empty());
    }

    #[test]
    fn test_new_content_detected() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&content_dir).unwrap();

        // Snapshot with no content
        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Add a content file
        fs::write(content_dir.join("new-post.md"), "# New Post").unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(!changeset.needs_full_rebuild);
        assert_eq!(changeset.changed_content.len(), 1);
        assert!(changeset.changed_content[0]
            .to_string_lossy()
            .contains("new-post"));
    }

    #[test]
    fn test_template_change_triggers_full_rebuild() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(template_dir.join("base.html"), "<html>").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Add a new template (avoids mtime granularity issues)
        fs::write(template_dir.join("post.html"), "<article>").unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(changeset.needs_full_rebuild);
        assert!(changeset
            .full_rebuild_reason
            .as_ref()
            .unwrap()
            .contains("template"));
    }

    #[test]
    fn test_deleted_content_detected() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&content_dir).unwrap();
        fs::write(content_dir.join("old.md"), "# Old").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Delete the content file
        fs::remove_file(content_dir.join("old.md")).unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(!changeset.needs_full_rebuild);
        assert_eq!(changeset.deleted_content.len(), 1);
    }

    #[test]
    fn test_fnv_hash16_deterministic() {
        let data = b"hello world";
        let h1 = fnv_hash16(data);
        let h2 = fnv_hash16(data);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn test_cache_save_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut cache = BuildCache {
            config_mtime: 12345,
            config_hash: "abc123".to_string(),
            ..Default::default()
        };
        cache.content.insert(
            "posts/hello.md".to_string(),
            CacheEntry {
                mtime: 67890,
                hash: "def456".to_string(),
            },
        );

        cache.save(tmp.path()).unwrap();
        let loaded = BuildCache::load(tmp.path());

        assert_eq!(loaded.config_mtime, 12345);
        assert_eq!(loaded.config_hash, "abc123");
        assert!(loaded.content.contains_key("posts/hello.md"));
    }

    #[test]
    fn test_data_change_triggers_full_rebuild() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&data_dir).unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Add a data file
        fs::write(data_dir.join("nav.yaml"), "- title: Home").unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(changeset.needs_full_rebuild);
        assert!(changeset
            .full_rebuild_reason
            .as_ref()
            .unwrap()
            .contains("data"));
    }

    #[test]
    fn test_changeset_content_rebuild_count() {
        let cs = ChangeSet {
            changed_content: vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
            deleted_content: Vec::new(),
            needs_full_rebuild: false,
            full_rebuild_reason: None,
            changed_static: Vec::new(),
            total_content: 10,
        };
        assert_eq!(cs.content_rebuild_count(), 2);
        assert!(!cs.is_empty());

        // Full rebuild reports total count
        let cs_full = ChangeSet {
            changed_content: Vec::new(),
            deleted_content: Vec::new(),
            needs_full_rebuild: true,
            full_rebuild_reason: Some("config".into()),
            changed_static: Vec::new(),
            total_content: 10,
        };
        assert_eq!(cs_full.content_rebuild_count(), 10);
    }

    #[test]
    fn test_static_file_change_detected() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&static_dir).unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Add a static file — should be detected but NOT trigger full rebuild
        fs::write(static_dir.join("style.css"), "body{}").unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(!changeset.needs_full_rebuild);
        assert_eq!(changeset.changed_static.len(), 1);
        assert!(!changeset.is_empty());
    }

    #[test]
    fn test_load_corrupt_json_returns_default() {
        let tmp = TempDir::new().unwrap();
        let cache_path = BuildCache::cache_path(tmp.path());
        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        fs::write(&cache_path, "not valid json{{{").unwrap();

        let cache = BuildCache::load(tmp.path());
        assert_eq!(cache.config_mtime, 0);
        assert!(cache.content.is_empty());
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let cache = BuildCache::load(tmp.path());
        assert_eq!(cache.config_mtime, 0);
        assert!(cache.content.is_empty());
    }

    #[test]
    fn test_config_mtime_changed_but_hash_same_no_rebuild() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();

        let mut cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );
        // Simulate mtime change without content change (e.g., touch)
        cache.config_mtime = cache.config_mtime.wrapping_sub(1);

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Hash matches even though mtime differs — no rebuild needed
        assert!(!changeset.needs_full_rebuild);
    }

    #[test]
    fn test_template_deleted_triggers_full_rebuild() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(template_dir.join("base.html"), "<html>").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Delete the template
        fs::remove_file(template_dir.join("base.html")).unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(changeset.needs_full_rebuild);
        assert!(changeset
            .full_rebuild_reason
            .as_ref()
            .unwrap()
            .contains("deleted"));
    }

    #[test]
    fn test_template_dir_removed_triggers_full_rebuild() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(template_dir.join("base.html"), "<html>").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Remove entire template directory
        fs::remove_dir_all(&template_dir).unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(changeset.needs_full_rebuild);
        assert!(changeset
            .full_rebuild_reason
            .as_ref()
            .unwrap()
            .contains("directory removed"));
    }

    #[test]
    fn test_snapshot_captures_all_dirs() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&content_dir).unwrap();
        fs::create_dir_all(&template_dir).unwrap();
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&static_dir).unwrap();

        fs::write(content_dir.join("post.md"), "# Post").unwrap();
        fs::write(template_dir.join("base.html"), "<html>").unwrap();
        fs::write(data_dir.join("nav.yaml"), "[]").unwrap();
        fs::write(static_dir.join("style.css"), "body{}").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        assert!(cache.config_mtime > 0);
        assert!(!cache.config_hash.is_empty());
        assert_eq!(cache.content.len(), 1);
        assert_eq!(cache.templates.len(), 1);
        assert_eq!(cache.data.len(), 1);
        assert_eq!(cache.static_files.len(), 1);
    }

    #[test]
    fn test_snapshot_handles_missing_dirs() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();

        // All dirs missing — should not panic
        let cache = BuildCache::snapshot(
            &config_path,
            &tmp.path().join("content"),
            &tmp.path().join("templates"),
            &tmp.path().join("data"),
            &tmp.path().join("static"),
        );

        assert!(cache.content.is_empty());
        assert!(cache.templates.is_empty());
        assert!(cache.data.is_empty());
        assert!(cache.static_files.is_empty());
    }

    #[test]
    fn test_scan_md_files_only_finds_md() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("content");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("post.md"), "# Hello").unwrap();
        fs::write(dir.join("notes.txt"), "not markdown").unwrap();
        fs::write(dir.join("image.png"), "binary").unwrap();

        let files = scan_md_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files.contains_key(&PathBuf::from("post.md")));
    }

    #[test]
    fn test_scan_md_files_nonexistent_dir() {
        let files = scan_md_files(Path::new("/nonexistent/path"));
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_all_files_nonexistent_dir() {
        let files = scan_all_files(Path::new("/nonexistent/path"));
        assert!(files.is_empty());
    }

    #[test]
    fn test_file_mtime_nonexistent_returns_zero() {
        assert_eq!(file_mtime(Path::new("/nonexistent/file")), 0);
    }

    #[test]
    fn test_fnv_hash16_different_inputs() {
        let h1 = fnv_hash16(b"hello");
        let h2 = fnv_hash16(b"world");
        assert_ne!(h1, h2);
        assert_eq!(h1.len(), 16);
        assert_eq!(h2.len(), 16);
    }

    #[test]
    fn test_cache_path_location() {
        let root = Path::new("/project");
        let path = BuildCache::cache_path(root);
        assert_eq!(path, PathBuf::from("/project/.seite/build-cache.json"));
    }

    #[test]
    fn test_check_dir_changed_no_cached_no_dir() {
        // Both empty — nothing existed before, nothing exists now
        let cache = BuildCache::default();
        let result = cache.check_dir_changed(Path::new("/nonexistent"), &HashMap::new());
        assert!(result.is_none());
    }

    #[test]
    fn test_content_skipped_when_full_rebuild() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("seite.toml");
        let content_dir = tmp.path().join("content");
        let template_dir = tmp.path().join("templates");
        let data_dir = tmp.path().join("data");
        let static_dir = tmp.path().join("static");

        fs::write(&config_path, "[site]\ntitle = \"Test\"").unwrap();
        fs::create_dir_all(&content_dir).unwrap();
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(content_dir.join("a.md"), "# A").unwrap();
        fs::write(content_dir.join("b.md"), "# B").unwrap();
        fs::write(template_dir.join("base.html"), "<html>").unwrap();

        let cache = BuildCache::snapshot(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Add a template (triggers full rebuild) AND a new content file
        fs::write(template_dir.join("new.html"), "<div>").unwrap();
        fs::write(content_dir.join("c.md"), "# C").unwrap();

        let changeset = cache.diff(
            &config_path,
            &content_dir,
            &template_dir,
            &data_dir,
            &static_dir,
        );

        // Full rebuild — content changes are NOT individually tracked
        assert!(changeset.needs_full_rebuild);
        assert!(changeset.changed_content.is_empty());
        assert_eq!(changeset.total_content, 3);
        assert_eq!(changeset.content_rebuild_count(), 3);
    }
}
