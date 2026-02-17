use clap::Args;

#[derive(Args)]
pub struct DeployArgs {
    /// Deploy target override (github-pages, cloudflare)
    #[arg(short, long)]
    pub target: Option<String>,

    /// Build before deploying
    #[arg(long, default_value = "true")]
    pub build: bool,
}

pub fn run(args: &DeployArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page deploy is not yet implemented");
    Ok(())
}
