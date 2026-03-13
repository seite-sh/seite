use std::io;

use clap::Args;
use clap_complete::{generate, Shell};

use crate::output::human;

#[derive(Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run(args: &CompletionsArgs) -> anyhow::Result<()> {
    let mut cmd = crate::cli::build_cli();
    generate(args.shell, &mut cmd, "seite", &mut io::stdout());

    eprintln!();
    human::info(&format!(
        "Add the output above to your shell config to enable completions for {}",
        args.shell
    ));
    Ok(())
}
