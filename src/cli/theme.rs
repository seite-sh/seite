use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand)]
pub enum ThemeCommand {
    /// List available themes
    List,
}

pub fn run(args: &ThemeArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page theme is not yet implemented");
    Ok(())
}
