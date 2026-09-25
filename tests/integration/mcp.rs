use super::common::*;

#[test]
fn test_mcp_initialize() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    assert_eq!(resp["id"], 1);
    assert!(resp["error"].is_null());
    let result = &resp["result"];
    assert_eq!(result["serverInfo"]["name"], "seite");
    assert!(result["capabilities"]["resources"].is_object());
    assert!(result["capabilities"]["tools"].is_object());
    // No requested version: the newest initialize-based revision.
    assert_eq!(result["protocolVersion"], "2025-11-25");
    assert!(result["instructions"].is_string());
}

#[test]
fn test_mcp_ping() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "ping",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 42);
    assert!(responses[0]["error"].is_null());
}

#[test]
fn test_mcp_unknown_method() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "bogus/method",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32601); // METHOD_NOT_FOUND
}

#[test]
fn test_mcp_tools_list() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let tools = responses[0]["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 10 && tools.len() <= 12, "{}", tools.len());
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"seite_build"));
    assert!(names.contains(&"seite_create_content"));
    assert!(names.contains(&"seite_search"));
    assert!(names.contains(&"seite_apply_theme"));
    assert!(names.contains(&"seite_lookup_docs"));
}

#[test]
fn test_mcp_resources_list_without_project() {
    // Outside a page project, only docs resources should be listed
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    // Should have docs index + individual doc pages, but no config/content/themes
    assert!(resources.iter().any(|r| r["uri"] == "seite://docs"));
    assert!(!resources.iter().any(|r| r["uri"] == "seite://config"));
    assert!(!resources.iter().any(|r| r["uri"] == "seite://themes"));
}

#[test]
fn test_mcp_resources_list_with_project() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpsite", "MCP Test", "posts,docs,pages");
    let site_dir = tmp.path().join("mcpsite");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    // Should have docs + site-specific resources
    assert!(resources.iter().any(|r| r["uri"] == "seite://docs"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://config"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://content"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://themes"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://mcp-config"));
    // Per-collection resources
    assert!(resources
        .iter()
        .any(|r| r["uri"] == "seite://content/posts"));
    assert!(resources.iter().any(|r| r["uri"] == "seite://content/docs"));
    assert!(resources
        .iter()
        .any(|r| r["uri"] == "seite://content/pages"));
}

#[test]
fn test_mcp_read_docs_index() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://docs" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "seite://docs");
    // Parse the text — should be a JSON array of docs
    let text = contents[0]["text"].as_str().unwrap();
    let docs: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert!(docs.len() >= 10); // We have 13 embedded docs
    assert!(docs.iter().any(|d| d["slug"] == "configuration"));
}

#[test]
fn test_mcp_read_doc_page() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://docs/configuration" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    // Should be markdown content without the leading frontmatter
    assert!(text.contains("seite.toml"));
    // Body should not start with frontmatter delimiters
    assert!(
        !text.starts_with("---\n"),
        "doc body should not start with frontmatter"
    );
}

#[test]
fn test_mcp_read_unknown_resource() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://nonexistent" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32602); // INVALID_PARAMS
}

#[test]
fn test_mcp_read_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpconf", "Config Test", "posts");
    let site_dir = tmp.path().join("mcpconf");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let config: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(config["site"]["title"], "Config Test");
}

#[test]
fn test_mcp_read_content_overview() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpcontent", "Content Test", "posts,docs");
    let site_dir = tmp.path().join("mcpcontent");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let collections: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert!(collections.iter().any(|c| c["name"] == "posts"));
    assert!(collections.iter().any(|c| c["name"] == "docs"));
}

#[test]
fn test_mcp_read_themes() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpthemes", "Theme Test", "posts");
    let site_dir = tmp.path().join("mcpthemes");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://themes" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let themes: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert!(themes.len() >= 6); // 6 bundled themes
    assert!(themes.iter().any(|t| t["name"] == "default"));
    assert!(themes.iter().any(|t| t["name"] == "dark"));
}

