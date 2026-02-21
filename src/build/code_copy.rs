use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::Result;

/// CSS for code block copy buttons.
/// Uses `position: relative` on `<pre>` and absolute positioning for the button.
/// The button appears on hover with adaptive semi-transparent styling that works
/// on both light and dark code block backgrounds.
const CODE_COPY_CSS: &str = r#"pre{position:relative}pre .seite-copy-btn{position:absolute;top:0.5rem;right:0.5rem;padding:0.25rem 0.5rem;font-size:0.75rem;font-family:system-ui,-apple-system,sans-serif;line-height:1.4;border:1px solid rgba(128,128,128,0.3);border-radius:4px;background:rgba(128,128,128,0.15);color:rgba(200,200,200,0.8);cursor:pointer;opacity:0;transition:opacity 0.2s;z-index:1}pre:hover .seite-copy-btn{opacity:1}pre .seite-copy-btn:hover{background:rgba(128,128,128,0.3);color:rgba(220,220,220,1)}pre .seite-copy-btn.copied{color:#22c55e;border-color:rgba(34,197,94,0.4)}"#;

/// JS that finds all <pre> elements and injects copy buttons at runtime.
const CODE_COPY_JS: &str = r#"document.addEventListener('DOMContentLoaded',function(){document.querySelectorAll('pre').forEach(function(pre){var btn=document.createElement('button');btn.className='seite-copy-btn';btn.textContent='Copy';btn.setAttribute('aria-label','Copy code to clipboard');btn.addEventListener('click',function(){var code=pre.querySelector('code');var text=(code||pre).textContent;navigator.clipboard.writeText(text).then(function(){btn.textContent='Copied!';btn.classList.add('copied');setTimeout(function(){btn.textContent='Copy';btn.classList.remove('copied')},2000)})});pre.appendChild(btn)})});"#;

/// Inject copy-button CSS + JS into a single HTML string, before `</body>`.
pub fn inject_code_copy(html: &str) -> String {
    // Only inject if the page has code blocks
    if !html.contains("<pre") {
        return html.to_string();
    }

    let snippet = format!(
        "<style>{}</style>\n<script>{}</script>",
        CODE_COPY_CSS, CODE_COPY_JS
    );

    if let Some(pos) = html.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + snippet.len() + 2);
        out.push_str(&html[..pos]);
        out.push('\n');
        out.push_str(&snippet);
        out.push('\n');
        out.push_str(&html[pos..]);
        out
    } else {
        html.to_string()
    }
}

/// Walk all `.html` files in the output directory and inject code copy buttons.
pub fn inject_code_copy_into_html_files(output_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|ext| ext == "html")
        })
    {
        let html = fs::read_to_string(entry.path())?;
        if !html.contains("<pre") {
            continue;
        }
        let rewritten = inject_code_copy(&html);
        if rewritten != html {
            fs::write(entry.path(), rewritten)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML_WITH_CODE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Test</title></head>
<body>
<pre style="background-color:#2b303b;"><code><span style="color:#b48ead;">fn</span> main() {}</code></pre>
</body>
</html>"#;

    const HTML_WITHOUT_CODE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Test</title></head>
<body><p>Hello world</p></body>
</html>"#;

    #[test]
    fn test_injects_copy_button_when_pre_exists() {
        let result = inject_code_copy(HTML_WITH_CODE);
        assert!(result.contains("seite-copy-btn"));
        assert!(result.contains("Copy code to clipboard"));
        assert!(result.contains("navigator.clipboard.writeText"));
        // Should be before </body>
        let body_end = result.find("</body>").unwrap();
        let script_pos = result.find("seite-copy-btn").unwrap();
        assert!(script_pos < body_end);
    }

    #[test]
    fn test_skips_pages_without_code_blocks() {
        let result = inject_code_copy(HTML_WITHOUT_CODE);
        assert_eq!(result, HTML_WITHOUT_CODE);
        assert!(!result.contains("seite-copy-btn"));
    }

    #[test]
    fn test_no_body_tag_unchanged() {
        let no_body = "<html><head><title>T</title></head><pre><code>x</code></pre></html>";
        let result = inject_code_copy(no_body);
        assert_eq!(result, no_body);
    }

    #[test]
    fn test_plain_code_block() {
        let html = r#"<!DOCTYPE html>
<html><head><title>T</title></head>
<body><pre><code>plain text here</code></pre></body></html>"#;
        let result = inject_code_copy(html);
        assert!(result.contains("seite-copy-btn"));
    }

    #[test]
    fn test_css_and_js_both_present() {
        let result = inject_code_copy(HTML_WITH_CODE);
        assert!(result.contains("<style>"));
        assert!(result.contains("<script>"));
        assert!(result.contains("position:relative"));
        assert!(result.contains("DOMContentLoaded"));
    }
}
