//! Social preview cards (`[og]`): one 1200×630 PNG per page, generated at
//! build time, offline and deterministic.
//!
//! - **Which pages.** Every public, non-draft content page without its own
//!   frontmatter `image:` gets `dist/og/<url-path>.png` (e.g.
//!   `og/posts/hello-world.png`, `og/es/posts/hola.png`). Each language's
//!   homepage gets `og/index.png` / `og/<lang>/index.png`, the *site card*,
//!   which every other page (collection indexes, tag pages, 404, drafts,
//!   private pages) falls back to. Private collections never get their own
//!   card: it would publish their titles. `noindex` pages do get one (they
//!   can still be shared).
//! - **Glyphs.** Text is drawn with the bundled Inter subset (Latin, Greek,
//!   Cyrillic) or `[og] font`. A page whose title has characters neither
//!   font has (CJK, emoji) gets no card of its own — no tofu boxes — and one
//!   `og-missing-glyphs` warning counts them.
//! - **Templates.** `head.html` sets `og:image`/`twitter:image` from
//!   `page.og_image`, else `site.og_image` (the site card), unless the page
//!   sets `image:`.
//! - **Cache.** `.seite/cache/og/<sha256>.png`, keyed by every input (text,
//!   colors, layout, font bytes, logo bytes, seite version,
//!   [`LAYOUT_VERSION`]); unchanged cards are copied, not re-rendered.
//!   Incremental dev-server rebuilds only touch the changed pages' cards.
//!   `seite check` validates and computes URLs but renders nothing.

pub mod card;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::{CollectionConfig, OgSection, ResolvedPaths, SiteConfig};
use crate::content::ContentItem;
use crate::diagnostics::{Diagnostic, Diagnostics};

use card::{CardText, Fonts, Logo, Rgb, Style};

/// Bump when a layout changes, so cached cards are re-rendered even without
/// a seite version bump.
pub const LAYOUT_VERSION: u32 = 1;

/// Cache directory, relative to the site root.
pub const CACHE_DIR: &str = ".seite/cache/og";

/// Card counts for `seite build --json` (`data.og_cards`).
#[derive(Debug, Default, Clone, Serialize)]
pub struct OgStats {
    /// Cards rendered in this build.
    pub rendered: usize,
    /// Cards copied unchanged from `.seite/cache/og/`.
    pub cached: usize,
    /// Pages without a card because their title needs glyphs the font lacks.
    pub missing_glyphs: usize,
}

/// URL path of the card for a page URL: `/` → `/og/index.png`,
/// `/posts/x` → `/og/posts/x.png`, `/docs/` → `/og/docs/index.png`.
pub fn card_path(url: &str) -> String {
    let trimmed = url.trim_matches('/');
    if trimmed.is_empty() {
        "/og/index.png".to_string()
    } else if url.ends_with('/') {
        format!("/og/{trimmed}/index.png")
    } else {
        format!("/og/{trimmed}.png")
    }
}

/// URL path of a language's site card (its homepage card).
pub fn site_card_path(config: &SiteConfig, lang: &str) -> String {
    if lang == config.site.language || !config.is_multilingual() {
        card_path("/")
    } else {
        card_path(&format!("/{lang}/"))
    }
}

/// Default colors (background, foreground, accent) of each bundled theme.
fn theme_palette(theme: &str) -> (Rgb, Rgb, Rgb) {
    let c = |s: &str| Rgb::parse(s).unwrap_or(Rgb(0, 0, 0));
    let (bg, fg, accent) = match theme {
        "minimal" => ("#ffffff", "#222222", "#555555"),
        "dark" => ("#111111", "#e8e8e8", "#8b5cf6"),
        "docs" => ("#ffffff", "#24292e", "#0366d6"),
        "brutalist" => ("#ffe600", "#000000", "#000000"),
        "bento" => ("#ffffff", "#1a1a1a", "#5046e5"),
        "landing" => ("#faf8f5", "#1b1b1f", "#4f46e5"),
        "terminal" => ("#0a0a0a", "#b8e0b4", "#88b884"),
        "magazine" => ("#ffffff", "#111111", "#e63946"),
        "academic" => ("#f7f4ee", "#111111", "#a00000"),
        _ => ("#ffffff", "#1a1a1a", "#0057b7"),
    };
    (c(bg), c(fg), c(accent))
}

