use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::error::{PageError, Result};

fn credentials_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("page");
    dir.join("credentials.toml")
}

fn load_credentials() -> HashMap<String, String> {
    let path = credentials_path();
    if let Ok(content) = fs::read_to_string(&path) {
        toml::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

fn save_credentials(creds: &HashMap<String, String>) -> Result<()> {
    let path = credentials_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| PageError::Auth(format!("failed to create config dir: {e}")))?;
    }
    let content = toml::to_string_pretty(creds)
        .map_err(|e| PageError::Auth(format!("failed to serialize credentials: {e}")))?;
    fs::write(&path, &content)
        .map_err(|e| PageError::Auth(format!("failed to write credentials: {e}")))?;

    // Restrict file permissions to owner only (unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(&path, perms);
    }

    Ok(())
}

pub fn store_key(provider: &str, key: &str) -> Result<()> {
    let mut creds = load_credentials();
    creds.insert(provider.to_string(), key.to_string());
    save_credentials(&creds)
}

pub fn get_key(provider: &str) -> Result<String> {
    let creds = load_credentials();
    creds
        .get(provider)
        .cloned()
        .ok_or_else(|| PageError::Auth(format!("no key found for {provider}. Run: page auth login {provider}")))
}

pub fn delete_key(provider: &str) -> Result<()> {
    let mut creds = load_credentials();
    creds.remove(provider);
    save_credentials(&creds)
}

pub fn list_providers() -> Vec<(&'static str, bool)> {
    let creds = load_credentials();
    let providers = ["claude", "openai"];
    providers
        .iter()
        .map(|p| (*p, creds.contains_key(*p)))
        .collect()
}
