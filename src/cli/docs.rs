//! `seite docs [query] [--topic slug]` — the embedded documentation, from the
//! shell. Same results as the MCP `seite_lookup_docs` tool.

use clap::Args;
use console::style;
use serde_json::{json, Value};

use super::site_tools::{array_of, call_tool, load_state, plural, str_of};
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct DocsArgs {
    /// Search the docs for these keywords (omit to list every topic)
    pub query: Option<String>,

    /// Show one topic by its slug (e.g. configuration, templates, errors)
    #[arg(long, value_name = "SLUG")]
    pub topic: Option<String>,
}

pub fn run(args: &DocsArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_state(site_filter)?;
    crate::cli::harness::ensure_local_docs(&state.cwd);
    let mut arguments = serde_json::Map::new();
    if let Some(query) = &args.query {
        arguments.insert("query".into(), json!(query));
    }
    if let Some(topic) = &args.topic {
        arguments.insert("topic".into(), json!(topic));
    }
    let result = call_tool(&state, "seite_lookup_docs", Value::Object(arguments))?;

    if result.get("found").is_some() {
        print_topic(&result, &state.cwd);
    } else if result.get("available_topics").is_some() {
        print_index(&result);
    } else {
        print_search(&result);
    }
    json_out::set_data(result);
    Ok(())
}

fn print_index(result: &Value) {
    human::header("seite documentation");
    let topics = array_of(result, "available_topics");
    let width = topics
        .iter()
        .map(|t| str_of(t, "slug").len())
        .max()
        .unwrap_or(0);
    for topic in topics {
        human_println!(
            "  {:width$}  {} — {}",
            str_of(topic, "slug"),
            style(str_of(topic, "title")).bold(),
            str_of(topic, "description"),
        );
    }
    human_println!();
    human_println!("Read a topic: seite docs --topic <slug>    Search: seite docs <query>");
}

fn print_topic(result: &Value, root: &std::path::Path) {
    let slug = str_of(result, "topic");
    human_println!("# {}", str_of(result, "title"));
    human_println!();
    human_println!("{}", str_of(result, "content").trim_end());
    if result.get("truncated").and_then(Value::as_bool) == Some(true) {
        let local = root.join(".seite/docs").join(format!("{slug}.md"));
        let full = if local.is_file() {
            format!(".seite/docs/{slug}.md")
        } else {
            format!("https://seite.sh/docs/{slug}")
        };
        human_println!();
        human::info(&format!(
            "Truncated. Full page: {full} (or search it: seite docs <query>)"
        ));
    }
}

fn print_search(result: &Value) {
    let results = array_of(result, "results");
    let count = result.get("count").and_then(Value::as_u64).unwrap_or(0);
    human::info(&format!(
        "{} for '{}'",
        plural(count, "matching topic", "matching topics"),
        str_of(result, "query")
    ));
    for item in results {
        human::header(&format!(
            "{} ({})",
            str_of(item, "title"),
            str_of(item, "slug")
        ));
        for section in array_of(item, "matched_sections") {
            human_println!("{}", section.as_str().unwrap_or("").trim_end());
            human_println!();
        }
    }
    let omitted = array_of(result, "omitted_topics");
    if !omitted.is_empty() {
        let names: Vec<&str> = omitted.iter().filter_map(Value::as_str).collect();
        human::info(&format!(
            "Also matching (output capped): {}. Read one with seite docs --topic <slug>.",
            names.join(", ")
        ));
    }
    if results.is_empty() {
        human_println!("Try other keywords, or list topics with `seite docs`.");
    }
}
