//! Structured diagnostics shared by `seite check`, `seite build`, and the MCP
//! server.
//!
//! A [`Diagnostic`] describes one problem with a stable machine-readable
//! `code`, a human message, and (when known) the source file and line it came
//! from. Commands collect diagnostics into a [`Diagnostics`] list so every
//! problem is reported in one pass instead of stopping at the first error.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// How serious a diagnostic is. Errors fail `check`/`build`; warnings fail
/// only in strict mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => f.write_str("error"),
            Severity::Warning => f.write_str("warning"),
        }
    }
}

/// One problem found while checking or building a site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable kebab-case identifier, e.g. `frontmatter-parse`,
    /// `config-unknown-key`, `broken-link`. Agents and tests match on this,
    /// so never rename an existing code.
    pub code: &'static str,
    pub message: String,
    /// Source file, relative to the site root when possible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// 1-based line number in `file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// 1-based column number in `file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// Actionable suggestion, e.g. "did you mean `minify`?".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }

    fn new(severity: Severity, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            file: None,
            line: None,
            column: None,
            hint: None,
        }
    }

    pub fn with_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Make `file` relative to `root` when it lives under it.
    pub fn relative_to(mut self, root: &Path) -> Self {
        if let Some(file) = &self.file {
            if let Ok(rel) = file.strip_prefix(root) {
                self.file = Some(rel.to_path_buf());
            }
        }
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Renders as `path:line:col: severity[code]: message` followed by an
/// indented `hint:` line, matching compiler-style output that editors and
/// agents already know how to parse.
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}", file.display())?;
            if let Some(line) = self.line {
                write!(f, ":{line}")?;
                if let Some(column) = self.column {
                    write!(f, ":{column}")?;
                }
            }
            f.write_str(": ")?;
        }
        write!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {hint}")?;
        }
        Ok(())
    }
}

/// An ordered collection of diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.0.push(diagnostic);
    }

    pub fn extend(&mut self, other: impl IntoIterator<Item = Diagnostic>) {
        self.0.extend(other);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.0.iter()
    }

    pub fn error_count(&self) -> usize {
        self.0.iter().filter(|d| d.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.0.len() - self.error_count()
    }

    pub fn has_errors(&self) -> bool {
        self.0.iter().any(Diagnostic::is_error)
    }

    /// Sort by file, then line, then severity, so output is stable and
    /// problems in the same file are grouped.
    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| {
            (&a.file, a.line, a.column, a.severity, a.code)
                .cmp(&(&b.file, b.line, b.column, b.severity, b.code))
        });
    }

    /// One-line summary such as `2 errors, 1 warning`.
    pub fn summary(&self) -> String {
        let errors = self.error_count();
        let warnings = self.warning_count();
        format!(
            "{errors} error{}, {warnings} warning{}",
            if errors == 1 { "" } else { "s" },
            if warnings == 1 { "" } else { "s" }
        )
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.0
    }
}

impl From<Vec<Diagnostic>> for Diagnostics {
    fn from(v: Vec<Diagnostic>) -> Self {
        Self(v)
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Every diagnostic on its own line, then the summary.
impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for d in &self.0 {
            writeln!(f, "{d}")?;
        }
        write!(f, "{}", self.summary())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_with_location_and_hint() {
        let d = Diagnostic::error("config-unknown-key", "unknown key `minfy` in [build]")
            .with_file("seite.toml")
            .with_line(12)
            .with_column(1)
            .with_hint("did you mean `minify`?");
        assert_eq!(
            d.to_string(),
            "seite.toml:12:1: error[config-unknown-key]: unknown key `minfy` in [build]\n  hint: did you mean `minify`?"
        );
    }

    #[test]
    fn test_display_without_file() {
        let d = Diagnostic::warning("broken-link", "link to /nope has no target");
        assert_eq!(
            d.to_string(),
            "warning[broken-link]: link to /nope has no target"
        );
    }

    #[test]
    fn test_relative_to_strips_root() {
        let d = Diagnostic::error("x", "m")
            .with_file("/site/content/posts/a.md")
            .relative_to(Path::new("/site"));
        assert_eq!(d.file.as_deref(), Some(Path::new("content/posts/a.md")));
    }

    #[test]
    fn test_counts_sort_and_summary() {
        let mut ds = Diagnostics::new();
        ds.push(Diagnostic::warning("w", "w").with_file("b.md").with_line(1));
        ds.push(Diagnostic::error("e", "e").with_file("a.md").with_line(5));
        ds.push(Diagnostic::error("e", "e").with_file("a.md").with_line(2));
        assert_eq!(ds.error_count(), 2);
        assert_eq!(ds.warning_count(), 1);
        assert!(ds.has_errors());
        ds.sort();
        let lines: Vec<_> = ds.iter().map(|d| d.line.unwrap()).collect();
        assert_eq!(lines, vec![2, 5, 1]);
        assert_eq!(ds.summary(), "2 errors, 1 warning");
    }

    #[test]
    fn test_serializes_without_empty_fields() {
        let d = Diagnostic::error("frontmatter-parse", "bad").with_file("a.md");
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["severity"], "error");
        assert_eq!(v["code"], "frontmatter-parse");
        assert_eq!(v["file"], "a.md");
        assert!(v.get("line").is_none());
        assert!(v.get("hint").is_none());
    }
}
