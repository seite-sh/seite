use std::io;

use clap::Args;
use clap_complete::{generate, Shell};

#[derive(Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub fn run(args: &CompletionsArgs) -> anyhow::Result<()> {
    let mut cmd = crate::cli::build_cli();
    generate(args.shell, &mut cmd, "seite", &mut io::stdout());

    eprintln!();
    eprintln!(
        "{} Add the output above to your shell config to enable completions for {}",
        console::style("ℹ").blue().bold(),
        args.shell
    );
    Ok(())
}
