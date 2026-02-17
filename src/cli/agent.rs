use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

use clap::Args;
use walkdir::WalkDir;

use crate::config::{ResolvedPaths, SiteConfig};
use crate::content;
use crate::error::PageError;
use crate::output::human;

#[derive(Args)]
pub struct AgentArgs {
    /// Prompt for the agent (omit for interactive chat)
    pub prompt: Option<String>,

    /// Run a single prompt and exit (no follow-up conversation)
    #[arg(long)]
    pub once: bool,
}

pub fn run(args: &AgentArgs) -> anyhow::Result<()> {
    ensure_claude_installed()?;

    let config = SiteConfig::load(&PathBuf::from("page.toml"))?;
    let paths = config.resolve_paths(&std::env::current_dir()?);
    let system_prompt = build_system_prompt(&config, &paths);

    let allowed_tools = "Read,Write,Edit,Glob,Grep,Bash";

    match &args.prompt {
        Some(prompt) if args.once => {
            // Single-shot mode: run one prompt and exit
            human::info("Starting agent...");
            let status = Command::new("claude")
                .args(["-p", prompt])
                .args(["--append-system-prompt", &system_prompt])
                .args(["--allowedTools", allowed_tools])
                .status()
                .map_err(|e| PageError::Agent(format!("failed to run claude: {e}")))?;

            if !status.success() {
                return Err(
                    PageError::Agent("claude exited with non-zero status".into()).into(),
                );
            }
        }
        Some(prompt) => {
            // Chat mode starting with a prompt
            human::info("Starting agent session...");
            let session_id = run_prompt(prompt, None, &system_prompt, allowed_tools)?;
            if let Some(sid) = session_id {
                chat_loop(&sid, allowed_tools)?;
            }
        }
        None => {
            // Interactive Claude Code session (full TUI)
            human::info("Starting interactive agent session...");
            human::info("The agent has full context about your site. Type your requests.");
            let status = Command::new("claude")
                .args(["--append-system-prompt", &system_prompt])
                .args(["--allowedTools", allowed_tools])
                .status()
                .map_err(|e| PageError::Agent(format!("failed to run claude: {e}")))?;

            if !status.success() {
                human::info("Agent session ended.");
            }
        }
    }

    Ok(())
}

/// Run a single prompt via `claude -p`, return the session ID for follow-ups.
fn run_prompt(
    prompt: &str,
    session_id: Option<&str>,
    system_prompt: &str,
    allowed_tools: &str,
) -> anyhow::Result<Option<String>> {
    let mut cmd = Command::new("claude");
    cmd.args(["-p", prompt])
        .args(["--output-format", "json"])
        .args(["--allowedTools", allowed_tools]);

    // First message gets the system prompt; follow-ups use --resume
    match session_id {
        Some(sid) => {
            cmd.args(["--resume", sid]);
        }
        None => {
            cmd.args(["--append-system-prompt", system_prompt]);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| PageError::Agent(format!("failed to run claude: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            human::error(&format!("Agent error: {stderr}"));
        }
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON response to extract session_id and result
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        // Print the result text
        if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
            println!("{result}");
        }
        // Return session_id for follow-up
        if let Some(sid) = json.get("session_id").and_then(|s| s.as_str()) {
            return Ok(Some(sid.to_string()));
        }
    } else {
        // Fallback: print raw output if JSON parsing fails
        print!("{stdout}");
    }

    Ok(None)
}

/// Interactive chat loop: prompt the user, send follow-ups to the same session.
fn chat_loop(session_id: &str, allowed_tools: &str) -> anyhow::Result<()> {
    println!();
    human::info("Chat session active. Type follow-up messages, or \"done\" to exit.");
    println!();

    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("you> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "done" | "quit" | "exit" | "stop") {
            human::info("Agent session ended.");
            break;
        }

        println!();
        match run_prompt(line, Some(session_id), "", allowed_tools) {
            Ok(_) => {}
            Err(e) => {
                human::error(&format!("Agent error: {e}"));
                break;
            }
        }
        println!();
    }

    Ok(())
}

fn ensure_claude_installed() -> anyhow::Result<()> {
    match Command::new("claude").arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(PageError::Agent(
            "Claude Code is not installed. Install it with: npm install -g @anthropic-ai/claude-code"
                .into(),
        )
        .into()),
    }
}

