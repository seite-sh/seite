//! `seite og preview <page>` — render one social card (`[og]`) to a PNG for
//! quick iteration on colors, layout and logo, without a build.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::{json, Value};

use super::page::{classify, Target};
use super::site_tools::{call_tool, load_site, str_of};
use crate::build::og::{OgPlan, PreviewPage};
use crate::config::OgTemplate;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct OgArgs {
    #[command(subcommand)]
    pub command: OgCommand,
}

#[derive(Subcommand)]
pub enum OgCommand {
    /// Render one page's social card to a PNG (`/` = the site card)
    Preview(PreviewArgs),
}

#[derive(Args)]
pub struct PreviewArgs {
    /// A content file (content/posts/hello.md) or a published URL path
    /// (/posts/hello); `/` or `/<lang>/` renders the site card
    pub target: String,

    /// Where to write the PNG (default: .seite/cache/og-preview.png)
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Try another layout without editing seite.toml
    #[arg(long, value_parser = ["default", "minimal", "bold"])]
    pub template: Option<String>,
}

pub fn run(args: &OgArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    match &args.command {
        OgCommand::Preview(a) => preview(a, site_filter),
    }
}

fn preview(args: &PreviewArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let (Some(config), Some(paths)) = (state.config.as_ref(), state.paths.as_ref()) else {
        anyhow::bail!("no site config");
    };
    let mut config = config.clone();
    // Preview works before `[og]` is enabled, with its defaults.
    let og = config.og.get_or_insert_with(Default::default);
    og.generate = true;
    if let Some(t) = &args.template {
        og.template = match t.as_str() {
            "minimal" => OgTemplate::Minimal,
            "bold" => OgTemplate::Bold,
            _ => OgTemplate::Default,
        };
    }
    let data = crate::data::load_data_dir(&paths.data_dir)
        .unwrap_or_else(|_| Value::Object(Default::default()));
    let plan = match OgPlan::new(&config, paths, &data) {
        Ok(Some(plan)) => plan,
        Ok(None) => anyhow::bail!("social cards are off"),
        Err(diags) => {
            return Err(crate::error::PageError::Diagnostics(diags.relative_to(&state.cwd)).into())
        }
    };

    // The site card for `/` and `/<lang>/`.
    let raw = args.target.trim();
    let site_lang = config.all_languages().into_iter().find(|lang| {
        raw == "/"
            || (lang != &config.site.language
                && raw.trim_matches('/') == lang.as_str()
                && raw.starts_with('/'))
    });
    let png;
    let described;
    if let Some(lang) = site_lang {
        let lang = if raw == "/" {
            config.site.language.clone()
        } else {
            lang
        };
        png = plan.preview(&config, None, &lang)?;
        described = format!("site card ({lang})");
    } else {
        let cwd = std::env::current_dir()?;
        let arguments = match classify(raw, &cwd) {
            Target::Path(p) => json!({ "path": p }),
            Target::Url(u) => json!({ "url": u }),
        };
        let page = call_tool(&state, "seite_get_page", arguments)?;
        let collection_name = str_of(&page, "collection");
        let collection = config
            .collections
            .iter()
            .find(|c| c.name == collection_name)
            .ok_or_else(|| anyhow::anyhow!("unknown collection '{collection_name}'"))?;
        let title = page["frontmatter"]["title"].as_str().unwrap_or_default();
        let date = page["date"].as_str().and_then(|d| {
            chrono::NaiveDate::parse_from_str(d.get(..10).unwrap_or(d), "%Y-%m-%d").ok()
        });
        let reading_time = page["reading_time"].as_u64().unwrap_or(0) as usize;
        let lang = str_of(&page, "lang");
        let lang = if lang.is_empty() {
            config.site.language.as_str()
        } else {
            lang
        };
        png = plan.preview(
            &config,
            Some(PreviewPage {
                collection,
                title,
                date,
                reading_time,
            }),
            lang,
        )?;
        described = str_of(&page, "path").to_string();
    }

    let out = match &args.out {
        Some(out) => {
            if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(out, &png)?;
            out.clone()
        }
        None => {
            let out = state.cwd.join(".seite/cache/og-preview.png");
            crate::safe_fs::write(&state.cwd, &out, &png)?;
            out
        }
    };
    human::success(&format!(
        "Social card for {described} → {} (1200×630, {} template)",
        out.display(),
        config
            .og
            .as_ref()
            .map(|o| o.template.as_str())
            .unwrap_or("default")
    ));
    json_out::set_data(json!({
        "target": args.target,
        "out": out.display().to_string(),
        "width": crate::build::og::card::WIDTH,
        "height": crate::build::og::card::HEIGHT,
        "bytes": png.len(),
    }));
    Ok(())
}
