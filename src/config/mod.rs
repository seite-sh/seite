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
    /// Deploy this collection to its own subdomain. When set, the collection gets
    /// its own output directory, base_url, sitemap, RSS, discovery files, and search index.
    /// Value is the subdomain prefix: `"docs"` → `docs.example.com`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdomain: Option<String>,
    /// Explicit base URL for this subdomain collection.
    /// Overrides the auto-derived `{subdomain}.{base_domain}` URL.
    /// Only meaningful when `subdomain` is set.
    /// Example: `"https://docs.example.com"` avoids `www`-prefix issues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdomain_base_url: Option<String>,
    /// Cloudflare/Netlify project name for this subdomain's deploy.
    /// Only used when `subdomain` is set. Falls back to the global `deploy.project`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy_project: Option<String>,
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
            subdomain: None,
            subdomain_base_url: None,
            deploy_project: None,
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
            subdomain: None,
            subdomain_base_url: None,
            deploy_project: None,
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
            subdomain: None,
            subdomain_base_url: None,
            deploy_project: None,
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
            subdomain: None,
            subdomain_base_url: None,
            deploy_project: None,
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
            subdomain: None,
            subdomain_base_url: None,
            deploy_project: None,
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
            subdomain: None,
            subdomain_base_url: None,
            deploy_project: None,
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

