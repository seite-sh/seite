use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::config::SiteConfig;
use crate::output::human;
use crate::themes;

#[derive(Args)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand)]
pub enum ThemeCommand {
    /// List available themes (bundled and installed)
    List,

    /// Apply a theme to the current site
    Apply {
        /// Theme name (bundled or installed)
        name: String,
    },

    /// Design a custom theme with your coding agent (or point you to
    /// impeccable when it is installed)
    Create {
        /// Design description for the theme (e.g. "dark glassmorphism with teal accents")
        prompt: String,
    },

    /// Install a theme from a URL
    Install {
        /// URL to a .tera theme file
        url: String,

        /// Name for the installed theme (defaults to filename)
        #[arg(long)]
        name: Option<String>,
    },

    /// Export the current theme as a shareable .tera file
    Export {
        /// Name for the exported theme file
        name: String,

        /// Optional description for the theme
        #[arg(long)]
        description: Option<String>,
    },
}

pub fn run(args: &ThemeArgs) -> anyhow::Result<()> {
    match &args.command {
        ThemeCommand::List => {
            run_list()?;
        }
        ThemeCommand::Apply { name } => {
            run_apply(name)?;
        }
        ThemeCommand::Create { prompt } => {
            run_create(prompt)?;
        }
        ThemeCommand::Install { url, name } => {
            run_install(url, name.as_deref())?;
        }
        ThemeCommand::Export { name, description } => {
            run_export(name, description.as_deref())?;
        }
    }
    Ok(())
}

fn run_list() -> anyhow::Result<()> {
    human::header("Bundled themes");
    // Find the longest theme name for alignment
    let all_themes = themes::all();
    let max_len = all_themes.iter().map(|t| t.name.len()).max().unwrap_or(0);
    for theme in &all_themes {
        crate::human_println!(
            "  {}  {}",
            console::style(format!("{:<max_len$}", theme.name)).bold(),
            theme.description,
        );
    }
    crate::human_println!();
    crate::human_println!(
        "  {} {}",
        console::style("Preview all:").dim(),
        console::style("https://seite.sh/docs/theme-gallery")
            .dim()
            .underlined()
    );

    let project_root = PathBuf::from(".");
    let installed = themes::installed_themes(&project_root);
    crate::output::json::set_data(serde_json::json!({
        "bundled": all_themes
            .iter()
            .map(|t| serde_json::json!({ "name": t.name, "description": t.description }))
            .collect::<Vec<_>>(),
        "installed": installed
            .iter()
            .map(|t| serde_json::json!({ "name": t.name, "description": t.description }))
            .collect::<Vec<_>>(),
    }));
    if !installed.is_empty() {
        crate::human_println!();
        human::header("Installed themes");
        for theme in &installed {
            crate::human_println!(
                "  {} - {}",
                console::style(&theme.name).bold().cyan(),
                theme.description
            );
        }
    }

    Ok(())
}

/// JSON envelope data for `theme apply`.
fn set_apply_data(name: &str, source: &str, template_dir: &std::path::Path) {
    crate::output::json::set_data(serde_json::json!({
        "theme": name,
        "source": source,
        "written": template_dir.join("base.html").display().to_string(),
    }));
}

fn run_apply(name: &str) -> anyhow::Result<()> {
    // Ensure we're in a page project
    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    themes::validate_theme_name(name)?;

    let project_root = std::env::current_dir()?;
    let paths = config.resolve_paths(&project_root);

    if themes::by_name(name).is_some() || themes::installed_by_name(&project_root, name).is_some() {
        let applied = themes::apply_theme(&project_root, &paths.templates, name)?;
        if let Some(ref backup) = applied.backup {
            human::info(&format!(
                "Backed up your customized base.html to {}",
                backup.display()
            ));
        }
        human::success(&format!(
            "Applied {} theme '{}'",
            applied.source.as_str(),
            name
        ));
        human::info("Run 'seite build' or the watcher will pick it up automatically.");
        set_apply_data(name, applied.source.as_str(), &paths.templates);
        return Ok(());
    }

    let available: Vec<&str> = themes::all().iter().map(|t| t.name).collect();
    let hint = human::suggest_match(name, &available);
    Err(anyhow::anyhow!(
        "unknown theme '{}'. Run 'seite theme list' to see available themes{}",
        name,
        hint
    ))
}

