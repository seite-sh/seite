use clap::{Args, Subcommand};

#[derive(Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Add credentials for an LLM provider
    Add {
        /// Provider name (claude, openai)
        provider: String,

        /// API key (if not provided, will prompt interactively)
        #[arg(long)]
        key: Option<String>,
    },

    /// List configured providers
    List,

    /// Remove credentials for a provider
    Remove {
        /// Provider name to remove
        provider: String,
    },
}

pub fn run(args: &AuthArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page auth is not yet implemented");
    Ok(())
}
