//! `seite check`: validate the whole site without touching the output dir.
//!
//! Checks the config (syntax + unknown keys), every template (parse), every
//! data and content file (frontmatter, shortcodes), then runs the real build
//! pipeline into a throwaway temporary directory to catch render errors and
//! broken internal links. The project's `dist/` (and `dist-subdomains/`) are
//! never created, cleaned, or modified.

use std::path::Path;

use clap::Args;
use serde_json::json;

use crate::build::{self, links, BuildOptions};
use crate::cli::harness::Agent;
use crate::cli::harness_hooks;
use crate::config::SiteConfig;
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::error::PageError;
use crate::output::{human, json as json_out};
use crate::templates;
use crate::workspace;

#[derive(Args)]
pub struct CheckArgs {
    /// Fail on warnings too (unknown config keys, broken links, ...)
    #[arg(long)]
    pub strict: bool,

    /// Include draft content in the check
    #[arg(long)]
    pub drafts: bool,

    /// Run as a coding agent's turn-end hook: read the hook input on stdin
    /// and, only when there are errors, answer in that agent's hook protocol
    /// so it fixes them before stopping (see `seite init --agents`)
    #[arg(
        long,
        value_name = "AGENT",
        value_parser = ["claude", "codex", "cursor", "opencode", "gemini"],
        conflicts_with = "strict"
    )]
    pub hook: Option<String>,

    /// With --hook: run as the agent's after-edit hook, checking only the
    /// file(s) named in the hook input
    #[arg(long, requires = "hook")]
    pub post_edit: bool,

    /// Check only these files (frontmatter, shortcodes, template syntax,
    /// data-file syntax; no render, no link check). Repeatable.
    #[arg(long = "file", value_name = "PATH", num_args = 1.., conflicts_with = "hook")]
    pub files: Vec<std::path::PathBuf>,
}

/// Check the site in the current directory. In a workspace, `--site <name>`
/// (or running from the workspace root, which has no `seite.toml`) checks
/// the workspace's sites instead, with workspace-relative file paths. With
/// `--hook <agent>` it speaks that coding agent's hook protocol; with
/// `--file` it checks only the given files.
pub fn run(args: &CheckArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    if let Some(agent) = args.hook.as_deref().and_then(Agent::from_id) {
        if crate::output::is_json() {
            anyhow::bail!("--hook prints its own protocol output; drop --json");
        }
        let kind = if args.post_edit {
            harness_hooks::HookKind::PostEdit
        } else {
            harness_hooks::HookKind::Stop
        };
        return harness_hooks::run(agent, args.drafts, kind);
    }
    if !args.files.is_empty() {
        let cwd = std::env::current_dir()?;
        let root = harness_hooks::find_site_root(std::slice::from_ref(&cwd)).ok_or_else(|| {
            anyhow::anyhow!("no seite.toml found in this directory or its parents")
        })?;
        let files: Vec<std::path::PathBuf> = args.files.iter().map(|f| cwd.join(f)).collect();
        let diagnostics = check_files(&root, &files)?;
        return report(diagnostics, args.strict);
    }
    let cwd = std::env::current_dir()?;
    if let Some(ws_root) = workspace::find_workspace_root(&cwd) {
        if site_filter.is_some() || !cwd.join("seite.toml").exists() {
            let ws_config =
                workspace::WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;
            let diagnostics = check_workspace(&ws_config, &ws_root, site_filter, args.drafts)?;
            return report(diagnostics, args.strict);
        }
    }
    if site_filter.is_some() {
        human::warning("--site flag ignored (not in a workspace)");
    }
    let diagnostics = check_site(&cwd, args.drafts)?;
    report(diagnostics, args.strict)
}

/// Check every (or the `site_filter`) site of a workspace. Each site is
/// checked like a standalone site (with the workspace's `base_url`
/// override); diagnostic files are prefixed with the site's path so they are
/// relative to the workspace root (`sites/blog/seite.toml`).
pub fn check_workspace(
    ws_config: &workspace::WorkspaceConfig,
    ws_root: &Path,
    site_filter: Option<&str>,
    include_drafts: bool,
) -> crate::error::Result<Diagnostics> {
    let mut all = Diagnostics::new();
    for ws_site in ws_config.sites_to_operate(site_filter)? {
        let site_dir = Path::new(&ws_site.path);
        let found = check_site_with(
            &ws_root.join(site_dir),
            include_drafts,
            ws_site.base_url.as_deref(),
        )?;
        all.extend(found.into_iter().map(|d| d.under(site_dir)));
    }
    Ok(all)
}