/// What `seite theme create` does.
#[derive(Debug, PartialEq, Eq)]
enum CreatePlan {
    /// impeccable is installed: print how to run it (design decisions are
    /// its job; seite's template rules load automatically).
    Impeccable { lines: Vec<String> },
    /// Run the agent one-shot with the `seite` skill's theme workflow.
    RunAgent { prompt: String },
}

/// Whether the impeccable design skill is installed in the project.
fn impeccable_installed(root: &std::path::Path) -> bool {
    [".claude/skills/impeccable", ".agents/skills/impeccable"]
        .iter()
        .any(|dir| root.join(dir).is_dir())
}

fn create_plan(
    root: &std::path::Path,
    description: &str,
    harness: crate::cli::agent::Harness,
) -> CreatePlan {
    if impeccable_installed(root) {
        return CreatePlan::Impeccable {
            lines: vec![
                format!("impeccable is installed: in your agent, run `/impeccable {description}` (Codex: `$impeccable {description}`)."),
                "seite's template rules load automatically when the agent edits templates/, and `/seite theme` has the base.html contract.".into(),
                "Afterwards: `seite check`, then preview with `seite serve`.".into(),
            ],
        };
    }
    CreatePlan::RunAgent {
        prompt: harness.skill_prompt(&format!("theme {description}")),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_create(description: &str) -> anyhow::Result<()> {
    SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let root = std::env::current_dir()?;
    let harness = crate::cli::agent::resolve_harness(None)?;
    match create_plan(&root, description, harness) {
        CreatePlan::Impeccable { lines } => {
            for line in lines {
                human::info(&line);
            }
            crate::output::json::set_data(serde_json::json!({
                "mode": "impeccable",
                "description": description,
            }));
        }
        CreatePlan::RunAgent { prompt } => {
            crate::cli::agent::ensure_installed(harness)?;
            human::info(&format!("Designing theme: \"{description}\""));
            crate::cli::agent::run_once(harness, &prompt)?;
            human::info(
                "For deeper design work, add impeccable: `seite skill install impeccable`.",
            );
            crate::output::json::set_data(serde_json::json!({
                "mode": "agent",
                "agent": harness.name(),
                "prompt": prompt,
            }));
        }
    }
    Ok(())
}

/// Derive a theme name from a URL or explicit override.
fn derive_theme_name(url: &str, name_override: Option<&str>) -> String {
    match name_override {
        Some(n) => n.to_string(),
        None => {
            let url_path = url.rsplit('/').next().unwrap_or("theme");
            url_path
                .strip_suffix(".tera")
                .or_else(|| url_path.strip_suffix(".html"))
                .unwrap_or(url_path)
                .to_string()
        }
    }
}

/// Validate that a theme name is safe for use as a filename.
fn validate_theme_name(name: &str) -> anyhow::Result<()> {
    // Same rule as `seite theme apply` / the MCP server, so every installed
    // theme can later be applied by name.
    themes::validate_theme_name(name)?;
    Ok(())
}

/// Validate that downloaded content looks like a Tera/HTML template.
fn validate_template_content(body: &str) -> anyhow::Result<()> {
    if !body.contains("<!DOCTYPE") && !body.contains("<!doctype") && !body.contains("<html") {
        return Err(anyhow::anyhow!(
            "downloaded file doesn't look like an HTML/Tera template"
        ));
    }
    Ok(())
}

/// Save a downloaded theme to the themes directory under `root`.
fn save_installed_theme(
    root: &std::path::Path,
    theme_name: &str,
    body: &str,
) -> anyhow::Result<PathBuf> {
    let themes_dir = root.join("templates").join("themes");
    std::fs::create_dir_all(&themes_dir)?;

    let dest = themes_dir.join(format!("{theme_name}.tera"));
    std::fs::write(&dest, body)?;
    Ok(dest)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install(url: &str, name_override: Option<&str>) -> anyhow::Result<()> {
    let _config = SiteConfig::load(&PathBuf::from("seite.toml"))?;

    let theme_name = derive_theme_name(url, name_override);
    validate_theme_name(&theme_name)?;

    // Warn if it would shadow a bundled theme
    if themes::by_name(&theme_name).is_some() {
        human::info(&format!(
            "Note: '{}' shadows a bundled theme. Use a different --name to avoid confusion.",
            theme_name
        ));
    }

    human::info(&format!("Downloading theme from {}...", url));

    let body = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("failed to download theme: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow::anyhow!("failed to read theme response: {e}"))?;

    validate_template_content(&body)?;
    let dest = save_installed_theme(std::path::Path::new("."), &theme_name, &body)?;

    human::success(&format!(
        "Installed theme '{}' to {}",
        theme_name,
        dest.display()
    ));
    human::info(&format!("Apply it with: seite theme apply {}", theme_name));

    Ok(())
}

fn run_export(name: &str, description: Option<&str>) -> anyhow::Result<()> {
    let _config = SiteConfig::load(&PathBuf::from("seite.toml"))?;

    let base_path = PathBuf::from("templates").join("base.html");
    if !base_path.exists() {
        return Err(anyhow::anyhow!(
            "no templates/base.html found. Apply or create a theme first."
        ));
    }

    validate_theme_name(name)?;

    let content = std::fs::read_to_string(&base_path)?;
    let exported = build_export_content(&content, description);

    let themes_dir = PathBuf::from("templates").join("themes");
    std::fs::create_dir_all(&themes_dir)?;

    let dest = themes_dir.join(format!("{name}.tera"));
    if dest.exists() {
        return Err(anyhow::anyhow!(
            "theme '{}' already exists at {}. Remove it first or choose a different name.",
            name,
            dest.display()
        ));
    }

    std::fs::write(&dest, exported)?;

    human::success(&format!("Exported theme '{}' to {}", name, dest.display()));
    human::info("Share this .tera file — others can install it with: seite theme install <url>");

    Ok(())
}

/// Build exported theme content with metadata header.
fn build_export_content(content: &str, description: Option<&str>) -> String {
    let desc = description.unwrap_or("Custom theme");
    format!("{{#- theme-description: {} -#}}\n{}", desc, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- derive_theme_name ---

    #[test]
    fn test_derive_name_from_tera_url() {
        assert_eq!(
            derive_theme_name("https://example.com/themes/coral.tera", None),
            "coral"
        );
    }

    #[test]
    fn test_derive_name_from_html_url() {
        assert_eq!(
            derive_theme_name("https://example.com/themes/dark.html", None),
            "dark"
        );
    }

    #[test]
    fn test_derive_name_from_url_no_extension() {
        assert_eq!(
            derive_theme_name("https://example.com/themes/neon", None),
            "neon"
        );
    }

    #[test]
    fn test_derive_name_override_takes_precedence() {
        assert_eq!(
            derive_theme_name("https://example.com/themes/coral.tera", Some("my-theme")),
            "my-theme"
        );
    }

    #[test]
    fn test_derive_name_url_with_query_string() {
        // rsplit('/') gets the last segment including query params
        assert_eq!(
            derive_theme_name("https://example.com/theme.tera?v=2", None),
            "theme.tera?v=2"
        );
    }

    #[test]
    fn test_derive_name_bare_domain() {
        // No path segments after domain
        assert_eq!(
            derive_theme_name("https://example.com", None),
            "example.com"
        );
    }

    // --- validate_theme_name ---

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_theme_name("coral-brutalist").is_ok());
        assert!(validate_theme_name("a").is_ok());
        assert!(validate_theme_name("my_theme").is_err());
        assert!(validate_theme_name("/abs/evil").is_err());
    }

    #[test]
    fn test_validate_name_empty() {
        let err = validate_theme_name("").unwrap_err();
        assert!(err.to_string().contains("invalid theme name"));
    }

    #[test]
    fn test_validate_name_with_separator() {
        let err = validate_theme_name("foo/bar").unwrap_err();
        assert!(err.to_string().contains("invalid theme name"));
    }

    #[test]
    fn test_validate_name_with_dot_dot() {
        let err = validate_theme_name("..sneaky").unwrap_err();
        assert!(err.to_string().contains("invalid theme name"));
    }

    // --- validate_template_content ---

    #[test]
    fn test_validate_content_doctype() {
        assert!(validate_template_content("<!DOCTYPE html><html></html>").is_ok());
    }

    #[test]
    fn test_validate_content_doctype_lowercase() {
        assert!(validate_template_content("<!doctype html><html></html>").is_ok());
    }

    #[test]
    fn test_validate_content_html_tag_only() {
        assert!(validate_template_content("<html lang=\"en\"><body></body></html>").is_ok());
    }

    #[test]
    fn test_validate_content_not_html() {
        let err = validate_template_content("just some random text").unwrap_err();
        assert!(err.to_string().contains("doesn't look like"));
    }

    #[test]
    fn test_validate_content_empty() {
        let err = validate_template_content("").unwrap_err();
        assert!(err.to_string().contains("doesn't look like"));
    }

    // --- build_export_content ---

    #[test]
    fn test_export_content_with_description() {
        let result = build_export_content("<html></html>", Some("My dark theme"));
        assert!(result.starts_with("{#- theme-description: My dark theme -#}\n"));
        assert!(result.contains("<html></html>"));
    }

    #[test]
    fn test_export_content_default_description() {
        let result = build_export_content("<html></html>", None);
        assert!(result.starts_with("{#- theme-description: Custom theme -#}\n"));
    }

    // --- save_installed_theme ---

    #[test]
    fn test_save_installed_theme() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = save_installed_theme(tmp.path(), "test-theme", "<html>content</html>").unwrap();
        assert!(dest.exists());
        assert_eq!(dest.file_name().unwrap(), "test-theme.tera");
        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "<html>content</html>");
    }

    #[test]
    fn test_save_installed_theme_creates_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(!tmp.path().join("templates").exists());
        let dest = save_installed_theme(tmp.path(), "new-theme", "<html></html>").unwrap();
        assert!(dest.exists());
        assert!(tmp.path().join("templates/themes").is_dir());
    }

    // --- theme create ---

    use crate::cli::agent::Harness;

    #[test]
    fn test_create_plan_with_impeccable_installed() {
        for dir in [".claude/skills/impeccable", ".agents/skills/impeccable"] {
            let tmp = tempfile::TempDir::new().unwrap();
            assert!(!impeccable_installed(tmp.path()));
            std::fs::create_dir_all(tmp.path().join(dir)).unwrap();
            assert!(impeccable_installed(tmp.path()));
            match create_plan(tmp.path(), "coral brutalist", Harness::Claude) {
                CreatePlan::Impeccable { lines } => {
                    let text = lines.join("\n");
                    assert!(text.contains("/impeccable coral brutalist"), "{text}");
                    assert!(text.contains("$impeccable"), "{text}");
                    assert!(text.contains("templates"), "{text}");
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn test_create_plan_without_impeccable_runs_the_seite_skill() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(
            create_plan(tmp.path(), "dark teal", Harness::Claude),
            CreatePlan::RunAgent {
                prompt: "/seite theme dark teal".into()
            }
        );
        assert_eq!(
            create_plan(tmp.path(), "dark teal", Harness::Codex),
            CreatePlan::RunAgent {
                prompt: "$seite theme dark teal".into()
            }
        );
    }
}
