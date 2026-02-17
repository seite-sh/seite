use clap::{Args, Subcommand};

#[derive(Args)]
pub struct NewArgs {
    #[command(subcommand)]
    pub kind: NewKind,
}

#[derive(Subcommand)]
pub enum NewKind {
    /// Create a new blog post
    Post {
        /// Title of the post
        title: String,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,

        /// Mark as draft
        #[arg(long)]
        draft: bool,
    },

    /// Create a new page
    Page {
        /// Title of the page
        title: String,
    },
}

pub fn run(args: &NewArgs) -> anyhow::Result<()> {
    let _ = args;
    crate::output::human::info("page new is not yet implemented");
    Ok(())
}