/// Fast check of individual files of the site at `root` (paths absolute or
/// relative to `root`): the config always (it has to load), plus per file:
///
/// - `seite.toml`: its unknown-key warnings
/// - content markdown: frontmatter and shortcode syntax / unknown shortcodes
/// - templates (`.html`): Tera syntax
/// - data files (`.yaml`, `.yml`, `.json`, `.toml`): syntax
///
/// Nothing is rendered, so render errors and broken links are left to the
/// full `seite check`. Other files (and files that no longer exist) are
/// skipped. Diagnostics are relative to `root`.
pub fn check_files(root: &Path, files: &[std::path::PathBuf]) -> crate::error::Result<Diagnostics> {
    let mut all = Diagnostics::new();
    let (config, config_warnings) =
        match SiteConfig::load_with_diagnostics(&root.join("seite.toml")) {
            Ok(loaded) => loaded,
            Err(PageError::Diagnostics(d)) => return Ok(d.relative_to(root)),
            Err(PageError::ConfigInvalid { message }) => {
                all.push(Diagnostic::error("config-invalid", message).with_file("seite.toml"));
                return Ok(all);
            }
            Err(e) => return Err(e),
        };
    let paths = config.resolve_paths(root);
    let registry =
        crate::shortcodes::ShortcodeRegistry::new(&paths.templates.join("shortcodes")).ok();
    let empty = json!({});
    let ext_of = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default()
    };
    // Compare canonical paths so `./content/x.md`, symlinked temp dirs, and
    // absolute paths from hooks all match.
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let (config_file, content_dir, templates_dir, data_dir) = (
        canon(&root.join("seite.toml")),
        canon(&paths.content),
        canon(&paths.templates),
        canon(&paths.data_dir),
    );
    let mut config_reported = false;
    for file in files {
        let abs = canon(&root.join(file));
        if !abs.is_file() {
            continue;
        }
        let ext = ext_of(&abs);
        if abs == config_file {
            if !config_reported {
                all.extend(config_warnings.clone());
                config_reported = true;
            }
        } else if abs.starts_with(&content_dir) && ext == "md" {
            match crate::content::parse_content_diagnostic(&abs) {
                Err(d) => all.push(*d),
                Ok(parsed) => {
                    let Some(registry) = &registry else { continue };
                    if let Err(problems) =
                        registry.expand_diagnostic(&parsed.body, &abs, &empty, &empty, &empty)
                    {
                        // Syntax and unknown names only: rendering needs the
                        // full page context, which the full check provides.
                        for mut d in problems
                            .into_iter()
                            .filter(|d| matches!(d.code, "shortcode-syntax" | "shortcode-unknown"))
                        {
                            d.line = d.line.map(|l| parsed.file_line(l));
                            all.push(d);
                        }
                    }
                }
            }
        } else if abs.starts_with(&templates_dir) && ext == "html" {
            if let Some(d) = templates::template_file_diagnostic(&templates_dir, &abs) {
                all.push(d);
            }
        } else if abs.starts_with(&data_dir)
            && matches!(ext.as_str(), "yaml" | "yml" | "json" | "toml")
        {
            if let Err(d) = crate::data::parse_data_file(&abs) {
                all.push(*d);
            }
        }
    }
    let canonical_root = canon(root);
    Ok(all.relative_to(&canonical_root).relative_to(root))
}

/// Collect every problem in the site at `root` as diagnostics (paths relative
/// to `root`, sorted). Only returns `Err` when checking cannot start at all
/// (e.g. there is no `seite.toml`).
pub fn check_site(root: &Path, include_drafts: bool) -> crate::error::Result<Diagnostics> {
    check_site_with(root, include_drafts, None)
}

/// [`check_site`] with an optional `base_url` override (workspace sites).
fn check_site_with(
    root: &Path,
    include_drafts: bool,
    base_url: Option<&str>,
) -> crate::error::Result<Diagnostics> {
    let mut all = Diagnostics::new();

    // 1. Config: syntax/type errors stop the check (nothing else can load).
    let (mut config, config_warnings) =
        match SiteConfig::load_with_diagnostics(&root.join("seite.toml")) {
            Ok(loaded) => loaded,
            Err(PageError::Diagnostics(d)) => return Ok(d),
            Err(PageError::ConfigInvalid { message }) => {
                all.push(Diagnostic::error("config-invalid", message).with_file("seite.toml"));
                return Ok(all);
            }
            Err(e) => return Err(e),
        };
    if let Some(base_url) = base_url {
        config.site.base_url = base_url.to_string();
    }
    all.extend(config_warnings);
    let paths = config.resolve_paths(root);

    // 2. Templates: every file that fails to parse (the build would silently
    //    fall back to the built-in templates, so these are errors here).
    let template_errors = templates::template_parse_diagnostics(&paths.templates);
    let has_template_errors = !template_errors.is_empty();
    all.extend(template_errors.relative_to(root));

    // 3. Data + content + render + links: the real pipeline, writing into a
    //    scratch directory. Only the output locations (main site and
    //    subdomain sites) move; every source path (root, content, templates,
    //    data, ...) stays the real site's, so links such as
    //    `/content/pages/x.md` resolve exactly as they do in `seite build`.
    let scratch = tempfile::Builder::new().prefix("seite-check-").tempdir()?;
    let mut scratch_paths = paths.clone();
    scratch_paths.output = scratch.path().join("dist");
    scratch_paths.subdomain_output_root = scratch.path().join("dist-subdomains");
    let opts = BuildOptions {
        include_drafts,
        incremental: false,
    };
    let build_diagnostics = match build::build_site(&config, &scratch_paths, &opts) {
        Ok(result) => {
            let mut found = result.diagnostics;
            found.extend(links::link_diagnostics(&result.link_check, false));
            found
        }
        Err(PageError::Diagnostics(d)) => d,
        Err(e) => Diagnostic::error("build-failed", e.to_string()).into(),
    };
    for d in build_diagnostics {
        // Already reported (as errors) by the template pass above.
        if has_template_errors && d.code == "template-parse" {
            continue;
        }
        all.push(relativize(d, root, scratch.path()));
    }

    all.sort();
    let mut unique = Diagnostics::new();
    for d in all {
        if !unique.iter().any(|u| *u == d) {
            unique.push(d);
        }
    }
    Ok(unique)
}

