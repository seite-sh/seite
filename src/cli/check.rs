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
use crate::config::SiteConfig;
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::error::PageError;
use crate::output::{human, json as json_out};
use crate::templates;

#[derive(Args)]
pub struct CheckArgs {
    /// Fail on warnings too (unknown config keys, broken links, ...)
    #[arg(long)]
    pub strict: bool,

    /// Include draft content in the check
    #[arg(long)]
    pub drafts: bool,
}

pub fn run(args: &CheckArgs) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let diagnostics = check_site(&root, args.drafts)?;
    report(diagnostics, args.strict)
}

/// Collect every problem in the site at `root` as diagnostics (paths relative
/// to `root`, sorted). Only returns `Err` when checking cannot start at all
/// (e.g. there is no `seite.toml`).
pub fn check_site(root: &Path, include_drafts: bool) -> crate::error::Result<Diagnostics> {
    let mut all = Diagnostics::new();

    // 1. Config: syntax/type errors stop the check (nothing else can load).
    let (config, config_warnings) =
        match SiteConfig::load_with_diagnostics(&root.join("seite.toml")) {
            Ok(loaded) => loaded,
            Err(PageError::Diagnostics(d)) => return Ok(d),
            Err(PageError::ConfigInvalid { message }) => {
                all.push(Diagnostic::error("config-invalid", message).with_file("seite.toml"));
                return Ok(all);
            }
            Err(e) => return Err(e),
        };
    all.extend(config_warnings);
    let paths = config.resolve_paths(root);

    // 2. Templates: every file that fails to parse (the build would silently
    //    fall back to the built-in templates, so these are errors here).
    let template_errors = templates::template_parse_diagnostics(&paths.templates);
    let has_template_errors = !template_errors.is_empty();
    all.extend(template_errors.relative_to(root));

    // 3. Data + content + render + links: the real pipeline, writing into a
    //    scratch directory. Pointing `root` at the scratch dir too keeps the
    //    subdomain output (`<root>/dist-subdomains`) out of the project.
    let scratch = tempfile::Builder::new().prefix("seite-check-").tempdir()?;
    let mut scratch_paths = paths.clone();
    scratch_paths.root = scratch.path().to_path_buf();
    scratch_paths.output = scratch.path().join("dist");
    let opts = BuildOptions {
        include_drafts,
        incremental: false,
    };
    let build_diagnostics = match build::build_site(&config, &scratch_paths, &opts) {
        Ok(result) => {
            let mut found = result.diagnostics;
            found.extend(broken_link_diagnostics(&result.link_check));
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

/// Convert the build's broken-link results into `broken-link` warnings.
///
/// Kept in one place so richer source attribution (the markdown file and line
/// that contains the link) can replace it without touching the rest of `check`.
pub fn broken_link_diagnostics(link_check: &links::LinkCheckResult) -> Vec<Diagnostic> {
    link_check
        .broken_links
        .iter()
        .map(|link| {
            Diagnostic::warning(
                "broken-link",
                format!(
                    "broken internal link `{}` (in generated page `{}`)",
                    link.href, link.source_file
                ),
            )
            .with_hint("point the link at an existing page, or create the missing page")
        })
        .collect()
}

/// Make paths in a diagnostic built against the scratch root relative to the
/// real site `root`: the `file` field, plus absolute paths quoted in the
/// message (render errors name the source and template files).
fn relativize(mut d: Diagnostic, root: &Path, scratch: &Path) -> Diagnostic {
    for base in [root, scratch] {
        let prefix = format!("{}{}", base.display(), std::path::MAIN_SEPARATOR);
        if d.message.contains(&prefix) {
            d.message = d.message.replace(&prefix, "");
        }
    }
    d.relative_to(root)
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
    fn test_broken_link_diagnostics() {
        let result = links::LinkCheckResult {
            total_links_checked: 3,
            broken_links: vec![links::BrokenLink {
                source_file: "posts/a/index.html".into(),
                href: "/nope".into(),
            }],
        };
        let d = broken_link_diagnostics(&result);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, "broken-link");
        assert!(!d[0].is_error());
        assert!(d[0].message.contains("/nope"));
        assert!(d[0].message.contains("posts/a/index.html"));
    }

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
