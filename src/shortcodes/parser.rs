use std::collections::HashMap;
use std::path::Path;

use crate::error::{PageError, Result};

/// The kind of shortcode invocation.
#[derive(Debug, Clone, PartialEq)]
pub enum ShortcodeKind {
    /// `{{< name(args) >}}` — output is raw HTML, not processed as markdown.
    Inline,
    /// `{{% name(args) %}} body {{% end %}}` — body content is processed as markdown.
    Body,
}

/// A typed shortcode argument value.
#[derive(Debug, Clone, PartialEq)]
pub enum ShortcodeValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

/// A parsed shortcode invocation found in markdown content.
#[derive(Debug, Clone)]
pub struct ShortcodeCall {
    /// Shortcode name (e.g., "youtube", "callout").
    pub name: String,
    /// Named arguments.
    pub args: HashMap<String, ShortcodeValue>,
    /// For body shortcodes, the raw body between open and close tags.
    pub body: Option<String>,
    /// Inline or body shortcode.
    pub kind: ShortcodeKind,
    /// Byte offset range `(start, end)` in the source string.
    pub span: (usize, usize),
    /// 1-based line number of the opening delimiter.
    pub line: usize,
}

/// Parse all shortcode invocations from markdown content.
///
/// Skips shortcodes inside fenced code blocks and inline code spans.
/// Returns calls in document order with byte spans for replacement.
pub fn parse_shortcodes(input: &str, source_path: &Path) -> Result<Vec<ShortcodeCall>> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut results = Vec::new();
    let mut pos: usize = 0;
    let mut line: usize = 1;

    // Fenced code block state
    let mut in_fenced_code = false;
    let mut fence_char: u8 = 0;
    let mut fence_len: usize = 0;

    while pos < len {
        let b = bytes[pos];

        // Track line numbers
        if b == b'\n' {
            line += 1;
            pos += 1;

            // Check for fenced code block start/end at beginning of line
            if pos < len {
                let line_start = pos;
                // Skip leading whitespace (up to 3 spaces)
                let mut ws = 0;
                while pos + ws < len && bytes[pos + ws] == b' ' && ws < 3 {
                    ws += 1;
                }
                let check_pos = pos + ws;

                if !in_fenced_code {
                    if let Some((fc, fl)) = detect_fence_start(bytes, check_pos) {
                        in_fenced_code = true;
                        fence_char = fc;
                        fence_len = fl;
                        // Skip to end of line
                        pos = skip_to_eol(bytes, line_start);
                        continue;
                    }
                } else if detect_fence_end(bytes, check_pos, fence_char, fence_len) {
                    in_fenced_code = false;
                    pos = skip_to_eol(bytes, line_start);
                    continue;
                }
            }
            continue;
        }

        // Skip everything inside fenced code blocks
        if in_fenced_code {
            pos += 1;
            continue;
        }

        // Handle inline code spans — skip their content
        if b == b'`' {
            let tick_count = count_char(bytes, pos, b'`');
            if tick_count < 3 || !is_line_start(bytes, pos) {
                // Inline code span (not a fenced block at line start)
                if let Some(end) = find_closing_backticks(bytes, pos + tick_count, tick_count) {
                    // Count newlines inside the code span for line tracking
                    for &ch in &bytes[pos..end + tick_count] {
                        if ch == b'\n' {
                            line += 1;
                        }
                    }
                    pos = end + tick_count;
                    continue;
                }
            }
            // Check for fenced code block at start of line (pos == 0 case)
            if pos == 0 || (pos > 0 && bytes[pos - 1] == b'\n') {
                if let Some((fc, fl)) = detect_fence_start(bytes, pos) {
                    in_fenced_code = true;
                    fence_char = fc;
                    fence_len = fl;
                    pos = skip_to_eol(bytes, pos);
                    continue;
                }
            }
            pos += tick_count;
            continue;
        }

        // Also handle tilde fences at line start
        if b == b'~' && (pos == 0 || bytes[pos - 1] == b'\n') {
            if let Some((fc, fl)) = detect_fence_start(bytes, pos) {
                in_fenced_code = true;
                fence_char = fc;
                fence_len = fl;
                pos = skip_to_eol(bytes, pos);
                continue;
            }
        }

        // Detect shortcode delimiters: {{< or {{%
        if b == b'{' && pos + 3 < len && bytes[pos + 1] == b'{' {
            let kind_byte = bytes[pos + 2];

            if kind_byte == b'<' {
                // Inline shortcode: {{< name(args) >}}
                let start = pos;
                let call_start = pos + 3;
                if let Some(close_offset) = find_inline_close(bytes, call_start) {
                    let call_str = &input[call_start..call_start + close_offset];
                    let (name, args) = parse_call(call_str.trim(), source_path, line)?;
                    let end = call_start + close_offset + 3; // skip past ">}}"
                    results.push(ShortcodeCall {
                        name,
                        args,
                        body: None,
                        kind: ShortcodeKind::Inline,
                        span: (start, end),
                        line,
                    });
                    pos = end;
                    continue;
                }
            } else if kind_byte == b'%' {
                // Possible body shortcode: {{% name(args) %}} ... {{% end %}}
                let start = pos;
                let start_line = line;
                let call_start = pos + 3;
                if let Some(close_offset) = find_body_close(bytes, call_start) {
                    let call_str = &input[call_start..call_start + close_offset];
                    let trimmed = call_str.trim();

                    // Skip stray {{% end %}} tags
                    if trimmed == "end" {
                        pos = call_start + close_offset + 3;
                        continue;
                    }

                    let (name, args) = parse_call(trimmed, source_path, start_line)?;
                    let open_end = call_start + close_offset + 3; // past "%}}"

                    // Find matching {{% end %}}
                    if let Some((body_end_rel, close_end_rel)) = find_end_tag(input, open_end) {
                        let body = &input[open_end..open_end + body_end_rel];
                        let total_end = open_end + close_end_rel;

                        // Count newlines in body for accurate line tracking of future calls
                        for &ch in &bytes[open_end..total_end] {
                            if ch == b'\n' {
                                line += 1;
                            }
                        }

                        results.push(ShortcodeCall {
                            name,
                            args,
                            body: Some(body.trim().to_string()),
                            kind: ShortcodeKind::Body,
                            span: (start, total_end),
                            line: start_line,
                        });
                        pos = total_end;
                        continue;
                    } else {
                        return Err(PageError::Shortcode {
                            path: source_path.to_path_buf(),
                            line: start_line,
                            message: format!(
                                "unclosed body shortcode `{name}`. Expected `{{{{% end %}}}}`."
                            ),
                        });
                    }
                }
            }
        }

        pos += 1;
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Detect a fenced code block opening at the given position.
/// Returns `(fence_char, fence_length)` if found.
fn detect_fence_start(bytes: &[u8], pos: usize) -> Option<(u8, usize)> {
    if pos >= bytes.len() {
        return None;
    }
    let ch = bytes[pos];
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let count = count_char(bytes, pos, ch);
    if count >= 3 {
        Some((ch, count))
    } else {
        None
    }
}

/// Detect a fenced code block closing fence at the given position.
fn detect_fence_end(bytes: &[u8], pos: usize, fence_char: u8, fence_len: usize) -> bool {
    if pos >= bytes.len() || bytes[pos] != fence_char {
        return false;
    }
    let count = count_char(bytes, pos, fence_char);
    if count < fence_len {
        return false;
    }
    // Rest of line should be blank (only whitespace)
    let after = pos + count;
    for &b in &bytes[after..] {
        if b == b'\n' {
            return true;
        }
        if b != b' ' && b != b'\t' {
            return false;
        }
    }
    true // at end of input
}

/// Count consecutive occurrences of `ch` starting at `pos`.
fn count_char(bytes: &[u8], pos: usize, ch: u8) -> usize {
    let mut count = 0;
    while pos + count < bytes.len() && bytes[pos + count] == ch {
        count += 1;
    }
    count
}

/// Check if `pos` is at the start of a line.
fn is_line_start(bytes: &[u8], pos: usize) -> bool {
    pos == 0 || (pos > 0 && bytes[pos - 1] == b'\n')
}

/// Skip to end of current line, returning the position of the `\n` + 1 (or input end).
fn skip_to_eol(bytes: &[u8], pos: usize) -> usize {
    for (i, &b) in bytes.iter().enumerate().skip(pos) {
        if b == b'\n' {
            return i + 1;
        }
    }
    bytes.len()
}

/// Find closing backtick sequence of `count` backticks starting search at `start`.
/// Returns position of the first backtick of the closing sequence.
fn find_closing_backticks(bytes: &[u8], start: usize, count: usize) -> Option<usize> {
    let mut pos = start;
    while pos < bytes.len() {
        if bytes[pos] == b'`' {
            let found = count_char(bytes, pos, b'`');
            if found == count {
                return Some(pos);
            }
            pos += found;
        } else {
            pos += 1;
        }
    }
    None
}

/// Find `>}}` closing an inline shortcode. Returns offset from `start` to the `>`.
fn find_inline_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    while pos + 2 < bytes.len() {
        if bytes[pos] == b'>' && bytes[pos + 1] == b'}' && bytes[pos + 2] == b'}' {
            return Some(pos - start);
        }
        // Don't cross newlines for inline shortcodes — they must be single-line
        if bytes[pos] == b'\n' {
            return None;
        }
        pos += 1;
    }
    None
}

