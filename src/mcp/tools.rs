//! MCP tool implementations — actions AI tools can execute.
//!
//! Tools are invoked via `tools/call` and return structured results.
//! Each tool wraps existing seite CLI functionality.

use std::fs;

use walkdir::WalkDir;

use super::{JsonRpcError, ServerState};
use crate::{build, content, themes};

/// Handle `tools/list` — enumerate all available tools with JSON schemas.
pub fn list() -> Result<serde_json::Value, JsonRpcError> {
    Ok(serde_json::json!({
        "tools": [
            {
                "name": "seite_build",
                "description": "Build the site to the output directory. Returns build statistics including pages built and timing.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "drafts": {
                            "type": "boolean",
                            "description": "Include draft content in the build",
                            "default": false
                        }
                    }
                }
            },
            {
                "name": "seite_create_content",
                "description": "Create a new content file with frontmatter in the specified collection.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "collection": {
                            "type": "string",
                            "description": "Collection name (e.g., posts, docs, pages, changelog, roadmap)"
                        },
                        "title": {
                            "type": "string",
                            "description": "Title of the content"
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Tags for the content"
                        },
                        "body": {
                            "type": "string",
                            "description": "Markdown body content"
                        },
                        "draft": {
                            "type": "boolean",
                            "description": "Create as draft",
                            "default": false
                        }
                    },
                    "required": ["collection", "title"]
                }
            },
            {
                "name": "seite_search",
                "description": "Search site content by keywords. Matches against titles, descriptions, and tags.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search keywords"
                        },
                        "collection": {
                            "type": "string",
                            "description": "Limit search to a specific collection"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "seite_apply_theme",
                "description": "Apply a bundled or installed theme to the site. Writes templates/base.html.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Theme name: default, minimal, dark, docs, brutalist, bento, or an installed theme"
                        }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "seite_lookup_docs",
                "description": "Look up page documentation by topic slug or search by keyword. Returns relevant documentation sections.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search keywords to find in documentation"
                        },
                        "topic": {
                            "type": "string",
                            "description": "Specific doc topic slug (e.g., configuration, templates, deployment)"
                        }
                    }
                }
            }
        ]
    }))
}

/// Handle `tools/call` — dispatch to the appropriate tool.
pub fn call(
    state: &mut ServerState,
    params: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'name' parameter"))?;

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = match name {
        "seite_build" => call_build(state, &arguments),
        "seite_create_content" => call_create_content(state, &arguments),
        "seite_search" => call_search(state, &arguments),
        "seite_apply_theme" => call_apply_theme(state, &arguments),
        "seite_lookup_docs" => call_lookup_docs(&arguments),
        _ => {
            return Err(JsonRpcError::invalid_params(format!(
                "Unknown tool: {name}"
            )));
        }
    }?;

    // MCP tool results are wrapped in content blocks
    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
        }]
    }))
}

// ---------------------------------------------------------------------------
// seite_build
// ---------------------------------------------------------------------------

