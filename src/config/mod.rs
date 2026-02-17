pub mod defaults;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PageError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site: SiteSection,
    #[serde(default)]
    pub build: BuildSection,
    #[serde(default)]
    pub deploy: DeploySection,
    #[serde(default)]
    pub ai: AiSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteSection {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "defaults::base_url")]
    pub base_url: String,
    #[serde(default = "defaults::language")]
    pub language: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSection {
    #[serde(default = "defaults::output_dir")]
    pub output_dir: String,
    #[serde(default = "defaults::content_dir")]
    pub content_dir: String,
    #[serde(default = "defaults::template_dir")]
    pub template_dir: String,
    #[serde(default = "defaults::static_dir")]
    pub static_dir: String,
}

impl Default for BuildSection {
    fn default() -> Self {
        Self {
            output_dir: defaults::output_dir(),
            content_dir: defaults::content_dir(),
            template_dir: defaults::template_dir(),
            static_dir: defaults::static_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploySection {
    #[serde(default)]
    pub target: DeployTarget,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
}

impl Default for DeploySection {
    fn default() -> Self {
        Self {
            target: DeployTarget::default(),
            repo: None,
            project: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DeployTarget {
    #[default]
    GithubPages,
    Cloudflare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSection {
    #[serde(default = "defaults::ai_provider")]
    pub default_provider: String,
}

impl Default for AiSection {
    fn default() -> Self {
        Self {
            default_provider: defaults::ai_provider(),
        }
    }
}

/// Resolved absolute paths for the project directories.
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub output: PathBuf,
    pub content: PathBuf,
    pub templates: PathBuf,
    pub static_dir: PathBuf,
}

impl SiteConfig {
    /// Load config from a `page.toml` file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(PageError::ConfigNotFound {
                path: path.to_path_buf(),
            });
        }
        let contents = std::fs::read_to_string(path)?;
        let config: SiteConfig =
            toml::from_str(&contents).map_err(|e| PageError::ConfigInvalid {
                message: e.to_string(),
            })?;
        Ok(config)
    }

    /// Resolve all directory paths relative to the project root.
    pub fn resolve_paths(&self, project_root: &Path) -> ResolvedPaths {
        ResolvedPaths {
            root: project_root.to_path_buf(),
            output: project_root.join(&self.build.output_dir),
            content: project_root.join(&self.build.content_dir),
            templates: project_root.join(&self.build.template_dir),
            static_dir: project_root.join(&self.build.static_dir),
        }
    }
}
