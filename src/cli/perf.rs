use clap::Args;
use console::style;

use crate::config::SiteConfig;
use crate::output::human;

#[derive(Args)]
pub struct PerfArgs {
    /// URL to audit (defaults to base_url from seite.toml)
    pub url: Option<String>,

    /// Strategy: mobile or desktop
    #[arg(long, default_value = "mobile")]
    pub strategy: String,

    /// PageSpeed Insights API key (optional, increases rate limit)
    #[arg(long)]
    pub key: Option<String>,
}

pub fn run(args: &PerfArgs) -> anyhow::Result<()> {
    let url = resolve_url(args.url.as_deref())?;
    if is_local_url(&url) {
        anyhow::bail!(
            "PageSpeed Insights requires a publicly accessible URL. Got: {url}\n\
             Deploy first, then run: seite perf https://your-site.com"
        );
    }
    human::info(&format!("Auditing {} ({})...", url, args.strategy));
    let report = fetch_psi(&url, &args.strategy, args.key.as_deref())?;
    print_report(&report);
    Ok(())
}

/// Called automatically after a production deploy. Non-fatal — only prints if URL is public.
pub fn run_for_deploy(url: &str) {
    let url = url.trim_end_matches('/');
    if is_local_url(url) {
        return;
    }
    human::info(&format!("Running PageSpeed audit on {} (mobile)...", url));
    match fetch_psi(url, "mobile", None) {
        Ok(report) => print_report(&report),
        Err(e) => human::warning(&format!("PageSpeed check skipped: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

struct PerfReport {
    performance_score: Option<f64>,
    fcp: Option<Metric>,
    lcp: Option<Metric>,
    tbt: Option<Metric>,
    cls: Option<Metric>,
    speed_index: Option<Metric>,
}

struct Metric {
    display: String,
    score: f64,
}

fn resolve_url(explicit: Option<&str>) -> anyhow::Result<String> {
    if let Some(u) = explicit {
        return Ok(u.trim_end_matches('/').to_string());
    }
    let config = SiteConfig::load(&std::path::PathBuf::from("seite.toml"))?;
    Ok(config.site.base_url.trim_end_matches('/').to_string())
}

fn fetch_psi(url: &str, strategy: &str, key: Option<&str>) -> anyhow::Result<PerfReport> {
    let mut api_url = format!(
        "https://www.googleapis.com/pagespeedonline/v5/runPagespeed\
         ?url={url}&strategy={strategy}&category=performance"
    );
    if let Some(k) = key {
        api_url.push_str(&format!("&key={k}"));
    }

    let mut response = ureq::get(&api_url)
        .call()
        .map_err(|e| anyhow::anyhow!("PageSpeed API request failed: {e}"))?;

    let body: serde_json::Value = response
        .body_mut()
        .read_json()
        .map_err(|e| anyhow::anyhow!("failed to parse PageSpeed API response: {e}"))?;

    let lr = &body["lighthouseResult"];

    let performance_score = lr["categories"]["performance"]["score"]
        .as_f64()
        .map(|s| (s * 100.0).round());

    let audits = &lr["audits"];

    let extract = |key: &str| -> Option<Metric> {
        let audit = &audits[key];
        Some(Metric {
            display: audit["displayValue"].as_str()?.to_string(),
            score: audit["score"].as_f64()?,
        })
    };

    Ok(PerfReport {
        performance_score,
        fcp: extract("first-contentful-paint"),
        lcp: extract("largest-contentful-paint"),
        tbt: extract("total-blocking-time"),
        cls: extract("cumulative-layout-shift"),
        speed_index: extract("speed-index"),
    })
}

fn print_report(report: &PerfReport) {
    let score_label = match report.performance_score {
        Some(s) if s >= 90.0 => format!("{}", style(format!("{s:.0}/100")).green().bold()),
        Some(s) if s >= 50.0 => format!("{}", style(format!("{s:.0}/100")).yellow().bold()),
        Some(s) => format!("{}", style(format!("{s:.0}/100")).red().bold()),
        None => "N/A".to_string(),
    };
    println!("\n{} Performance Score: {score_label}", style("●").bold());
    print_metric("  FCP (First Contentful Paint)", &report.fcp);
    print_metric("  LCP (Largest Contentful Paint)", &report.lcp);
    print_metric("  TBT (Total Blocking Time)    ", &report.tbt);
    print_metric("  CLS (Cumulative Layout Shift)", &report.cls);
    print_metric("  Speed Index                  ", &report.speed_index);
    println!();
}

fn print_metric(label: &str, metric: &Option<Metric>) {
    match metric {
        None => println!("{label}: N/A"),
        Some(m) => {
            let val = if m.score >= 0.9 {
                format!("{}", style(&m.display).green())
            } else if m.score >= 0.5 {
                format!("{}", style(&m.display).yellow())
            } else {
                format!("{}", style(&m.display).red())
            };
            println!("{label}: {val}");
        }
    }
}

/// Returns true if the URL should be skipped (localhost / loopback).
fn is_local_url(url: &str) -> bool {
    url.contains("localhost") || url.contains("127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_url_explicit_strips_trailing_slash() {
        let url = resolve_url(Some("https://example.com/")).unwrap();
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn test_resolve_url_explicit_no_slash() {
        let url = resolve_url(Some("https://seite.sh")).unwrap();
        assert_eq!(url, "https://seite.sh");
    }

    #[test]
    fn test_is_local_url_localhost() {
        assert!(is_local_url("http://localhost:3000"));
        assert!(is_local_url("http://localhost"));
    }

    #[test]
    fn test_is_local_url_loopback() {
        assert!(is_local_url("http://127.0.0.1:3000"));
    }

    #[test]
    fn test_is_local_url_public() {
        assert!(!is_local_url("https://seite.sh"));
        assert!(!is_local_url("https://example.com"));
    }

    #[test]
    fn test_run_for_deploy_skips_localhost() {
        // Should return without panicking — no HTTP call made
        run_for_deploy("http://localhost:3000");
        run_for_deploy("http://127.0.0.1:8080/");
    }
}
