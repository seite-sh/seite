use clap::Args;

#[derive(Args)]
pub struct InitArgs {
    /// Name of the site / directory to create
    pub name: Option<String>,

    /// Site title
    #[arg(long)]
    pub title: Option<String>,

    /// Site description
    #[arg(long)]
    pub description: Option<String>,

    /// Deploy target (github-pages, cloudflare)
    #[arg(long)]
    pub deploy_target: Option<String>,
}

pub fn run(args: &InitArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page init is not yet implemented");
    Ok(())
}