fn call_build(
    state: &mut ServerState,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    // Reload config to pick up any changes
    state.reload_config();

    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project (no seite.toml)"))?;
    let paths = state.paths.as_ref().unwrap();

    let include_drafts = arguments
        .get("drafts")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let opts = build::BuildOptions { include_drafts };

    match build::build_site(config, paths, &opts) {
        Ok(result) => {
            let items_built: serde_json::Value = result
                .stats
                .items_built
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect();

            Ok(serde_json::json!({
                "success": true,
                "items_built": items_built,
                "static_files_copied": result.stats.static_files_copied,
                "data_files_loaded": result.stats.data_files_loaded,
                "duration_ms": result.stats.duration_ms,
            }))
        }
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

// ---------------------------------------------------------------------------
// seite_create_content
// ---------------------------------------------------------------------------

fn call_create_content(
    state: &ServerState,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project"))?;
    let paths = state.paths.as_ref().unwrap();

    let collection_name = arguments
        .get("collection")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'collection' parameter"))?;

    let title = arguments
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'title' parameter"))?;

    let collection = crate::config::find_collection(collection_name, &config.collections)
        .ok_or_else(|| {
            JsonRpcError::invalid_params(format!("Collection not found: {collection_name}"))
        })?;

    let slug = content::slug_from_title(title);
    let tags: Vec<String> = arguments
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let draft = arguments
        .get("draft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let body = arguments
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("Your content here...");

    // Build frontmatter
    let mut fm = content::Frontmatter {
        title: title.to_string(),
        tags,
        draft,
        ..Default::default()
    };

    // Build filename
    let filename = if collection.has_date {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        fm.date = Some(chrono::Local::now().date_naive());
        format!("{date}-{slug}.md")
    } else {
        format!("{slug}.md")
    };

    let filepath = paths.content.join(&collection.directory).join(&filename);
    if let Some(parent) = filepath.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| JsonRpcError::internal(format!("Cannot create directory: {e}")))?;
    }

    let frontmatter_str = content::generate_frontmatter(&fm);
    let file_content = format!("{frontmatter_str}\n\n{body}\n");
    fs::write(&filepath, file_content)
        .map_err(|e| JsonRpcError::internal(format!("Cannot write file: {e}")))?;

    let url = if collection.url_prefix.is_empty() {
        format!("/{slug}")
    } else {
        format!("{}/{slug}", collection.url_prefix)
    };

    Ok(serde_json::json!({
        "path": filepath.to_string_lossy(),
        "url": url,
        "slug": slug,
        "collection": collection.name,
    }))
}

// ---------------------------------------------------------------------------
// seite_search
// ---------------------------------------------------------------------------

fn call_search(
    state: &ServerState,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project"))?;
    let paths = state.paths.as_ref().unwrap();

    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'query' parameter"))?;

    let filter_collection = arguments.get("collection").and_then(|v| v.as_str());

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    let collections: Vec<_> = if let Some(name) = filter_collection {
        config
            .collections
            .iter()
            .filter(|c| c.name == name)
            .collect()
    } else {
        config.collections.iter().collect()
    };

    for collection in &collections {
        let dir = paths.content.join(&collection.directory);
        if !dir.exists() {
            continue;
        }

        for entry in WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        {
            if let Ok((fm, body)) = content::parse_content_file(entry.path()) {
                // Match against title, description, tags, and body
                let title_match = fm.title.to_lowercase().contains(&query_lower);
                let desc_match = fm
                    .description
                    .as_ref()
                    .is_some_and(|d| d.to_lowercase().contains(&query_lower));
                let tag_match = fm
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower));
                let body_match = body.to_lowercase().contains(&query_lower);

                if title_match || desc_match || tag_match || body_match {
                    let slug = fm
                        .slug
                        .clone()
                        .unwrap_or_else(|| content::slug_from_title(&fm.title));
                    let url = if collection.url_prefix.is_empty() {
                        format!("/{slug}")
                    } else {
                        format!("{}/{slug}", collection.url_prefix)
                    };

                    // Extract a short excerpt around the match
                    let excerpt = if body_match {
                        extract_excerpt(&body, &query_lower)
                    } else {
                        first_paragraph(&body)
                    };

                    results.push(serde_json::json!({
                        "title": fm.title,
                        "collection": collection.name,
                        "slug": slug,
                        "url": url,
                        "tags": fm.tags,
                        "description": fm.description,
                        "date": fm.date.map(|d| d.to_string()),
                        "draft": fm.draft,
                        "excerpt": excerpt,
                    }));
                }
            }
        }
    }

    // Limit to top 20
    results.truncate(20);

    Ok(serde_json::json!({
        "query": query,
        "count": results.len(),
        "results": results,
    }))
}

