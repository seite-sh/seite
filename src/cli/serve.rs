use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use clap::Args;

use crate::ai::{AiClient, Provider};
use crate::build::{self, BuildOptions};
use crate::cli::auth;
use crate::config::{self, SiteConfig};
use crate::content::{self, Frontmatter};
use crate::credential;
use crate::output::human;
use crate::output::CommandOutput;
use crate::server;

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

pub fn run(args: &ServeArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);

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

        match dispatch(&line, &config, &paths) {
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
            println!("  new <collection> <title>       Create new content");
            println!("  ai <prompt> [--type <type>]    Generate content/template with AI");
            println!("  login [provider]               Authenticate with an AI provider");
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
                human::error("Usage: new <collection> <title>");
                return LoopAction::Continue;
            }
            let collection_name = &args[0];
            let title = args[1..].join(" ");
            cmd_new(config, paths, collection_name, &title);
        }

        "ai" => {
            if args.is_empty() {
                human::error("Usage: ai <prompt> [--type post|doc|page|template]");
                return LoopAction::Continue;
            }
            cmd_ai(config, paths, args);
        }

        "login" => {
            let provider = args.first().map(|s| s.as_str()).unwrap_or("claude");
            if let Err(e) = auth::login(provider) {
                human::error(&format!("Login failed: {e}"));
            }
        }

        "theme" => {
            if args.is_empty() {
                // List themes
                for t in crate::themes::all() {
                    println!("  {} - {}", console::style(t.name).bold(), t.description);
                }
            } else {
                let name = &args[0];
                match crate::themes::by_name(name) {
                    Some(theme) => {
                        let template_dir = paths.templates.clone();
                        let _ = std::fs::create_dir_all(&template_dir);
                        match std::fs::write(template_dir.join("base.html"), theme.base_html) {
                            Ok(()) => human::success(&format!("Applied theme '{name}'")),
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
        format!("{date_str}-{slug}.md")
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

fn cmd_ai(config: &SiteConfig, paths: &crate::config::ResolvedPaths, args: &[String]) {
    // Parse flags out of args: --type <type>, --output <path>
    let mut gen_type = "post".to_string();
    let mut output_path: Option<PathBuf> = None;
    let mut prompt_parts = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--type" | "-t" => {
                if i + 1 < args.len() {
                    gen_type = args[i + 1].clone();
                    i += 2;
                } else {
                    human::error("--type requires a value");
                    return;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    human::error("--output requires a value");
                    return;
                }
            }
            _ => {
                prompt_parts.push(args[i].clone());
                i += 1;
            }
        }
    }

    let prompt = prompt_parts.join(" ");
    if prompt.is_empty() {
        human::error("Usage: ai <prompt> [--type post|doc|page|template]");
        return;
    }

    let provider_name = &config.ai.default_provider;
    let api_key = match credential::get_key(provider_name) {
        Ok(key) => key,
        Err(_) => {
            human::info(&format!("No API key for {provider_name}. Let's set one up."));
            if let Err(e) = auth::login(provider_name) {
                human::error(&format!("Login failed: {e}"));
                return;
            }
            match credential::get_key(provider_name) {
                Ok(key) => key,
                Err(e) => {
                    human::error(&format!("Still no key after login: {e}"));
                    return;
                }
            }
        }
    };

    let provider = match provider_name.as_str() {
        "claude" => Provider::Claude,
        "openai" => Provider::OpenAI,
        other => {
            human::error(&format!("Unknown provider: {other}"));
            return;
        }
    };

    let model = match &provider {
        Provider::Claude => "claude-sonnet-4-20250514".to_string(),
        Provider::OpenAI => "gpt-4o".to_string(),
    };

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    let client = AiClient::new(provider, api_key, model);

    if gen_type == "template" {
        spinner.set_message("Generating template...");
        let raw = match client.generate_template(&prompt) {
            Ok(r) => r,
            Err(e) => {
                spinner.finish_and_clear();
                human::error(&format!("AI generation failed: {e}"));
                return;
            }
        };
        spinner.finish_and_clear();

        let out = output_path.unwrap_or_else(|| paths.templates.join("custom.html"));
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&out, raw) {
            Ok(()) => human::success(&format!("Generated template: {}", out.display())),
            Err(e) => human::error(&format!("Failed to write: {e}")),
        }
    } else {
        spinner.set_message("Generating content...");
        let generated = match client.generate(&prompt, &gen_type) {
            Ok(g) => g,
            Err(e) => {
                spinner.finish_and_clear();
                human::error(&format!("AI generation failed: {e}"));
                return;
            }
        };
        spinner.finish_and_clear();

        let slug = content::slug_from_title(&generated.title);
        let collection = config::find_collection(&gen_type, &config.collections);
        let has_date = collection.map(|c| c.has_date).unwrap_or(false);
        let date = if has_date {
            Some(chrono::Local::now().date_naive())
        } else {
            None
        };

        let fm = Frontmatter {
            title: generated.title.clone(),
            date,
            draft: true,
            ..Default::default()
        };

        let out = output_path.unwrap_or_else(|| {
            let dir_name = collection.map(|c| c.directory.as_str()).unwrap_or("posts");
            if has_date {
                let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
                paths.content.join(dir_name).join(format!("{date_str}-{slug}.md"))
            } else {
                paths.content.join(dir_name).join(format!("{slug}.md"))
            }
        });

        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file_content = format!(
            "{}\n\n{}\n",
            content::generate_frontmatter(&fm),
            generated.body
        );
        match std::fs::write(&out, file_content) {
            Ok(()) => human::success(&format!("Generated: {}", out.display())),
            Err(e) => human::error(&format!("Failed to write: {e}")),
        }
    }
}

/// Split a line into tokens, respecting double quotes.
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
