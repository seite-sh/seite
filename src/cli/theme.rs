use std::path::PathBuf;
use std::process::Command;

use clap::{Args, Subcommand};

use crate::config::SiteConfig;
use crate::error::PageError;
use crate::output::human;
use crate::themes;

#[derive(Args)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand)]
pub enum ThemeCommand {
    /// List available themes
    List,

    /// Apply a theme to the current site
    Apply {
        /// Theme name
        name: String,
    },

    /// Generate a custom theme using AI (requires Claude Code)
    Create {
        /// Design description for the theme (e.g. "dark glassmorphism with teal accents")
        prompt: String,
    },
}

pub fn run(args: &ThemeArgs) -> anyhow::Result<()> {
    match &args.command {
        ThemeCommand::List => {
            human::header("Available themes");
            for theme in themes::all() {
                println!(
                    "  {} - {}",
                    console::style(theme.name).bold(),
                    theme.description
                );
            }
        }
        ThemeCommand::Apply { name } => {
            let theme = themes::by_name(name)
                .ok_or_else(|| anyhow::anyhow!(
                    "unknown theme '{}'. Run 'page theme list' to see available themes",
                    name
                ))?;

            // Ensure we're in a page project
            let _config = SiteConfig::load(&PathBuf::from("page.toml"))?;

            let template_dir = PathBuf::from("templates");
            std::fs::create_dir_all(&template_dir)?;
            std::fs::write(template_dir.join("base.html"), theme.base_html)?;

            human::success(&format!("Applied theme '{}'", name));
            human::info("Run 'page build' or the watcher will pick it up automatically.");
        }
        ThemeCommand::Create { prompt } => {
            run_create(prompt)?;
        }
    }
    Ok(())
}

fn run_create(user_prompt: &str) -> anyhow::Result<()> {
    // Check Claude Code is available
    match Command::new("claude").arg("--version").output() {
        Ok(o) if o.status.success() => {}
        _ => return Err(PageError::Agent(
            "Claude Code is not installed. Install it with: npm install -g @anthropic-ai/claude-code".into()
        ).into()),
    }

    let _config = SiteConfig::load(&PathBuf::from("page.toml"))?;

    // Ensure templates/ dir exists so Claude can write into it
    std::fs::create_dir_all("templates")?;

    let full_prompt = build_theme_prompt(user_prompt);

    human::info(&format!("Generating theme: \"{}\"", user_prompt));
    human::info("Claude is writing templates/base.html...");

    let status = Command::new("claude")
        .args(["-p", &full_prompt])
        .args(["--allowedTools", "Write,Edit,Read"])
        .status()
        .map_err(|e| PageError::Agent(format!("failed to run claude: {e}")))?;

    if !status.success() {
        return Err(PageError::Agent("claude exited with non-zero status".into()).into());
    }

    human::success("Theme written to templates/base.html");
    human::info("Run 'page build' to apply, or 'page serve' to preview live.");
    Ok(())
}

fn build_theme_prompt(user_prompt: &str) -> String {
    format!(r#"You are generating a custom theme for a `page` static site generator project.

Write a complete, self-contained `templates/base.html` file based on this design direction:

  {user_prompt}

## Requirements

The file must be a valid Tera/Jinja2 template (page uses the Tera engine).
It IS the base template — it does NOT extend anything.
It must define these blocks:

  {{% block title %}}Site Title{{% endblock %}}   ← used for <title>
  {{% block head %}}{{% endblock %}}              ← optional extra <link>/<meta> in <head>
  {{% block content %}}{{% endblock %}}           ← main page content

## Available Template Variables

| Variable | Description |
|----------|-------------|
| `site.title` | Site title |
| `site.description` | Site description |
| `site.base_url` | Base URL |
| `site.language` | Language code (e.g. "en") |
| `site.author` | Author name |
| `page.title` | Current page title (may be empty on index) |
| `page.content` | Rendered HTML body — always use `| safe` filter |
| `page.description` | Page description |
| `page.date` | Date string (posts only) |
| `page.tags` | Array of tag strings |
| `collections` | Array of `{{name, label, items[]}}` on index pages |
| `item.title` | Collection item title |
| `item.url` | Collection item URL |
| `item.date` | Collection item date |
| `item.description` | Collection item description |
| `item.tags` | Collection item tags |
| `pagination` | Pagination context (may be undefined) |
| `pagination.current_page` | Current page number |
| `pagination.total_pages` | Total pages |
| `pagination.prev_url` | URL of previous page (may be undefined) |
| `pagination.next_url` | URL of next page (may be undefined) |
| `translations` | Array of `{{lang, url}}` for language switcher |
| `lang` | Current language code |
| `nav` | Docs sidebar nav array `{{label, url, active}}` |

## Template Patterns to Follow

Search box (client-side, always include):
```html
<form class="search-form" onsubmit="return false">
  <input type="search" id="search-input" placeholder="Search..." autocomplete="off">
</form>
<div id="search-results"></div>
```

Search script (copy this exactly, at end of body):
```html
<script>
(function(){{
    var index = null;
    var input = document.getElementById('search-input');
    var results = document.getElementById('search-results');
    var indexUrl = '/search-index.json';
    function load(cb) {{ if (index) {{ cb(); return; }} fetch(indexUrl).then(function(r){{return r.json();}}).then(function(d){{index=d;cb();}}).catch(function(){{index=[];}}); }}
    function search(q) {{
        q = q.toLowerCase().trim();
        if (!q) {{ results.innerHTML = ''; return; }}
        var hits = index.filter(function(e){{
            return (e.title||'').toLowerCase().includes(q) || (e.description||'').toLowerCase().includes(q) || (e.tags||[]).some(function(t){{return t.toLowerCase().includes(q);}});
        }}).slice(0, 8);
        if (!hits.length) {{ results.innerHTML = '<div class="no-results">No results</div>'; return; }}
        results.innerHTML = hits.map(function(e){{
            var meta = [e.collection, e.date].filter(Boolean).join(' · ');
            return '<a href="' + e.url + '"><strong>' + e.title + '</strong>' + (meta ? '<div class="result-meta">' + meta + '</div>' : '') + '</a>';
        }}).join('');
    }}
    input.addEventListener('input', function(){{ load(function(){{ search(input.value); }}); }});
}})();
</script>
```

Pagination nav (include when pagination context exists):
```html
{{% if pagination %}}
<nav class="pagination">
    {{% if pagination.prev_url %}}<a href="{{{{ pagination.prev_url }}}}">&larr; Newer</a>{{% endif %}}
    <span>Page {{{{ pagination.current_page }}}} of {{{{ pagination.total_pages }}}}</span>
    {{% if pagination.next_url %}}<a href="{{{{ pagination.next_url }}}}">Older &rarr;</a>{{% endif %}}
</nav>
{{% endif %}}
```

Language switcher (include when translations exist):
```html
{{% if translations %}}<div class="lang-switcher">
    {{% for t in translations %}}<a href="{{{{ t.url }}}}">{{{{ t.lang }}}}</a>{{% endfor %}}
</div>{{% endif %}}
```

Hreflang links in <head>:
```html
{{% for t in translations %}}<link rel="alternate" hreflang="{{{{ t.lang }}}}" href="{{{{ site.base_url }}}}{{{{ t.url }}}}">
{{% endfor %}}
```

## Output

Write the complete file to `templates/base.html`. Include all CSS inline in a `<style>` block — no external stylesheets. The design should be fully self-contained and production-quality.

Design direction to implement: {user_prompt}
"#)
}
