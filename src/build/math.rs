//! Math/LaTeX rendering: extract `$inline$` and `$$display$$` blocks from markdown
//! and replace them with KaTeX-rendered HTML before the markdown pipeline runs.
//!
//! This module is only compiled when the `math` feature is enabled.

/// KaTeX CSS CDN URL for inclusion in page `<head>`.
pub const KATEX_CSS_URL: &str = "https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css";

/// Pre-process markdown to render math expressions via KaTeX.
///
/// Scans for `$$...$$` (display) and `$...$` (inline) delimiters, renders each
/// through KaTeX, and replaces them with raw HTML that survives markdown processing.
/// Code blocks and inline code spans are left untouched.
#[cfg(feature = "math")]
pub fn render_math(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let chars: Vec<char> = markdown.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip fenced code blocks (``` or ~~~)
        if i + 2 < len
            && ((chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`')
                || (chars[i] == '~' && chars[i + 1] == '~' && chars[i + 2] == '~'))
        {
            let fence_char = chars[i];
            // Count fence length
            let fence_start = i;
            while i < len && chars[i] == fence_char {
                i += 1;
            }
            let fence_len = i - fence_start;
            // Copy fence + skip to end of info string (rest of line)
            for c in &chars[fence_start..i] {
                result.push(*c);
            }
            while i < len && chars[i] != '\n' {
                result.push(chars[i]);
                i += 1;
            }
            if i < len {
                result.push(chars[i]);
                i += 1;
            }
            // Copy until closing fence
            loop {
                if i >= len {
                    break;
                }
                if chars[i] == fence_char {
                    let close_start = i;
                    let mut close_count = 0;
                    while i < len && chars[i] == fence_char {
                        close_count += 1;
                        i += 1;
                    }
                    for c in &chars[close_start..i] {
                        result.push(*c);
                    }
                    if close_count >= fence_len {
                        break;
                    }
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }

        // Skip inline code spans (`...`)
        if chars[i] == '`' && (i + 1 >= len || chars[i + 1] != '`') {
            result.push('`');
            i += 1;
            while i < len && chars[i] != '`' {
                result.push(chars[i]);
                i += 1;
            }
            if i < len {
                result.push('`');
                i += 1;
            }
            continue;
        }

        // Display math: $$...$$
        if i + 1 < len && chars[i] == '$' && chars[i + 1] == '$' {
            i += 2;
            let expr_start = i;
            while i + 1 < len && !(chars[i] == '$' && chars[i + 1] == '$') {
                i += 1;
            }
            if i + 1 < len {
                let expr: String = chars[expr_start..i].iter().collect();
                i += 2; // skip closing $$
                match render_katex(&expr, true) {
                    Ok(html) => result.push_str(&html),
                    Err(_) => {
                        // On error, preserve the original
                        result.push_str("$$");
                        result.push_str(&expr);
                        result.push_str("$$");
                    }
                }
            } else {
                // Unclosed $$, preserve as-is
                result.push_str("$$");
                let rest: String = chars[expr_start..].iter().collect();
                result.push_str(&rest);
                break;
            }
            continue;
        }

        // Inline math: $...$
        // Must not be preceded by \ (escape) or followed by space (not math)
        if chars[i] == '$' {
            // Check it's not escaped
            let escaped = i > 0 && chars[i - 1] == '\\';
            // Check content is not empty or starting with space
            let has_content =
                i + 1 < len && chars[i + 1] != '$' && chars[i + 1] != ' ' && chars[i + 1] != '\n';

            if !escaped && has_content {
                i += 1;
                let expr_start = i;
                while i < len && chars[i] != '$' && chars[i] != '\n' {
                    i += 1;
                }
                if i < len && chars[i] == '$' {
                    let expr: String = chars[expr_start..i].iter().collect();
                    i += 1; // skip closing $
                    if !expr.is_empty() && !expr.ends_with(' ') {
                        match render_katex(&expr, false) {
                            Ok(html) => result.push_str(&html),
                            Err(_) => {
                                result.push('$');
                                result.push_str(&expr);
                                result.push('$');
                            }
                        }
                    } else {
                        // Not valid math (trailing space), preserve
                        result.push('$');
                        result.push_str(&expr);
                        result.push('$');
                    }
                } else {
                    // No closing $, preserve
                    result.push('$');
                    let rest: String = chars[expr_start..i].iter().collect();
                    result.push_str(&rest);
                }
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Render a single math expression to HTML via KaTeX.
#[cfg(feature = "math")]
fn render_katex(expr: &str, display_mode: bool) -> Result<String, String> {
    let opts = katex::Opts::builder()
        .display_mode(display_mode)
        .output_type(katex::OutputType::HtmlAndMathml)
        .trust(true)
        .build()
        .map_err(|e| format!("KaTeX options error: {e}"))?;

    katex::render_with_opts(expr, &opts).map_err(|e| format!("KaTeX render error: {e}"))
}

/// Stub when math feature is not enabled — returns input unchanged.
#[cfg(not(feature = "math"))]
pub fn render_math(markdown: &str) -> String {
    markdown.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_math_passthrough_no_math() {
        let input = "Hello world, no math here.";
        let output = render_math(input);
        assert_eq!(output, input);
    }

    #[cfg(feature = "math")]
    #[test]
    fn test_render_math_inline() {
        let input = "The formula $E=mc^2$ is famous.";
        let output = render_math(input);
        assert!(
            output.contains("katex"),
            "should contain KaTeX HTML: {output}"
        );
        assert!(!output.contains("$E=mc^2$"), "should not contain raw math");
    }

    #[cfg(feature = "math")]
    #[test]
    fn test_render_math_display() {
        let input = "Here is a display equation:\n\n$$\\int_0^1 x^2 dx$$\n\nEnd.";
        let output = render_math(input);
        assert!(
            output.contains("katex"),
            "should contain KaTeX HTML: {output}"
        );
        assert!(!output.contains("$$"), "should not contain raw $$");
    }

    #[test]
    fn test_render_math_skips_code_blocks() {
        let input = "```\n$not math$\n```\n\nBut $x+1$ is.";
        let output = render_math(input);
        // The code block content should be preserved
        assert!(output.contains("$not math$"));
    }

    #[test]
    fn test_render_math_skips_inline_code() {
        let input = "Use `$PATH` variable, and $x^2$ is math.";
        let output = render_math(input);
        assert!(output.contains("`$PATH`"));
    }

    #[test]
    fn test_render_math_preserves_escaped_dollar() {
        let input = r"Price is \$5 and $x$ is math.";
        let output = render_math(input);
        // The \$ should not be treated as math
        assert!(output.contains(r"\$5"));
    }

    #[test]
    fn test_render_math_no_space_after_dollar() {
        let input = "I have $ 5 in my wallet.";
        let output = render_math(input);
        // $ followed by space is not math
        assert_eq!(output, input);
    }

    #[test]
    fn test_render_math_fenced_tildes() {
        let input = "~~~\n$not math$\n~~~\n\nBut $x+1$ is.";
        let output = render_math(input);
        assert!(output.contains("$not math$"));
    }

    #[test]
    fn test_render_math_fenced_with_info_string() {
        let input = "```rust\nlet x = $5;\n```\n";
        let output = render_math(input);
        assert!(output.contains("let x = $5;"));
        assert!(output.contains("```rust"));
    }

    #[test]
    fn test_render_math_fenced_longer_closing() {
        // 4 backticks can close a 3-backtick fence
        let input = "```\n$skip$\n````\n";
        let output = render_math(input);
        assert!(output.contains("$skip$"));
    }

    #[test]
    fn test_render_math_fenced_inner_shorter_fence() {
        // Shorter fence markers inside should NOT close the block
        let input = "````\ninner ``` text $x$\n````\n";
        let output = render_math(input);
        assert!(output.contains("inner ``` text $x$"));
    }

    #[test]
    fn test_render_math_unclosed_display() {
        let input = "Text $$E=mc^2 no close";
        let output = render_math(input);
        // Unclosed $$ preserved as-is
        assert!(output.contains("$$E=mc^2 no close"));
    }

    #[test]
    fn test_render_math_unclosed_inline() {
        let input = "The formula $E=mc^2";
        let output = render_math(input);
        // No closing $ — preserved as literal
        assert!(output.contains("$E=mc^2"));
    }

    #[test]
    fn test_render_math_inline_trailing_space() {
        let input = "Here $trailing $ end";
        let output = render_math(input);
        // Trailing space = not valid math, preserved
        assert!(output.contains("$trailing $"));
    }

    #[test]
    fn test_render_math_dollar_followed_by_newline() {
        let input = "Price $\nnot math";
        let output = render_math(input);
        // $ followed by newline is not math
        assert_eq!(output, input);
    }

    #[test]
    fn test_render_math_inline_hits_newline_before_close() {
        let input = "Start $x+\ny$ end";
        let output = render_math(input);
        // Inline math doesn't span lines — no closing $ found on same line
        assert!(output.contains("$x+"));
    }

    #[test]
    fn test_render_math_fenced_eof_inside_fence() {
        let input = "```\nunclosed fence with $math$";
        let output = render_math(input);
        // EOF inside fence — everything preserved
        assert!(output.contains("$math$"));
    }

    #[cfg(feature = "math")]
    #[test]
    fn test_render_math_display_multiline() {
        let input = "$$\nE = mc^2\n$$";
        let output = render_math(input);
        assert!(
            output.contains("katex"),
            "display math should span lines: {output}"
        );
        assert!(!output.contains("$$"));
    }
}
