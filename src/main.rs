use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use page::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Change working directory if --dir is specified
    if let Some(ref dir) = cli.dir {
        std::env::set_current_dir(dir)?;
    }

    match &cli.command {
        Command::Init(args) => page::cli::init::run(args)?,
        Command::New(args) => page::cli::new::run(args)?,
        Command::Build(args) => page::cli::build::run(args, cli.site.as_deref())?,
        Command::Serve(args) => page::cli::serve::run(args, cli.site.as_deref())?,
        Command::Deploy(args) => page::cli::deploy::run(args, cli.site.as_deref())?,
        Command::Agent(args) => page::cli::agent::run(args)?,
        Command::Collection(args) => page::cli::collection::run(args)?,
        Command::Theme(args) => page::cli::theme::run(args)?,
        Command::Workspace(args) => page::cli::workspace::run(args)?,
        Command::Upgrade(args) => page::cli::upgrade::run(args)?,
        Command::SelfUpdate(args) => page::cli::self_update::run(args)?,
        Command::Mcp(args) => page::cli::mcp::run(args)?,
    }

    Ok(())
}
