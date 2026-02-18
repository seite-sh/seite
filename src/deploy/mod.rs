use std::fs;
use std::process::Command;

use crate::config::{ResolvedPaths, SiteConfig};
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
                return Err(PageError::Deploy(format!(
                    "wrangler pages deploy failed for project '{project}'. \
                     Ensure the project exists in your Cloudflare account. \
                     Create it at https://dash.cloudflare.com/ or run: \
                     wrangler pages project create {project}"
                )));
            }
            Ok(())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(PageError::Deploy(format!(
                "wrangler CLI returned an error: {stderr}\n\
                 Ensure wrangler is properly installed and authenticated.\n\
                 Install: npm install -g wrangler\n\
                 Auth:    wrangler login"
            )))
        }
        Err(_) => Err(PageError::Deploy(
            "wrangler CLI is not installed. Install it with:\n  npm install -g wrangler\n\
             Then authenticate with:\n  wrangler login".into(),
        )),
    }
}

/// Try to auto-detect the Cloudflare project name from wrangler.toml or the directory name.
pub fn detect_cloudflare_project(paths: &ResolvedPaths) -> Option<String> {
    // Try wrangler.toml first
    let wrangler_path = paths.root.join("wrangler.toml");
    if wrangler_path.exists() {
        if let Ok(content) = fs::read_to_string(&wrangler_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("name") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        let name = val.trim().trim_matches('"').trim_matches('\'');
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fall back to directory name
    paths.root.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

pub fn deploy_netlify(paths: &ResolvedPaths, site_id: Option<&str>) -> Result<()> {
    let output_dir = &paths.output;
    if !output_dir.exists() {
        return Err(PageError::Deploy("output directory does not exist; run build first".into()));
    }

    // Check if netlify CLI is available
    let netlify_check = Command::new("netlify")
        .arg("--version")
        .output();

    match netlify_check {
        Ok(output) if output.status.success() => {
            let mut args = vec![
                "deploy",
                "--prod",
                "--dir",
                output_dir.to_str().unwrap_or("dist"),
            ];
            if let Some(id) = site_id {
                args.push("--site");
                args.push(id);
            }

            let result = Command::new("netlify")
                .args(&args)
                .status()
                .map_err(|e| PageError::Deploy(format!("netlify deploy failed: {e}")))?;
            if !result.success() {
                return Err(PageError::Deploy(
                    "netlify deploy failed. Ensure you are logged in (netlify login) \
                     and the site exists. You can link to an existing site with: \
                     netlify link".into(),
                ));
            }
            Ok(())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(PageError::Deploy(format!(
                "netlify CLI returned an error: {stderr}\n\
                 Ensure netlify-cli is properly installed and authenticated.\n\
                 Install: npm install -g netlify-cli\n\
                 Auth:    netlify login"
            )))
        }
        Err(_) => Err(PageError::Deploy(
            "netlify CLI is not installed. Install it with:\n  npm install -g netlify-cli\n\
             Then authenticate with:\n  netlify login".into(),
        )),
    }
}

/// Generate a GitHub Actions workflow YAML for building and deploying with GitHub Pages.
pub fn generate_github_actions_workflow(config: &SiteConfig) -> String {
    let rust_version = "1.75";
    let output_dir = &config.build.output_dir;
    format!(
        r#"name: Deploy to GitHub Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: {rust_version}

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{{{ runner.os }}}}-cargo-${{{{ hashFiles('**/Cargo.lock') }}}}

      - name: Install page
        run: cargo install --path .

      - name: Build site
        run: page build

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: {output_dir}

  deploy:
    environment:
      name: github-pages
      url: ${{{{ steps.deployment.outputs.page_url }}}}
    runs-on: ubuntu-latest
    needs: build
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
"#
    )
}
