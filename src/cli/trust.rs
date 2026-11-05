//! `seite trust [--revoke] [--list]`: record (per user, outside the
//! repository) that this project may run its own code at build time —
//! today Tailwind CSS, whose `@plugin`/`@config` run project JavaScript.
//! Running the command is the explicit act; it never prompts. Generated
//! agent permissions never allow it (see `cli::harness`).

use clap::Args;

use crate::output::human;

#[derive(Args)]
pub struct TrustArgs {
    /// Stop trusting this project
    #[arg(long, conflicts_with = "list")]
    pub revoke: bool,

    /// List the trusted projects
    #[arg(long)]
    pub list: bool,
}

pub fn run(args: &TrustArgs) -> anyhow::Result<()> {
    let file = crate::trust::trust_file().ok_or_else(|| {
        anyhow::anyhow!("no user config directory (HOME/APPDATA is not set); cannot record trust")
    })?;
    if args.list {
        let projects = crate::trust::list_in(&file);
        if projects.is_empty() {
            human::info("No trusted projects.");
        }
        for project in &projects {
            crate::human_println!("{}", project);
        }
        crate::output::json::set_data(serde_json::json!({
            "file": file.display().to_string(),
            "projects": projects,
        }));
        return Ok(());
    }
    let cwd = std::env::current_dir()?;
    if !cwd.join("seite.toml").is_file() {
        anyhow::bail!("no seite.toml here: run `seite trust` from the site's root");
    }
    let root = cwd.canonicalize()?;
    let changed = crate::trust::set_trusted_in(&file, &root, !args.revoke)?;
    if args.revoke {
        human::success(&format!(
            "{} is no longer trusted: `seite build` will not run Tailwind CSS for it",
            root.display()
        ));
    } else {
        human::success(&format!("Trusted {}", root.display()));
        human::info(
            "`seite build`/`serve` may now run Tailwind CSS here; Tailwind plugins (`@plugin`, `@config`) run this project's JavaScript.",
        );
    }
    crate::output::json::set_data(serde_json::json!({
        "root": root.display().to_string(),
        "trusted": !args.revoke,
        "changed": changed,
        "file": file.display().to_string(),
    }));
    Ok(())
}
