pub mod access;
pub mod agent;
mod agent_instructions;
pub mod build;
pub mod collection;
pub mod completions;
pub mod contact;
pub mod deploy;
pub mod harness;
pub mod init;
pub mod mcp;
pub mod new;
pub mod perf;
pub mod prompt;
pub mod self_update;
pub mod serve;
pub mod skill;
pub mod telemetry;
pub mod theme;
pub mod upgrade;
pub mod workspace;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "seite",
    about = "A static site generator with LLM integration",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Verbose output (debug logging, per-step build timings)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Machine-readable output: print exactly one JSON document on stdout
    /// ({"ok":true,"command":..,"data":..} or {"ok":false,..,"error":{"message":..,"chain":[..]}}),
    /// with all human output on stderr. Not supported by serve, agent, mcp,
    /// completions and self-update (they stream output or take over the terminal).
    #[arg(long, global = true)]
    pub json: bool,

    /// Never prompt: accept defaults and answer "yes" to confirmations
    /// (also enabled by SEITE_YES=1). Without a terminal, prompts use their
    /// defaults and required values must be passed as flags.
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Path to the project's seite.toml (runs the command in that file's
    /// directory; applied after --dir). Custom config file names are not supported.
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

    /// Manage private password access on Cloudflare Pages
    Access(access::AccessArgs),

    /// Manage Claude Code skills and skill packs
    Skill(skill::SkillArgs),

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

    /// Audit site performance via PageSpeed Insights
    Perf(perf::PerfArgs),

    /// Generate shell completions
    Completions(completions::CompletionsArgs),

    /// Manage anonymous usage telemetry
    Telemetry(telemetry::TelemetryArgs),
}

/// Build the clap Command (used by shell completion generation).
pub fn build_cli() -> clap::Command {
    Cli::command()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cli_returns_valid_command() {
        let cmd = build_cli();
        assert_eq!(cmd.get_name(), "seite");
    }

    #[test]
    fn test_build_cli_has_subcommands() {
        let cmd = build_cli();
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();
        assert!(subcommands.contains(&"init"));
        assert!(subcommands.contains(&"build"));
        assert!(subcommands.contains(&"serve"));
        assert!(subcommands.contains(&"completions"));
        assert!(subcommands.contains(&"access"));
    }
}