#[test]
fn test_mcp_read_mcp_config() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpmcp", "MCP Config Test", "posts");
    let site_dir = tmp.path().join("mcpmcp");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://mcp-config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    assert!(text.contains("mcpServers"));
    assert!(text.contains("seite"));
}

#[test]
fn test_mcp_tool_lookup_docs() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_lookup_docs",
                "arguments": { "topic": "configuration" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["found"], true);
    assert_eq!(result["title"], "Configuration");
}

#[test]
fn test_mcp_tool_lookup_docs_search() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_lookup_docs",
                "arguments": { "query": "deploy" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(result["count"].as_u64().unwrap() > 0);
}

#[test]
fn test_mcp_tool_build() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpbuild", "Build Test", "posts");
    let site_dir = tmp.path().join("mcpbuild");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_build",
                "arguments": {}
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["success"], true);
    // Verify the site was actually built
    assert!(site_dir.join("dist/index.html").exists());
}

#[test]
fn test_mcp_tool_create_content() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpnew", "Create Test", "posts,docs");
    let site_dir = tmp.path().join("mcpnew");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_create_content",
                "arguments": {
                    "collection": "docs",
                    "title": "MCP Test Doc",
                    "body": "This is test content."
                }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["collection"], "docs");
    assert_eq!(result["slug"], "mcp-test-doc");
    assert_eq!(result["url"], "/docs/mcp-test-doc");
    // Verify file was created
    assert!(site_dir.join("content/docs/mcp-test-doc.md").exists());
    let file_content = fs::read_to_string(site_dir.join("content/docs/mcp-test-doc.md")).unwrap();
    assert!(file_content.contains("title: MCP Test Doc"));
    assert!(file_content.contains("This is test content."));
}

#[test]
fn test_mcp_create_content_no_overwrite() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpnoover", "No Overwrite", "posts,docs");
    let site_dir = tmp.path().join("mcpnoover");

    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_create_content",
        serde_json::json!({
            "collection": "docs",
            "title": "Setup",
            "tags": ["keep-me"],
            "body": "Original body."
        }),
    );
    assert!(!is_error, "{text}");
    let created: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(created["path"], "content/docs/setup.md");

    // Same title again: must refuse and leave the file untouched.
    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_create_content",
        serde_json::json!({ "collection": "docs", "title": "Setup", "body": "Clobbered." }),
    );
    assert!(is_error, "second create should fail: {text}");
    assert!(text.contains("already exists"), "{text}");
    let on_disk = fs::read_to_string(site_dir.join("content/docs/setup.md")).unwrap();
    assert!(on_disk.contains("Original body."));
    assert!(on_disk.contains("keep-me"));

    // Title with no slug-able characters is rejected, not written as `.md`.
    let (is_error, _) = mcp_tool_call(
        &site_dir,
        "seite_create_content",
        serde_json::json!({ "collection": "docs", "title": "!!!" }),
    );
    assert!(is_error);
    assert!(!site_dir.join("content/docs/.md").exists());
}

#[test]
fn test_mcp_build_reports_broken_links() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcplinks", "Broken Links", "posts");
    let site_dir = tmp.path().join("mcplinks");
    fs::write(
        site_dir.join("content/posts/2025-01-15-linker.md"),
        "---\ntitle: Linker\n---\n\nSee [gone](/posts/not-a-real-post).\n",
    )
    .unwrap();

    let (is_error, text) = mcp_tool_call(&site_dir, "seite_build", serde_json::json!({}));
    assert!(!is_error, "{text}");
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    let broken = result["broken_links"].as_array().unwrap();
    let entry = broken
        .iter()
        .find(|b| b["target"] == "/posts/not-a-real-post")
        .unwrap_or_else(|| panic!("broken link not reported: {result}"));
    assert!(!entry["sources"].as_array().unwrap().is_empty());
    assert!(result["warnings"].is_array());

    // strict=true turns the same problems into a tool error.
    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_build",
        serde_json::json!({ "strict": true }),
    );
    assert!(is_error);
    assert!(text.contains("/posts/not-a-real-post"), "{text}");
}

