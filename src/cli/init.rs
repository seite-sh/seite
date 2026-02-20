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
    let mut md = String::with_capacity(16384);

    // Header (dynamic)
    md.push_str(&format!("# {title}\n\n"));
    if !description.is_empty() {
        md.push_str(&format!("{description}\n\n"));
    }
    md.push_str("This is a static site built with the `page` CLI tool.\n\n");

    // SEO and GEO requirements (static)
    md.push_str(include_str!("../scaffold/seo-requirements.md"));

    // Commands (dynamic — iterates collections)
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
    md.push_str("page collection list                    # List site collections\n");
    md.push_str("page collection add <preset>            # Add a preset collection (posts, docs, pages, changelog, roadmap, trust)\n");
    md.push_str("page theme list                         # List available themes\n");
    md.push_str("page theme apply <name>                 # Apply a bundled theme (default, minimal, dark, docs, brutalist, bento)\n");
    md.push_str("page theme create \"coral brutalist\"     # Generate a custom theme with AI (requires Claude Code)\n");
    md.push_str("page theme install <url>                # Install theme from URL\n");
    md.push_str("page theme export <name>                # Export current theme for sharing\n");
    md.push_str("page agent                              # Interactive AI agent session\n");
    md.push_str("page agent \"write about Rust\"           # One-shot AI agent prompt\n");
    md.push_str("page deploy                             # Commit, push, build, and deploy\n");
    md.push_str("page deploy --no-commit                 # Deploy without auto-commit/push\n");
    md.push_str("```\n\n");

    // Dev server REPL (static)
    md.push_str(include_str!("../scaffold/repl.md"));

    // Project structure (dynamic — iterates collections)
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

    // Collections (dynamic — iterates collections with conditional sections)
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
        if c.name == "changelog" {
            md.push_str("- Tag conventions: `new` (features), `fix` (bug fixes), `breaking` (breaking changes), `improvement` (enhancements), `deprecated` (deprecations)\n");
            md.push_str("- Tags render as colored badges in the changelog template\n");
            md.push_str("- Create entries: `page new changelog \"v1.0.0\" --tags new,improvement`\n");
        }
        if c.name == "roadmap" {
            md.push_str("- Status tags: `planned`, `in-progress`, `done`, `cancelled`\n");
            md.push_str("- Use `weight:` in frontmatter to control ordering (lower = higher priority)\n");
            md.push_str("- Default index groups items by status tag\n");
            md.push_str("- Alternative layouts: copy `roadmap-kanban.html` or `roadmap-timeline.html` to `templates/roadmap-index.html`\n");
            md.push_str("- Create items: `page new roadmap \"Feature Name\" --tags planned`\n");
        }
        md.push('\n');
    }

    // Content format (dynamic — conditional date field)
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
    md.push_str("extra:                   # optional, arbitrary data → {{ page.extra.field }}\n");
    md.push_str("  key: value\n");
    md.push_str("---\n\n");
    md.push_str("Markdown content here.\n");
    md.push_str("```\n\n");
    md.push_str("Use `<!-- more -->` in content to mark the excerpt boundary. Without it, the first paragraph is used as the excerpt.\n\n");

    // Homepage (dynamic — conditional on pages collection)
    if collections.iter().any(|c| c.name == "pages") {
        md.push_str("### Homepage\n\n");
        md.push_str("To add custom content to the homepage, create `content/pages/index.md`. ");
        md.push_str("Its rendered content will appear above the collection listings on the index page. ");
        md.push_str(
            "The homepage is injected as `{{ page.content }}` in the index template.\n\n",
        );
    }

    // Multi-language support (static)
    md.push_str(include_str!("../scaffold/i18n.md"));

    // Data files (static)
    md.push_str(include_str!("../scaffold/data-files.md"));

    // Templates and themes (static)
    md.push_str(include_str!("../scaffold/templates.md"));

    // Features (static — docs sidebar is common enough to always include)
    md.push_str(include_str!("../scaffold/features.md"));

    // Optional configuration (static)
    md.push_str(include_str!("../scaffold/config-reference.md"));

    // MCP Server (static)
    md.push_str(include_str!("../scaffold/mcp.md"));

    // Trust Center (dynamic — only if trust collection is present)
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

    // Shortcodes (static)
    md.push_str(include_str!("../scaffold/shortcodes.md"));

    // Design prompts (static)
    md.push_str(include_str!("../scaffold/design-prompts.md"));

    // Key conventions (short, mixed static/dynamic — keep inline)
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