/// `data/brand.yaml` colors (`colors.background` / `colors.text` /
/// `colors.primary`), each `None` when missing or not a hex color.
fn brand_palette(data: &serde_json::Value) -> [Option<Rgb>; 3] {
    let colors = &data["brand"]["colors"];
    let get = |k: &str| colors[k].as_str().and_then(Rgb::parse);
    [get("background"), get("text"), get("primary")]
}

/// 1-based line of `key = …` inside the `[og]` table of `seite.toml`, or of
/// the `[og]` header itself.
fn og_line(contents: &str, key: Option<&str>) -> Option<usize> {
    let mut in_og = false;
    let mut header = None;
    for (i, line) in contents.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_og = t == "[og]";
            if in_og {
                header = Some(i + 1);
            }
            continue;
        }
        if in_og {
            if let Some(key) = key {
                if t.strip_prefix(key)
                    .is_some_and(|rest| rest.trim_start().starts_with('='))
                {
                    return Some(i + 1);
                }
            }
        }
    }
    header
}

/// An `og-config-invalid` error located at `key` in `seite.toml`.
fn config_error(root: &Path, key: &str, message: String, hint: &str) -> Diagnostic {
    let mut d = Diagnostic::error("og-config-invalid", message)
        .with_file("seite.toml")
        .with_hint(hint);
    if let Ok(contents) = std::fs::read_to_string(root.join("seite.toml")) {
        if let Some(line) = og_line(&contents, Some(key)) {
            d = d.with_line(line);
        }
    }
    d
}

/// A file named by `[og] logo` / `[og] font`, read and confined to the site
/// root.
fn read_site_file(root: &Path, key: &str, rel: &str) -> Result<Vec<u8>, Box<Diagnostic>> {
    let path = root.join(rel);
    let inside = Path::new(rel).is_relative()
        && !Path::new(rel)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
    if !inside {
        return Err(Box::new(config_error(
            root,
            key,
            format!("[og] {key} = \"{rel}\" must be a path inside the site"),
            "use a path relative to the site root, e.g. \"static/logo.svg\"",
        )));
    }
    std::fs::read(&path).map_err(|e| {
        Box::new(config_error(
            root,
            key,
            format!("[og] {key}: cannot read \"{rel}\": {e}"),
            "the path is relative to the site root (the directory holding seite.toml)",
        ))
    })
}

/// Everything needed to render cards for one build.
pub struct OgPlan {
    root: PathBuf,
    style: Style,
    fonts: Fonts,
    logo: Option<Logo>,
    /// Hash of every input shared by all cards.
    base_hash: Vec<u8>,
    render: bool,
}

/// The page fields a preview card needs.
pub struct PreviewPage<'a> {
    pub collection: &'a CollectionConfig,
    pub title: &'a str,
    pub date: Option<chrono::NaiveDate>,
    pub reading_time: usize,
}

/// What a content page gets.
#[derive(Debug, PartialEq, Eq)]
pub enum PageCard {
    /// Its own card, at this URL path (`/og/...png`).
    Card(String),
    /// No own card (falls back to the site card).
    None,
    /// No own card: the title needs glyphs the font lacks.
    MissingGlyphs,
}

