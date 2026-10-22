use std::io::{self, BufRead, IsTerminal, Write};
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
    /// Host to bind to (default: 127.0.0.1, use 0.0.0.0 for network access)
    #[arg(long)]
    pub host: Option<String>,

    /// Port to serve on (auto-finds available port if default is taken)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Build before serving
    #[arg(long, default_value = "true")]
    pub build: bool,

    /// Open the site in the default browser after starting
    #[arg(long)]
    pub open: bool,

    /// Don't read commands from stdin; serve (with live reload) until
    /// interrupted (Ctrl-C / SIGTERM). Without it, the server also keeps running
    /// when stdin closes (e.g. `</dev/null` or a background job).
    #[arg(long)]
    pub no_repl: bool,

    /// Start a new server even if `.seite/dev.json` records a live one
    #[arg(long)]
    pub force_new: bool,
}

/// Fail fast (before building) when an explicitly requested port is taken.
fn ensure_port_free(host: &str, port: u16) -> anyhow::Result<u16> {
    if server::find_available_port(host, port) != Some(port) {
        anyhow::bail!(
            "port {port} is already in use on {host} (omit --port to pick a free port automatically)"
        );
    }
    Ok(port)
}

/// Keep serving until the process is interrupted. The server, file watcher and
/// live reload run on background threads; the default SIGINT/SIGTERM handling
/// terminates the process.
fn serve_until_interrupted(port: u16) -> ! {
    human::info(&format!(
        "Serving on port {port} until interrupted (Ctrl-C / SIGTERM to stop)"
    ));
    loop {
        std::thread::park();
    }
}

/// `.seite/dev.json` bookkeeping so `seite context` (and a second `seite serve`)
/// can find the running dev server.
mod devfile {
    use std::path::{Path, PathBuf};

    use crate::output::human;

    pub fn path(root: &Path) -> PathBuf {
        root.join(".seite").join("dev.json")
    }

    /// Write `{pid, port, url, started_at}` atomically (a fresh temp file
    /// renamed over it), refusing a symlinked `.seite/` or `dev.json`.
    pub fn write(root: &Path, pid: u32, port: u16, url: &str) -> anyhow::Result<()> {
        let doc = serde_json::json!({
            "pid": pid,
            "port": port,
            "url": url,
            "started_at": chrono::Utc::now().to_rfc3339(),
        });
        crate::safe_fs::write(root, &path(root), serde_json::to_string_pretty(&doc)?)
    }

    /// Pid recorded in dev.json, if readable (never through a symlink).
    pub fn read_pid(root: &Path) -> Option<u32> {
        crate::safe_fs::guard(root, &path(root)).ok()?;
        let raw = std::fs::read_to_string(path(root)).ok()?;
        let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
        u32::try_from(v.get("pid")?.as_u64()?).ok()
    }

    /// Remove dev.json only if it belongs to this process.
    pub fn remove_if_ours(root: &Path) -> bool {
        remove_if_pid(root, std::process::id())
    }

    pub fn remove_if_pid(root: &Path, pid: u32) -> bool {
        read_pid(root) == Some(pid) && crate::safe_fs::remove(root, &path(root)).is_ok()
    }

    /// Removes dev.json (if ours) when dropped.
    pub struct Guard {
        root: PathBuf,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            remove_if_ours(&self.root);
        }
    }

    /// Record the running server; failure is a warning, never fatal. Also makes
    /// Ctrl-C / SIGTERM / SIGHUP remove the file before exiting.
    pub fn publish(root: &Path, port: u16, url: &str) -> Option<Guard> {
        if let Err(e) = write(root, std::process::id(), port, url) {
            human::warning(&format!("Could not write .seite/dev.json: {e:#}"));
            return None;
        }
        install_signal_cleanup(root.to_path_buf());
        Some(Guard {
            root: root.to_path_buf(),
        })
    }

    #[cfg(unix)]
    fn install_signal_cleanup(root: PathBuf) {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
        let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP]) else {
            return;
        };
        std::thread::spawn(move || {
            if let Some(sig) = signals.forever().next() {
                remove_if_ours(&root);
                std::process::exit(128 + sig);
            }
        });
    }

    // No signal handler on Windows; a stale file is detected by the port probe.
    #[cfg(not(unix))]
    fn install_signal_cleanup(_root: PathBuf) {}
}

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3000;

