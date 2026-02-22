use std::path::PathBuf;

use clap::Args;

use crate::build::{self, links, BuildOptions};
use crate::config::SiteConfig;
use crate::meta;
use crate::output::human;
use crate::output::CommandOutput;
use crate::workspace;

#[derive(Args)]
pub struct BuildArgs {
    /// Include draft content in the build
    #[arg(long)]
    pub drafts: bool,

    /// Treat broken internal links as build errors
    #[arg(long)]
    pub strict: bool,
}

pub fn run(args: &BuildArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    // Nudge if project config is outdated
    if cwd.join("seite.toml").exists() && meta::needs_upgrade(&cwd) {
        let project_ver = meta::project_version(&cwd);
        let label = if project_ver == (0, 0, 0) {
            "pre-tracking".to_string()
        } else {
            meta::format_version(project_ver)
        };
        human::info(&format!(
            "Project config is from seite {label}. Run `seite upgrade` for new features."
        ));
    }

    // Check for workspace context
    if let Some(ws_root) = workspace::find_workspace_root(&cwd) {
        let ws_config = workspace::WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;

        let opts = workspace::build::WorkspaceBuildOptions {
            include_drafts: args.drafts,
            strict: args.strict,
            site_filter: site_filter.map(String::from),
        };

        workspace::build::build_workspace(&ws_config, &ws_root, &opts)?;
        return Ok(());
    }

    // Standalone mode
    if site_filter.is_some() {
        human::warning("--site flag ignored (not in a workspace)");
    }

    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    let paths = config.resolve_paths(&cwd);

    let opts = BuildOptions {
        include_drafts: args.drafts,
    };

    let result = build::build_site(&config, &paths, &opts)?;
    human::success(&result.stats.human_display());

    // Post-build: validate internal links
    let link_result = links::check_internal_links(&paths.output)?;
    if !link_result.broken_links.is_empty() {
        let grouped = links::group_broken_links(&link_result.broken_links);
        let count = link_result.broken_links.len();
        let target_count = grouped.len();

        let header = format!(
            "Found {count} broken internal link{} ({target_count} broken target{})",
            if count == 1 { "" } else { "s" },
            if target_count == 1 { "" } else { "s" },
        );

        if args.strict {
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

        if args.strict {
            anyhow::bail!(
                "Build failed: {count} broken internal link{}",
                if count == 1 { "" } else { "s" },
            );
        }
    }

    Ok(())
}
