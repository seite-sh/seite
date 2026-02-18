use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::config::{CollectionConfig, DeployTarget};
use crate::content;
use crate::output::human;
use crate::templates;

#[derive(Args)]
pub struct InitArgs {
    /// Name of the site / directory to create
    pub name: Option<String>,

    /// Site title
    #[arg(long)]
    pub title: Option<String>,

    /// Site description
    #[arg(long)]
    pub description: Option<String>,

    /// Deploy target (github-pages, cloudflare)
    #[arg(long)]
    pub deploy_target: Option<String>,

    /// Collections to include (comma-separated: posts,docs,pages)
    #[arg(long)]
    pub collections: Option<String>,
}

pub fn run(args: &InitArgs) -> anyhow::Result<()> {
    let name = match &args.name {
        Some(n) => n.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Site name (directory)")
            .interact_text()?,
    };

    let title = match &args.title {
        Some(t) => t.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Site title")
            .default(name.clone())
            .interact_text()?,
    };

    let description = match &args.description {
        Some(d) => d.clone(),
        None => dialoguer::Input::<String>::new()
            .with_prompt("Site description")
            .default(String::new())
            .allow_empty(true)
            .interact_text()?,
    };

    let deploy_target = match &args.deploy_target {
        Some(t) => t.clone(),
        None => {
            let options = ["github-pages", "cloudflare", "netlify"];
            let selection = dialoguer::Select::new()
                .with_prompt("Deploy target")
                .items(&options)
                .default(0)
                .interact()?;
            options[selection].to_string()
        }
    };

    // Resolve collections
    let collections: Vec<CollectionConfig> = match &args.collections {
        Some(list) => list
            .split(',')
            .filter_map(|name| CollectionConfig::from_preset(name.trim()))
            .collect(),
        None => {
            let preset_names = ["posts", "docs", "pages"];
            let defaults = &[true, false, true]; // posts + pages on by default
            let selections = dialoguer::MultiSelect::new()
                .with_prompt("Collections to include")
                .items(&preset_names)
                .defaults(defaults)
                .interact()?;
            selections
                .into_iter()
                .filter_map(|i| CollectionConfig::from_preset(preset_names[i]))
                .collect()
        }
    };

    if collections.is_empty() {
        anyhow::bail!("at least one collection is required");
    }

    let root = PathBuf::from(&name);
    if root.exists() {
        anyhow::bail!("directory '{}' already exists", name);
    }

    // Create directory structure per collection
    for c in &collections {
        fs::create_dir_all(root.join("content").join(&c.directory))?;
    }
    fs::create_dir_all(root.join("templates"))?;
    fs::create_dir_all(root.join("static"))?;
    fs::create_dir_all(root.join(".claude"))?;

    // Generate page.toml
    let target = match deploy_target.as_str() {
        "cloudflare" => DeployTarget::Cloudflare,
        "netlify" => DeployTarget::Netlify,
        _ => DeployTarget::GithubPages,
    };
    let config = crate::config::SiteConfig {
        site: crate::config::SiteSection {
            title: title.clone(),
            description: description.clone(),
            base_url: "http://localhost:3000".into(),
            language: "en".into(),
            author: String::new(),
        },
        collections: collections.clone(),
        build: Default::default(),
        deploy: crate::config::DeploySection {
            target: target.clone(),
            repo: None,
            project: None,
            domain: None,
        },
        languages: Default::default(),
        images: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config)?;
    fs::write(root.join("page.toml"), toml_str)?;

    // Write default templates
    fs::write(root.join("templates/base.html"), templates::default_base())?;
    fs::write(root.join("templates/index.html"), templates::DEFAULT_INDEX)?;
    for c in &collections {
        let tmpl_name = &c.default_template;
        let content = match tmpl_name.as_str() {
            "post.html" => templates::DEFAULT_POST,
            "doc.html" => templates::DEFAULT_DOC,
            "page.html" => templates::DEFAULT_PAGE,
            _ => continue,
        };
        fs::write(root.join("templates").join(tmpl_name), content)?;
    }

    // Create sample hello-world post if posts collection is included
    if collections.iter().any(|c| c.name == "posts") {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let fm = content::Frontmatter {
            title: "Hello World".into(),
            date: Some(chrono::Local::now().date_naive()),
            description: Some("Welcome to your new site!".into()),
            tags: vec!["intro".into()],
            draft: false,
            ..Default::default()
        };
        let frontmatter_str = content::generate_frontmatter(&fm);
        let post_content = format!(
            "{frontmatter_str}\n\nWelcome to your new site built with **page**.\n\nEdit this post or create new ones with `page new post \"My Post\"`.\n"
        );
        fs::write(
            root.join(format!("content/posts/{today}-hello-world.md")),
            post_content,
        )?;
    }

    // Generate CI workflow and config files based on deploy target
    match target {
        DeployTarget::GithubPages => {
            let workflow_dir = root.join(".github/workflows");
            fs::create_dir_all(&workflow_dir)?;
            let workflow = crate::deploy::generate_github_actions_workflow(&config);
            fs::write(workflow_dir.join("deploy.yml"), workflow)?;
        }
        DeployTarget::Cloudflare => {
            let workflow_dir = root.join(".github/workflows");
            fs::create_dir_all(&workflow_dir)?;
            let workflow = crate::deploy::generate_cloudflare_workflow(&config);
            fs::write(workflow_dir.join("deploy.yml"), workflow)?;
        }
        DeployTarget::Netlify => {
            // Generate netlify.toml
            let netlify_config = crate::deploy::generate_netlify_config(&config);
            fs::write(root.join("netlify.toml"), &netlify_config)?;
            // Also generate GitHub Actions workflow as alternative
            let workflow_dir = root.join(".github/workflows");
            fs::create_dir_all(&workflow_dir)?;
            let workflow = crate::deploy::generate_netlify_workflow(&config);
            fs::write(workflow_dir.join("deploy.yml"), workflow)?;
        }
    }

    // Write Claude Code settings (.claude/settings.json)
    fs::write(
        root.join(".claude/settings.json"),
        generate_claude_settings(),
    )?;

    // Write CLAUDE.md with site-specific context
    fs::write(
        root.join("CLAUDE.md"),
        generate_claude_md(&title, &description, &collections),
    )?;

    human::success(&format!("Created new site in '{name}'"));
    human::info("Next steps:");
    println!("  cd {name}");
    println!("  page build");
    println!("  page serve");

    Ok(())
}

