//! JSON output for the global `--json` flag.
//!
//! Every command run with `--json` prints exactly one JSON document on stdout
//! when it finishes:
//!
//! ```json
//! {"ok":true,"command":"build","data":{...},"warnings":[]}
//! {"ok":false,"command":"build","error":{"message":"...","chain":["cause"]},"warnings":[]}
//! ```
//!
//! Commands contribute `data` through [`set_data`]; when they don't, `data` is
//! `null`. All human-readable output is routed to stderr in JSON mode (see
//! [`redirect_stdout_to_stderr`] and [`super::human::emit_line`]).

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};

static DATA: Mutex<Option<Value>> = Mutex::new(None);
static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Handle to the original stdout, saved when stdout is redirected to stderr.
static SAVED_STDOUT: Mutex<Option<File>> = Mutex::new(None);

/// Set the `data` payload of the JSON envelope for the current command.
/// Later calls replace earlier ones.
pub fn set_data(value: Value) {
    *DATA.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
}

/// Take the `data` payload set by the current command (if any).
pub fn take_data() -> Option<Value> {
    DATA.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Record a warning for the JSON envelope's `warnings` array.
pub fn record_warning(msg: &str) {
    WARNINGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(msg.to_string());
}

/// Snapshot of the warnings recorded so far.
pub fn warnings() -> Vec<String> {
    WARNINGS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Flatten an error into its top-level message plus a de-duplicated cause chain.
///
/// Causes whose text is empty, or already contained in the previous message
/// (common with `thiserror` variants that interpolate their `source`), are
/// dropped so renderers never show blank or repeated "Caused by" lines.
pub fn error_chain(error: &anyhow::Error) -> (String, Vec<String>) {
    let mut iter = error.chain();
    // A diagnostics list is summarised here; callers render the individual
    // diagnostics themselves (see [`find_diagnostics`]).
    let message = iter
        .next()
        .map(|e| match as_diagnostics(e) {
            Some(d) => format!("found {}", d.summary()),
            None => e.to_string(),
        })
        .unwrap_or_default();
    let mut previous = message.clone();
    let mut chain = Vec::new();
    for cause in iter {
        if as_diagnostics(cause).is_some() {
            continue;
        }
        let text = cause.to_string().trim().to_string();
        if text.is_empty() || previous.contains(&text) {
            continue;
        }
        previous = text.clone();
        chain.push(text);
    }
    (message, chain)
}

fn as_diagnostics<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a crate::diagnostics::Diagnostics> {
    match error.downcast_ref::<crate::error::PageError>() {
        Some(crate::error::PageError::Diagnostics(d)) => Some(d),
        _ => None,
    }
}

/// The diagnostics carried by `error` (anywhere in its cause chain), if it is
/// a [`crate::error::PageError::Diagnostics`].
pub fn find_diagnostics(error: &anyhow::Error) -> Option<&crate::diagnostics::Diagnostics> {
    error.chain().find_map(as_diagnostics)
}

/// Build the success envelope for `command`.
pub fn success_document(command: &str, data: Option<Value>, warnings: Vec<String>) -> Value {
    json!({
        "ok": true,
        "command": command,
        "data": data.unwrap_or(Value::Null),
        "warnings": warnings,
    })
}

/// Build the failure envelope for `command`.
///
/// When the error carries diagnostics (`PageError::Diagnostics`), they are
/// included as `error.diagnostics` (plus `error.summary` counts) so agents get
/// every problem with its file, line, code, and hint.
pub fn error_document(command: &str, error: &anyhow::Error, warnings: Vec<String>) -> Value {
    let (message, chain) = error_chain(error);
    let mut err = json!({ "message": message, "chain": chain });
    if let Some(diagnostics) = find_diagnostics(error) {
        err["diagnostics"] = serde_json::to_value(diagnostics).unwrap_or(Value::Null);
        err["summary"] = json!({
            "errors": diagnostics.error_count(),
            "warnings": diagnostics.warning_count(),
        });
    }
    json!({
        "ok": false,
        "command": command,
        "error": err,
        "warnings": warnings,
    })
}

/// Point the process's stdout (fd 1) at stderr, keeping a private handle to
/// the original stdout for the final JSON document. This guarantees stdout
/// stays pure JSON even for stray `println!`s and inherited child-process
/// output (git, wrangler, …). No-op on non-Unix platforms, where human output
/// is still routed to stderr by [`super::human::emit_line`] and child
/// processes get their stdout pointed at stderr at each spawn site via
/// [`super::child_stdout`] (which is also used on Unix, making this a
/// belt-and-braces there).
pub fn redirect_stdout_to_stderr() {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        let _ = std::io::stdout().flush();
        let Ok(saved) = rustix::io::dup(std::io::stdout().as_fd()) else {
            return;
        };
        if rustix::stdio::dup2_stdout(std::io::stderr().as_fd()).is_ok() {
            *SAVED_STDOUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(File::from(saved));
        }
    }
}

/// Write the final JSON document (one line) to the real stdout.
pub fn emit_document(doc: &Value) {
    let line = format!("{doc}\n");
    let _ = std::io::stdout().flush();
    let mut saved = SAVED_STDOUT.lock().unwrap_or_else(|e| e.into_inner());
    match saved.as_mut() {
        Some(file) => {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
        None => {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(line.as_bytes());
            let _ = out.flush();
        }
    }
}

/// Wrap any serializable value in a standard JSON envelope.
#[derive(Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> JsonEnvelope<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
}

impl JsonEnvelope<()> {
    pub fn error(message: String) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("outer failed: {source}")]
    struct Outer {
        source: std::io::Error,
    }

    #[test]
    fn test_success_document_shape() {
        let doc = success_document("build", Some(json!({"pages": 3})), vec!["w".into()]);
        assert_eq!(doc["ok"], true);
        assert_eq!(doc["command"], "build");
        assert_eq!(doc["data"]["pages"], 3);
        assert_eq!(doc["warnings"], json!(["w"]));
        assert!(doc.get("error").is_none());
    }

    #[test]
    fn test_success_document_null_data() {
        let doc = success_document("new", None, vec![]);
        assert!(doc["data"].is_null());
        assert_eq!(doc["warnings"], json!([]));
    }

    #[test]
    fn test_error_document_shape() {
        let err = anyhow::anyhow!("root cause").context("top level");
        let doc = error_document("build", &err, vec![]);
        assert_eq!(doc["ok"], false);
        assert_eq!(doc["command"], "build");
        assert_eq!(doc["error"]["message"], "top level");
        assert_eq!(doc["error"]["chain"], json!(["root cause"]));
        assert!(doc.get("data").is_none());
    }

    #[test]
    fn test_error_chain_drops_duplicated_source() {
        // thiserror variants that interpolate `{source}` repeat it in the chain.
        let err = anyhow::Error::new(Outer {
            source: std::io::Error::other("disk on fire"),
        });
        let (message, chain) = error_chain(&err);
        assert_eq!(message, "outer failed: disk on fire");
        assert!(
            chain.is_empty(),
            "duplicate cause should be dropped: {chain:?}"
        );
    }

    #[test]
    fn test_error_chain_drops_empty_causes() {
        let err = anyhow::anyhow!("").context("something failed");
        let (message, chain) = error_chain(&err);
        assert_eq!(message, "something failed");
        assert!(chain.is_empty());
    }

    #[test]
    fn test_success_envelope() {
        let env = JsonEnvelope::success("hello");
        assert!(env.ok);
        assert_eq!(env.data, Some("hello"));
        assert!(env.error.is_none());
    }

    #[test]
    fn test_error_envelope() {
        let env = JsonEnvelope::<()>::error("something broke".into());
        assert!(!env.ok);
        assert!(env.data.is_none());
        assert_eq!(env.error.as_deref(), Some("something broke"));
    }

    #[test]
    fn test_success_serialization_omits_none_fields() {
        let env = JsonEnvelope::success(42);
        let json = serde_json::to_value(&env).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["data"], 42);
        assert!(json.get("error").is_none());
    }

    #[test]
    fn test_error_serialization_omits_none_fields() {
        let env = JsonEnvelope::<()>::error("fail".into());
        let json = serde_json::to_value(&env).unwrap();
        assert_eq!(json["ok"], false);
        assert!(json.get("data").is_none());
        assert_eq!(json["error"], "fail");
    }

    #[test]
    fn test_success_with_struct() {
        #[derive(Serialize, PartialEq, Debug)]
        struct Info {
            count: u32,
        }
        let env = JsonEnvelope::success(Info { count: 5 });
        assert!(env.ok);
        assert_eq!(env.data.unwrap().count, 5);
    }

    #[test]
    fn test_error_document_includes_diagnostics() {
        use crate::diagnostics::{Diagnostic, Diagnostics};
        let mut ds = Diagnostics::new();
        ds.push(
            Diagnostic::error("frontmatter-parse", "bad")
                .with_file("content/posts/a.md")
                .with_line(3),
        );
        ds.push(Diagnostic::warning("config-unknown-key", "unknown"));
        let err = anyhow::Error::new(crate::error::PageError::Diagnostics(ds));
        let doc = error_document("build", &err, vec![]);
        assert_eq!(doc["error"]["message"], "found 1 error, 1 warning");
        assert_eq!(doc["error"]["chain"], json!([]));
        let diags = doc["error"]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0]["code"], "frontmatter-parse");
        assert_eq!(diags[0]["file"], "content/posts/a.md");
        assert_eq!(diags[0]["line"], 3);
        assert_eq!(doc["error"]["summary"]["errors"], 1);

        // Wrapped in context: the context is the message, the list is not
        // repeated in the chain, and the diagnostics are still found.
        let wrapped = err.context("check failed");
        let doc = error_document("check", &wrapped, vec![]);
        assert_eq!(doc["error"]["message"], "check failed");
        assert_eq!(doc["error"]["chain"], json!([]));
        assert_eq!(doc["error"]["diagnostics"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_error_document_without_diagnostics_has_no_field() {
        let err = anyhow::anyhow!("plain");
        let doc = error_document("build", &err, vec![]);
        assert!(doc["error"].get("diagnostics").is_none());
    }
}
