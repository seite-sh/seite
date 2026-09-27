//! Shared helpers for MCP tool unit tests.
//!
//! Every successful call made through [`call_tool`] is checked the way a
//! strict client (Cursor, OpenCode) checks it: `structuredContent` must be
//! present, equal the compact text payload, and validate against the tool's
//! declared `outputSchema`. Error results must carry text only.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::{call, TOOLS};
use crate::mcp::protocol::{Protocol, LEGACY_VERSIONS};
use crate::mcp::ServerState;

pub fn empty_state() -> ServerState {
    ServerState {
        config: None,
        paths: None,
        cwd: std::path::PathBuf::new(),
        config_error: None,
    }
}

/// A real site on disk (seite.toml + content) loaded like the server does.
/// `posts` is dated; `docs` is nested.
pub fn site(toml_extra: &str) -> (TempDir, ServerState) {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("seite.toml"),
        format!(
            "[site]\ntitle = \"Test\"\nbase_url = \"http://localhost:3000\"\n\n\
             [[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nurl_prefix = \"/posts\"\ndefault_template = \"post.html\"\n\n\
             [[collections]]\nname = \"docs\"\nlabel = \"Docs\"\ndirectory = \"docs\"\nnested = true\nurl_prefix = \"/docs\"\ndefault_template = \"doc.html\"\n{toml_extra}"
        ),
    )
    .unwrap();
    let state = ServerState::load(tmp.path().to_path_buf());
    assert!(state.config.is_some(), "{:?}", state.config_error);
    (tmp, state)
}

pub fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

/// Validate `instance` against `schema`, panicking with every violation.
pub fn assert_valid(schema: &serde_json::Value, instance: &serde_json::Value, context: &str) {
    let validator = jsonschema::validator_for(schema)
        .unwrap_or_else(|e| panic!("{context}: invalid schema: {e}"));
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{context}: output does not match outputSchema:\n{}\n{instance}",
        errors.join("\n")
    );
}

/// Call a tool at the newest legacy revision and return (isError, text).
pub fn call_tool(state: &mut ServerState, name: &str, args: serde_json::Value) -> (bool, String) {
    let proto = Protocol::new(LEGACY_VERSIONS[0]);
    let result = call(
        state,
        proto,
        &serde_json::json!({ "name": name, "arguments": args }),
    )
    .unwrap();
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let text = result["content"][0]["text"].as_str().unwrap().to_string();
    if is_error {
        assert!(
            result.get("structuredContent").is_none(),
            "{name}: error results must be text-only"
        );
    } else {
        let structured = &result["structuredContent"];
        assert!(structured.is_object(), "{name}: missing structuredContent");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            &parsed, structured,
            "{name}: text and structuredContent differ"
        );
        assert!(
            !text.contains("\n  "),
            "{name}: text content must be compact JSON"
        );
        let tool = TOOLS.iter().find(|t| t.name == name).unwrap();
        if let Some(schema) = tool.output_schema {
            assert_valid(&schema(), structured, name);
        }
    }
    (is_error, text)
}

pub fn call_ok(state: &mut ServerState, name: &str, args: serde_json::Value) -> serde_json::Value {
    let (is_error, text) = call_tool(state, name, args);
    assert!(!is_error, "{name} failed: {text}");
    serde_json::from_str(&text).unwrap()
}

pub fn call_err(state: &mut ServerState, name: &str, args: serde_json::Value) -> String {
    let (is_error, text) = call_tool(state, name, args);
    assert!(is_error, "{name} unexpectedly succeeded: {text}");
    text
}