/// Find `%}}` closing a body shortcode open tag. Returns offset from `start` to `%`.
fn find_body_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    while pos + 2 < bytes.len() {
        if bytes[pos] == b'%' && bytes[pos + 1] == b'}' && bytes[pos + 2] == b'}' {
            return Some(pos - start);
        }
        if bytes[pos] == b'\n' {
            return None;
        }
        pos += 1;
    }
    None
}

/// Find `{{% end %}}` in the input starting at `start`.
/// Returns `(body_end_offset, close_end_offset)` relative to `start`:
/// - `body_end_offset`: where the body content ends (start of `{{% end %}}`)
/// - `close_end_offset`: where the closing tag ends (after `%}}`)
fn find_end_tag(input: &str, start: usize) -> Option<(usize, usize)> {
    let bytes = input.as_bytes();
    let mut pos = start;
    while pos + 9 < bytes.len() {
        // Look for {{% end %}}  (10 chars minimum: {{% end %}})
        if bytes[pos] == b'{' && bytes[pos + 1] == b'{' && bytes[pos + 2] == b'%' {
            // Find the matching %}}
            let tag_start = pos;
            let inner_start = pos + 3;
            if let Some(close) = find_body_close(bytes, inner_start) {
                let inner = &input[inner_start..inner_start + close];
                if inner.trim() == "end" {
                    let tag_end = inner_start + close + 3; // past %}}
                    return Some((tag_start - start, tag_end - start));
                }
            }
        }
        pos += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

/// Parse a shortcode call: `name(key="val", num=42, flag=true)`.
/// Returns `(name, args)`.
fn parse_call(
    input: &str,
    source_path: &Path,
    line: usize,
) -> Result<(String, HashMap<String, ShortcodeValue>)> {
    let paren_pos = input.find('(').ok_or_else(|| PageError::Shortcode {
        path: source_path.to_path_buf(),
        line,
        message: format!("invalid shortcode syntax: `{input}`. Expected `name(args...)`"),
    })?;

    let name = input[..paren_pos].trim().to_string();
    if name.is_empty() {
        return Err(PageError::Shortcode {
            path: source_path.to_path_buf(),
            line,
            message: "empty shortcode name".to_string(),
        });
    }

    // Validate name characters
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(PageError::Shortcode {
            path: source_path.to_path_buf(),
            line,
            message: format!(
                "invalid shortcode name `{name}`. Use only alphanumeric, underscore, or hyphen."
            ),
        });
    }

    let close_paren = input.rfind(')').ok_or_else(|| PageError::Shortcode {
        path: source_path.to_path_buf(),
        line,
        message: format!("unclosed parenthesis in shortcode `{name}`"),
    })?;

    let args_str = input[paren_pos + 1..close_paren].trim();
    let args = if args_str.is_empty() {
        HashMap::new()
    } else {
        parse_args(args_str, source_path, line, &name)?
    };

    Ok((name, args))
}

/// Parse a comma-separated list of `key=value` arguments.
fn parse_args(
    input: &str,
    source_path: &Path,
    line: usize,
    shortcode_name: &str,
) -> Result<HashMap<String, ShortcodeValue>> {
    let mut args = HashMap::new();
    let mut pos = 0;
    let bytes = input.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace and commas
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b',') {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }

        // Parse key
        let key_start = pos;
        while pos < bytes.len() && bytes[pos] != b'=' && bytes[pos] != b' ' && bytes[pos] != b',' {
            pos += 1;
        }
        let key = input[key_start..pos].trim().to_string();
        if key.is_empty() {
            break;
        }

        // Skip whitespace
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }

        // Expect '='
        if pos >= bytes.len() || bytes[pos] != b'=' {
            return Err(PageError::Shortcode {
                path: source_path.to_path_buf(),
                line,
                message: format!(
                    "expected `=` after argument `{key}` in shortcode `{shortcode_name}`"
                ),
            });
        }
        pos += 1; // skip '='

        // Skip whitespace
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }

        // Parse value
        if pos >= bytes.len() {
            return Err(PageError::Shortcode {
                path: source_path.to_path_buf(),
                line,
                message: format!(
                    "missing value for argument `{key}` in shortcode `{shortcode_name}`"
                ),
            });
        }

        let (value, consumed) =
            parse_value(&input[pos..], source_path, line, shortcode_name, &key)?;
        pos += consumed;
        args.insert(key, value);
    }

    Ok(args)
}

