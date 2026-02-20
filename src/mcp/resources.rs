//! MCP resource providers — expose site data as structured resources.
//!
//! Resources are read-only data items identified by URI. The MCP client
//! discovers them via `resources/list` and reads them via `resources/read`.

use std::fs;

use walkdir::WalkDir;

use super::{JsonRpcError, ServerState};
use crate::content;

/// Handle `resources/list` — enumerate all available resources.
pub fn list(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let mut resources = Vec::new();

    // Documentation resources (always available — embedded in binary)
    resources.push(serde_json::json!({
        "uri": "page://docs",
        "name": "Documentation Index",
        "description": "List of all page documentation pages",
        "mimeType": "application/json"
    }));
    for doc in crate::docs::all() {
        resources.push(serde_json::json!({
            "uri": format!("page://docs/{}", doc.slug),
            "name": doc.title,
            "description": doc.description,
            "mimeType": "text/markdown"
        }));
    }

    // Site-specific resources (only when page.toml exists)
    if let Some(ref config) = state.config {
        resources.push(serde_json::json!({
            "uri": "page://config",
            "name": "Site Configuration",
            "description": "Current page.toml configuration as JSON",
            "mimeType": "application/json"
        }));

        resources.push(serde_json::json!({
            "uri": "page://content",
            "name": "Content Overview",
            "description": "All collections with item counts",
            "mimeType": "application/json"
        }));

        for collection in &config.collections {
            resources.push(serde_json::json!({
                "uri": format!("page://content/{}", collection.name),
                "name": format!("{} collection", collection.label),
                "description": format!("Content items in the {} collection", collection.name),
                "mimeType": "application/json"
            }));
        }

        resources.push(serde_json::json!({
            "uri": "page://themes",
            "name": "Themes",
            "description": "Available bundled and installed themes",
            "mimeType": "application/json"
        }));

        // MCP configuration
        let mcp_config_path = state.cwd.join(".claude/settings.json");
        if mcp_config_path.exists() {
            resources.push(serde_json::json!({
                "uri": "page://mcp-config",
                "name": "MCP Configuration",
                "description": "Claude Code MCP server configuration (.claude/settings.json)",
                "mimeType": "application/json"
            }));
        }
    }

    Ok(serde_json::json!({ "resources": resources }))
}

