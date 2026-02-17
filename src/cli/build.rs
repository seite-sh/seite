use std::path::PathBuf;

use clap::Args;

use crate::build::{self, BuildOptions};
use crate::config::SiteConfig;
use crate::output::human;
use crate::output::CommandOutput;

#[derive(Args)]
pub struct BuildArgs {
    /// Include draft content in the build
    #[arg(long)]
    pub drafts: bool,
}

pub fn run(args: &BuildArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

    let opts = BuildOptions {
        include_drafts: args.drafts,
    };

    let result = build::build_site(&config, &paths, &opts)?;
    human::success(&result.stats.human_display());

    Ok(())
}
