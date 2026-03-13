use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::output::human;

#[derive(Args)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Subcommand)]
pub enum SkillCommand {
    /// Install a skill pack or individual skill from a URL
    Install {
        /// Known pack name (e.g., "seomachine") or URL to a SKILL.md file
        source: String,

        /// Override the skill name (for URL installs)
        #[arg(long)]
        name: Option<String>,
    },

    /// List installed skills and skill packs
    List,

    /// Remove an installed skill pack or individual skill
    Remove {
        /// Pack or skill name to remove
        name: String,
    },

    /// Update installed skills from their original sources
    Update {
        /// Specific pack or skill to update (updates all if omitted)
        name: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Known skill packs
// ---------------------------------------------------------------------------

struct SkillPack {
    name: &'static str,
    description: &'static str,
    repo: &'static str,
    branch: &'static str,
    agents: &'static [&'static str],
    commands: &'static [&'static str],
    skills: &'static [&'static str],
    context_files: &'static [&'static str],
    python_scripts: &'static [&'static str],
}

fn known_packs() -> &'static [SkillPack] {
    &[SkillPack {
        name: "seomachine",
        description:
            "SEO content research, writing, and optimization — by SEOMachine (seomachine.com)",
        repo: "TheCraigHewitt/seomachine",
        branch: "4e04990d4b79745275a013b613c597d9189fc142",
        agents: &[
            "cluster-strategist.md",
            "content-analyzer.md",
            "cro-analyst.md",
            "editor.md",
            "headline-generator.md",
            "internal-linker.md",
            "keyword-mapper.md",
            "landing-page-optimizer.md",
            "meta-creator.md",
            "performance.md",
            "seo-optimizer.md",
        ],
        commands: &[
            "analyze-existing.md",
            "article.md",
            "cluster.md",
            "content-calendar.md",
            "landing-audit.md",
            "landing-competitor.md",
            "landing-publish.md",
            "landing-research.md",
            "landing-write.md",
            "optimize.md",
            "performance-review.md",
            "priorities.md",
            "publish-draft.md",
            "research-gaps.md",
            "research-performance.md",
            "research-serp.md",
            "research-topics.md",
            "research-trending.md",
            "research.md",
            "rewrite.md",
            "scrub.md",
            "write.md",
        ],
        skills: &[
            "ab-test-setup",
            "analytics-tracking",
            "competitor-alternatives",
            "content-strategy",
            "copy-editing",
            "copywriting",
            "email-sequence",
            "form-cro",
            "free-tool-strategy",
            "launch-strategy",
            "marketing-ideas",
            "marketing-psychology",
            "onboarding-cro",
            "page-cro",
            "paid-ads",
            "paywall-upgrade-cro",
            "popup-cro",
            "pricing-strategy",
            "product-marketing-context",
            "programmatic-seo",
            "referral-program",
            "schema-markup",
            "seo-audit",
            "signup-flow-cro",
            "social-content",
        ],
        context_files: &[
            "brand-voice.md",
            "competitor-analysis.md",
            "cro-best-practices.md",
            "features.md",
            "internal-links-map.md",
            "seo-guidelines.md",
            "style-guide.md",
            "target-keywords.md",
            "writing-examples.md",
        ],
        python_scripts: &[
            // Top-level scripts
            "research_competitor_gaps.py",
            "research_performance_matrix.py",
            "research_priorities_comprehensive.py",
            "research_quick_wins.py",
            "research_serp_analysis.py",
            "research_topic_clusters.py",
            "research_trending.py",
            "seo_baseline_analysis.py",
            "seo_bofu_rankings.py",
            "seo_competitor_analysis.py",
            "test_dataforseo.py",
            // data_sources modules
            "data_sources/modules/above_fold_analyzer.py",
            "data_sources/modules/article_planner.py",
            "data_sources/modules/competitor_gap_analyzer.py",
            "data_sources/modules/content_length_comparator.py",
            "data_sources/modules/content_scorer.py",
            "data_sources/modules/content_scrubber.py",
            "data_sources/modules/cro_checker.py",
            "data_sources/modules/cta_analyzer.py",
            "data_sources/modules/data_aggregator.py",
            "data_sources/modules/dataforseo.py",
            "data_sources/modules/engagement_analyzer.py",
            "data_sources/modules/google_analytics.py",
            "data_sources/modules/google_search_console.py",
            "data_sources/modules/keyword_analyzer.py",
            "data_sources/modules/landing_page_scorer.py",
            "data_sources/modules/landing_performance.py",
            "data_sources/modules/opportunity_scorer.py",
            "data_sources/modules/readability_scorer.py",
            "data_sources/modules/search_intent_analyzer.py",
            "data_sources/modules/section_writer.py",
            "data_sources/modules/seo_quality_rater.py",
            "data_sources/modules/social_research_aggregator.py",
            "data_sources/modules/trust_signal_analyzer.py",
            "data_sources/modules/wordpress_publisher.py",
            // data_sources config
            "data_sources/config/.env.example",
            "config/competitors.example.json",
        ],
    }]
}

fn find_pack(name: &str) -> Option<&'static SkillPack> {
    known_packs().iter().find(|p| p.name == name)
}

// ---------------------------------------------------------------------------
// Source tracking
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
struct SkillPacksManifest {
    packs: HashMap<String, PackEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PackEntry {
    source: String,
    branch: String,
    installed_at: String,
    files: Vec<String>,
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join(".claude").join(".seite-skill-packs.json")
}

fn load_manifest(root: &Path) -> SkillPacksManifest {
    let path = manifest_path(root);
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        SkillPacksManifest::default()
    }
}