pub fn run(args: &ServeArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let host = args.host.as_deref().unwrap_or(DEFAULT_HOST);
    let ws_root_opt = workspace::find_workspace_root(&cwd);
    let dev_root = ws_root_opt.clone().unwrap_or_else(|| cwd.clone());

    if !args.force_new {
        if let Some(running) = crate::cli::context::running_dev_server(&dev_root) {
            human::info(&format!(
                "Dev server already running at {} (pid {}). Use --force-new to start another.",
                running.url, running.pid
            ));
            return Ok(());
        }
    }

    // Check for workspace context
    if let Some(ws_root) = ws_root_opt {
        let ws_config = workspace::WorkspaceConfig::load(&ws_root.join("seite-workspace.toml"))?;

        // Single-site serve: build and serve just that site standalone, baking
        // the live dev address into its base_url (mirrors non-workspace serve).
        // Resolve the port up front so even the first build is correct, then
        // hand that exact port to the server with auto-increment off. (#86)
        if let Some(site_name) = site_filter {
            let ws_site = ws_config
                .find_site(site_name)
                .ok_or_else(|| anyhow::anyhow!("unknown site '{site_name}' in workspace"))?;
            let (mut config, paths) = workspace::load_site_in_workspace(&ws_root, ws_site)?;
            // The dev server previews scheduled content, like drafts.
            config.build.future = true;

            let port = match args.port {
                Some(p) => ensure_port_free(host, p)?,
                None => server::find_available_port(host, DEFAULT_PORT).ok_or_else(|| {
                    anyhow::anyhow!(
                        "no available port found in {}-{}",
                        DEFAULT_PORT,
                        DEFAULT_PORT.saturating_add(99)
                    )
                })?,
            };
            config.site.base_url = server::dev_base_url(host, port);

            if args.build {
                human::info(&format!("Building site '{site_name}'..."));
                let opts = BuildOptions {
                    include_drafts: true,
                    incremental: false,
                };
                let result = build::build_site(&config, &paths, &opts)?;
                human::success(&result.stats.human_display());
            }

            let handle = server::start(&config, &paths, host, port, true, false)?;
            let _dev = devfile::publish(
                &dev_root,
                handle.port(),
                &server::dev_base_url(host, handle.port()),
            );

            if args.no_repl {
                serve_until_interrupted(handle.port());
            }

            human::info(&format!(
                "Serving site '{site_name}'. Type \"help\" for commands, \"stop\" to quit (port {})",
                handle.port()
            ));

            if !run_repl(&config, &paths, &handle)? {
                serve_until_interrupted(handle.port());
            }
            return Ok(());
        }

        // Multi-site dev server: build every site (each keeps its base_url) and serve.
        if args.build {
            human::info("Building workspace...");
            let build_opts = workspace::build::WorkspaceBuildOptions {
                include_drafts: true,
                strict: false,
                site_filter: None,
                future: true,
            };
            workspace::build::build_workspace(&ws_config, &ws_root, &build_opts)?;
        }

        let port = args.port.unwrap_or(DEFAULT_PORT);
        let auto_increment = args.port.is_none();
        let handle = workspace::server::start(&ws_config, &ws_root, host, port, auto_increment)?;
        let _dev = devfile::publish(
            &dev_root,
            handle.port(),
            &server::dev_base_url(host, handle.port()),
        );

        if args.no_repl {
            serve_until_interrupted(handle.port());
        }

        human::info(&format!(
            "Type \"stop\" to quit (server on port {})",
            handle.port()
        ));

        let stdin = io::stdin();
        let reader = stdin.lock();
        print_prompt();

        let mut stopped = false;
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
                    stopped = true;
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

        // stdin closed without "stop" (e.g. </dev/null): keep serving.
        if !stopped {
            serve_until_interrupted(handle.port());
        }
        return Ok(());
    }

    // Standalone mode
    if site_filter.is_some() {
        human::warning("--site flag ignored (not in a workspace)");
    }

    let mut config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    // The dev server previews scheduled (future-dated) content, like drafts.
    config.build.future = true;
    let paths = config.resolve_paths(&cwd);
    if let Some(line) = crate::content::schedule::hidden_summary(
        &crate::content::schedule::hidden_pages(&config, &paths),
    ) {
        human::info(&line);
    }

    // Resolve the exact port we'll bind *before* building, then point base_url
    // at that live address. Otherwise absolute URLs (og:image, canonical,
    // sitemap, feeds, JSON-LD) bake in the configured base_url — defaulting to
    // localhost:3000 — and silently ignore --host/--port. Mirrors `hugo server`
    // / `zola serve`, which rewrite the base URL to the address served. (#86)
    //
    // The port is resolved once here (auto-incrementing past a busy default),
    // then handed to `server::start` with auto-increment disabled — so the
    // bound port is exactly the one baked into base_url, or startup fails
    // loudly, instead of the two independent scans drifting apart and serving
    // on a different port than the URLs advertise.
    let port = match args.port {
        Some(p) => ensure_port_free(host, p)?,
        None => server::find_available_port(host, DEFAULT_PORT).ok_or_else(|| {
            anyhow::anyhow!(
                "no available port found in {}-{}",
                DEFAULT_PORT,
                DEFAULT_PORT.saturating_add(99)
            )
        })?,
    };
    config.site.base_url = server::dev_base_url(host, port);

    if args.build {
        human::info("Building site...");
        let opts = BuildOptions {
            include_drafts: true,
            incremental: false,
        };
        let result = build::build_site(&config, &paths, &opts)?;
        human::success(&result.stats.human_display());
    }

    let handle = server::start(&config, &paths, host, port, true, false)?;
    let _dev = devfile::publish(
        &dev_root,
        handle.port(),
        &server::dev_base_url(host, handle.port()),
    );

    if !args.no_repl {
        human::info("Type \"help\" for commands, \"stop\" to quit");
    }

    if args.open {
        let display_host = match host {
            "0.0.0.0" | "::" => "localhost",
            h => h,
        };
        let url = if display_host.contains(':') {
            format!("http://[{display_host}]:{}/", handle.port())
        } else {
            format!("http://{display_host}:{}/", handle.port())
        };
        if let Err(e) = open::that(&url) {
            human::warning(&format!("Could not open browser: {e}"));
        }
    }

    if args.no_repl || !run_repl(&config, &paths, &handle)? {
        serve_until_interrupted(handle.port());
    }

    Ok(())
}

