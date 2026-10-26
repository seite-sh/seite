use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::cli::harness::{self, Agent, SiteFeatures};
use crate::cli::prompt;
use crate::config::{CollectionConfig, DeployTarget, SiteConfig};
use crate::content;
use crate::meta;
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

    /// Deploy target (github-pages, cloudflare, netlify); required when not interactive
    #[arg(long)]
    pub deploy_target: Option<String>,

    /// Collections to include (comma-separated: posts,docs,pages,trust)
    #[arg(long)]
    pub collections: Option<String>,

    /// Company name for trust center (defaults to --title)
    #[arg(long)]
    pub trust_company: Option<String>,

    /// Compliance frameworks (comma-separated: soc2,iso27001,gdpr,hipaa,pci-dss,ccpa,soc3)
    #[arg(long)]
    pub trust_frameworks: Option<String>,

    /// Trust center sections (comma-separated: overview,certifications,subprocessors,faq,disclosure,dpa,changelog)
    #[arg(long)]
    pub trust_sections: Option<String>,

    /// Contact form provider (formspree, web3forms, netlify, hubspot, typeform)
    #[arg(long)]
    pub contact_provider: Option<String>,

    /// Contact form endpoint/ID
    #[arg(long)]
    pub contact_endpoint: Option<String>,

    /// Coding agents to set the site up for (comma-separated: claude,codex,opencode,cursor,gemini,
    /// or all). Default: all
    #[arg(long, value_name = "LIST")]
    pub agents: Option<String>,

    /// Starter theme: default, docs, landing, or blank (for designing your
    /// own). Default: docs for a docs-only site, else default. The gallery
    /// themes are examples: `seite theme apply <name>` later
    #[arg(long, value_name = "STARTER")]
    pub theme: Option<String>,

    /// Also wire the seite MCP server into each agent's config (coding agents
    /// use the `seite` CLI; MCP is meant for chat apps without a shell)
    #[arg(long)]
    pub mcp: bool,
}

