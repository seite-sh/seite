//! Interactive prompts that degrade safely when nobody is at the keyboard.
//!
//! All CLI prompts go through this module instead of calling `dialoguer`
//! directly. A prompt is shown only when stdin and stderr are both terminals
//! and neither `--yes` nor `SEITE_YES=1` is set. Otherwise:
//!
//! - prompts with a default return that default without prompting;
//! - [`confirm`] returns its default, or `true` under `--yes`;
//! - [`confirm_or_fail`] returns `true` under `--yes` and errors otherwise, for
//!   confirmations that guard side effects which must never happen silently;
//! - prompts without a default fail with an error naming the flag to pass.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

static ASSUME_YES: AtomicBool = AtomicBool::new(false);

/// Record the global `--yes` flag (set once in `main`).
pub fn set_assume_yes(enabled: bool) {
    ASSUME_YES.store(enabled, Ordering::Relaxed);
}

/// Whether confirmations should be auto-accepted (`--yes` or `SEITE_YES`).
pub fn assume_yes() -> bool {
    ASSUME_YES.load(Ordering::Relaxed)
        || std::env::var("SEITE_YES")
            .map(|v| env_truthy(&v))
            .unwrap_or(false)
}

fn env_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Whether prompts may be shown: a human is at a terminal and `--yes` is off.
pub fn is_interactive() -> bool {
    !assume_yes() && std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Build the error returned when a required value can't be prompted for.
///
/// `flag` names what to pass (e.g. `--deploy-target`); `choices` lists the
/// accepted values, if any.
pub fn missing_value(flag: &str, choices: &[&str]) -> anyhow::Error {
    if choices.is_empty() {
        anyhow::anyhow!("missing {flag} (required when not running interactively)")
    } else {
        anyhow::anyhow!(
            "missing {flag} (required when not running interactively; one of: {})",
            choices.join(", ")
        )
    }
}

/// Resolve a non-interactive answer: the default if there is one, else an
/// error naming `flag`.
fn resolve_default<T>(default: Option<T>, flag: &str, choices: &[&str]) -> anyhow::Result<T> {
    default.ok_or_else(|| missing_value(flag, choices))
}

/// Prompt for a line of text. `default` of `Some("")` allows an empty answer.
pub fn input(prompt: &str, default: Option<&str>, flag: &str) -> anyhow::Result<String> {
    if !is_interactive() {
        return resolve_default(default.map(str::to_string), flag, &[]);
    }
    let mut input = dialoguer::Input::<String>::new().with_prompt(prompt);
    if let Some(d) = default {
        input = input.default(d.to_string()).allow_empty(d.is_empty());
    }
    Ok(input.interact_text()?)
}

/// Prompt for a secret. Never has a default, so it always errors when not
/// interactive; `hint` tells the user how to proceed.
pub fn password(prompt: &str, confirmation: Option<&str>, hint: &str) -> anyhow::Result<String> {
    if !is_interactive() {
        anyhow::bail!("cannot prompt for a password when not running interactively; {hint}");
    }
    let mut p = dialoguer::Password::new().with_prompt(prompt);
    if let Some(confirm) = confirmation {
        p = p.with_confirmation(confirm, "Passwords do not match");
    }
    Ok(p.interact()?)
}

/// Pick one item. `default: None` means the choice is required when not
/// interactive (the interactive cursor then starts at the first item).
/// `choices` are the flag values listed in the error message.
pub fn select(
    prompt: &str,
    items: &[&str],
    default: Option<usize>,
    flag: &str,
    choices: &[&str],
) -> anyhow::Result<usize> {
    if !is_interactive() {
        return resolve_default(default, flag, choices);
    }
    Ok(dialoguer::Select::new()
        .with_prompt(prompt)
        .items(items)
        .default(default.unwrap_or(0))
        .interact()?)
}

/// Pick any number of items. When not interactive, returns the indices whose
/// `defaults` entry is `true`.
pub fn multi_select(prompt: &str, items: &[&str], defaults: &[bool]) -> anyhow::Result<Vec<usize>> {
    if !is_interactive() {
        return Ok(default_indices(defaults));
    }
    Ok(dialoguer::MultiSelect::new()
        .with_prompt(prompt)
        .items(items)
        .defaults(defaults)
        .interact()?)
}

fn default_indices(defaults: &[bool]) -> Vec<usize> {
    defaults
        .iter()
        .enumerate()
        .filter_map(|(i, &on)| on.then_some(i))
        .collect()
}

/// Ask a yes/no question. Returns `true` under `--yes`, and `default` when
/// not interactive.
pub fn confirm(prompt: &str, default: bool) -> anyhow::Result<bool> {
    if assume_yes() {
        return Ok(true);
    }
    if !is_interactive() {
        return Ok(default);
    }
    Ok(dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

/// Ask a yes/no question guarding a side effect that must not happen (or be
/// skipped) silently. Returns `true` under `--yes`; when not interactive
/// without `--yes`, fails with an error explaining how to proceed. `action`
/// completes the sentence "re-run with --yes to …".
pub fn confirm_or_fail(prompt: &str, default: bool, action: &str) -> anyhow::Result<bool> {
    if assume_yes() {
        return Ok(true);
    }
    if !is_interactive() {
        anyhow::bail!(
            "confirmation required: \"{prompt}\" (not running interactively; re-run with --yes to {action})"
        );
    }
    Ok(dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_truthy() {
        for v in ["1", "true", "TRUE", "yes", " on "] {
            assert!(env_truthy(v), "{v} should be truthy");
        }
        for v in ["", "0", "false", "no", "off", "maybe"] {
            assert!(!env_truthy(v), "{v} should be falsy");
        }
    }

    #[test]
    fn test_missing_value_names_flag() {
        let err = missing_value("--title", &[]).to_string();
        assert_eq!(
            err,
            "missing --title (required when not running interactively)"
        );
    }

    #[test]
    fn test_missing_value_lists_choices() {
        let err = missing_value(
            "--deploy-target",
            &["github-pages", "cloudflare", "netlify"],
        )
        .to_string();
        assert!(err.starts_with("missing --deploy-target"));
        assert!(err.contains("one of: github-pages, cloudflare, netlify"));
    }

    #[test]
    fn test_resolve_default_uses_default() {
        assert_eq!(resolve_default(Some(3), "--x", &[]).unwrap(), 3);
    }

    #[test]
    fn test_resolve_default_errors_without_default() {
        let err = resolve_default::<usize>(None, "--x", &["a"]).unwrap_err();
        assert!(err.to_string().contains("missing --x"));
    }

    #[test]
    fn test_default_indices() {
        assert_eq!(default_indices(&[true, false, true]), vec![0, 2]);
        assert!(default_indices(&[false, false]).is_empty());
    }

    // `cargo test` runs without a TTY on stdin (captured), so these exercise the
    // non-interactive paths. Guard anyway in case a developer runs them from a
    // terminal with inherited stdin.
    #[test]
    fn test_non_interactive_prompts_use_defaults() {
        if is_interactive() {
            return;
        }
        assert_eq!(input("Title", Some("dflt"), "--title").unwrap(), "dflt");
        assert_eq!(input("Desc", Some(""), "--description").unwrap(), "");
        assert!(input("Name", None, "--name")
            .unwrap_err()
            .to_string()
            .contains("--name"));
        assert_eq!(
            select("Pick", &["a", "b"], Some(1), "--pick", &[]).unwrap(),
            1
        );
        assert!(select("Pick", &["a", "b"], None, "--pick", &["a", "b"])
            .unwrap_err()
            .to_string()
            .contains("one of: a, b"));
        assert_eq!(
            multi_select("Many", &["a", "b", "c"], &[false, true, true]).unwrap(),
            vec![1, 2]
        );
        assert!(password("Pw", None, "use a terminal")
            .unwrap_err()
            .to_string()
            .contains("use a terminal"));
    }

    #[test]
    fn test_non_interactive_confirm_behaviour() {
        if is_interactive() || assume_yes() {
            return;
        }
        assert!(!confirm("Proceed?", false).unwrap());
        assert!(confirm("Proceed?", true).unwrap());
        let err = confirm_or_fail("Deploy anyway?", false, "deploy anyway").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("--yes"), "{msg}");
        assert!(msg.contains("deploy anyway"), "{msg}");
    }
}
