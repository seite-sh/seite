pub mod build;
pub mod deploy;
pub mod server;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{ResolvedPaths, SiteConfig};
use crate::error::{PageError, Result};

/// Workspace configuration loaded from `seite-workspace.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace: WorkspaceSection,
    pub sites: Vec<WorkspaceSite>,
    #[serde(default)]
    pub cross_site: CrossSiteSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSection {
    pub name: String,
    #[serde(default)]
    pub shared_data: Option<String>,
    #[serde(default)]
    pub shared_static: Option<String>,
    #[serde(default)]
    pub shared_templates: Option<String>,
}

/// A site entry within the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSite {
    pub name: String,
    pub path: String,
    /// Override the site's seite.toml base_url.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Override the site's output directory (relative to workspace root).
    #[serde(default)]
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossSiteSection {
    #[serde(default)]
    pub unified_sitemap: bool,
    #[serde(default)]
    pub unified_search: bool,
    #[serde(default)]
    pub cross_site_links: bool,
}

/// The execution context resolved at startup: either standalone or workspace.
pub enum ExecutionContext {
    Standalone {
        config: Box<SiteConfig>,
        paths: ResolvedPaths,
    },
    Workspace {
        ws_config: WorkspaceConfig,
        ws_root: PathBuf,
        site_filter: Option<String>,
    },
}

const WORKSPACE_FILE: &str = "seite-workspace.toml";

impl WorkspaceConfig {
    /// Load workspace config from a `seite-workspace.toml` file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(PageError::Workspace(format!(
                "workspace config not found: {}",
                path.display()
            )));
        }
        let contents = std::fs::read_to_string(path)?;
        let config: WorkspaceConfig =
            toml::from_str(&contents).map_err(|e| PageError::Workspace(e.to_string()))?;
        config.validate(path.parent().unwrap_or(Path::new(".")))?;
        Ok(config)
    }

    /// Validate the workspace config: check for duplicate site names and valid paths.
    fn validate(&self, ws_root: &Path) -> Result<()> {
        if self.sites.is_empty() {
            return Err(PageError::Workspace(
                "workspace must contain at least one site".into(),
            ));
        }

        // Check for duplicate site names
        let mut seen = std::collections::HashSet::new();
        for site in &self.sites {
            if !seen.insert(&site.name) {
                return Err(PageError::Workspace(format!(
                    "duplicate site name: '{}'",
                    site.name
                )));
            }
        }

        // Check that each site directory has a seite.toml
        for site in &self.sites {
            let site_toml = ws_root.join(&site.path).join("seite.toml");
            if !site_toml.exists() {
                return Err(PageError::Workspace(format!(
                    "site '{}' has no seite.toml at {}",
                    site.name,
                    site_toml.display()
                )));
            }
        }

        Ok(())
    }

    /// Find a site by name.
    pub fn find_site(&self, name: &str) -> Option<&WorkspaceSite> {
        self.sites.iter().find(|s| s.name == name)
    }

    /// Return the list of sites to operate on, filtered by an optional site name.
    pub fn sites_to_operate(&self, filter: Option<&str>) -> Result<Vec<&WorkspaceSite>> {
        match filter {
            Some(name) => {
                let site = self.find_site(name).ok_or_else(|| {
                    let available: Vec<&str> = self.sites.iter().map(|s| s.name.as_str()).collect();
                    PageError::Workspace(format!(
                        "unknown site '{}'. Available: {}",
                        name,
                        available.join(", ")
                    ))
                })?;
                Ok(vec![site])
            }
            None => Ok(self.sites.iter().collect()),
        }
    }
}

/// Walk up from a starting directory to find `seite-workspace.toml`.
/// Returns the workspace root directory if found.
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(WORKSPACE_FILE).exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Resolve the execution context from the current directory and CLI flags.
pub fn resolve_context(
    config_path: Option<&str>,
    site_filter: Option<String>,
) -> Result<ExecutionContext> {
    let cwd = std::env::current_dir().map_err(PageError::Io)?;

    // Check for workspace first
    if let Some(ws_root) = find_workspace_root(&cwd) {
        let ws_config = WorkspaceConfig::load(&ws_root.join(WORKSPACE_FILE))?;
        return Ok(ExecutionContext::Workspace {
            ws_config,
            ws_root,
            site_filter,
        });
    }

    // Standalone mode
    let config_file = config_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("seite.toml"));
    let config = SiteConfig::load(&config_file)?;
    let paths = config.resolve_paths(&cwd);
    Ok(ExecutionContext::Standalone {
        config: Box::new(config),
        paths,
    })
}