pub fn run(args: &InitArgs) -> anyhow::Result<()> {
    let name = match &args.name {
        Some(n) => n.clone(),
        None => prompt::input("Site name (directory)", None, "<NAME> argument")?,
    };

    let title = match &args.title {
        Some(t) => t.clone(),
        None => prompt::input("Site title", Some(&name), "--title")?,
    };

    let description = match &args.description {
        Some(d) => d.clone(),
        None => prompt::input("Site description", Some(""), "--description")?,
    };

    const DEPLOY_TARGETS: [&str; 3] = ["github-pages", "cloudflare", "netlify"];
    let deploy_target = match &args.deploy_target {
        Some(t) => {
            if !DEPLOY_TARGETS.contains(&t.as_str()) {
                anyhow::bail!(
                    "unknown deploy target '{t}'. Valid targets: {}",
                    DEPLOY_TARGETS.join(", ")
                );
            }
            t.clone()
        }
        None => {
            // No silent default: the target decides which CI workflow is generated.
            let selection = prompt::select(
                "Deploy target",
                &DEPLOY_TARGETS,
                None,
                "--deploy-target",
                &DEPLOY_TARGETS,
            )?;
            DEPLOY_TARGETS[selection].to_string()
        }
    };

    // Resolve collections
    let collections: Vec<CollectionConfig> = match &args.collections {
        Some(list) => list
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| {
                CollectionConfig::from_preset(name).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown collection preset '{name}'. Available: posts, docs, pages, changelog, roadmap, trust"
                    )
                })
            })
            .collect::<anyhow::Result<_>>()?,
        None => {
            let preset_names = ["posts", "docs", "pages", "changelog", "roadmap", "trust"];
            let defaults = &[true, false, true, false, false, false]; // posts + pages on by default
            let selections =
                prompt::multi_select("Collections to include", &preset_names, defaults)?;
            selections
                .into_iter()
                .filter_map(|i| CollectionConfig::from_preset(preset_names[i]))
                .collect()
        }
    };

    if collections.is_empty() {
        anyhow::bail!("at least one collection is required");
    }

    let starter = resolve_starter(args, &collections)?;

    // Coding agents to generate config for (AGENTS.md is always written).
    let agents = harness::resolve_init_agents(args.agents.as_deref())?;

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
    fs::create_dir_all(root.join("public"))?;
    fs::create_dir_all(root.join("data"))?;
    fs::create_dir_all(root.join(".seite"))?;

    // Write .gitignore
    fs::write(root.join(".gitignore"), harness::GITIGNORE)?;

    // Generate seite.toml
    let target = match deploy_target.as_str() {
        "cloudflare" => DeployTarget::Cloudflare,
        "netlify" => DeployTarget::Netlify,
        _ => DeployTarget::GithubPages,
    };
    let mut config = crate::config::SiteConfig {
        site: crate::config::SiteSection {
            title: title.clone(),
            description: description.clone(),
            base_url: "http://localhost:3000".into(),
            language: "en".into(),
            author: String::new(),
            timezone: None,
        },
        collections: collections.clone(),
        build: crate::config::BuildSection {
            minify: true,
            schedule: true,
            ..Default::default()
        },
        deploy: crate::config::DeploySection {
            target: target.clone(),
            repo: None,
            project: None,
            domain: None,
            auto_commit: true,
        },
        languages: Default::default(),
        images: Some(crate::config::ImageSection::default()),
        analytics: None,
        trust: None,
        contact: None,
        access: None,
        taxonomies: Vec::new(),
        check: None,
    };

    // If trust collection is included, run trust center scaffolding
    let has_trust = collections.iter().any(|c| c.name == "trust");
    let trust_opts = if has_trust {
        let opts = prompt_trust_options(args, &title)?;
        config.trust = Some(crate::config::TrustSection {
            company: Some(opts.company.clone()),
            frameworks: opts.frameworks.clone(),
        });
        Some(opts)
    } else {
        None
    };

    // Optional contact form setup
    if args.contact_provider.is_some() {
        let setup_args = crate::cli::contact::SetupArgs {
            provider: args.contact_provider.clone(),
            endpoint: args.contact_endpoint.clone(),
            region: None,
            redirect: None,
            subject: None,
        };
        config.contact = Some(crate::cli::contact::prompt_contact_config(
            &setup_args,
            &config,
        )?);
    } else if args.name.is_none() {
        // Interactive mode (no --name given = user is in interactive init flow)
        let add_contact =
            prompt::is_interactive() && prompt::confirm("Add a contact form?", false)?;
        if add_contact {
            let setup_args = crate::cli::contact::SetupArgs {
                provider: None,
                endpoint: None,
                region: None,
                redirect: None,
                subject: None,
            };
            config.contact = Some(crate::cli::contact::prompt_contact_config(
                &setup_args,
                &config,
            )?);
        }
    }

    let toml_str = toml::to_string_pretty(&config)?.replacen(
        "\nschedule = true\n",
        "\n# Hide posts dated in the future until their date (and `expires:` posts after it).\n# The generated deploy workflow rebuilds daily so they go live on their own.\nschedule = true\n",
        1,
    );
    fs::write(root.join("seite.toml"), toml_str)?;

    // Write default templates
    fs::write(root.join("templates/base.html"), starter.base_html)?;
    if let Some((rel, css)) = starter.stylesheet {
        crate::safe_fs::write(&root, &root.join("static").join(rel), css)?;
    }
    fs::write(root.join("templates/index.html"), templates::DEFAULT_INDEX)?;
    for c in &collections {
        let tmpl_name = &c.default_template;
        let content = match tmpl_name.as_str() {
            "post.html" => templates::DEFAULT_POST,
            "doc.html" => templates::DEFAULT_DOC,
            "page.html" => templates::DEFAULT_PAGE,
            "changelog-entry.html" => templates::DEFAULT_CHANGELOG_ENTRY,
            "roadmap-item.html" => templates::DEFAULT_ROADMAP_ITEM,
            "trust-item.html" => templates::DEFAULT_TRUST_ITEM,
            _ => continue,
        };
        fs::write(root.join("templates").join(tmpl_name), content)?;
    }
    // Write trust-index.html if trust collection is present
    if has_trust {
        fs::write(
            root.join("templates/trust-index.html"),
            templates::DEFAULT_TRUST_INDEX,
        )?;
    }

    // Create sample hello-world post if posts collection is included
    if collections.iter().any(|c| c.name == "posts") {
        let today_date = content::schedule::today(&config);
        let today = today_date.format("%Y-%m-%d").to_string();
        let fm = content::Frontmatter {
            title: "Hello World".into(),
            date: Some(today_date),
            description: Some("Welcome to your new site!".into()),
            tags: vec!["intro".into()],
            draft: false,
            ..Default::default()
        };
        let frontmatter_str = content::generate_frontmatter(&fm);
        let post_content = format!(
            "{frontmatter_str}\n\nWelcome to your new site built with **seite**.\n\nEdit this post or create new ones with `seite new post \"My Post\"`.\n"
        );
        fs::write(
            root.join(format!("content/posts/{today}-hello-world.md")),
            post_content,
        )?;
    }

    // Create sample changelog entry if changelog collection is included
    if collections.iter().any(|c| c.name == "changelog") {
        let today_date = content::schedule::today(&config);
        let today = today_date.format("%Y-%m-%d").to_string();
        let fm = content::Frontmatter {
            title: "v0.1.0".into(),
            date: Some(today_date),
            description: Some("Initial release".into()),
            tags: vec!["new".into()],
            draft: false,
            ..Default::default()
        };
        let frontmatter_str = content::generate_frontmatter(&fm);
        let changelog_content = format!(
            "{frontmatter_str}\n\nFirst release of the project.\n\n## What's New\n\n- Initial feature set\n- Project scaffolding\n"
        );
        fs::write(
            root.join(format!("content/changelog/{today}-v0-1-0.md")),
            changelog_content,
        )?;
    }

    // Create sample roadmap items if roadmap collection is included
    if collections.iter().any(|c| c.name == "roadmap") {
        let items = [
            (
                "Dark Mode",
                "planned",
                1,
                "Add dark mode support to the application.",
            ),
            (
                "API v2",
                "in-progress",
                2,
                "Redesign the API with improved authentication and rate limiting.",
            ),
            (
                "Initial Release",
                "done",
                3,
                "Ship the first public version.",
            ),
        ];
        for (title, status, weight, description) in &items {
            let slug = content::slug_from_title(title);
            let fm = content::Frontmatter {
                title: title.to_string(),
                description: Some(description.to_string()),
                tags: vec![status.to_string()],
                weight: Some(*weight),
                draft: false,
                ..Default::default()
            };
            let frontmatter_str = content::generate_frontmatter(&fm);
            let roadmap_content = format!("{frontmatter_str}\n\n{description}\n");
            fs::write(
                root.join(format!("content/roadmap/{slug}.md")),
                roadmap_content,
            )?;
        }
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

    // Scaffold trust center data files and content if trust collection is present
    if let Some(ref opts) = trust_opts {
        scaffold_trust_center(&root, opts)?;
    }

    // Create contact page if contact form is configured and pages collection exists
    if config.contact.is_some() {
        let has_pages = collections.iter().any(|c| c.name == "pages");
        let contact_page = root.join("content/pages/contact.md");
        if has_pages && !contact_page.exists() {
            fs::write(
                &contact_page,
                "---\ntitle: Contact\ndescription: Get in touch\n---\n\n{{< contact_form() >}}\n",
            )?;
        }
    }

    // Write project metadata (.seite/config.json), recording the agent
    // selection so `seite upgrade` knows which harness files to maintain.
    meta::write(
        &root,
        &meta::PageMeta {
            agents: Some(harness::to_ids(&agents)),
            hooks_installed: Some(crate::cli::harness_hooks::all_tier_ids(&agents)),
            ..meta::PageMeta::current()
        },
    )?;

    // Per-agent harness files: permissions, hooks, path-scoped rules, the
    // `seite` skill (plus the CLAUDE.md/GEMINI.md `@AGENTS.md` shims), and
    // with --mcp the MCP server config.
    let features = SiteFeatures::from_config(&config);
    harness::write_plan(&root, &agents, features, args.mcp)?;

    // Version-matched docs the agent reads locally (.seite/docs/).
    harness::write_local_docs(&root)?;

    // Cross-agent project instructions, read by every supported agent.
    fs::write(
        root.join("AGENTS.md"),
        generate_agents_md(
            &config,
            &title,
            &description,
            &collections,
            trust_opts.as_ref(),
            &agents,
            args.mcp,
        ),
    )?;

    human::success(&format!("Created new site in '{name}'"));
    crate::human_println!();

    // Show project structure so users know what was created
    for line in project_tree_lines(&name, &collections) {
        crate::human_println!("{line}");
    }

    crate::human_println!();
    human::info(&if starter.name == crate::themes::BLANK {
        "Theme: blank, ready for your own design: `/seite theme <description>` in your agent (or `/impeccable` with `seite skill install impeccable`)".to_string()
    } else {
        format!(
            "Theme: {} (a starting point; for your own design run `seite theme new`, examples: `seite theme list`)",
            starter.name
        )
    });
    crate::human_println!();
    human::info("Next steps:");
    crate::human_println!("  cd {name}");
    crate::human_println!(
        "  seite serve             {} start dev server with live reload",
        console::style("←").dim()
    );
    crate::human_println!(
        "  seite agent             {} open your coding agent in the site (then /seite)",
        console::style("←").dim()
    );
    crate::human_println!();
    human::info(&format!(
        "Set up for {} — see AGENTS.md → Agent setup for each agent's one-time step.",
        agents
            .iter()
            .map(|a| a.label())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    let root_display = std::env::current_dir()
        .map(|cwd| cwd.join(&root))
        .unwrap_or_else(|_| root.clone());
    crate::output::json::set_data(serde_json::json!({
        "path": root_display.display().to_string(),
        "name": name,
        "title": title,
        "collections": collections.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        "deploy_target": deploy_target,
        "theme": starter.name,
        "contact": config.contact.is_some(),
        "agents": harness::to_ids(&agents),
        "mcp": args.mcp,
    }));

    Ok(())
}

/// The starter theme for a new site: `--theme`, else a pick in the
/// interactive flow, else docs for a docs-only site and default otherwise.
fn resolve_starter(
    args: &InitArgs,
    collections: &[CollectionConfig],
) -> anyhow::Result<crate::themes::Theme> {
    let starters = crate::themes::of_kind(crate::themes::ThemeKind::Starter);
    let names: Vec<&str> = starters.iter().map(|t| t.name).collect();
    let fallback = starter_for(collections);
    let name = match &args.theme {
        Some(name) => {
            if !names.contains(&name.as_str()) {
                let gallery = if crate::themes::by_name(name).is_some() {
                    format!(" '{name}' is a gallery example: apply it after init with `seite theme apply {name}`.")
                } else {
                    String::new()
                };
                anyhow::bail!(
                    "unknown starter '{name}'. Starters: {}.{gallery}",
                    names.join(", ")
                );
            }
            name.clone()
        }
        None if args.name.is_none() => {
            let labels: Vec<String> = starters
                .iter()
                .map(|t| format!("{} — {}", t.name, t.description))
                .collect();
            let items: Vec<&str> = labels.iter().map(String::as_str).collect();
            let default = names.iter().position(|n| *n == fallback);
            names[prompt::select("Starter theme", &items, default, "--theme", &names)?].to_string()
        }
        None => fallback.to_string(),
    };
    Ok(crate::themes::by_name(&name).expect("starter names are bundled themes"))
}

/// The starter that fits the collections: `docs` for a docs-only site,
/// `default` otherwise. `landing` and `blank` are only chosen explicitly.
pub fn starter_for(collections: &[CollectionConfig]) -> &'static str {
    let docs_only = !collections.is_empty() && collections.iter().all(|c| c.name == "docs");
    if docs_only {
        "docs"
    } else {
        "default"
    }
}

/// Lines of the "what was created" tree shown after `seite init`.
fn project_tree_lines(name: &str, collections: &[CollectionConfig]) -> Vec<String> {
    let mut lines = vec![
        format!("  {name}/"),
        format!(
            "  ├── seite.toml          {}",
            console::style("← site config").dim()
        ),
        "  ├── content/".to_string(),
    ];
    for (i, c) in collections.iter().enumerate() {
        let branch = if i + 1 == collections.len() {
            "└──"
        } else {
            "├──"
        };
        lines.push(format!("  │   {branch} {}/", c.directory));
    }
    lines.push(format!(
        "  ├── templates/base.html {}",
        console::style("← theme template").dim()
    ));
    lines.push(format!(
        "  └── static/             {}",
        console::style("← CSS, images, etc.").dim()
    ));
    lines
}

/// Trust center framework metadata.
struct FrameworkInfo {
    slug: &'static str,
    name: &'static str,
    description: &'static str,
}

const FRAMEWORKS: &[FrameworkInfo] = &[
    FrameworkInfo {
        slug: "soc2",
        name: "SOC 2 Type II",
        description: "Annual audit covering Security and Availability trust service criteria",
    },
    FrameworkInfo {
        slug: "iso27001",
        name: "ISO 27001",
        description: "International standard for information security management systems (ISMS)",
    },
    FrameworkInfo {
        slug: "gdpr",
        name: "GDPR",
        description: "EU General Data Protection Regulation for personal data processing",
    },
    FrameworkInfo {
        slug: "hipaa",
        name: "HIPAA",
        description: "US health data privacy and security regulation",
    },
    FrameworkInfo {
        slug: "pci-dss",
        name: "PCI DSS",
        description: "Payment Card Industry Data Security Standard",
    },
    FrameworkInfo {
        slug: "ccpa",
        name: "CCPA / CPRA",
        description: "California Consumer Privacy Act and California Privacy Rights Act",
    },
    FrameworkInfo {
        slug: "soc3",
        name: "SOC 3",
        description: "Public-facing summary of SOC 2 report",
    },
];

fn framework_by_slug(slug: &str) -> Option<&'static FrameworkInfo> {
    FRAMEWORKS.iter().find(|f| f.slug == slug)
}