/// Generate .claude/settings.json with pre-approved tools for the page workflow.
fn generate_claude_settings() -> String {
    r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "permissions": {
    "allow": [
      "Read",
      "Write(content/**)",
      "Write(templates/**)",
      "Write(static/**)",
      "Edit(content/**)",
      "Edit(templates/**)",
      "Bash(page build:*)",
      "Bash(page build)",
      "Bash(page new:*)",
      "Bash(page serve:*)",
      "Bash(page theme:*)",
      "Glob",
      "Grep",
      "WebSearch"
    ],
    "deny": [
      "Read(.env)",
      "Read(.env.*)"
    ]
  }
}
"#
    .to_string()
}

/// Generate a CLAUDE.md tailored to the site's collections and structure.
fn generate_claude_md(
    title: &str,
    description: &str,
    collections: &[CollectionConfig],
) -> String {
    let mut md = String::with_capacity(8192);

    // Header
    md.push_str(&format!("# {title}\n\n"));
    if !description.is_empty() {
        md.push_str(&format!("{description}\n\n"));
    }
    md.push_str("This is a static site built with the `page` CLI tool.\n\n");

    // SEO and GEO requirements — top-level, mandatory
    md.push_str("## SEO and GEO Requirements\n\n");
    md.push_str("> **These are non-negotiable rules for every page on this site.**\n");
    md.push_str("> They apply when writing content, creating templates, or asking the AI agent to build or redesign anything.\n\n");
    md.push_str("### Every page `<head>` MUST include\n\n");
    md.push_str("1. **Canonical URL** — `<link rel=\"canonical\" href=\"{{ site.base_url }}{{ page.url | default(value='/') }}\">` (deduplicates indexed URLs)\n");
    md.push_str("2. **Open Graph tags** — `og:type`, `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`\n");
    md.push_str("   - `og:type = article` when `page.collection` is set; `website` for the homepage\n");
    md.push_str("   - `og:image` only when `page.image` is set\n");
    md.push_str("3. **Twitter Card tags** — `twitter:card`, `twitter:title`, `twitter:description`\n");
    md.push_str("   - `twitter:card = summary_large_image` when `page.image` is set; `summary` otherwise\n");
    md.push_str("4. **JSON-LD structured data** — `<script type=\"application/ld+json\">` block:\n");
    md.push_str("   - `BlogPosting` for posts (include `datePublished`, `dateModified` if `page.updated` is set)\n");
    md.push_str("   - `Article` for docs and other collection pages\n");
    md.push_str("   - `WebSite` for the homepage/index\n");
    md.push_str("5. **Markdown alternate link** — `<link rel=\"alternate\" type=\"text/markdown\" href=\"{{ site.base_url }}{{ page.url }}.md\">` (LLM-native differentiator)\n");
    md.push_str("6. **llms.txt discovery** — `<link rel=\"alternate\" type=\"text/plain\" title=\"LLM Summary\" href=\"/llms.txt\">`\n");
    md.push_str("7. **RSS autodiscovery** — `<link rel=\"alternate\" type=\"application/rss+xml\" ...>`\n");
    md.push_str("8. **Language attribute** — `<html lang=\"{{ site.language }}\">` (already in bundled themes)\n\n");
    md.push_str("### Per-page frontmatter best practices\n\n");
    md.push_str("- **Always set `description:`** — used verbatim in `<meta name=\"description\">`, `og:description`, `twitter:description`, and JSON-LD. Without it, `site.description` is used as a fallback but that is generic.\n");
    md.push_str("- **Set `image:`** for posts with a visual — unlocks `og:image`, `twitter:image`, and the `summary_large_image` card type\n");
    md.push_str("- **Set `updated:`** when you revise existing content — populates `dateModified` in JSON-LD\n");
    md.push_str("- **Set `robots: noindex`** on draft-like or utility pages (tag pages, test pages) that should not appear in search results\n\n");
    md.push_str("### What NOT to do\n\n");
    md.push_str("- Do not remove canonical, OG, Twitter Card, or JSON-LD blocks when customizing `base.html`\n");
    md.push_str("- Do not use `site.description` directly for meta tags — always use `page.description | default(value=site.description)`\n");
    md.push_str("- Do not hardcode URLs — always compose from `site.base_url ~ page.url`\n\n");

    // Quick commands
    md.push_str("## Commands\n\n");
    md.push_str("```bash\n");
    md.push_str("page build                              # Build the site\n");
    md.push_str("page build --drafts                     # Build including draft content\n");
    md.push_str("page serve                              # Dev server with live reload + REPL\n");
    md.push_str("page serve --port 8080                  # Use a specific port\n");
    for c in collections {
        let singular = singularize(&c.name);
        md.push_str(&format!(
            "page new {singular} \"Title\"                  # Create new {singular}\n",
        ));
    }
    md.push_str("page new post \"Title\" --tags rust,web   # Create with tags\n");
    md.push_str("page new post \"Title\" --draft           # Create as draft\n");
    md.push_str("page new post \"Title\" --lang es         # Create translation (needs [languages.es] in config)\n");
    md.push_str("page theme list                         # List available themes\n");
    md.push_str("page theme apply <name>                 # Apply a bundled theme (default, minimal, dark, docs, brutalist, bento)\n");
    md.push_str("page theme create \"coral brutalist\"     # Generate a custom theme with AI (requires Claude Code)\n");
    md.push_str("page agent                              # Interactive AI agent session\n");
    md.push_str("page agent \"write about Rust\"           # One-shot AI agent prompt\n");
    md.push_str("page deploy                             # Deploy to configured target\n");
    md.push_str("```\n\n");

    // Dev server REPL
    md.push_str("### Dev Server REPL\n\n");
    md.push_str("`page serve` starts a dev server with live reload and an interactive REPL:\n\n");
    md.push_str("```\n");
    md.push_str("new <collection> <title> [--lang <code>]  Create new content\n");
    md.push_str("agent [prompt]                           Start AI agent or run one-shot\n");
    md.push_str("theme <name>                             Apply a theme\n");
    md.push_str("build [--drafts]                         Rebuild the site\n");
    md.push_str("status                                   Show server info\n");
    md.push_str("stop                                     Stop and exit\n");
    md.push_str("```\n\n");

    // Project structure
    md.push_str("## Project Structure\n\n");
    md.push_str("```\n");
    for c in collections {
        md.push_str(&format!(
            "content/{}/    # {} content (markdown + YAML frontmatter)\n",
            c.directory, c.label
        ));
    }
    md.push_str("templates/       # Tera (Jinja2-compatible) HTML templates\n");
    md.push_str("static/          # Static assets (copied as-is to dist/)\n");
    md.push_str("dist/            # Build output (generated, do not edit)\n");
    md.push_str("page.toml        # Site configuration\n");
    md.push_str("```\n\n");

    // Collections
    md.push_str("## Collections\n\n");
    for c in collections {
        md.push_str(&format!("### {}\n", c.label));
        md.push_str(&format!("- Directory: `content/{}/`\n", c.directory));
        md.push_str(&format!(
            "- URL prefix: `{}`\n",
            if c.url_prefix.is_empty() {
                "(root)"
            } else {
                &c.url_prefix
            }
        ));
        md.push_str(&format!("- Template: `{}`\n", c.default_template));
        if c.has_date {
            md.push_str("- Date-based: yes (filename format: `YYYY-MM-DD-slug.md`)\n");
        } else {
            md.push_str("- Date-based: no (filename format: `slug.md`)\n");
        }
        if c.nested {
            md.push_str(
                "- Supports nested directories (e.g., `section/slug.md` → `/docs/section/slug`)\n",
            );
        }
        if c.has_rss {
            md.push_str("- Included in RSS feed (`/feed.xml`)\n");
        }
        md.push('\n');
    }

    // Content format
    md.push_str("## Content Format\n\n");
    md.push_str("Content files are markdown with YAML frontmatter:\n\n");
    md.push_str("```yaml\n");
    md.push_str("---\n");
    md.push_str("title: \"Post Title\"\n");
    if collections.iter().any(|c| c.has_date) {
        md.push_str("date: 2025-01-15        # required for dated collections\n");
    }
    md.push_str("description: \"Optional\"  # page description — used in meta/OG/Twitter/JSON-LD\n");
    md.push_str("image: /static/og.png    # optional social-preview image (og:image / twitter:image)\n");
    md.push_str("updated: 2025-06-01      # optional last-modified date → JSON-LD dateModified\n");
    md.push_str("tags:                     # optional\n");
    md.push_str("  - tag1\n");
    md.push_str("  - tag2\n");
    md.push_str("draft: true              # optional, hides from default build\n");
    md.push_str("slug: custom-slug        # optional, overrides auto-generated slug\n");
    md.push_str("template: custom.html    # optional, overrides collection default template\n");
    md.push_str("robots: noindex          # optional, per-page <meta name=\"robots\">\n");
    md.push_str("---\n\n");
    md.push_str("Markdown content here.\n");
    md.push_str("```\n\n");

    // Homepage
    if collections.iter().any(|c| c.name == "pages") {
        md.push_str("### Homepage\n\n");
        md.push_str("To add custom content to the homepage, create `content/pages/index.md`. ");
        md.push_str("Its rendered content will appear above the collection listings on the index page. ");
        md.push_str(
            "The homepage is injected as `{{ page.content }}` in the index template.\n\n",
        );
    }

    // Multi-language
    md.push_str("## Multi-language Support\n\n");
    md.push_str("Add translations by configuring languages in `page.toml` and creating translated content files:\n\n");
    md.push_str("```toml\n");
    md.push_str("# page.toml\n");
    md.push_str("[languages.es]\n");
    md.push_str("title = \"Mi Sitio\"              # optional title override\n");
    md.push_str("description = \"Un sitio web\"     # optional description override\n");
    md.push_str("```\n\n");
    md.push_str("Then create translated files with a language suffix before `.md`:\n\n");
    md.push_str("```\n");
    md.push_str("content/pages/about.md       # English (default) → /about\n");
    md.push_str("content/pages/about.es.md    # Spanish            → /es/about\n");
    if collections.iter().any(|c| c.has_date) {
        md.push_str("content/posts/2025-01-15-hello.es.md  # Spanish post → /es/posts/hello\n");
    }
    md.push_str("```\n\n");
    md.push_str("- Default language content lives at the root URL (`/about`)\n");
    md.push_str("- Other languages get a `/{lang}/` prefix (`/es/about`)\n");
    md.push_str("- Items with the same slug across languages are automatically linked as translations\n");
    md.push_str("- Per-language RSS feeds, sitemaps with hreflang alternates, and discovery files are generated automatically\n");
    md.push_str("- Without `[languages.*]` config, the site is single-language and works as normal\n\n");

    // Templates and themes
    md.push_str("## Templates and Themes\n\n");
    md.push_str("Templates use [Tera](https://keats.github.io/tera/) syntax (Jinja2-compatible). All templates extend `base.html`.\n\n");
    md.push_str("### Available Themes\n\n");
    md.push_str("| Theme | Description |\n");
    md.push_str("|-------|-------------|\n");
    md.push_str("| `default` | Clean, readable with system fonts |\n");
    md.push_str("| `minimal` | Typography-first, serif |\n");
    md.push_str("| `dark` | Dark mode (true black, violet accent) |\n");
    md.push_str("| `docs` | Sidebar layout for documentation |\n");
    md.push_str("| `brutalist` | Neo-brutalist: thick borders, hard shadows, yellow accent |\n");
    md.push_str("| `bento` | Card grid layout with rounded corners and soft shadows |\n\n");
    md.push_str("Apply with `page theme apply <name>`. This overwrites `templates/base.html`.\n\n");

    md.push_str("### Template Variables\n\n");
    md.push_str("Available in all templates:\n\n");
    md.push_str("| Variable | Type | Description |\n");
    md.push_str("|----------|------|-------------|\n");
    md.push_str("| `site.title` | string | Site title (language-specific if multilingual) |\n");
    md.push_str("| `site.description` | string | Site description |\n");
    md.push_str("| `site.base_url` | string | Base URL (e.g., `https://example.com`) |\n");
    md.push_str("| `site.language` | string | Language code (e.g., `en`) |\n");
    md.push_str("| `site.author` | string | Author name |\n");
    md.push_str("| `lang` | string | Current page language code |\n");
    md.push_str("| `translations` | array | Translation links `[{lang, url}]` (empty if no translations) |\n");
    md.push_str("| `page.title` | string | Page title |\n");
    md.push_str("| `page.content` | string | Rendered HTML (use `{{ page.content \\| safe }}`) |\n");
    md.push_str("| `page.date` | string? | Publish date (if set) |\n");
    md.push_str("| `page.updated` | string? | Last-modified date (from `updated:` frontmatter) |\n");
    md.push_str("| `page.description` | string? | Page description |\n");
    md.push_str("| `page.image` | string? | Social-preview image URL (from `image:` frontmatter) |\n");
    md.push_str("| `page.tags` | array | Tags |\n");
    md.push_str("| `page.url` | string | URL path |\n");
    md.push_str("| `page.collection` | string | Collection name (e.g., `posts`) — empty string on homepage |\n");
    md.push_str("| `page.robots` | string? | Per-page robots directive (from `robots:` frontmatter) |\n");
    md.push_str("| `nav` | array | Sidebar nav sections `[{name, label, items: [{title, url, active}]}]` |\n\n");
    md.push_str("Index template also gets:\n\n");
    md.push_str("| Variable | Type | Description |\n");
    md.push_str("|----------|------|-------------|\n");
    md.push_str("| `collections` | array | Listed collections `[{name, label, items}]` |\n");
    md.push_str("| `page` | object? | Homepage content (if `content/pages/index.md` exists) |\n\n");

    md.push_str("### Customizing Templates\n\n");
    md.push_str("Edit files in `templates/` to customize. Key rules:\n\n");
    md.push_str("- `base.html` is the root layout — all other templates extend it via `{% extends \"base.html\" %}`\n");
    md.push_str("- Content goes in `{% block content %}...{% endblock %}`\n");
    md.push_str("- Title goes in `{% block title %}...{% endblock %}`\n");
    md.push_str("- When editing `base.html`, preserve these for full functionality:\n");
    md.push_str("  - `<html lang=\"{{ site.language }}\">` — language attribute\n");
    md.push_str("  - `<link rel=\"canonical\">` — canonical URL (required for SEO)\n");
    md.push_str("  - Open Graph tags: `og:type`, `og:url`, `og:title`, `og:description`, `og:site_name`, `og:locale`\n");
    md.push_str("  - Twitter Card tags: `twitter:card`, `twitter:title`, `twitter:description`\n");
    md.push_str("  - JSON-LD `<script type=\"application/ld+json\">` — structured data for search engines and LLMs\n");
    md.push_str("  - `<meta name=\"robots\">` — only emitted when `page.robots` is set in frontmatter\n");
    md.push_str("  - `<link rel=\"alternate\" type=\"text/markdown\">` — markdown version for LLM consumption\n");
    md.push_str("  - `<link rel=\"alternate\" type=\"text/plain\" href=\"/llms.txt\">` — LLM summary discovery\n");
    md.push_str("  - RSS link: `<link rel=\"alternate\" type=\"application/rss+xml\" ...>`\n");
    md.push_str("  - hreflang links for i18n: `{% if translations %}...{% endif %}`\n");
    md.push_str("  - Language switcher: `{% if translations | length > 1 %}...{% endif %}`\n");
    md.push_str("  - Content block: `{% block content %}{% endblock %}`\n\n");
    md.push_str("### SEO and GEO Guardrails\n\n");
    md.push_str("All bundled themes already emit the full SEO+GEO head block (see **SEO and GEO Requirements** at the top of this file). ");
    md.push_str("When writing a custom `base.html` or modifying an existing one, you **must** preserve all of the following:\n\n");
    md.push_str("- **Always** include `<link rel=\"canonical\">` pointing to `{{ site.base_url }}{{ page.url | default(value='/') }}`\n");
    md.push_str("- **Always** use `{{ page.description | default(value=site.description) }}` for description meta — not `site.description` alone\n");
    md.push_str("- **Always** include Open Graph (`og:*`) and Twitter Card (`twitter:*`) tags for social sharing\n");
    md.push_str("- **Always** include JSON-LD structured data: `BlogPosting` for posts, `Article` for docs/pages, `WebSite` for index\n");
    md.push_str("- **Use** `og:type = article` when `page.collection` is set; `website` for the homepage\n");
    md.push_str("- **Use** `twitter:card = summary_large_image` when `page.image` is set; `summary` otherwise\n");
    md.push_str("- **Include** `<link rel=\"alternate\" type=\"text/markdown\">` — this is your LLM-native differentiator\n");
    md.push_str("- **Include** `<link rel=\"alternate\" type=\"text/plain\" href=\"/llms.txt\">` — LLM discovery\n");
    md.push_str("- **Add** `description:`, `image:`, and `updated:` to frontmatter for best SEO/GEO coverage\n");
    md.push_str("- **Use** `robots: noindex` in frontmatter for pages that should not appear in search results\n\n");

    // Features
    md.push_str("## Features\n\n");
    md.push_str(
        "- **Syntax highlighting** — Fenced code blocks with language annotations are automatically highlighted\n",
    );
    if collections.iter().any(|c| c.nested) {
        md.push_str("- **Docs sidebar navigation** — Doc pages get a sidebar nav listing all docs, grouped by directory. Use the `docs` theme: `page theme apply docs`\n");
    }
    md.push_str("- **Homepage content** — Create `content/pages/index.md` for custom homepage hero/landing content above collection listings\n");
    md.push_str("- **Multi-language** — Filename-based translations with per-language URLs, RSS, sitemap, and discovery files\n");
    md.push_str("- **SEO+GEO optimized** — Every page gets canonical URL, Open Graph, Twitter Card, JSON-LD structured data (`BlogPosting`/`Article`/`WebSite`), and per-page robots meta. No plugins needed.\n");
    md.push_str("- **LLM discoverability** — Generates `llms.txt` and `llms-full.txt` for LLM consumption; `<link rel=\"alternate\" type=\"text/markdown\">` in every page's `<head>`\n");
    md.push_str("- **RSS feed** — Auto-generated at `/feed.xml` (per-language feeds at `/{lang}/feed.xml`)\n");
    md.push_str("- **Sitemap** — Auto-generated at `/sitemap.xml` with hreflang alternates\n");
    md.push_str("- **Search** — `dist/search-index.json` is auto-generated every build; the default theme includes a client-side search input that queries it. No config needed.\n");
    md.push_str("- **Asset pipeline** — Add `minify = true` and/or `fingerprint = true` to `[build]` in `page.toml` to minify CSS/JS and add content-hash suffixes (`main.a1b2c3d4.css`) with a `dist/asset-manifest.json`\n");
    md.push_str("- **Markdown output** — Every page gets a `.md` file alongside `.html` in `dist/`\n");
    md.push_str("- **Clean URLs** — `/posts/hello-world` (no `.html` extension)\n");
    md.push_str("- **Draft exclusion** — `draft: true` in frontmatter hides from builds (use `--drafts` to include)\n");
    md.push_str("- **Shortcodes** — Reusable content components in markdown. See syntax below.\n\n");

    // Shortcodes
    md.push_str("## Shortcodes\n\n");
    md.push_str("Shortcodes are reusable content components you can use inside markdown files.\n\n");
    md.push_str("### Inline shortcodes (raw HTML output)\n\n");
    md.push_str("```markdown\n");
    md.push_str("{{< youtube(id=\"dQw4w9WgXcQ\") >}}\n");
    md.push_str("{{< vimeo(id=\"123456\") >}}\n");
    md.push_str("{{< gist(user=\"octocat\", id=\"abc123\") >}}\n");
    md.push_str("{{< figure(src=\"/static/photo.jpg\", caption=\"A great photo\", alt=\"Description\") >}}\n");
    md.push_str("```\n\n");
    md.push_str("### Body shortcodes (markdown-processed body)\n\n");
    md.push_str("```markdown\n");
    md.push_str("{{% callout(type=\"warning\") %}}\n");
    md.push_str("This is **bold** markdown inside a callout box.\n");
    md.push_str("{{% end %}}\n");
    md.push_str("```\n\n");
    md.push_str("Callout types: `info`, `warning`, `danger`, `tip`\n\n");
    md.push_str("### Built-in shortcodes\n\n");
    md.push_str("| Shortcode | Type | Parameters |\n");
    md.push_str("|-----------|------|------------|\n");
    md.push_str("| `youtube` | inline | `id` (required), `start`, `title` |\n");
    md.push_str("| `vimeo` | inline | `id` (required), `title` |\n");
    md.push_str("| `gist` | inline | `user` (required), `id` (required) |\n");
    md.push_str("| `figure` | inline | `src` (required), `alt`, `caption`, `width`, `height`, `class` |\n");
    md.push_str("| `callout` | body | `type` (default: `info`) |\n\n");
    md.push_str("### Custom shortcodes\n\n");
    md.push_str("Create Tera templates in `templates/shortcodes/`. Example `templates/shortcodes/alert.html`:\n\n");
    md.push_str("```html\n");
    md.push_str("<div class=\"alert alert-{{ level }}\">{{ body }}</div>\n");
    md.push_str("```\n\n");
    md.push_str("Use in markdown: `{{% alert(level=\"error\") %}}Something went wrong{{% end %}}`\n\n");
    md.push_str("Shortcode templates have access to `{{ page }}` and `{{ site }}` context variables.\n\n");

    // Design prompts
    md.push_str("## Design Prompts\n\n");
    md.push_str("When asked to redesign or create a theme, use one of these directions as a starting point.\n");
    md.push_str("Edit `templates/base.html` directly — or apply a bundled theme first with `page theme apply <name>` then edit.\n\n");

    md.push_str("**Minimal / Editorial** — Single column max 620px, Georgia serif body, geometric sans for UI elements.\n");
    md.push_str("No decorative elements. Bottom-border-only search input. White/off-white (`#FAF9F6`) background,\n");
    md.push_str("near-black (`#1A1A1A`) text, one muted link accent. Typography carries all personality.\n\n");

    md.push_str("**Bold / Neo-Brutalist** — Thick black borders (3px solid `#000000`), hard non-blurred box shadows\n");
    md.push_str("(`6px 6px 0 #000`). No border-radius. Saturated fill: yellow `#FFE600`, lime `#AAFF00`, or coral `#FF4D00`.\n");
    md.push_str("Cream (`#FFFEF0`) background. Font-weight 900. Headlines 4rem+. Buttons shift their shadow on hover to press in.\n\n");

    md.push_str("**Bento / Card Grid** — Responsive CSS grid, gap 16px, all cards border-radius 20px. Mixed card sizes\n");
    md.push_str("(1-, 2-, 3-col spans). Cards have independent background colors. Floating shadow:\n");
    md.push_str("`box-shadow: 0 4px 24px rgba(0,0,0,0.08)`. Warm neutral palette (`#F5F0EB`) with one dark-accent card per row.\n\n");

    md.push_str("**Dark / Expressive** — True black (`#000000` or `#0A0A0A`) surfaces. One neon accent:\n");
    md.push_str("green `#00FF87`, blue `#0066FF`, or violet `#8B5CF6`. Off-white text (`#E8E8E8`).\n");
    md.push_str("Translucent nav with `backdrop-filter: blur(12px)`. Visible, styled focus rings.\n\n");

    md.push_str("**Glass / Aurora** — Gradient mesh background (violet `#7B2FBE` → teal `#00C9A7`, or\n");
    md.push_str("indigo `#1A1040` → electric blue `#4361EE`). Floating panels: `backdrop-filter: blur(16px)`,\n");
    md.push_str("`rgba(255,255,255,0.10)` fill, `1px solid rgba(255,255,255,0.2)` border. Use for cards/nav only.\n\n");

    md.push_str("**Accessible / High-Contrast** — WCAG AAA ratios. Min 16px body. 3px colored focus rings\n");
    md.push_str("(design feature, not afterthought). Min 44px click targets. One semantic accent. No color-only\n");
    md.push_str("information. Full `prefers-reduced-motion: reduce` support.\n\n");

    // Key conventions
    md.push_str("## Key Conventions\n\n");
    md.push_str("- Run `page build` after creating or editing content to regenerate the site\n");
    md.push_str("- URLs are clean (no extension): `/posts/hello-world` on disk is `dist/posts/hello-world.html`\n");
    md.push_str("- Templates use Tera syntax and extend `base.html`\n");
    md.push_str("- Use `{{ page.content | safe }}` to render HTML content (the `safe` filter is required)\n");
    md.push_str("- Themes only replace `base.html` — collection templates (`post.html`, `doc.html`, `page.html`) are separate\n");
    md.push_str("- The `static/` directory is copied as-is to `dist/static/` during build\n");
    md.push_str("- Pagination: add `paginate = 10` to a `[[collections]]` block in `page.toml` to generate `/posts/`, `/posts/page/2/`, etc.\n");
    md.push_str("  Use `{% if pagination %}<nav>...</nav>{% endif %}` in templates; variables: `pagination.current_page`, `pagination.total_pages`, `pagination.prev_url`, `pagination.next_url`\n");
    md.push_str("- Search is always enabled: `dist/search-index.json` is generated every build. All bundled themes include a search box wired to it. No config needed.\n");
    md.push_str("- Asset pipeline: set `minify = true` and/or `fingerprint = true` under `[build]` in `page.toml`\n");
    md.push_str("  - `minify` strips CSS/JS comments and collapses whitespace\n");
    md.push_str("  - `fingerprint` writes `file.<hash8>.ext` copies of each static asset and a `dist/asset-manifest.json` mapping original names to fingerprinted names\n");
    md.push_str("- Custom theme: `page theme create \"your design description\"` generates `templates/base.html` with Claude (requires Claude Code)\n");

    md
}

/// Convert a plural collection name to singular for display.
fn singularize(name: &str) -> &str {
    match name {
        "posts" => "post",
        "docs" => "doc",
        "pages" => "page",
        _ => name,
    }
}
