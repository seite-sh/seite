use std::path::{Path, PathBuf};

use crate::error::{PageError, Result};

/// Bundled themes. Each is a `base.html` Tera template embedded at compile
/// time via include_str!, so the binary ships with all of them: no downloads,
/// works offline, and every theme is versioned with the binary.
///
/// Themes own layout and CSS only. Everything seite must keep correct (the
/// SEO/GEO head, search, language switcher, end-of-body scripts) comes from
/// the `seite/*` partials in `src/templates/seite/`, which every theme
/// includes. Edit the theme files in `src/themes/`; editing one changes its
/// hash, so append the new hash to `LEGACY_THEME_HASHES` in
/// `src/cli/upgrade.rs` (a test fails with instructions until you do).
pub struct Theme {
    pub name: &'static str,
    pub description: &'static str,
    pub base_html: &'static str,
    pub kind: ThemeKind,
    /// A stylesheet the theme links (`static/<path>`) and `apply` writes when
    /// it does not exist yet: `(path under static/, content)`.
    pub stylesheet: Option<(&'static str, &'static str)>,
}

/// How a bundled theme is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// Offered by `seite init` and listed first.
    Starter,
    /// Example looks, applied with `seite theme apply <name>`.
    Gallery,
}

impl ThemeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeKind::Starter => "starter",
            ThemeKind::Gallery => "gallery",
        }
    }
}

/// The starter made for designing your own theme (`seite theme new`).
pub const BLANK: &str = "blank";

/// Where the `blank` starter's stylesheet lives, relative to `static/`.
pub const BLANK_STYLESHEET: &str = "css/site.css";

/// An installed theme loaded from `templates/themes/<name>.tera` on disk.
pub struct InstalledTheme {
    pub name: String,
    pub description: String,
    pub base_html: String,
}

/// Every bundled theme: the starters first, then the gallery.
pub fn all() -> Vec<Theme> {
    vec![
        default(),
        docs(),
        landing(),
        blank(),
        minimal(),
        dark(),
        brutalist(),
        bento(),
        terminal(),
        magazine(),
        academic(),
    ]
}

/// The bundled themes of one kind, in [`all`] order.
pub fn of_kind(kind: ThemeKind) -> Vec<Theme> {
    all().into_iter().filter(|t| t.kind == kind).collect()
}

pub fn by_name(name: &str) -> Option<Theme> {
    all().into_iter().find(|t| t.name == name)
}

/// Discover installed themes from `templates/themes/*.tera` in the given project root.
pub fn installed_themes(project_root: &std::path::Path) -> Vec<InstalledTheme> {
    let themes_dir = project_root.join("templates").join("themes");
    let mut themes = Vec::new();

    let entries = match std::fs::read_dir(&themes_dir) {
        Ok(entries) => entries,
        Err(_) => return themes,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tera") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let description =
            parse_theme_description(&content).unwrap_or_else(|| "Installed theme".to_string());
        themes.push(InstalledTheme {
            name,
            description,
            base_html: content,
        });
    }

    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}

/// Returns true if `name` is a safe theme name: `^[a-z0-9][a-z0-9-]*$`.
///
/// Theme names are joined into filesystem paths (`templates/themes/<name>.tera`),
/// so anything outside this alphabet (path separators, `..`, absolute paths,
/// uppercase, whitespace) is rejected.
pub fn is_valid_theme_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Validate a theme name, returning a descriptive error for unsafe names.
pub fn validate_theme_name(name: &str) -> Result<()> {
    if is_valid_theme_name(name) {
        Ok(())
    } else {
        Err(PageError::Other(format!(
            "invalid theme name '{name}': theme names must match ^[a-z0-9][a-z0-9-]*$ \
             (lowercase letters, digits and hyphens)"
        )))
    }
}

/// Find an installed theme by name in the given project root.
///
/// Returns `None` for names that fail [`is_valid_theme_name`], so a
/// user-supplied name can never escape `templates/themes/`.
pub fn installed_by_name(project_root: &std::path::Path, name: &str) -> Option<InstalledTheme> {
    if !is_valid_theme_name(name) {
        return None;
    }
    let path = project_root
        .join("templates")
        .join("themes")
        .join(format!("{name}.tera"));
    let content = std::fs::read_to_string(&path).ok()?;
    let description =
        parse_theme_description(&content).unwrap_or_else(|| "Installed theme".to_string());
    Some(InstalledTheme {
        name: name.to_string(),
        description,
        base_html: content,
    })
}