fn save_manifest(root: &Path, manifest: &SkillPacksManifest) -> anyhow::Result<()> {
    let path = manifest_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(&path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API for agent prompt integration
// ---------------------------------------------------------------------------

/// Summary of installed skill packs and context files, for agent prompt injection.
pub struct SkillPackSummary {
    pub packs: Vec<InstalledPackInfo>,
    pub context_files: Vec<String>,
    pub custom_skills: Vec<String>,
}

pub struct InstalledPackInfo {
    pub name: String,
    pub description: String,
    pub agents: Vec<String>,
    pub commands: Vec<String>,
    pub skills: Vec<String>,
}

/// Gather skill pack information for the agent system prompt.
pub fn gather_skill_summary(root: &Path) -> SkillPackSummary {
    let manifest = load_manifest(root);
    let bundled_skills = ["theme-builder", "brand-identity", "landing-page"];

    let mut packs = Vec::new();
    let mut all_pack_skills: Vec<String> = Vec::new();

    for (name, entry) in &manifest.packs {
        let description = find_pack(name)
            .map(|p| p.description.to_string())
            .unwrap_or_else(|| format!("from {}", entry.source));

        let mut agents = Vec::new();
        let mut commands = Vec::new();
        let mut skills = Vec::new();

        for file in &entry.files {
            if let Some(rest) = file.strip_prefix(".claude/agents/") {
                if let Some(n) = rest.strip_suffix(".md") {
                    agents.push(n.to_string());
                }
            } else if let Some(rest) = file.strip_prefix(".claude/commands/") {
                if let Some(n) = rest.strip_suffix(".md") {
                    commands.push(n.to_string());
                }
            } else if let Some(rest) = file.strip_prefix(".claude/skills/") {
                if let Some(n) = rest.strip_suffix("/SKILL.md") {
                    skills.push(n.to_string());
                    all_pack_skills.push(n.to_string());
                }
            }
        }

        agents.sort();
        commands.sort();
        skills.sort();

        packs.push(InstalledPackInfo {
            name: name.clone(),
            description,
            agents,
            commands,
            skills,
        });
    }

    // Context files
    let context_dir = root.join("context");
    let mut context_files = Vec::new();
    if context_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&context_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        context_files.push(stem.to_string());
                    }
                }
            }
        }
    }
    context_files.sort();

    // Custom skills (not bundled, not in packs)
    let skills_dir = root.join(".claude").join("skills");
    let mut custom_skills = Vec::new();
    if skills_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if bundled_skills.contains(&name.as_str()) {
                    continue;
                }
                if all_pack_skills.contains(&name) {
                    continue;
                }
                if entry.path().join("SKILL.md").exists() {
                    custom_skills.push(name);
                }
            }
        }
    }
    custom_skills.sort();

    SkillPackSummary {
        packs,
        context_files,
        custom_skills,
    }
}

// ---------------------------------------------------------------------------
// HTTP download
// ---------------------------------------------------------------------------

fn download_text(url: &str) -> anyhow::Result<String> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("download failed ({url}): {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow::anyhow!("failed to read response ({url}): {e}"))?;
    Ok(body)
}

fn raw_github_url(repo: &str, branch: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo}/{branch}/{path}")
}

// ---------------------------------------------------------------------------
// Context injection
// ---------------------------------------------------------------------------

fn seite_context_preamble() -> &'static str {
    r#"## seite Integration Context

You are working inside a **seite** static site project (https://seite.sh). Adapt your workflow to use seite's content model and CLI:

### Content model
- Content lives in `content/{collection}/*.md` with YAML frontmatter
- Use `seite new post "Title" --tags tag1,tag2` to create a new post
- Set `draft: true` in frontmatter for work-in-progress drafts
- Run `seite build` to build the site, `seite serve` for live preview, `seite deploy` to publish

### Directory mapping (SEOMachine → seite)
| SEOMachine | seite equivalent |
|---|---|
| `drafts/` | `content/{collection}/` with `draft: true` in frontmatter |
| `published/` | `content/{collection}/` (no draft flag) |
| `research/` | `research/` (same) |
| `output/` | `output/` (same, for agent reports) |
| `topics/` | `topics/` (same, for idea capture) |

### Frontmatter format
```yaml
---
title: "SEO-Optimized Title (50-60 chars)"
date: 2026-03-12
description: "Compelling meta description with primary keyword (150-160 chars)"
tags:
  - keyword1
  - keyword2
image: /static/og-image.png
draft: false
extra:
  primary_keyword: "target keyword"
---
```

### SEO features (automatic)
seite themes automatically handle: canonical URLs, Open Graph tags, Twitter Cards, JSON-LD structured data (BlogPosting/Article/WebSite), hreflang tags (multilingual), robots.txt with AI crawler management, llms.txt + llms-full.txt for AI discovery, and raw markdown alongside HTML. Focus on content quality — technical SEO is built in.

### Publishing workflow
Instead of WordPress REST API, use:
1. Write content to `content/posts/` (or other collection)
2. Remove `draft: true` when ready to publish
3. Run `seite build` to verify
4. Run `seite deploy` to publish

### Site context
- Read `seite.toml` for site configuration (title, collections, base_url, language)
- Read `data/brand.yaml` for brand identity (if created via `/brand-identity`)
- Read `CLAUDE.md` for full site documentation
"#
}

fn seite_publish_draft_command() -> &'static str {
    r#"# Publish Draft — seite Edition

Publish a draft article from `content/` by removing the `draft: true` flag, building the site, and optionally deploying.

## Usage

`/publish-draft <file-path>`

Example: `/publish-draft content/posts/2026-03-12-rust-error-handling.md`

## Workflow

1. **Read the specified file** and verify it exists and has `draft: true` in frontmatter
2. **Run the content scrubber** — check for AI watermarks and telltale patterns (same as SEOMachine's `/scrub`). Fix any issues found
3. **Remove `draft: true`** from the YAML frontmatter (or set it to `draft: false`)
4. **Run `seite build`** to verify the site builds successfully with the new content
5. **Report the result** — show the article title, URL it will be published at, and word count
6. **Ask if user wants to deploy** — if yes, run `seite deploy`

## Important

- Do NOT use WordPress or any external CMS — this is a seite static site
- The article stays in its current location in `content/` — seite serves it from there
- After publishing, the article will appear at its URL (e.g., `/posts/rust-error-handling`)
- Run `seite serve` if the user wants to preview before deploying
"#
}

// ---------------------------------------------------------------------------
// Internal links map generation
// ---------------------------------------------------------------------------

