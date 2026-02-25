pub mod builtins;
pub mod parser;

pub use parser::{ShortcodeCall, ShortcodeKind, ShortcodeValue};

use std::collections::HashSet;
use std::path::Path;

use crate::error::{PageError, Result};

/// Registry of available shortcodes (built-in and user-defined).
///
/// Constructed once per build, holds a dedicated Tera instance for rendering
/// shortcode templates independently of the page template engine.
pub struct ShortcodeRegistry {
    tera: tera::Tera,
    known: HashSet<String>,
}

impl ShortcodeRegistry {
    /// Create a new registry by loading built-in shortcodes and any user-defined
    /// shortcode templates from the given directory (`templates/shortcodes/`).
    pub fn new(shortcodes_dir: &Path) -> Result<Self> {
        let mut tera = tera::Tera::default();
        tera.autoescape_on(vec![]); // disable auto-escaping for shortcode HTML output
        let mut known = HashSet::new();

        // Load built-in shortcodes
        for builtin in builtins::all() {
            let template_name = format!("shortcodes/{}.html", builtin.name);
            tera.add_raw_template(&template_name, builtin.template)
                .map_err(|e| {
                    PageError::Build(format!("built-in shortcode '{}': {e}", builtin.name))
                })?;
            known.insert(builtin.name.to_string());
        }

        // Load user-defined shortcodes (override built-ins with same name)
        if shortcodes_dir.exists() {
            for entry in std::fs::read_dir(shortcodes_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("html") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| {
                            PageError::Build(format!(
                                "invalid shortcode filename: {}",
                                path.display()
                            ))
                        })?
                        .to_string();
                    let content = std::fs::read_to_string(&path)?;
                    let template_name = format!("shortcodes/{}.html", name);
                    tera.add_raw_template(&template_name, &content)
                        .map_err(|e| {
                            PageError::Build(format!(
                                "shortcode template '{}': {e}",
                                path.display()
                            ))
                        })?;
                    known.insert(name);
                }
            }
        }

        Ok(Self { tera, known })
    }

    /// Returns true if there are any registered shortcodes.
    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }

    /// Expand all shortcodes in the given markdown body.
    ///
    /// Inline shortcodes (`{{< >}}`) are replaced with rendered HTML.
    /// Body shortcodes (`{{% %}}`) are replaced with rendered template output
    /// where the `body` variable contains the raw markdown body content.
    ///
    /// Shortcodes inside code blocks are left untouched.
    pub fn expand(
        &self,
        input: &str,
        source_path: &Path,
        page_context: &serde_json::Value,
        site_context: &serde_json::Value,
    ) -> Result<String> {
        let calls = parser::parse_shortcodes(input, source_path)?;

        if calls.is_empty() {
            return Ok(input.to_string());
        }

        // Validate all shortcode names
        for call in &calls {
            if !self.known.contains(&call.name) {
                let mut available: Vec<&str> = self.known.iter().map(|s| s.as_str()).collect();
                available.sort();
                return Err(PageError::Shortcode {
                    path: source_path.to_path_buf(),
                    line: call.line,
                    message: format!(
                        "unknown shortcode `{}`. Available: {}",
                        call.name,
                        available.join(", ")
                    ),
                });
            }
        }

        // Replace spans back-to-front so byte offsets stay valid
        let mut output = input.to_string();
        for call in calls.iter().rev() {
            let rendered = self.render_shortcode(call, source_path, page_context, site_context)?;
            output.replace_range(call.span.0..call.span.1, &rendered);
        }

        Ok(output)
    }

    /// Render a single shortcode call using its Tera template.
    fn render_shortcode(
        &self,
        call: &ShortcodeCall,
        source_path: &Path,
        page_context: &serde_json::Value,
        site_context: &serde_json::Value,
    ) -> Result<String> {
        let template_name = format!("shortcodes/{}.html", call.name);
        let mut ctx = tera::Context::new();

        // Insert all named arguments
        for (key, val) in &call.args {
            ctx.insert(key, &val.to_tera_value());
        }

        // Insert body for body shortcodes
        if let Some(ref body) = call.body {
            ctx.insert("body", body);
        }

        // Insert page and site context
        ctx.insert("page", page_context);
        ctx.insert("site", site_context);

        self.tera
            .render(&template_name, &ctx)
            .map_err(|e| PageError::Shortcode {
                path: source_path.to_path_buf(),
                line: call.line,
                message: format!("rendering shortcode `{}`: {e}", call.name),
            })
    }
}

impl ShortcodeValue {
    /// Convert to a `serde_json::Value` for Tera template rendering.
    pub fn to_tera_value(&self) -> serde_json::Value {
        match self {
            ShortcodeValue::String(s) => serde_json::Value::String(s.clone()),
            ShortcodeValue::Integer(i) => serde_json::json!(i),
            ShortcodeValue::Float(f) => serde_json::json!(f),
            ShortcodeValue::Boolean(b) => serde_json::Value::Bool(*b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_registry() -> ShortcodeRegistry {
        ShortcodeRegistry::new(&PathBuf::from("/nonexistent")).unwrap()
    }

    fn empty_contexts() -> (serde_json::Value, serde_json::Value) {
        (serde_json::json!({}), serde_json::json!({}))
    }

    #[test]
    fn test_expand_youtube_shortcode() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("youtube.com/embed/dQw4w9WgXcQ"));
        assert!(result.contains("video-embed"));
    }

    #[test]
    fn test_expand_vimeo_shortcode() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"{{< vimeo(id="123456") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("player.vimeo.com/video/123456"));
    }

