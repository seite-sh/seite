//! Embedded documentation pages compiled into the binary.
//!
//! Follows the same `include_str!` pattern as `src/themes.rs`. The source files
//! live in `seite-sh/content/docs/` — the same files that deploy to seite.sh.
//! Single source of truth: edit once, both the binary and the website update.

/// A documentation page embedded in the binary.
pub struct DocPage {
    pub slug: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub weight: i32,
    pub raw_content: &'static str,
}

/// Return all embedded documentation pages, sorted by weight.
pub fn all() -> Vec<DocPage> {
    let mut pages = vec![
        getting_started(),
        configuration(),
        collections(),
        templates(),
        custom_themes(),
        shortcodes(),
        i18n(),
        trust_center(),
        contact_forms(),
        deployment(),
        agent(),
        workspace(),
        mcp_server(),
        theme_gallery(),
        cli_reference(),
        releases(),
    ];
    pages.sort_by_key(|p| p.weight);
    pages
}

/// Find a documentation page by slug.
pub fn by_slug(slug: &str) -> Option<DocPage> {
    all().into_iter().find(|d| d.slug == slug)
}

/// Strip YAML frontmatter from raw markdown, returning only the body.
pub fn strip_frontmatter(raw: &str) -> &str {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return raw;
    }
    let after_first = &trimmed[3..];
    match after_first.find("\n---") {
        Some(end) => {
            let body = &after_first[end + 4..];
            body.trim_start_matches('\n').trim_start_matches('\r')
        }
        None => raw,
    }
}

fn getting_started() -> DocPage {
    DocPage {
        slug: "getting-started",
        title: "Getting Started",
        description: "Install seite and build your first static site in under a minute.",
        weight: 1,
        raw_content: include_str!("../seite-sh/content/docs/getting-started.md"),
    }
}

fn configuration() -> DocPage {
    DocPage {
        slug: "configuration",
        title: "Configuration",
        description: "Complete seite.toml reference — site settings, collections, build options, deployment, languages, and images.",
        weight: 2,
        raw_content: include_str!("../seite-sh/content/docs/configuration.md"),
    }
}

fn collections() -> DocPage {
    DocPage {
        slug: "collections",
        title: "Collections",
        description: "How posts, docs, and pages work — presets, custom collections, and configuration options.",
        weight: 3,
        raw_content: include_str!("../seite-sh/content/docs/collections.md"),
    }
}

fn templates() -> DocPage {
    DocPage {
        slug: "templates",
        title: "Templates & Themes",
        description: "Tera template variables, blocks, data files, and theme customization.",
        weight: 4,
        raw_content: include_str!("../seite-sh/content/docs/templates.md"),
    }
}

fn custom_themes() -> DocPage {
    DocPage {
        slug: "custom-themes",
        title: "Building Custom Themes",
        description: "Step-by-step guide to creating a custom theme from scratch — template structure, CSS, SEO requirements, and testing.",
        weight: 5,
        raw_content: include_str!("../seite-sh/content/docs/custom-themes.md"),
    }
}

fn shortcodes() -> DocPage {
    DocPage {
        slug: "shortcodes",
        title: "Shortcodes",
        description: "Reusable content components in markdown — built-in and custom shortcodes.",
        weight: 6,
        raw_content: include_str!("../seite-sh/content/docs/shortcodes.md"),
    }
}

fn i18n() -> DocPage {
    DocPage {
        slug: "i18n",
        title: "Multi-Language",
        description: "Filename-based translation system with per-language URLs, RSS, sitemap, and discovery files.",
        weight: 7,
        raw_content: include_str!("../seite-sh/content/docs/i18n.md"),
    }
}

fn trust_center() -> DocPage {
    DocPage {
        slug: "trust-center",
        title: "Trust Center",
        description: "Build a compliance hub with certifications, subprocessors, FAQs, and security policies.",
        weight: 8,
        raw_content: include_str!("../seite-sh/content/docs/trust-center.md"),
    }
}

