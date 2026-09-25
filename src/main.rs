use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use seite::cli::{Cli, Command};
use seite::output::json;

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Process-global flags, readable from deep inside commands.
    seite::output::set_json_mode(cli.json);
    seite::output::set_verbose(cli.verbose);
    seite::cli::prompt::set_assume_yes(cli.yes);
    if cli.json {
        // stdout carries exactly one JSON document; everything else → stderr.
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
        json::redirect_stdout_to_stderr();
    }

    // Set up logging (always on stderr so stdout stays parseable)
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let cmd_name = cli.command.as_ref().map(command_name).unwrap_or("seite");
    let result = run(&cli);
    finish(cmd_name, result)
}

/// Apply global flags, then dispatch the subcommand (with telemetry and the
/// update check).
fn run(cli: &Cli) -> Result<()> {
    // Change working directory if --dir is specified
    if let Some(ref dir) = cli.dir {
        std::env::set_current_dir(dir)
            .with_context(|| format!("cannot change to --dir '{dir}'"))?;
    }
    if let Some(ref config) = cli.config {
        apply_config_flag(config)?;
    }

    let Some(command) = cli.command.as_ref() else {
        if cli.json {
            anyhow::bail!("no command given (run `seite --help` to see commands)");
        }
        // No subcommand: show welcome or help
        print_welcome();
        return Ok(());
    };

    if cli.json && !supports_json(command) {
        anyhow::bail!(
            "--json is not supported by `seite {}` (it streams output or takes over the terminal)",
            command_name(command)
        );
    }

    use std::time::Instant;

    let cmd_name = command_name(command);
    let started = Instant::now();
    let result = dispatch(cli.site.as_deref(), command);
    let elapsed = started.elapsed();
    let success = result.is_ok();

    // Telemetry + update check share the same exclusion list (stdout/stderr must
    // stay clean for these commands). The `telemetry` command is also excluded
    // so it doesn't report itself.
    let skip = matches!(
        command,
        Command::SelfUpdate(_)
            | Command::Mcp(_)
            | Command::Perf(_)
            | Command::Completions(_)
            | Command::Telemetry(_)
    );
    if !skip {
        seite::telemetry::maybe_record_command(cmd_name, success, elapsed);
        seite::update_check::maybe_notify();
    }

    result
}

