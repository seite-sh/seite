use std::path::{Path, PathBuf};

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
    /// List available themes: starters, the gallery of examples, installed
    List,

    /// Start a custom design: scaffold the blank starter (templates/base.html
    /// plus static/css/site.css) and print how to design it with your agent
    New {
        /// Name for your theme (used when you export it later)
        name: Option<String>,

        /// Replace a customized base.html (it is backed up first)
        #[arg(long)]
        force: bool,

        /// Start a Tailwind CSS theme: add [build.tailwind] to seite.toml, write
        /// the entry CSS (static/css/tailwind.css) and link the compiled
        /// stylesheet from base.html
        #[arg(long)]
        tailwind: bool,
    },

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
        ThemeCommand::New {
            name,
            force,
            tailwind,
        } => {
            run_new(name.as_deref(), *force, *tailwind)?;
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

fn theme_json(t: &themes::Theme) -> serde_json::Value {
    serde_json::json!({ "name": t.name, "description": t.description, "kind": t.kind.as_str() })
}

fn print_theme_group(title: &str, group: &[themes::Theme], max_len: usize) {
    human::header(title);
    for theme in group {
        crate::human_println!(
            "  {}  {}",
            console::style(format!("{:<max_len$}", theme.name)).bold(),
            theme.description,
        );
    }
}

fn run_list() -> anyhow::Result<()> {
    let all_themes = themes::all();
    let max_len = all_themes.iter().map(|t| t.name.len()).max().unwrap_or(0);
    let starters = themes::of_kind(themes::ThemeKind::Starter);
    let gallery = themes::of_kind(themes::ThemeKind::Gallery);
    print_theme_group("Starters", &starters, max_len);
    crate::human_println!();
    print_theme_group(
        "Gallery (examples: seite theme apply <name>)",
        &gallery,
        max_len,
    );
    crate::human_println!();
    crate::human_println!(
        "  {}",
        console::style(
            "Bundled themes are examples and starting points. For your own design: seite theme new"
        )
        .dim()
    );
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
        "starters": starters.iter().map(theme_json).collect::<Vec<_>>(),
        "gallery": gallery.iter().map(theme_json).collect::<Vec<_>>(),
        "bundled": all_themes.iter().map(theme_json).collect::<Vec<_>>(),
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
        let applied =
            themes::apply_theme(&project_root, &paths.templates, &paths.static_dir, name)?;
        if let Some(ref backup) = applied.backup {
            human::info(&format!(
                "Backed up your customized base.html to {}",
                backup.display()
            ));
        }
        if let Some(ref css) = applied.stylesheet {
            human::info(&format!(
                "Wrote the theme's stylesheet to {}",
                css.display()
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
    Impeccable {
        scaffold_blank: bool,
        lines: Vec<String>,
    },
    /// Run the agent one-shot with the `seite` skill's theme workflow.
    RunAgent {
        scaffold_blank: bool,
        prompt: String,
    },
}

/// Whether the impeccable design skill is installed in the project.
fn impeccable_installed(root: &Path) -> bool {
    crate::cli::skill::impeccable_installed(root)
}

/// Whether `active` (from [`themes::active_theme`]) is one of the bundled
/// example looks. A custom design starts from the blank starter instead of
/// being painted over one of them.
fn is_bundled_example(active: &str) -> bool {
    active != themes::BLANK && themes::by_name(active).is_some()
}

/// How to design the theme with impeccable, including `/impeccable init`
/// when the project has no PRODUCT.md yet.
fn impeccable_lines(root: &Path, description: Option<&str>) -> Vec<String> {
    let design = description.unwrap_or("<describe your design>");
    let mut lines = Vec::new();
    if !root.join("PRODUCT.md").exists() {
        lines.push("impeccable is installed: in your agent, run `/impeccable init` first (it writes PRODUCT.md, the site brief).".to_string());
    }
    lines.push(format!(
        "Design with `/impeccable {design}` (Codex: `$impeccable {design}`). It edits templates/base.html and static/css/site.css; keep the `{{% include \"seite/...\" %}}` lines."
    ));
    lines.push("seite's template rules load automatically when the agent edits templates/, and `/seite theme` has the base.html contract.".into());
    lines.push(
        "Afterwards: `seite check`, `seite build`, then one `impeccable detect dist/`; preview with `seite serve`."
            .into(),
    );
    lines
}

fn create_plan(
    root: &Path,
    templates_dir: &Path,
    description: &str,
    harness: crate::cli::agent::Harness,
) -> CreatePlan {
    let scaffold_blank = is_bundled_example(&themes::active_theme(root, templates_dir));
    if impeccable_installed(root) {
        return CreatePlan::Impeccable {
            scaffold_blank,
            lines: impeccable_lines(root, Some(description)),
        };
    }
    CreatePlan::RunAgent {
        scaffold_blank,
        prompt: harness.skill_prompt(&format!("theme {description}")),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_create(description: &str) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let root = std::env::current_dir()?;
    let paths = config.resolve_paths(&root);
    let harness = crate::cli::agent::resolve_harness(None)?;
    let plan = create_plan(&root, &paths.templates, description, harness);
    let scaffold = match &plan {
        CreatePlan::Impeccable { scaffold_blank, .. }
        | CreatePlan::RunAgent { scaffold_blank, .. } => *scaffold_blank,
    };
    // Everything that can refuse runs before any file is written.
    if matches!(plan, CreatePlan::RunAgent { .. }) {
        crate::cli::agent::refuse_nested(&format!(
            "Run `/seite theme {description}` directly in this session (Codex: `$seite theme {description}`)."
        ))?;
        crate::cli::agent::ensure_installed(harness)?;
    }
    if scaffold {
        let applied = scaffold_blank(&root, &paths, false)?;
        human::info(&format!(
            "Started from the blank starter: {} (CSS in static/{}), so the design is not painted over an example theme.",
            applied.path.display(),
            themes::BLANK_STYLESHEET
        ));
    }
    match plan {
        CreatePlan::Impeccable { lines, .. } => {
            for line in lines {
                human::info(&line);
            }
            crate::output::json::set_data(serde_json::json!({
                "mode": "impeccable",
                "description": description,
                "scaffolded_blank": scaffold,
            }));
        }
        CreatePlan::RunAgent { prompt, .. } => {
            human::info(&format!("Designing theme: \"{description}\""));
            crate::cli::agent::run_once(harness, &prompt, false)?;
            human::info(
                "For deeper design work, add impeccable: `seite skill install impeccable`.",
            );
            crate::output::json::set_data(serde_json::json!({
                "mode": "agent",
                "agent": harness.name(),
                "prompt": prompt,
                "scaffolded_blank": scaffold,
            }));
        }
    }
    Ok(())
}

/// Scaffold the blank starter into `templates/base.html` and
/// `static/css/site.css` (an existing stylesheet is kept). A customized
/// `base.html` is replaced only with `force`, after a backup (as `apply`).
/// With impeccable installed, also point its live mode at the template.
fn scaffold_blank(
    root: &Path,
    paths: &crate::config::ResolvedPaths,
    force: bool,
) -> anyhow::Result<themes::AppliedTheme> {
    if !force && themes::active_theme(root, &paths.templates) == "custom" {
        anyhow::bail!(
            "{} is customized; `seite theme new --force` replaces it (after a backup), or keep it with `seite theme export <name>` first",
            paths.templates.join("base.html").display()
        );
    }
    let applied = themes::apply_theme(root, &paths.templates, &paths.static_dir, themes::BLANK)?;
    if impeccable_installed(root) {
        configure_impeccable_live(root, &paths.templates)?;
    }
    Ok(applied)
}

/// Next steps printed by `seite theme new`.
fn new_theme_next_steps(root: &Path, name: &str) -> Vec<String> {
    let mut lines = vec![
        "Layout lives in templates/base.html, the look in static/css/site.css. Keep the `{% include \"seite/...\" %}` lines: they carry SEO, search and i18n.".to_string(),
    ];
    if impeccable_installed(root) {
        lines.extend(impeccable_lines(root, None));
    } else {
        lines.push(
            "Design it with your agent: `/seite theme <description>` (Codex: `$seite theme <description>`), or `seite theme create \"<description>\"`."
                .into(),
        );
        lines.push("For deeper design tooling: `seite skill install impeccable`.".into());
        lines.push("Afterwards: `seite check`; preview with `seite serve`.".into());
    }
    lines.push(format!(
        "To reuse it elsewhere: `seite theme export {name}`."
    ));
    lines
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_new(name: Option<&str>, force: bool, tailwind: bool) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let root = std::env::current_dir()?;
    let paths = config.resolve_paths(&root);
    let name = name.unwrap_or("my-theme");
    themes::validate_theme_name(name)?;
    let applied = scaffold_blank(&root, &paths, force)?;
    let tailwind = if tailwind {
        Some(scaffold_tailwind(&root, &config, &paths)?)
    } else {
        None
    };
    if let Some(ref backup) = applied.backup {
        human::info(&format!(
            "Backed up your customized base.html to {}",
            backup.display()
        ));
    }
    human::success(&format!(
        "Scaffolded the blank starter for '{name}': {}{}",
        applied.path.display(),
        applied
            .stylesheet
            .as_ref()
            .map(|css| format!(" and {}", css.display()))
            .unwrap_or_default()
    ));
    for line in tailwind.iter().flat_map(|t| &t.notes) {
        human::info(line);
    }
    for line in new_theme_next_steps(&root, name) {
        human::info(&line);
    }
    crate::output::json::set_data(serde_json::json!({
        "theme": name,
        "starter": themes::BLANK,
        "written": applied.path.display().to_string(),
        "stylesheet": applied.stylesheet.as_ref().map(|p| p.display().to_string()),
        "backup": applied.backup.as_ref().map(|p| p.display().to_string()),
        "impeccable": impeccable_installed(&root),
        "tailwind": tailwind.as_ref().map(|t| serde_json::json!({
            "input": t.input,
            "stylesheet": t.stylesheet,
            "input_written": t.input_written,
            "config_added": t.config_added,
        })),
    }));
    Ok(())
}

/// What `seite theme new --tailwind` did on top of the blank starter.
struct TailwindScaffold {
    /// The entry CSS, relative to the site root.
    input: String,
    /// The compiled stylesheet's URL (`/static/<output>`).
    stylesheet: String,
    input_written: bool,
    config_added: bool,
    /// Human-readable lines about what was done.
    notes: Vec<String>,
}

/// Turn the freshly scaffolded blank starter into a Tailwind theme: add
/// `[build.tailwind]` to `seite.toml` (when missing), write the entry CSS
/// (when missing; explicit `@source` paths so the build is deterministic),
/// and point `base.html`'s stylesheet link at the compiled CSS.
fn scaffold_tailwind(
    root: &Path,
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
) -> anyhow::Result<TailwindScaffold> {
    let mut notes = Vec::new();
    let tw = config.build.tailwind.clone().unwrap_or_default();

    let config_added = config.build.tailwind.is_none();
    if config_added {
        let toml_path = root.join("seite.toml");
        let text = std::fs::read_to_string(&toml_path)?;
        let updated = add_tailwind_table(&text, &tw)?;
        crate::safe_fs::write(root, &toml_path, updated)?;
        notes.push("Added [build.tailwind] to seite.toml.".into());
    }

    let input = root.join(&tw.input);
    let input_written = !input.exists();
    if input_written {
        let css = tailwind_entry_css(root, paths, &input);
        if let Some(parent) = input.parent() {
            crate::safe_fs::create_dir_all(root, parent)?;
        }
        crate::safe_fs::write(root, &input, css)?;
        notes.push(format!(
            "Wrote {} (Tailwind entry; `@source` lines list where your classes live).",
            tw.input
        ));
    }

    // The user just set this project up: trust it to run Tailwind (whose
    // plugins run project JavaScript). Recorded per user, outside the repo.
    match crate::trust::set_trusted(root, true) {
        Ok(true) => notes.push(
            "Trusted this project for Tailwind builds (`seite trust --revoke` undoes it).".into(),
        ),
        Ok(false) => {}
        Err(e) => notes.push(format!(
            "Could not record trust ({e}); run `seite trust` before `seite build`."
        )),
    }

    let stylesheet = format!("/static/{}", tw.output.replace('\\', "/"));
    let base = paths.templates.join("base.html");
    let html = std::fs::read_to_string(&base)?;
    let blank_link = format!("href=\"/static/{}\"", themes::BLANK_STYLESHEET);
    let linked = if html.contains(&format!("href=\"{stylesheet}\"")) {
        html
    } else if html.contains(&blank_link) {
        html.replacen(&blank_link, &format!("href=\"{stylesheet}\""), 1)
    } else {
        html.replacen(
            "{% block extra_css %}",
            &format!(
                "<link rel=\"stylesheet\" href=\"{stylesheet}\">\n    {{% block extra_css %}}"
            ),
            1,
        )
    };
    if !linked.contains(&format!("href=\"{stylesheet}\"")) {
        notes.push(format!(
            "Add <link rel=\"stylesheet\" href=\"{stylesheet}\"> to {} yourself.",
            base.display()
        ));
    }
    crate::safe_fs::write(root, &base, linked)?;
    notes.push(format!(
        "base.html links {stylesheet}; `seite build` compiles it with Tailwind v{} (downloaded once, checksum-verified). Style with utility classes in templates/ and content/.",
        tw.version
            .clone()
            .unwrap_or_else(|| crate::build::tailwind::PINNED_VERSION.to_string())
    ));
    Ok(TailwindScaffold {
        input: tw.input,
        stylesheet,
        input_written,
        config_added,
        notes,
    })
}

/// `seite.toml` with a `[build.tailwind]` table (input/output spelled out),
/// comments and formatting kept.
fn add_tailwind_table(text: &str, tw: &crate::config::TailwindSection) -> anyhow::Result<String> {
    let mut doc: toml_edit::DocumentMut = text.parse()?;
    let build = doc
        .entry("build")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    let Some(build) = build.as_table_mut() else {
        anyhow::bail!("[build] in seite.toml is not a table; add [build.tailwind] by hand");
    };
    let mut table = toml_edit::Table::new();
    table.insert("input", toml_edit::value(tw.input.clone()));
    table.insert("output", toml_edit::value(tw.output.clone()));
    build.insert("tailwind", toml_edit::Item::Table(table));
    Ok(doc.to_string())
}

/// The scaffolded Tailwind entry CSS. `source(none)` plus explicit
/// `@source` lines (relative to the CSS file) keep class detection to the
/// site's own sources, so `dist/` and caches never feed into the build.
fn tailwind_entry_css(root: &Path, paths: &crate::config::ResolvedPaths, input: &Path) -> String {
    let up = input
        .parent()
        .and_then(|dir| dir.strip_prefix(root).ok())
        .map(|rel| "../".repeat(rel.components().count()))
        .unwrap_or_default();
    let rel = |dir: &Path| {
        dir.strip_prefix(root)
            .unwrap_or(dir)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut css = String::from("@import \"tailwindcss\" source(none);\n");
    // The blank starter's look, under Tailwind's utilities.
    let blank_css = paths.static_dir.join(themes::BLANK_STYLESHEET);
    if blank_css.is_file() && blank_css.parent() == input.parent() {
        let file = blank_css
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        css.push_str(&format!(
            "/* The blank starter's styles as Tailwind's base layer; delete this line to start from Tailwind's preflight alone. */\n@import \"./{file}\" layer(base);\n"
        ));
    }
    css.push_str("\n/* Where class names are found (paths relative to this file). */\n");
    for dir in [&paths.templates, &paths.content, &paths.data_dir] {
        css.push_str(&format!("@source \"{up}{}\";\n", rel(dir)));
    }
    css.push_str("\n@theme {\n  /* Design tokens, e.g. --color-brand: #0057b7; → bg-brand, text-brand */\n}\n");
    css
}

/// impeccable's live-mode config (see its `reference/live-setup.md`).
pub const IMPECCABLE_LIVE_CONFIG: &str = ".impeccable/live/config.json";

/// Point impeccable's live mode at the site's `base.html` (every page
/// renders through it, and the dev server re-renders on change), merging
/// add-only: keys the file already has are never changed. Leaves an
/// unparsable file alone. Returns whether anything was written.
///
/// `cspChecked` is left for impeccable to ask about.
pub fn configure_impeccable_live(root: &Path, templates_dir: &Path) -> anyhow::Result<bool> {
    let path = root.join(IMPECCABLE_LIVE_CONFIG);
    let mut config = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => return Ok(false),
        },
        Err(_) => serde_json::Map::new(),
    };
    let base = templates_dir.join("base.html");
    let base = base
        .strip_prefix(root)
        .unwrap_or(&base)
        .to_string_lossy()
        .replace('\\', "/");
    let mut added = false;
    for (key, value) in [
        ("files", serde_json::json!([base])),
        ("insertBefore", serde_json::json!("</body>")),
        ("commentSyntax", serde_json::json!("html")),
    ] {
        if !config.contains_key(key) {
            config.insert(key.to_string(), value);
            added = true;
        }
    }
    if !added {
        return Ok(false);
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(config))? + "\n";
    crate::safe_fs::write(root, &path, text)?;
    Ok(true)
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
    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let static_dir = config.resolve_paths(&std::env::current_dir()?).static_dir;

    let base_path = PathBuf::from("templates").join("base.html");
    if !base_path.exists() {
        return Err(anyhow::anyhow!(
            "no templates/base.html found. Apply or create a theme first."
        ));
    }

    validate_theme_name(name)?;

    let content = std::fs::read_to_string(&base_path)?;
    // A theme is one file: the site's own stylesheets go in as <style>.
    let (content, inlined, refused) = inline_local_stylesheets(&content, &static_dir);
    for warning in &refused {
        human::warning(warning);
    }
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

    for css in &inlined {
        human::info(&format!("Inlined {css} into the exported theme"));
    }
    human::success(&format!("Exported theme '{}' to {}", name, dest.display()));
    human::info("Share this .tera file — others can install it with: seite theme install <url>");

    Ok(())
}

/// Replace each `<link rel="stylesheet" href="/static/….css">` whose file
/// exists under `static_dir` with a `<style>` element holding that CSS, so
/// an exported theme (e.g. one built on the blank starter) works on its own
/// once applied elsewhere. Returns the new template, the inlined hrefs, and
/// one warning per stylesheet refused because its path leaves `static/`.
fn inline_local_stylesheets(
    content: &str,
    static_dir: &Path,
) -> (String, Vec<String>, Vec<String>) {
    let mut out = String::with_capacity(content.len());
    let mut inlined = Vec::new();
    let mut refused = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("<link") {
        let Some(len) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start..start + len + 1];
        out.push_str(&rest[..start]);
        let href = tag
            .split_once("href=\"")
            .and_then(|(_, after)| after.split_once('"'))
            .map(|(href, _)| href);
        let css = match href {
            Some(href) if tag.contains("rel=\"stylesheet\"") && href.ends_with(".css") => {
                match href.strip_prefix("/static/") {
                    Some(rel) => match read_static_file(static_dir, rel) {
                        Ok(css) => css,
                        Err(reason) => {
                            refused.push(format!("Not inlining {href}: {reason}"));
                            None
                        }
                    },
                    None => None,
                }
            }
            _ => None,
        };
        match (href, css) {
            (Some(href), Some(css)) => {
                // `raw` keeps Tera from reading `{#`/`{{` in the CSS; a
                // stray `</style` must not end the element early.
                out.push_str(&format!(
                    "<style>{{% raw %}}\n{}\n{{% endraw %}}</style>",
                    css.trim_end().replace("</style", "<\\/style")
                ));
                inlined.push(href.to_string());
            }
            _ => out.push_str(tag),
        }
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    (out, inlined, refused)
}

/// Read `static_dir/<rel>` only when it is a real file inside `static_dir`:
/// `rel` must consist of plain path segments (no `..`, no root, no empty
/// segment as in `/static//etc/x.css`), no component below `static_dir` may
/// be a symlink, and the canonical path must stay under the canonical
/// `static_dir`. `Ok(None)` when the file doesn't exist.
fn read_static_file(static_dir: &Path, rel: &str) -> Result<Option<String>, String> {
    let rel_path = Path::new(rel);
    let plain = !rel.is_empty()
        && !rel.contains('\\')
        && !rel.split('/').any(str::is_empty)
        && rel_path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !plain {
        return Err("the path is not a plain path inside static/".to_string());
    }
    let path = static_dir.join(rel_path);
    if std::fs::symlink_metadata(&path).is_err() {
        return Ok(None);
    }
    crate::safe_fs::guard(static_dir, &path).map_err(|e| e.to_string())?;
    let canonical_dir = static_dir.canonicalize().map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&canonical_dir) || !canonical.is_file() {
        return Err("it resolves outside static/".to_string());
    }
    std::fs::read_to_string(&canonical)
        .map(Some)
        .map_err(|e| e.to_string())
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
            let tpl = tmp.path().join("templates");
            assert!(!impeccable_installed(tmp.path()));
            std::fs::create_dir_all(tmp.path().join(dir)).unwrap();
            assert!(impeccable_installed(tmp.path()));
            match create_plan(tmp.path(), &tpl, "coral brutalist", Harness::Claude) {
                CreatePlan::Impeccable {
                    lines,
                    scaffold_blank,
                } => {
                    // No base.html: the build uses the default example theme.
                    assert!(scaffold_blank);
                    let text = lines.join("\n");
                    assert!(text.contains("/impeccable init"), "{text}");
                    assert!(text.contains("/impeccable coral brutalist"), "{text}");
                    assert!(text.contains("$impeccable"), "{text}");
                    assert!(text.contains("static/css/site.css"), "{text}");
                    assert!(text.contains("impeccable detect dist/"), "{text}");
                }
                other => panic!("{other:?}"),
            }
            // With a PRODUCT.md the init step is skipped.
            std::fs::write(tmp.path().join("PRODUCT.md"), "brief").unwrap();
            let lines = impeccable_lines(tmp.path(), None).join("\n");
            assert!(!lines.contains("/impeccable init"), "{lines}");
            assert!(lines.contains("<describe your design>"), "{lines}");
        }
    }

    #[test]
    fn test_create_plan_without_impeccable_runs_the_seite_skill() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        assert_eq!(
            create_plan(tmp.path(), &tpl, "dark teal", Harness::Claude),
            CreatePlan::RunAgent {
                scaffold_blank: true,
                prompt: "/seite theme dark teal".into()
            }
        );
        assert_eq!(
            create_plan(tmp.path(), &tpl, "dark teal", Harness::Codex),
            CreatePlan::RunAgent {
                scaffold_blank: true,
                prompt: "$seite theme dark teal".into()
            }
        );
    }

    #[test]
    fn test_create_plan_scaffolds_blank_only_over_bundled_examples() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        std::fs::create_dir_all(&tpl).unwrap();
        let scaffold = |root: &Path| match create_plan(root, &tpl, "x", Harness::Claude) {
            CreatePlan::RunAgent { scaffold_blank, .. }
            | CreatePlan::Impeccable { scaffold_blank, .. } => scaffold_blank,
        };
        for theme in ["dark", "docs", "landing", "terminal"] {
            std::fs::write(
                tpl.join("base.html"),
                themes::by_name(theme).unwrap().base_html,
            )
            .unwrap();
            assert!(scaffold(tmp.path()), "{theme}");
        }
        // Already on blank, or a custom design: keep it.
        std::fs::write(tpl.join("base.html"), themes::blank().base_html).unwrap();
        assert!(!scaffold(tmp.path()));
        std::fs::write(tpl.join("base.html"), "<html>mine</html>").unwrap();
        assert!(!scaffold(tmp.path()));
        assert!(!is_bundled_example("custom"));
        assert!(!is_bundled_example(themes::BLANK));
        assert!(is_bundled_example("default"));
    }

    // --- theme new ---

    fn site_paths(root: &Path) -> crate::config::ResolvedPaths {
        let config: SiteConfig =
            toml::from_str("collections = []\n[site]\ntitle = \"T\"\n").unwrap();
        config.resolve_paths(root)
    }

    #[test]
    fn test_add_tailwind_table_keeps_comments() {
        let text = "# my site\n[site]\ntitle = \"x\" # keep\n\n[build]\nminify = true\n\n[deploy]\ntarget = \"netlify\"\n";
        let out = add_tailwind_table(text, &crate::config::TailwindSection::default()).unwrap();
        assert!(out.contains("# my site") && out.contains("# keep"), "{out}");
        assert!(
            out.contains("[build.tailwind]\ninput = \"static/css/tailwind.css\"\noutput = \"css/tailwind.css\"\n"),
            "{out}"
        );
        let parsed: toml::Value = toml::from_str(&out).unwrap();
        assert_eq!(parsed["build"]["minify"].as_bool(), Some(true));
        // No [build] yet: it is created.
        let out = add_tailwind_table("[site]\ntitle = \"x\"\n", &Default::default()).unwrap();
        let parsed: toml::Value = toml::from_str(&out).unwrap();
        assert!(parsed["build"]["tailwind"]["input"].is_str());
    }

    #[test]
    fn test_tailwind_entry_css_sources_relative_to_input() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = site_paths(tmp.path());
        let css = tailwind_entry_css(
            tmp.path(),
            &paths,
            &tmp.path().join("static/css/tailwind.css"),
        );
        assert!(css.starts_with("@import \"tailwindcss\" source(none);\n"));
        for dir in ["templates", "content", "data"] {
            assert!(css.contains(&format!("@source \"../../{dir}\";")), "{css}");
        }
        // No blank stylesheet next to it: no import of it.
        assert!(!css.contains("site.css"));
    }

    #[test]
    fn test_scaffold_blank_fresh_writes_base_and_stylesheet() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = site_paths(tmp.path());
        let applied = scaffold_blank(tmp.path(), &paths, false).unwrap();
        assert!(applied.backup.is_none());
        let base = std::fs::read_to_string(paths.templates.join("base.html")).unwrap();
        assert_eq!(base, themes::blank().base_html);
        assert!(base.contains(r#"{% include "seite/head.html" %}"#));
        assert!(base.contains(r#"href="/static/css/site.css""#));
        let css = paths.static_dir.join("css/site.css");
        assert_eq!(applied.stylesheet.as_deref(), Some(css.as_path()));
        assert!(std::fs::read_to_string(&css).unwrap().contains(":root"));
        // No impeccable: no live config.
        assert!(!tmp.path().join(IMPECCABLE_LIVE_CONFIG).exists());
    }

    #[test]
    fn test_scaffold_blank_refuses_custom_base_without_force() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = site_paths(tmp.path());
        std::fs::create_dir_all(&paths.templates).unwrap();
        std::fs::write(paths.templates.join("base.html"), "<html>mine</html>").unwrap();
        let err = scaffold_blank(tmp.path(), &paths, false).unwrap_err();
        assert!(err.to_string().contains("--force"), "{err}");
        assert_eq!(
            std::fs::read_to_string(paths.templates.join("base.html")).unwrap(),
            "<html>mine</html>"
        );

        // --force backs the custom file up first.
        let applied = scaffold_blank(tmp.path(), &paths, true).unwrap();
        let backup = applied.backup.unwrap();
        assert_eq!(
            std::fs::read_to_string(backup).unwrap(),
            "<html>mine</html>"
        );
        assert_eq!(
            std::fs::read_to_string(paths.templates.join("base.html")).unwrap(),
            themes::blank().base_html
        );
    }

    #[test]
    fn test_scaffold_blank_over_bundled_theme_needs_no_force_and_keeps_css() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = site_paths(tmp.path());
        std::fs::create_dir_all(&paths.templates).unwrap();
        std::fs::write(paths.templates.join("base.html"), themes::dark().base_html).unwrap();
        let css = paths.static_dir.join("css/site.css");
        std::fs::create_dir_all(css.parent().unwrap()).unwrap();
        std::fs::write(&css, "body { color: red; }").unwrap();
        let applied = scaffold_blank(tmp.path(), &paths, false).unwrap();
        assert!(applied.backup.is_none());
        // The site's own stylesheet is never overwritten.
        assert!(applied.stylesheet.is_none());
        assert_eq!(
            std::fs::read_to_string(&css).unwrap(),
            "body { color: red; }"
        );
    }

    #[test]
    fn test_new_theme_next_steps() {
        let tmp = tempfile::TempDir::new().unwrap();
        let text = new_theme_next_steps(tmp.path(), "coral").join("\n");
        assert!(text.contains("/seite theme <description>"), "{text}");
        assert!(text.contains("seite skill install impeccable"), "{text}");
        assert!(text.contains("seite theme export coral"), "{text}");
        assert!(text.contains("static/css/site.css"), "{text}");

        std::fs::create_dir_all(tmp.path().join(".claude/skills/impeccable")).unwrap();
        let text = new_theme_next_steps(tmp.path(), "coral").join("\n");
        assert!(text.contains("/impeccable init"), "{text}");
        assert!(!text.contains("seite skill install impeccable"), "{text}");
    }

    #[test]
    fn test_scaffold_blank_with_impeccable_configures_live_mode() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = site_paths(tmp.path());
        std::fs::create_dir_all(tmp.path().join(".claude/skills/impeccable")).unwrap();
        scaffold_blank(tmp.path(), &paths, false).unwrap();
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join(IMPECCABLE_LIVE_CONFIG)).unwrap(),
        )
        .unwrap();
        assert_eq!(config["files"], serde_json::json!(["templates/base.html"]));
        assert_eq!(config["insertBefore"], "</body>");
        assert_eq!(config["commentSyntax"], "html");
        assert!(config.get("cspChecked").is_none());
    }

    #[test]
    fn test_configure_impeccable_live_is_add_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl = tmp.path().join("templates");
        let path = tmp.path().join(IMPECCABLE_LIVE_CONFIG);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"files":["dist/**/*.html"],"cspChecked":true}"#).unwrap();
        assert!(configure_impeccable_live(tmp.path(), &tpl).unwrap());
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["files"], serde_json::json!(["dist/**/*.html"]));
        assert_eq!(config["cspChecked"], true);
        assert_eq!(config["insertBefore"], "</body>");
        // Nothing left to add: no write.
        assert!(!configure_impeccable_live(tmp.path(), &tpl).unwrap());
        // Unparsable config is left alone.
        std::fs::write(&path, "not json").unwrap();
        assert!(!configure_impeccable_live(tmp.path(), &tpl).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
    }

    #[test]
    fn test_scaffold_blank_over_unedited_pre_partial_theme_needs_no_force() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = site_paths(tmp.path());
        std::fs::create_dir_all(&paths.templates).unwrap();
        let legacy = include_str!("../../tests/fixtures/themes-pre-partials/landing.tera");
        std::fs::write(paths.templates.join("base.html"), legacy).unwrap();
        // An unedited 0.20.x copy is still "landing", not a custom design.
        assert_eq!(
            themes::active_theme(tmp.path(), &paths.templates),
            "landing"
        );
        let applied = scaffold_blank(tmp.path(), &paths, false).unwrap();
        assert!(applied.backup.is_none());
        assert!(!paths.templates.join("base.html.bak").exists());
    }

    #[test]
    fn test_inline_local_stylesheets() {
        let tmp = tempfile::TempDir::new().unwrap();
        let static_dir = tmp.path().join("static");
        std::fs::create_dir_all(static_dir.join("css")).unwrap();
        std::fs::write(
            static_dir.join("css/site.css"),
            "body { color: #123456; }\n/* {# not tera #} </style> */\n",
        )
        .unwrap();
        let base = "<head>\n<link rel=\"stylesheet\" href=\"/static/css/site.css\">\n<link rel=\"stylesheet\" href=\"/static/css/missing.css\">\n<link rel=\"stylesheet\" href=\"https://cdn.example/x.css\">\n<link rel=\"icon\" href=\"/favicon.ico\">\n</head>";
        let (out, inlined, refused) = inline_local_stylesheets(base, &static_dir);
        assert_eq!(inlined, ["/static/css/site.css"]);
        assert!(refused.is_empty(), "{refused:?}");
        assert!(!out.contains("href=\"/static/css/site.css\""), "{out}");
        assert!(
            out.contains("<style>{% raw %}\nbody { color: #123456; }"),
            "{out}"
        );
        assert!(out.contains("<\\/style> */\n{% endraw %}</style>"), "{out}");
        // Other links stay as they are.
        assert!(out.contains("href=\"/static/css/missing.css\""));
        assert!(out.contains("href=\"https://cdn.example/x.css\""));
        assert!(out.contains("<link rel=\"icon\" href=\"/favicon.ico\">"));
        // The result is still a valid template.
        let mut tera = tera::Tera::default();
        tera.add_raw_template("t", &out).unwrap();
        let rendered = tera.render("t", &tera::Context::new()).unwrap();
        assert!(rendered.contains("{# not tera #}"), "{rendered}");
    }

    #[test]
    fn test_inline_local_stylesheets_refuses_paths_outside_static() {
        let tmp = tempfile::TempDir::new().unwrap();
        let static_dir = tmp.path().join("static");
        std::fs::create_dir_all(static_dir.join("css")).unwrap();
        std::fs::write(static_dir.join("css/site.css"), "body{}").unwrap();
        let outside = tmp.path().join("secret.css");
        std::fs::write(&outside, "SECRET").unwrap();
        let abs = outside.to_str().unwrap().to_string();
        let mut links = vec![
            format!("<link rel=\"stylesheet\" href=\"/static/{abs}\">"),
            "<link rel=\"stylesheet\" href=\"/static/../secret.css\">".to_string(),
            "<link rel=\"stylesheet\" href=\"/static/css/../../secret.css\">".to_string(),
        ];
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, static_dir.join("css/link.css")).unwrap();
            std::os::unix::fs::symlink(tmp.path(), static_dir.join("up")).unwrap();
            links.push("<link rel=\"stylesheet\" href=\"/static/css/link.css\">".to_string());
            links.push("<link rel=\"stylesheet\" href=\"/static/up/secret.css\">".to_string());
        }
        let base = links.join("\n");
        let (out, inlined, refused) = inline_local_stylesheets(&base, &static_dir);
        assert!(inlined.is_empty(), "{inlined:?}");
        assert!(!out.contains("SECRET"), "{out}");
        assert_eq!(out, base);
        assert_eq!(refused.len(), links.len(), "{refused:?}");

        // A normal file next to them is still inlined.
        let (out, inlined, refused) = inline_local_stylesheets(
            "<link rel=\"stylesheet\" href=\"/static/css/site.css\">",
            &static_dir,
        );
        assert_eq!(inlined, ["/static/css/site.css"]);
        assert!(refused.is_empty());
        assert!(out.contains("body{}"));
    }
}