/// Handle `resources/read` — return the content of a specific resource.
pub fn read(
    state: &ServerState,
    params: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let uri = params
        .get("uri")
        .and_then(|u| u.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'uri' parameter"))?;

    // Route based on URI
    if uri == "page://docs" {
        return read_docs_index();
    }
    if let Some(slug) = uri.strip_prefix("page://docs/") {
        return read_doc(slug);
    }
    if uri == "page://config" {
        return read_config(state);
    }
    if uri == "page://content" {
        return read_content_overview(state);
    }
    if let Some(collection) = uri.strip_prefix("page://content/") {
        return read_collection(state, collection);
    }
    if uri == "page://themes" {
        return read_themes(state);
    }
    if uri == "page://mcp-config" {
        return read_mcp_config(state);
    }

    Err(JsonRpcError::invalid_params(format!(
        "Unknown resource URI: {uri}"
    )))
}

// ---------------------------------------------------------------------------
// Documentation resources
// ---------------------------------------------------------------------------

fn read_docs_index() -> Result<serde_json::Value, JsonRpcError> {
    let docs: Vec<serde_json::Value> = crate::docs::all()
        .iter()
        .map(|d| {
            serde_json::json!({
                "slug": d.slug,
                "title": d.title,
                "description": d.description,
                "weight": d.weight,
            })
        })
        .collect();

    let text = serde_json::to_string_pretty(&docs).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": "page://docs",
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

fn read_doc(slug: &str) -> Result<serde_json::Value, JsonRpcError> {
    let doc = crate::docs::by_slug(slug).ok_or_else(|| {
        JsonRpcError::invalid_params(format!("Documentation page not found: {slug}"))
    })?;

    let body = crate::docs::strip_frontmatter(doc.raw_content);
    Ok(serde_json::json!({
        "contents": [{
            "uri": format!("page://docs/{slug}"),
            "mimeType": "text/markdown",
            "text": body
        }]
    }))
}

// ---------------------------------------------------------------------------
// Site configuration resource
// ---------------------------------------------------------------------------

fn read_config(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a page project (no page.toml)"))?;

    let value =
        serde_json::to_value(config).map_err(|e| JsonRpcError::internal(e.to_string()))?;
    let text = serde_json::to_string_pretty(&value).unwrap_or_default();

    Ok(serde_json::json!({
        "contents": [{
            "uri": "page://config",
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

// ---------------------------------------------------------------------------
// Content resources
// ---------------------------------------------------------------------------

fn read_content_overview(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a page project"))?;
    let paths = state.paths.as_ref().unwrap();

    let mut collections = Vec::new();
    for coll in &config.collections {
        let dir = paths.content.join(&coll.directory);
        let count = if dir.exists() {
            WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .count()
        } else {
            0
        };
        collections.push(serde_json::json!({
            "name": coll.name,
            "label": coll.label,
            "items": count,
            "has_date": coll.has_date,
            "has_rss": coll.has_rss,
            "nested": coll.nested,
            "url_prefix": coll.url_prefix,
        }));
    }

    let text = serde_json::to_string_pretty(&collections).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": "page://content",
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

fn read_collection(
    state: &ServerState,
    collection_name: &str,
) -> Result<serde_json::Value, JsonRpcError> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a page project"))?;
    let paths = state.paths.as_ref().unwrap();

    let collection = config
        .collections
        .iter()
        .find(|c| c.name == collection_name)
        .ok_or_else(|| {
            JsonRpcError::invalid_params(format!("Collection not found: {collection_name}"))
        })?;

    let dir = paths.content.join(&collection.directory);
    let mut items = Vec::new();

    if dir.exists() {
        for entry in WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        {
            if let Ok((fm, _body)) = content::parse_content_file(entry.path()) {
                let slug = fm
                    .slug
                    .clone()
                    .unwrap_or_else(|| content::slug_from_title(&fm.title));
                let url = if collection.url_prefix.is_empty() {
                    format!("/{slug}")
                } else {
                    format!("{}/{slug}", collection.url_prefix)
                };
                items.push(serde_json::json!({
                    "title": fm.title,
                    "slug": slug,
                    "url": url,
                    "date": fm.date.map(|d| d.to_string()),
                    "tags": fm.tags,
                    "draft": fm.draft,
                    "description": fm.description,
                    "weight": fm.weight,
                }));
            }
        }
    }

    // Sort: dated items by date descending, otherwise by weight then title
    if collection.has_date {
        items.sort_by(|a, b| {
            let date_a = a["date"].as_str().unwrap_or("");
            let date_b = b["date"].as_str().unwrap_or("");
            date_b.cmp(date_a)
        });
    } else {
        items.sort_by(|a, b| {
            let weight_a = a["weight"].as_i64().unwrap_or(i64::MAX);
            let weight_b = b["weight"].as_i64().unwrap_or(i64::MAX);
            weight_a
                .cmp(&weight_b)
                .then_with(|| {
                    let title_a = a["title"].as_str().unwrap_or("");
                    let title_b = b["title"].as_str().unwrap_or("");
                    title_a.cmp(title_b)
                })
        });
    }

    let text = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": format!("page://content/{collection_name}"),
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

// ---------------------------------------------------------------------------
// Theme resources
// ---------------------------------------------------------------------------

fn read_themes(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let mut themes = Vec::new();

    // Bundled themes
    for theme in crate::themes::all() {
        themes.push(serde_json::json!({
            "name": theme.name,
            "description": theme.description,
            "source": "bundled",
        }));
    }

    // Installed themes (if in a project)
    for theme in crate::themes::installed_themes(&state.cwd) {
        themes.push(serde_json::json!({
            "name": theme.name,
            "description": theme.description,
            "source": "installed",
        }));
    }

    let text = serde_json::to_string_pretty(&themes).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": "page://themes",
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

// ---------------------------------------------------------------------------
// MCP configuration resource
// ---------------------------------------------------------------------------

fn read_mcp_config(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let path = state.cwd.join(".claude/settings.json");
    let content = fs::read_to_string(&path).map_err(|e| {
        JsonRpcError::invalid_params(format!("Cannot read .claude/settings.json: {e}"))
    })?;

    Ok(serde_json::json!({
        "contents": [{
            "uri": "page://mcp-config",
            "mimeType": "application/json",
            "text": content
        }]
    }))
}
