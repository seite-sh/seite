use console::style;

/// Print a success message.
pub fn success(msg: &str) {
    println!("{} {}", style("✓").green().bold(), msg);
}

/// Print an info message.
pub fn info(msg: &str) {
    println!("{} {}", style("ℹ").blue().bold(), msg);
}

/// Print a warning message.
pub fn warning(msg: &str) {
    println!("{} {}", style("⚠").yellow().bold(), msg);
}

/// Print an error message.
pub fn error(msg: &str) {
    eprintln!("{} {}", style("✗").red().bold(), msg);
}

/// Print a header/section title.
pub fn header(msg: &str) {
    println!("\n{}", style(msg).bold().underlined());
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
