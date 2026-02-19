use std::path::Path;

use crate::build::{self, links, BuildOptions, BuildResult};
use crate::output::human;
use crate::output::CommandOutput;

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
        };

        let result = build::build_site(&config, &paths, &build_opts)?;
        human::success(&result.stats.human_display());

        // Post-build: validate internal links per site
        let link_result = links::check_internal_links(&paths.output)?;
        if !link_result.broken_links.is_empty() {
            let grouped = links::group_broken_links(&link_result.broken_links);
            let count = link_result.broken_links.len();
            let target_count = grouped.len();

            let header = format!(
                "Site '{}': {count} broken internal link{} ({target_count} broken target{})",
                ws_site.name,
                if count == 1 { "" } else { "s" },
                if target_count == 1 { "" } else { "s" },
            );

            if opts.strict {
                human::error(&header);
            } else {
                human::warning(&header);
            }

            for (href, sources) in &grouped {
                human::info(&format!(
                    "  {} (linked from {} file{})",
                    href,
                    sources.len(),
                    if sources.len() == 1 { "" } else { "s" }
                ));
                for source in sources {
                    human::info(&format!("    - {source}"));
                }
            }

            if opts.strict {
                anyhow::bail!(
                    "Build failed: site '{}' has {count} broken internal link{}",
                    ws_site.name,
                    if count == 1 { "" } else { "s" },
                );
            }
        }

        site_results.push((ws_site.name.clone(), result));
    }

    human::header("Workspace build complete");
    let ws_result = WorkspaceBuildResult { site_results };
    human::success(&ws_result.stats_summary());

    Ok(ws_result)
}
