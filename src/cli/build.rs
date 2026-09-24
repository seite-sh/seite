use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{json, Value};

use crate::build::{self, links, BuildOptions, BuildResult};
use crate::config::SiteConfig;
use crate::meta;
use crate::output::{self, human, json as json_out, CommandOutput};
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

        let ws_result = workspace::build::build_workspace(&ws_config, &ws_root, &opts)?;
        let sites: serde_json::Map<String, Value> = ws_result
            .site_results
            .iter()
            .map(|(name, result)| (name.clone(), build_data(result, None)))
            .collect();
        json_out::set_data(
            json!({ "workspace_root": ws_root.display().to_string(), "sites": sites }),
        );
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
        incremental: false,
    };

    let result = build::build_site(&config, &paths, &opts)?;
    human::success(&result.stats.human_display());
    if output::is_verbose() {
        if let Some(timings) = result.stats.timings_display() {
            crate::human_println!("{timings}");
        }
    }

    // Display subdomain build results
    for sub in &result.subdomain_builds {
        let items: usize = sub.stats.items_built.values().sum();
        human::success(&format!(
            "Subdomain {}.{}: built {} item{} -> {}",
            sub.subdomain,
            config.base_domain().unwrap_or_default(),
            items,
            if items == 1 { "" } else { "s" },
            sub.output_dir.display()
        ));
    }

    // Link validation results from the post-process pass (no extra file walk)
    if !result.link_check.broken_links.is_empty() {
        let grouped = links::group_broken_links(&result.link_check.broken_links);
        let count = result.link_check.broken_links.len();
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

    json_out::set_data(build_data(&result, Some(&paths.output)));
    Ok(())
}

/// Structured build summary for the `--json` envelope.
fn build_data(result: &BuildResult, output_dir: Option<&Path>) -> Value {
    let stats = &result.stats;
    let timings: serde_json::Map<String, Value> = stats
        .step_timings
        .iter()
        .map(|(step, ms)| (step.clone(), json!((ms * 10.0).round() / 10.0)))
        .collect();
    let broken_links: Vec<Value> = links::group_broken_links(&result.link_check.broken_links)
        .into_iter()
        .map(|(target, sources)| json!({ "target": target, "sources": sources }))
        .collect();
    let subdomains: Vec<Value> = result
        .subdomain_builds
        .iter()
        .map(|sub| {
            json!({
                "collection": sub.collection_name,
                "subdomain": sub.subdomain,
                "base_url": sub.base_url,
                "output_dir": sub.output_dir.display().to_string(),
                "collections": sub.stats.items_built,
            })
        })
        .collect();
    json!({
        "output_dir": output_dir.map(|p| p.display().to_string()),
        "collections": stats.items_built,
        "pages_written": stats.items_built.values().sum::<usize>(),
        "static_files_copied": stats.static_files_copied,
        "public_files_copied": stats.public_files_copied,
        "data_files_loaded": stats.data_files_loaded,
        "duration_ms": stats.duration_ms,
        "timings_ms": timings,
        "broken_links": broken_links,
        "subdomains": subdomains,
        "warnings": json_out::warnings(),
    })
}
