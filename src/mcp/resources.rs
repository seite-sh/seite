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
        "uri": "seite://docs",
        "name": "Documentation Index",
        "description": "List of all page documentation pages",
        "mimeType": "application/json"
    }));
    for doc in crate::docs::all() {
        resources.push(serde_json::json!({
            "uri": format!("seite://docs/{}", doc.slug),
            "name": doc.title,
            "description": doc.description,
            "mimeType": "text/markdown"
        }));
    }

    // Site-specific resources (only when seite.toml exists)
    if let Some(ref config) = state.config {
        resources.push(serde_json::json!({
            "uri": "seite://config",
            "name": "Site Configuration",
            "description": "Current seite.toml configuration as JSON",
            "mimeType": "application/json"
        }));

        resources.push(serde_json::json!({
            "uri": "seite://content",
            "name": "Content Overview",
            "description": "All collections with item counts",
            "mimeType": "application/json"
        }));

        for collection in &config.collections {
            resources.push(serde_json::json!({
                "uri": format!("seite://content/{}", collection.name),
                "name": format!("{} collection", collection.label),
                "description": format!("Content items in the {} collection", collection.name),
                "mimeType": "application/json"
            }));
        }

        resources.push(serde_json::json!({
            "uri": "seite://themes",
            "name": "Themes",
            "description": "Available bundled and installed themes",
            "mimeType": "application/json"
        }));

        // Trust center resource (only when trust collection is configured)
        if config.trust.is_some() || config.collections.iter().any(|c| c.name == "trust") {
            resources.push(serde_json::json!({
                "uri": "seite://trust",
                "name": "Trust Center",
                "description": "Trust center state: certifications, subprocessors, FAQs, and content",
                "mimeType": "application/json"
            }));
        }

        // MCP configuration
        let mcp_config_path = state.cwd.join(".claude/settings.json");
        if mcp_config_path.exists() {
            resources.push(serde_json::json!({
                "uri": "seite://mcp-config",
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
    if uri == "seite://docs" {
        return read_docs_index();
    }
    if let Some(slug) = uri.strip_prefix("seite://docs/") {
        return read_doc(slug);
    }
    if uri == "seite://config" {
        return read_config(state);
    }
    if uri == "seite://content" {
        return read_content_overview(state);
    }
    if let Some(collection) = uri.strip_prefix("seite://content/") {
        return read_collection(state, collection);
    }
    if uri == "seite://themes" {
        return read_themes(state);
    }
    if uri == "seite://trust" {
        return read_trust(state);
    }
    if uri == "seite://mcp-config" {
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
            "uri": "seite://docs",
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
            "uri": format!("seite://docs/{slug}"),
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
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project (no seite.toml)"))?;

    let value = serde_json::to_value(config).map_err(|e| JsonRpcError::internal(e.to_string()))?;
    let text = serde_json::to_string_pretty(&value).unwrap_or_default();

    Ok(serde_json::json!({
        "contents": [{
            "uri": "seite://config",
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
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project"))?;
    let paths = state
        .paths
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("No project paths configured"))?;

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
        let mut entry = serde_json::json!({
            "name": coll.name,
            "label": coll.label,
            "items": count,
            "has_date": coll.has_date,
            "has_rss": coll.has_rss,
            "nested": coll.nested,
            "url_prefix": coll.url_prefix,
        });
        if let Some(ref subdomain) = coll.subdomain {
            entry["subdomain"] = serde_json::json!(subdomain);
            entry["subdomain_url"] = serde_json::json!(config.subdomain_base_url(subdomain));
        }
        if let Some(ref deploy_project) = coll.deploy_project {
            entry["deploy_project"] = serde_json::json!(deploy_project);
        }
        collections.push(entry);
    }

    let text = serde_json::to_string_pretty(&collections).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": "seite://content",
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
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project"))?;
    let paths = state
        .paths
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("No project paths configured"))?;

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
            weight_a.cmp(&weight_b).then_with(|| {
                let title_a = a["title"].as_str().unwrap_or("");
                let title_b = b["title"].as_str().unwrap_or("");
                title_a.cmp(title_b)
            })
        });
    }

    let text = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": format!("seite://content/{collection_name}"),
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
            "uri": "seite://themes",
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

// ---------------------------------------------------------------------------
// Trust center resource
// ---------------------------------------------------------------------------

fn read_trust(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("Not in a seite project"))?;
    let paths = state
        .paths
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("No project paths configured"))?;

    let mut result = serde_json::json!({});

    // Trust config from seite.toml
    if let Some(ref trust) = config.trust {
        result["config"] = serde_json::json!({
            "company": trust.company,
            "frameworks": trust.frameworks,
        });
    }

    // Load trust data files
    let data_dir = paths.data_dir.join("trust");
    if data_dir.exists() {
        if let Ok(certs) = fs::read_to_string(data_dir.join("certifications.yaml")) {
            if let Ok(val) = serde_yaml_ng::from_str::<serde_json::Value>(&certs) {
                result["certifications"] = val;
            }
        }
        if let Ok(subs) = fs::read_to_string(data_dir.join("subprocessors.yaml")) {
            if let Ok(val) = serde_yaml_ng::from_str::<serde_json::Value>(&subs) {
                let count = val.as_array().map(|a| a.len()).unwrap_or(0);
                result["subprocessors"] = serde_json::json!({ "count": count, "items": val });
            }
        }
        if let Ok(faq) = fs::read_to_string(data_dir.join("faq.yaml")) {
            if let Ok(val) = serde_yaml_ng::from_str::<serde_json::Value>(&faq) {
                let count = val.as_array().map(|a| a.len()).unwrap_or(0);
                result["faq"] = serde_json::json!({ "count": count, "items": val });
            }
        }
    }

    // Trust content items
    let trust_dir = paths.content.join("trust");
    if trust_dir.exists() {
        let mut items = Vec::new();
        for entry in WalkDir::new(&trust_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        {
            if let Ok((fm, _body)) = content::parse_content_file(entry.path()) {
                let rel = entry
                    .path()
                    .strip_prefix(&trust_dir)
                    .unwrap_or(entry.path());
                let stem = rel
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("untitled");
                let slug = if let Some(parent) = rel.parent() {
                    let p = parent.to_string_lossy();
                    if p.is_empty() {
                        stem.to_string()
                    } else {
                        format!("{}/{stem}", p.replace('\\', "/"))
                    }
                } else {
                    stem.to_string()
                };
                items.push(serde_json::json!({
                    "title": fm.title,
                    "slug": slug,
                    "url": format!("/trust/{slug}"),
                    "description": fm.description,
                    "weight": fm.weight,
                    "extra": fm.extra,
                }));
            }
        }
        result["content_items"] = serde_json::json!(items);
    }

    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(serde_json::json!({
        "contents": [{
            "uri": "seite://trust",
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
            "uri": "seite://mcp-config",
            "mimeType": "application/json",
            "text": content
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BuildSection, CollectionConfig, DeploySection, SiteConfig, SiteSection, TrustSection,
    };
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    /// Helper: create a minimal SiteConfig with given collections.
    fn make_config(collections: Vec<CollectionConfig>) -> SiteConfig {
        SiteConfig {
            site: SiteSection {
                title: "Test Site".into(),
                description: "A test site".into(),
                base_url: "http://localhost:3000".into(),
                language: "en".into(),
                author: "Tester".into(),
            },
            collections,
            build: BuildSection::default(),
            deploy: DeploySection::default(),
            languages: BTreeMap::new(),
            images: None,
            analytics: None,
            trust: None,
            contact: None,
        }
    }

    /// Helper: create a ServerState with config and paths rooted in the given dir.
    fn make_state(dir: &std::path::Path, config: SiteConfig) -> ServerState {
        let paths = config.resolve_paths(dir);
        ServerState {
            config: Some(config),
            paths: Some(paths),
            cwd: dir.to_path_buf(),
        }
    }

    /// Helper: create a ServerState with no config (not in a seite project).
    fn make_empty_state(dir: &std::path::Path) -> ServerState {
        ServerState {
            config: None,
            paths: None,
            cwd: dir.to_path_buf(),
        }
    }

    /// Helper: write a markdown content file with frontmatter.
    fn write_content(dir: &std::path::Path, rel_path: &str, title: &str, extra_fm: &str) {
        let path = dir.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let content = format!("---\ntitle: \"{title}\"\n{extra_fm}---\n\nBody of {title}.\n");
        fs::write(&path, content).unwrap();
    }

    // -----------------------------------------------------------------------
    // list() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_list_without_config_returns_only_docs() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();

        // Should contain docs index + individual doc pages, nothing else
        assert!(!resources.is_empty());
        // First resource is docs index
        assert_eq!(resources[0]["uri"], "seite://docs");
        assert_eq!(resources[0]["name"], "Documentation Index");
        // All resources should be docs (no config/content/themes)
        for r in resources {
            let uri = r["uri"].as_str().unwrap();
            assert!(
                uri.starts_with("seite://docs"),
                "Expected only docs resources without config, got: {uri}"
            );
        }
    }

    #[test]
    fn test_list_with_config_includes_site_resources() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();

        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();

        assert!(uris.contains(&"seite://docs"));
        assert!(uris.contains(&"seite://config"));
        assert!(uris.contains(&"seite://content"));
        assert!(uris.contains(&"seite://content/posts"));
        assert!(uris.contains(&"seite://themes"));
    }

    #[test]
    fn test_list_with_multiple_collections() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![
            CollectionConfig::preset_posts(),
            CollectionConfig::preset_docs(),
            CollectionConfig::preset_pages(),
        ]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();

        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(uris.contains(&"seite://content/posts"));
        assert!(uris.contains(&"seite://content/docs"));
        assert!(uris.contains(&"seite://content/pages"));
    }

    #[test]
    fn test_list_without_trust_excludes_trust_resource() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();
        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(!uris.contains(&"seite://trust"));
    }

    #[test]
    fn test_list_with_trust_config_includes_trust_resource() {
        let tmp = TempDir::new().unwrap();
        let mut config = make_config(vec![CollectionConfig::preset_posts()]);
        config.trust = Some(TrustSection {
            company: Some("Acme Corp".into()),
            frameworks: vec!["soc2".into()],
        });
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();
        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(uris.contains(&"seite://trust"));
    }

    #[test]
    fn test_list_with_trust_collection_includes_trust_resource() {
        let tmp = TempDir::new().unwrap();
        // Trust section is None, but there's a trust collection
        let mut trust_coll = CollectionConfig::preset_posts();
        trust_coll.name = "trust".into();
        let config = make_config(vec![trust_coll]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();
        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(uris.contains(&"seite://trust"));
    }

    #[test]
    fn test_list_with_mcp_config_file() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("settings.json"), "{}").unwrap();

        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();
        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(uris.contains(&"seite://mcp-config"));
    }

    #[test]
    fn test_list_without_mcp_config_file() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();
        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(!uris.contains(&"seite://mcp-config"));
    }

    // -----------------------------------------------------------------------
    // read() routing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_missing_uri_parameter() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({});
        let err = read(&state, &params).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing 'uri'"));
    }

    #[test]
    fn test_read_uri_not_string() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": 42});
        let err = read(&state, &params).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing 'uri'"));
    }

    #[test]
    fn test_read_unknown_uri() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": "seite://unknown"});
        let err = read(&state, &params).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Unknown resource URI"));
    }

    #[test]
    fn test_read_routes_to_docs_index() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": "seite://docs"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://docs");
        assert_eq!(result["contents"][0]["mimeType"], "application/json");
    }

    #[test]
    fn test_read_routes_to_specific_doc() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        // "getting-started" should be a valid embedded doc slug
        let params = serde_json::json!({"uri": "seite://docs/getting-started"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://docs/getting-started");
        assert_eq!(result["contents"][0]["mimeType"], "text/markdown");
    }

    #[test]
    fn test_read_invalid_doc_slug() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": "seite://docs/nonexistent-page-xyz"});
        let err = read(&state, &params).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("not found"));
    }

    // -----------------------------------------------------------------------
    // read_docs_index() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_docs_index_returns_all_docs() {
        let result = read_docs_index().unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let docs: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();

        // We know there are 16 docs in the binary
        assert_eq!(docs.len(), crate::docs::all().len());

        // Each doc should have slug, title, description, weight
        for doc in &docs {
            assert!(doc["slug"].is_string());
            assert!(doc["title"].is_string());
            assert!(doc["description"].is_string());
            assert!(doc["weight"].is_number());
        }
    }

    #[test]
    fn test_read_docs_index_mime_type() {
        let result = read_docs_index().unwrap();
        assert_eq!(result["contents"][0]["mimeType"], "application/json");
        assert_eq!(result["contents"][0]["uri"], "seite://docs");
    }

    // -----------------------------------------------------------------------
    // read_doc() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_doc_valid_slug() {
        let result = read_doc("getting-started").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        // Body should not start with "---" (frontmatter should be stripped)
        assert!(
            !text.starts_with("---"),
            "Frontmatter should be stripped from doc body"
        );
        // Should contain some actual content
        assert!(!text.is_empty());
    }

    #[test]
    fn test_read_doc_all_embedded_slugs() {
        // Verify every embedded doc is readable
        for doc in crate::docs::all() {
            let result = read_doc(doc.slug);
            assert!(result.is_ok(), "Failed to read embedded doc: {}", doc.slug);
        }
    }

    #[test]
    fn test_read_doc_nonexistent_slug() {
        let err = read_doc("this-slug-does-not-exist").unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("not found"));
    }

    #[test]
    fn test_read_doc_empty_slug() {
        let err = read_doc("").unwrap_err();
        assert_eq!(err.code, -32602);
    }

    // -----------------------------------------------------------------------
    // read_config() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_config_with_config() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let result = read_config(&state).unwrap();

        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["site"]["title"], "Test Site");
        assert_eq!(parsed["site"]["description"], "A test site");
        assert_eq!(result["contents"][0]["mimeType"], "application/json");
        assert_eq!(result["contents"][0]["uri"], "seite://config");
    }

    #[test]
    fn test_read_config_without_config() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let err = read_config(&state).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("seite.toml"));
    }

    #[test]
    fn test_read_config_serializes_collections() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![
            CollectionConfig::preset_posts(),
            CollectionConfig::preset_docs(),
        ]);
        let state = make_state(tmp.path(), config);
        let result = read_config(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let collections = parsed["collections"].as_array().unwrap();
        assert_eq!(collections.len(), 2);
        assert_eq!(collections[0]["name"], "posts");
        assert_eq!(collections[1]["name"], "docs");
    }

    #[test]
    fn test_read_config_with_trust_section() {
        let tmp = TempDir::new().unwrap();
        let mut config = make_config(vec![]);
        config.trust = Some(TrustSection {
            company: Some("Acme Corp".into()),
            frameworks: vec!["soc2".into(), "iso27001".into()],
        });
        let state = make_state(tmp.path(), config);
        let result = read_config(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["trust"]["company"], "Acme Corp");
        let frameworks = parsed["trust"]["frameworks"].as_array().unwrap();
        assert_eq!(frameworks.len(), 2);
    }

    // -----------------------------------------------------------------------
    // read_content_overview() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_content_overview_without_config() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let err = read_content_overview(&state).unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_read_content_overview_with_missing_content_dir() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let result = read_content_overview(&state).unwrap();

        let text = result["contents"][0]["text"].as_str().unwrap();
        let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0]["name"], "posts");
        assert_eq!(collections[0]["items"], 0); // dir doesn't exist
    }

    #[test]
    fn test_read_content_overview_counts_md_files() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        // Create content directory with markdown files
        let posts_dir = tmp.path().join("content").join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        fs::write(posts_dir.join("first.md"), "---\ntitle: First\n---\n").unwrap();
        fs::write(posts_dir.join("second.md"), "---\ntitle: Second\n---\n").unwrap();
        // Non-md file should not be counted
        fs::write(posts_dir.join("readme.txt"), "not markdown").unwrap();

        let result = read_content_overview(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(collections[0]["items"], 2);
    }

    #[test]
    fn test_read_content_overview_includes_collection_metadata() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let result = read_content_overview(&state).unwrap();

        let text = result["contents"][0]["text"].as_str().unwrap();
        let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        let posts = &collections[0];
        assert_eq!(posts["name"], "posts");
        assert_eq!(posts["label"], "Posts");
        assert_eq!(posts["has_date"], true);
        assert_eq!(posts["has_rss"], true);
        assert_eq!(posts["nested"], false);
        assert_eq!(posts["url_prefix"], "/posts");
    }

    #[test]
    fn test_read_content_overview_multiple_collections() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![
            CollectionConfig::preset_posts(),
            CollectionConfig::preset_pages(),
        ]);
        let state = make_state(tmp.path(), config);

        // Create some files for posts but not pages
        let posts_dir = tmp.path().join("content").join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        fs::write(posts_dir.join("hello.md"), "---\ntitle: Hello\n---\n").unwrap();

        let result = read_content_overview(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(collections.len(), 2);
        assert_eq!(collections[0]["items"], 1); // posts
        assert_eq!(collections[1]["items"], 0); // pages (no dir)
    }

    #[test]
    fn test_read_content_overview_with_no_paths() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = ServerState {
            config: Some(config),
            paths: None,
            cwd: tmp.path().to_path_buf(),
        };
        let err = read_content_overview(&state).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("paths"));
    }

    // -----------------------------------------------------------------------
    // read_collection() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_collection_without_config() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let err = read_collection(&state, "posts").unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_read_collection_unknown_collection() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let err = read_collection(&state, "nonexistent").unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Collection not found"));
    }

    #[test]
    fn test_read_collection_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        let result = read_collection(&state, "posts").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert!(items.is_empty());
        assert_eq!(result["contents"][0]["uri"], "seite://content/posts");
    }

    #[test]
    fn test_read_collection_with_content_files() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        let posts_dir = tmp.path().join("content").join("posts");
        write_content(
            &posts_dir,
            "hello-world.md",
            "Hello World",
            "date: 2026-01-15\ntags:\n  - rust\n  - web\n",
        );

        let result = read_collection(&state, "posts").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "Hello World");
        assert_eq!(items[0]["slug"], "hello-world");
        assert_eq!(items[0]["url"], "/posts/hello-world");
        assert_eq!(items[0]["date"], "2026-01-15");
        assert_eq!(items[0]["draft"], false);
        let tags = items[0]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_read_collection_date_sorting_descending() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        let posts_dir = tmp.path().join("content").join("posts");
        write_content(&posts_dir, "old.md", "Old Post", "date: 2025-01-01\n");
        write_content(&posts_dir, "new.md", "New Post", "date: 2026-06-15\n");
        write_content(&posts_dir, "mid.md", "Mid Post", "date: 2025-07-01\n");

        let result = read_collection(&state, "posts").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items.len(), 3);
        // Should be sorted newest first
        assert_eq!(items[0]["title"], "New Post");
        assert_eq!(items[1]["title"], "Mid Post");
        assert_eq!(items[2]["title"], "Old Post");
    }

    #[test]
    fn test_read_collection_weight_sorting() {
        let tmp = TempDir::new().unwrap();
        // Pages collection has has_date = false, so sorts by weight then title
        let config = make_config(vec![CollectionConfig::preset_pages()]);
        let state = make_state(tmp.path(), config);

        let pages_dir = tmp.path().join("content").join("pages");
        write_content(&pages_dir, "about.md", "About", "weight: 2\n");
        write_content(&pages_dir, "contact.md", "Contact", "weight: 1\n");
        write_content(&pages_dir, "faq.md", "FAQ", ""); // no weight

        let result = read_collection(&state, "pages").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items.len(), 3);
        // weight=1 first, weight=2 second, no weight last
        assert_eq!(items[0]["title"], "Contact");
        assert_eq!(items[1]["title"], "About");
        assert_eq!(items[2]["title"], "FAQ");
    }

    #[test]
    fn test_read_collection_weight_sorting_alphabetical_tiebreak() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_pages()]);
        let state = make_state(tmp.path(), config);

        let pages_dir = tmp.path().join("content").join("pages");
        // Two items with no weight — should sort alphabetically by title
        write_content(&pages_dir, "banana.md", "Banana", "");
        write_content(&pages_dir, "apple.md", "Apple", "");

        let result = read_collection(&state, "pages").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items[0]["title"], "Apple");
        assert_eq!(items[1]["title"], "Banana");
    }

    #[test]
    fn test_read_collection_url_with_empty_prefix() {
        let tmp = TempDir::new().unwrap();
        // Pages collection has empty url_prefix
        let config = make_config(vec![CollectionConfig::preset_pages()]);
        let state = make_state(tmp.path(), config);

        let pages_dir = tmp.path().join("content").join("pages");
        write_content(&pages_dir, "about.md", "About", "");

        let result = read_collection(&state, "pages").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        // Empty url_prefix means URL is just /slug
        assert_eq!(items[0]["url"], "/about");
    }

    #[test]
    fn test_read_collection_with_custom_slug() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        let posts_dir = tmp.path().join("content").join("posts");
        write_content(
            &posts_dir,
            "my-file.md",
            "My Post",
            "date: 2026-01-01\nslug: custom-slug\n",
        );

        let result = read_collection(&state, "posts").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items[0]["slug"], "custom-slug");
        assert_eq!(items[0]["url"], "/posts/custom-slug");
    }

    #[test]
    fn test_read_collection_draft_items_included() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        let posts_dir = tmp.path().join("content").join("posts");
        write_content(
            &posts_dir,
            "draft.md",
            "Draft Post",
            "date: 2026-01-01\ndraft: true\n",
        );

        let result = read_collection(&state, "posts").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        // Content listing includes drafts (filtering is at build time)
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["draft"], true);
    }

    #[test]
    fn test_read_collection_with_description_and_weight() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_pages()]);
        let state = make_state(tmp.path(), config);

        let pages_dir = tmp.path().join("content").join("pages");
        write_content(
            &pages_dir,
            "about.md",
            "About",
            "description: \"About this site\"\nweight: 5\n",
        );

        let result = read_collection(&state, "pages").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items[0]["description"], "About this site");
        assert_eq!(items[0]["weight"], 5);
    }

    #[test]
    fn test_read_collection_no_paths() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = ServerState {
            config: Some(config),
            paths: None,
            cwd: tmp.path().to_path_buf(),
        };
        let err = read_collection(&state, "posts").unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("paths"));
    }

    #[test]
    fn test_read_collection_ignores_non_md_files() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);

        let posts_dir = tmp.path().join("content").join("posts");
        fs::create_dir_all(&posts_dir).unwrap();
        write_content(&posts_dir, "valid.md", "Valid", "date: 2026-01-01\n");
        fs::write(posts_dir.join("notes.txt"), "just a text file").unwrap();
        fs::write(posts_dir.join("data.json"), "{}").unwrap();

        let result = read_collection(&state, "posts").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "Valid");
    }

    // -----------------------------------------------------------------------
    // read_themes() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_themes_includes_bundled() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let result = read_themes(&state).unwrap();

        let text = result["contents"][0]["text"].as_str().unwrap();
        let themes: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://themes");
        assert_eq!(result["contents"][0]["mimeType"], "application/json");

        // Should include all 10 bundled themes
        let bundled_names: Vec<&str> = themes
            .iter()
            .filter(|t| t["source"] == "bundled")
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(bundled_names.contains(&"default"));
        assert!(bundled_names.contains(&"minimal"));
        assert!(bundled_names.contains(&"dark"));
        assert!(bundled_names.contains(&"docs"));
        assert!(bundled_names.contains(&"brutalist"));
        assert!(bundled_names.contains(&"bento"));
        assert!(bundled_names.contains(&"landing"));
        assert!(bundled_names.contains(&"terminal"));
        assert!(bundled_names.contains(&"magazine"));
        assert!(bundled_names.contains(&"academic"));
        assert_eq!(bundled_names.len(), 10);
    }

    #[test]
    fn test_read_themes_bundled_have_descriptions() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let result = read_themes(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let themes: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();

        for theme in &themes {
            if theme["source"] == "bundled" {
                assert!(
                    theme["description"].is_string(),
                    "Bundled theme {} should have a description",
                    theme["name"]
                );
                let desc = theme["description"].as_str().unwrap();
                assert!(
                    !desc.is_empty(),
                    "Bundled theme {} should have non-empty description",
                    theme["name"]
                );
            }
        }
    }

    #[test]
    fn test_read_themes_with_installed_theme() {
        let tmp = TempDir::new().unwrap();
        let themes_dir = tmp.path().join("templates").join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(
            themes_dir.join("custom.tera"),
            "{#- theme-description: My custom theme -#}\n<html></html>",
        )
        .unwrap();

        let state = make_empty_state(tmp.path());
        let result = read_themes(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let themes: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();

        let installed: Vec<&serde_json::Value> = themes
            .iter()
            .filter(|t| t["source"] == "installed")
            .collect();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0]["name"], "custom");
        assert_eq!(installed[0]["description"], "My custom theme");
    }

    #[test]
    fn test_read_themes_no_installed_themes_dir() {
        let tmp = TempDir::new().unwrap();
        // No templates/themes dir — should just return bundled themes
        let state = make_empty_state(tmp.path());
        let result = read_themes(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let themes: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();

        let installed_count = themes.iter().filter(|t| t["source"] == "installed").count();
        assert_eq!(installed_count, 0);
    }

    // -----------------------------------------------------------------------
    // read_trust() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_trust_without_config() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let err = read_trust(&state).unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_read_trust_with_trust_config() {
        let tmp = TempDir::new().unwrap();
        let mut config = make_config(vec![]);
        config.trust = Some(TrustSection {
            company: Some("Acme Corp".into()),
            frameworks: vec!["soc2".into(), "iso27001".into()],
        });
        let state = make_state(tmp.path(), config);
        let result = read_trust(&state).unwrap();

        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["config"]["company"], "Acme Corp");
        let frameworks = parsed["config"]["frameworks"].as_array().unwrap();
        assert_eq!(frameworks.len(), 2);
        assert_eq!(result["contents"][0]["uri"], "seite://trust");
    }

    #[test]
    fn test_read_trust_without_trust_section() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);
        let result = read_trust(&state).unwrap();

        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        // No trust config means no "config" key
        assert!(parsed.get("config").is_none());
    }

    #[test]
    fn test_read_trust_with_certifications_data() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        fs::write(
            trust_data_dir.join("certifications.yaml"),
            "- name: SOC 2\n  status: active\n- name: ISO 27001\n  status: pending\n",
        )
        .unwrap();

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let certs = parsed["certifications"].as_array().unwrap();
        assert_eq!(certs.len(), 2);
        assert_eq!(certs[0]["name"], "SOC 2");
    }

    #[test]
    fn test_read_trust_with_subprocessors_data() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        fs::write(
            trust_data_dir.join("subprocessors.yaml"),
            "- name: AWS\n  purpose: Hosting\n- name: Stripe\n  purpose: Payments\n- name: Datadog\n  purpose: Monitoring\n",
        )
        .unwrap();

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["subprocessors"]["count"], 3);
        let items = parsed["subprocessors"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["name"], "AWS");
    }

    #[test]
    fn test_read_trust_with_faq_data() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        fs::write(
            trust_data_dir.join("faq.yaml"),
            "- question: Is data encrypted?\n  answer: Yes\n",
        )
        .unwrap();

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["faq"]["count"], 1);
        let items = parsed["faq"]["items"].as_array().unwrap();
        assert_eq!(items[0]["question"], "Is data encrypted?");
    }

    #[test]
    fn test_read_trust_without_data_dir() {
        let tmp = TempDir::new().unwrap();
        let mut config = make_config(vec![]);
        config.trust = Some(TrustSection {
            company: Some("Test".into()),
            frameworks: vec![],
        });
        let state = make_state(tmp.path(), config);
        // No data/trust dir — should succeed with just config
        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["config"]["company"], "Test");
        assert!(parsed.get("certifications").is_none());
        assert!(parsed.get("subprocessors").is_none());
        assert!(parsed.get("faq").is_none());
    }

    #[test]
    fn test_read_trust_with_content_items() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_content_dir = tmp.path().join("content").join("trust");
        write_content(
            &trust_content_dir,
            "security-overview.md",
            "Security Overview",
            "description: \"Our security practices\"\nweight: 1\n",
        );

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let items = parsed["content_items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "Security Overview");
        assert_eq!(items[0]["slug"], "security-overview");
        assert_eq!(items[0]["url"], "/trust/security-overview");
        assert_eq!(items[0]["description"], "Our security practices");
        assert_eq!(items[0]["weight"], 1);
    }

    #[test]
    fn test_read_trust_with_nested_content() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_content_dir = tmp.path().join("content").join("trust");
        write_content(
            &trust_content_dir,
            "frameworks/soc2.md",
            "SOC 2 Details",
            "weight: 2\n",
        );

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let items = parsed["content_items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["slug"], "frameworks/soc2");
        assert_eq!(items[0]["url"], "/trust/frameworks/soc2");
    }

    #[test]
    fn test_read_trust_no_paths() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = ServerState {
            config: Some(config),
            paths: None,
            cwd: tmp.path().to_path_buf(),
        };
        let err = read_trust(&state).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("paths"));
    }

    #[test]
    fn test_read_trust_with_invalid_yaml_data() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        // Write invalid YAML — should be silently ignored
        fs::write(
            trust_data_dir.join("certifications.yaml"),
            "{{{{ not valid yaml",
        )
        .unwrap();

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        // Invalid yaml is silently skipped
        assert!(parsed.get("certifications").is_none());
    }

    #[test]
    fn test_read_trust_subprocessors_non_array() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        // Write a scalar value instead of an array
        fs::write(trust_data_dir.join("subprocessors.yaml"), "count: 5\n").unwrap();

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        // Non-array value means count defaults to 0
        assert_eq!(parsed["subprocessors"]["count"], 0);
    }

    #[test]
    fn test_read_trust_faq_non_array() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        fs::write(trust_data_dir.join("faq.yaml"), "message: hello\n").unwrap();

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["faq"]["count"], 0);
    }

    // -----------------------------------------------------------------------
    // read_mcp_config() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_mcp_config_with_file() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let settings = serde_json::json!({
            "mcpServers": {
                "seite": {
                    "command": "seite",
                    "args": ["mcp"]
                }
            }
        });
        fs::write(
            claude_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let state = make_empty_state(tmp.path());
        let result = read_mcp_config(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["mcpServers"]["seite"].is_object());
        assert_eq!(result["contents"][0]["uri"], "seite://mcp-config");
        assert_eq!(result["contents"][0]["mimeType"], "application/json");
    }

    #[test]
    fn test_read_mcp_config_missing_file() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let err = read_mcp_config(&state).unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Cannot read"));
    }

    // -----------------------------------------------------------------------
    // read() integration with all URI routes
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_config_via_read() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let params = serde_json::json!({"uri": "seite://config"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://config");
    }

    #[test]
    fn test_read_content_via_read() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let params = serde_json::json!({"uri": "seite://content"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://content");
    }

    #[test]
    fn test_read_collection_via_read() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_posts()]);
        let state = make_state(tmp.path(), config);
        let params = serde_json::json!({"uri": "seite://content/posts"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://content/posts");
    }

    #[test]
    fn test_read_themes_via_read() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": "seite://themes"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://themes");
    }

    #[test]
    fn test_read_trust_via_read() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![]);
        let state = make_state(tmp.path(), config);
        let params = serde_json::json!({"uri": "seite://trust"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://trust");
    }

    #[test]
    fn test_read_mcp_config_via_read() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("settings.json"), "{}").unwrap();

        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": "seite://mcp-config"});
        let result = read(&state, &params).unwrap();
        assert_eq!(result["contents"][0]["uri"], "seite://mcp-config");
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_uri_with_trailing_content() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        // "seite://docs/getting-started/extra" should not match a doc
        let params = serde_json::json!({"uri": "seite://docs/getting-started/extra"});
        let err = read(&state, &params).unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_read_completely_wrong_scheme() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": "https://example.com"});
        let err = read(&state, &params).unwrap_err();
        assert!(err.message.contains("Unknown resource URI"));
    }

    #[test]
    fn test_read_empty_uri() {
        let tmp = TempDir::new().unwrap();
        let state = make_empty_state(tmp.path());
        let params = serde_json::json!({"uri": ""});
        let err = read(&state, &params).unwrap_err();
        assert!(err.message.contains("Unknown resource URI"));
    }

    #[test]
    fn test_list_collection_label_matches_preset() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![
            CollectionConfig::preset_posts(),
            CollectionConfig::preset_docs(),
        ]);
        let state = make_state(tmp.path(), config);
        let result = list(&state).unwrap();
        let resources = result["resources"].as_array().unwrap();

        let posts_resource = resources
            .iter()
            .find(|r| r["uri"] == "seite://content/posts")
            .unwrap();
        assert_eq!(posts_resource["name"], "Posts collection");

        let docs_resource = resources
            .iter()
            .find(|r| r["uri"] == "seite://content/docs")
            .unwrap();
        assert_eq!(docs_resource["name"], "Documentation collection");
    }

    #[test]
    fn test_read_content_overview_counts_nested_md_files() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(vec![CollectionConfig::preset_docs()]);
        let state = make_state(tmp.path(), config);

        let docs_dir = tmp.path().join("content").join("docs");
        write_content(&docs_dir, "intro.md", "Intro", "");
        write_content(&docs_dir, "guides/setup.md", "Setup Guide", "");
        write_content(&docs_dir, "guides/advanced.md", "Advanced Guide", "");

        let result = read_content_overview(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
        assert_eq!(collections[0]["items"], 3);
    }

    #[test]
    fn test_read_trust_all_data_files_together() {
        let tmp = TempDir::new().unwrap();
        let mut config = make_config(vec![]);
        config.trust = Some(TrustSection {
            company: Some("TestCo".into()),
            frameworks: vec!["soc2".into()],
        });
        let state = make_state(tmp.path(), config);

        let trust_data_dir = tmp.path().join("data").join("trust");
        fs::create_dir_all(&trust_data_dir).unwrap();
        fs::write(
            trust_data_dir.join("certifications.yaml"),
            "- name: SOC 2\n",
        )
        .unwrap();
        fs::write(trust_data_dir.join("subprocessors.yaml"), "- name: AWS\n").unwrap();
        fs::write(
            trust_data_dir.join("faq.yaml"),
            "- question: Q1?\n  answer: A1\n",
        )
        .unwrap();

        let trust_content_dir = tmp.path().join("content").join("trust");
        write_content(&trust_content_dir, "overview.md", "Overview", "weight: 1\n");

        let result = read_trust(&state).unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

        assert_eq!(parsed["config"]["company"], "TestCo");
        assert!(parsed["certifications"].is_array());
        assert_eq!(parsed["subprocessors"]["count"], 1);
        assert_eq!(parsed["faq"]["count"], 1);
        assert!(parsed["content_items"].is_array());
    }
}
