use std::process::Command;

use crate::config::ResolvedPaths;
use crate::error::{PageError, Result};

pub fn deploy_github_pages(paths: &ResolvedPaths, repo: Option<&str>) -> Result<()> {
    // Verify git is available
    Command::new("git")
        .arg("--version")
        .output()
        .map_err(|_| PageError::Deploy("git is not installed".into()))?;

    let output_dir = &paths.output;
    if !output_dir.exists() {
        return Err(PageError::Deploy("output directory does not exist; run build first".into()));
    }

    // Determine repo URL
    let repo_url = match repo {
        Some(url) => url.to_string(),
        None => {
            // Try to detect from parent git repo
            let output = Command::new("git")
                .args(["remote", "get-url", "origin"])
                .current_dir(&paths.root)
                .output()
                .map_err(|e| PageError::Deploy(format!("failed to detect git remote: {e}")))?;
            if !output.status.success() {
                return Err(PageError::Deploy(
                    "no repo URL provided and could not detect git remote. Set deploy.repo in page.toml or pass --target".into(),
                ));
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };

    // Initialize a git repo in the output directory and push to gh-pages
    let run = |args: &[&str]| -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(output_dir)
            .output()
            .map_err(|e| PageError::Deploy(format!("git {}: {e}", args.join(" "))))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PageError::Deploy(format!(
                "git {} failed: {stderr}",
                args.join(" ")
            )));
        }
        Ok(())
    };

    run(&["init"])?;
    run(&["checkout", "-b", "gh-pages"])?;
    run(&["add", "-A"])?;
    run(&["commit", "-m", "Deploy"])?;
    run(&["push", "--force", &repo_url, "gh-pages"])?;

    Ok(())
}

pub fn deploy_cloudflare(paths: &ResolvedPaths, project: &str) -> Result<()> {
    let output_dir = &paths.output;
    if !output_dir.exists() {
        return Err(PageError::Deploy("output directory does not exist; run build first".into()));
    }

    // Check if wrangler is available
    let wrangler_check = Command::new("wrangler")
        .arg("--version")
        .output();

    match wrangler_check {
        Ok(output) if output.status.success() => {
            let result = Command::new("wrangler")
                .args([
                    "pages",
                    "deploy",
                    output_dir.to_str().unwrap_or("dist"),
                    "--project-name",
                    project,
                ])
                .status()
                .map_err(|e| PageError::Deploy(format!("wrangler failed: {e}")))?;
            if !result.success() {
                return Err(PageError::Deploy("wrangler pages deploy failed".into()));
            }
            Ok(())
        }
        _ => Err(PageError::Deploy(
            "wrangler CLI is not installed. Install it with: npm install -g wrangler".into(),
        )),
    }
}
