use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use clap::Args;

use crate::build::{self, BuildOptions};
use crate::cli::agent;
use crate::config::{self, SiteConfig};
use crate::content::{self, Frontmatter};
use crate::output::human;
use crate::output::CommandOutput;
use crate::server;
use crate::workspace;

#[derive(Args)]
pub struct ServeArgs {
    /// Port to serve on (auto-finds available port if default is taken)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Build before serving
    #[arg(long, default_value = "true")]
    pub build: bool,
}

const DEFAULT_PORT: u16 = 3000;

pub fn run(args: &ServeArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    // Check for workspace context
    if let Some(ws_root) = workspace::find_workspace_root(&cwd) {
        let ws_config =
            workspace::WorkspaceConfig::load(&ws_root.join("page-workspace.toml"))?;

        // Build all sites first
        if args.build {
            human::info("Building workspace...");
            let build_opts = workspace::build::WorkspaceBuildOptions {
                include_drafts: true,
                strict: false,
                site_filter: site_filter.map(String::from),
            };
            workspace::build::build_workspace(&ws_config, &ws_root, &build_opts)?;
        }

        let port = args.port.unwrap_or(DEFAULT_PORT);
        let auto_increment = args.port.is_none();

        // If --site is specified, serve only that site in standalone mode
        if let Some(site_name) = site_filter {
            let ws_site = ws_config.find_site(site_name).ok_or_else(|| {
                anyhow::anyhow!("unknown site '{site_name}' in workspace")
            })?;
            let (config, paths) = workspace::load_site_in_workspace(&ws_root, ws_site)?;
            let handle = server::start(&config, &paths, port, true, auto_increment)?;

            human::info(&format!(
                "Serving site '{site_name}'. Type \"help\" for commands, \"stop\" to quit (port {})",
                handle.port()
            ));

            run_repl(&config, &paths, &handle)?;
            return Ok(());
        }

        // Workspace dev server (all sites)
        let handle = workspace::server::start(&ws_config, &ws_root, port, auto_increment)?;

        human::info(&format!(
            "Type \"stop\" to quit (server on port {})",
            handle.port()
        ));

        let stdin = io::stdin();
        let reader = stdin.lock();
        print_prompt();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let line = line.trim().to_string();
            if line.is_empty() {
                print_prompt();
                continue;
            }

            match line.as_str() {
                "stop" | "quit" | "exit" => {
                    handle.stop();
                    human::info("Server stopped");
                    break;
                }
                "status" => {
                    human::info(&format!("Workspace: {}", ws_config.workspace.name));
                    for site in &ws_config.sites {
                        human::info(&format!("  /{} -> {}", site.name, site.path));
                    }
                }
                "help" => {
                    println!("  status                         Show workspace sites");
                    println!("  stop                           Stop the server and exit");
                }
                _ => {
                    human::error(&format!(
                        "Unknown command: {line}. Type \"help\" for available commands."
                    ));
                }
            }
            print_prompt();
        }

        return Ok(());
    }

    // Standalone mode
    if site_filter.is_some() {
        human::warning("--site flag ignored (not in a workspace)");
    }

    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&cwd);

    if args.build {
        human::info("Building site...");
        let opts = BuildOptions {
            include_drafts: true,
        };
        let result = build::build_site(&config, &paths, &opts)?;
        human::success(&result.stats.human_display());
    }

    let port = args.port.unwrap_or(DEFAULT_PORT);
    let auto_increment = args.port.is_none();
    let handle = server::start(&config, &paths, port, true, auto_increment)?;

    human::info(&format!(
        "Type \"help\" for commands, \"stop\" to quit (server on port {})",
        handle.port()
    ));

    run_repl(&config, &paths, &handle)?;

    Ok(())
}

fn run_repl(
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    handle: &server::ServerHandle,
) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let reader = stdin.lock();
    print_prompt();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            print_prompt();
            continue;
        }

        match dispatch(&line, config, paths) {
            LoopAction::Continue => {}
            LoopAction::Stop => {
                handle.stop();
                human::info("Server stopped");
                break;
            }
        }
        print_prompt();
    }

    Ok(())
}

fn print_prompt() {
    print!("page> ");
    let _ = io::stdout().flush();
}

enum LoopAction {
    Continue,
    Stop,
}