#[test]
fn test_mcp_build_reports_template_fallback_warning() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcptplwarn", "Template Warning", "posts");
    let site_dir = tmp.path().join("mcptplwarn");
    fs::create_dir_all(site_dir.join("templates")).unwrap();
    fs::write(
        site_dir.join("templates/base.html"),
        "<html>{% if %}</html>",
    )
    .unwrap();

    let (is_error, text) = mcp_tool_call(&site_dir, "seite_build", serde_json::json!({}));
    assert!(!is_error, "{text}");
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(result["warnings"].as_array().unwrap().iter().any(|w| w
        .as_str()
        .unwrap()
        .contains("failed to parse user templates")));
}

#[test]
fn test_mcp_tool_errors_use_is_error_and_surface_config_errors() {
    let tmp = TempDir::new().unwrap();
    // No site: actionable tool error, not a JSON-RPC error.
    let (is_error, text) = mcp_tool_call(tmp.path(), "seite_build", serde_json::json!({}));
    assert!(is_error);
    assert!(text.contains("seite init"), "{text}");

    // Broken seite.toml: the real load error, not "not a site".
    fs::write(tmp.path().join("seite.toml"), "[site\ntitle = \"x\"\n").unwrap();
    let (is_error, text) = mcp_tool_call(tmp.path(), "seite_build", serde_json::json!({}));
    assert!(is_error);
    assert!(text.contains("Failed to load"), "{text}");
    assert!(!text.contains("Not in a seite project"), "{text}");
}

#[test]
fn test_mcp_finds_project_root_from_subdirectory() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpsub", "Subdir Root", "posts");
    let site_dir = tmp.path().join("mcpsub");
    let (is_error, text) = mcp_tool_call(
        &site_dir.join("content/posts"),
        "seite_search",
        serde_json::json!({ "query": "hello" }),
    );
    assert!(!is_error, "{text}");
}

#[test]
fn test_mcp_apply_theme_rejects_path_traversal() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpevil", "Evil Theme", "posts");
    let site_dir = tmp.path().join("mcpevil");
    let evil = tmp.path().join("evil");
    fs::write(tmp.path().join("evil.tera"), "<html>EVIL</html>").unwrap();

    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_apply_theme",
        serde_json::json!({ "name": evil.to_string_lossy() }),
    );
    assert!(is_error);
    assert!(text.contains("invalid theme name"), "{text}");
    let base = site_dir.join("templates/base.html");
    if base.exists() {
        assert!(!fs::read_to_string(base).unwrap().contains("EVIL"));
    }
}

#[test]
fn test_mcp_tool_search() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpsearch", "Search Test", "posts");
    let site_dir = tmp.path().join("mcpsearch");

    // Create a post with searchable content
    fs::write(
        site_dir.join("content/posts/2025-01-15-rust-guide.md"),
        "---\ntitle: \"Rust Programming Guide\"\ntags:\n  - rust\n  - tutorial\n---\n\nLearn Rust from scratch.\n",
    ).unwrap();

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_search",
                "arguments": { "query": "rust" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(result["total"].as_u64().unwrap() >= 1);
    let results = result["results"].as_array().unwrap();
    assert!(results
        .iter()
        .any(|r| r["title"] == "Rust Programming Guide"));
}

#[test]
fn test_mcp_tool_apply_theme() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcptheme", "Theme Test", "posts");
    let site_dir = tmp.path().join("mcptheme");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_apply_theme",
                "arguments": { "name": "dark" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["applied"], true);
    assert_eq!(result["theme"], "dark");
    assert_eq!(result["source"], "bundled");
    // Verify theme file was written
    assert!(site_dir.join("templates/base.html").exists());
}