/// Make paths in a diagnostic relative: sources to the real site `root`, and
/// generated pages (built into `scratch`) to the scratch dir, so they read as
/// `dist/...` like in `seite build`. Covers the `file` field plus absolute
/// paths quoted in the message (render errors name the source and template
/// files).
fn relativize(mut d: Diagnostic, root: &Path, scratch: &Path) -> Diagnostic {
    for base in [root, scratch] {
        let prefix = format!("{}{}", base.display(), std::path::MAIN_SEPARATOR);
        if d.message.contains(&prefix) {
            d.message = d.message.replace(&prefix, "");
        }
    }
    d.relative_to(root).relative_to(scratch)
}

/// Print the result and pick success/failure. On failure the diagnostics
/// travel in the error (`PageError::Diagnostics`), which `main` renders as
/// compiler-style lines and as `error.diagnostics` in the `--json` envelope.
fn report(diagnostics: Diagnostics, strict: bool) -> anyhow::Result<()> {
    let errors = diagnostics.error_count();
    let warnings = diagnostics.warning_count();
    json_out::set_data(json!({
        "diagnostics": &diagnostics,
        "summary": { "errors": errors, "warnings": warnings },
    }));

    if errors > 0 || (strict && warnings > 0) {
        let err = anyhow::Error::new(PageError::Diagnostics(diagnostics));
        if errors == 0 {
            return Err(err.context(format!(
                "check failed: {warnings} warning{} (--strict treats warnings as errors)",
                if warnings == 1 { "" } else { "s" }
            )));
        }
        return Err(err);
    }

    if diagnostics.is_empty() {
        human::success("No problems found");
    } else {
        for d in diagnostics.iter() {
            crate::human_println!("{d}");
        }
        human::success(&format!("No errors ({})", diagnostics.summary()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relativize_strips_roots_from_file_and_message() {
        let root = Path::new("/site");
        let scratch = Path::new("/tmp/scratch");
        let sep = std::path::MAIN_SEPARATOR;
        let d = Diagnostic::error(
            "template-render",
            format!("failed to render /site{sep}content{sep}a.md"),
        )
        .with_file("/site/content/a.md");
        let d = relativize(d, root, scratch);
        assert_eq!(d.file.as_deref(), Some(Path::new("content/a.md")));
        assert_eq!(d.message, format!("failed to render content{sep}a.md"));

        // Generated pages live in the scratch output dir.
        let d = Diagnostic::warning("broken-link", "x").with_file("/tmp/scratch/dist/tags.html");
        let d = relativize(d, root, scratch);
        assert_eq!(d.file.as_deref(), Some(Path::new("dist/tags.html")));
    }

    #[test]
    fn test_report_exit_semantics() {
        assert!(report(Diagnostics::new(), true).is_ok());
        let warn: Diagnostics = Diagnostic::warning("w", "w").into();
        assert!(report(warn.clone(), false).is_ok());
        let err = report(warn, true).unwrap_err();
        assert!(err.to_string().contains("--strict"));
        let error: Diagnostics = Diagnostic::error("e", "e").into();
        assert!(report(error, false).is_err());
    }

    #[test]
    fn test_check_site_without_config_is_an_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(matches!(
            check_site(tmp.path(), false),
            Err(PageError::ConfigNotFound { .. })
        ));
    }

    #[test]
    fn test_check_site_invalid_config_is_a_located_diagnostic() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("seite.toml"), "[site]\ntitle = \n").unwrap();
        let d = check_site(tmp.path(), false).unwrap();
        assert_eq!(d.len(), 1);
        let first = d.iter().next().unwrap();
        assert_eq!(first.code, "config-invalid");
        assert_eq!(first.line, Some(2));
    }
}