/// Render the command's outcome (JSON envelope or human error) and pick the
/// process exit code.
fn finish(cmd_name: &str, result: Result<()>) -> ExitCode {
    let json_mode = seite::output::is_json();
    match result {
        Ok(()) => {
            if json_mode {
                let doc = json::success_document(cmd_name, json::take_data(), json::warnings());
                json::emit_document(&doc);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            let (message, chain) = json::error_chain(&err);
            if json_mode {
                json::emit_document(&json::error_document(cmd_name, &err, json::warnings()));
                // The document carries the error; only echo it with --verbose.
                if !seite::output::is_verbose() {
                    return ExitCode::FAILURE;
                }
            }
            // Human-readable error goes to stderr: every diagnostic on its own
            // compiler-style line, then the summary.
            if let Some(diagnostics) = json::find_diagnostics(&err) {
                for diagnostic in diagnostics.iter() {
                    eprintln!("{diagnostic}");
                }
                eprintln!();
            }
            eprintln!("Error: {message}");
            if !chain.is_empty() {
                eprintln!("\nCaused by:");
                for cause in &chain {
                    eprintln!("    {cause}");
                }
            }
            ExitCode::FAILURE
        }
    }
}

/// Commands that stream output or take over the terminal can't produce a
/// single JSON document, so `--json` is rejected for them.
fn supports_json(command: &Command) -> bool {
    !matches!(
        command,
        Command::Serve(_)
            | Command::Agent(_)
            | Command::Mcp(_)
            | Command::Completions(_)
            | Command::SelfUpdate(_)
    )
}

/// `--config <path>`: only `seite.toml` files are supported. Run the command
/// from that file's directory (after `--dir` has been applied).
fn apply_config_flag(config: &str) -> Result<()> {
    let path = Path::new(config);
    if !path.is_file() {
        anyhow::bail!("--config file not found: {config}");
    }
    if path.file_name().and_then(|n| n.to_str()) != Some("seite.toml") {
        anyhow::bail!(
            "custom config file names are not supported; point --config at a seite.toml or use --dir"
        );
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::env::set_current_dir(parent)
            .with_context(|| format!("cannot change to directory of --config '{config}'"))?;
    }
    Ok(())
}

/// Dispatch a parsed command. Returns the command's result so `main` can record
/// success/failure for telemetry before propagating.
fn dispatch(site: Option<&str>, command: &Command) -> anyhow::Result<()> {
    match command {
        Command::Init(args) => seite::cli::init::run(args),
        Command::New(args) => seite::cli::new::run(args),
        Command::Build(args) => seite::cli::build::run(args, site),
        Command::Check(args) => seite::cli::check::run(args, site),
        Command::Serve(args) => seite::cli::serve::run(args, site),
        Command::Deploy(args) => seite::cli::deploy::run(args, site),
        Command::Agent(args) => seite::cli::agent::run(args),
        Command::Collection(args) => seite::cli::collection::run(args),
        Command::Contact(args) => seite::cli::contact::run(args),
        Command::Access(args) => seite::cli::access::run(args),
        Command::Skill(args) => seite::cli::skill::run(args),
        Command::Theme(args) => seite::cli::theme::run(args),
        Command::Workspace(args) => seite::cli::workspace::run(args),
        Command::Upgrade(args) => seite::cli::upgrade::run(args),
        Command::SelfUpdate(args) => seite::cli::self_update::run(args),
        Command::Mcp(args) => seite::cli::mcp::run(args),
        Command::Perf(args) => seite::cli::perf::run(args),
        Command::Completions(args) => seite::cli::completions::run(args),
        Command::Telemetry(args) => seite::cli::telemetry::run(args),
    }
}

/// Map a command to its stable telemetry name (the clap subcommand string).
fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Init(_) => "init",
        Command::New(_) => "new",
        Command::Build(_) => "build",
        Command::Check(_) => "check",
        Command::Serve(_) => "serve",
        Command::Deploy(_) => "deploy",
        Command::Agent(_) => "agent",
        Command::Collection(_) => "collection",
        Command::Contact(_) => "contact",
        Command::Access(_) => "access",
        Command::Skill(_) => "skill",
        Command::Theme(_) => "theme",
        Command::Workspace(_) => "workspace",
        Command::Upgrade(_) => "upgrade",
        Command::SelfUpdate(_) => "self-update",
        Command::Mcp(_) => "mcp",
        Command::Perf(_) => "perf",
        Command::Completions(_) => "completions",
        Command::Telemetry(_) => "telemetry",
    }
}

/// Show a friendly welcome screen when no subcommand is given.
fn print_welcome() {
    use console::style;

    let version = env!("CARGO_PKG_VERSION");
    let has_project = std::path::Path::new("seite.toml").exists();

    println!();
    println!(
        "  {} {}",
        style("seite").bold().cyan(),
        style(format!("v{version}")).dim()
    );
    println!("  {}", style("AI-native static site generator").dim());
    println!();

    if has_project {
        println!("  {}", style("Commands:").bold());
        println!("    seite build              Build the site");
        println!("    seite serve              Start dev server with live reload");
        println!("    seite new post \"Title\"   Create a new post");
        println!("    seite deploy             Deploy to production");
        println!("    seite agent \"prompt\"     AI assistant with full site context");
        println!("    seite --help             See all commands");
    } else {
        println!("  {}", style("Get started:").bold());
        println!("    seite init mysite        Create a new site");
        println!("    seite --help             See all commands");
        println!();
        println!(
            "  {}  {}",
            style("Docs:").bold(),
            style("https://seite.sh/docs").dim()
        );
    }
    println!();
}
