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

    /// Treat broken internal links and missing assets as build errors
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
    let problems = print_link_report(&result.link_check, args.strict, None);
    if args.strict && problems > 0 {
        json_out::set_data(build_data(&result, Some(&paths.output)));
        anyhow::bail!("Build failed: {}", problem_summary(&result.link_check));
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
    let broken_links = grouped_json(&result.link_check.broken_links);
    let missing_assets = grouped_json(&result.link_check.missing_assets);
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
        "links_checked": result.link_check.total_links_checked,
        "broken_links": broken_links,
        "missing_assets": missing_assets,
        "subdomains": subdomains,
        "warnings": json_out::warnings(),
    })
}

/// Broken links / missing assets grouped by target for the `--json`
/// envelope. `sources` lists human-readable locations (kept for
/// compatibility); `locations` has the structured source file + line.
fn grouped_json(links_: &[links::BrokenLink]) -> Vec<Value> {
    links::group_by_target(links_)
        .into_iter()
        .map(|(target, group)| {
            let sources: Vec<String> = group.iter().map(|l| l.location()).collect();
            let mut locations: Vec<Value> = Vec::new();
            for link in &group {
                locations.push(json!({
                    "source": link.file(),
                    "line": link.line,
                    "page": link.source_file,
                    "from_template": link.from_template,
                }));
            }
            let first = group[0];
            json!({
                "target": target,
                "kind": first.kind,
                "sources": sources,
                "locations": locations,
                "suggestion": first.suggestion,
                // Location of the first occurrence, for convenience.
                "source": first.file(),
                "line": first.line,
            })
        })
        .collect()
}

/// e.g. `2 broken internal links, 1 missing asset`.
pub fn problem_summary(check: &links::LinkCheckResult) -> String {
    let mut parts = Vec::new();
    let broken = links::distinct_links(&check.broken_links).len();
    if broken > 0 {
        parts.push(format!(
            "{broken} broken internal link{}",
            if broken == 1 { "" } else { "s" }
        ));
    }
    let missing = links::distinct_links(&check.missing_assets).len();
    if missing > 0 {
        parts.push(format!(
            "{missing} missing asset{}",
            if missing == 1 { "" } else { "s" }
        ));
    }
    parts.join(", ")
}

/// Print broken links and missing assets grouped by target, each with the
/// source file (and line) it was written in. Warnings normally, errors with
/// `strict`. `site` prefixes the headers in workspace builds. Returns the
/// number of problems printed.
pub fn print_link_report(
    check: &links::LinkCheckResult,
    strict: bool,
    site: Option<&str>,
) -> usize {
    let prefix = site.map(|s| format!("Site '{s}': ")).unwrap_or_default();
    let sections = [
        (&check.broken_links, "broken internal link", "broken target"),
        (
            &check.missing_assets,
            "missing asset reference",
            "missing file",
        ),
    ];
    for (items, noun, target_noun) in sections {
        if items.is_empty() {
            continue;
        }
        let grouped = links::group_by_target(items);
        let count: usize = grouped.iter().map(|(_, group)| group.len()).sum();
        let target_count = grouped.len();
        let header = format!(
            "{prefix}Found {count} {noun}{} ({target_count} {target_noun}{})",
            if count == 1 { "" } else { "s" },
            if target_count == 1 { "" } else { "s" },
        );
        if strict {
            human::error(&header);
        } else {
            human::warning(&header);
        }
        for (href, group) in &grouped {
            let locations: Vec<String> = group.iter().map(|l| l.location()).collect();
            let hint = group[0]
                .suggestion
                .as_deref()
                .map(|s| format!(" — did you mean {s}?"))
                .unwrap_or_default();
            human::info(&format!(
                "  {} (linked from {} file{}){hint}",
                href,
                locations.len(),
                if locations.len() == 1 { "" } else { "s" }
            ));
            for loc in &locations {
                human::info(&format!("    - {loc}"));
            }
        }
    }
    check.problem_count()
}