#[test]
fn test_mcp_initialize_negotiates_2025_06_18_and_gates_tool_fields() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "it", "version": "1.0" }
                }
            }),
            serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            serde_json::json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/templates/list" }),
            serde_json::json!({ "jsonrpc": "2.0", "id": 4, "method": "prompts/list" }),
        ],
    );
    // The notification gets no response.
    assert_eq!(responses.len(), 4);
    let init = &responses[0]["result"];
    assert_eq!(init["protocolVersion"], "2025-06-18");
    let instructions = init["instructions"].as_str().unwrap();
    assert!(
        instructions.contains("seite_create_content"),
        "{instructions}"
    );
    assert!(instructions.contains("seite_build"), "{instructions}");
    assert!(init["capabilities"].get("prompts").is_none());

    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(tools.len() <= 12);
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert!(format!("mcp__seite__{name}").len() <= 60, "{name}");
        assert!(tool["title"].is_string(), "{name}");
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(tool["annotations"][hint].is_boolean(), "{name} {hint}");
        }
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object", "{name}");
        assert_eq!(schema["additionalProperties"], false, "{name}");
        for key in ["oneOf", "anyOf", "allOf"] {
            assert!(schema.get(key).is_none(), "{name}: top-level {key}");
        }
        for (prop, def) in schema["properties"].as_object().unwrap() {
            assert!(def["type"].is_string(), "{name}.{prop} has no type");
        }
        if let Some(out) = tool.get("outputSchema") {
            assert_eq!(out["type"], "object", "{name}");
        }
    }
    let get_page = tools
        .iter()
        .find(|t| t["name"] == "seite_get_page")
        .unwrap();
    assert_eq!(get_page["annotations"]["readOnlyHint"], true);

    let templates: Vec<&str> = responses[2]["result"]["resourceTemplates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["uriTemplate"].as_str().unwrap())
        .collect();
    assert!(templates.contains(&"seite://content/{collection}"));
    assert!(templates.contains(&"seite://docs/{slug}"));
    assert_eq!(responses[3]["error"]["code"], -32601);
}

#[test]
fn test_mcp_initialize_2024_11_05_omits_newer_fields() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2024-11-05", "capabilities": {} }
            }),
            serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "seite_lookup_docs", "arguments": {} }
            }),
        ],
    );
    assert_eq!(responses[0]["result"]["protocolVersion"], "2024-11-05");
    for tool in responses[1]["result"]["tools"].as_array().unwrap() {
        assert!(tool.get("annotations").is_none());
        assert!(tool.get("outputSchema").is_none());
        assert!(tool.get("title").is_none());
    }
    assert!(responses[2]["result"].get("structuredContent").is_none());
}

#[test]
fn test_mcp_modern_discover_and_stateless_request() {
    let tmp = TempDir::new().unwrap();
    let meta = serde_json::json!({ "io.modelcontextprotocol/protocolVersion": "2026-07-28" });
    let responses = mcp_request(
        tmp.path(),
        &[
            serde_json::json!({
                "jsonrpc": "2.0", "id": "d1", "method": "server/discover",
                "params": { "_meta": meta }
            }),
            serde_json::json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "_meta": meta, "name": "seite_lookup_docs", "arguments": {} }
            }),
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/list",
                "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": "2030-01-01" } }
            }),
        ],
    );
    let discover = &responses[0]["result"];
    assert_eq!(responses[0]["id"], "d1");
    assert_eq!(
        discover["supportedVersions"],
        serde_json::json!(["2026-07-28"])
    );
    assert_eq!(discover["resultType"], "complete");
    assert!(discover["instructions"].is_string());
    let call = &responses[1]["result"];
    assert_eq!(call["resultType"], "complete");
    assert!(call["structuredContent"]["available_topics"].is_array());
    assert_eq!(responses[2]["error"]["code"], -32022);
    assert_eq!(
        responses[2]["error"]["data"]["supported"],
        serde_json::json!(["2026-07-28"])
    );
}

