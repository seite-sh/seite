use clap::Args;

#[derive(Args)]
pub struct BuildArgs {
    /// Include draft content in the build
    #[arg(long)]
    pub drafts: bool,
}

pub fn run(args: &BuildArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page build is not yet implemented");
    Ok(())
}
