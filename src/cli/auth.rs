use clap::{Args, Subcommand};

use crate::credential;
use crate::error::PageError;
use crate::output::human;

#[derive(Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Log in to an LLM provider (opens browser)
    Login {
        /// Provider name (claude, openai)
        #[arg(default_value = "claude")]
        provider: String,
    },

    /// Add credentials for an LLM provider
    Add {
        /// Provider name (claude, openai)
        provider: String,

        /// API key (if not provided, will prompt interactively)
        #[arg(long)]
        key: Option<String>,
    },

    /// List configured providers
    List,

    /// Remove credentials for a provider
    Remove {
        /// Provider name to remove
        provider: String,
    },
}

pub fn run(args: &AuthArgs) -> anyhow::Result<()> {
    match &args.command {
        AuthCommand::Login { provider } => {
            validate_provider(provider)?;
            login(provider)?;
        }
        AuthCommand::Add { provider, key } => {
            validate_provider(provider)?;
            let api_key = match key {
                Some(k) => k.clone(),
                None => dialoguer::Password::new()
                    .with_prompt(format!("Enter API key for {provider}"))
                    .interact()?,
            };
            credential::store_key(provider, &api_key)?;
            human::success(&format!("Stored key for {provider}"));
        }
        AuthCommand::List => {
            human::header("Configured providers");
            let providers = credential::list_providers();
            for (name, configured) in providers {
                if configured {
                    human::success(&format!("{name}: configured"));
                } else {
                    human::info(&format!("{name}: not configured"));
                }
            }
        }
        AuthCommand::Remove { provider } => {
            validate_provider(provider)?;
            credential::delete_key(provider)?;
            human::success(&format!("Removed key for {provider}"));
        }
    }
    Ok(())
}

fn validate_provider(provider: &str) -> anyhow::Result<()> {
    match provider {
        "claude" | "openai" => Ok(()),
        other => Err(
            PageError::Auth(format!("unknown provider: {other}. Use 'claude' or 'openai'")).into(),
        ),
    }
}

/// Open the provider's API key page in the browser, then prompt the user to paste it.
pub fn login(provider: &str) -> anyhow::Result<()> {
    let (url, key_prefix, key_hint) = match provider {
        "claude" => (
            "https://console.anthropic.com/settings/keys",
            "sk-ant-",
            "sk-ant-api03-...",
        ),
        "openai" => (
            "https://platform.openai.com/api-keys",
            "sk-",
            "sk-...",
        ),
        other => {
            return Err(
                PageError::Auth(format!("unknown provider: {other}")).into(),
            );
        }
    };

    human::info(&format!("Opening {provider} API keys page in your browser..."));
    human::info("Create or copy an API key, then paste it below.");
    println!();

    if webbrowser::open(url).is_err() {
        human::info(&format!("Could not open browser. Go to: {url}"));
    }

    // Use a plain line read so the user can see what they paste.
    // dialoguer::Password hides input which makes paste feel broken.
    print!("Paste your API key ({key_hint}): ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut api_key = String::new();
    std::io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err(PageError::Auth("no key provided".into()).into());
    }

    if !api_key.starts_with(key_prefix) {
        human::warning(&format!(
            "Key doesn't look like a {provider} key (expected prefix '{key_prefix}'). Storing anyway."
        ));
    }

    credential::store_key(provider, &api_key)?;
    human::success(&format!("Authenticated with {provider}"));

    Ok(())
}
