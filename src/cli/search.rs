//! `seite search <query>` — keyword search over the site's content (drafts
//! included). Same results as the MCP `seite_search` tool.

use clap::Args;
use console::style;
use serde_json::{json, Value};

use super::site_tools::{array_of, call_tool, load_site, plural, str_of};
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct SearchArgs {
    /// Keywords (case-insensitive) matched against titles, descriptions,
    /// tags and body text; ranked title > description/tags > body
    pub query: String,

    /// Only search this collection (singular aliases like `post` work)
    #[arg(long)]
    pub collection: Option<String>,

    /// Maximum results (1-100)
    #[arg(long, default_value_t = 20)]
    pub limit: i64,
}

pub fn run(args: &SearchArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let mut arguments = json!({ "query": args.query, "limit": args.limit });
    if let Some(collection) = &args.collection {
        arguments["collection"] = json!(collection);
    }
    let result = call_tool(&state, "seite_search", arguments)?;
    print_results(&result);
    json_out::set_data(result);
    Ok(())
}

fn print_results(result: &Value) {
    let total = result.get("total").and_then(Value::as_u64).unwrap_or(0);
    let results = array_of(result, "results");
    if total == 0 {
        human::info(&format!("No matches for '{}'", str_of(result, "query")));
    } else {
        human::info(&format!(
            "{} for '{}'",
            plural(total, "match", "matches"),
            str_of(result, "query")
        ));
    }
    for item in results {
        let mut flags = vec![str_of(item, "collection").to_string()];
        if item.get("draft").and_then(Value::as_bool) == Some(true) {
            flags.push("draft".into());
        }
        let lang = str_of(item, "lang");
        if !lang.is_empty() {
            flags.push(lang.to_string());
        }
        human_println!(
            "{}  {}  [{}]",
            style(str_of(item, "title")).bold(),
            str_of(item, "url"),
            flags.join(", ")
        );
        human_println!("  {}", style(str_of(item, "path")).dim());
        let excerpt = str_of(item, "excerpt").trim();
        if !excerpt.is_empty() {
            human_println!("  {}", excerpt.replace('\n', " "));
        }
    }
    if let Some(note) = result.get("note").and_then(Value::as_str) {
        human::info(&note.replace("'limit'", "--limit"));
    }
    for e in array_of(result, "parse_errors") {
        human::warning(&format!(
            "{}: {} (not searched)",
            str_of(e, "path"),
            str_of(e, "error")
        ));
    }
}
