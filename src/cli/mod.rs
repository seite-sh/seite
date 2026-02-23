pub mod agent;
pub mod build;
pub mod collection;
pub mod contact;
pub mod deploy;
pub mod init;
pub mod mcp;
pub mod new;
pub mod self_update;
pub mod serve;
pub mod theme;
pub mod upgrade;
pub mod workspace;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "seite",
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

    /// Target a specific site in a workspace
    #[arg(short, long, global = true)]
    pub site: Option<String>,
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

    /// Manage collections
    Collection(collection::CollectionArgs),

    /// Manage contact form configuration
    Contact(contact::ContactArgs),

    /// Manage themes
    Theme(theme::ThemeArgs),

    /// Manage multi-site workspaces
    Workspace(workspace::WorkspaceArgs),

    /// Upgrade project config to match the current seite version
    Upgrade(upgrade::UpgradeArgs),

    /// Update the seite binary to the latest release
    SelfUpdate(self_update::SelfUpdateArgs),

    /// Start MCP server for AI tool integration (stdio JSON-RPC)
    Mcp(mcp::McpArgs),
}
