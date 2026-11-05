pub mod builtins;
pub mod parser;

pub use parser::{ShortcodeCall, ShortcodeKind, ShortcodeValue};

use std::collections::HashSet;
use std::path::Path;

use crate::diagnostics::Diagnostic;
use crate::error::{PageError, Result};

/// Registry of available shortcodes (built-in and user-defined).
///
/// Constructed once per build, holds a dedicated Tera instance for rendering
/// shortcode templates independently of the page template engine.
pub struct ShortcodeRegistry {
    tera: tera::Tera,
    known: HashSet<String>,
    /// Names defined in `templates/shortcodes/` (they replace built-ins).
    user_defined: HashSet<String>,
}

impl ShortcodeRegistry {
    /// Create a new registry by loading built-in shortcodes and any user-defined
    /// shortcode templates from the given directory (`templates/shortcodes/`).
    pub fn new(shortcodes_dir: &Path) -> Result<Self> {
        let mut tera = tera::Tera::default();
        tera.autoescape_on(vec![]); // disable auto-escaping for shortcode HTML output
        let mut known = HashSet::new();
        let mut user_defined = HashSet::new();

        // Load built-in shortcodes
        for builtin in builtins::all() {
            let template_name = format!("shortcodes/{}.html", builtin.name);
            tera.add_raw_template(&template_name, builtin.template)
                .map_err(|e| {
                    PageError::Build(format!("built-in shortcode '{}': {e}", builtin.name))
                })?;
            known.insert(builtin.name.to_string());
        }

        // Load user-defined shortcodes (override built-ins with same name).
        // A symlinked one is refused (`symlinked-source`, like the build): it
        // could render a file from outside the project into pages. Cycles of
        // includes among them are refused before anything renders
        // (`template-cycle`: Tera would recurse until the stack overflows).
        let mut refused = crate::diagnostics::Diagnostics::new();
        let mut user_sources = std::collections::BTreeMap::new();
        if shortcodes_dir.exists() {
            let mut entries =
                std::fs::read_dir(shortcodes_dir)?.collect::<std::io::Result<Vec<_>>>()?;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let path = entry.path();
                if entry.file_type()?.is_symlink() {
                    refused.push(
                        Diagnostic::error(
                            "symlinked-source",
                            "this file is a symlink: seite does not build from symlinked content or templates (it could publish files from outside the project)",
                        )
                        .with_file(&path)
                        .with_hint("replace the link with a copy of the file"),
                    );
                    continue;
                }
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
                    user_sources.insert(template_name, (path.clone(), content));
                    known.insert(name.clone());
                    user_defined.insert(name);
                }
            }
        }
        // Built-in shortcodes include nothing, so only user ones can cycle.
        refused.extend(crate::templates::cycle_diagnostics(&user_sources, |_| None));
        if !refused.is_empty() {
            return Err(PageError::Diagnostics(refused));
        }
        crate::templates::guard_macro_recursion(&mut tera);

        Ok(Self {
            tera,
            known,
            user_defined,
        })
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
    ///
    /// The `i18n` parameter provides translated UI strings as `{{ t }}` inside
    /// shortcode templates, matching the convention used by page templates.
    pub fn expand(
        &self,
        input: &str,
        source_path: &Path,
        page_context: &serde_json::Value,
        site_context: &serde_json::Value,
        i18n: &serde_json::Value,
    ) -> Result<String> {
        self.expand_diagnostic(input, source_path, page_context, site_context, i18n)
            .map_err(|mut diagnostics| parser::diagnostic_to_error(diagnostics.remove(0)))
    }

    /// Like [`expand`](Self::expand), but reports every unknown shortcode in
    /// the input (codes `shortcode-unknown`, `shortcode-syntax`,
    /// `shortcode-render`) with did-you-mean hints. Lines are relative to
    /// `input`. The returned `Vec` is never empty on `Err`.
    pub fn expand_diagnostic(
        &self,
        input: &str,
        source_path: &Path,
        page_context: &serde_json::Value,
        site_context: &serde_json::Value,
        i18n: &serde_json::Value,
    ) -> std::result::Result<String, Vec<Diagnostic>> {
        // Syntax errors don't stop the scan, so every malformed tag and every
        // unknown name in the file is reported together, in document order.
        let (calls, syntax_errors) = parser::parse_shortcodes_recovering(input, source_path);

        if calls.is_empty() && syntax_errors.is_empty() {
            return Ok(input.to_string());
        }

        let mut problems: Vec<Diagnostic> = syntax_errors;
        problems.extend(
            calls
                .iter()
                .filter(|call| !self.known.contains(&call.name))
                .map(|call| {
                    self.unknown_shortcode(call, source_path)
                        .with_column(crate::diagnostics::line_col_at(input, call.span.0).1)
                }),
        );
        if !problems.is_empty() {
            problems.sort_by_key(|d| (d.line, d.column));
            return Err(problems);
        }

        // Replace spans back-to-front so byte offsets stay valid
        let mut output = input.to_string();
        for call in calls.iter().rev() {
            let rendered = self
                .render_shortcode(call, source_path, page_context, site_context, i18n)
                .map_err(|e| {
                    let message = match e {
                        PageError::Shortcode { message, .. } => message,
                        other => other.to_string(),
                    };
                    vec![Diagnostic::error("shortcode-render", message)
                        .with_file(source_path)
                        .with_line(call.line)]
                })?;
            output.replace_range(call.span.0..call.span.1, &rendered);
        }

        Ok(output)
    }

    /// Warnings for built-in shortcodes whose `seite.toml` section is
    /// missing: `comments()` without `[comments]` (code
    /// `comments-not-configured`) and `newsletter()` without `[newsletter]`
    /// (`newsletter-not-configured`). Those render nothing. Lines are
    /// relative to `input`; a user template of the same name is never
    /// reported.
    pub fn config_warnings(
        &self,
        input: &str,
        source_path: &Path,
        config: &crate::config::SiteConfig,
    ) -> Vec<Diagnostic> {
        let missing_comments = config.comments.is_none();
        let missing_newsletter = config.newsletter.is_none();
        if !(missing_comments || missing_newsletter)
            || !(input.contains("comments") || input.contains("newsletter"))
        {
            return Vec::new();
        }
        let (calls, _) = parser::parse_shortcodes_recovering(input, source_path);
        calls
            .iter()
            .filter(|call| !self.user_defined.contains(&call.name))
            .filter_map(|call| {
                let diagnostic = match call.name.as_str() {
                    "comments" if missing_comments => Diagnostic::warning(
                        "comments-not-configured",
                        "`comments()` renders nothing: seite.toml has no [comments] section",
                    )
                    .with_hint(
                        "add [comments] with provider = \"giscus\" or \"utterances\" (see `seite docs shortcodes`), or remove the shortcode",
                    ),
                    "newsletter" if missing_newsletter => Diagnostic::warning(
                        "newsletter-not-configured",
                        "`newsletter()` renders nothing: seite.toml has no [newsletter] section",
                    )
                    .with_hint(
                        "add [newsletter] with provider = \"buttondown\" or \"kit\" (see `seite docs shortcodes`), or remove the shortcode",
                    ),
                    _ => return None,
                };
                Some(
                    diagnostic
                        .with_file(source_path)
                        .with_line(call.line)
                        .with_column(crate::diagnostics::line_col_at(input, call.span.0).1),
                )
            })
            .collect()
    }

    /// Sorted names of every registered shortcode.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.known.iter().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    fn unknown_shortcode(&self, call: &ShortcodeCall, source_path: &Path) -> Diagnostic {
        let available = self.names();
        let hint = crate::diagnostics::did_you_mean(&call.name, available.iter().copied())
            .unwrap_or_else(|| {
                format!(
                    "available shortcodes: {} (add your own in templates/shortcodes/{}.html)",
                    available.join(", "),
                    call.name
                )
            });
        Diagnostic::error(
            "shortcode-unknown",
            format!(
                "unknown shortcode `{}`. Available: {}",
                call.name,
                available.join(", ")
            ),
        )
        .with_file(source_path)
        .with_line(call.line)
        .with_hint(hint)
    }

    /// Render a single shortcode call using its Tera template.
    fn render_shortcode(
        &self,
        call: &ShortcodeCall,
        source_path: &Path,
        page_context: &serde_json::Value,
        site_context: &serde_json::Value,
        i18n: &serde_json::Value,
    ) -> Result<String> {
        let template_name = format!("shortcodes/{}.html", call.name);
        let mut ctx = tera::Context::new();

        // The page's language as `lang` (like page templates), before the
        // arguments so an explicit `lang=` argument wins.
        if let Some(lang) = page_context.get("lang") {
            ctx.insert("lang", lang);
        }

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

        // Insert i18n translations (matches `{{ t }}` convention in page templates)
        ctx.insert("t", i18n);

        self.tera
            .render(&template_name, &ctx)
            .map_err(|e| PageError::Shortcode {
                path: source_path.to_path_buf(),
                line: call.line,
                message: format!(
                    "rendering shortcode `{}`: {}",
                    call.name,
                    crate::templates::error_chain(&e)
                ),
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

    fn empty_contexts() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
        (
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        )
    }

    #[test]
    fn test_expand_youtube_shortcode() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< youtube(id="dQw4w9WgXcQ") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("youtube.com/embed/dQw4w9WgXcQ"));
        assert!(result.contains("video-embed"));
    }

    #[test]
    fn test_expand_vimeo_shortcode() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< vimeo(id="123456") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("player.vimeo.com/video/123456"));
    }

    #[test]
    fn test_expand_gist_shortcode() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< gist(user="octocat", id="abc123") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("gist.github.com/octocat/abc123.js"));
    }

    #[test]
    fn test_expand_callout_body_shortcode() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = "{{% callout(type=\"warning\") %}}\nThis is **important**\n{{% end %}}";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("callout-warning"));
        assert!(result.contains("This is **important**"));
    }

    #[test]
    fn test_expand_figure_shortcode() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< figure(src="/static/img.jpg", caption="A photo", alt="Photo") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("<figure"));
        assert!(result.contains("src=\"/static/img.jpg\""));
        assert!(result.contains("<figcaption>A photo</figcaption>"));
        assert!(result.contains("alt=\"Photo\""));
    }

    #[test]
    fn test_expand_unknown_shortcode_errors() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< nonexistent(x="y") >}}"#;
        let result = registry.expand(input, &PathBuf::from("test.md"), &page, &site, &i18n);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown shortcode `nonexistent`"));
    }

    #[test]
    fn test_expand_no_shortcodes_returns_input() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = "Just regular markdown.";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_expand_preserves_surrounding_content() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"Before {{< youtube(id="test") >}} after"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.starts_with("Before "));
        assert!(result.ends_with(" after"));
        assert!(result.contains("youtube.com/embed/test"));
    }

    #[test]
    fn test_expand_preserves_code_blocks() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = "```\n{{< youtube(id=\"test\") >}}\n```";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_expand_multiple_shortcodes() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< youtube(id="a") >}}