impl OgPlan {
    /// The plan for `config`, `Ok(None)` when cards are off, or the
    /// `og-config-invalid` errors (bad colors, unreadable logo/font).
    pub fn new(
        config: &SiteConfig,
        paths: &ResolvedPaths,
        data: &serde_json::Value,
    ) -> Result<Option<OgPlan>, Diagnostics> {
        let Some(og) = config.og.as_ref().filter(|og| og.generate) else {
            return Ok(None);
        };
        let root = &paths.root;
        let mut errors = Diagnostics::new();
        let theme = crate::themes::active_theme(root, &paths.templates);
        let (theme_bg, theme_fg, theme_accent) = theme_palette(&theme);
        let [brand_bg, brand_fg, brand_accent] = brand_palette(data);
        let mut color = |key: &str, value: &Option<String>, fallback: Rgb| match value {
            None => fallback,
            Some(s) => Rgb::parse(s).unwrap_or_else(|| {
                errors.push(config_error(
                    root,
                    key,
                    format!("[og] {key} = \"{s}\" is not a hex color"),
                    "use #rgb or #rrggbb, e.g. \"#0057b7\"",
                ));
                fallback
            }),
        };
        let style = Style {
            template: og.template,
            background: color("background", &og.background, brand_bg.unwrap_or(theme_bg)),
            foreground: color("foreground", &og.foreground, brand_fg.unwrap_or(theme_fg)),
            accent: color("accent", &og.accent, brand_accent.unwrap_or(theme_accent)),
        };

        let font_bytes = match og.font.as_deref() {
            None => None,
            Some(rel) => match read_site_file(root, "font", rel) {
                Ok(bytes) => Some(bytes),
                Err(d) => {
                    errors.push(*d);
                    None
                }
            },
        };
        let mut hasher = Sha256::new();
        hasher.update(format!(
            "seite-og\0{}\0{LAYOUT_VERSION}\0{}\0{}\0{}\0{}\0",
            env!("CARGO_PKG_VERSION"),
            style.template.as_str(),
            style.background.hex(),
            style.foreground.hex(),
            style.accent.hex()
        ));
        hasher.update(card::BUNDLED_TITLE);
        hasher.update(card::BUNDLED_BODY);
        if let Some(bytes) = &font_bytes {
            hasher.update(b"\0font\0");
            hasher.update(bytes);
        }
        let fonts = match Fonts::load(font_bytes) {
            Ok(fonts) => fonts,
            Err(message) => {
                errors.push(config_error(
                    root,
                    "font",
                    format!(
                        "[og] font = \"{}\": {message}",
                        og.font.as_deref().unwrap_or_default()
                    ),
                    "use a .ttf or .otf file",
                ));
                Fonts::load(None)
                    .map_err(|m| Diagnostics::from(Diagnostic::error("og-config-invalid", m)))?
            }
        };

        let logo = match og.logo.as_deref() {
            None => None,
            Some(rel) => {
                let is_svg = rel.to_ascii_lowercase().ends_with(".svg");
                let is_png = rel.to_ascii_lowercase().ends_with(".png");
                if !is_svg && !is_png {
                    errors.push(config_error(
                        root,
                        "logo",
                        format!("[og] logo = \"{rel}\" must be an .svg or .png file"),
                        "convert the logo to SVG or PNG",
                    ));
                    None
                } else {
                    match read_site_file(root, "logo", rel) {
                        Err(d) => {
                            errors.push(*d);
                            None
                        }
                        Ok(bytes) => match Logo::parse(&bytes, is_svg, &fonts) {
                            Ok(logo) => {
                                hasher.update(if is_svg { b"\0svg\0" } else { b"\0png\0" });
                                hasher.update(&bytes);
                                Some(logo)
                            }
                            Err(message) => {
                                errors.push(config_error(
                                    root,
                                    "logo",
                                    format!("[og] logo = \"{rel}\": {message}"),
                                    "check that the file opens in a browser",
                                ));
                                None
                            }
                        },
                    }
                }
            }
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Some(OgPlan {
            root: root.clone(),
            style,
            fonts,
            logo,
            base_hash: hasher.finalize().to_vec(),
            render: !og.skip_render,
        }))
    }

    /// Whether `item` gets its own card, and where.
    pub fn page_card(
        &self,
        config: &SiteConfig,
        item: &ContentItem,
        collection: &CollectionConfig,
    ) -> PageCard {
        if collection.private || item.frontmatter.draft || item.frontmatter.image.is_some() {
            return PageCard::None;
        }
        // The homepage's content page shares the site card.
        let lang_root = if item.lang == config.site.language || !config.is_multilingual() {
            "/".to_string()
        } else {
            format!("/{}/", item.lang)
        };
        if item.url == lang_root || item.url.split('/').any(|s| s == "..") {
            return PageCard::None;
        }
        if item.frontmatter.title.trim().is_empty() {
            return PageCard::None;
        }
        if !self.fonts.covers(&item.frontmatter.title) {
            return PageCard::MissingGlyphs;
        }
        PageCard::Card(card_path(&item.url))
    }

    /// The text of `item`'s card.
    fn page_text(
        &self,
        config: &SiteConfig,
        item: &ContentItem,
        collection: &CollectionConfig,
    ) -> CardText {
        self.page_text_parts(
            config,
            collection,
            &item.frontmatter.title,
            item.frontmatter.date,
            &item.lang,
            item.reading_time,
        )
    }