fn generate_internal_links_map(root: &Path) -> String {
    let mut map = String::from("# Internal Links Map\n\n");
    map.push_str("Auto-generated by `seite skill install`. Update with `seite skill update`.\n\n");

    let content_dir = root.join("content");
    if !content_dir.exists() {
        return map;
    }

    // Walk content directories (collections)
    let mut entries: Vec<(String, String, String)> = Vec::new(); // (collection, title, url)

    if let Ok(collections) = fs::read_dir(&content_dir) {
        for collection_entry in collections.flatten() {
            if !collection_entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let collection_name = collection_entry.file_name().to_string_lossy().to_string();

            walk_content_dir(
                &collection_entry.path(),
                &collection_name,
                &collection_name,
                &mut entries,
            );
        }
    }

    // Sort by collection then title
    entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut current_collection = String::new();
    for (collection, title, url) in &entries {
        if *collection != current_collection {
            map.push_str(&format!("\n## {}\n\n", collection));
            current_collection.clone_from(collection);
        }
        map.push_str(&format!("- [{}]({})\n", title, url));
    }

    map
}

fn walk_content_dir(
    dir: &Path,
    collection_name: &str,
    url_prefix: &str,
    entries: &mut Vec<(String, String, String)>,
) {
    let Ok(items) = fs::read_dir(dir) else {
        return;
    };
    for item in items.flatten() {
        let path = item.path();
        if path.is_dir() {
            let subdir = path.file_name().unwrap_or_default().to_string_lossy();
            walk_content_dir(
                &path,
                collection_name,
                &format!("{url_prefix}/{subdir}"),
                entries,
            );
        } else if path.extension().is_some_and(|e| e == "md") {
            let filename = path.file_stem().unwrap_or_default().to_string_lossy();
            if filename == "index" {
                continue;
            }
            // Try to extract title from frontmatter
            let title = extract_title_from_file(&path).unwrap_or_else(|| filename.to_string());
            // Build URL: strip date prefix if present (YYYY-MM-DD-)
            let slug = strip_date_prefix(&filename);
            let url = format!("/{url_prefix}/{slug}");
            entries.push((collection_name.to_string(), title, url));
        }
    }
}

