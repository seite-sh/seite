use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::config::{CollectionConfig, DeployTarget};
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

    /// Deploy target (github-pages, cloudflare)
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
            let preset_names = ["posts", "docs", "pages", "changelog", "roadmap", "trust"];
            let defaults = &[true, false, true, false, false, false]; // posts + pages on by default
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
    fs::create_dir_all(root.join("data"))?;
    fs::create_dir_all(root.join(".claude"))?;
    fs::create_dir_all(root.join(".page"))?;

    // Generate page.toml
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
        },
        collections: collections.clone(),
        build: Default::default(),
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

    // Create sample changelog entry if changelog collection is included
    if collections.iter().any(|c| c.name == "changelog") {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let fm = content::Frontmatter {
            title: "v0.1.0".into(),
            date: Some(chrono::Local::now().date_naive()),
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
            ("Dark Mode", "planned", 1, "Add dark mode support to the application."),
            ("API v2", "in-progress", 2, "Redesign the API with improved authentication and rate limiting."),
            ("Initial Release", "done", 3, "Ship the first public version."),
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
            let roadmap_content = format!(
                "{frontmatter_str}\n\n{description}\n"
            );
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

    // Write project metadata (.page/config.json)
    meta::write(&root, &meta::PageMeta::current())?;

    // Write Claude Code settings (.claude/settings.json)
    fs::write(
        root.join(".claude/settings.json"),
        generate_claude_settings(),
    )?;

    // Write CLAUDE.md with site-specific context
    fs::write(
        root.join("CLAUDE.md"),
        generate_claude_md(&title, &description, &collections, trust_opts.as_ref()),
    )?;

    human::success(&format!("Created new site in '{name}'"));
    human::info("Next steps:");
    println!("  cd {name}");
    println!("  page build");
    println!("  page serve");

    Ok(())
}

/// Trust center framework metadata.
struct FrameworkInfo {
    slug: &'static str,
    name: &'static str,
    description: &'static str,
}

const FRAMEWORKS: &[FrameworkInfo] = &[
    FrameworkInfo { slug: "soc2", name: "SOC 2 Type II", description: "Annual audit covering Security and Availability trust service criteria" },
    FrameworkInfo { slug: "iso27001", name: "ISO 27001", description: "International standard for information security management systems (ISMS)" },
    FrameworkInfo { slug: "gdpr", name: "GDPR", description: "EU General Data Protection Regulation for personal data processing" },
    FrameworkInfo { slug: "hipaa", name: "HIPAA", description: "US health data privacy and security regulation" },
    FrameworkInfo { slug: "pci-dss", name: "PCI DSS", description: "Payment Card Industry Data Security Standard" },
    FrameworkInfo { slug: "ccpa", name: "CCPA / CPRA", description: "California Consumer Privacy Act and California Privacy Rights Act" },
    FrameworkInfo { slug: "soc3", name: "SOC 3", description: "Public-facing summary of SOC 2 report" },
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
        None => dialoguer::Input::<String>::new()
            .with_prompt("Trust center company name")
            .default(title.to_string())
            .interact_text()?,
    };

    let frameworks: Vec<String> = match &args.trust_frameworks {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).collect(),
        None => {
            let names: Vec<&str> = FRAMEWORKS.iter().map(|f| f.name).collect();
            let defaults = &[true, false, false, false, false, false, false];
            let selections = dialoguer::MultiSelect::new()
                .with_prompt("Compliance frameworks")
                .items(&names)
                .defaults(defaults)
                .interact()?;
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
            let selections = dialoguer::MultiSelect::new()
                .with_prompt("Trust center sections")
                .items(&section_names)
                .defaults(defaults)
                .interact()?;
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
        let status = if args.trust_frameworks.is_some() {
            // Non-interactive: default to "in_progress"
            "in_progress".to_string()
        } else {
            let options = ["Active (certified)", "In Progress (pursuing)", "Planned (on roadmap)"];
            let selection = dialoguer::Select::new()
                .with_prompt(format!("{fw_name} status"))
                .items(&options)
                .default(0)
                .interact()?;
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
        let mut faq = String::from("# Security FAQ — edit answers to match your actual security posture.\n\n");
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
                "in_progress" => format!("{} is currently pursuing {name} certification.", opts.company),
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

/// Generate .claude/settings.json with pre-approved tools and MCP server config.
fn generate_claude_settings() -> String {
    r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "permissions": {
    "allow": [
      "Read",
      "Write(content/**)",
      "Write(templates/**)",
      "Write(static/**)",
      "Write(data/**)",
      "Edit(content/**)",
      "Edit(templates/**)",
      "Edit(data/**)",
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
  },
  "mcpServers": {
    "page": {
      "command": "page",
      "args": ["mcp"]
    }
  }
}
"#
    .to_string()
}

/// The MCP server block that should be present in .claude/settings.json.
/// Used by upgrade to merge into existing settings.
pub fn mcp_server_block() -> serde_json::Value {
    serde_json::json!({
        "page": {
            "command": "page",
            "args": ["mcp"]
        }
    })
}

/// Generate a CLAUDE.md tailored to the site's collections and structure.
fn generate_claude_md(
    title: &str,
    description: &str,
    collections: &[CollectionConfig],
    trust_opts: Option<&TrustOptions>,
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
    md.push_str("page deploy                             # Commit, push, build, and deploy\n");
    md.push_str("page deploy --no-commit                 # Deploy without auto-commit/push\n");
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
    md.push_str("data/            # Data files (YAML/JSON/TOML) → {{ data.filename }} in templates\n");
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
        // Changelog-specific guidance
        if c.name == "changelog" {
            md.push_str("- Tag conventions: `new` (features), `fix` (bug fixes), `breaking` (breaking changes), `improvement` (enhancements), `deprecated` (deprecations)\n");
            md.push_str("- Tags render as colored badges in the changelog template\n");
            md.push_str("- Create entries: `page new changelog \"v1.0.0\" --tags new,improvement`\n");
        }
        // Roadmap-specific guidance
        if c.name == "roadmap" {
            md.push_str("- Status tags: `planned`, `in-progress`, `done`, `cancelled`\n");
            md.push_str("- Use `weight:` in frontmatter to control ordering (lower = higher priority)\n");
            md.push_str("- Default index groups items by status tag\n");
            md.push_str("- Alternative layouts: copy `roadmap-kanban.html` or `roadmap-timeline.html` to `templates/roadmap-index.html`\n");
            md.push_str("- Create items: `page new roadmap \"Feature Name\" --tags planned`\n");
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
    md.push_str("weight: 1                # optional, sort order for non-date collections (lower first)\n");
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
    md.push_str("### i18n Template Helpers\n\n");
    md.push_str("Use `{{ lang_prefix }}` to prefix internal links for the current language (empty for default, `\"/es\"` for Spanish, etc.):\n\n");
    md.push_str("```html\n");
    md.push_str("<a href=\"{{ lang_prefix }}/about\">About</a>\n");
    md.push_str("```\n\n");
    md.push_str("In `data/nav.yaml` and `data/footer.yaml`, mark external links with `external: true`:\n\n");
    md.push_str("```yaml\n");
    md.push_str("- title: Blog\n");
    md.push_str("  url: /posts\n");
    md.push_str("- title: GitHub\n");
    md.push_str("  url: https://github.com/user/repo\n");
    md.push_str("  external: true\n");
    md.push_str("```\n\n");
    md.push_str("All bundled themes automatically apply `lang_prefix` to internal data file links and open external links in new tabs.\n\n");
    md.push_str("### Translating UI Strings\n\n");
    md.push_str("All UI text in themes uses the `{{ t.key }}` object (search placeholder, pagination, 404 text, etc.). ");
    md.push_str("Override defaults by creating `data/i18n/{lang}.yaml`:\n\n");
    md.push_str("```yaml\n");
    md.push_str("# data/i18n/es.yaml\n");
    md.push_str("search_placeholder: \"Buscar\\u{2026}\"\n");
    md.push_str("skip_to_content: \"Ir al contenido principal\"\n");
    md.push_str("no_results: \"Sin resultados\"\n");
    md.push_str("newer: \"M\\u{00e1}s recientes\"\n");
    md.push_str("older: \"M\\u{00e1}s antiguos\"\n");
    md.push_str("```\n\n");
    md.push_str("Available keys: `search_placeholder`, `skip_to_content`, `no_results`, `newer`, `older`, `page_n_of_total`, `search_label`, `min_read`, `contents`, `tags`, `all_tags`, `tagged`, `changelog`, `roadmap`, `not_found_title`, `not_found_message`, `go_home`, `in_progress`, `planned`, `done`, `other`, `trust_center`, `trust_hero_subtitle`, `certifications_compliance`, `active`, `learn_more`, `auditor`, `scope`, `issued`, `expires`, `subprocessors`, `vendor`, `purpose`, `location`, `dpa`, `yes`, `no`, `faq`.\n\n");

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
    md.push_str("| `site.language` | string | Default language code (configured in `page.toml`) |\n");
    md.push_str("| `site.author` | string | Author name |\n");
    md.push_str("| `lang` | string | Current page language code |\n");
    md.push_str("| `default_language` | string | Default language code (same as `site.language`) |\n");
    md.push_str("| `lang_prefix` | string | URL prefix for current language (empty for default, `\"/es\"` for Spanish, etc.) |\n");
    md.push_str("| `t` | object | UI translation strings (override via `data/i18n/{lang}.yaml`) |\n");
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
    md.push_str("  - `<html lang=\"{{ lang }}\">` — language attribute\n");
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

    // MCP Server
    md.push_str("## MCP Server\n\n");
    md.push_str("This project includes a built-in MCP (Model Context Protocol) server that AI tools\n");
    md.push_str("connect to for structured access to site content, documentation, themes, and build tools.\n\n");
    md.push_str("The server is configured in `.claude/settings.json` and starts automatically\n");
    md.push_str("when Claude Code opens this project. No API keys or setup required.\n\n");
    md.push_str("**Available tools:** `page_build`, `page_create_content`, `page_search`,\n");
    md.push_str("`page_apply_theme`, `page_lookup_docs`\n\n");
    md.push_str("**Available resources:** `page://docs/*` (page documentation),\n");
    md.push_str("`page://content/*` (site content), `page://themes` (themes),\n");
    md.push_str("`page://config` (site configuration), `page://mcp-config` (MCP settings)\n\n");

    // Trust Center section (only if trust collection is present)
    if let Some(opts) = trust_opts {
        md.push_str("## Trust Center\n\n");
        md.push_str(&format!("This site includes a compliance trust center at `/trust/`. Company: **{}**.\n\n", opts.company));

        if !opts.frameworks.is_empty() {
            md.push_str("### Active Frameworks\n\n");
            for (slug, status) in &opts.framework_statuses {
                let fw = framework_by_slug(slug);
                let name = fw.map(|f| f.name).unwrap_or(slug.as_str());
                let badge = match status.as_str() {
                    "active" => "Active",
                    "in_progress" => "In Progress",
                    _ => "Planned",
                };
                md.push_str(&format!("- **{name}** — {badge}\n"));
            }
            md.push('\n');
        }

        md.push_str("### How the Trust Center Works\n\n");
        md.push_str("The trust center has three layers:\n\n");
        md.push_str("1. **Data files** (`data/trust/`) — structured YAML that drives the templates\n");
        md.push_str("2. **Content pages** (`content/trust/`) — markdown prose for each section\n");
        md.push_str("3. **Templates** (`templates/trust-index.html`, `templates/trust-item.html`) — layout (rarely edited)\n\n");

        md.push_str("### Managing Certifications\n\n");
        md.push_str("Edit `data/trust/certifications.yaml` to update certification statuses:\n\n");
        md.push_str("```yaml\n");
        md.push_str("- name: SOC 2 Type II\n");
        md.push_str("  slug: soc2\n");
        md.push_str("  status: active         # active | in_progress | planned\n");
        md.push_str("  framework: soc2\n");
        md.push_str("  description: >         # shown on trust center hub\n");
        md.push_str("    Annual audit covering Security and Availability\n");
        md.push_str("  issued: 2025-11-15     # date cert was issued\n");
        md.push_str("  expires: 2026-11-15    # expiration date\n");
        md.push_str("  auditor: \"Deloitte\"\n");
        md.push_str("  scope: \"Security, Availability\"\n");
        md.push_str("  report_url: \"mailto:security@example.com\"\n");
        md.push_str("```\n\n");
        md.push_str("Status values: `active` (green badge), `in_progress` (yellow), `planned` (gray).\n\n");
        md.push_str("To add a new certification:\n");
        md.push_str("1. Add entry to `data/trust/certifications.yaml`\n");
        md.push_str("2. Create `content/trust/certifications/{slug}.md` with framework details\n");
        md.push_str("3. Run `page build`\n\n");

        md.push_str("### Managing Subprocessors\n\n");
        md.push_str("Edit `data/trust/subprocessors.yaml`:\n\n");
        md.push_str("```yaml\n");
        md.push_str("- name: \"AWS\"\n");
        md.push_str("  purpose: \"Cloud infrastructure\"\n");
        md.push_str("  data_types: [\"Customer data\", \"Logs\"]\n");
        md.push_str("  location: \"United States\"\n");
        md.push_str("  dpa: true\n");
        md.push_str("```\n\n");
        md.push_str("Fields: `name` (required), `purpose`, `data_types` (array), `location`, `dpa` (bool).\n\n");

        md.push_str("### Managing FAQs\n\n");
        md.push_str("Edit `data/trust/faq.yaml`:\n\n");
        md.push_str("```yaml\n");
        md.push_str("- question: \"Do you encrypt data at rest?\"\n");
        md.push_str("  answer: \"Yes. All data encrypted with AES-256.\"\n");
        md.push_str("  category: encryption     # groups FAQs in the UI\n");
        md.push_str("```\n\n");
        md.push_str("Categories: `encryption`, `access`, `data-residency`, `incident-response`, `compliance`, `general`.\n\n");

        md.push_str("### Trust Center Content Pages\n\n");
        md.push_str("Each section is a markdown file in `content/trust/`:\n\n");
        md.push_str("| File | URL | Purpose |\n");
        md.push_str("|------|-----|----------|\n");
        md.push_str("| `security-overview.md` | `/trust/security-overview` | Main security narrative |\n");
        md.push_str("| `vulnerability-disclosure.md` | `/trust/vulnerability-disclosure` | Responsible disclosure |\n");
        md.push_str("| `data-processing.md` | `/trust/data-processing` | DPA / data processing terms |\n");
        md.push_str("| `certifications/soc2.md` | `/trust/certifications/soc2` | Framework detail page |\n\n");
        md.push_str("Use `weight:` in frontmatter to control section ordering (lower = first).\n");
        md.push_str("Use `extra.type:` to categorize: `overview`, `certification`, `policy`, `changelog`.\n\n");

        md.push_str("### Common Trust Center Tasks\n\n");
        md.push_str("```bash\n");
        md.push_str("page new trust \"PCI DSS\"                    # Add a new certification page\n");
        md.push_str("page new trust \"Q1 2026 Security Update\"    # Add a changelog entry\n");
        md.push_str("page new trust \"Security Overview\" --lang es # Create a translation\n");
        md.push_str("page build                                   # Rebuild after editing data files\n");
        md.push_str("```\n\n");

        md.push_str("### Multi-language Trust Center\n\n");
        md.push_str("Data files (`data/trust/*.yaml`) are language-neutral. Content pages get translated via the standard i18n system:\n\n");
        md.push_str("```\n");
        md.push_str("content/trust/security-overview.md       # English → /trust/security-overview\n");
        md.push_str("content/trust/security-overview.es.md    # Spanish → /es/trust/security-overview\n");
        md.push_str("```\n\n");
        md.push_str("The trust center index at `/trust/` is rendered per-language automatically.\n\n");

        md.push_str("### MCP Integration\n\n");
        md.push_str("`page://trust` returns the full trust center state (certifications, subprocessors, FAQs, content items).\n");
        md.push_str("Use `page_search` with `collection: \"trust\"` to find trust center content.\n");
        md.push_str("Use `page_create_content` with `collection: \"trust\"` and `extra: {\"type\": \"certification\", \"framework\": \"soc2\"}` to create trust center pages.\n\n");
    }

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
    md.push_str("- Deploy auto-commits and pushes before deploying. On non-main branches, it auto-uses preview mode. Disable with `auto_commit = false` in `[deploy]` or `--no-commit` flag\n\n");

    // Documentation links
    md.push_str("## Documentation\n\n");
    md.push_str("Full documentation: <https://pagecli.dev/docs/getting-started>\n\n");
    md.push_str("- [Getting Started](https://pagecli.dev/docs/getting-started) — install and create your first site\n");
    md.push_str("- [Configuration](https://pagecli.dev/docs/configuration) — full `page.toml` reference\n");
    md.push_str("- [Templates & Themes](https://pagecli.dev/docs/templates) — customize templates and themes\n");
    md.push_str("- [Shortcodes](https://pagecli.dev/docs/shortcodes) — reusable content components\n");
    md.push_str("- [CLI Reference](https://pagecli.dev/docs/cli-reference) — all commands and flags\n");
    md.push_str("- [AI Agent](https://pagecli.dev/docs/agent) — using the AI assistant\n");

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
