//! `seite_create_collection` — add a preset collection (same code path as
//! `seite collection add`).

use super::{input_object, object_schema, prop, Args, ServerState, ToolError, ToolResult};
use crate::cli::collection::{add_preset_collection, PRESET_NAMES};
use crate::mcp::content_index;

pub fn input_schema() -> serde_json::Value {
    input_object(
        serde_json::json!({
            "preset": prop(
                "string",
                "Preset name: posts (dated, RSS), docs (nested, sidebar nav), pages (top-level URLs), changelog, roadmap, or trust"
            )
        }),
        &["preset"],
    )
}

pub fn output_schema() -> serde_json::Value {
    object_schema(
        serde_json::json!({
            "added": prop("string", "Name of the collection that was added"),
            "content_dir": prop("string", "Content directory, relative to the site root"),
            "url_prefix": { "type": "string" },
            "collection": {
                "type": "object",
                "description": "The collection's seite.toml settings",
                "additionalProperties": true
            },
            "next_steps": { "type": "array", "items": { "type": "string" } }
        }),
        &["added", "content_dir", "url_prefix", "collection"],
    )
}

pub fn call_create_collection(state: &ServerState, arguments: &serde_json::Value) -> ToolResult {
    let args = Args::new("seite_create_collection", arguments, &["preset"])?;
    let preset = args.req_str("preset")?;
    let (config, paths) = state.site()?;

    if config.collections.iter().any(|c| c.name == preset) {
        return Err(ToolError::new(format!(
            "seite_create_collection: collection '{preset}' already exists in seite.toml. \
             Existing collections: {}",
            config
                .collections
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if !PRESET_NAMES.contains(&preset) {
        return Err(ToolError::new(format!(
            "seite_create_collection: unknown preset '{preset}'. Available presets: {}",
            PRESET_NAMES.join(", ")
        )));
    }

    let added = add_preset_collection(&paths.root, preset)
        .map_err(|e| ToolError::new(format!("seite_create_collection: {e}")))?;
    let collection_json = serde_json::to_value(&added.collection)
        .map_err(|e| ToolError::new(format!("seite_create_collection: {e}")))?;
    Ok(serde_json::json!({
        "added": added.collection.name,
        "content_dir": content_index::relative_path(&paths.root, &added.content_dir),
        "url_prefix": added.collection.url_prefix,
        "collection": collection_json,
        "next_steps": [
            format!(
                "Create content with seite_create_content (collection: \"{}\").",
                added.collection.name
            ),
            "seite.toml was rewritten (comments are not preserved); the MCP server picks the new collection up on the next request.".to_string(),
        ],
    }))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    #[test]
    fn test_create_collection_adds_preset() {
        let (tmp, mut state) = site("");
        let out = call_ok(
            &mut state,
            "seite_create_collection",
            serde_json::json!({ "preset": "changelog" }),
        );
        assert_eq!(out["added"], "changelog");
        assert_eq!(out["content_dir"], "content/changelog");
        assert_eq!(out["url_prefix"], "/changelog");
        assert!(tmp.path().join("content/changelog").is_dir());
        state.reload_config();
        let config = state.config.as_ref().unwrap();
        assert!(config.collections.iter().any(|c| c.name == "changelog"));
    }

    #[test]
    fn test_create_collection_errors() {
        let (_tmp, mut state) = site("");
        let err = call_err(
            &mut state,
            "seite_create_collection",
            serde_json::json!({ "preset": "posts" }),
        );
        assert!(err.contains("already exists"), "{err}");
        let err = call_err(
            &mut state,
            "seite_create_collection",
            serde_json::json!({ "preset": "blog" }),
        );
        assert!(
            err.contains("unknown preset") && err.contains("roadmap"),
            "{err}"
        );
        let err = call_err(&mut state, "seite_create_collection", serde_json::json!({}));
        assert!(err.contains("missing required argument 'preset'"), "{err}");
        let err = call_err(
            &mut state,
            "seite_create_collection",
            serde_json::json!({ "preset": "docs", "extra": 1 }),
        );
        assert!(err.contains("unknown argument"), "{err}");
    }
}