impl ResolvedPaths {
    /// Output directory for a subdomain collection (e.g., `{root}/dist-subdomains/docs/`).
    pub fn subdomain_output(&self, collection_name: &str) -> PathBuf {
        self.root.join("dist-subdomains").join(collection_name)
    }
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
        config.validate_subdomains()?;
        Ok(config)
    }

    /// Validate subdomain configuration.
    fn validate_subdomains(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for c in &self.collections {
            if let Some(ref sub) = c.subdomain {
                if !seen.insert(sub.as_str()) {
                    return Err(PageError::ConfigInvalid {
                        message: format!("duplicate subdomain '{sub}' on collection '{}'", c.name),
                    });
                }
            }
            if c.deploy_project.is_some() && c.subdomain.is_none() {
                return Err(PageError::ConfigInvalid {
                    message: format!(
                        "deploy_project on collection '{}' requires subdomain to be set",
                        c.name
                    ),
                });
            }
            if c.subdomain_base_url.is_some() && c.subdomain.is_none() {
                return Err(PageError::ConfigInvalid {
                    message: format!(
                        "subdomain_base_url on collection '{}' requires subdomain to be set",
                        c.name
                    ),
                });
            }
            if let Some(ref url) = c.subdomain_base_url {
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(PageError::ConfigInvalid {
                        message: format!(
                            "subdomain_base_url on collection '{}' must start with http:// or https://",
                            c.name
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    /// Returns collections that have a subdomain configured.
    pub fn subdomain_collections(&self) -> Vec<&CollectionConfig> {
        self.collections
            .iter()
            .filter(|c| c.subdomain.is_some())
            .collect()
    }

    /// Returns collections that do NOT have a subdomain (belong to the main site).
    pub fn main_site_collections(&self) -> Vec<CollectionConfig> {
        self.collections
            .iter()
            .filter(|c| c.subdomain.is_none())
            .cloned()
            .collect()
    }

    /// Whether any collection uses subdomains.
    pub fn has_subdomains(&self) -> bool {
        self.collections.iter().any(|c| c.subdomain.is_some())
    }

    /// Extract the base domain from `base_url` (e.g., `"https://example.com"` → `"example.com"`).
    pub fn base_domain(&self) -> Option<String> {
        let url = self.site.base_url.trim_end_matches('/');
        url.strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .map(|after| after.split('/').next().unwrap_or(after).to_string())
    }

    /// Compute the subdomain base_url for a collection.
    ///
    /// If `collection.subdomain_base_url` is set, returns that (trimmed of trailing slash).
    /// Otherwise derives it from `{subdomain}.{base_domain}`.
    pub fn subdomain_base_url(&self, collection: &CollectionConfig) -> String {
        // Explicit override takes priority
        if let Some(ref explicit) = collection.subdomain_base_url {
            return explicit.trim_end_matches('/').to_string();
        }
        // Fall back to auto-derivation
        let subdomain = collection.subdomain.as_deref().unwrap_or("");
        let scheme = if self.site.base_url.starts_with("https://") {
            "https"
        } else {
            "http"
        };
        if let Some(domain) = self.base_domain() {
            format!("{scheme}://{subdomain}.{domain}")
        } else {
            format!("{scheme}://{subdomain}.localhost")
        }
    }

    /// Build a map of URL prefixes → absolute subdomain URLs for link rewriting.
    ///
    /// For each collection with `subdomain` set, maps its `url_prefix` (e.g., `"/docs"`)
    /// to the subdomain base URL (e.g., `"https://docs.example.com"`). Collections with
    /// empty `url_prefix` are skipped (can't match on empty prefix).
    pub fn subdomain_rewrite_map(&self) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        for c in &self.collections {
            if c.subdomain.is_some() && !c.url_prefix.is_empty() {
                let prefix = if c.url_prefix.starts_with('/') {
                    c.url_prefix.clone()
                } else {
                    format!("/{}", c.url_prefix)
                };
                map.insert(prefix, self.subdomain_base_url(c));
            }
        }
        map
    }

    /// Build a reverse rewrite map for a subdomain site: maps main-site collection
    /// prefixes to their absolute URLs on the main site.
    ///
    /// Used when building subdomain sites so links to other collections resolve
    /// to the main site domain.
    pub fn reverse_subdomain_rewrite_map(
        &self,
        exclude_collection: &str,
    ) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        let base_url = self.site.base_url.trim_end_matches('/');
        for c in &self.collections {
            if c.name == exclude_collection {
                continue;
            }
            // Subdomain collections get their resolved subdomain URL
            if c.subdomain.is_some() {
                if !c.url_prefix.is_empty() {
                    let prefix = if c.url_prefix.starts_with('/') {
                        c.url_prefix.clone()
                    } else {
                        format!("/{}", c.url_prefix)
                    };
                    map.insert(prefix, self.subdomain_base_url(c));
                }
                continue;
            }
            // Main-site collections: map prefix → base_url + prefix
            if !c.url_prefix.is_empty() {
                let prefix = if c.url_prefix.starts_with('/') {
                    c.url_prefix.clone()
                } else {
                    format!("/{}", c.url_prefix)
                };
                map.insert(prefix.clone(), format!("{base_url}{prefix}"));
            }
        }
        map
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(base_url: &str, collections: Vec<CollectionConfig>) -> SiteConfig {
        SiteConfig {
            site: SiteSection {
                title: "Test".into(),
                description: "".into(),
                base_url: base_url.into(),
                language: "en".into(),
                author: "".into(),
            },
            collections,
            build: BuildSection::default(),
            deploy: DeploySection::default(),
            languages: BTreeMap::new(),
            images: None,
            analytics: None,
            trust: None,
            contact: None,
        }
    }

    fn posts_collection() -> CollectionConfig {
        CollectionConfig::preset_posts()
    }

    fn docs_collection_with_subdomain() -> CollectionConfig {
        let mut c = CollectionConfig::preset_docs();
        c.subdomain = Some("docs".into());
        c
    }

    #[test]
    fn test_subdomain_collections_filter() {
        let config = make_config(
            "https://example.com",
            vec![posts_collection(), docs_collection_with_subdomain()],
        );
        let subs = config.subdomain_collections();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].name, "docs");
    }

    #[test]
    fn test_main_site_collections_filter() {
        let config = make_config(
            "https://example.com",
            vec![posts_collection(), docs_collection_with_subdomain()],
        );
        let main = config.main_site_collections();
        assert_eq!(main.len(), 1);
        assert_eq!(main[0].name, "posts");
    }

    #[test]
    fn test_has_subdomains_true() {
        let config = make_config(
            "https://example.com",
            vec![posts_collection(), docs_collection_with_subdomain()],
        );
        assert!(config.has_subdomains());
    }

    #[test]
    fn test_has_subdomains_false() {
        let config = make_config("https://example.com", vec![posts_collection()]);
        assert!(!config.has_subdomains());
    }

    #[test]
    fn test_base_domain_https() {
        let config = make_config("https://example.com", vec![]);
        assert_eq!(config.base_domain(), Some("example.com".into()));
    }

    #[test]
    fn test_base_domain_http_localhost() {
        let config = make_config("http://localhost:3000", vec![]);
        assert_eq!(config.base_domain(), Some("localhost:3000".into()));
    }

    #[test]
    fn test_base_domain_with_path() {
        let config = make_config("https://user.github.io/repo", vec![]);
        assert_eq!(config.base_domain(), Some("user.github.io".into()));
    }

    #[test]
    fn test_subdomain_base_url() {
        let docs = docs_collection_with_subdomain();
        let config = make_config("https://example.com", vec![docs.clone()]);
        assert_eq!(config.subdomain_base_url(&docs), "https://docs.example.com");
    }

    #[test]
    fn test_subdomain_base_url_http() {
        let docs = docs_collection_with_subdomain();
        let config = make_config("http://localhost:3000", vec![docs.clone()]);
        assert_eq!(
            config.subdomain_base_url(&docs),
            "http://docs.localhost:3000"
        );
    }

    #[test]
    fn test_subdomain_base_url_explicit_override() {
        let mut docs = docs_collection_with_subdomain();
        docs.subdomain_base_url = Some("https://docs.example.com".into());
        let config = make_config("https://www.example.com", vec![docs.clone()]);
        assert_eq!(config.subdomain_base_url(&docs), "https://docs.example.com");
    }

    #[test]
    fn test_subdomain_base_url_explicit_strips_trailing_slash() {
        let mut docs = docs_collection_with_subdomain();
        docs.subdomain_base_url = Some("https://docs.example.com/".into());
        let config = make_config("https://example.com", vec![docs.clone()]);
        assert_eq!(config.subdomain_base_url(&docs), "https://docs.example.com");
    }

    #[test]
    fn test_validate_subdomain_base_url_without_subdomain() {
        let mut posts = posts_collection();
        posts.subdomain_base_url = Some("https://blog.example.com".into());
        let config = make_config("https://example.com", vec![posts]);
        let err = config.validate_subdomains().unwrap_err();
        assert!(err.to_string().contains("requires subdomain to be set"));
    }

    #[test]
    fn test_validate_subdomain_base_url_invalid_scheme() {
        let mut docs = docs_collection_with_subdomain();
        docs.subdomain_base_url = Some("ftp://docs.example.com".into());
        let config = make_config("https://example.com", vec![docs]);
        let err = config.validate_subdomains().unwrap_err();
        assert!(err.to_string().contains("must start with http://"));
    }

    #[test]
    fn test_validate_subdomains_duplicate() {
        let mut docs2 = CollectionConfig::preset_pages();
        docs2.subdomain = Some("docs".into());
        let config = make_config(
            "https://example.com",
            vec![docs_collection_with_subdomain(), docs2],
        );
        let err = config.validate_subdomains().unwrap_err();
        assert!(err.to_string().contains("duplicate subdomain 'docs'"));
    }

    #[test]
    fn test_validate_deploy_project_without_subdomain() {
        let mut posts = posts_collection();
        posts.deploy_project = Some("my-project".into());
        let config = make_config("https://example.com", vec![posts]);
        let err = config.validate_subdomains().unwrap_err();
        assert!(err.to_string().contains("requires subdomain"));
    }

    #[test]
    fn test_validate_subdomains_ok() {
        let config = make_config(
            "https://example.com",
            vec![posts_collection(), docs_collection_with_subdomain()],
        );
        assert!(config.validate_subdomains().is_ok());
    }

    #[test]
    fn test_subdomain_config_deserialization() {
        let toml = r#"
[site]
title = "Test"
base_url = "https://example.com"

[[collections]]
name = "docs"
label = "Docs"
directory = "docs"
url_prefix = "/docs"
default_template = "doc.html"
subdomain = "docs"
subdomain_base_url = "https://docs.example.com"
deploy_project = "my-docs"
"#;
        let config: SiteConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.collections[0].subdomain, Some("docs".into()));
        assert_eq!(
            config.collections[0].subdomain_base_url,
            Some("https://docs.example.com".into())
        );
        assert_eq!(config.collections[0].deploy_project, Some("my-docs".into()));
    }

    #[test]
    fn test_subdomain_output_path() {
        let paths = ResolvedPaths {
            root: PathBuf::from("/project"),
            output: PathBuf::from("/project/dist"),
            content: PathBuf::from("/project/content"),
            templates: PathBuf::from("/project/templates"),
            static_dir: PathBuf::from("/project/static"),
            data_dir: PathBuf::from("/project/data"),
            public_dir: PathBuf::from("/project/public"),
        };
        assert_eq!(
            paths.subdomain_output("docs"),
            PathBuf::from("/project/dist-subdomains/docs")
        );
    }
}