#[test]
fn test_mcp_get_page_nested_doc() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcppage", "Page Test", "posts,docs");
    let site_dir = tmp.path().join("mcppage");
    fs::create_dir_all(site_dir.join("content/docs/guides")).unwrap();
    fs::write(
        site_dir.join("content/docs/guides/setup.md"),
        "---\ntitle: Setup Guide\ndescription: How to set up\n---\n## Install\n\nRun **it**.\n",
    )
    .unwrap();

    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_get_page",
        serde_json::json!({ "url": "/docs/guides/setup" }),
    );
    assert!(!is_error, "{text}");
    let page: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(page["path"], "content/docs/guides/setup.md");
    assert_eq!(page["collection"], "docs");
    assert_eq!(page["slug"], "guides/setup");
    assert_eq!(page["frontmatter"]["description"], "How to set up");
    assert!(page["html"]
        .as_str()
        .unwrap()
        .contains("<strong>it</strong>"));

    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_get_page",
        serde_json::json!({ "path": "../../etc/passwd" }),
    );
    assert!(is_error, "{text}");
}

#[test]
fn test_mcp_update_frontmatter_round_trip() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpfm", "FM Test", "posts");
    let site_dir = tmp.path().join("mcpfm");
    let rel = "content/posts/2026-02-03-hello.md";
    let body = "\nFirst paragraph.\n\n```yaml\n---\nnot: frontmatter\n---\n```\n";
    fs::write(site_dir.join(rel), format!("---\ntitle: Hello\n---{body}")).unwrap();

    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_update_frontmatter",
        serde_json::json!({
            "path": rel,
            "set": { "description": "Updated", "tags": ["a", "b"] }
        }),
    );
    assert!(!is_error, "{text}");
    let out: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(out["changed"], true);
    assert_eq!(out["frontmatter"]["tags"], serde_json::json!(["a", "b"]));
    assert!(fs::read_to_string(site_dir.join(rel))
        .unwrap()
        .ends_with(body));

    // Read it back through get_page: the build sees the new frontmatter.
    let (_, text) = mcp_tool_call(
        &site_dir,
        "seite_get_page",
        serde_json::json!({ "path": rel }),
    );
    let page: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(page["frontmatter"]["description"], "Updated");
    assert_eq!(page["date"], "2026-02-03");

    // Path escapes are refused and leave files untouched.
    let (is_error, text) = mcp_tool_call(
        &site_dir,
        "seite_update_frontmatter",
        serde_json::json!({ "path": "seite.toml", "set": { "title": "x" } }),
    );
    assert!(is_error);
    assert!(text.contains("outside the content directory"), "{text}");
}

#[test]
fn test_mcp_tool_unknown_tool() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "nonexistent_tool",
                "arguments": {}
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32602);
}

#[test]
fn test_mcp_full_session() {
    // Simulate a realistic MCP session: initialize → tools/list → resources/list → lookup docs
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpfull", "Full Session", "posts,docs");
    let site_dir = tmp.path().join("mcpfull");

    let responses = mcp_request(
        &site_dir,
        &[
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "resources/list",
                "params": {}
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "seite_lookup_docs",
                    "arguments": { "topic": "templates" }
                }
            }),
        ],
    );

    // Notifications don't get responses, so we should have 4 responses
    assert_eq!(responses.len(), 4);

    // initialize
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "seite");

    // tools/list
    let tool_count = responses[1]["result"]["tools"].as_array().unwrap().len();
    assert!((10..=12).contains(&tool_count), "{tool_count}");
    assert_eq!(responses[1]["id"], 2);

    // resources/list
    assert_eq!(responses[2]["id"], 3);
    let resources = responses[2]["result"]["resources"].as_array().unwrap();
    assert!(resources.iter().any(|r| r["uri"] == "seite://config"));

    // tools/call (lookup docs)
    assert_eq!(responses[3]["id"], 4);
    let content = responses[3]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["found"], true);
    assert_eq!(result["title"], "Templates & Themes");
}

