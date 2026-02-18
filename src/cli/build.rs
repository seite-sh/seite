use std::path::PathBuf;

use clap::Args;

use crate::build::{self, links, BuildOptions};
use crate::config::SiteConfig;
use crate::output::human;
use crate::output::CommandOutput;

#[derive(Args)]
pub struct BuildArgs {
    /// Include draft content in the build
    #[arg(long)]
    pub drafts: bool,

    /// Treat broken internal links as build errors
    #[arg(long)]
    pub strict: bool,
}

pub fn run(args: &BuildArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

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
            human::info(&format!("  {} (linked from {} file{})", href, sources.len(), if sources.len() == 1 { "" } else { "s" }));
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
