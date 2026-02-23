use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::config::{ContactProvider, ContactSection, SiteConfig};
use crate::output::human;

#[derive(Args)]
pub struct ContactArgs {
    #[command(subcommand)]
    pub command: ContactCommand,
}

#[derive(Subcommand)]
pub enum ContactCommand {
    /// Set up a contact form provider
    Setup(SetupArgs),
    /// Remove the contact form configuration
    Remove,
    /// Show current contact form configuration
    Status,
}

#[derive(Args)]
pub struct SetupArgs {
    /// Contact form provider (formspree, web3forms, netlify, hubspot, typeform)
    #[arg(long)]
    pub provider: Option<String>,
    /// Provider-specific endpoint/ID
    #[arg(long)]
    pub endpoint: Option<String>,
    /// HubSpot region (na1 or eu1)
    #[arg(long)]
    pub region: Option<String>,
    /// Custom redirect URL after form submission
    #[arg(long)]
    pub redirect: Option<String>,
    /// Email subject line prefix
    #[arg(long)]
    pub subject: Option<String>,
}

pub fn run(args: &ContactArgs) -> anyhow::Result<()> {
    match &args.command {
        ContactCommand::Setup(setup_args) => run_setup(setup_args),
        ContactCommand::Remove => run_remove(),
        ContactCommand::Status => run_status(),
    }
}

fn run_setup(args: &SetupArgs) -> anyhow::Result<()> {
    let config_path = PathBuf::from("seite.toml");
    let site_config = SiteConfig::load(&config_path)?;

    if site_config.contact.is_some() {
        human::info("A contact form is already configured.");
        let replace = dialoguer::Confirm::new()
            .with_prompt("Replace existing configuration?")
            .default(false)
            .interact()?;
        if !replace {
            return Ok(());
        }
    }

    let contact = prompt_contact_config(args, &site_config)?;

    // Write [contact] section to seite.toml
    let contents = fs::read_to_string(&config_path)?;
    let mut doc: toml::Table = contents
        .parse()
        .map_err(|e: toml::de::Error| anyhow::anyhow!("failed to parse seite.toml: {}", e))?;

    let contact_value = toml::Value::try_from(&contact)?;
    doc.insert("contact".to_string(), contact_value);

    let new_contents = toml::to_string_pretty(&doc)?;
    fs::write(&config_path, new_contents)?;

    human::success(&format!(
        "Contact form configured with {} provider",
        provider_display_name(&contact.provider)
    ));

    // Create contact page if pages collection exists and contact.md doesn't
    let paths = site_config.resolve_paths(&std::env::current_dir()?);
    let has_pages = site_config.collections.iter().any(|c| c.name == "pages");
    let contact_page = paths.content.join("pages/contact.md");
    if has_pages && !contact_page.exists() {
        let frontmatter =
            "---\ntitle: Contact\ndescription: Get in touch\n---\n\n{{< contact_form() >}}\n";
        if let Some(parent) = contact_page.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&contact_page, frontmatter)?;
        human::success("Created content/pages/contact.md");
    }

    human::info("Run `seite build` to generate the form, or `seite serve` to preview.");
    Ok(())
}

fn run_remove() -> anyhow::Result<()> {
    let config_path = PathBuf::from("seite.toml");
    let site_config = SiteConfig::load(&config_path)?;

    if site_config.contact.is_none() {
        human::info("No contact form configured.");
        return Ok(());
    }

    let contents = fs::read_to_string(&config_path)?;
    let mut doc: toml::Table = contents
        .parse()
        .map_err(|e: toml::de::Error| anyhow::anyhow!("failed to parse seite.toml: {}", e))?;

    doc.remove("contact");

    let new_contents = toml::to_string_pretty(&doc)?;
    fs::write(&config_path, new_contents)?;

    human::success("Removed [contact] section from seite.toml");
    human::info(
        "Note: Any {{< contact_form() >}} shortcodes in content will show an error on next build.",
    );
    Ok(())
}