#[test]
fn test_mcp_parse_error() {
    // Send invalid JSON and verify we get a parse error response
    let tmp = TempDir::new().unwrap();
    let bin = assert_cmd::cargo::cargo_bin!("seite");
    let mut child = std::process::Command::new(bin)
        .arg("mcp")
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn page mcp");

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "this is not valid json").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(response["error"].is_object());
    assert_eq!(response["error"]["code"], -32700); // PARSE_ERROR
}

#[test]
fn test_mcp_config_exposes_analytics() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpanalytics", "Analytics MCP", "posts");
    let site_dir = tmp.path().join("mcpanalytics");

    // Add analytics config
    let config_path = site_dir.join("seite.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(
        "\n[analytics]\nprovider = \"plausible\"\nid = \"example.com\"\ncookie_consent = true\n",
    );
    fs::write(&config_path, config).unwrap();

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let config_json: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(config_json["analytics"]["provider"], "plausible");
    assert_eq!(config_json["analytics"]["id"], "example.com");
    assert_eq!(config_json["analytics"]["cookie_consent"], true);
}

#[test]
fn test_mcp_docs_include_analytics() {
    let tmp = TempDir::new().unwrap();
    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "seite_lookup_docs",
                "arguments": { "topic": "configuration" }
            }
        })],
    );

    assert_eq!(responses.len(), 1);
    let content = responses[0]["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["found"], true);
    let doc_content = result["content"].as_str().unwrap();
    assert!(
        doc_content.contains("[analytics]"),
        "Configuration docs should include the [analytics] section"
    );
    assert!(
        doc_content.contains("cookie_consent"),
        "Configuration docs should document cookie_consent field"
    );
}

// --- MCP resource coverage additions ---

#[test]
fn test_mcp_read_content_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpcoll", "Coll MCP", "posts,docs");
    let site_dir = tmp.path().join("mcpcoll");

    // Create a specific post
    fs::write(
        site_dir.join("content/posts/2025-03-15-test-post.md"),
        "---\ntitle: Test Post\ndate: 2025-03-15\ntags:\n  - test\ndescription: A test post\n---\nHello world\n",
    )
    .unwrap();

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content/posts" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    // Should include the sample post from init + our test post
    assert!(items.iter().any(|i| i["title"] == "Test Post"));
}

#[test]
fn test_mcp_read_content_unknown_collection() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "mcpunkcoll", "Unknown Coll", "posts");
    let site_dir = tmp.path().join("mcpunkcoll");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content/nonexistent" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32602);
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[test]
fn test_mcp_read_config_outside_project() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://config" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("seite project"));
}

#[test]
fn test_mcp_read_content_outside_project() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://content" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
}

#[test]
fn test_mcp_read_themes_outside_project() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://themes" }
        })],
    );

    // Themes should still work (bundled themes are always available)
    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
}

#[test]
fn test_mcp_read_missing_uri_param() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("uri"));
}

#[test]
fn test_mcp_read_doc_unknown_slug() {
    let tmp = TempDir::new().unwrap();

    let responses = mcp_request(
        tmp.path(),
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://docs/nonexistent-page" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert!(responses[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[test]
fn test_mcp_resources_list_with_trust() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "mcptrust");
    let site_dir = tmp.path().join("mcptrust");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list",
            "params": {}
        })],
    );

    assert_eq!(responses.len(), 1);
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    assert!(resources.iter().any(|r| r["uri"] == "seite://trust"));
}

#[test]
fn test_mcp_read_trust_resource() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "mcptrustr");
    let site_dir = tmp.path().join("mcptrustr");

    let responses = mcp_request(
        &site_dir,
        &[serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/read",
            "params": { "uri": "seite://trust" }
        })],
    );

    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_null());
    let contents = responses[0]["result"]["contents"].as_array().unwrap();
    let text = contents[0]["text"].as_str().unwrap();
    let trust: serde_json::Value = serde_json::from_str(text).unwrap();
    // Trust resource should include content_items from init scaffold
    assert!(trust.get("content_items").is_some());
}
