//! Mermaid diagram support.
//!
//! When `[build] mermaid = true`, fenced ```mermaid code blocks are emitted as
//! `<div class="mermaid">…source…</div>` by the markdown renderer (rather than
//! syntax-highlighted or dumped as plain `<pre><code>`), and this module injects
//! a small client-side loader that renders them in the browser.
//!
//! Rendering is client-side because there is no practical pure-Rust Mermaid
//! renderer — Mermaid needs a JS/DOM runtime. The loader is injected only on
//! pages that actually contain a diagram, so diagram-free pages pay nothing.

/// Marker class placed on emitted diagram containers. The loader targets it, and
/// the post-processor uses it to decide whether a page needs the loader at all.
pub const MERMAID_CLASS: &str = "mermaid";

/// Client-side loader: import Mermaid from a CDN and render every `.mermaid`
/// block. `startOnLoad: false` + an explicit `run()` keeps control predictable,
/// and `securityLevel: 'strict'` is set explicitly rather than relying on the
/// library default (which could change across versions). The CDN URL is pinned
/// to an exact version for deterministic, reproducible builds — matching how
/// `math.rs` pins KaTeX. Bump the version here to upgrade Mermaid.
const MERMAID_SCRIPT: &str = r#"<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11.16.0/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' });
await mermaid.run({ querySelector: '.mermaid' });
</script>"#;

/// Inject the Mermaid loader before `</body>`, but only on pages that actually
/// contain a `<div class="mermaid">` block. Pages without diagrams are returned
/// unchanged, so there is zero overhead where Mermaid isn't used.
pub fn inject_mermaid(html: &str) -> String {
    if !html.contains(&format!("class=\"{MERMAID_CLASS}\"")) {
        return html.to_string();
    }

    if let Some(pos) = html.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + MERMAID_SCRIPT.len() + 2);
        out.push_str(&html[..pos]);
        out.push('\n');
        out.push_str(MERMAID_SCRIPT);
        out.push('\n');
        out.push_str(&html[pos..]);
        out
    } else {
        html.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML_WITH_DIAGRAM: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Test</title></head>
<body>
<div class="mermaid">graph TD
A--&gt;B</div>
</body>
</html>"#;

    const HTML_WITHOUT_DIAGRAM: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Test</title></head>
<body><p>Hello world</p></body>
</html>"#;

    #[test]
    fn test_injects_loader_when_diagram_present() {
        let result = inject_mermaid(HTML_WITH_DIAGRAM);
        assert!(result.contains("mermaid.esm.min.mjs"));
        assert!(result.contains("mermaid.run"));
        // Injected before </body>
        let script_pos = result.find("mermaid.esm.min.mjs").unwrap();
        let body_end = result.rfind("</body>").unwrap();
        assert!(script_pos < body_end);
        // Original diagram markup preserved
        assert!(result.contains(r#"<div class="mermaid">graph TD"#));
    }

    #[test]
    fn test_skips_pages_without_diagram() {
        let result = inject_mermaid(HTML_WITHOUT_DIAGRAM);
        assert_eq!(result, HTML_WITHOUT_DIAGRAM);
        assert!(!result.contains("mermaid"));
    }

    #[test]
    fn test_no_body_tag_unchanged() {
        let no_body = r#"<html><head></head><div class="mermaid">graph TD</div></html>"#;
        let result = inject_mermaid(no_body);
        assert_eq!(result, no_body);
    }

    #[test]
    fn test_uses_module_script() {
        let result = inject_mermaid(HTML_WITH_DIAGRAM);
        assert!(result.contains(r#"<script type="module">"#));
    }

    #[test]
    fn test_loader_is_version_pinned_and_strict() {
        // Deterministic builds: the CDN URL must pin an exact version, not a
        // floating major range. Security: strict mode is set explicitly rather
        // than relying on the library default.
        assert!(
            MERMAID_SCRIPT.contains("mermaid@11.16.0"),
            "Mermaid CDN URL should pin an exact version"
        );
        assert!(
            !MERMAID_SCRIPT.contains("mermaid@11/"),
            "Mermaid CDN URL should not use a floating major range"
        );
        assert!(
            MERMAID_SCRIPT.contains("securityLevel: 'strict'"),
            "Mermaid should be initialized with explicit strict security"
        );
    }
}