/// Read REPL commands from stdin. Returns `true` when the user stopped the
/// server, `false` when stdin closed (the caller then keeps serving).
fn run_repl(
    config: &SiteConfig,
    paths: &crate::config::ResolvedPaths,
    handle: &server::ServerHandle,
) -> anyhow::Result<bool> {
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
                return Ok(true);
            }
        }
        print_prompt();
    }

    Ok(false)
}

fn print_prompt() {
    // Only show the prompt to a human; piped stdin gets clean logs.
    if !io::stdin().is_terminal() {
        return;
    }
    print!("seite> ");
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
            let opts = BuildOptions {
                include_drafts,
                incremental: false,
            };
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
                        println!(
                            "  {} - {}",
                            console::style(&t.name).bold().cyan(),
                            t.description
                        );
                    }
                }
            } else {
                let name = &args[0];
                // Try bundled first, then installed
                let template_content: Option<String> = crate::themes::by_name(name)
                    .map(|t| t.base_html.to_string())
                    .or_else(|| {
                        let project_root = std::path::PathBuf::from(".");
                        crate::themes::installed_by_name(&project_root, name).map(|t| t.base_html)
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
                                let opts = BuildOptions {
                                    include_drafts: true,
                                    incremental: false,
                                };
                                match build::build_site(config, paths, &opts) {
                                    Ok(result) => human::success(&result.stats.human_display()),
                                    Err(e) => human::error(&format!("Rebuild failed: {e}")),
                                }
                            }
                            Err(e) => human::error(&format!("Failed to apply theme: {e}")),
                        }
                    }
                    None => human::error(&format!(
                        "Unknown theme '{name}'. Type 'theme' to list available themes."
                    )),
                }
            }
        }

        "status" => {
            println!("  Site: {}", config.site.title);
            println!(
                "  Collections: {}",
                config
                    .collections
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        _ => {
            human::error(&format!(
                "Unknown command: {cmd}. Type \"help\" for available commands."
            ));
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
            let available: Vec<&str> = config.collections.iter().map(|c| c.name.as_str()).collect();
            let hint = human::suggest_match(collection_name, &available);
            human::error(&format!(
                "Unknown collection '{}'. Available: {}{}",
                collection_name,
                available.join(", "),
                hint,
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
        Some(crate::content::schedule::today(config))
    } else {
        None
    };

    let fm = Frontmatter {
        title: title.to_string(),
        date,
        ..Default::default()
    };

    let filename = if collection.has_date {
        let date_str = crate::content::schedule::today(config)
            .format("%Y-%m-%d")
            .to_string();
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
    if let Err(e) = std::fs::create_dir_all(
        filepath
            .parent()
            .expect("content file must have a parent directory"),
    ) {
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

fn cmd_agent(_config: &SiteConfig, _paths: &crate::config::ResolvedPaths, args: &[String]) {
    let agent_args = agent::AgentArgs {
        prompt: if args.is_empty() {
            None
        } else {
            Some(args.join(" "))
        },
        once: false,
        with: None,
        auto_approve: false,
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

#[cfg(test)]
mod devfile_tests {
    use super::devfile;

    #[test]
    fn test_devfile_write_read_remove() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        // .seite is created on demand
        devfile::write(root, 4242, 3001, "http://localhost:3001").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(devfile::path(root)).unwrap()).unwrap();
        assert_eq!(v["pid"], 4242);
        assert_eq!(v["port"], 3001);
        assert_eq!(v["url"], "http://localhost:3001");
        assert!(v["started_at"].is_string());
        assert_eq!(devfile::read_pid(root), Some(4242));
        // no leftover temp file
        let leftovers = std::fs::read_dir(root.join(".seite"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != "dev.json")
            .count();
        assert_eq!(leftovers, 0);

        // Not ours: kept.
        assert!(!devfile::remove_if_ours(root));
        assert!(devfile::path(root).exists());
        // Ours: removed.
        assert!(devfile::remove_if_pid(root, 4242));
        assert!(!devfile::path(root).exists());
        assert_eq!(devfile::read_pid(root), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_devfile_refuses_symlinked_seite_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join(".seite")).unwrap();
        let err = devfile::write(tmp.path(), 1, 3000, "x").unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert!(!outside.path().join("dev.json").exists());
        // Removal doesn't follow it either.
        std::fs::write(outside.path().join("dev.json"), r#"{"pid":1}"#).unwrap();
        assert!(!devfile::remove_if_pid(tmp.path(), 1));
        assert!(outside.path().join("dev.json").exists());
    }

    #[test]
    fn test_devfile_guard_removes_own_file_on_drop() {
        let tmp = tempfile::TempDir::new().unwrap();
        let guard = devfile::publish(tmp.path(), 1, "http://localhost:1");
        assert!(devfile::path(tmp.path()).exists());
        drop(guard);
        assert!(!devfile::path(tmp.path()).exists());
    }
}