/// Options collected during trust center scaffolding.
pub struct TrustOptions {
    pub company: String,
    pub frameworks: Vec<String>,
    pub framework_statuses: Vec<(String, String)>, // (slug, status)
    pub sections: Vec<String>,
}

/// Prompt for trust center options interactively or from CLI args.
fn prompt_trust_options(args: &InitArgs, title: &str) -> anyhow::Result<TrustOptions> {
    let company = match &args.trust_company {
        Some(c) => c.clone(),
        None => prompt::input("Trust center company name", Some(title), "--trust-company")?,
    };

    let frameworks: Vec<String> = match &args.trust_frameworks {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).collect(),
        None => {
            let names: Vec<&str> = FRAMEWORKS.iter().map(|f| f.name).collect();
            let defaults = &[true, false, false, false, false, false, false];
            let selections = prompt::multi_select("Compliance frameworks", &names, defaults)?;
            selections
                .into_iter()
                .map(|i| FRAMEWORKS[i].slug.to_string())
                .collect()
        }
    };

    let sections: Vec<String> = match &args.trust_sections {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).collect(),
        None => {
            let section_names = [
                "Security Overview",
                "Certifications",
                "Subprocessor List",
                "FAQ / Security Questionnaire",
                "Vulnerability Disclosure",
                "Data Processing Agreement",
                "Security Changelog",
            ];
            let section_slugs = [
                "overview",
                "certifications",
                "subprocessors",
                "faq",
                "disclosure",
                "dpa",
                "changelog",
            ];
            let defaults = &[true, true, true, true, true, false, false];
            let selections =
                prompt::multi_select("Trust center sections", &section_names, defaults)?;
            selections
                .into_iter()
                .map(|i| section_slugs[i].to_string())
                .collect()
        }
    };

    // Ask per-framework status (only in interactive mode)
    let mut framework_statuses = Vec::new();
    for fw_slug in &frameworks {
        let fw = framework_by_slug(fw_slug);
        let fw_name = fw.map(|f| f.name).unwrap_or(fw_slug.as_str());
        let status = if args.trust_frameworks.is_some() || !prompt::is_interactive() {
            // Non-interactive: default to "in_progress" (never claim certification)
            "in_progress".to_string()
        } else {
            let options = [
                "Active (certified)",
                "In Progress (pursuing)",
                "Planned (on roadmap)",
            ];
            let selection = prompt::select(
                &format!("{fw_name} status"),
                &options,
                Some(0),
                "--trust-frameworks",
                &[],
            )?;
            match selection {
                0 => "active",
                1 => "in_progress",
                _ => "planned",
            }
            .to_string()
        };
        framework_statuses.push((fw_slug.clone(), status));
    }

    Ok(TrustOptions {
        company,
        frameworks,
        framework_statuses,
        sections,
    })
}