    #[test]
    fn test_expand_gist_shortcode() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"{{< gist(user="octocat", id="abc123") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("gist.github.com/octocat/abc123.js"));
    }

    #[test]
    fn test_expand_callout_body_shortcode() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = "{{% callout(type=\"warning\") %}}\nThis is **important**\n{{% end %}}";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("callout-warning"));
        assert!(result.contains("This is **important**"));
    }

    #[test]
    fn test_expand_figure_shortcode() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"{{< figure(src="/static/img.jpg", caption="A photo", alt="Photo") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("<figure"));
        assert!(result.contains("src=\"/static/img.jpg\""));
        assert!(result.contains("<figcaption>A photo</figcaption>"));
        assert!(result.contains("alt=\"Photo\""));
    }

    #[test]
    fn test_expand_unknown_shortcode_errors() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"{{< nonexistent(x="y") >}}"#;
        let result = registry.expand(input, &PathBuf::from("test.md"), &page, &site);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown shortcode `nonexistent`"));
    }

    #[test]
    fn test_expand_no_shortcodes_returns_input() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = "Just regular markdown.";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_expand_preserves_surrounding_content() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"Before {{< youtube(id="test") >}} after"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.starts_with("Before "));
        assert!(result.ends_with(" after"));
        assert!(result.contains("youtube.com/embed/test"));
    }

    #[test]
    fn test_expand_preserves_code_blocks() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = "```\n{{< youtube(id=\"test\") >}}\n```";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_expand_multiple_shortcodes() {
        let registry = test_registry();
        let (page, site) = empty_contexts();
        let input = r#"{{< youtube(id="a") >}}

{{< vimeo(id="b") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("youtube.com/embed/a"));
        assert!(result.contains("player.vimeo.com/video/b"));
    }

    #[test]
    fn test_user_shortcode_override() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sc_dir = tmp.path().join("shortcodes");
        std::fs::create_dir(&sc_dir).unwrap();
        std::fs::write(sc_dir.join("youtube.html"), "<custom>{{ id }}</custom>").unwrap();

        let registry = ShortcodeRegistry::new(&sc_dir).unwrap();
        let (page, site) = empty_contexts();
        let input = r#"{{< youtube(id="test") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("<custom>test</custom>"));
        assert!(!result.contains("youtube.com"));
    }

    #[test]
    fn test_registry_is_not_empty() {
        let registry = test_registry();
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_shortcode_value_to_tera_value() {
        assert_eq!(
            ShortcodeValue::String("hello".into()).to_tera_value(),
            serde_json::Value::String("hello".into())
        );
        assert_eq!(
            ShortcodeValue::Integer(42).to_tera_value(),
            serde_json::json!(42)
        );
        assert_eq!(
            ShortcodeValue::Float(3.14).to_tera_value(),
            serde_json::json!(3.14)
        );
        assert_eq!(
            ShortcodeValue::Boolean(true).to_tera_value(),
            serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn test_user_defined_shortcode() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sc_dir = tmp.path().join("shortcodes");
        std::fs::create_dir(&sc_dir).unwrap();
        std::fs::write(sc_dir.join("custom.html"), "<div>{{ text }}</div>").unwrap();

        let registry = ShortcodeRegistry::new(&sc_dir).unwrap();
        let (page, site) = empty_contexts();
        let input = r#"{{< custom(text="hello") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("<div>hello</div>"));
    }

    #[test]
    fn test_shortcode_with_page_context() {
        let registry = test_registry();
        let page = serde_json::json!({"title": "My Page"});
        let site = serde_json::json!({"base_url": "https://example.com"});
        // youtube shortcode doesn't use page context, but the rendering shouldn't fail
        let input = r#"{{< youtube(id="abc") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("youtube.com/embed/abc"));
    }

    #[test]
    fn test_expand_contact_form_shortcode() {
        let registry = test_registry();
        let page = serde_json::json!({});
        let site = serde_json::json!({
            "contact": {
                "provider": "formspree",
                "endpoint": "xpznqkdl"
            }
        });
        let input = r#"{{< contact_form() >}}"#;
        let result = registry.expand(input, &PathBuf::from("test.md"), &page, &site);
        // Contact form renders even without provider — it just won't have an action URL
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_registry() {
        // Create a registry with a custom non-existent shortcodes dir that exists but is empty
        let tmp = tempfile::TempDir::new().unwrap();
        let sc_dir = tmp.path().join("shortcodes");
        std::fs::create_dir(&sc_dir).unwrap();

        // The registry still has built-in shortcodes
        let registry = ShortcodeRegistry::new(&sc_dir).unwrap();
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_user_shortcode_non_html_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sc_dir = tmp.path().join("shortcodes");
        std::fs::create_dir(&sc_dir).unwrap();
        std::fs::write(sc_dir.join("readme.txt"), "Not a template").unwrap();
        std::fs::write(sc_dir.join("valid.html"), "<p>{{ text }}</p>").unwrap();

        let registry = ShortcodeRegistry::new(&sc_dir).unwrap();
        // "readme" should not be registered, "valid" should
        let (page, site) = empty_contexts();
        let input = r#"{{< valid(text="ok") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site)
            .unwrap();
        assert!(result.contains("<p>ok</p>"));
    }
}
