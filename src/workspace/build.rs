use std::path::Path;

use crate::build::{self, links, BuildOptions, BuildResult};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::output::{human, CommandOutput};

use super::{load_site_in_workspace, WorkspaceConfig};

pub struct WorkspaceBuildOptions {
    pub include_drafts: bool,
    pub strict: bool,
    pub site_filter: Option<String>,
}

pub struct WorkspaceBuildResult {
    pub site_results: Vec<(String, BuildResult)>,
}

impl WorkspaceBuildResult {
    pub fn stats_summary(&self) -> String {
        let mut parts = Vec::new();
        for (name, result) in &self.site_results {
            let items: Vec<String> = result
                .stats
                .items_built
                .iter()
                .map(|(col, count)| format!("{count} {col}"))
                .collect();
            parts.push(format!("{name}: {}", items.join(", ")));
        }
        parts.join(" | ")
    }
}

/// Build all (or filtered) sites in a workspace.
///
/// With `strict`, every site is still built so one run reports all sites'
/// broken links / missing assets; the build then fails once at the end with
/// the problems as diagnostics whose files are workspace-relative
/// (`sites/blog/content/...`), mirroring single-site `build --strict`.
pub fn build_workspace(
    ws_config: &WorkspaceConfig,
    ws_root: &Path,
    opts: &WorkspaceBuildOptions,
) -> anyhow::Result<WorkspaceBuildResult> {
    let sites = ws_config
        .sites_to_operate(opts.site_filter.as_deref())
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let total = sites.len();
    let mut site_results = Vec::new();
    let mut strict_failures: Vec<String> = Vec::new();
    let mut strict_diagnostics = Diagnostics::new();

    for (i, ws_site) in sites.iter().enumerate() {
        human::header(&format!(
            "[{}/{}] Building site '{}'",
            i + 1,
            total,
            ws_site.name
        ));

        let (config, paths) = load_site_in_workspace(ws_root, ws_site)?;

        let build_opts = BuildOptions {
            include_drafts: opts.include_drafts,
            incremental: false,
        };

        let result = build::build_site(&config, &paths, &build_opts)?;
        human::success(&result.stats.human_display());

        // Link validation results from the post-process pass (no extra file walk)
        let problems = crate::cli::build::print_link_report(
            &result.link_check,
            opts.strict,
            Some(&ws_site.name),
        );
        if opts.strict && problems > 0 {
            strict_failures.push(format!(
                "site '{}' has {}",
                ws_site.name,
                crate::cli::build::problem_summary(&result.link_check),
            ));
            let site_dir = Path::new(&ws_site.path);
            strict_diagnostics.extend(
                links::link_diagnostics(&result.link_check, true)
                    .into_iter()
                    .map(|d| prefix_file(d, site_dir)),
            );
        }

        site_results.push((ws_site.name.clone(), result));
    }

    if !strict_failures.is_empty() {
        return Err(crate::cli::build::strict_failure(
            format!("Build failed: {}", strict_failures.join("; ")),
            strict_diagnostics,
        ));
    }

    human::header("Workspace build complete");
    let ws_result = WorkspaceBuildResult { site_results };
    human::success(&ws_result.stats_summary());

    Ok(ws_result)
}

/// Prefix a site-relative diagnostic file with the site's workspace path so
/// files stay unambiguous across sites.
pub(crate) fn prefix_file(mut d: Diagnostic, site_dir: &Path) -> Diagnostic {
    if let Some(file) = d.file.take() {
        d.file = Some(if file.is_absolute() {
            file
        } else {
            site_dir.join(file)
        });
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_prefix_file_joins_site_path() {
        let d = prefix_file(
            Diagnostic::error("broken-link", "x").with_file("content/a.md"),
            Path::new("sites/blog"),
        );
        assert_eq!(d.file, Some(PathBuf::from("sites/blog/content/a.md")));
        let d = prefix_file(Diagnostic::error("broken-link", "x"), Path::new("s"));
        assert_eq!(d.file, None);
    }
}