    fn page_text_parts(
        &self,
        config: &SiteConfig,
        collection: &CollectionConfig,
        title: &str,
        date: Option<chrono::NaiveDate>,
        lang: &str,
        reading_time: usize,
    ) -> CardText {
        let site = config.title_for_lang(lang);
        let header = if self.fonts.covers(site) {
            site.to_string()
        } else {
            host(config)
        };
        let mut parts: Vec<String> = Vec::new();
        if collection.name != "pages" && self.fonts.covers(&collection.label) {
            parts.push(collection.label.clone());
        }
        if let Some(date) = date {
            parts.push(date.to_string());
        }
        if collection.has_date && reading_time > 0 {
            parts.push(if lang.starts_with("en") {
                format!("{reading_time} min read")
            } else {
                format!("{reading_time} min")
            });
        }
        CardText {
            header,
            title: title.trim().to_string(),
            subtitle: None,
            meta: parts.join("  ·  "),
        }
    }

    /// The text of a language's site card.
    fn site_text(&self, config: &SiteConfig, lang: &str) -> CardText {
        let site = config.title_for_lang(lang);
        let description = config.description_for_lang(lang);
        let host = host(config);
        let (header, title) = if self.fonts.covers(site) && !site.trim().is_empty() {
            (host, site.trim().to_string())
        } else {
            (String::new(), host)
        };
        CardText {
            header,
            title,
            subtitle: (!description.trim().is_empty() && self.fonts.covers(description))
                .then(|| description.trim().to_string()),
            meta: String::new(),
        }
    }

    fn cache_key(&self, text: &CardText) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.base_hash);
        for field in [
            text.header.as_str(),
            text.title.as_str(),
            text.subtitle.as_deref().unwrap_or("\u{1}"),
            text.meta.as_str(),
        ] {
            hasher.update(field.as_bytes());
            hasher.update([0]);
        }
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Render (or copy from the cache) one card to PNG bytes. Returns the
    /// bytes and whether they came from the cache.
    fn card_png(&self, text: &CardText) -> crate::error::Result<(Vec<u8>, bool)> {
        let cache = self
            .root
            .join(CACHE_DIR)
            .join(format!("{}.png", self.cache_key(text)));
        if let Ok(bytes) = std::fs::read(&cache) {
            if bytes.starts_with(b"\x89PNG") {
                return Ok((bytes, true));
            }
        }
        let png = self.render_text(text)?;
        // A cache write failure only costs a re-render next time.
        if let Err(e) = crate::safe_fs::write(&self.root, &cache, &png) {
            tracing::debug!("og cache write failed: {e}");
        }
        Ok((png, false))
    }

    fn render_text(&self, text: &CardText) -> crate::error::Result<Vec<u8>> {
        card::render(text, &self.style, &self.fonts, self.logo.as_ref()).map_err(|message| {
            crate::error::PageError::Build(format!("social card \"{}\": {message}", text.title))
        })
    }

    /// Render one card without touching the cache (`seite og preview`):
    /// a page's card, or with `page: None` the site card of `lang`.
    pub fn preview(
        &self,
        config: &SiteConfig,
        page: Option<PreviewPage<'_>>,
        lang: &str,
    ) -> crate::error::Result<Vec<u8>> {
        let text = match page {
            Some(p) => {
                if !self.fonts.covers(p.title) {
                    return Err(crate::error::PageError::Build(format!(
                        "\"{}\" has characters the card font lacks (e.g. CJK, emoji); set [og] font",
                        p.title
                    )));
                }
                self.page_text_parts(config, p.collection, p.title, p.date, lang, p.reading_time)
            }
            None => self.site_text(config, lang),
        };
        self.render_text(&text)
    }

    /// Write every card into `output`. With `only` (incremental rebuilds),
    /// just the cards of those source files; otherwise also each language's
    /// site card. Returns the counts and the pages skipped for missing glyphs.
    pub fn generate(
        &self,
        config: &SiteConfig,
        collections: &indexmap::IndexMap<String, Vec<ContentItem>>,
        output: &Path,
        only: Option<&HashSet<PathBuf>>,
    ) -> crate::error::Result<(OgStats, Vec<PathBuf>)> {
        let mut jobs: Vec<(String, CardText)> = Vec::new();
        let mut missing = Vec::new();
        if only.is_none() {
            for lang in config.all_languages() {
                jobs.push((site_card_path(config, &lang), self.site_text(config, &lang)));
            }
        }
        for collection in &config.collections {
            let Some(items) = collections.get(&collection.name) else {
                continue;
            };
            for item in items {
                if only.is_some_and(|set| !set.contains(&item.source_path)) {
                    continue;
                }
                match self.page_card(config, item, collection) {
                    PageCard::Card(path) => {
                        jobs.push((path, self.page_text(config, item, collection)))
                    }
                    PageCard::MissingGlyphs => missing.push(item.source_path.clone()),
                    PageCard::None => {}
                }
            }
        }
        let mut stats = OgStats {
            missing_glyphs: missing.len(),
            ..OgStats::default()
        };
        if !self.render {
            return Ok((stats, missing));
        }
        // Two pages can map to one card path only through odd URLs; keep the
        // first so the output is deterministic.
        let mut seen = HashSet::new();
        jobs.retain(|(path, _)| seen.insert(path.clone()));
        let results: Vec<crate::error::Result<bool>> = jobs
            .par_iter()
            .map(|(path, text)| {
                let (png, cached) = self.card_png(text)?;
                let dest = output.join(path.trim_start_matches('/'));
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&dest, png)?;
                Ok(cached)
            })
            .collect();
        for result in results {
            if result? {
                stats.cached += 1;
            } else {
                stats.rendered += 1;
            }
        }
        Ok((stats, missing))
    }
}