/// Where a theme applied by [`apply_theme`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    Bundled,
    Installed,
}

impl ThemeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeSource::Bundled => "bundled",
            ThemeSource::Installed => "installed",
        }
    }
}

/// Result of applying a theme to a project.
#[derive(Debug)]
pub struct AppliedTheme {
    pub name: String,
    pub description: String,
    pub source: ThemeSource,
    /// The `base.html` that was written.
    pub path: PathBuf,
    /// Backup of a customized `base.html` that was replaced, if any.
    pub backup: Option<PathBuf>,
    /// The theme's stylesheet, when this apply created it.
    pub stylesheet: Option<PathBuf>,
}

/// Identify which theme `templates_dir/base.html` currently corresponds to.
///
/// Returns the bundled or installed theme name whose template matches the file
/// (ignoring surrounding whitespace; any released version of a bundled theme
/// counts as that theme), `"default"` when there is no `base.html`
/// (the build falls back to the default theme), or `"custom"` when the file
/// matches no known theme.
pub fn active_theme(project_root: &Path, templates_dir: &Path) -> String {
    let base = match std::fs::read_to_string(templates_dir.join("base.html")) {
        Ok(content) => content,
        Err(_) => return "default".to_string(),
    };
    let current = base.trim();
    if let Some(theme) = all().into_iter().find(|t| t.base_html.trim() == current) {
        return theme.name.to_string();
    }
    if let Some(theme) = installed_themes(project_root)
        .into_iter()
        .find(|t| t.base_html.trim() == current)
    {
        return theme.name;
    }
    // An unedited copy of an older version of a bundled theme is still
    // that theme (`seite upgrade` replaces it with the current version).
    if let Some(name) = crate::cli::upgrade::legacy_bundled_theme(&base) {
        return name.to_string();
    }
    "custom".to_string()
}

/// Apply a bundled or installed theme by writing `templates_dir/base.html`.
///
/// The theme name is validated first (see [`is_valid_theme_name`]). If an
/// existing `base.html` has been customized — i.e. it matches no bundled or
/// installed theme — it is copied to `base.html.bak` (or `base.html.bak.N`
/// if that name is taken) before being replaced, so no work is lost.
///
/// A theme with its own stylesheet (`blank`) also gets `static_dir/<path>`
/// written, unless that file already exists.
pub fn apply_theme(
    project_root: &Path,
    templates_dir: &Path,
    static_dir: &Path,
    name: &str,
) -> Result<AppliedTheme> {
    validate_theme_name(name)?;

    let (html, description, source, stylesheet) = if let Some(theme) = by_name(name) {
        (
            theme.base_html.to_string(),
            theme.description.to_string(),
            ThemeSource::Bundled,
            theme.stylesheet,
        )
    } else if let Some(theme) = installed_by_name(project_root, name) {
        (
            theme.base_html,
            theme.description,
            ThemeSource::Installed,
            None,
        )
    } else {
        let mut available: Vec<String> = all().iter().map(|t| t.name.to_string()).collect();
        available.extend(installed_themes(project_root).into_iter().map(|t| t.name));
        return Err(PageError::Other(format!(
            "unknown theme '{name}'. Available themes: {}",
            available.join(", ")
        )));
    };

    // Never through a symlink: agents may run `seite theme apply` without
    // asking, and a cloned repository could link base.html (or templates/)
    // to a file outside the project.
    let confined = |e: anyhow::Error| PageError::Other(format!("{e:#}"));
    crate::safe_fs::create_dir_all(project_root, templates_dir).map_err(confined)?;
    let base_path = templates_dir.join("base.html");
    crate::safe_fs::guard(project_root, &base_path).map_err(confined)?;
    let backup = if base_path.exists() && active_theme(project_root, templates_dir) == "custom" {
        let backup_path = next_backup_path(&base_path);
        let current = std::fs::read(&base_path)?;
        crate::safe_fs::write(project_root, &backup_path, current).map_err(confined)?;
        Some(backup_path)
    } else {
        None
    };

    crate::safe_fs::write(project_root, &base_path, html).map_err(confined)?;
    let stylesheet = match stylesheet {
        Some((rel, css)) => write_stylesheet(project_root, static_dir, rel, css)?,
        None => None,
    };
    Ok(AppliedTheme {
        name: name.to_string(),
        description,
        source,
        path: base_path,
        backup,
        stylesheet,
    })
}

