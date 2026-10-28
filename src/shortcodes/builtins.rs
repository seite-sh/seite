/// A built-in shortcode template compiled into the binary.
pub struct BuiltinShortcode {
    pub name: &'static str,
    pub template: &'static str,
    /// Whether this shortcode uses body syntax (`{{% name() %}}...{{% end %}}`).
    pub is_body: bool,
}

/// Return all built-in shortcode definitions.
pub fn all() -> Vec<BuiltinShortcode> {
    vec![
        BuiltinShortcode {
            name: "youtube",
            template: include_str!("builtins/youtube.html"),
            is_body: false,
        },
        BuiltinShortcode {
            name: "vimeo",
            template: include_str!("builtins/vimeo.html"),
            is_body: false,
        },
        BuiltinShortcode {
            name: "gist",
            template: include_str!("builtins/gist.html"),
            is_body: false,
        },
        BuiltinShortcode {
            name: "callout",
            template: include_str!("builtins/callout.html"),
            is_body: true,
        },
        BuiltinShortcode {
            name: "figure",
            template: include_str!("builtins/figure.html"),
            is_body: false,
        },
        BuiltinShortcode {
            name: "contact_form",
            template: include_str!("builtins/contact_form.html"),
            is_body: false,
        },
        // Shared with the `seite/comments.html` / `seite/newsletter.html`
        // partials, so the shortcode and a theme include render the same.
        BuiltinShortcode {
            name: "comments",
            template: include_str!("../templates/seite/comments.html"),
            is_body: false,
        },
        BuiltinShortcode {
            name: "newsletter",
            template: include_str!("../templates/seite/newsletter.html"),
            is_body: false,
        },
    ]
}
