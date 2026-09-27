use std::fmt;

use console::style;

/// Write one line of human-readable output. Goes to stdout normally. In
/// `--json` mode it is dropped (or sent to stderr with `--verbose`): agents
/// often run commands in a pty that merges stdout and stderr, and status
/// chatter around the JSON document makes it harder to find. Warnings still
/// reach the document's `warnings` array.
pub fn emit_line(args: fmt::Arguments<'_>) {
    if super::is_json() {
        if super::is_verbose() {
            eprintln!("{args}");
        }
    } else {
        println!("{args}");
    }
}

/// `println!` replacement for human-readable command output. Behaves exactly
/// like `println!` except that it writes to stderr in `--json` mode.
#[macro_export]
macro_rules! human_println {
    () => {
        $crate::output::human::emit_line(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::output::human::emit_line(format_args!($($arg)*))
    };
}

/// Print a success message.
pub fn success(msg: &str) {
    emit_line(format_args!("{} {}", style("✓").green().bold(), msg));
}

/// Print an info message.
pub fn info(msg: &str) {
    emit_line(format_args!("{} {}", style("ℹ").blue().bold(), msg));
}

/// Print a warning message. Warnings are also collected for the `--json`
/// envelope (see [`super::json::warnings`]).
pub fn warning(msg: &str) {
    super::json::record_warning(msg);
    emit_line(format_args!("{} {}", style("⚠").yellow().bold(), msg));
}

/// Print a warning to stderr (for library code that must keep stdout clean).
/// Also collected for the `--json` envelope.
pub fn warning_stderr(msg: &str) {
    super::json::record_warning(msg);
    if !super::is_json() || super::is_verbose() {
        eprintln!("{} {}", style("⚠").yellow().bold(), msg);
    }
}

/// Print an error message.
pub fn error(msg: &str) {
    eprintln!("{} {}", style("✗").red().bold(), msg);
}

/// Print a header/section title.
pub fn header(msg: &str) {
    emit_line(format_args!("\n{}", style(msg).bold().underlined()));
}

/// Find the closest match for `input` among `candidates` using string similarity.
/// Returns a hint string like `\n  hint: did you mean 'posts'?` or empty if no close match.
pub fn suggest_match(input: &str, candidates: &[&str]) -> String {
    let mut best: Option<(&str, f64)> = None;
    for &candidate in candidates {
        let dist = strsim::jaro_winkler(input, candidate);
        if dist > 0.7 && (best.is_none() || dist > best.unwrap().1) {
            best = Some((candidate, dist));
        }
    }
    match best {
        Some((name, _)) => format!("\n  hint: did you mean '{name}'?"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_match_close_typo() {
        let result = suggest_match("posst", &["posts", "docs", "pages"]);
        assert!(
            result.contains("posts"),
            "should suggest 'posts' for 'posst'"
        );
    }

    #[test]
    fn test_suggest_match_no_match() {
        let result = suggest_match("zzzzz", &["posts", "docs", "pages"]);
        assert!(result.is_empty(), "should return empty for no close match");
    }

    #[test]
    fn test_suggest_match_empty_candidates() {
        let result = suggest_match("posts", &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_suggest_match_exact() {
        let result = suggest_match("posts", &["posts", "docs"]);
        assert!(result.contains("posts"));
    }

    #[test]
    fn test_suggest_match_picks_best() {
        let result = suggest_match("doc", &["docs", "dock", "dog"]);
        assert!(result.contains("docs"));
    }

    #[test]
    fn test_warning_does_not_panic() {
        warning("test warning");
    }

    #[test]
    fn test_header_does_not_panic() {
        header("test header");
    }
}