/// The site's host (and path) from `base_url`, e.g. `example.com/blog`.
fn host(config: &SiteConfig) -> String {
    let url = config.site.base_url.trim();
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.trim_end_matches('/').to_string()
}

/// Absolute URL of a card path (`/og/...png`) under `base_url`, which already
/// carries any base path.
pub fn absolute(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim_end_matches('/'))
}

/// `og-config-invalid` errors for `config`'s `[og]` section (empty when it is
/// fine or absent). Used by `seite check` via the build.
pub fn validate(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    data: &serde_json::Value,
) -> Diagnostics {
    match OgPlan::new(config, paths, data) {
        Ok(_) => Diagnostics::new(),
        Err(d) => d,
    }
}

/// `[og]` written by `seite init`.
pub fn init_section() -> OgSection {
    OgSection::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_paths_mirror_page_urls() {
        assert_eq!(card_path("/"), "/og/index.png");
        assert_eq!(card_path(""), "/og/index.png");
        assert_eq!(card_path("/posts/hello-world"), "/og/posts/hello-world.png");
        assert_eq!(card_path("/es/posts/hola"), "/og/es/posts/hola.png");
        assert_eq!(card_path("/docs/"), "/og/docs/index.png");
        assert_eq!(card_path("/es/"), "/og/es/index.png");
    }

    #[test]
    fn og_line_finds_keys_in_the_og_table_only() {
        let toml = "[site]\nlogo = \"x\"\n\n[og]\ngenerate = true\nlogo = \"static/l.svg\"\n[build]\nfont = 1\n";
        assert_eq!(og_line(toml, Some("logo")), Some(6));
        assert_eq!(og_line(toml, Some("font")), Some(4));
        assert_eq!(og_line(toml, None), Some(4));
        assert_eq!(og_line("[site]\n", Some("logo")), None);
    }

    #[test]
    fn host_strips_scheme_and_slash() {
        let mut config: SiteConfig = toml::from_str(
            "[site]\ntitle = \"t\"\nbase_url = \"https://example.com/blog/\"\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n",
        )
        .unwrap();
        assert_eq!(host(&config), "example.com/blog");
        config.site.base_url = "http://localhost:3000".into();
        assert_eq!(host(&config), "localhost:3000");
    }

    #[test]
    fn absolute_joins_base_url() {
        assert_eq!(
            absolute("https://x.io/repo/", "/og/index.png"),
            "https://x.io/repo/og/index.png"
        );
    }

    #[test]
    fn palettes() {
        assert_eq!(theme_palette("dark").0, Rgb(0x11, 0x11, 0x11));
        assert_eq!(theme_palette("custom"), theme_palette("default"));
        let data = serde_json::json!({"brand": {"colors": {"primary": "#ff0000", "text": "nope"}}});
        assert_eq!(brand_palette(&data), [None, None, Some(Rgb(255, 0, 0))]);
    }

    fn plan_with(dir: &Path, og: OgSection) -> Result<Option<OgPlan>, Diagnostics> {
        let mut config: SiteConfig = toml::from_str(
            "[site]\ntitle = \"T\"\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\ndefault_template = \"post.html\"\n",
        )
        .unwrap();
        config.og = Some(og);
        let paths = config.resolve_paths(dir);
        OgPlan::new(&config, &paths, &serde_json::json!({}))
    }

    fn key_for(dir: &Path, og: OgSection) -> String {
        let plan = plan_with(dir, og).unwrap().unwrap();
        plan.cache_key(&CardText {
            header: "h".into(),
            title: "t".into(),
            subtitle: None,
            meta: "m".into(),
        })
    }

    #[test]
    fn cache_key_changes_with_every_input() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("seite.toml"), "[og]\n").unwrap();
        std::fs::write(
            dir.path().join("logo.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"10\"/></svg>",
        )
        .unwrap();
        let base = key_for(dir.path(), OgSection::default());
        assert_eq!(base, key_for(dir.path(), OgSection::default()), "stable");
        let variants = [
            OgSection {
                template: crate::config::OgTemplate::Bold,
                ..OgSection::default()
            },
            OgSection {
                background: Some("#123456".into()),
                ..OgSection::default()
            },
            OgSection {
                foreground: Some("#123456".into()),
                ..OgSection::default()
            },
            OgSection {
                accent: Some("#123456".into()),
                ..OgSection::default()
            },
            OgSection {
                logo: Some("logo.svg".into()),
                ..OgSection::default()
            },
        ];
        let mut keys: HashSet<String> = HashSet::from([base.clone()]);
        for og in variants {
            assert!(keys.insert(key_for(dir.path(), og)), "key must change");
        }
        // Logo bytes, not just the path.
        std::fs::write(
            dir.path().join("logo.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><circle r=\"5\"/></svg>",
        )
        .unwrap();
        let og = OgSection {
            logo: Some("logo.svg".into()),
            ..OgSection::default()
        };
        assert!(keys.insert(key_for(dir.path(), og)));
        // Text fields.
        let plan = plan_with(dir.path(), OgSection::default())
            .unwrap()
            .unwrap();
        let t = |h: &str, ti: &str, s: Option<&str>, m: &str| {
            plan.cache_key(&CardText {
                header: h.into(),
                title: ti.into(),
                subtitle: s.map(Into::into),
                meta: m.into(),
            })
        };
        let all = [
            t("h", "t", None, "m"),
            t("x", "t", None, "m"),
            t("h", "x", None, "m"),
            t("h", "t", Some(""), "m"),
            t("h", "t", None, "x"),
            t("ht", "", None, "m"),
        ];
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len());
    }

    #[test]
    fn invalid_config_is_reported_per_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("seite.toml"),
            "[site]\ntitle = \"T\"\n\n[og]\naccent = \"blue\"\nlogo = \"missing.svg\"\nfont = \"static/x.ttf\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("static")).unwrap();
        std::fs::write(dir.path().join("static/x.ttf"), "nope").unwrap();
        let err = plan_with(
            dir.path(),
            OgSection {
                accent: Some("blue".into()),
                logo: Some("missing.svg".into()),
                font: Some("static/x.ttf".into()),
                ..OgSection::default()
            },
        )
        .err()
        .expect("errors");
        let lines: Vec<(String, Option<usize>)> =
            err.iter().map(|d| (d.code.to_string(), d.line)).collect();
        assert_eq!(
            lines,
            vec![
                ("og-config-invalid".to_string(), Some(5)),
                ("og-config-invalid".to_string(), Some(7)),
                ("og-config-invalid".to_string(), Some(6)),
            ]
        );
        assert!(plan_with(
            dir.path(),
            OgSection {
                logo: Some("../outside.svg".into()),
                ..OgSection::default()
            }
        )
        .is_err());
        assert!(plan_with(
            dir.path(),
            OgSection {
                logo: Some("logo.gif".into()),
                ..OgSection::default()
            }
        )
        .is_err());
    }

    #[test]
    fn disabled_section_has_no_plan() {
        let dir = tempfile::tempdir().unwrap();
        assert!(plan_with(
            dir.path(),
            OgSection {
                generate: false,
                ..OgSection::default()
            }
        )
        .unwrap()
        .is_none());
    }
}
