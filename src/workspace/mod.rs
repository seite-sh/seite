pub mod build;
pub mod deploy;
pub mod server;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{ResolvedPaths, SiteConfig};
use crate::error::{PageError, Result};

/// Workspace configuration loaded from `page-workspace.toml`.
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
    /// Override the site's page.toml base_url.
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

const WORKSPACE_FILE: &str = "page-workspace.toml";

impl WorkspaceConfig {
    /// Load workspace config from a `page-workspace.toml` file.
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

        // Check that each site directory has a page.toml
        for site in &self.sites {
            let site_toml = ws_root.join(&site.path).join("page.toml");
            if !site_toml.exists() {
                return Err(PageError::Workspace(format!(
                    "site '{}' has no page.toml at {}",
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

/// Walk up from a starting directory to find `page-workspace.toml`.
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
        .unwrap_or_else(|| PathBuf::from("page.toml"));
    let config = SiteConfig::load(&config_file)?;
    let paths = config.resolve_paths(&cwd);
    Ok(ExecutionContext::Standalone { config: Box::new(config), paths })
}

/// Load a site's config and paths within a workspace context.
pub fn load_site_in_workspace(
    ws_root: &Path,
    ws_site: &WorkspaceSite,
) -> Result<(SiteConfig, ResolvedPaths)> {
    let site_root = ws_root.join(&ws_site.path);
    let mut config = SiteConfig::load(&site_root.join("page.toml"))?;

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
