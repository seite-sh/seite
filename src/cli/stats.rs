//! `seite stats` — content health per collection. Same results as the MCP
//! `seite_content_stats` tool.

use clap::Args;
use console::style;
use serde_json::{json, Value};

use super::site_tools::{array_of, call_tool, load_site, str_of};
use crate::human_println;
use crate::output::{human, json as json_out};

/// How many paths to print per list in human output (JSON has them all, up
/// to the tool's own cap).
const LIST_PREVIEW: usize = 5;

#[derive(Args)]
pub struct StatsArgs {
    /// Only report this collection (singular aliases like `post` work)
    #[arg(long)]
    pub collection: Option<String>,
}

pub fn run(args: &StatsArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let mut arguments = json!({});
    if let Some(collection) = &args.collection {
        arguments["collection"] = json!(collection);
    }
    let result = call_tool(&state, "seite_content_stats", arguments)?;
    print_stats(&result);
    json_out::set_data(result);
    Ok(())
}

fn count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn print_list(label: &str, items: &[Value], render: impl Fn(&Value) -> String) {
    if items.is_empty() {
        return;
    }
    human_println!("  {label} ({}):", items.len());
    for item in items.iter().take(LIST_PREVIEW) {
        human_println!("    {}", render(item));
    }
    if items.len() > LIST_PREVIEW {
        human_println!(
            "    … {} more (seite stats --json lists them)",
            items.len() - LIST_PREVIEW
        );
    }
}

fn print_stats(result: &Value) {
    for c in array_of(result, "collections") {
        human::header(str_of(c, "name"));
        human_println!(
            "  {} items, {} drafts",
            count(c, "total"),
            count(c, "drafts")
        );
        let path = |v: &Value| v.as_str().unwrap_or("").to_string();
        print_list(
            "missing description",
            array_of(c, "missing_description"),
            path,
        );
        print_list("missing tags", array_of(c, "missing_tags"), path);
        print_list("future-dated", array_of(c, "future_dated"), |v| {
            format!("{} ({})", str_of(v, "path"), str_of(v, "date"))
        });
        print_list("expired", array_of(c, "expired"), |v| {
            format!(
                "{} (expired after {})",
                str_of(v, "path"),
                str_of(v, "date")
            )
        });
        print_list("parse errors", array_of(c, "parse_errors"), |v| {
            format!("{}: {}", str_of(v, "path"), str_of(v, "error"))
        });
        if let Some(missing) = c.get("missing_translations").and_then(Value::as_object) {
            for (lang, paths) in missing {
                let paths = paths.as_array().map(Vec::as_slice).unwrap_or(&[]);
                print_list(&format!("missing {lang} translation"), paths, path);
            }
        }
    }
    for tax in array_of(result, "taxonomies") {
        let terms = array_of(tax, "terms");
        if terms.is_empty() {
            continue;
        }
        let top: Vec<String> = terms
            .iter()
            .take(LIST_PREVIEW)
            .map(|t| format!("{} ({})", str_of(t, "name"), count(t, "count")))
            .collect();
        human_println!(
            "{} {} terms: {}",
            style(format!("{}:", str_of(tax, "label"))).bold(),
            terms.len(),
            top.join(", ")
        );
    }
    let series = array_of(result, "series");
    if !series.is_empty() {
        let names: Vec<String> = series
            .iter()
            .map(|s| format!("{} ({} parts)", str_of(s, "name"), count(s, "parts")))
            .collect();
        human_println!("{} {}", style("Series:").bold(), names.join(", "));
    }
    if let Some(totals) = result.get("totals") {
        human_println!();
        human_println!(
            "{} {} items, {} drafts, {} missing a description, {} missing tags, {} future-dated, {} expired, {} unparseable, {} missing translations",
            style("Total:").bold(),
            count(totals, "items"),
            count(totals, "drafts"),
            count(totals, "missing_description"),
            count(totals, "missing_tags"),
            count(totals, "future_dated"),
            count(totals, "expired"),
            count(totals, "parse_errors"),
            count(totals, "missing_translations"),
        );
    }
    if let Some(next) = result.get("next_scheduled") {
        human_println!(
            "{} {} — {} ({})",
            style("Next scheduled:").bold(),
            str_of(next, "date"),
            str_of(next, "title"),
            str_of(next, "path")
        );
    }
}
