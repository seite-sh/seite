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
        Command::Build(args) => page::cli::build::run(args)?,
        Command::Serve(args) => page::cli::serve::run(args)?,
        Command::Deploy(args) => page::cli::deploy::run(args)?,
        Command::Auth(args) => page::cli::auth::run(args)?,
        Command::Ai(args) => page::cli::ai::run(args)?,
        Command::Theme(args) => page::cli::theme::run(args)?,
    }

    Ok(())
}
