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

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // No subcommand: show welcome or help
            print_welcome();
            return Ok(());
        }
    };

    match &command {
        Command::Init(args) => seite::cli::init::run(args)?,
        Command::New(args) => seite::cli::new::run(args)?,
        Command::Build(args) => seite::cli::build::run(args, cli.site.as_deref())?,
        Command::Serve(args) => seite::cli::serve::run(args, cli.site.as_deref())?,
        Command::Deploy(args) => seite::cli::deploy::run(args, cli.site.as_deref())?,
        Command::Agent(args) => seite::cli::agent::run(args)?,
        Command::Collection(args) => seite::cli::collection::run(args)?,
        Command::Contact(args) => seite::cli::contact::run(args)?,
        Command::Skill(args) => seite::cli::skill::run(args)?,
        Command::Theme(args) => seite::cli::theme::run(args)?,
        Command::Workspace(args) => seite::cli::workspace::run(args)?,
        Command::Upgrade(args) => seite::cli::upgrade::run(args)?,
        Command::SelfUpdate(args) => seite::cli::self_update::run(args)?,
        Command::Mcp(args) => seite::cli::mcp::run(args)?,
        Command::Perf(args) => seite::cli::perf::run(args)?,
        Command::Completions(args) => seite::cli::completions::run(args)?,
    }

    // Check for available updates (skip for self-update, mcp, and completions — stdout must stay clean)
    if !matches!(
        &command,
        Command::SelfUpdate(_) | Command::Mcp(_) | Command::Perf(_) | Command::Completions(_)
    ) {
        seite::update_check::maybe_notify();
    }

    Ok(())
}

/// Show a friendly welcome screen when no subcommand is given.
fn print_welcome() {
    use console::style;

    let version = env!("CARGO_PKG_VERSION");
    let has_project = std::path::Path::new("seite.toml").exists();

    println!();
    println!(
        "  {} {}",
        style("seite").bold().cyan(),
        style(format!("v{version}")).dim()
    );
    println!("  {}", style("AI-native static site generator").dim());
    println!();

    if has_project {
        println!("  {}", style("Commands:").bold());
        println!("    seite build              Build the site");
        println!("    seite serve              Start dev server with live reload");
        println!("    seite new post \"Title\"   Create a new post");
        println!("    seite deploy             Deploy to production");
        println!("    seite agent \"prompt\"     AI assistant with full site context");
        println!("    seite --help             See all commands");
    } else {
        println!("  {}", style("Get started:").bold());
        println!("    seite init mysite        Create a new site");
        println!("    seite --help             See all commands");
        println!();
        println!(
            "  {}  {}",
            style("Docs:").bold(),
            style("https://seite.sh/docs").dim()
        );
    }
    println!();
}
