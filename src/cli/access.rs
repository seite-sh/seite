use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

use clap::{Args, Subcommand};

use crate::config::{
    normalize_access_prefix, password_secret_binding, session_secret_binding, DeployTarget,
    SiteConfig,
};
use crate::output::human;
use crate::platform::npm_cmd;

#[derive(Args)]
pub struct AccessArgs {
    #[command(subcommand)]
    pub command: AccessCommand,
}

#[derive(Subcommand)]
pub enum AccessCommand {
    /// List password groups and the paths or subdomains they protect
    Groups,
    /// Securely upload a password for one access group to Cloudflare Pages
    SetPassword {
        /// Access group (inferred when only one group exists)
        group: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessEnvironment {
    Production,
    Preview,
}

impl AccessEnvironment {
    fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Preview => "preview",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccessGroup {
    name: String,
    scopes: Vec<String>,
    projects: Vec<String>,
    missing_projects: bool,
}

pub fn run(args: &AccessArgs) -> anyhow::Result<()> {
    let config = SiteConfig::load(&PathBuf::from("seite.toml"))?;
    if config.access.is_none() {
        anyhow::bail!("password access is not enabled; add [access] to seite.toml");
    }

    let groups = collect_access_groups(&config);
    match &args.command {
        AccessCommand::Groups => print_groups(&groups),
        AccessCommand::SetPassword { group } => set_password(&config, &groups, group.as_deref()),
    }
}

fn collect_access_groups(config: &SiteConfig) -> Vec<AccessGroup> {
    let mut grouped: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>, bool)> = BTreeMap::new();

    for collection in &config.collections {
        let Some(group) = collection.resolved_access_group() else {
            continue;
        };
        let (scope, project) = if let Some(subdomain) = &collection.subdomain {
            (
                format!("subdomain {subdomain} (entire site)"),
                collection.deploy_project.as_deref(),
            )
        } else {
            let path = normalize_access_prefix(&collection.url_prefix);
            (format!("path {path}"), config.deploy.project.as_deref())
        };

        let entry = grouped
            .entry(group.to_string())
            .or_insert_with(|| (BTreeSet::new(), BTreeSet::new(), false));
        entry.0.insert(scope);
        if let Some(project) = project {
            entry.1.insert(project.to_string());
        } else {
            entry.2 = true;
        }
    }

    grouped
        .into_iter()
        .map(|(name, (scopes, projects, missing_projects))| AccessGroup {
            name,
            scopes: scopes.into_iter().collect(),
            projects: projects.into_iter().collect(),
            missing_projects,
        })
        .collect()
}

fn select_group<'a>(
    groups: &'a [AccessGroup],
    requested: Option<&str>,
) -> anyhow::Result<&'a AccessGroup> {
    if groups.is_empty() {
        anyhow::bail!("no private collections are configured");
    }
    if let Some(requested) = requested {
        return groups
            .iter()
            .find(|group| group.name == requested)
            .ok_or_else(|| anyhow::anyhow!("unknown access group '{requested}'"));
    }
    if groups.len() == 1 {
        return Ok(&groups[0]);
    }
    anyhow::bail!(
        "multiple access groups are configured; specify a group: {}",
        groups
            .iter()
            .map(|group| group.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn print_groups(groups: &[AccessGroup]) -> anyhow::Result<()> {
    crate::output::json::set_data(serde_json::json!({
        "groups": groups
            .iter()
            .map(|g| serde_json::json!({
                "name": g.name,
                "scopes": g.scopes,
                "projects": g.projects,
                "missing_projects": g.missing_projects,
            }))
            .collect::<Vec<_>>(),
    }));
    if groups.is_empty() {
        human::info("No private collections are configured.");
        return Ok(());
    }

    crate::human_println!("{:<20} {:<30} CLOUDFLARE PROJECTS", "GROUP", "SCOPES");
    crate::human_println!("{}", "-".repeat(76));
    for group in groups {
        let projects = if group.missing_projects {
            "(project not configured)".to_string()
        } else {
            group.projects.join(", ")
        };
        crate::human_println!(
            "{:<20} {:<30} {}",
            group.name,
            group.scopes.join(", "),
            projects
        );
    }
    Ok(())
}

fn set_password(
    config: &SiteConfig,
    groups: &[AccessGroup],
    requested: Option<&str>,
) -> anyhow::Result<()> {
    if config.deploy.target != DeployTarget::Cloudflare {
        anyhow::bail!("password access is only supported on Cloudflare Pages");
    }
    let group = select_group(groups, requested)?;
    if group.missing_projects || group.projects.is_empty() {
        anyhow::bail!(
            "access group '{}' has a scope without a Cloudflare Pages project; set deploy.project for the main site and deploy_project for each subdomain collection",
            group.name
        );
    }

    let password = crate::cli::prompt::password(
        &format!("Password for access group '{}'", group.name),
        Some("Confirm password"),
        "run `seite access set-password` from an interactive terminal (passwords are never read from flags)",
    )?;
    if password.is_empty() {
        anyhow::bail!("password cannot be empty");
    }

    let password_key = password_secret_binding(&group.name);
    let session_key = session_secret_binding(&group.name);
    for project in &group.projects {
        // Stage this group's secrets for both Pages environments. Pages applies
        // them to subsequent deployments, not the currently running deployment.
        // If any upload fails, rerun the command before deploying.
        for environment in [AccessEnvironment::Production, AccessEnvironment::Preview] {
            let session_secret = random_secret()?;
            put_pages_secret(project, &session_key, &session_secret, environment)?;
            put_pages_secret(project, &password_key, &password, environment)?;
        }
        human::success(&format!(
            "Stored password for '{}' on Cloudflare Pages project '{}' (production and preview)",
            group.name, project
        ));
    }
    human::info(
        "Deploy this commit from the Cloudflare Pages project's production branch to activate production (seite-created projects use `main`). Run `seite deploy --preview` to activate preview. Existing deployments keep their current password and sessions until redeployed.",
    );
    Ok(())
}

fn random_secret() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("could not generate session secret: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn pages_secret_args(project: &str, key: &str, environment: AccessEnvironment) -> Vec<String> {
    vec![
        "pages".into(),
        "secret".into(),
        "put".into(),
        key.into(),
        "--project-name".into(),
        project.into(),
        "--env".into(),
        environment.as_str().into(),
    ]
}

fn put_pages_secret(
    project: &str,
    key: &str,
    value: &str,
    environment: AccessEnvironment,
) -> anyhow::Result<()> {
    let args = pages_secret_args(project, key, environment);
    let mut child = npm_cmd("wrangler")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| anyhow::anyhow!("failed to start wrangler: {error}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to open wrangler stdin"))?;
    stdin.write_all(value.as_bytes())?;
    drop(stdin);

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "wrangler could not update secret '{key}' for project '{project}': {}",
            message.trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AccessSection, CollectionConfig, SiteConfig};

    fn grouped_config() -> SiteConfig {
        let mut config: SiteConfig = toml::from_str(
            r#"
collections = []

[site]
title = "Test"
base_url = "https://example.com"

[deploy]
target = "cloudflare"
project = "main-site"

[access]
mode = "password"
"#,
        )
        .unwrap();
        config.access = Some(AccessSection::default());

        let mut posts = CollectionConfig::preset_posts();
        posts.private = true;
        posts.url_prefix = "/members".into();
        posts.access_group = Some("shared".into());

        let mut docs = CollectionConfig::preset_docs();
        docs.private = true;
        docs.access_group = Some("docs-team".into());
        docs.subdomain = Some("docs".into());
        docs.deploy_project = Some("docs-site".into());
        config.collections = vec![posts, docs];
        config
    }

    #[test]
    fn access_groups_map_paths_and_subdomains_to_projects() {
        let groups = collect_access_groups(&grouped_config());

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "docs-team");
        assert_eq!(groups[0].projects, vec!["docs-site"]);
        assert!(groups[0]
            .scopes
            .iter()
            .any(|scope| scope.contains("subdomain")));
        assert_eq!(groups[1].name, "shared");
        assert_eq!(groups[1].projects, vec!["main-site"]);
        assert!(groups[1]
            .scopes
            .iter()
            .any(|scope| scope.contains("/members")));
    }

