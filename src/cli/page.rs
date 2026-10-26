//! `seite page <path|url>` — one content file as the build sees it: resolved
//! frontmatter, URL, output file and rendered body. Same results as the MCP
//! `seite_get_page` tool.

use std::path::Path;

use clap::Args;
use console::style;
use serde_json::{json, Value};

use super::site_tools::{call_tool, load_site, str_of};
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct PageArgs {
    /// A content file (content/posts/hello.md, posts/hello.md) or a published
    /// URL path (/posts/hello)
    pub target: String,

    /// Also print the rendered body HTML (markdown + shortcodes, no page template)
    #[arg(long)]
    pub html: bool,
}

/// What the user passed: a source file or a published URL.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Path(String),
    Url(String),
}

/// Source path when it exists on disk (relative to the current directory)
/// or ends in `.md`; URL when it starts with `/` or `http(s)://`; otherwise
/// a path (the tool resolves it against the site and content dirs).
pub(crate) fn classify(raw: &str, cwd: &Path) -> Target {
    let on_disk = cwd.join(raw);
    if on_disk.is_file() {
        // Absolute, so it also works from a subdirectory of the site.
        let abs = on_disk.canonicalize().unwrap_or(on_disk);
        return Target::Path(abs.to_string_lossy().into_owned());
    }
    if raw.ends_with(".md") {
        return Target::Path(raw.to_string());
    }
    if raw.starts_with('/') || raw.starts_with("http://") || raw.starts_with("https://") {
        return Target::Url(raw.to_string());
    }
    Target::Path(raw.to_string())
}

pub fn run(args: &PageArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_site(site_filter)?;
    let cwd = std::env::current_dir()?;
    let arguments = match classify(&args.target, &cwd) {
        Target::Path(p) => json!({ "path": p }),
        Target::Url(u) => json!({ "url": u }),
    };
    let result = call_tool(&state, "seite_get_page", arguments)?;
    print_page(&result, args.html);
    json_out::set_data(result);
    Ok(())
}

fn print_page(result: &Value, with_html: bool) {
    let fm = result.get("frontmatter").cloned().unwrap_or(json!({}));
    human_println!(
        "{}  [{}{}]",
        style(str_of(result, "path")).bold(),
        str_of(result, "collection"),
        if result.get("draft").and_then(Value::as_bool) == Some(true) {
            ", draft"
        } else {
            ""
        }
    );
    human_println!("  title:       {}", str_of(&fm, "title"));
    if let Some(d) = fm.get("description").and_then(Value::as_str) {
        human_println!("  description: {d}");
    }
    human_println!("  url:         {}", str_of(result, "url"));
    human_println!("  lang:        {}", str_of(result, "lang"));
    if let Some(date) = result.get("date").and_then(Value::as_str) {
        human_println!("  date:        {date}");
    }
    if let Some(tags) = fm.get("tags").and_then(Value::as_array) {
        if !tags.is_empty() {
            let tags: Vec<&str> = tags.iter().filter_map(Value::as_str).collect();
            human_println!("  tags:        {}", tags.join(", "));
        }
    }
    human_println!(
        "  words:       {} (~{} min read)",
        result
            .get("word_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        result
            .get("reading_time")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    );
    match result.get("output_path").and_then(Value::as_str) {
        Some(out) => human_println!("  output:      {out}"),
        None => human_println!("  output:      (not built yet; run `seite build`)"),
    }
    match result.get("html").and_then(Value::as_str) {
        Some(html) if with_html => {
            human_println!();
            // The page body itself is the command's output.
            human_println!("{html}");
        }
        Some(html) => human_println!(
            "  html:        {} bytes rendered (--html prints it, --json includes it)",
            html.len()
        ),
        None => {}
    }
    if let Some(err) = result.get("render_error").and_then(Value::as_str) {
        human::warning(&format!("render failed: {err}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_targets() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("exists.txt"), "x").unwrap();
        assert!(matches!(
            classify("exists.txt", tmp.path()),
            Target::Path(p) if p.ends_with("exists.txt") && Path::new(&p).is_absolute()
        ));
        assert_eq!(
            classify("posts/missing.md", tmp.path()),
            Target::Path("posts/missing.md".into())
        );
        assert_eq!(
            classify("/posts/hello", tmp.path()),
            Target::Url("/posts/hello".into())
        );
        assert_eq!(
            classify("https://example.com/a", tmp.path()),
            Target::Url("https://example.com/a".into())
        );
        assert_eq!(
            classify("posts/hello", tmp.path()),
            Target::Path("posts/hello".into())
        );
    }
}
