use clap::Args;

#[derive(Args)]
pub struct AiArgs {
    /// The prompt describing what content to generate
    pub prompt: String,

    /// LLM provider to use (overrides config default)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Output file path (default: auto-generated from prompt)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Content type to generate
    #[arg(short, long, default_value = "post")]
    pub r#type: String,
}

pub fn run(args: &AiArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page ai is not yet implemented");
    Ok(())
}