fn run_status() -> anyhow::Result<()> {
    let config_path = PathBuf::from("seite.toml");
    let site_config = SiteConfig::load(&config_path)?;

    match &site_config.contact {
        Some(contact) => {
            println!("Contact Form Configuration:");
            println!("  Provider: {}", provider_display_name(&contact.provider));
            println!("  Endpoint: {}", contact.endpoint);
            if let Some(ref region) = contact.region {
                println!("  Region:   {region}");
            }
            if let Some(ref redirect) = contact.redirect {
                println!("  Redirect: {redirect}");
            }
            if let Some(ref subject) = contact.subject {
                println!("  Subject:  {subject}");
            }
        }
        None => {
            human::info("No contact form configured. Run `seite contact setup` to configure.");
        }
    }
    Ok(())
}

/// Interactive or CLI-driven contact configuration.
pub fn prompt_contact_config(
    args: &SetupArgs,
    site_config: &SiteConfig,
) -> anyhow::Result<ContactSection> {
    let provider = match &args.provider {
        Some(p) => parse_provider(p)?,
        None => {
            let is_netlify = site_config.deploy.target == crate::config::DeployTarget::Netlify;

            let items = if is_netlify {
                vec![
                    "Netlify Forms (recommended for Netlify)",
                    "Formspree",
                    "Web3Forms",
                    "HubSpot",
                    "Typeform",
                ]
            } else {
                vec![
                    "Formspree",
                    "Web3Forms",
                    "Netlify Forms",
                    "HubSpot",
                    "Typeform",
                ]
            };

            let selection = dialoguer::Select::new()
                .with_prompt("Select contact form provider")
                .items(&items)
                .default(0)
                .interact()?;

            let name = items[selection];
            match name {
                n if n.starts_with("Netlify") => ContactProvider::Netlify,
                "Formspree" => ContactProvider::Formspree,
                "Web3Forms" => ContactProvider::Web3forms,
                "HubSpot" => ContactProvider::Hubspot,
                "Typeform" => ContactProvider::Typeform,
                _ => unreachable!(),
            }
        }
    };

    let endpoint = match &args.endpoint {
        Some(e) => e.clone(),
        None => match provider {
            ContactProvider::Formspree => dialoguer::Input::new()
                .with_prompt("Formspree form ID (e.g., xpznqkdl)")
                .interact_text()?,
            ContactProvider::Web3forms => dialoguer::Input::new()
                .with_prompt("Web3Forms access key")
                .interact_text()?,
            ContactProvider::Netlify => dialoguer::Input::new()
                .with_prompt("Form name")
                .default("contact".to_string())
                .interact_text()?,
            ContactProvider::Hubspot => {
                let portal_id: String = dialoguer::Input::new()
                    .with_prompt("HubSpot portal ID")
                    .interact_text()?;
                let form_guid: String = dialoguer::Input::new()
                    .with_prompt("HubSpot form GUID")
                    .interact_text()?;
                format!("{portal_id}/{form_guid}")
            }
            ContactProvider::Typeform => dialoguer::Input::new()
                .with_prompt("Typeform form ID (e.g., abc123XY)")
                .interact_text()?,
        },
    };

    let region = if provider == ContactProvider::Hubspot {
        match &args.region {
            Some(r) => Some(r.clone()),
            None => {
                let r: String = dialoguer::Input::new()
                    .with_prompt("HubSpot region")
                    .default("na1".to_string())
                    .interact_text()?;
                if r == "na1" {
                    None // na1 is the default, don't store it
                } else {
                    Some(r)
                }
            }
        }
    } else {
        args.region.clone()
    };

    Ok(ContactSection {
        provider,
        endpoint,
        region,
        redirect: args.redirect.clone(),
        subject: args.subject.clone(),
    })
}

fn parse_provider(s: &str) -> anyhow::Result<ContactProvider> {
    match s.to_lowercase().as_str() {
        "formspree" => Ok(ContactProvider::Formspree),
        "web3forms" => Ok(ContactProvider::Web3forms),
        "netlify" => Ok(ContactProvider::Netlify),
        "hubspot" => Ok(ContactProvider::Hubspot),
        "typeform" => Ok(ContactProvider::Typeform),
        _ => anyhow::bail!(
            "unknown contact provider '{}'. Available: formspree, web3forms, netlify, hubspot, typeform",
            s
        ),
    }
}

fn provider_display_name(p: &ContactProvider) -> &'static str {
    match p {
        ContactProvider::Formspree => "Formspree",
        ContactProvider::Web3forms => "Web3Forms",
        ContactProvider::Netlify => "Netlify Forms",
        ContactProvider::Hubspot => "HubSpot",
        ContactProvider::Typeform => "Typeform",
    }
}