/// Write a theme's stylesheet to `static_dir/rel` unless it already exists
/// (from then on it is the site's own CSS). Returns the path when written.
fn write_stylesheet(
    project_root: &Path,
    static_dir: &Path,
    rel: &str,
    css: &str,
) -> Result<Option<PathBuf>> {
    let path = static_dir.join(rel);
    if path.exists() {
        return Ok(None);
    }
    crate::safe_fs::write(project_root, &path, css)
        .map_err(|e| PageError::Other(format!("{e:#}")))?;
    Ok(Some(path))
}

/// First free backup path: `base.html.bak`, then `base.html.bak.1`, `.2`, ...
fn next_backup_path(base_path: &Path) -> PathBuf {
    let first = base_path.with_file_name("base.html.bak");
    if !first.exists() {
        return first;
    }
    let mut n = 1;
    loop {
        let candidate = base_path.with_file_name(format!("base.html.bak.{n}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Parse a description from theme metadata comments.
/// Looks for `{#- theme-description: ... -#}` at the top of the file.
fn parse_theme_description(content: &str) -> Option<String> {
    for line in content.lines().take(10) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("{#-") {
            if let Some(rest) = rest.strip_suffix("-#}") {
                let rest = rest.trim();
                if let Some(desc) = rest.strip_prefix("theme-description:") {
                    return Some(desc.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn default() -> Theme {
    Theme {
        name: "default",
        description: "Clean, readable theme with system fonts",
        base_html: include_str!("themes/default.tera"),
        kind: ThemeKind::Starter,
        stylesheet: None,
    }
}

pub fn minimal() -> Theme {
    Theme {
        name: "minimal",
        description: "Ultra-minimal, typography-first theme",
        base_html: include_str!("themes/minimal.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

pub fn dark() -> Theme {
    Theme {
        name: "dark",
        description: "Dark mode theme, easy on the eyes",
        base_html: include_str!("themes/dark.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

pub fn docs() -> Theme {
    Theme {
        name: "docs",
        description: "Documentation-focused theme with sidebar layout",
        base_html: include_str!("themes/docs.tera"),
        kind: ThemeKind::Starter,
        stylesheet: None,
    }
}

pub fn brutalist() -> Theme {
    Theme {
        name: "brutalist",
        description: "Neo-brutalist theme with thick borders and hard shadows",
        base_html: include_str!("themes/brutalist.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

pub fn bento() -> Theme {
    Theme {
        name: "bento",
        description: "Card grid layout inspired by bento box design",
        base_html: include_str!("themes/bento.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

pub fn landing() -> Theme {
    Theme {
        name: "landing",
        description: "Marketing and landing page theme with hero sections and CTAs",
        base_html: include_str!("themes/landing.tera"),
        kind: ThemeKind::Starter,
        stylesheet: None,
    }
}

pub fn terminal() -> Theme {
    Theme {
        name: "terminal",
        description: "Monospace hacker theme with green-on-black terminal aesthetic",
        base_html: include_str!("themes/terminal.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

pub fn magazine() -> Theme {
    Theme {
        name: "magazine",
        description: "Multi-column editorial layout with featured articles",
        base_html: include_str!("themes/magazine.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

pub fn blank() -> Theme {
    Theme {
        name: BLANK,
        description: "Bare semantic layout with its CSS in static/css/site.css, made for designing your own theme",
        base_html: include_str!("themes/blank.tera"),
        kind: ThemeKind::Starter,
        stylesheet: Some((BLANK_STYLESHEET, include_str!("themes/blank.css"))),
    }
}

pub fn academic() -> Theme {
    Theme {
        name: "academic",
        description: "Scholarly serif theme for research and long-form writing",
        base_html: include_str!("themes/academic.tera"),
        kind: ThemeKind::Gallery,
        stylesheet: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_theme_description() {
        let content = "{#- theme-description: A cool dark theme -#}\n<!DOCTYPE html>";
        assert_eq!(
            parse_theme_description(content),
            Some("A cool dark theme".to_string())
        );
    }

    #[test]
    fn test_parse_theme_description_missing() {
        let content = "<!DOCTYPE html>\n<html>";
        assert_eq!(parse_theme_description(content), None);
    }

    #[test]
    fn test_all_themes_have_unique_names() {
        let themes = all();
        let mut names: Vec<_> = themes.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 11);
    }

    #[test]
    fn test_by_name_found() {
        assert!(by_name("dark").is_some());
        assert!(by_name("brutalist").is_some());
    }

    #[test]
    fn test_by_name_not_found() {
        assert!(by_name("nonexistent").is_none());
    }

    #[test]
    fn test_all_themes_have_non_empty_html() {
        for theme in all() {
            assert!(
                !theme.base_html.is_empty(),
                "Theme '{}' has empty HTML",
                theme.name
            );
            assert!(
                theme.base_html.contains("<!DOCTYPE html>")
                    || theme.base_html.contains("<!doctype html>"),
                "Theme '{}' should contain DOCTYPE",
                theme.name
            );
        }
    }

    #[test]
    fn test_all_themes_have_descriptions() {
        for theme in all() {
            assert!(
                !theme.description.is_empty(),
                "Theme '{}' has empty description",
                theme.name
            );
        }
    }

    #[test]
    fn test_all_bundled_theme_names() {
        let themes = all();
        let names: Vec<&str> = themes.iter().map(|t| t.name).collect();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"minimal"));
        assert!(names.contains(&"dark"));
        assert!(names.contains(&"docs"));
        assert!(names.contains(&"brutalist"));
        assert!(names.contains(&"bento"));
        assert!(names.contains(&"landing"));
        assert!(names.contains(&"terminal"));
        assert!(names.contains(&"magazine"));
        assert!(names.contains(&"academic"));
    }

    #[test]
    fn test_installed_themes_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes = installed_themes(tmp.path());
        assert!(themes.is_empty());
    }

    #[test]
    fn test_installed_themes_with_themes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp.path().join("templates").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(
            themes_dir.join("custom.tera"),
            "{#- theme-description: My custom theme -#}\n<!DOCTYPE html><html></html>",
        )
        .unwrap();
        let themes = installed_themes(tmp.path());
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "custom");
        assert_eq!(themes[0].description, "My custom theme");
    }

    #[test]
    fn test_installed_themes_skips_non_tera() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp.path().join("templates").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("readme.txt"), "not a theme").unwrap();
        std::fs::write(themes_dir.join("valid.tera"), "<!DOCTYPE html>").unwrap();
        let themes = installed_themes(tmp.path());
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "valid");
    }

    #[test]
    fn test_installed_by_name_found() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp.path().join("templates").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(
            themes_dir.join("mytest.tera"),
            "{#- theme-description: Test theme -#}\n<html></html>",
        )
        .unwrap();
        let theme = installed_by_name(tmp.path(), "mytest");
        assert!(theme.is_some());
        let theme = theme.unwrap();
        assert_eq!(theme.name, "mytest");
        assert_eq!(theme.description, "Test theme");
    }

    #[test]
    fn test_installed_by_name_not_found() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(installed_by_name(tmp.path(), "missing").is_none());
    }

    #[test]
    fn test_installed_theme_no_description() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp.path().join("templates").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("plain.tera"), "<html></html>").unwrap();
        let theme = installed_by_name(tmp.path(), "plain").unwrap();
        assert_eq!(theme.description, "Installed theme");
    }

    #[test]
    fn test_parse_theme_description_deep_in_file() {
        // Description must be in first 10 lines
        let mut content = String::new();
        for _ in 0..11 {
            content.push_str("<!-- line -->\n");
        }
        content.push_str("{#- theme-description: Too deep -#}");
        assert_eq!(parse_theme_description(&content), None);
    }

    #[test]
    fn test_installed_themes_sorted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp.path().join("templates").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("zebra.tera"), "<html></html>").unwrap();
        std::fs::write(themes_dir.join("alpha.tera"), "<html></html>").unwrap();
        std::fs::write(themes_dir.join("middle.tera"), "<html></html>").unwrap();
        let themes = installed_themes(tmp.path());
        assert_eq!(themes[0].name, "alpha");
        assert_eq!(themes[1].name, "middle");
        assert_eq!(themes[2].name, "zebra");
    }

    #[test]
    fn test_is_valid_theme_name() {
        for ok in ["dark", "my-theme", "a", "0x", "coral-2"] {
            assert!(is_valid_theme_name(ok), "{ok} should be valid");
        }
        for bad in [
            "",
            "-dark",
            "Dark",
            "../evil",
            "/abs/path/evil",
            "a/b",
            "a\\b",
            "a.b",
            "a b",
            "my_theme",
        ] {
            assert!(!is_valid_theme_name(bad), "{bad:?} should be invalid");
            assert!(validate_theme_name(bad).is_err());
        }
    }

    #[test]
    fn test_installed_by_name_rejects_path_escape() {
        let tmp = tempfile::TempDir::new().unwrap();
        // A .tera file outside templates/themes/ must not be reachable.
        let evil = tmp.path().join("evil.tera");
        std::fs::write(&evil, "<html>evil</html>").unwrap();
        let abs = tmp.path().join("evil");
        assert!(installed_by_name(tmp.path(), abs.to_str().unwrap()).is_none());
        assert!(installed_by_name(tmp.path(), "../../evil").is_none());
    }

    #[test]
    fn test_apply_theme_rejects_invalid_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        let err =
            apply_theme(tmp.path(), &tpl, &tmp.path().join("static"), "/etc/evil").unwrap_err();
        assert!(err.to_string().contains("invalid theme name"));
        assert!(!tpl.join("base.html").exists());
    }

    #[test]
    fn test_apply_theme_unknown_lists_available() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        let err = apply_theme(tmp.path(), &tpl, &tmp.path().join("static"), "nope").unwrap_err();
        assert!(err.to_string().contains("unknown theme 'nope'"));
        assert!(err.to_string().contains("dark"));
    }

    #[test]
    fn test_apply_theme_writes_configured_dir_without_backup_for_known_theme() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("my-templates");
        std::fs::create_dir_all(&tpl).unwrap();
        std::fs::write(tpl.join("base.html"), minimal().base_html).unwrap();
        let applied = apply_theme(tmp.path(), &tpl, &tmp.path().join("static"), "dark").unwrap();
        assert_eq!(applied.source, ThemeSource::Bundled);
        assert!(applied.backup.is_none());
        assert_eq!(
            std::fs::read_to_string(tpl.join("base.html")).unwrap(),
            dark().base_html
        );
        assert!(!tmp.path().join("templates/base.html").exists());
    }

    #[test]
    fn test_apply_theme_backs_up_custom_base() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        std::fs::create_dir_all(&tpl).unwrap();
        std::fs::write(tpl.join("base.html"), "<html>custom one</html>").unwrap();
        let applied = apply_theme(tmp.path(), &tpl, &tmp.path().join("static"), "dark").unwrap();
        let backup = applied.backup.unwrap();
        assert_eq!(backup, tpl.join("base.html.bak"));
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "<html>custom one</html>"
        );

        // A second customization gets a fresh backup name instead of clobbering.
        std::fs::write(tpl.join("base.html"), "<html>custom two</html>").unwrap();
        let applied = apply_theme(tmp.path(), &tpl, &tmp.path().join("static"), "minimal").unwrap();
        assert_eq!(applied.backup.unwrap(), tpl.join("base.html.bak.1"));
        assert_eq!(
            std::fs::read_to_string(tpl.join("base.html.bak")).unwrap(),
            "<html>custom one</html>"
        );
    }

    #[test]
    fn test_active_theme_detection() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        assert_eq!(active_theme(tmp.path(), &tpl), "default");
        std::fs::create_dir_all(tpl.join("themes")).unwrap();
        std::fs::write(tpl.join("base.html"), format!("{}\n", dark().base_html)).unwrap();
        assert_eq!(active_theme(tmp.path(), &tpl), "dark");
        std::fs::write(tpl.join("themes/coral.tera"), "<html>coral</html>").unwrap();
        std::fs::write(tpl.join("base.html"), "<html>coral</html>").unwrap();
        assert_eq!(active_theme(tmp.path(), &tpl), "coral");
        std::fs::write(tpl.join("base.html"), "<html>mine</html>").unwrap();
        assert_eq!(active_theme(tmp.path(), &tpl), "custom");
    }
}
