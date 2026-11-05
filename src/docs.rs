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
        cms(),
        deployment(),
        agent(),
        workspace(),
        mcp_server(),
        theme_gallery(),
        cli_reference(),
        audit(),
        import(),
        errors(),
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

fn cms() -> DocPage {
    DocPage {
        slug: "cms",
        title: "Git-based CMS",
        description: "Set up Pages CMS or Decap CMS for your collections with `seite cms setup`.",
        weight: 16,
        raw_content: include_str!("../seite-sh/content/docs/cms.md"),
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
        title: "Coding Agents",
        description: "Set up and run the site with Claude Code, Codex, OpenCode, Cursor, Gemini CLI or GitHub Copilot.",
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

fn audit() -> DocPage {
    DocPage {
        slug: "audit",
        title: "Audit",
        description: "seite audit: a free, offline SEO, AI-visibility (GEO), accessibility, content and performance audit of your built site, with the source file to fix for every finding.",
        weight: 11,
        raw_content: include_str!("../seite-sh/content/docs/audit.md"),
    }
}

fn import() -> DocPage {
    DocPage {
        slug: "import",
        title: "Import",
        description: "seite import: move a WordPress, Ghost, Jekyll or Hugo site into seite in one command, with images, old-URL redirects and a report your coding agent can finish.",
        weight: 12,
        raw_content: include_str!("../seite-sh/content/docs/import.md"),
    }
}

fn errors() -> DocPage {
    DocPage {
        slug: "errors",
        title: "Error Reference",
        description: "Every diagnostic code reported by seite check and seite build, and every seite audit finding: what it means, common causes, and how to fix it.",
        weight: 12,
        raw_content: include_str!("../seite-sh/content/docs/errors.md"),
    }
}

/// The `## <code>` sections of the error reference, in document order, as
/// `(code, body)` pairs (body without the heading). Backs `seite explain`.
pub fn error_sections() -> Vec<(&'static str, &'static str)> {
    let body = strip_frontmatter(errors().raw_content);
    let mut sections: Vec<(&'static str, &'static str)> = Vec::new();
    let mut current: Option<(&'static str, usize)> = None;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some((code, start)) = current.take() {
                sections.push((code, &body[start..offset]));
            }
            current = Some((heading.trim(), offset + line.len()));
        }
        offset += line.len();
    }
    if let Some((code, start)) = current {
        sections.push((code, &body[start..]));
    }
    sections
}

/// One release's notes, from the changelog entries compiled into the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNote {
    pub version: (u64, u64, u64),
    /// The entry's heading, e.g. `v0.21.0`.
    pub title: String,
    /// The entry's body (its own `##` headings included), trimmed.
    pub body: String,
}

/// Every release note embedded in this binary, newest first (the changelog
/// order).
pub fn release_notes() -> Vec<ReleaseNote> {
    parse_release_notes(strip_frontmatter(releases().raw_content))
}

/// Split the generated `releases.md` body into entries. An entry starts at a
/// `## v<major>.<minor>.<patch>` line; other `##` lines belong to the body.
pub fn parse_release_notes(text: &str) -> Vec<ReleaseNote> {
    fn heading_version(line: &str) -> Option<(u64, u64, u64)> {
        let word = line.strip_prefix("## ")?.split_whitespace().next()?;
        let mut parts = word.trim_start_matches('v').split('.');
        let v = (
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        );
        parts.next().is_none().then_some(v)
    }

    let mut notes: Vec<ReleaseNote> = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        match heading_version(line).filter(|_| !in_fence) {
            Some(version) => notes.push(ReleaseNote {
                version,
                title: line.trim_start_matches("## ").trim().to_string(),
                body: String::new(),
            }),
            None => {
                if let Some(last) = notes.last_mut() {
                    last.body.push_str(line);
                    last.body.push('\n');
                }
            }
        }
    }
    for note in &mut notes {
        note.body = note.body.trim().to_string();
    }
    notes
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
            docs.len() >= 18,
            "Expected at least 18 docs, got {}",
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
        assert!(by_slug("audit").is_some());
    }

    #[test]
    fn test_error_sections_parse_codes_and_bodies() {
        let sections = error_sections();
        assert!(sections.len() >= 18, "{}", sections.len());
        for (code, body) in &sections {
            assert!(
                code.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "heading `{code}` is not a diagnostic code"
            );
            assert!(!body.contains("\n## "), "{code} swallowed the next section");
            assert!(body.trim().len() > 40, "{code} has no explanation");
        }
        assert!(sections.iter().any(|(c, _)| *c == "broken-link"));
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

    #[test]
    fn test_parse_release_notes_splits_on_version_headings_only() {
        let text = "\n\n## v0.2.0\nbody two\n\n## A sub heading\nmore\n```\n## v9.9.9\n```\n\n## v0.1.0\nbody one\n";
        let notes = parse_release_notes(text);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].version, (0, 2, 0));
        assert_eq!(notes[0].title, "v0.2.0");
        assert!(notes[0].body.contains("## A sub heading"));
        assert!(notes[0].body.contains("## v9.9.9"));
        assert_eq!(notes[1].body, "body one");
    }

    #[test]
    fn test_release_notes_embedded() {
        let notes = release_notes();
        assert!(!notes.is_empty());
        assert!(notes.iter().any(|n| n.version == (0, 20, 2)));
    }
}
