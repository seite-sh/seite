use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use seite::cli::{Cli, Command};

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
        Command::Init(args) => seite::cli::init::run(args)?,
        Command::New(args) => seite::cli::new::run(args)?,
        Command::Build(args) => seite::cli::build::run(args, cli.site.as_deref())?,
        Command::Serve(args) => seite::cli::serve::run(args, cli.site.as_deref())?,
        Command::Deploy(args) => seite::cli::deploy::run(args, cli.site.as_deref())?,
        Command::Agent(args) => seite::cli::agent::run(args)?,
        Command::Collection(args) => seite::cli::collection::run(args)?,
        Command::Theme(args) => seite::cli::theme::run(args)?,
        Command::Workspace(args) => seite::cli::workspace::run(args)?,
        Command::Upgrade(args) => seite::cli::upgrade::run(args)?,
        Command::SelfUpdate(args) => seite::cli::self_update::run(args)?,
        Command::Mcp(args) => seite::cli::mcp::run(args)?,
    }

    Ok(())
}
