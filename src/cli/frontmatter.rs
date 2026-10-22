//! `seite frontmatter set|unset` — edit top-level frontmatter keys of a
//! content file, preserving the body byte-for-byte and the formatting of
//! untouched keys. Same behavior as the MCP `seite_update_frontmatter` tool.

use std::path::Path;

use anyhow::{anyhow, bail};
use clap::{Args, Subcommand};
use serde_json::{json, Value};

use super::site_tools::{array_of, call_tool, load_site, str_of};
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct FrontmatterArgs {
    #[command(subcommand)]
    pub command: FrontmatterCommand,
}

#[derive(Subcommand)]
pub enum FrontmatterCommand {
    /// Add or replace keys: `seite frontmatter set content/posts/a.md draft=false tags=[rust,web]`
    ///
    /// Values are YAML scalars or flow collections: true/false, numbers,
    /// null, [a, b], {k: v}, or quoted strings. Anything else is a plain
    /// string, taken verbatim (`title=Hello: world` sets that exact title).
    Set {
        /// Content file (relative to the current directory, the site root or content/)
        path: String,
        /// One or more key=value pairs
        #[arg(required = true, value_name = "KEY=VALUE")]
        pairs: Vec<String>,
    },
    /// Remove keys: `seite frontmatter unset content/posts/a.md image weight`
    Unset {
        /// Content file (relative to the current directory, the site root or content/)
        path: String,
        /// One or more keys (`title` cannot be removed)
        #[arg(required = true, value_name = "KEY")]
        keys: Vec<String>,
    },
}

pub fn run(args: &FrontmatterArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let cwd = std::env::current_dir()?;
    let arguments = match &args.command {
        FrontmatterCommand::Set { path, pairs } => {
            let mut set = serde_json::Map::new();
            for pair in pairs {
                let (key, value) = parse_pair(pair)?;
                set.insert(key, value);
            }
            json!({ "path": resolve_path(path, &cwd), "set": set })
        }
        FrontmatterCommand::Unset { path, keys } => {
            json!({ "path": resolve_path(path, &cwd), "unset": keys })
        }
    };
    let result = call_tool(&state, "seite_update_frontmatter", arguments)?;
    print_result(&result);
    json_out::set_data(result);
    Ok(())
}

/// A path that exists relative to the current directory becomes absolute
/// (so it works from a subdirectory of the site); anything else is passed
/// through for the tool to resolve against the site root / content dir.
fn resolve_path(raw: &str, cwd: &Path) -> String {
    let on_disk = cwd.join(raw);
    if on_disk.is_file() {
        let abs = on_disk.canonicalize().unwrap_or(on_disk);
        return abs.to_string_lossy().into_owned();
    }
    raw.to_string()
}

/// Split `key=value` (at the first `=`) and parse the value.
fn parse_pair(pair: &str) -> anyhow::Result<(String, Value)> {
    let (key, raw) = pair
        .split_once('=')
        .ok_or_else(|| anyhow!("expected KEY=VALUE, got '{pair}'"))?;
    let key = key.trim();
    if key.is_empty() {
        bail!("missing key in '{pair}' (expected KEY=VALUE)");
    }
    Ok((
        key.to_string(),
        parse_value(raw).map_err(|e| anyhow!("{key}: {e}"))?,
    ))
}

/// Parse a command-line frontmatter value.
///
/// - flow collections (`[a, b]`, `{k: v}`) and quoted strings are parsed as YAML
/// - bare scalars become booleans, numbers or null when YAML reads them so
/// - everything else (including text YAML would read as a mapping, like
///   `Note: this`) is the literal string; an empty value is `""`
fn parse_value(raw: &str) -> anyhow::Result<Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Value::String(String::new()));
    }
    let first = trimmed.as_bytes()[0];
    if matches!(first, b'[' | b'{' | b'"' | b'\'') {
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(trimmed)
            .map_err(|e| anyhow!("invalid YAML value '{trimmed}': {e}"))?;
        return serde_json::to_value(yaml)
            .map_err(|e| anyhow!("unsupported value '{trimmed}': {e}"));
    }
    use serde_yaml_ng::Value as Yaml;
    Ok(match serde_yaml_ng::from_str::<Yaml>(trimmed) {
        Ok(yaml @ (Yaml::Bool(_) | Yaml::Number(_) | Yaml::Null)) => {
            serde_json::to_value(yaml).unwrap_or_else(|_| Value::String(raw.to_string()))
        }
        _ => Value::String(raw.to_string()),
    })
}

fn print_result(result: &Value) {
    let path = str_of(result, "path");
    if result.get("changed").and_then(Value::as_bool) == Some(true) {
        human::success(&format!("Updated frontmatter of {path}"));
    } else {
        human::info(&format!(
            "{path} already has these values (nothing written)"
        ));
    }
    for note in array_of(result, "notes") {
        human::info(note.as_str().unwrap_or(""));
    }
    if let Some(fm) = result.get("frontmatter") {
        if let Ok(yaml) = serde_yaml_ng::to_string(fm) {
            human_println!("---\n{}---", yaml);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_value_scalars() {
        assert_eq!(parse_value("true").unwrap(), json!(true));
        assert_eq!(parse_value("false").unwrap(), json!(false));
        assert_eq!(parse_value("3").unwrap(), json!(3));
        assert_eq!(parse_value("2.5").unwrap(), json!(2.5));
        assert_eq!(parse_value("null").unwrap(), Value::Null);
        assert_eq!(parse_value("~").unwrap(), Value::Null);
    }

    #[test]
    fn test_parse_value_collections() {
        assert_eq!(parse_value("[a, b]").unwrap(), json!(["a", "b"]));
        assert_eq!(parse_value("[]").unwrap(), json!([]));
        assert_eq!(parse_value("{k: 1}").unwrap(), json!({ "k": 1 }));
        assert!(parse_value("[unclosed").is_err());
    }

    #[test]
    fn test_parse_value_strings_stay_verbatim() {
        assert_eq!(parse_value("Hello world").unwrap(), json!("Hello world"));
        // YAML would read these as a mapping / sequence / date.
        assert_eq!(parse_value("Note: this").unwrap(), json!("Note: this"));
        assert_eq!(parse_value("- item").unwrap(), json!("- item"));
        assert_eq!(parse_value("2026-01-02").unwrap(), json!("2026-01-02"));
        assert_eq!(parse_value("1.2.3").unwrap(), json!("1.2.3"));
        assert_eq!(parse_value("").unwrap(), json!(""));
        assert_eq!(parse_value("a # b").unwrap(), json!("a # b"));
    }

    #[test]
    fn test_parse_value_quoted_strings() {
        assert_eq!(parse_value("\"true\"").unwrap(), json!("true"));
        assert_eq!(parse_value("'42'").unwrap(), json!("42"));
    }

    #[test]
    fn test_parse_pair() {
        assert_eq!(
            parse_pair("title=a=b").unwrap(),
            ("title".to_string(), json!("a=b"))
        );
        assert_eq!(
            parse_pair("draft=true").unwrap(),
            ("draft".to_string(), json!(true))
        );
        assert!(parse_pair("noequals").is_err());
        assert!(parse_pair("=x").is_err());
    }
}