/// Scaffold the trust center data files and content.
fn scaffold_trust_center(root: &std::path::Path, opts: &TrustOptions) -> anyhow::Result<()> {
    // Create nested content directories
    fs::create_dir_all(root.join("content/trust/certifications"))?;
    fs::create_dir_all(root.join("data/trust"))?;

    // -- Data files --

    // certifications.yaml
    let mut certs_yaml = String::new();
    for (slug, status) in &opts.framework_statuses {
        let fw = framework_by_slug(slug);
        let name = fw.map(|f| f.name).unwrap_or(slug.as_str());
        let desc = fw.map(|f| f.description).unwrap_or("");
        certs_yaml.push_str(&format!(
            "- name: \"{name}\"\n  slug: \"{slug}\"\n  status: {status}\n  framework: \"{slug}\"\n  description: >\n    {desc}\n"
        ));
        certs_yaml.push_str("  # issued: 2025-01-01\n  # expires: 2026-01-01\n  # auditor: \"\"\n  # scope: \"\"\n  # report_url: \"\"\n\n");
    }
    fs::write(root.join("data/trust/certifications.yaml"), certs_yaml)?;

    // subprocessors.yaml
    if opts.sections.iter().any(|s| s == "subprocessors") {
        let subprocessors = format!(
            "# Subprocessors for {company}\n# Update when vendors change. Last reviewed: {date}\n\n\
            - name: \"Example Cloud Provider\"\n  purpose: \"Cloud infrastructure and hosting\"\n  data_types: [\"Customer data\", \"Application logs\"]\n  location: \"United States\"\n  dpa: true\n\n\
            - name: \"Example Payment Processor\"\n  purpose: \"Payment processing\"\n  data_types: [\"Billing information\"]\n  location: \"United States\"\n  dpa: true\n\n\
            # Add your actual subprocessors below:\n\
            # - name: \"Vendor Name\"\n\
            #   purpose: \"What they do with data\"\n\
            #   data_types: [\"Types of data shared\"]\n\
            #   location: \"Country\"\n\
            #   dpa: true\n",
            company = opts.company,
            date = chrono::Local::now().format("%Y-%m-%d"),
        );
        fs::write(root.join("data/trust/subprocessors.yaml"), subprocessors)?;
    }

    // faq.yaml
    if opts.sections.iter().any(|s| s == "faq") {
        let mut faq = String::from(
            "# Security FAQ — edit answers to match your actual security posture.\n\n",
        );
        faq.push_str("- question: \"Do you encrypt data at rest?\"\n  answer: \"Yes. All customer data is encrypted at rest using AES-256 encryption.\"\n  category: encryption\n\n");
        faq.push_str("- question: \"Do you encrypt data in transit?\"\n  answer: \"Yes. All data in transit is encrypted using TLS 1.2 or higher.\"\n  category: encryption\n\n");
        faq.push_str("- question: \"Do you support SSO / SAML?\"\n  answer: \"Yes. We support SAML 2.0 SSO integration with major identity providers.\"\n  category: access\n\n");
        faq.push_str("- question: \"Do you enforce multi-factor authentication?\"\n  answer: \"Yes. MFA is enforced for all employee access to production systems.\"\n  category: access\n\n");
        faq.push_str("- question: \"Where is customer data stored?\"\n  answer: \"All customer data is stored in [REGION]. Contact us for data residency documentation.\"\n  category: data-residency\n\n");
        faq.push_str("- question: \"Do you perform regular penetration testing?\"\n  answer: \"Yes. We conduct annual third-party penetration testing and address all findings.\"\n  category: compliance\n\n");
        faq.push_str("- question: \"Do you have a vulnerability disclosure program?\"\n  answer: \"Yes. See our vulnerability disclosure page for responsible reporting guidelines.\"\n  category: compliance\n\n");
        faq.push_str("- question: \"What is your incident response process?\"\n  answer: \"We maintain a documented incident response plan with defined notification timelines.\"\n  category: incident-response\n\n");

        if opts.frameworks.iter().any(|f| f == "gdpr" || f == "ccpa") {
            faq.push_str("- question: \"Can customers request data deletion?\"\n  answer: \"Yes. We honor all data deletion requests within 30 days per applicable regulations.\"\n  category: data-residency\n\n");
            faq.push_str("- question: \"Do you have a Data Processing Agreement (DPA)?\"\n  answer: \"Yes. We provide a DPA to all customers upon request.\"\n  category: compliance\n\n");
        }
        if opts.frameworks.iter().any(|f| f == "hipaa") {
            faq.push_str("- question: \"Do you sign Business Associate Agreements (BAA)?\"\n  answer: \"Yes. We execute BAAs with customers who require HIPAA compliance.\"\n  category: compliance\n\n");
        }
        if opts.frameworks.iter().any(|f| f == "soc2") {
            faq.push_str("- question: \"Can we review your SOC 2 report?\"\n  answer: \"Yes. SOC 2 Type II reports are available under NDA. Contact security@yourcompany.com.\"\n  category: compliance\n\n");
        }

        fs::write(root.join("data/trust/faq.yaml"), faq)?;
    }

    // -- Content files --

    // Security overview
    if opts.sections.iter().any(|s| s == "overview") {
        let overview = format!(
            "---\ntitle: \"Security Overview\"\ndescription: \"How {company} protects your data\"\nweight: 1\nextra:\n  type: overview\n---\n\n\
            ## Our Commitment to Security\n\n\
            At {company}, security is foundational to everything we build. We maintain industry-standard certifications and follow security best practices across our entire platform.\n\n\
            ## Infrastructure Security\n\n\
            Our platform runs on enterprise-grade cloud infrastructure with network-level isolation, DDoS protection, and automated threat detection.\n\n\
            ## Application Security\n\n\
            We follow a secure software development lifecycle (SDLC) with code review, static analysis, dependency scanning, and regular penetration testing.\n\n\
            ## Data Protection\n\n\
            All customer data is encrypted at rest (AES-256) and in transit (TLS 1.2+). Encryption keys are managed through a dedicated key management service.\n\n\
            ## Access Management\n\n\
            We enforce the principle of least privilege across all systems. Employee access requires SSO with MFA, and all access is logged and audited.\n",
            company = opts.company,
        );
        fs::write(root.join("content/trust/security-overview.md"), overview)?;
    }

    // Vulnerability disclosure
    if opts.sections.iter().any(|s| s == "disclosure") {
        let disclosure = format!(
            "---\ntitle: \"Vulnerability Disclosure\"\ndescription: \"How to responsibly report security vulnerabilities to {company}\"\nweight: 5\nextra:\n  type: policy\n---\n\n\
            ## Responsible Disclosure\n\n\
            {company} takes security seriously. If you believe you have found a security vulnerability, we encourage you to report it responsibly.\n\n\
            ## How to Report\n\n\
            Email your findings to **security@yourcompany.com**. Please include:\n\n\
            - Description of the vulnerability\n\
            - Steps to reproduce\n\
            - Potential impact\n\
            - Any suggested remediation\n\n\
            ## Our Commitment\n\n\
            - We will acknowledge receipt within 2 business days\n\
            - We will provide an initial assessment within 5 business days\n\
            - We will keep you informed of our progress\n\
            - We will not take legal action against good-faith researchers\n\n\
            ## Scope\n\n\
            All production services and applications are in scope. Please do not test against other customers' data or accounts.\n",
            company = opts.company,
        );
        fs::write(
            root.join("content/trust/vulnerability-disclosure.md"),
            disclosure,
        )?;
    }

    // Data Processing Agreement
    if opts.sections.iter().any(|s| s == "dpa") {
        let dpa = format!(
            "---\ntitle: \"Data Processing\"\ndescription: \"Data processing practices at {company}\"\nweight: 6\nextra:\n  type: policy\n---\n\n\
            ## Data Processing Overview\n\n\
            {company} processes customer data in accordance with applicable data protection regulations.\n\n\
            ## Data Processing Agreement\n\n\
            A Data Processing Agreement (DPA) is available for all customers. Contact us to request a copy.\n\n\
            ## Data Categories\n\n\
            We process the following categories of data on behalf of our customers:\n\n\
            - Account information (name, email, organization)\n\
            - Usage data (application logs, analytics)\n\
            - Customer-provided content\n\n\
            ## Data Retention\n\n\
            Customer data is retained for the duration of the service agreement. Upon termination, data is deleted within 30 days unless a longer retention period is required by law.\n",
            company = opts.company,
        );
        fs::write(root.join("content/trust/data-processing.md"), dpa)?;
    }

    // Per-framework certification pages
    if opts.sections.iter().any(|s| s == "certifications") {
        for (slug, status) in &opts.framework_statuses {
            let fw = framework_by_slug(slug);
            let name = fw.map(|f| f.name).unwrap_or(slug.as_str());
            let desc = fw.map(|f| f.description).unwrap_or("");
            let status_text = match status.as_str() {
                "active" => format!("{} is actively certified under {name}.", opts.company),
                "in_progress" => format!(
                    "{} is currently pursuing {name} certification.",
                    opts.company
                ),
                _ => format!("{} has {name} certification on its roadmap.", opts.company),
            };
            let cert_page = format!(
                "---\ntitle: \"{name}\"\ndescription: \"{desc}\"\nweight: 2\nextra:\n  type: certification\n  framework: \"{slug}\"\n---\n\n\
                ## {name}\n\n\
                {status_text}\n\n\
                {desc}.\n\n\
                ## Scope\n\n\
                <!-- Describe the scope of your certification here -->\n\n\
                ## Resources\n\n\
                <!-- Link to reports, certificates, or contact information -->\n\
                For access to our {name} report, contact security@yourcompany.com.\n",
            );
            fs::write(
                root.join(format!("content/trust/certifications/{slug}.md")),
                cert_page,
            )?;
        }
    }

    human::success(&format!(
        "Trust center scaffolded ({} frameworks, {} sections)",
        opts.frameworks.len(),
        opts.sections.len()
    ));

    Ok(())
}