fn dispatch(line: &str, config: &SiteConfig, paths: &crate::config::ResolvedPaths) -> LoopAction {
    let parts = shell_split(line);
    if parts.is_empty() {
        return LoopAction::Continue;
    }

    let cmd = parts[0].as_str();
    let args = &parts[1..];

    match cmd {
        "stop" | "quit" | "exit" => return LoopAction::Stop,

        "help" => {
            println!("  new <collection> <title> [--lang <code>]  Create new content");
            println!("  agent [prompt]                 Start an AI agent session (or run a single prompt)");
            println!("  theme <name>                   Apply a bundled theme");
            println!("  build [--drafts]               Rebuild the site");
            println!("  status                         Show server info");
            println!("  stop                           Stop the server and exit");
        }

        "build" => {
            let include_drafts = args.iter().any(|a| a == "--drafts");
            let opts = BuildOptions { include_drafts };
            match build::build_site(config, paths, &opts) {
                Ok(result) => human::success(&result.stats.human_display()),
                Err(e) => human::error(&format!("Build failed: {e}")),
            }
        }

        "new" => {
            if args.len() < 2 {
                human::error("Usage: new <collection> <title> [--lang <code>]");
                return LoopAction::Continue;
            }
            let collection_name = &args[0];
            // Parse optional --lang flag from the remaining args
            let mut title_parts = Vec::new();
            let mut lang_arg: Option<String> = None;
            let mut skip_next = false;
            for (i, arg) in args[1..].iter().enumerate() {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg == "--lang" {
                    if let Some(next) = args[1..].get(i + 1) {
                        lang_arg = Some(next.clone());
                        skip_next = true;
                    }
                } else {
                    title_parts.push(arg.as_str());
                }
            }
            let title = title_parts.join(" ");
            cmd_new(config, paths, collection_name, &title, lang_arg.as_deref());
        }

        "agent" => {
            cmd_agent(config, paths, args);
        }

        "theme" => {
            if args.is_empty() {
                // List bundled themes
                for t in crate::themes::all() {
                    println!("  {} - {}", console::style(t.name).bold(), t.description);
                }
                // List installed themes
                let project_root = std::path::PathBuf::from(".");
                let installed = crate::themes::installed_themes(&project_root);
                if !installed.is_empty() {
                    println!();
                    println!("  {}", console::style("Installed:").underlined());
                    for t in &installed {
                        println!("  {} - {}", console::style(&t.name).bold().cyan(), t.description);
                    }
                }
            } else {
                let name = &args[0];
                // Try bundled first, then installed
                let template_content: Option<String> = crate::themes::by_name(name)
                    .map(|t| t.base_html.to_string())
                    .or_else(|| {
                        let project_root = std::path::PathBuf::from(".");
                        crate::themes::installed_by_name(&project_root, name)
                            .map(|t| t.base_html)
                    });

                match template_content {
                    Some(content) => {
                        let template_dir = paths.templates.clone();
                        let _ = std::fs::create_dir_all(&template_dir);
                        match std::fs::write(template_dir.join("base.html"), content) {
                            Ok(()) => {
                                human::success(&format!("Applied theme '{name}'"));
                                // Rebuild site to reflect the new theme
                                human::info("Rebuilding site...");
                                let opts = BuildOptions { include_drafts: true };
                                match build::build_site(config, paths, &opts) {
                                    Ok(result) => human::success(&result.stats.human_display()),
                                    Err(e) => human::error(&format!("Rebuild failed: {e}")),
                                }
                            }
                            Err(e) => human::error(&format!("Failed to apply theme: {e}")),
                        }
                    }
                    None => human::error(&format!("Unknown theme '{name}'. Type 'theme' to list available themes.")),
                }
            }
        }

        "status" => {
            println!("  Site: {}", config.site.title);
            println!("  Collections: {}", config.collections.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
        }

        _ => {
            human::error(&format!("Unknown command: {cmd}. Type \"help\" for available commands."));
        }
    }

    LoopAction::Continue
}

fn cmd_new(
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    collection_name: &str,
    title: &str,
    lang: Option<&str>,
) {
    let collection = match config::find_collection(collection_name, &config.collections) {
        Some(c) => c,
        None => {
            human::error(&format!(
                "Unknown collection '{}'. Available: {}",
                collection_name,
                config.collections.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            ));
            return;
        }
    };

    // Resolve language suffix
    let lang_suffix = if let Some(lang) = lang {
        if lang == config.site.language {
            None // default language doesn't need suffix
        } else if config.languages.contains_key(lang) {
            Some(lang)
        } else {
            human::error(&format!(
                "Unknown language '{}'. Configured: {}",
                lang,
                config.all_languages().join(", ")
            ));
            return;
        }
    } else {
        None
    };

    let slug = content::slug_from_title(title);
    let date = if collection.has_date {
        Some(chrono::Local::now().date_naive())
    } else {
        None
    };

    let fm = Frontmatter {
        title: title.to_string(),
        date,
        ..Default::default()
    };

    let filename = if collection.has_date {
        let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
        if let Some(lang) = lang_suffix {
            format!("{date_str}-{slug}.{lang}.md")
        } else {
            format!("{date_str}-{slug}.md")
        }
    } else if let Some(lang) = lang_suffix {
        format!("{slug}.{lang}.md")
    } else {
        format!("{slug}.md")
    };

    let filepath = paths.content.join(&collection.directory).join(&filename);
    if let Err(e) = std::fs::create_dir_all(filepath.parent().unwrap()) {
        human::error(&format!("Failed to create directory: {e}"));
        return;
    }

    let file_content = format!(
        "{}\n\nWrite your content here.\n",
        content::generate_frontmatter(&fm)
    );
    match std::fs::write(&filepath, file_content) {
        Ok(()) => human::success(&format!("Created {}", filepath.display())),
        Err(e) => human::error(&format!("Failed to write file: {e}")),
    }
}

fn cmd_agent(
    _config: &SiteConfig,
    _paths: &crate::config::ResolvedPaths,
    args: &[String],
) {
    let agent_args = agent::AgentArgs {
        prompt: if args.is_empty() {
            None
        } else {
            Some(args.join(" "))
        },
        once: false,
    };
    if let Err(e) = agent::run(&agent_args) {
        human::error(&format!("Agent failed: {e}"));
    }
}

/// Split a line into tokens, respecting double quotes.
/// Note: does not handle escaped quotes (\") or single quotes — sufficient for REPL use.
fn shell_split(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
