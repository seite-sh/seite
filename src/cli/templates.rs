//! `seite templates` — everything to know before editing templates: user
//! templates and the defaults they override, base-template blocks, context
//! variables per template kind, shortcodes and data keys. Same results as the
//! MCP `seite_list_templates` tool.

use clap::Args;
use console::style;
use serde_json::{json, Value};

use super::site_tools::{array_of, call_tool, load_site, str_of};
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct TemplatesArgs {}

pub fn run(_args: &TemplatesArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let result = call_tool(&state, "seite_list_templates", json!({}))?;
    print_templates(&result);
    json_out::set_data(result);
    Ok(())
}

fn names(items: &[Value], key: &str) -> String {
    items
        .iter()
        .map(|v| match v.as_str() {
            Some(s) => s.to_string(),
            None => str_of(v, key).to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_templates(result: &Value) {
    human_println!(
        "{} {}   {} {}",
        style("Template dir:").bold(),
        str_of(result, "template_dir"),
        style("Active theme:").bold(),
        str_of(result, "active_theme")
    );

    if let Some(base) = result.get("base_template") {
        let source = match base.get("path").and_then(Value::as_str) {
            Some(path) => format!("{} ({path})", str_of(base, "source")),
            None => str_of(base, "source").to_string(),
        };
        human_println!(
            "{} {source}; blocks: {}",
            style("Base template:").bold(),
            names(array_of(base, "blocks"), "name")
        );
        if let Some(error) = base.get("error").and_then(Value::as_str) {
            human::warning(&format!("base template: {error}"));
        }
    }

    human::header("User templates");
    let user = array_of(result, "user_templates");
    if user.is_empty() {
        human_println!("  (none: every page uses the embedded defaults)");
    }
    for t in user {
        let overrides = if t.get("overrides_default").and_then(Value::as_bool) == Some(true) {
            ", overrides default"
        } else {
            ""
        };
        human_println!(
            "  {}  [{}{overrides}]",
            str_of(t, "path"),
            str_of(t, "kind")
        );
    }

    human::header("Context variables");
    for kind in array_of(result, "context") {
        human_println!(
            "  {} ({}): {}",
            style(str_of(kind, "kind")).bold(),
            names(array_of(kind, "templates"), ""),
            names(array_of(kind, "variables"), "name")
        );
    }

    human::header("Shortcodes");
    for sc in array_of(result, "shortcodes") {
        human_println!(
            "  {}  [{}, {}]",
            str_of(sc, "usage"),
            str_of(sc, "source"),
            str_of(sc, "syntax")
        );
    }

    if let Some(data) = result.get("data") {
        human::header(&format!("Data ({})", str_of(data, "dir")));
        let keys = array_of(data, "keys");
        if keys.is_empty() {
            human_println!("  (no data files)");
        }
        for k in keys {
            let nested = names(array_of(k, "keys"), "");
            human_println!(
                "  data.{}  [{}]{}",
                str_of(k, "key"),
                str_of(k, "type"),
                if nested.is_empty() {
                    String::new()
                } else {
                    format!(": {nested}")
                }
            );
        }
        if let Some(error) = data.get("error").and_then(Value::as_str) {
            human::warning(&format!("data: {error}"));
        }
    }

    for w in array_of(result, "warnings") {
        human::warning(w.as_str().unwrap_or(""));
    }
}
