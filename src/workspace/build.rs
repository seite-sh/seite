use std::path::Path;

use crate::build::{self, links, BuildOptions, BuildResult};
use crate::diagnostics::Diagnostics;
use crate::error::PageError;
use crate::output::{human, CommandOutput};

use super::{load_site_in_workspace_with_diagnostics, WorkspaceConfig};

pub struct WorkspaceBuildOptions {
    pub include_drafts: bool,
    pub strict: bool,
    pub site_filter: Option<String>,
}

pub struct WorkspaceBuildResult {
    /// Per-site results. Each result's `diagnostics` starts with the site's
    /// config warnings (`config-unknown-key`), with site-relative files.
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
/// Each site's `seite.toml` is loaded with diagnostics, so unknown keys are
/// warned about (as `sites/<name>/seite.toml:line:col: warning[...]`) like in
/// a single-site build. With `strict`, every site is still built so one run
/// reports all sites' broken links / missing assets; the build then fails
/// once at the end with the problems as diagnostics whose files are
/// workspace-relative (`sites/blog/content/...`), mirroring single-site
/// `build --strict`.
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

        let site_dir = Path::new(&ws_site.path);
        let (config, paths, config_diagnostics) =
            load_site_in_workspace_with_diagnostics(ws_root, ws_site).map_err(|e| match e {
                // Locate config syntax errors in this site's seite.toml.
                PageError::Diagnostics(d) => PageError::Diagnostics(Diagnostics::from(
                    d.into_iter().map(|d| d.under(site_dir)).collect::<Vec<_>>(),
                )),
                other => other,
            })?;
        // Unknown keys (typos such as `minfy`) are warnings: the build goes on.
        for d in &config_diagnostics {
            human::warning(&d.clone().under(site_dir).to_string());
        }

        let build_opts = BuildOptions {
            include_drafts: opts.include_drafts,
            incremental: false,
        };

        let mut result = build::build_site(&config, &paths, &build_opts)?;
        for d in result
            .diagnostics
            .iter()
            .filter(|d| d.code == "template-parse")
        {
            // Workspace-relative, like the config warnings above: a bare
            // `templates/…` would name the workspace's own templates dir.
            human::warning(&d.clone().under(site_dir).to_string());
        }
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
            strict_diagnostics.extend(
                links::link_diagnostics(&result.link_check, true)
                    .into_iter()
                    .map(|d| d.under(site_dir)),
            );
        }

        let mut diagnostics = Diagnostics::from(config_diagnostics);
        diagnostics.extend(result.diagnostics.iter().cloned());
        result.diagnostics = diagnostics;
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