/// Parse a single argument value starting at the beginning of `input`.
/// Returns `(value, bytes_consumed)`.
fn parse_value(
    input: &str,
    source_path: &Path,
    line: usize,
    shortcode_name: &str,
    key: &str,
) -> Result<(ShortcodeValue, usize)> {
    let bytes = input.as_bytes();

    // String value: "..."
    if bytes[0] == b'"' {
        let mut end = 1;
        while end < bytes.len() {
            if bytes[end] == b'\\' && end + 1 < bytes.len() {
                end += 2; // skip escaped character
                continue;
            }
            if bytes[end] == b'"' {
                let s = &input[1..end];
                // Unescape basic sequences
                let unescaped = s.replace("\\\"", "\"").replace("\\\\", "\\");
                return Ok((ShortcodeValue::String(unescaped), end + 1));
            }
            end += 1;
        }
        return Err(PageError::Shortcode {
            path: source_path.to_path_buf(),
            line,
            message: format!(
                "unclosed string for argument `{key}` in shortcode `{shortcode_name}`"
            ),
        });
    }

    // Boolean: true / false
    if input.starts_with("true") && (input.len() == 4 || !bytes[4].is_ascii_alphanumeric()) {
        return Ok((ShortcodeValue::Boolean(true), 4));
    }
    if input.starts_with("false") && (input.len() == 5 || !bytes[5].is_ascii_alphanumeric()) {
        return Ok((ShortcodeValue::Boolean(false), 5));
    }

    // Numeric value (integer or float)
    let mut end = 0;
    let mut has_dot = false;
    if end < bytes.len() && bytes[end] == b'-' {
        end += 1;
    }
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        if bytes[end] == b'.' {
            if has_dot {
                break;
            }
            has_dot = true;
        }
        end += 1;
    }
    if end > 0 && (end > 1 || bytes[0] != b'-') {
        let num_str = &input[..end];
        if has_dot {
            if let Ok(f) = num_str.parse::<f64>() {
                return Ok((ShortcodeValue::Float(f), end));
            }
        } else if let Ok(i) = num_str.parse::<i64>() {
            return Ok((ShortcodeValue::Integer(i), end));
        }
    }

    Err(PageError::Shortcode {
        path: source_path.to_path_buf(),
        line,
        message: format!(
            "invalid value for argument `{key}` in shortcode `{shortcode_name}`. \
             Expected a quoted string, number, or boolean."
        ),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.md")
    }

    #[test]
    fn test_parse_inline_shortcode() {
        let input = r#"Hello {{< youtube(id="dQw4w9WgXcQ") >}} world"#;
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "youtube");
        assert_eq!(calls[0].kind, ShortcodeKind::Inline);
        assert_eq!(
            calls[0].args.get("id"),
            Some(&ShortcodeValue::String("dQw4w9WgXcQ".into()))
        );
        assert!(calls[0].body.is_none());
    }

    #[test]
    fn test_parse_body_shortcode() {
        let input = "{{% callout(type=\"warning\") %}}\nSome **bold** text\n{{% end %}}";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "callout");
        assert_eq!(calls[0].kind, ShortcodeKind::Body);
        assert_eq!(
            calls[0].args.get("type"),
            Some(&ShortcodeValue::String("warning".into()))
        );
        assert_eq!(calls[0].body.as_deref(), Some("Some **bold** text"));
    }

    #[test]
    fn test_parse_shortcode_in_fenced_code_block_ignored() {
        let input = "```\n{{< youtube(id=\"test\") >}}\n```\n\nReal content.";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_shortcode_in_tilde_fenced_block_ignored() {
        let input = "~~~\n{{< youtube(id=\"test\") >}}\n~~~\n\nReal content.";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_shortcode_in_inline_code_ignored() {
        let input = "Use `{{< youtube(id=\"test\") >}}` for videos.";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_multiple_shortcodes() {
        let input = r#"{{< youtube(id="abc") >}} and {{< vimeo(id="123") >}}"#;
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "youtube");
        assert_eq!(calls[1].name, "vimeo");
    }

    #[test]
    fn test_parse_args_string() {
        let (name, args) = parse_call(r#"test(key="hello world")"#, &test_path(), 1).unwrap();
        assert_eq!(name, "test");
        assert_eq!(
            args.get("key"),
            Some(&ShortcodeValue::String("hello world".into()))
        );
    }

    #[test]
    fn test_parse_args_integer() {
        let (_, args) = parse_call("test(count=42)", &test_path(), 1).unwrap();
        assert_eq!(args.get("count"), Some(&ShortcodeValue::Integer(42)));
    }

    #[test]
    fn test_parse_args_negative_integer() {
        let (_, args) = parse_call("test(offset=-5)", &test_path(), 1).unwrap();
        assert_eq!(args.get("offset"), Some(&ShortcodeValue::Integer(-5)));
    }

    #[test]
    fn test_parse_args_float() {
        let (_, args) = parse_call("test(ratio=1.5)", &test_path(), 1).unwrap();
        assert_eq!(args.get("ratio"), Some(&ShortcodeValue::Float(1.5)));
    }

    #[test]
    fn test_parse_args_boolean() {
        let (_, args) = parse_call("test(autoplay=true, muted=false)", &test_path(), 1).unwrap();
        assert_eq!(args.get("autoplay"), Some(&ShortcodeValue::Boolean(true)));
        assert_eq!(args.get("muted"), Some(&ShortcodeValue::Boolean(false)));
    }

    #[test]
    fn test_parse_args_multiple_mixed() {
        let (_, args) = parse_call(
            r#"embed(id="abc", width=800, autoplay=true)"#,
            &test_path(),
            1,
        )
        .unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args.get("id"), Some(&ShortcodeValue::String("abc".into())));
        assert_eq!(args.get("width"), Some(&ShortcodeValue::Integer(800)));
        assert_eq!(args.get("autoplay"), Some(&ShortcodeValue::Boolean(true)));
    }

    #[test]
    fn test_parse_args_empty() {
        let (name, args) = parse_call("test()", &test_path(), 1).unwrap();
        assert_eq!(name, "test");
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_args_escaped_string() {
        let (_, args) = parse_call(r#"test(text="say \"hello\"")"#, &test_path(), 1).unwrap();
        assert_eq!(
            args.get("text"),
            Some(&ShortcodeValue::String("say \"hello\"".into()))
        );
    }

    #[test]
    fn test_error_unclosed_body_shortcode() {
        let input = "{{% callout(type=\"info\") %}}\nBody without end tag.";
        let result = parse_shortcodes(input, &test_path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unclosed body shortcode"));
    }

    #[test]
    fn test_error_invalid_shortcode_name() {
        let input = "{{< bad name(x=\"y\") >}}";
        let result = parse_shortcodes(input, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_error_unclosed_paren() {
        let input = "{{< test(id=\"x\" >}}";
        let result = parse_shortcodes(input, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_shortcode_after_fenced_code_block() {
        let input = "```\ncode\n```\n\n{{< youtube(id=\"real\") >}}";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "youtube");
    }

    #[test]
    fn test_shortcode_preserves_span() {
        let input = "before {{< test(x=\"y\") >}} after";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 1);
        let span = calls[0].span;
        assert_eq!(&input[span.0..span.1], "{{< test(x=\"y\") >}}");
    }

    #[test]
    fn test_body_shortcode_preserves_span() {
        let input = "before {{% note() %}}\nbody\n{{% end %}} after";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 1);
        let span = calls[0].span;
        assert_eq!(&input[span.0..span.1], "{{% note() %}}\nbody\n{{% end %}}");
    }

    #[test]
    fn test_line_number_tracking() {
        let input = "line 1\nline 2\n{{< test() >}}\nline 4";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls[0].line, 3);
    }

    #[test]
    fn test_no_shortcodes_returns_empty() {
        let input = "Just regular markdown.\n\nNo shortcodes here.";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_fenced_code_at_start_of_file() {
        let input = "```\n{{< test() >}}\n```";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_indented_fence_up_to_3_spaces() {
        let input = "   ```\n{{< test() >}}\n   ```\n{{< real() >}}";
        let calls = parse_shortcodes(input, &test_path()).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "real");
    }
}
