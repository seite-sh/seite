use clap::Args;

#[derive(Args)]
pub struct ServeArgs {
    /// Port to serve on
    #[arg(short, long, default_value = "3000")]
    pub port: u16,

    /// Build before serving
    #[arg(long, default_value = "true")]
    pub build: bool,
}

pub fn run(args: &ServeArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page serve is not yet implemented");
    Ok(())
}
