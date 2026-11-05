//! `seite explain <code>` — what a diagnostic code means and how to fix it,
//! from the embedded error reference (`seite-sh/content/docs/errors.md`).

use anyhow::bail;
use clap::Args;
use serde_json::json;

use crate::diagnostics::docs_url;
use crate::human_println;
use crate::output::{human, json as json_out};

#[derive(Args)]
pub struct ExplainArgs {
    /// A diagnostic code as printed by `seite check`/`seite build`,
    /// e.g. broken-link (`error[broken-link]` also works)
    pub code: String,
}

pub fn run(args: &ExplainArgs) -> anyhow::Result<()> {
    let code = normalize(&args.code);
    let sections = crate::docs::error_sections();
    let Some((_, body)) = sections.iter().find(|(c, _)| *c == code) else {
        let known: Vec<&str> = sections.iter().map(|(c, _)| *c).collect();
        let close: Vec<&str> = known
            .iter()
            .copied()
            .filter(|k| k.contains(code.as_str()) || strsim::jaro_winkler(&code, k) > 0.8)
            .collect();
        let close = if close.is_empty() {
            String::new()
        } else {
            format!("\nClose matches: {}", close.join(", "))
        };
        bail!(
            "unknown diagnostic code '{code}'{}{close}\nKnown codes: {}",
            human::suggest_match(&code, &known),
            known.join(", ")
        );
    };
    let url = docs_url(&code);
    human_println!("## {code}");
    human_println!();
    human_println!("{}", body.trim());
    human_println!();
    human_println!("Online: {url}");
    json_out::set_data(json!({
        "code": code,
        "explanation": body.trim(),
        "docs_url": url,
    }));
    Ok(())
}

/// Accept the code as printed in diagnostics: `error[broken-link]`,
/// `[broken-link]`, ` Broken-Link `.
fn normalize(raw: &str) -> String {
    let mut code = raw.trim();
    if let (Some(start), Some(end)) = (code.find('['), code.rfind(']')) {
        if start < end {
            code = &code[start + 1..end];
        }
    }
    code.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("broken-link"), "broken-link");
        assert_eq!(normalize(" Broken-Link "), "broken-link");
        assert_eq!(normalize("error[broken-link]"), "broken-link");
        assert_eq!(
            normalize("warning[config-unknown-key]:"),
            "config-unknown-key"
        );
    }
}