fn extract_title_from_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    if !content.starts_with("---") {
        return None;
    }
    for line in content.lines().skip(1) {
        if line.trim() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("title:") {
            let title = rest.trim().trim_matches('"').trim_matches('\'');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

fn strip_date_prefix(filename: &str) -> &str {
    // Strip YYYY-MM-DD- prefix if present
    if filename.len() > 11 && filename.as_bytes()[4] == b'-' && filename.as_bytes()[7] == b'-' {
        if let Some(rest) = filename.get(11..) {
            if filename[..10]
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-')
            {
                return rest;
            }
        }
    }
    filename
}

// ---------------------------------------------------------------------------
// .gitignore helpers
// ---------------------------------------------------------------------------

fn ensure_gitignore_entries(root: &Path, entries: &[&str]) -> anyhow::Result<()> {
    let gitignore_path = root.join(".gitignore");
    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut additions = Vec::new();
    for entry in entries {
        if !existing.lines().any(|l| l.trim() == *entry) {
            additions.push(*entry);
        }
    }

    if !additions.is_empty() {
        let mut content = existing;
        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }
        content.push_str("\n# SEOMachine\n");
        for entry in &additions {
            content.push_str(entry);
            content.push('\n');
        }
        fs::write(&gitignore_path, content)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CLAUDE.md integration
// ---------------------------------------------------------------------------

const CLAUDE_MD_MARKER_START: &str = "<!-- seite-seomachine-start -->";
const CLAUDE_MD_MARKER_END: &str = "<!-- seite-seomachine-end -->";

fn append_claude_md_section(root: &Path) -> anyhow::Result<()> {
    let claude_md_path = root.join("CLAUDE.md");
    let existing = if claude_md_path.exists() {
        fs::read_to_string(&claude_md_path)?
    } else {
        String::new()
    };

    // Remove any existing SEOMachine section
    let cleaned = remove_claude_md_section(&existing);

    let section = format!(
        r#"
{CLAUDE_MD_MARKER_START}
### SEOMachine Integration

This project has [SEOMachine](https://github.com/TheCraigHewitt/seomachine) installed for SEO content research, writing, and optimization.

**Key commands:** `/research`, `/write`, `/article`, `/optimize`, `/analyze-existing`, `/rewrite`, `/priorities`, `/publish-draft`, `/cluster`, `/performance-review`

**Context files** in `context/` define your brand voice, target keywords, and SEO guidelines. Fill these in for best results.

**Directory layout:**
- `content/posts/` — articles (use `draft: true` for work-in-progress)
- `research/` — research briefs and analysis reports
- `output/` — agent optimization reports
- `topics/` — raw topic ideas
- `context/` — brand voice, style guide, target keywords, competitor analysis

**Analytics (optional):** Python scripts in `scripts/seo/` connect to Google Analytics, Search Console, and DataForSEO. Set up `.env` with API credentials. Run `pip install -r requirements-seo.txt` to install dependencies.

**Publishing:** Use `/publish-draft <file>` to remove draft status, build, and deploy. This replaces SEOMachine's WordPress integration with seite's native deploy.

{seite_preamble}
{CLAUDE_MD_MARKER_END}
"#,
        seite_preamble = seite_context_preamble()
    );

    let mut result = cleaned;
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&section);

    fs::write(&claude_md_path, result)?;
    Ok(())
}

fn remove_claude_md_section(content: &str) -> String {
    if let (Some(start), Some(end)) = (
        content.find(CLAUDE_MD_MARKER_START),
        content.find(CLAUDE_MD_MARKER_END),
    ) {
        let end_pos = end + CLAUDE_MD_MARKER_END.len();
        // Also strip trailing newline after marker
        let end_pos = if content[end_pos..].starts_with('\n') {
            end_pos + 1
        } else {
            end_pos
        };
        // Also strip leading newline before marker
        let start = if start > 0 && content.as_bytes()[start - 1] == b'\n' {
            start - 1
        } else {
            start
        };
        format!("{}{}", &content[..start], &content[end_pos..])
    } else {
        content.to_string()
    }
}

// ---------------------------------------------------------------------------
// .env.example scaffolding
// ---------------------------------------------------------------------------

fn scaffold_env_example(root: &Path) -> anyhow::Result<()> {
    let env_path = root.join(".env.example");
    if env_path.exists() {
        return Ok(());
    }
    fs::write(
        &env_path,
        r#"# SEOMachine Analytics Configuration
# Copy this file to .env and fill in your credentials.
# See: https://github.com/TheCraigHewitt/seomachine/blob/main/data-sources-setup.md

# Google Analytics 4
GA4_PROPERTY_ID=
GA4_CREDENTIALS_PATH=credentials/ga4-service-account.json

# Google Search Console
GSC_SITE_URL=
GSC_CREDENTIALS_PATH=credentials/gsc-service-account.json

# DataForSEO
DATAFORSEO_LOGIN=
DATAFORSEO_PASSWORD=

# WordPress (not used with seite — kept for SEOMachine compatibility)
# WP_URL=
# WP_USERNAME=
# WP_APP_PASSWORD=
"#,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// requirements-seo.txt scaffolding
// ---------------------------------------------------------------------------

fn scaffold_requirements(root: &Path, pack: &SkillPack) -> anyhow::Result<()> {
    let req_path = root.join("requirements-seo.txt");
    if req_path.exists() {
        return Ok(());
    }
    let url = raw_github_url(pack.repo, pack.branch, "data_sources/requirements.txt");
    let content = match download_text(&url) {
        Ok(body) => body,
        Err(_) => {
            // Fallback if download fails
            "google-analytics-data>=0.18.0\n\
             google-auth>=2.23.0\n\
             google-auth-oauthlib>=1.1.0\n\
             google-auth-httplib2>=0.1.1\n\
             google-api-python-client>=2.100.0\n\
             requests>=2.31.0\n\
             python-dotenv>=1.0.0\n\
             beautifulsoup4>=4.12.0\n\
             nltk>=3.8.0\n\
             scikit-learn>=1.3.0\n"
                .to_string()
        }
    };
    fs::write(&req_path, content)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Name derivation and validation
// ---------------------------------------------------------------------------

fn derive_skill_name(url: &str, name_override: Option<&str>) -> String {
    match name_override {
        Some(n) => n.to_string(),
        None => {
            // Try to get a meaningful name from the URL path
            let segments: Vec<&str> = url.trim_end_matches('/').rsplit('/').collect();
            // For .../skills/content-strategy/SKILL.md → "content-strategy"
            // Check if the filename is literally "SKILL.md" (the standard name)
            if segments.len() >= 2 && segments[0].eq_ignore_ascii_case("skill.md") {
                return segments[1].to_string();
            }
            // For .../foo.md → "foo"
            let filename = segments.first().unwrap_or(&"skill");
            filename.strip_suffix(".md").unwrap_or(filename).to_string()
        }
    }
}

fn validate_skill_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.contains(std::path::is_separator) || name.contains("..") {
        return Err(anyhow::anyhow!("invalid skill name: '{}'", name));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI dispatch
// ---------------------------------------------------------------------------

pub fn run(args: &SkillArgs) -> anyhow::Result<()> {
    match &args.command {
        SkillCommand::Install { source, name } => run_install(source, name.as_deref()),
        SkillCommand::List => run_list(),
        SkillCommand::Remove { name } => run_remove(name),
        SkillCommand::Update { name } => run_update(name.as_deref()),
    }
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install(source: &str, name_override: Option<&str>) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;

    // Check if it's a known pack
    if let Some(pack) = find_pack(source) {
        return run_install_pack(&root, pack);
    }

    // Check if it looks like a URL
    if source.starts_with("http://") || source.starts_with("https://") {
        return run_install_url(&root, source, name_override);
    }

    Err(anyhow::anyhow!(
        "unknown skill pack '{}'. Known packs: {}. Or provide a URL to a SKILL.md file.",
        source,
        known_packs()
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install_pack(root: &Path, pack: &SkillPack) -> anyhow::Result<()> {
    human::info(&format!(
        "Installing skill pack '{}': {}",
        pack.name, pack.description
    ));

    let mut installed_files: Vec<String> = Vec::new();
    let mut download_errors: Vec<String> = Vec::new();

    // Download agents
    let agents_dir = root.join(".claude").join("agents");
    fs::create_dir_all(&agents_dir)?;
    human::info(&format!("Downloading {} agents...", pack.agents.len()));
    for agent in pack.agents {
        let url = raw_github_url(pack.repo, pack.branch, &format!(".claude/agents/{agent}"));
        match download_text(&url) {
            Ok(body) => {
                fs::write(agents_dir.join(agent), &body)?;
                installed_files.push(format!(".claude/agents/{agent}"));
            }
            Err(e) => download_errors.push(format!("  agent {agent}: {e}")),
        }
    }

    // Download commands
    let commands_dir = root.join(".claude").join("commands");
    fs::create_dir_all(&commands_dir)?;
    human::info(&format!("Downloading {} commands...", pack.commands.len()));
    for command in pack.commands {
        // Skip publish-draft — we'll write our own seite-native version
        if *command == "publish-draft.md" {
            continue;
        }
        let url = raw_github_url(
            pack.repo,
            pack.branch,
            &format!(".claude/commands/{command}"),
        );
        match download_text(&url) {
            Ok(body) => {
                fs::write(commands_dir.join(command), &body)?;
                installed_files.push(format!(".claude/commands/{command}"));
            }
            Err(e) => download_errors.push(format!("  command {command}: {e}")),
        }
    }

    // Write seite-native publish-draft override
    fs::write(
        commands_dir.join("publish-draft.md"),
        seite_publish_draft_command(),
    )?;
    installed_files.push(".claude/commands/publish-draft.md".to_string());

    // Download skills
    human::info(&format!("Downloading {} skills...", pack.skills.len()));
    for skill in pack.skills {
        let skill_dir = root.join(".claude").join("skills").join(skill);
        fs::create_dir_all(&skill_dir)?;
        let url = raw_github_url(
            pack.repo,
            pack.branch,
            &format!(".claude/skills/{skill}/SKILL.md"),
        );
        match download_text(&url) {
            Ok(body) => {
                fs::write(skill_dir.join("SKILL.md"), &body)?;
                installed_files.push(format!(".claude/skills/{skill}/SKILL.md"));
            }
            Err(e) => download_errors.push(format!("  skill {skill}: {e}")),
        }
    }

    // Download context file templates (only if they don't already exist)
    let context_dir = root.join("context");
    fs::create_dir_all(&context_dir)?;
    human::info("Scaffolding context templates...");
    for context_file in pack.context_files {
        let dest = context_dir.join(context_file);
        if dest.exists() {
            continue; // Don't overwrite user-customized context files
        }
        let url = raw_github_url(pack.repo, pack.branch, &format!("context/{context_file}"));
        match download_text(&url) {
            Ok(body) => {
                fs::write(&dest, &body)?;
                installed_files.push(format!("context/{context_file}"));
            }
            Err(e) => download_errors.push(format!("  context {context_file}: {e}")),
        }
    }

    // Download Python scripts
    let scripts_dir = root.join("scripts").join("seo");
    fs::create_dir_all(&scripts_dir)?;
    human::info(&format!(
        "Downloading {} Python scripts...",
        pack.python_scripts.len()
    ));
    for script in pack.python_scripts {
        let dest = scripts_dir.join(script);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let url = raw_github_url(pack.repo, pack.branch, script);
        match download_text(&url) {
            Ok(body) => {
                fs::write(&dest, &body)?;
                installed_files.push(format!("scripts/seo/{script}"));
            }
            Err(e) => download_errors.push(format!("  script {script}: {e}")),
        }
    }

    // Scaffold supporting files
    scaffold_env_example(root)?;
    scaffold_requirements(root, pack)?;

    // Create directories for SEOMachine workflow
    for dir in &["research", "output", "topics"] {
        fs::create_dir_all(root.join(dir))?;
    }

    // Add to .gitignore
    ensure_gitignore_entries(
        root,
        &[
            "research/",
            "output/",
            "topics/",
            ".env",
            "credentials/",
            ".claude/.seite-skill-packs.json",
        ],
    )?;

    // Generate internal links map
    let links_map = generate_internal_links_map(root);
    fs::write(context_dir.join("internal-links-map.md"), &links_map)?;

    // Append to CLAUDE.md
    append_claude_md_section(root)?;

    // Save manifest
    let mut manifest = load_manifest(root);
    manifest.packs.insert(
        pack.name.to_string(),
        PackEntry {
            source: format!("github:{}", pack.repo),
            branch: pack.branch.to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            files: installed_files.clone(),
        },
    );
    save_manifest(root, &manifest)?;

    // Report
    if !download_errors.is_empty() {
        human::info(&format!(
            "Some files failed to download ({} errors):",
            download_errors.len()
        ));
        for err in &download_errors {
            human::info(err);
        }
    }

    let total = installed_files.len();
    human::success(&format!(
        "Installed '{}' — {} files (agents, commands, skills, context, scripts)",
        pack.name, total
    ));
    human::info("");
    human::info("Next steps:");
    human::info("  1. Fill in context/ files with your brand voice, keywords, and guidelines");
    human::info("  2. (Optional) Set up analytics: copy .env.example to .env, fill in API keys");
    human::info("  3. (Optional) Install Python deps: pip install -r requirements-seo.txt");
    human::info("  4. Start with: /research \"your topic\" or /write \"your topic\"");

    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_install_url(root: &Path, url: &str, name_override: Option<&str>) -> anyhow::Result<()> {
    let skill_name = derive_skill_name(url, name_override);
    validate_skill_name(&skill_name)?;

    human::info(&format!("Downloading skill from {}...", url));

    let body = download_text(url)?;

    // Basic validation — should look like markdown
    if body.is_empty() {
        return Err(anyhow::anyhow!("downloaded file is empty"));
    }

    let skill_dir = root.join(".claude").join("skills").join(&skill_name);
    fs::create_dir_all(&skill_dir)?;
    fs::write(skill_dir.join("SKILL.md"), &body)?;

    // Save source URL for updates
    fs::write(skill_dir.join(".source"), url)?;

    human::success(&format!(
        "Installed skill '{}' to .claude/skills/{}/SKILL.md",
        skill_name, skill_name
    ));

    Ok(())
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

fn run_list() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let skills_dir = root.join(".claude").join("skills");

    // Bundled skills (always present after init)
    let bundled = ["theme-builder", "brand-identity", "landing-page"];

    human::info("Bundled skills:");
    for name in &bundled {
        let skill_path = skills_dir.join(name).join("SKILL.md");
        if skill_path.exists() {
            human::info(&format!("  {} (bundled)", name));
        }
    }

    // Installed packs
    let manifest = load_manifest(&root);
    if !manifest.packs.is_empty() {
        human::info("");
        human::info("Installed packs:");
        for (name, entry) in &manifest.packs {
            human::info(&format!(
                "  {} — {} files (from {})",
                name,
                entry.files.len(),
                entry.source
            ));
        }
    }

    // Other installed skills (not bundled, not part of a pack)
    let pack_skills: Vec<String> = manifest
        .packs
        .values()
        .flat_map(|e| {
            e.files.iter().filter_map(|f| {
                f.strip_prefix(".claude/skills/")
                    .and_then(|s| s.strip_suffix("/SKILL.md"))
                    .map(|s| s.to_string())
            })
        })
        .collect();

    if skills_dir.exists() {
        let mut custom_skills = Vec::new();
        if let Ok(entries) = fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if bundled.contains(&name.as_str()) || pack_skills.contains(&name) {
                    continue;
                }
                if entry.path().join("SKILL.md").exists() {
                    custom_skills.push(name);
                }
            }
        }
        if !custom_skills.is_empty() {
            human::info("");
            human::info("Custom skills:");
            for name in &custom_skills {
                human::info(&format!("  {}", name));
            }
        }
    }

    // Available packs (not yet installed)
    let available: Vec<_> = known_packs()
        .iter()
        .filter(|p| !manifest.packs.contains_key(p.name))
        .collect();
    if !available.is_empty() {
        human::info("");
        human::info("Available packs (not installed):");
        for pack in &available {
            human::info(&format!("  {} — {}", pack.name, pack.description));
            human::info(&format!(
                "    Install with: seite skill install {}",
                pack.name
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

fn run_remove(name: &str) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    remove_from(&root, name)
}

fn remove_from(root: &Path, name: &str) -> anyhow::Result<()> {
    let mut manifest = load_manifest(root);

    // Check if it's an installed pack
    if let Some(entry) = manifest.packs.remove(name) {
        human::info(&format!("Removing skill pack '{}'...", name));

        for file in &entry.files {
            let path = root.join(file);
            if path.exists() {
                fs::remove_file(&path)?;
            }
            // Clean up empty parent directories
            if let Some(parent) = path.parent() {
                let _ = fs::remove_dir(parent); // ignore errors (dir not empty)
            }
        }

        // Remove CLAUDE.md section
        let claude_md_path = root.join("CLAUDE.md");
        if claude_md_path.exists() {
            let content = fs::read_to_string(&claude_md_path)?;
            let cleaned = remove_claude_md_section(&content);
            fs::write(&claude_md_path, cleaned)?;
        }

        save_manifest(root, &manifest)?;
        human::success(&format!(
            "Removed pack '{}' ({} files). Context files in context/ were kept.",
            name,
            entry.files.len()
        ));
        return Ok(());
    }

    // Check if it's a custom skill
    let skill_dir = root.join(".claude").join("skills").join(name);
    if skill_dir.exists() {
        fs::remove_dir_all(&skill_dir)?;
        human::success(&format!("Removed skill '{}'", name));
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "no skill or pack named '{}' is installed",
        name
    ))
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_update(name: Option<&str>) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let manifest = load_manifest(&root);

    if let Some(name) = name {
        // Update specific pack
        if manifest.packs.contains_key(name) {
            if let Some(pack) = find_pack(name) {
                human::info(&format!("Updating pack '{}'...", name));
                return run_install_pack(&root, pack);
            }
            return Err(anyhow::anyhow!(
                "pack '{}' was installed but is no longer in the known packs registry",
                name
            ));
        }

        // Check if it's a URL-installed skill with .source file
        let source_path = root
            .join(".claude")
            .join("skills")
            .join(name)
            .join(".source");
        if source_path.exists() {
            let url = fs::read_to_string(&source_path)?.trim().to_string();
            human::info(&format!("Updating skill '{}' from {}...", name, url));
            return run_install_url(&root, &url, Some(name));
        }

        return Err(anyhow::anyhow!(
            "no updatable skill or pack named '{}' found",
            name
        ));
    }

    // Update all packs
    let pack_names: Vec<String> = manifest.packs.keys().cloned().collect();
    if pack_names.is_empty() {
        human::info("No skill packs installed. Nothing to update.");
        return Ok(());
    }

    for pack_name in &pack_names {
        if let Some(pack) = find_pack(pack_name) {
            human::info(&format!("Updating pack '{}'...", pack_name));
            run_install_pack(&root, pack)?;
        }
    }

    // Also regenerate internal links map
    let context_dir = root.join("context");
    if context_dir.exists() {
        let links_map = generate_internal_links_map(&root);
        fs::write(context_dir.join("internal-links-map.md"), &links_map)?;
        human::info("Regenerated internal links map");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_known_pack() {
        assert!(find_pack("seomachine").is_some());
        assert!(find_pack("nonexistent").is_none());
    }

    #[test]
    fn test_derive_skill_name_from_url() {
        assert_eq!(
            derive_skill_name(
                "https://raw.githubusercontent.com/user/repo/main/.claude/skills/content-strategy/SKILL.md",
                None
            ),
            "content-strategy"
        );
    }

    #[test]
    fn test_derive_skill_name_plain_md() {
        assert_eq!(
            derive_skill_name("https://example.com/my-skill.md", None),
            "my-skill"
        );
    }

    #[test]
    fn test_derive_skill_name_override() {
        assert_eq!(
            derive_skill_name("https://example.com/whatever.md", Some("custom-name")),
            "custom-name"
        );
    }

    #[test]
    fn test_validate_skill_name() {
        assert!(validate_skill_name("seo-content").is_ok());
        assert!(validate_skill_name("my_skill").is_ok());
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("../evil").is_err());
    }

    #[test]
    fn test_strip_date_prefix() {
        assert_eq!(strip_date_prefix("2026-03-12-hello-world"), "hello-world");
        assert_eq!(strip_date_prefix("no-date-here"), "no-date-here");
        assert_eq!(strip_date_prefix("short"), "short");
    }

    #[test]
    fn test_manifest_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "test".to_string(),
            PackEntry {
                source: "github:user/repo".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec!["file1.md".to_string()],
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();
        let loaded = load_manifest(tmp.path());
        assert!(loaded.packs.contains_key("test"));
        assert_eq!(loaded.packs["test"].files.len(), 1);
    }

    #[test]
    fn test_generate_internal_links_map_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let map = generate_internal_links_map(tmp.path());
        assert!(map.contains("# Internal Links Map"));
    }

    #[test]
    fn test_generate_internal_links_map_with_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let posts_dir = tmp.path().join("content").join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        fs::write(
            posts_dir.join("2026-03-12-hello-world.md"),
            "---\ntitle: \"Hello World\"\n---\nContent here",
        )
        .unwrap();
        let map = generate_internal_links_map(tmp.path());
        assert!(map.contains("Hello World"));
        assert!(map.contains("/posts/hello-world"));
    }

    #[test]
    fn test_seite_context_preamble_nonempty() {
        let preamble = seite_context_preamble();
        assert!(preamble.contains("seite"));
        assert!(preamble.contains("frontmatter"));
    }

    #[test]
    fn test_claude_md_section_add_and_remove() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# My Project\n").unwrap();
        append_claude_md_section(tmp.path()).unwrap();

        let content = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains("SEOMachine Integration"));
        assert!(content.contains(CLAUDE_MD_MARKER_START));
        assert!(content.contains(CLAUDE_MD_MARKER_END));
        assert!(content.contains("# My Project")); // Original content preserved

        // Remove
        let cleaned = remove_claude_md_section(&content);
        assert!(!cleaned.contains("SEOMachine Integration"));
        assert!(cleaned.contains("# My Project"));
    }

    #[test]
    fn test_ensure_gitignore_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        ensure_gitignore_entries(tmp.path(), &["research/", ".env"]).unwrap();
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("research/"));
        assert!(content.contains(".env"));

        // Calling again shouldn't duplicate
        ensure_gitignore_entries(tmp.path(), &["research/", ".env"]).unwrap();
        let content2 = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(
            content2.matches("research/").count(),
            1,
            "should not duplicate entries"
        );
    }

    #[test]
    fn test_scaffold_env_example() {
        let tmp = tempfile::TempDir::new().unwrap();
        scaffold_env_example(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join(".env.example")).unwrap();
        assert!(content.contains("DATAFORSEO_LOGIN"));

        // Should not overwrite
        fs::write(tmp.path().join(".env.example"), "custom").unwrap();
        scaffold_env_example(tmp.path()).unwrap();
        let content2 = fs::read_to_string(tmp.path().join(".env.example")).unwrap();
        assert_eq!(content2, "custom");
    }

    #[test]
    fn test_scaffold_requirements() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pack = find_pack("seomachine").unwrap();
        scaffold_requirements(tmp.path(), pack).unwrap();
        let content = fs::read_to_string(tmp.path().join("requirements-seo.txt")).unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_extract_title_from_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.md");
        fs::write(
            &path,
            "---\ntitle: \"Hello World\"\ndate: 2026-01-01\n---\nBody",
        )
        .unwrap();
        assert_eq!(
            extract_title_from_file(&path),
            Some("Hello World".to_string())
        );
    }

    #[test]
    fn test_extract_title_no_frontmatter() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.md");
        fs::write(&path, "# Just a heading\n\nBody").unwrap();
        assert_eq!(extract_title_from_file(&path), None);
    }

    #[test]
    fn test_run_list_no_skills_dir() {
        // Just verify it doesn't panic when no .claude/skills exists
        // We can't easily test the output without capturing stdout
        // but the function should not error
        let _result = run_list();
        // May fail if cwd doesn't exist, that's fine for this test
    }

    #[test]
    fn test_remove_nonexistent() {
        let result = run_remove("definitely-not-installed");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no skill or pack named"));
    }

    #[test]
    fn test_raw_github_url() {
        let url = raw_github_url(
            "TheCraigHewitt/seomachine",
            "main",
            ".claude/agents/editor.md",
        );
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/TheCraigHewitt/seomachine/main/.claude/agents/editor.md"
        );
    }

    #[test]
    fn test_load_manifest_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let manifest = load_manifest(tmp.path());
        assert!(manifest.packs.is_empty());
    }

    #[test]
    fn test_load_manifest_corrupt_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(".claude");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join(".seite-skill-packs.json"), "not valid json {{{").unwrap();
        let manifest = load_manifest(tmp.path());
        assert!(manifest.packs.is_empty()); // Falls back to default
    }

    #[test]
    fn test_remove_claude_md_section_no_markers() {
        let content = "# My Project\n\nSome content here.\n";
        let result = remove_claude_md_section(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_append_claude_md_no_existing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No CLAUDE.md exists
        append_claude_md_section(tmp.path()).unwrap();
        let content = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains("SEOMachine Integration"));
        assert!(content.contains(CLAUDE_MD_MARKER_START));
    }

    #[test]
    fn test_append_claude_md_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# My Project\n").unwrap();

        // Call twice
        append_claude_md_section(tmp.path()).unwrap();
        append_claude_md_section(tmp.path()).unwrap();

        let content = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        // Should have exactly one section
        assert_eq!(
            content.matches(CLAUDE_MD_MARKER_START).count(),
            1,
            "should not duplicate section"
        );
        assert!(content.contains("# My Project"));
    }

    #[test]
    fn test_generate_internal_links_map_multiple_collections() {
        let tmp = tempfile::TempDir::new().unwrap();
        let posts_dir = tmp.path().join("content").join("posts");
        let docs_dir = tmp.path().join("content").join("docs");
        fs::create_dir_all(&posts_dir).unwrap();
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(
            posts_dir.join("2026-01-01-first.md"),
            "---\ntitle: \"First Post\"\n---\n",
        )
        .unwrap();
        fs::write(
            docs_dir.join("getting-started.md"),
            "---\ntitle: \"Getting Started\"\n---\n",
        )
        .unwrap();
        let map = generate_internal_links_map(tmp.path());
        assert!(map.contains("First Post"));
        assert!(map.contains("Getting Started"));
        assert!(map.contains("## posts"));
        assert!(map.contains("## docs"));
    }

    #[test]
    fn test_generate_internal_links_map_nested_docs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let nested_dir = tmp.path().join("content").join("docs").join("guides");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(
            nested_dir.join("setup.md"),
            "---\ntitle: \"Setup Guide\"\n---\n",
        )
        .unwrap();
        let map = generate_internal_links_map(tmp.path());
        assert!(map.contains("Setup Guide"));
        assert!(map.contains("/docs/guides/setup"));
    }

    #[test]
    fn test_generate_internal_links_map_skips_index() {
        let tmp = tempfile::TempDir::new().unwrap();
        let posts_dir = tmp.path().join("content").join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        fs::write(
            posts_dir.join("index.md"),
            "---\ntitle: \"Posts Index\"\n---\n",
        )
        .unwrap();
        fs::write(
            posts_dir.join("real-post.md"),
            "---\ntitle: \"Real Post\"\n---\n",
        )
        .unwrap();
        let map = generate_internal_links_map(tmp.path());
        assert!(!map.contains("Posts Index"));
        assert!(map.contains("Real Post"));
    }

    #[test]
    fn test_derive_skill_name_trailing_slash() {
        assert_eq!(
            derive_skill_name("https://example.com/my-skill.md/", None),
            "my-skill"
        );
    }

    #[test]
    fn test_derive_skill_name_no_extension() {
        assert_eq!(
            derive_skill_name("https://example.com/raw-name", None),
            "raw-name"
        );
    }

    #[test]
    fn test_validate_skill_name_with_separator() {
        // Platform-dependent path separator
        assert!(validate_skill_name("a/b").is_err());
    }

    #[test]
    fn test_strip_date_prefix_edge_cases() {
        // Exactly 11 chars (YYYY-MM-DD-) — len is not > 11, returns as-is
        assert_eq!(strip_date_prefix("2026-03-12-"), "2026-03-12-");
        // Non-date that looks similar — fails digit check
        assert_eq!(strip_date_prefix("abcd-ef-gh-rest"), "abcd-ef-gh-rest");
        // Date with 10 chars exactly (no trailing -)
        assert_eq!(strip_date_prefix("2026-03-12"), "2026-03-12");
        // Date prefix with one char after
        assert_eq!(strip_date_prefix("2026-03-12-x"), "x");
    }

    #[test]
    fn test_seite_publish_draft_command_content() {
        let content = seite_publish_draft_command();
        assert!(content.contains("Publish Draft"));
        assert!(content.contains("seite build"));
        assert!(content.contains("seite deploy"));
        assert!(content.contains("draft: true"));
        // Should explicitly say NOT to use WordPress
        assert!(content.contains("Do NOT use WordPress"));
    }

    #[test]
    fn test_remove_custom_skill() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".claude").join("skills").join("my-custom");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# My Custom Skill\n").unwrap();

        let result = remove_from(tmp.path(), "my-custom");

        assert!(result.is_ok());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_remove_pack_cleans_files_and_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Create a fake installed pack with a few files
        let agent_dir = tmp.path().join(".claude").join("agents");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(agent_dir.join("test-agent.md"), "# Agent").unwrap();

        let cmd_dir = tmp.path().join(".claude").join("commands");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("test-cmd.md"), "# Command").unwrap();

        // Write manifest
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "test-pack".to_string(),
            PackEntry {
                source: "github:user/repo".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![
                    ".claude/agents/test-agent.md".to_string(),
                    ".claude/commands/test-cmd.md".to_string(),
                ],
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let result = remove_from(tmp.path(), "test-pack");

        assert!(result.is_ok());
        assert!(!agent_dir.join("test-agent.md").exists());
        assert!(!cmd_dir.join("test-cmd.md").exists());

        // Manifest should no longer have the pack
        let updated = load_manifest(tmp.path());
        assert!(!updated.packs.contains_key("test-pack"));
    }

    #[test]
    fn test_ensure_gitignore_appends_to_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitignore"), "dist/\nnode_modules/\n").unwrap();
        ensure_gitignore_entries(tmp.path(), &["research/"]).unwrap();
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("dist/"));
        assert!(content.contains("research/"));
    }

    #[test]
    fn test_ensure_gitignore_skips_existing_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitignore"), "research/\n").unwrap();
        ensure_gitignore_entries(tmp.path(), &["research/", ".env"]).unwrap();
        let content = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(content.matches("research/").count(), 1);
        assert!(content.contains(".env"));
    }

    #[test]
    fn test_extract_title_single_quotes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.md");
        fs::write(&path, "---\ntitle: 'Single Quoted'\n---\nBody").unwrap();
        assert_eq!(
            extract_title_from_file(&path),
            Some("Single Quoted".to_string())
        );
    }

    #[test]
    fn test_extract_title_unquoted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.md");
        fs::write(&path, "---\ntitle: Bare Title\n---\nBody").unwrap();
        assert_eq!(
            extract_title_from_file(&path),
            Some("Bare Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_empty_title() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.md");
        fs::write(&path, "---\ntitle: \"\"\n---\nBody").unwrap();
        assert_eq!(extract_title_from_file(&path), None);
    }

    #[test]
    fn test_known_packs_seomachine_completeness() {
        let pack = find_pack("seomachine").unwrap();
        assert!(!pack.agents.is_empty());
        assert!(!pack.commands.is_empty());
        assert!(!pack.skills.is_empty());
        assert!(!pack.context_files.is_empty());
        assert!(!pack.python_scripts.is_empty());
        assert_eq!(pack.agents.len(), 11);
        assert_eq!(pack.commands.len(), 22);
        assert_eq!(pack.skills.len(), 25);
        assert_eq!(pack.context_files.len(), 9);
        assert_eq!(pack.python_scripts.len(), 37);
    }

    #[test]
    fn test_run_install_unknown_source() {
        let result = run_install("completely-unknown-pack", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown skill pack"));
        assert!(err.contains("seomachine")); // Should suggest known packs
    }

    #[test]
    fn test_gather_skill_summary_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let summary = gather_skill_summary(tmp.path());
        assert!(summary.packs.is_empty());
        assert!(summary.context_files.is_empty());
        assert!(summary.custom_skills.is_empty());
    }

    #[test]
    fn test_gather_skill_summary_with_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "seomachine".to_string(),
            PackEntry {
                source: "github:TheCraigHewitt/seomachine".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![
                    ".claude/agents/editor.md".to_string(),
                    ".claude/agents/seo-optimizer.md".to_string(),
                    ".claude/commands/research.md".to_string(),
                    ".claude/commands/write.md".to_string(),
                    ".claude/commands/publish-draft.md".to_string(),
                    ".claude/skills/content-strategy/SKILL.md".to_string(),
                ],
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.packs.len(), 1);
        assert_eq!(summary.packs[0].name, "seomachine");
        assert_eq!(summary.packs[0].agents.len(), 2);
        assert_eq!(summary.packs[0].commands.len(), 3);
        assert_eq!(summary.packs[0].skills.len(), 1);
        assert!(summary.packs[0].description.contains("SEO"));
    }

    #[test]
    fn test_gather_skill_summary_context_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let context_dir = tmp.path().join("context");
        fs::create_dir_all(&context_dir).unwrap();
        fs::write(context_dir.join("brand-voice.md"), "# Brand Voice").unwrap();
        fs::write(context_dir.join("target-keywords.md"), "# Keywords").unwrap();
        fs::write(context_dir.join("notes.txt"), "not markdown").unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(
            summary.context_files,
            vec!["brand-voice", "target-keywords"]
        );
    }

    #[test]
    fn test_gather_skill_summary_custom_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".claude").join("skills");

        // Create a custom skill
        let custom = skills_dir.join("my-custom-skill");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("SKILL.md"), "# My Skill").unwrap();

        // Create a bundled skill (should be excluded)
        let bundled = skills_dir.join("theme-builder");
        fs::create_dir_all(&bundled).unwrap();
        fs::write(bundled.join("SKILL.md"), "# Theme Builder").unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.custom_skills, vec!["my-custom-skill"]);
    }

    #[test]
    fn test_gather_skill_summary_unknown_pack() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut manifest = SkillPacksManifest::default();
        manifest.packs.insert(
            "future-pack".to_string(),
            PackEntry {
                source: "github:someone/future-pack".to_string(),
                branch: "main".to_string(),
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                files: vec![".claude/commands/cool.md".to_string()],
            },
        );
        save_manifest(tmp.path(), &manifest).unwrap();

        let summary = gather_skill_summary(tmp.path());
        assert_eq!(summary.packs.len(), 1);
        assert!(summary.packs[0]
            .description
            .contains("github:someone/future-pack"));
    }
}
