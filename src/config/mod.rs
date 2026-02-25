pub mod defaults;

use std::collections::BTreeMap;
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
    pub languages: BTreeMap<String, LanguageConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analytics: Option<AnalyticsSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<ContactSection>,
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
    /// Number of items per paginated page. None means no pagination.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paginate: Option<usize>,
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
            paginate: None,
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
            paginate: None,
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
            paginate: None,
        }
    }

    pub fn preset_changelog() -> Self {
        Self {
            name: "changelog".into(),
            label: "Changelog".into(),
            directory: "changelog".into(),
            has_date: true,
            has_rss: true,
            listed: true,
            url_prefix: "/changelog".into(),
            nested: false,
            default_template: "changelog-entry.html".into(),
            paginate: None,
        }
    }

    pub fn preset_roadmap() -> Self {
        Self {
            name: "roadmap".into(),
            label: "Roadmap".into(),
            directory: "roadmap".into(),
            has_date: false,
            has_rss: false,
            listed: true,
            url_prefix: "/roadmap".into(),
            nested: false,
            default_template: "roadmap-item.html".into(),
            paginate: None,
        }
    }

    pub fn preset_trust() -> Self {
        Self {
            name: "trust".into(),
            label: "Trust Center".into(),
            directory: "trust".into(),
            has_date: false,
            has_rss: false,
            listed: true,
            url_prefix: "/trust".into(),
            nested: true,
            default_template: "trust-item.html".into(),
            paginate: None,
        }
    }

    pub fn from_preset(name: &str) -> Option<Self> {
        match name {
            "posts" => Some(Self::preset_posts()),
            "docs" => Some(Self::preset_docs()),
            "pages" => Some(Self::preset_pages()),
            "changelog" => Some(Self::preset_changelog()),
            "roadmap" => Some(Self::preset_roadmap()),
            "trust" => Some(Self::preset_trust()),
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
    #[serde(default = "defaults::data_dir")]
    pub data_dir: String,
    #[serde(default = "defaults::public_dir")]
    pub public_dir: String,
    /// Minify CSS and JS files during build. Default: false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub minify: bool,
    /// Add content-hash fingerprints to static filenames and write asset-manifest.json. Default: false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fingerprint: bool,
    /// Enable math/LaTeX rendering ($inline$ and $$display$$ blocks). Default: false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub math: bool,
}

impl Default for BuildSection {
    fn default() -> Self {
        Self {
            output_dir: defaults::output_dir(),
            content_dir: defaults::content_dir(),
            template_dir: defaults::template_dir(),
            static_dir: defaults::static_dir(),
            data_dir: defaults::data_dir(),
            public_dir: defaults::public_dir(),
            minify: false,
            fingerprint: false,
            math: false,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Auto-commit and push before deploying. Default: true.
    #[serde(default = "crate::config::defaults::bool_true")]
    pub auto_commit: bool,
}

impl Default for DeploySection {
    fn default() -> Self {
        Self {
            target: DeployTarget::default(),
            repo: None,
            project: None,
            domain: None,
            auto_commit: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DeployTarget {
    #[default]
    GithubPages,
    Cloudflare,
    Netlify,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSection {
    /// Generate resized copies at these widths (pixels). Default: [480, 800, 1200].
    #[serde(default = "defaults::image_widths")]
    pub widths: Vec<u32>,
    /// JPEG/WebP quality (1-100). Default: 80.
    #[serde(default = "defaults::image_quality")]
    pub quality: u8,
    /// Add loading="lazy" to img tags. Default: true.
    #[serde(default = "defaults::bool_true")]
    pub lazy_loading: bool,
    /// Generate WebP copies alongside originals. Default: true.
    #[serde(default = "defaults::bool_true")]
    pub webp: bool,
    /// Generate AVIF copies alongside originals. Default: false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub avif: bool,
    /// AVIF quality (1-100). Default: 70 (AVIF compresses better than WebP, so lower is OK).
    #[serde(default = "defaults::avif_quality")]
    pub avif_quality: u8,
}

impl Default for ImageSection {
    fn default() -> Self {
        Self {
            widths: defaults::image_widths(),
            quality: defaults::image_quality(),
            lazy_loading: true,
            webp: true,
            avif: false,
            avif_quality: defaults::avif_quality(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AnalyticsProvider {
    Google,
    Gtm,
    Plausible,
    Fathom,
    Umami,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsSection {
    /// Analytics provider: "google", "gtm", "plausible", "fathom", "umami"
    pub provider: AnalyticsProvider,
    /// Measurement/tracking ID (e.g., "G-XXXXXXX", "GTM-XXXXX", site ID)
    pub id: String,
    /// Show a cookie consent banner and gate analytics on user consent. Default: false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cookie_consent: bool,
    /// Custom script URL (required for Umami, optional for others).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_url: Option<String>,
    /// Plausible script extensions (e.g., ["tagged-events", "outbound-links"]).
    /// Appended to the script filename: script.tagged-events.outbound-links.js
    /// Only used when provider is "plausible" and script_url is not set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
}

/// Trust center configuration for compliance hub features.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrustSection {
    /// Company name displayed on the trust center (defaults to site.title).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Active compliance frameworks (e.g., ["soc2", "iso27001"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
}

/// Contact form provider for static site form handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContactProvider {
    Formspree,
    Web3forms,
    Netlify,
    Hubspot,
    Typeform,
}

/// Contact form configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactSection {
    /// Contact form provider.
    pub provider: ContactProvider,
    /// Provider-specific identifier:
    /// - Formspree: form ID (e.g., "xpznqkdl")
    /// - Web3Forms: access key
    /// - Netlify: form name (e.g., "contact")
    /// - HubSpot: "{portalId}/{formGuid}"
    /// - Typeform: form ID (e.g., "abc123XY")
    pub endpoint: String,
    /// HubSpot region (default: "na1"). Set to "eu1" for EU data center.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Custom success/thank-you redirect URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<String>,
    /// Email subject line prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// Resolved absolute paths for the project directories.
#[derive(Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub output: PathBuf,
    pub content: PathBuf,
    pub templates: PathBuf,
    pub static_dir: PathBuf,
    pub data_dir: PathBuf,
    pub public_dir: PathBuf,
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

    /// Load config from a `seite.toml` file.
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

    /// Extract the URL path prefix from `base_url`.
    ///
    /// For GitHub Pages project sites and other subpath deployments:
    /// - `"https://user.github.io/repo"` → `"/repo"`
    /// - `"https://user.github.io/repo/"` → `"/repo"`
    /// - `"https://example.com"` → `""`
    /// - `"https://example.com/"` → `""`
    pub fn base_path(&self) -> String {
        let url = self.site.base_url.trim_end_matches('/');
        if let Some(after_scheme) = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
        {
            if let Some(slash_pos) = after_scheme.find('/') {
                return after_scheme[slash_pos..].to_string();
            }
        }
        String::new()
    }

    /// Resolve all directory paths relative to the project root.
    pub fn resolve_paths(&self, project_root: &Path) -> ResolvedPaths {
        ResolvedPaths {
            root: project_root.to_path_buf(),
            output: project_root.join(&self.build.output_dir),
            content: project_root.join(&self.build.content_dir),
            templates: project_root.join(&self.build.template_dir),
            static_dir: project_root.join(&self.build.static_dir),
            data_dir: project_root.join(&self.build.data_dir),
            public_dir: project_root.join(&self.build.public_dir),
        }
    }
}