/// Generate a compact AGENTS.md for the site: what it is, the layout, the
/// collections, the content format, the hard rules, and the workflow. Details
/// live in the `seite` skill, the context rules, and `.seite/docs/`, so
/// nothing here is repeated there.
fn generate_agents_md(
    config: &SiteConfig,
    title: &str,
    description: &str,
    collections: &[CollectionConfig],
    trust_opts: Option<&TrustOptions>,
    agents: &[Agent],
    mcp: bool,
) -> String {
    let mut md = String::with_capacity(4096);
    let features = SiteFeatures::from_config(config);

    md.push_str(&format!("# {title}\n\n"));
    if !description.is_empty() {
        md.push_str(&format!("{description}\n\n"));
    }
    md.push_str("A static site built with [seite](https://seite.sh): markdown in, HTML + markdown + `llms.txt` out.\n\n");

    md.push_str("## Workflow\n\n");
    md.push_str("1. Run `seite context` once per session: config, collections, theme, dev server, pending upgrades.\n");
    md.push_str("2. Work through the `seite` skill: `/seite <command>` (Codex: `$seite <command>`), e.g. `check`, `new`, `preview`, `theme`, `deploy`.\n");
    md.push_str(
        "3. Reference: `.seite/docs/` (docs for this seite version, written by seite and gitignored) or `seite docs <query>`.\n\n",
    );

    md.push_str("## Layout\n\n```\n");
    md.push_str("content/<collection>/  markdown + YAML frontmatter\n");
    md.push_str("templates/             Tera templates; base.html is the theme\n");
    md.push_str("data/                  YAML/JSON/TOML -> {{ data.<file> }}\n");
    md.push_str("static/                copied to dist/static/\n");
    md.push_str("public/                copied to the dist/ root (favicon.ico)\n");
    md.push_str("seite.toml             site config\n");
    md.push_str("dist/                  build output (generated)\n");
    md.push_str("```\n\n");

    md.push_str("## Collections\n\n");
    md.push_str("| Collection | Files | URL | Template |\n|---|---|---|---|\n");
    for c in collections {
        let file = if c.has_date {
            "YYYY-MM-DD-slug.md"
        } else {
            "slug.md"
        };
        let prefix = if c.url_prefix.is_empty() {
            String::new()
        } else {
            c.url_prefix.clone()
        };
        md.push_str(&format!(
            "| {} (`seite new {}`) | `content/{}/{file}` | `{prefix}/<slug>` | `{}` |\n",
            c.label,
            singularize(&c.name),
            c.directory,
            c.default_template
        ));
    }
    for c in collections {
        match c.name.as_str() {
            "changelog" => md.push_str(
                "\nChangelog tags: `new`, `fix`, `breaking`, `improvement`, `deprecated`.",
            ),
            "roadmap" => md.push_str(
                "\nRoadmap status tags: `planned`, `in-progress`, `done`, `cancelled`; `weight:` orders items.",
            ),
            _ => {}
        }
        if let Some(ref subdomain) = c.subdomain {
            md.push_str(&format!(
                "\n{} deploys to the `{subdomain}` subdomain (`{}`).",
                c.label,
                config.subdomain_base_url(c)
            ));
        }
    }
    if collections
        .iter()
        .any(|c| c.name == "changelog" || c.name == "roadmap" || c.subdomain.is_some())
    {
        md.push('\n');
    }
    if let Some(opts) = trust_opts {
        md.push_str(&format!(
            "\nTrust center for **{}** at `/trust/` (data in `data/trust/`).\n",
            opts.company
        ));
    }
    md.push('\n');

    md.push_str("## Content format\n\n```yaml\n---\n");
    md.push_str("title: \"Title\"           # required\n");
    if collections.iter().any(|c| c.has_date) {
        md.push_str("date: 2026-01-15         # required in dated collections\n");
    }
    md.push_str("description: \"...\"       # meta, Open Graph, JSON-LD\n");
    md.push_str("image: /static/og.png    # social preview\n");
    md.push_str("updated: 2026-02-01      # last modified\n");
    md.push_str("tags: [a, b]             # tag pages and feeds\n");
    md.push_str("draft: true              # left out of builds\n");
    md.push_str("weight: 1                # order in undated collections\n");
    md.push_str("extra: {key: value}      # -> {{ page.extra.key }}\n");
    md.push_str("---\n```\n\n");

    md.push_str("## Rules\n\n");
    md.push_str("- Link between content files with relative `.md` paths (`[intro](../docs/intro.md)`); the build turns them into URLs.\n");
    md.push_str("- Never edit `dist/`.\n");
    md.push_str(
        "- Templates extend `base.html`; render content with `{{ page.content | safe }}`.\n",
    );
    md.push_str("- Shortcodes: `{{< name(arg=\"v\") >}}`, or `{{% name() %}}…{{% end %}}` with a body. No `{{< /name >}}`.\n\n");

    md.push_str("## Verify\n\n");
    md.push_str("Run `seite check` after changes. Fix every error at its `file:line`, run `seite check` once more, then stop and report what is left.");
    if !agents.is_empty() {
        md.push(' ');
        md.push_str(&crate::cli::harness_hooks::verify_note(agents));
    }
    md.push_str("\n\n");

    md.push_str("## Agent setup\n\n");
    md.push_str(&harness::wrap_block(
        harness::MCP_SETUP_BLOCK,
        &harness::mcp_setup_table(agents, mcp),
    ));
    md.push('\n');

    md.push_str("## Context Rules\n\n");
    md.push_str(&harness::wrap_block(
        harness::RULES_INDEX_BLOCK,
        &harness::rules_index(features, agents),
    ));
    md
}

/// Convert a plural collection name to singular for display.
fn singularize(name: &str) -> &str {
    match name {
        "posts" => "post",
        "docs" => "doc",
        "pages" => "page",
        "trust" => "trust",
        _ => name,
    }
}