/// Load a site's config and paths within a workspace context.
pub fn load_site_in_workspace(
    ws_root: &Path,
    ws_site: &WorkspaceSite,
) -> Result<(SiteConfig, ResolvedPaths)> {
    let site_root = ws_root.join(&ws_site.path);
    let mut config = SiteConfig::load(&site_root.join("seite.toml"))?;

    // Apply workspace-level overrides
    if let Some(ref base_url) = ws_site.base_url {
        config.site.base_url = base_url.clone();
    }

    let mut paths = config.resolve_paths(&site_root);

    // Override output dir if specified in workspace config
    if let Some(ref output_dir) = ws_site.output_dir {
        paths.output = ws_root.join(output_dir);
    }

    Ok((config, paths))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_workspace_config(sites: Vec<WorkspaceSite>) -> WorkspaceConfig {
        WorkspaceConfig {
            workspace: WorkspaceSection {
                name: "test-ws".into(),
                shared_data: None,
                shared_static: None,
                shared_templates: None,
            },
            sites,
            cross_site: CrossSiteSection::default(),
        }
    }

    fn make_site(name: &str, path: &str) -> WorkspaceSite {
        WorkspaceSite {
            name: name.into(),
            path: path.into(),
            base_url: None,
            output_dir: None,
        }
    }

    #[test]
    fn test_find_site_found() {
        let config = make_workspace_config(vec![
            make_site("blog", "sites/blog"),
            make_site("docs", "sites/docs"),
        ]);
        let site = config.find_site("blog");
        assert!(site.is_some());
        assert_eq!(site.unwrap().name, "blog");
    }

    #[test]
    fn test_find_site_not_found() {
        let config = make_workspace_config(vec![make_site("blog", "sites/blog")]);
        assert!(config.find_site("missing").is_none());
    }

    #[test]
    fn test_sites_to_operate_all() {
        let config = make_workspace_config(vec![
            make_site("blog", "sites/blog"),
            make_site("docs", "sites/docs"),
        ]);
        let sites = config.sites_to_operate(None).unwrap();
        assert_eq!(sites.len(), 2);
    }

    #[test]
    fn test_sites_to_operate_filtered() {
        let config = make_workspace_config(vec![
            make_site("blog", "sites/blog"),
            make_site("docs", "sites/docs"),
        ]);
        let sites = config.sites_to_operate(Some("docs")).unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].name, "docs");
    }

    #[test]
    fn test_sites_to_operate_unknown_filter() {
        let config = make_workspace_config(vec![make_site("blog", "sites/blog")]);
        let err = config.sites_to_operate(Some("nope")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown site 'nope'"));
        assert!(msg.contains("blog"));
    }

    #[test]
    fn test_validate_empty_sites() {
        let config = make_workspace_config(vec![]);
        let err = config.validate(Path::new("/tmp")).unwrap_err();
        assert!(err.to_string().contains("at least one site"));
    }

    #[test]
    fn test_validate_duplicate_names() {
        let config = make_workspace_config(vec![
            make_site("blog", "sites/blog1"),
            make_site("blog", "sites/blog2"),
        ]);
        let err = config.validate(Path::new("/tmp")).unwrap_err();
        assert!(err.to_string().contains("duplicate site name: 'blog'"));
    }

    #[test]
    fn test_validate_missing_seite_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("sites/blog")).unwrap();
        // No seite.toml created
        let config = make_workspace_config(vec![make_site("blog", "sites/blog")]);
        let err = config.validate(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("no seite.toml"));
    }

    #[test]
    fn test_load_nonexistent_file() {
        let err =
            WorkspaceConfig::load(Path::new("/nonexistent/seite-workspace.toml")).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_load_valid_workspace() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Create a workspace toml
        let ws_toml = r#"
[workspace]
name = "my-ws"

[[sites]]
name = "blog"
path = "sites/blog"
"#;
        std::fs::write(tmp.path().join("seite-workspace.toml"), ws_toml).unwrap();
        // Create site dir with seite.toml
        let blog_dir = tmp.path().join("sites/blog");
        std::fs::create_dir_all(&blog_dir).unwrap();
        let site_toml = r#"
[site]
title = "Blog"
description = ""
base_url = "http://localhost:3000"
language = "en"

[[collections]]
name = "posts"
label = "Posts"
directory = "posts"
has_date = true
has_rss = true
listed = true
nested = false
url_prefix = "/posts"
default_template = "post.html"
"#;
        std::fs::write(blog_dir.join("seite.toml"), site_toml).unwrap();

        let config = WorkspaceConfig::load(&tmp.path().join("seite-workspace.toml")).unwrap();
        assert_eq!(config.workspace.name, "my-ws");
        assert_eq!(config.sites.len(), 1);
        assert_eq!(config.sites[0].name, "blog");
    }

    #[test]
    fn test_load_invalid_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("seite-workspace.toml"),
            "not valid [[[ toml",
        )
        .unwrap();
        let err = WorkspaceConfig::load(&tmp.path().join("seite-workspace.toml")).unwrap_err();
        assert!(!err.to_string().is_empty()); // Should have a parse error
    }

    #[test]
    fn test_find_workspace_root_found() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("seite-workspace.toml"), "").unwrap();
        let nested = tmp.path().join("deep/nested");
        std::fs::create_dir_all(&nested).unwrap();

        let root = find_workspace_root(&nested);
        assert!(root.is_some());
        assert_eq!(root.unwrap(), tmp.path());
    }

    #[test]
    fn test_find_workspace_root_not_found() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No seite-workspace.toml anywhere
        let root = find_workspace_root(tmp.path());
        assert!(root.is_none());
    }

    #[test]
    fn test_load_site_in_workspace_applies_overrides() {
        let tmp = tempfile::TempDir::new().unwrap();
        let blog_dir = tmp.path().join("sites/blog");
        std::fs::create_dir_all(&blog_dir).unwrap();
        let site_toml = r#"
[site]
title = "Blog"
description = ""
base_url = "http://localhost:3000"
language = "en"

[[collections]]
name = "posts"
label = "Posts"
directory = "posts"
has_date = true
has_rss = true
listed = true
nested = false
url_prefix = "/posts"
default_template = "post.html"
"#;
        std::fs::write(blog_dir.join("seite.toml"), site_toml).unwrap();

        let ws_site = WorkspaceSite {
            name: "blog".into(),
            path: "sites/blog".into(),
            base_url: Some("https://blog.example.com".into()),
            output_dir: Some("dist/blog".into()),
        };

        let (config, paths) = load_site_in_workspace(tmp.path(), &ws_site).unwrap();
        assert_eq!(config.site.base_url, "https://blog.example.com");
        assert_eq!(paths.output, tmp.path().join("dist/blog"));
    }

    #[test]
    fn test_load_site_in_workspace_no_overrides() {
        let tmp = tempfile::TempDir::new().unwrap();
        let blog_dir = tmp.path().join("sites/blog");
        std::fs::create_dir_all(&blog_dir).unwrap();
        let site_toml = r#"
[site]
title = "Blog"
description = ""
base_url = "http://localhost:3000"
language = "en"

[[collections]]
name = "posts"
label = "Posts"
directory = "posts"
has_date = true
has_rss = true
listed = true
nested = false
url_prefix = "/posts"
default_template = "post.html"
"#;
        std::fs::write(blog_dir.join("seite.toml"), site_toml).unwrap();

        let ws_site = make_site("blog", "sites/blog");
        let (config, paths) = load_site_in_workspace(tmp.path(), &ws_site).unwrap();
        assert_eq!(config.site.base_url, "http://localhost:3000");
        // output should be relative to the site root, not workspace root
        assert_eq!(paths.output, blog_dir.join("dist"));
    }

    #[test]
    fn test_cross_site_section_defaults() {
        let css = CrossSiteSection::default();
        assert!(!css.unified_sitemap);
        assert!(!css.unified_search);
        assert!(!css.cross_site_links);
    }
}