    #[test]
    fn access_groups_normalize_path_scopes() {
        let mut config = grouped_config();
        config.collections.truncate(1);
        config.collections[0].url_prefix = "members/".into();

        let groups = collect_access_groups(&config);

        assert_eq!(groups[0].scopes, vec!["path /members"]);
    }

    #[test]
    fn group_selection_requires_a_name_when_multiple_exist() {
        let groups = collect_access_groups(&grouped_config());
        let error = select_group(&groups, None).unwrap_err().to_string();
        assert!(error.contains("specify a group"));
        assert_eq!(
            select_group(&groups, Some("shared")).unwrap().name,
            "shared"
        );
    }

    #[test]
    fn group_selection_infers_the_only_group() {
        let mut config = grouped_config();
        config.collections.truncate(1);
        let groups = collect_access_groups(&config);
        assert_eq!(select_group(&groups, None).unwrap().name, "shared");
    }

    #[test]
    fn pages_secret_args_target_the_selected_environment() {
        assert_eq!(
            pages_secret_args("site", "SEITE_PASSWORD_STAFF", AccessEnvironment::Preview),
            vec![
                "pages",
                "secret",
                "put",
                "SEITE_PASSWORD_STAFF",
                "--project-name",
                "site",
                "--env",
                "preview",
            ]
        );
        assert_eq!(AccessEnvironment::Production.as_str(), "production");
    }

    #[test]
    fn missing_cloudflare_project_is_reported() {
        let mut config = grouped_config();
        config.deploy.project = None;
        config.collections.truncate(1);
        let groups = collect_access_groups(&config);
        assert!(groups[0].missing_projects);
    }
}