/// Extract a ~200 character excerpt around the first occurrence of the query.
fn extract_excerpt(body: &str, query_lower: &str) -> String {
    let body_lower = body.to_lowercase();
    if let Some(pos) = body_lower.find(query_lower) {
        let start = pos.saturating_sub(100);
        let end = (pos + query_lower.len() + 100).min(body.len());
        let mut excerpt = body[start..end].to_string();
        if start > 0 {
            excerpt = format!("...{excerpt}");
        }
        if end < body.len() {
            excerpt = format!("{excerpt}...");
        }
        excerpt
    } else {
        first_paragraph(body)
    }
}

/// Get the first paragraph of markdown content as an excerpt.
fn first_paragraph(body: &str) -> String {
    body.trim()
        .lines()
        .take_while(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

// ---------------------------------------------------------------------------
// seite_apply_theme
// ---------------------------------------------------------------------------

fn call_apply_theme(
    state: &ServerState,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let _config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a page project"))?;

    let name = arguments
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'name' parameter"))?;

    let template_dir = state.cwd.join("templates");
    fs::create_dir_all(&template_dir)
        .map_err(|e| JsonRpcError::internal(format!("Cannot create templates dir: {e}")))?;

    // Check bundled themes first
    if let Some(theme) = themes::by_name(name) {
        fs::write(template_dir.join("base.html"), theme.base_html)
            .map_err(|e| JsonRpcError::internal(format!("Cannot write theme: {e}")))?;
        return Ok(serde_json::json!({
            "applied": true,
            "theme": name,
            "description": theme.description,
            "source": "bundled",
        }));
    }

    // Check installed themes
    if let Some(theme) = themes::installed_by_name(&state.cwd, name) {
        fs::write(template_dir.join("base.html"), &theme.base_html)
            .map_err(|e| JsonRpcError::internal(format!("Cannot write theme: {e}")))?;
        return Ok(serde_json::json!({
            "applied": true,
            "theme": name,
            "description": theme.description,
            "source": "installed",
        }));
    }

    Ok(serde_json::json!({
        "applied": false,
        "error": format!("Theme not found: {name}. Available bundled themes: default, minimal, dark, docs, brutalist, bento"),
    }))
}

// ---------------------------------------------------------------------------
// seite_lookup_docs
// ---------------------------------------------------------------------------

fn call_lookup_docs(arguments: &serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    let topic = arguments.get("topic").and_then(|v| v.as_str());
    let query = arguments.get("query").and_then(|v| v.as_str());

    // If topic matches a slug, return the full page
    if let Some(topic_slug) = topic {
        if let Some(doc) = crate::docs::by_slug(topic_slug) {
            let body = crate::docs::strip_frontmatter(doc.raw_content);
            return Ok(serde_json::json!({
                "found": true,
                "topic": topic_slug,
                "title": doc.title,
                "description": doc.description,
                "content": body,
            }));
        }
    }

    // Search across all docs by keyword
    if let Some(query_str) = query {
        let query_lower = query_str.to_lowercase();
        let mut results = Vec::new();

        for doc in crate::docs::all() {
            let body = crate::docs::strip_frontmatter(doc.raw_content);

            // Check title and description
            let title_match = doc.title.to_lowercase().contains(&query_lower);
            let desc_match = doc.description.to_lowercase().contains(&query_lower);
            let body_match = body.to_lowercase().contains(&query_lower);

            if title_match || desc_match || body_match {
                // Extract matching sections (split by ## headings)
                let sections = extract_matching_sections(body, &query_lower);
                results.push(serde_json::json!({
                    "slug": doc.slug,
                    "title": doc.title,
                    "description": doc.description,
                    "matched_sections": sections,
                }));
            }
        }

        return Ok(serde_json::json!({
            "query": query_str,
            "count": results.len(),
            "results": results,
        }));
    }

    // Neither topic nor query provided — return the index
    if topic.is_none() && query.is_none() {
        let docs: Vec<serde_json::Value> = crate::docs::all()
            .iter()
            .map(|d| {
                serde_json::json!({
                    "slug": d.slug,
                    "title": d.title,
                    "description": d.description,
                })
            })
            .collect();

        return Ok(serde_json::json!({
            "available_topics": docs,
        }));
    }

    // Topic provided but not found
    Ok(serde_json::json!({
        "found": false,
        "error": format!("Documentation topic not found: {}", topic.unwrap_or("(none)")),
        "available_topics": crate::docs::all().iter().map(|d| d.slug).collect::<Vec<_>>(),
    }))
}

/// Extract sections (split by `## ` headings) that contain the query string.
fn extract_matching_sections(body: &str, query_lower: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current_section = String::new();
    let mut current_heading = String::new();

    for line in body.lines() {
        if line.starts_with("## ") {
            // Check if the previous section matched
            if !current_section.is_empty() && current_section.to_lowercase().contains(query_lower) {
                let section = if current_heading.is_empty() {
                    current_section.trim().to_string()
                } else {
                    format!("{current_heading}\n{}", current_section.trim())
                };
                // Truncate long sections
                if section.len() > 500 {
                    sections.push(format!("{}...", &section[..500]));
                } else {
                    sections.push(section);
                }
            }
            current_heading = line.to_string();
            current_section.clear();
        } else {
            current_section.push_str(line);
            current_section.push('\n');
        }
    }

    // Check final section
    if !current_section.is_empty() && current_section.to_lowercase().contains(query_lower) {
        let section = if current_heading.is_empty() {
            current_section.trim().to_string()
        } else {
            format!("{current_heading}\n{}", current_section.trim())
        };
        if section.len() > 500 {
            sections.push(format!("{}...", &section[..500]));
        } else {
            sections.push(section);
        }
    }

    // Limit to 5 most relevant sections
    sections.truncate(5);
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_returns_all_tools() {
        let result = list().unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"seite_build"));
        assert!(names.contains(&"seite_create_content"));
        assert!(names.contains(&"seite_search"));
        assert!(names.contains(&"seite_apply_theme"));
        assert!(names.contains(&"seite_lookup_docs"));
    }

    #[test]
    fn test_lookup_docs_by_topic() {
        let args = serde_json::json!({ "topic": "configuration" });
        let result = call_lookup_docs(&args).unwrap();
        assert_eq!(result["found"], true);
        assert_eq!(result["title"], "Configuration");
        assert!(result["content"].as_str().unwrap().contains("seite.toml"));
    }

    #[test]
    fn test_lookup_docs_by_query() {
        let args = serde_json::json!({ "query": "deploy" });
        let result = call_lookup_docs(&args).unwrap();
        assert!(result["count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_lookup_docs_topic_not_found() {
        let args = serde_json::json!({ "topic": "nonexistent" });
        let result = call_lookup_docs(&args).unwrap();
        assert_eq!(result["found"], false);
        assert!(result["available_topics"].as_array().is_some());
    }

    #[test]
    fn test_lookup_docs_no_args_returns_index() {
        let args = serde_json::json!({});
        let result = call_lookup_docs(&args).unwrap();
        assert!(result["available_topics"].as_array().is_some());
    }

    #[test]
    fn test_extract_matching_sections() {
        let body = "## Intro\nSome intro text.\n\n## Deploy\nDeploy to GitHub.\n\n## Config\nConfig options.";
        let sections = extract_matching_sections(body, "deploy");
        assert_eq!(sections.len(), 1);
        assert!(sections[0].contains("Deploy"));
    }

    #[test]
    fn test_extract_excerpt() {
        let body = "This is a long text about deploying your site to production servers.";
        let excerpt = extract_excerpt(body, "deploying");
        assert!(excerpt.contains("deploying"));
    }

    #[test]
    fn test_first_paragraph() {
        let body = "First line.\nSecond line.\n\nSecond paragraph.";
        let result = first_paragraph(body);
        assert!(result.contains("First line"));
        assert!(result.contains("Second line"));
        assert!(!result.contains("Second paragraph"));
    }
}
