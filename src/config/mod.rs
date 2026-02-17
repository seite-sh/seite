pub mod defaults;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PageError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site: SiteSection,
    pub collections: Vec<CollectionConfig>,
    #[serde(default)]
    pub build: BuildSection,
    #[serde(default)]
    pub deploy: DeploySection,
    #[serde(default)]
    pub languages: HashMap<String, LanguageConfig>,
}

/// Per-language overrides for site metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LanguageConfig {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConfig {
    pub name: String,
    pub label: String,
    pub directory: String,
    #[serde(default)]
    pub has_date: bool,
    #[serde(default)]
    pub has_rss: bool,
    #[serde(default)]
    pub listed: bool,
    #[serde(default)]
    pub url_prefix: String,
    #[serde(default)]
    pub nested: bool,
    pub default_template: String,
}

impl CollectionConfig {
    pub fn preset_posts() -> Self {
        Self {
            name: "posts".into(),
            label: "Posts".into(),
            directory: "posts".into(),
            has_date: true,
            has_rss: true,
            listed: true,
            url_prefix: "/posts".into(),
            nested: false,
            default_template: "post.html".into(),
        }
    }

    pub fn preset_docs() -> Self {
        Self {
            name: "docs".into(),
            label: "Documentation".into(),
            directory: "docs".into(),
            has_date: false,
            has_rss: false,
            listed: true,
            url_prefix: "/docs".into(),
            nested: true,
            default_template: "doc.html".into(),
        }
    }

    pub fn preset_pages() -> Self {
        Self {
            name: "pages".into(),
            label: "Pages".into(),
            directory: "pages".into(),
            has_date: false,
            has_rss: false,
            listed: false,
            url_prefix: "".into(),
            nested: false,
            default_template: "page.html".into(),
        }
    }

    pub fn from_preset(name: &str) -> Option<Self> {
        match name {
            "posts" => Some(Self::preset_posts()),
            "docs" => Some(Self::preset_docs()),
            "pages" => Some(Self::preset_pages()),
            _ => None,
        }
    }
}

/// Find a collection by name, supporting singular→plural normalization.
pub fn find_collection<'a>(
    name: &str,
    collections: &'a [CollectionConfig],
) -> Option<&'a CollectionConfig> {
    let normalized = match name {
        "post" => "posts",
        "doc" => "docs",
        "page" => "pages",
        other => other,
    };
    collections.iter().find(|c| c.name == normalized)
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

/// Resolved absolute paths for the project directories.
#[derive(Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub output: PathBuf,
    pub content: PathBuf,
    pub templates: PathBuf,
    pub static_dir: PathBuf,
}

impl SiteConfig {
    /// Returns true if the site has any non-default languages configured.
    pub fn is_multilingual(&self) -> bool {
        !self.languages.is_empty()
    }

    /// All language codes: default language first, then configured extras.
    pub fn all_languages(&self) -> Vec<String> {
        let mut langs = vec![self.site.language.clone()];
        for key in self.languages.keys() {
            if *key != self.site.language {
                langs.push(key.clone());
            }
        }
        langs
    }

    /// The set of configured non-default language codes (for filename detection).
    pub fn configured_lang_codes(&self) -> std::collections::HashSet<&str> {
        self.languages.keys().map(|s| s.as_str()).collect()
    }

    /// Get the site title for a specific language, falling back to the default.
    pub fn title_for_lang(&self, lang: &str) -> &str {
        if lang == self.site.language {
            &self.site.title
        } else {
            self.languages
                .get(lang)
                .and_then(|l| l.title.as_deref())
                .unwrap_or(&self.site.title)
        }
    }

    /// Get the site description for a specific language, falling back to the default.
    pub fn description_for_lang(&self, lang: &str) -> &str {
        if lang == self.site.language {
            &self.site.description
        } else {
            self.languages
                .get(lang)
                .and_then(|l| l.description.as_deref())
                .unwrap_or(&self.site.description)
        }
    }

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