fn contact_forms() -> DocPage {
    DocPage {
        slug: "contact-forms",
        title: "Contact Forms",
        description: "Add contact forms to your static site with built-in provider support.",
        weight: 15,
        raw_content: include_str!("../seite-sh/content/docs/contact-forms.md"),
    }
}

fn deployment() -> DocPage {
    DocPage {
        slug: "deployment",
        title: "Deployment",
        description: "Deploy to GitHub Pages, Cloudflare Pages, or Netlify.",
        weight: 8,
        raw_content: include_str!("../seite-sh/content/docs/deployment.md"),
    }
}

fn agent() -> DocPage {
    DocPage {
        slug: "agent",
        title: "AI Agent",
        description: "Use Claude Code as an AI assistant with full site context.",
        weight: 9,
        raw_content: include_str!("../seite-sh/content/docs/agent.md"),
    }
}

fn workspace() -> DocPage {
    DocPage {
        slug: "workspace",
        title: "Workspaces",
        description: "Manage multiple sites in a single repository.",
        weight: 9,
        raw_content: include_str!("../seite-sh/content/docs/workspace.md"),
    }
}

fn mcp_server() -> DocPage {
    DocPage {
        slug: "mcp-server",
        title: "MCP Server",
        description: "Structured AI access to site content, configuration, themes, and build tools via the Model Context Protocol.",
        weight: 10,
        raw_content: include_str!("../seite-sh/content/docs/mcp-server.md"),
    }
}

fn theme_gallery() -> DocPage {
    DocPage {
        slug: "theme-gallery",
        title: "Theme Gallery",
        description: "Visual showcase of all bundled themes.",
        weight: 10,
        raw_content: include_str!("../seite-sh/content/docs/theme-gallery.md"),
    }
}

fn cli_reference() -> DocPage {
    DocPage {
        slug: "cli-reference",
        title: "CLI Reference",
        description: "Complete reference for all seite CLI commands, flags, and options.",
        weight: 11,
        raw_content: include_str!("../seite-sh/content/docs/cli-reference.md"),
    }
}

fn releases() -> DocPage {
    DocPage {
        slug: "releases",
        title: "Releases",
        description: "Version history and release notes.",
        weight: 12,
        raw_content: include_str!(concat!(env!("OUT_DIR"), "/releases.md")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_returns_all_docs() {
        let docs = all();
        assert!(
            docs.len() >= 15,
            "Expected at least 15 docs, got {}",
            docs.len()
        );
    }

    #[test]
    fn test_all_sorted_by_weight() {
        let docs = all();
        for i in 1..docs.len() {
            assert!(
                docs[i].weight >= docs[i - 1].weight,
                "Docs not sorted: {} (weight {}) came after {} (weight {})",
                docs[i].slug,
                docs[i].weight,
                docs[i - 1].slug,
                docs[i - 1].weight
            );
        }
    }

    #[test]
    fn test_by_slug_found() {
        assert!(by_slug("configuration").is_some());
        assert!(by_slug("getting-started").is_some());
        assert!(by_slug("cli-reference").is_some());
    }

    #[test]
    fn test_by_slug_not_found() {
        assert!(by_slug("nonexistent").is_none());
    }

    #[test]
    fn test_strip_frontmatter() {
        let raw = "---\ntitle: \"Test\"\n---\n\n## Hello\n\nWorld";
        let body = strip_frontmatter(raw);
        assert!(body.starts_with("## Hello"), "Got: {body}");
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let raw = "## Hello\n\nWorld";
        let body = strip_frontmatter(raw);
        assert_eq!(body, raw);
    }

    #[test]
    fn test_all_docs_have_content() {
        for doc in all() {
            assert!(
                !doc.raw_content.is_empty(),
                "Doc {} has empty content",
                doc.slug
            );
            assert!(
                doc.raw_content.contains("---"),
                "Doc {} is missing frontmatter",
                doc.slug
            );
        }
    }

    #[test]
    fn test_all_docs_have_unique_slugs() {
        let docs = all();
        let mut slugs: Vec<_> = docs.iter().map(|d| d.slug).collect();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), docs.len(), "Duplicate slugs found");
    }
}
