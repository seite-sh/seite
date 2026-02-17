pub mod agent;
pub mod build;
pub mod deploy;
pub mod init;
pub mod new;
pub mod serve;
pub mod theme;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "page",
    about = "A static site generator with LLM integration",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Enable verbose logging output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output results as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Path to config file
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    /// Project directory
    #[arg(short, long, global = true)]
    pub dir: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new site project
    Init(init::InitArgs),

    /// Create new content
    New(new::NewArgs),

    /// Build the site
    Build(build::BuildArgs),

    /// Start a local development server
    Serve(serve::ServeArgs),

    /// Deploy the site to a hosting provider
    Deploy(deploy::DeployArgs),

    /// Start an AI agent session with full site context
    Agent(agent::AgentArgs),

    /// Manage themes
    Theme(theme::ThemeArgs),
}