/// Build a system prompt with full site context for the Claude Code agent.
///
/// This is public so the REPL in serve.rs can reuse it.
pub fn build_system_prompt(config: &SiteConfig, paths: &ResolvedPaths) -> String {
    let mut prompt = String::with_capacity(4096);

    // Site overview
    prompt.push_str(&format!(
        r#"You are an AI assistant helping manage a static site built with the `page` CLI tool.

## Site Configuration
- Title: {}
- Description: {}
- Base URL: {}
- Language: {}
- Author: {}

"#,
        config.site.title,
        config.site.description,
        config.site.base_url,
        config.site.language,
        config.site.author,
    ));

    // Collections
    prompt.push_str("## Collections\n\n");
    for c in &config.collections {
        prompt.push_str(&format!(
            "### {} (\"{}\")\n- Directory: `content/{}/`\n- URL prefix: `{}`\n- Template: `{}`\n- Date-based: {}\n- RSS: {}\n- Nested: {}\n\n",
            c.label, c.name, c.directory, c.url_prefix, c.default_template,
            c.has_date, c.has_rss, c.nested,
        ));
    }

    // Content inventory
    prompt.push_str("## Existing Content\n\n");
    for c in &config.collections {
        let items = scan_collection_content(paths, c);
        prompt.push_str(&format!("### {} ({} items)\n", c.label, items.len()));
        for item in items.iter().take(50) {
            prompt.push_str(&format!("- {}\n", item));
        }
        if items.len() > 50 {
            prompt.push_str(&format!("- ... and {} more\n", items.len() - 50));
        }
        prompt.push('\n');
    }

    // Templates
    prompt.push_str("## Templates\n\n");
    for name in list_templates(paths) {
        prompt.push_str(&format!("- `templates/{name}`\n"));
    }
    prompt.push('\n');

    // Content format
    prompt.push_str(
        r#"## Content Format

Content files are markdown with YAML frontmatter delimited by `---`:

```
---
title: "Post Title"
date: 2025-01-15        # required for posts, omit for docs/pages
description: "Optional"  # optional
tags:                     # optional
  - rust
  - web
draft: true              # optional, omit when false
---

Markdown content here.
```

## File Naming Conventions
- Posts: `content/posts/YYYY-MM-DD-slug-here.md` (date prefix required)
- Docs: `content/docs/slug-here.md` or `content/docs/section/slug-here.md` (nested OK)
- Pages: `content/pages/slug-here.md` (no date prefix)

## Available Commands
- `page build` — Rebuild the site after making changes
- `page build --drafts` — Build including draft content
- `page new post "Title" --tags tag1,tag2` — Create a new post
- `page new doc "Title"` — Create a new doc
- `page new page "Title"` — Create a new page
- `page theme list` — List available themes
- `page theme apply <name>` — Apply a bundled theme

## Important Notes
- After creating or editing content files, run `page build` to regenerate the site.
- Set `draft: true` in frontmatter to exclude content from the default build.
- The site output goes to the `dist/` directory.
- Templates use Tera (Jinja2-compatible) syntax and extend `base.html`.
- Each content file produces both `slug.html` and `slug.md` in the output.
- URLs are clean (no extension): `/posts/hello-world`
"#,
    );

    prompt
}

/// Scan a collection's content directory and return a summary of each item.
fn scan_collection_content(
    paths: &ResolvedPaths,
    collection: &crate::config::CollectionConfig,
) -> Vec<String> {
    let collection_dir = paths.content.join(&collection.directory);
    let mut items = Vec::new();

    if !collection_dir.exists() {
        return items;
    }

    for entry in WalkDir::new(&collection_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let path = entry.path();
        if let Ok((fm, _body)) = content::parse_content_file(path) {
            let mut summary = fm.title.clone();
            if let Some(date) = fm.date {
                summary = format!("{summary} ({date})");
            }
            if !fm.tags.is_empty() {
                summary = format!("{summary} [{}]", fm.tags.join(", "));
            }
            if fm.draft {
                summary = format!("{summary} (draft)");
            }
            items.push(summary);
        }
    }

    items
}

/// List template files in the templates directory.
fn list_templates(paths: &ResolvedPaths) -> Vec<String> {
    let mut names = Vec::new();

    if !paths.templates.exists() {
        return names;
    }

    for entry in WalkDir::new(&paths.templates)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Some(name) = entry
            .path()
            .strip_prefix(&paths.templates)
            .ok()
            .and_then(|p| p.to_str())
        {
            names.push(name.to_string());
        }
    }

    names.sort();
    names
}
