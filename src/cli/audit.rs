//! `seite audit`: SEO, GEO (AI visibility), accessibility, content and
//! performance findings for the built site (see [`crate::audit`]). Same
//! report as the MCP `seite_audit` tool.

use std::path::PathBuf;

use clap::Args;
use console::style;

use super::site_tools::load_site;
use crate::audit::{self, Category, Finding, Report, Severity};
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct AuditArgs {
    /// Audit this existing output directory (e.g. `dist`) instead of a
    /// fresh scratch build. Findings then point at the generated files.
    #[arg(long, value_name = "DIR")]
    pub dist: Option<PathBuf>,

    /// Only these categories (repeatable or comma-separated)
    #[arg(long, value_enum, value_delimiter = ',')]
    pub category: Vec<Category>,

    /// Exit with status 1 when there is a finding of this severity (or
    /// worse) — for CI. Default: report only, exit 0.
    #[arg(long, value_enum, value_name = "SEVERITY")]
    pub fail_on: Option<FailOn>,

    /// Findings listed per category in the human report (`--json` has all)
    #[arg(long, default_value_t = 10, value_name = "N")]
    pub top: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FailOn {
    Error,
    Warning,
}

pub fn run(args: &AuditArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let cwd = std::env::current_dir()?;
    let dist = args.dist.as_ref().map(|d| cwd.join(d));
    let report = audit::run(&audit::AuditOptions {
        root: &state.cwd,
        dist: dist.as_deref(),
        categories: &args.category,
    })?;
    print_report(&report, args.top);
    let failed = match args.fail_on {
        None => false,
        Some(FailOn::Error) => report.summary.errors > 0,
        Some(FailOn::Warning) => report.summary.errors + report.summary.warnings > 0,
    };
    json_out::set_data(serde_json::to_value(&report)?);
    if failed {
        anyhow::bail!(
            "audit failed (--fail-on {}): {} error(s), {} warning(s)",
            match args.fail_on {
                Some(FailOn::Warning) => "warning",
                _ => "error",
            },
            report.summary.errors,
            report.summary.warnings
        );
    }
    Ok(())
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn counts(errors: usize, warnings: usize, notices: usize) -> String {
    format!(
        "{}, {}, {}",
        plural(errors, "error"),
        plural(warnings, "warning"),
        plural(notices, "notice")
    )
}

fn print_finding(f: &Finding) {
    let label = match f.severity {
        Severity::Error => style("error").red().bold(),
        Severity::Warning => style("warning").yellow().bold(),
        Severity::Notice => style("notice").cyan(),
    };
    let page = f.page_url.as_deref().unwrap_or("(site)");
    human_println!("  {label}[{}] {page}: {}", f.code, f.message);
    let mut at = f.file.clone().unwrap_or_default();
    if let Some(line) = f.line {
        at.push_str(&format!(":{line}"));
    }
    if !at.is_empty() {
        human_println!("      {} {at}", style("-->").dim());
    }
    if !f.hint.is_empty() {
        human_println!("      {} {}", style("fix:").dim(), f.hint);
    }
}

fn print_report(report: &Report, top: usize) {
    let s = &report.summary;
    human::header(&format!(
        "Audit: score {}/100 — {} ({})",
        s.score,
        counts(s.errors, s.warnings, s.notices),
        plural(s.pages, "page")
    ));
    for category in Category::ALL {
        let Some(c) = s.by_category.get(category.as_str()) else {
            continue;
        };
        human_println!();
        human_println!(
            "{} — {}",
            style(category.label()).bold(),
            counts(c.errors, c.warnings, c.notices)
        );
        let mut found: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.category == category)
            .collect();
        found.sort_by_key(|f| f.severity);
        for f in found.iter().take(top) {
            print_finding(f);
        }
        if found.len() > top {
            human_println!(
                "  … and {} more (`seite audit --json` lists all)",
                found.len() - top
            );
        }
    }
    human_println!();
    if report.findings.is_empty() {
        human::success("No findings");
    } else {
        human::info(
            "Fix in the files shown, then re-run `seite audit`. `seite explain <code>` explains a code.",
        );
    }
}
