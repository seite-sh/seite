//! `seite preview` — what the running dev server knows, for agents with a
//! shell: its URL, the last build outcome (compiler-style diagnostics, like
//! `seite check`) and errors reported by open preview pages.
//!
//! Reads `/__seite/status` of the server recorded in `.seite/dev.json` (site
//! root, then workspace root), trusting only a live pid and a loopback port.

use std::time::Duration;

use clap::Args;
use serde_json::{json, Value};

use super::site_tools::{array_of, site_root, str_of};
use crate::human_println;
use crate::output::json as json_out;
use crate::server::remote;

const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Args)]
pub struct PreviewArgs {
    /// Forget the recorded browser errors after printing them (so the next
    /// `seite preview` shows only new ones)
    #[arg(long)]
    pub clear_browser_errors: bool,
}

pub fn run(args: &PreviewArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let root = site_root(site_filter)?;
    let (server, mut status) = match remote::find_server(&root, TIMEOUT) {
        Ok(found) => found,
        Err(message) => {
            json_out::set_data(json!({ "running": false }));
            anyhow::bail!("{message}");
        }
    };
    print_status(&status);
    if args.clear_browser_errors {
        let (code, reply) = remote::post_json(
            server.port,
            "/__seite/clear-browser-errors",
            &json!({}),
            TIMEOUT,
        )
        .map_err(anyhow::Error::msg)?;
        if code != 200 {
            anyhow::bail!("could not clear browser errors (HTTP {code})");
        }
        let cleared = reply
            .as_ref()
            .and_then(|r| r.get("cleared"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        human_println!("Cleared {cleared} browser error(s).");
        status["cleared_browser_errors"] = cleared.into();
    }
    status["running"] = true.into();
    json_out::set_data(status);
    Ok(())
}

/// `file:line:col: severity[code]: message` + `  hint:` (as `seite check`).
pub fn format_diagnostic(d: &Value) -> String {
    let mut out = String::new();
    if let Some(file) = d.get("file").and_then(Value::as_str) {
        out.push_str(file);
        if let Some(line) = d.get("line").and_then(Value::as_u64) {
            out.push_str(&format!(":{line}"));
            if let Some(col) = d.get("column").and_then(Value::as_u64) {
                out.push_str(&format!(":{col}"));
            }
        }
        out.push_str(": ");
    }
    out.push_str(&format!(
        "{}[{}]: {}",
        str_of(d, "severity"),
        str_of(d, "code"),
        str_of(d, "message")
    ));
    if let Some(hint) = d.get("hint").and_then(Value::as_str) {
        out.push_str(&format!("\n  hint: {hint}"));
    }
    out
}

/// One-line summary of a `last_build` object (`None`: no build recorded).
pub fn build_summary(last: Option<&Value>) -> String {
    let Some(last) = last.filter(|v| !v.is_null()) else {
        return "no build recorded yet (started with --build=false?)".into();
    };
    let n = |k: &str| last.get(k).and_then(Value::as_u64).unwrap_or(0);
    let plural = |n: u64, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    if last.get("ok").and_then(Value::as_bool) == Some(true) {
        let warnings = n("warnings");
        let mut s = format!("ok ({} ms", n("duration_ms"));
        if warnings > 0 {
            s.push_str(&format!(", {}", plural(warnings, "warning")));
        }
        s.push(')');
        s
    } else {
        format!("FAILED ({})", plural(n("errors"), "error"))
    }
}

fn print_status(status: &Value) {
    human_println!(
        "Dev server:     {} (pid {})",
        str_of(status, "url"),
        status.get("pid").and_then(Value::as_u64).unwrap_or(0)
    );
    let last = status.get("last_build");
    human_println!("Last build:     {}", build_summary(last));
    if let Some(last) = last.filter(|v| !v.is_null()) {
        for d in array_of(last, "diagnostics") {
            human_println!("{}", format_diagnostic(d));
        }
        if let Some(message) = last.get("message").and_then(Value::as_str) {
            human_println!("error: {message}");
        }
    }
    let browser = array_of(status, "browser_errors");
    if browser.is_empty() {
        human_println!("Browser errors: none");
    } else {
        human_println!("Browser errors: {}", browser.len());
        for e in browser {
            let mut line = format!(
                "  {} {}: {}",
                str_of(e, "page"),
                str_of(e, "kind"),
                str_of(e, "message")
            );
            if !str_of(e, "source").is_empty() {
                line.push_str(&format!(
                    " ({}:{}:{})",
                    str_of(e, "source"),
                    e.get("line").and_then(Value::as_u64).unwrap_or(0),
                    e.get("col").and_then(Value::as_u64).unwrap_or(0)
                ));
            }
            let count = e.get("count").and_then(Value::as_u64).unwrap_or(1);
            if count > 1 {
                line.push_str(&format!(" (x{count})"));
            }
            human_println!("{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_diagnostic_matches_check_style() {
        let d = json!({"severity": "error", "code": "template-render", "message": "boom",
                       "file": "templates/base.html", "line": 3, "column": 5, "hint": "fix it"});
        assert_eq!(
            format_diagnostic(&d),
            "templates/base.html:3:5: error[template-render]: boom\n  hint: fix it"
        );
        let bare = json!({"severity": "warning", "code": "x", "message": "m"});
        assert_eq!(format_diagnostic(&bare), "warning[x]: m");
    }

    #[test]
    fn test_build_summary() {
        assert!(build_summary(None).contains("no build"));
        assert_eq!(
            build_summary(Some(&json!({"ok": true, "duration_ms": 12, "warnings": 1}))),
            "ok (12 ms, 1 warning)"
        );
        assert_eq!(
            build_summary(Some(&json!({"ok": false, "errors": 2}))),
            "FAILED (2 errors)"
        );
    }
}