{{< vimeo(id="b") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
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
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< youtube(id="test") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
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
            ShortcodeValue::Float(2.72).to_tera_value(),
            serde_json::json!(2.72)
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
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< custom(text="hello") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("<div>hello</div>"));
    }

    #[test]
    fn test_shortcode_with_page_context() {
        let registry = test_registry();
        let page = serde_json::json!({"title": "My Page"});
        let site = serde_json::json!({"base_url": "https://example.com"});
        let i18n = serde_json::json!({});
        // youtube shortcode doesn't use page context, but the rendering shouldn't fail
        let input = r#"{{< youtube(id="abc") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
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
        let i18n = serde_json::json!({});
        let input = r#"{{< contact_form() >}}"#;
        let result = registry.expand(input, &PathBuf::from("test.md"), &page, &site, &i18n);
        // Contact form renders even without provider — it just won't have an action URL
        assert!(result.is_ok());
    }

    const HOSTILE: &str = "\"><script>alert(1)</script>";

    #[test]
    fn test_comments_giscus_escapes_every_value() {
        let registry = test_registry();
        let site = serde_json::json!({
            "language": "en",
            "comments": {
                "provider": "giscus", "repo": HOSTILE, "repo_id": HOSTILE,
                "category": HOSTILE, "category_id": HOSTILE, "mapping": HOSTILE,
                "reactions": "1", "theme": HOSTILE, "lang": "", "loading": "lazy"
            }
        });
        let page = serde_json::json!({"lang": HOSTILE});
        let html = registry
            .expand(
                "{{< comments() >}}",
                &PathBuf::from("t.md"),
                &page,
                &site,
                &serde_json::json!({}),
            )
            .unwrap();
        assert!(
            html.contains("<script src=\"https://giscus.app/client.js\""),
            "{html}"
        );
        assert!(html.contains("data-loading=\"lazy\""));
        assert!(html.contains("crossorigin=\"anonymous\" async></script>"));
        assert_eq!(html.matches("<script").count(), 1, "{html}");
        assert!(!html.contains("\"><"), "{html}");
        assert!(
            html.contains("data-repo=\"&quot;&gt;&lt;script&gt;"),
            "{html}"
        );
        // The page language is the giscus fallback.
        assert!(html.contains("data-lang=\"&quot;&gt;"), "{html}");
    }

    #[test]
    fn test_comments_giscus_lang_fallbacks() {
        let registry = test_registry();
        let site = |lang: &str| {
            serde_json::json!({
                "language": "en",
                "comments": {
                    "provider": "giscus", "repo": "o/r", "repo_id": "R_1",
                    "category": "General", "category_id": "DIC_1", "mapping": "pathname",
                    "reactions": "0", "theme": "dark", "lang": lang, "loading": "eager"
                }
            })
        };
        let render = |site: &serde_json::Value, page: &serde_json::Value| {
            registry
                .expand(
                    "{{< comments() >}}",
                    &PathBuf::from("t.md"),
                    page,
                    site,
                    &serde_json::json!({}),
                )
                .unwrap()
        };
        let html = render(&site(""), &serde_json::json!({"lang": "de"}));
        assert!(html.contains("data-lang=\"de\""), "{html}");
        assert!(html.contains("data-reactions-enabled=\"0\""));
        assert!(!html.contains("data-loading"));
        let html = render(&site("fr"), &serde_json::json!({"lang": "de"}));
        assert!(html.contains("data-lang=\"fr\""), "{html}");
        let html = render(&site(""), &serde_json::json!({}));
        assert!(html.contains("data-lang=\"en\""), "{html}");
    }

    #[test]
    fn test_comments_utterances_escapes_every_value() {
        let registry = test_registry();
        let site = serde_json::json!({
            "comments": {
                "provider": "utterances", "repo": HOSTILE, "issue_term": HOSTILE,
                "label": HOSTILE, "theme": HOSTILE
            }
        });
        let html = registry
            .expand(
                "{{< comments() >}}",
                &PathBuf::from("t.md"),
                &serde_json::json!({}),
                &site,
                &serde_json::json!({}),
            )
            .unwrap();
        assert!(
            html.contains("<script src=\"https://utteranc.es/client.js\""),
            "{html}"
        );
        assert_eq!(html.matches("<script").count(), 1, "{html}");
        assert!(!html.contains("\"><"), "{html}");
        assert!(html.contains("label=\"&quot;"));
    }

    #[test]
    fn test_comments_without_config_renders_nothing() {
        let registry = test_registry();
        let empty = serde_json::json!({});
        let html = registry
            .expand(
                "a\n\n{{< comments() >}}\n\n{{< newsletter() >}}\nb",
                &PathBuf::from("t.md"),
                &empty,
                &empty,
                &empty,
            )
            .unwrap();
        assert_eq!(html, "a\n\n\n\n\nb");
    }

    #[test]
    fn test_newsletter_buttondown_form() {
        let registry = test_registry();
        let site = serde_json::json!({"newsletter": {
            "provider": "buttondown",
            "action": "https://buttondown.com/api/emails/embed-subscribe/news",
            "email_field": "email"
        }});
        let t =
            serde_json::json!({"newsletter_email": "E-Mail", "newsletter_submit": "Abonnieren"});
        let html = registry
            .expand(
                "{{< newsletter() >}}",
                &PathBuf::from("t.md"),
                &serde_json::json!({}),
                &site,
                &t,
            )
            .unwrap();
        assert!(html.contains("action=\"https:&#x2F;&#x2F;buttondown.com&#x2F;api&#x2F;emails&#x2F;embed-subscribe&#x2F;news\" method=\"post\""), "{html}");
        assert!(html.contains("<input type=\"email\" id=\"newsletter-email\" name=\"email\" required autocomplete=\"email\""), "{html}");
        assert!(html.contains("<label for=\"newsletter-email\">E-Mail</label>"));
        assert!(html.contains("name=\"embed\" value=\"1\""));
        assert!(html.contains(">Abonnieren</button>"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn test_newsletter_kit_args_are_escaped() {
        let registry = test_registry();
        let site = serde_json::json!({"newsletter": {
            "provider": "kit",
            "action": "https://app.kit.com/forms/123/subscriptions",
            "email_field": "email_address"
        }});
        let input = format!(
            "{{{{< newsletter(title={HOSTILE:?}, description=\"Weekly <b>\", button=\"Join\", id=\"nl-2\") >}}}}"
        );
        let html = registry
            .expand(
                &input,
                &PathBuf::from("t.md"),
                &serde_json::json!({}),
                &site,
                &serde_json::json!({}),
            )
            .unwrap();
        assert!(html.contains("name=\"email_address\""), "{html}");
        assert!(html.contains("id=\"nl-2\""));
        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains("Weekly &lt;b&gt;"));
        assert!(html.contains(">Join</button>"));
        assert!(!html.contains("name=\"embed\""));
        assert!(
            html.contains(">Email address</label>"),
            "English fallback: {html}"
        );
    }

    #[test]
    fn test_config_warnings_for_missing_sections() {
        let registry = test_registry();
        let config: crate::config::SiteConfig = toml::from_str(
            "[site]\ntitle = \"t\"\ndescription = \"\"\nbase_url = \"http://x\"\nlanguage = \"en\"\nauthor = \"\"\n\n[[collections]]\nname = \"posts\"\nlabel = \"Posts\"\ndirectory = \"posts\"\nhas_date = true\nhas_rss = true\nlisted = true\nurl_prefix = \"/posts\"\nnested = false\ndefault_template = \"post.html\"\n",
        )
        .unwrap();
        let input = "intro\n\n{{< comments() >}}\n\n```\n{{< newsletter() >}}\n```\n  {{< newsletter() >}}\n";
        let warnings = registry.config_warnings(input, &PathBuf::from("t.md"), &config);
        let found: Vec<_> = warnings
            .iter()
            .map(|d| (d.code, d.line, d.column))
            .collect();
        assert_eq!(
            found,
            vec![
                ("comments-not-configured", Some(3), Some(1)),
                ("newsletter-not-configured", Some(8), Some(3)),
            ]
        );
        let mut configured = config.clone();
        configured.comments = Some(crate::config::CommentsSection {
            provider: crate::config::CommentsProvider::Utterances,
            repo: "o/r".into(),
            repo_id: None,
            category: None,
            category_id: None,
            mapping: None,
            reactions: None,
            theme: None,
            lang: None,
            loading: None,
            issue_term: None,
            label: None,
        });
        let warnings = registry.config_warnings(input, &PathBuf::from("t.md"), &configured);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "newsletter-not-configured");
    }

    #[test]
    fn test_config_warnings_skip_user_override() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("comments.html"), "<div>mine</div>").unwrap();
        let registry = ShortcodeRegistry::new(tmp.path()).unwrap();
        let config: crate::config::SiteConfig = toml::from_str(
            "[site]\ntitle = \"t\"\ndescription = \"\"\nbase_url = \"http://x\"\nlanguage = \"en\"\nauthor = \"\"\ncollections = []\n",
        )
        .unwrap_or_else(|_| {
            let mut c: crate::config::SiteConfig = toml::from_str("[site]\ntitle = \"t\"\ndescription = \"\"\nbase_url = \"http://x\"\nlanguage = \"en\"\nauthor = \"\"\n[[collections]]\nname = \"p\"\nlabel = \"P\"\ndirectory = \"p\"\nhas_date = false\nhas_rss = false\nlisted = true\nurl_prefix = \"\"\nnested = false\ndefault_template = \"page.html\"\n").unwrap();
            c.collections.clear();
            c
        });
        assert!(registry
            .config_warnings("{{< comments() >}}", &PathBuf::from("t.md"), &config)
            .is_empty());
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
    fn test_callout_uses_i18n_translations() {
        let registry = test_registry();
        let page = serde_json::json!({});
        let site = serde_json::json!({});
        let i18n = serde_json::json!({
            "callout_tip": "Tipp",
            "callout_warning": "Warnung",
            "callout_danger": "Gefahr",
            "callout_info": "Hinweis"
        });
        let input = "{{% callout(type=\"tip\") %}}\nSome text\n{{% end %}}";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(
            result.contains("Tipp"),
            "Expected translated 'Tipp' in: {result}"
        );
        assert!(
            !result.contains(">Tip<"),
            "Should not contain English 'Tip'"
        );
    }

    #[test]
    fn test_callout_title_arg_overrides_i18n() {
        let registry = test_registry();
        let page = serde_json::json!({});
        let site = serde_json::json!({});
        let i18n = serde_json::json!({"callout_tip": "Tipp"});
        let input = "{{% callout(type=\"tip\", title=\"Custom Title\") %}}\nText\n{{% end %}}";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(
            result.contains("Custom Title"),
            "Expected custom title in: {result}"
        );
        assert!(
            !result.contains("Tipp"),
            "i18n should be overridden by title arg"
        );
    }

    #[test]
    fn test_callout_falls_back_to_english_without_i18n() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = "{{% callout(type=\"tip\") %}}\nText\n{{% end %}}";
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(
            result.contains(">Tip<"),
            "Expected English fallback 'Tip' in: {result}"
        );
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
        let (page, site, i18n) = empty_contexts();
        let input = r#"{{< valid(text="ok") >}}"#;
        let result = registry
            .expand(input, &PathBuf::from("test.md"), &page, &site, &i18n)
            .unwrap();
        assert!(result.contains("<p>ok</p>"));
    }

    #[test]
    fn test_expand_diagnostic_reports_every_unknown_with_did_you_mean() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = "{{< youtub(id=\"x\") >}}\n\n{{< zzzzzz() >}}";
        let diags = registry
            .expand_diagnostic(input, &PathBuf::from("t.md"), &page, &site, &i18n)
            .unwrap_err();
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == "shortcode-unknown"));
        assert_eq!(diags[0].line, Some(1));
        assert_eq!(diags[0].hint.as_deref(), Some("did you mean `youtube`?"));
        assert_eq!(diags[1].line, Some(3));
        assert!(diags[1]
            .hint
            .as_ref()
            .unwrap()
            .contains("available shortcodes"));
    }

    #[test]
    fn test_expand_diagnostic_recovers_after_syntax_errors() {
        let registry = test_registry();
        let (page, site, i18n) = empty_contexts();
        let input = "{{< youtube id=\"x\" >}}\n\
                     \n\
                     {{% bad name() %}}\nbody\n{{% end %}}\n\
                     {{< figure(src=) >}}\n\
                     {{% callout(type=\"tip\") %}}\nnever closed\n\
                     {{< zzzzzz() >}}\n\
                     {{< youtube(id=\"ok\") >}}\n";
        let diags = registry
            .expand_diagnostic(input, &PathBuf::from("t.md"), &page, &site, &i18n)
            .unwrap_err();
        let summary: Vec<(&str, Option<usize>)> = diags.iter().map(|d| (d.code, d.line)).collect();
        assert_eq!(
            summary,
            vec![
                ("shortcode-syntax", Some(1)),
                ("shortcode-syntax", Some(3)),
                ("shortcode-syntax", Some(6)),
                ("shortcode-syntax", Some(7)),
                ("shortcode-unknown", Some(9)),
            ],
            "{diags:#?}"
        );
        // Hugo-style hint still attached to the first one.
        assert!(diags[0]
            .hint
            .as_ref()
            .unwrap()
            .contains("youtube(id=\"x\")"));
        assert!(diags[3].message.contains("unclosed body shortcode"));
        // `expand` still reports the first problem.
        let err = registry
            .expand(input, &PathBuf::from("t.md"), &page, &site, &i18n)
            .unwrap_err();
        assert!(err.to_string().contains("youtube"), "{err}");
    }
}
