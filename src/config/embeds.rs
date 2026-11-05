//! `[comments]` and `[newsletter]`: third-party embeds rendered by the
//! `comments()` / `newsletter()` shortcodes and the `seite/comments.html` /
//! `seite/newsletter.html` partials.
//!
//! `seite.toml` is semi-trusted (a cloned repository's config must not inject
//! script beyond what the provider's own embed does), so every value that
//! reaches HTML is validated here against the provider's format, and the
//! templates attribute-escape it as well.
//!
//! Provider references:
//! - giscus: <https://giscus.app/> (`<script src="https://giscus.app/client.js" data-repo …>`;
//!   mappings `pathname|url|title|og:title`, `data-loading="lazy"`).
//! - utterances: <https://utteranc.es/>, configuration component
//!   <https://github.com/utterance/utterances/blob/master/src/configuration-component.ts>
//!   (`issue-term`, `label`, `theme` values).
//! - Buttondown: <https://docs.buttondown.com/building-your-subscriber-base>, snippet
//!   <https://github.com/buttondown/docs/blob/main/public/code/building-your-subscriber-base/1.html>
//!   (`POST https://buttondown.com/api/emails/embed-subscribe/<username>`, field
//!   `email`, hidden `embed=1`; Buttondown asks for a plain form POST, not `fetch`).
//! - Kit (ConvertKit): `POST https://app.kit.com/forms/<form_id>/subscriptions`,
//!   field `email_address`. UNVERIFIED: Kit publishes no spec page; this is the
//!   shape of the form builder's "HTML" embed output as reported by third
//!   parties (<https://help.kit.com/en/articles/4009572-form-embedding-basics>
//!   only says to copy that code). If your form's embed code has a different
//!   action URL, set `[newsletter] action` to it.

use serde::{Deserialize, Serialize};

use super::SiteConfig;

/// Comment system provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommentsProvider {
    Giscus,
    Utterances,
}

/// `[comments]`: GitHub-backed comments (free).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentsSection {
    pub provider: CommentsProvider,
    /// GitHub repository, `owner/name`.
    pub repo: String,
    /// giscus: repository ID (from giscus.app).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    /// giscus: discussion category name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// giscus: discussion category ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_id: Option<String>,
    /// giscus: page ↔ discussion mapping (default `pathname`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<String>,
    /// giscus: reactions on the main post (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<bool>,
    /// Widget theme (giscus default `preferred_color_scheme`, utterances
    /// default `preferred-color-scheme`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// giscus: UI language (default: the page's language).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// giscus: `lazy` (default) or `eager`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loading: Option<String>,
    /// utterances: page ↔ issue mapping (default `pathname`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_term: Option<String>,
    /// utterances: label added to created issues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Newsletter provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NewsletterProvider {
    Buttondown,
    Kit,
}

/// `[newsletter]`: a subscribe form posting to the provider (free tiers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsletterSection {
    pub provider: NewsletterProvider,
    /// Buttondown: newsletter username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Kit: numeric form ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    /// Kit: the form's `action` URL copied from Kit's embed code, when it
    /// differs from `https://app.kit.com/forms/<form_id>/subscriptions`
    /// (https, on kit.com or convertkit.com).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

const GISCUS_MAPPINGS: &[&str] = &["pathname", "url", "title", "og:title"];
const UTTERANCES_TERMS: &[&str] = &["pathname", "url", "title", "og:title"];
const UTTERANCES_THEMES: &[&str] = &[
    "github-light",
    "github-dark",
    "preferred-color-scheme",
    "github-dark-orange",
    "icy-dark",
    "dark-blue",
    "photon-dark",
    "boxy-light",
    "gruvbox-dark",
];

/// `owner/name` with GitHub's allowed characters.
pub fn is_github_repo(value: &str) -> bool {
    let Some((owner, name)) = value.split_once('/') else {
        return false;
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    };
    ok(owner) && ok(name) && owner != "." && owner != ".." && name != "." && name != ".."
}

/// A provider ID: ASCII letters, digits, `_` or `-`.
fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// A BCP 47-ish language tag (`en`, `pt-BR`, `zh-Hans`).
fn is_lang(value: &str) -> bool {
    let mut parts = value.split('-');
    let primary = parts.next().unwrap_or("");
    (2..=3).contains(&primary.len())
        && primary.bytes().all(|b| b.is_ascii_alphabetic())
        && parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// Free text shown to visitors or sent to a provider (category, label): no
/// control characters or markup characters, bounded length.
fn is_plain_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= 100
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '`'))
}

/// A giscus theme: a built-in name or an `https://` stylesheet URL.
fn is_giscus_theme(value: &str) -> bool {
    if let Some(rest) = value.strip_prefix("https://") {
        return !rest.is_empty()
            && value.len() <= 300
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&()*+,;=%".contains(&b));
    }
    !value.is_empty()
        && value.len() <= 50
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A Kit form action: `https://<host>/<path>` with host `kit.com`,
/// `convertkit.com` or a subdomain of either, and a plain path (letters,
/// digits, `/ _ . -`; no query, fragment or credentials).
pub fn is_kit_action(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host_ok = ["kit.com", "convertkit.com"].iter().any(|d| {
        host == *d
            || host
                .strip_suffix(d)
                .and_then(|h| h.strip_suffix('.'))
                .is_some_and(|sub| {
                    !sub.is_empty()
                        && sub
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
                })
    });
    host_ok
        && value.len() <= 300
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'.' | b'-'))
        && !path.contains("..")
}

/// Validate `[comments]` and `[newsletter]`. The error carries the message
/// and the `[section]key` locator understood by `schema::config_line`.
pub fn validate_config(config: &SiteConfig) -> Result<(), (String, String)> {
    if let Some(c) = &config.comments {
        validate_comments(c)?;
    }
    if let Some(n) = &config.newsletter {
        validate_newsletter(n)?;
    }
    Ok(())
}

fn comments_err(key: &str, message: String) -> Result<(), (String, String)> {
    Err((format!("[comments] {message}"), format!("[comments]{key}")))
}

fn validate_comments(c: &CommentsSection) -> Result<(), (String, String)> {
    if !is_github_repo(&c.repo) {
        return comments_err(
            "repo",
            format!(
                "repo = \"{}\" must be a GitHub repository as `owner/name`",
                c.repo.escape_debug()
            ),
        );
    }
    if let Some(theme) = &c.theme {
        let ok = match c.provider {
            CommentsProvider::Giscus => is_giscus_theme(theme),
            CommentsProvider::Utterances => UTTERANCES_THEMES.contains(&theme.as_str()),
        };
        if !ok {
            let expected = match c.provider {
                CommentsProvider::Giscus => {
                    "a giscus theme name (e.g. preferred_color_scheme, light, dark) or an https:// stylesheet URL".to_string()
                }
                CommentsProvider::Utterances => format!("one of {}", UTTERANCES_THEMES.join(", ")),
            };
            return comments_err(
                "theme",
                format!("theme = \"{}\" must be {expected}", theme.escape_debug()),
            );
        }
    }
    match c.provider {
        CommentsProvider::Giscus => {
            for (key, value) in [("repo_id", &c.repo_id), ("category_id", &c.category_id)] {
                match value {
                    None => {
                        return comments_err(
                            key,
                            format!("giscus needs `{key}` (copy it from https://giscus.app)"),
                        )
                    }
                    Some(v) if !is_id(v) => {
                        return comments_err(
                            key,
                            format!(
                                "{key} = \"{}\" may contain only ASCII letters, digits, '_' or '-'",
                                v.escape_debug()
                            ),
                        )
                    }
                    Some(_) => {}
                }
            }
            match &c.category {
                None => {
                    return comments_err(
                        "category",
                        "giscus needs `category` (the discussion category name)".into(),
                    )
                }
                Some(v) if !is_plain_text(v) => {
                    return comments_err(
                        "category",
                        format!(
                            "category = \"{}\" must be plain text (no control characters, quotes or < >, at most 100 characters)",
                            v.escape_debug()
                        ),
                    )
                }
                Some(_) => {}
            }
            if let Some(m) = &c.mapping {
                if !GISCUS_MAPPINGS.contains(&m.as_str()) {
                    return comments_err(
                        "mapping",
                        format!(
                            "mapping = \"{}\" must be one of {}",
                            m.escape_debug(),
                            GISCUS_MAPPINGS.join(", ")
                        ),
                    );
                }
            }
            if let Some(l) = &c.lang {
                if !is_lang(l) {
                    return comments_err(
                        "lang",
                        format!(
                            "lang = \"{}\" must be a language code such as en or pt-BR",
                            l.escape_debug()
                        ),
                    );
                }
            }
            if let Some(l) = &c.loading {
                if l != "lazy" && l != "eager" {
                    return comments_err(
                        "loading",
                        format!("loading = \"{}\" must be lazy or eager", l.escape_debug()),
                    );
                }
            }
            for (key, set) in [
                ("issue_term", c.issue_term.is_some()),
                ("label", c.label.is_some()),
            ] {
                if set {
                    return comments_err(key, format!("`{key}` only applies to utterances"));
                }
            }
        }
        CommentsProvider::Utterances => {
            if let Some(t) = &c.issue_term {
                if !UTTERANCES_TERMS.contains(&t.as_str()) {
                    return comments_err(
                        "issue_term",
                        format!(
                            "issue_term = \"{}\" must be one of {}",
                            t.escape_debug(),
                            UTTERANCES_TERMS.join(", ")
                        ),
                    );
                }
            }
            if let Some(l) = &c.label {
                if !is_plain_text(l) || l.chars().count() > 50 {
                    return comments_err(
                        "label",
                        format!(
                            "label = \"{}\" must be plain text (no control characters, quotes or < >, at most 50 characters)",
                            l.escape_debug()
                        ),
                    );
                }
            }
            for (key, set) in [
                ("repo_id", c.repo_id.is_some()),
                ("category", c.category.is_some()),
                ("category_id", c.category_id.is_some()),
                ("mapping", c.mapping.is_some()),
                ("reactions", c.reactions.is_some()),
                ("lang", c.lang.is_some()),
                ("loading", c.loading.is_some()),
            ] {
                if set {
                    return comments_err(key, format!("`{key}` only applies to giscus"));
                }
            }
        }
    }
    Ok(())
}

fn validate_newsletter(n: &NewsletterSection) -> Result<(), (String, String)> {
    let err = |key: &str, message: String| {
        Err((
            format!("[newsletter] {message}"),
            format!("[newsletter]{key}"),
        ))
    };
    match n.provider {
        NewsletterProvider::Buttondown => {
            if n.form_id.is_some() {
                return err("form_id", "`form_id` only applies to kit".into());
            }
            if n.action.is_some() {
                return err("action", "`action` only applies to kit".into());
            }
            match &n.username {
                None => err(
                    "username",
                    "buttondown needs `username` (your newsletter's username)".into(),
                ),
                Some(u) if !is_id(u) => err(
                    "username",
                    format!(
                        "username = \"{}\" may contain only ASCII letters, digits, '_' or '-'",
                        u.escape_debug()
                    ),
                ),
                Some(_) => Ok(()),
            }
        }
        NewsletterProvider::Kit => {
            if n.username.is_some() {
                return err("username", "`username` only applies to buttondown".into());
            }
            if let Some(action) = &n.action {
                if !is_kit_action(action) {
                    return err(
                        "action",
                        format!(
                            "action = \"{}\" must be an https:// URL on kit.com or convertkit.com (copy the form's action from Kit's embed code)",
                            action.escape_debug()
                        ),
                    );
                }
            }
            match &n.form_id {
                None if n.action.is_some() => Ok(()),
                None => err(
                    "form_id",
                    "kit needs `form_id` (the number in the form's embed code) or `action`".into(),
                ),
                Some(id)
                    if id.is_empty()
                        || id.len() > 20
                        || !id.bytes().all(|b| b.is_ascii_digit()) =>
                {
                    err(
                        "form_id",
                        format!(
                            "form_id = \"{}\" must be the numeric Kit form ID",
                            id.escape_debug()
                        ),
                    )
                }
                Some(_) => Ok(()),
            }
        }
    }
}

/// The `site.comments` template value (defaults filled in), or `None`
/// without `[comments]`. Shared by page templates and shortcodes.
pub fn comments_context(config: &SiteConfig) -> Option<serde_json::Value> {
    let c = config.comments.as_ref()?;
    Some(match c.provider {
        CommentsProvider::Giscus => serde_json::json!({
            "provider": "giscus",
            "repo": c.repo,
            "repo_id": c.repo_id.as_deref().unwrap_or(""),
            "category": c.category.as_deref().unwrap_or(""),
            "category_id": c.category_id.as_deref().unwrap_or(""),
            "mapping": c.mapping.as_deref().unwrap_or("pathname"),
            "reactions": if c.reactions.unwrap_or(true) { "1" } else { "0" },
            "theme": c.theme.as_deref().unwrap_or("preferred_color_scheme"),
            "lang": c.lang.as_deref().unwrap_or(""),
            "loading": c.loading.as_deref().unwrap_or("lazy"),
        }),
        CommentsProvider::Utterances => serde_json::json!({
            "provider": "utterances",
            "repo": c.repo,
            "issue_term": c.issue_term.as_deref().unwrap_or("pathname"),
            "label": c.label.as_deref().unwrap_or(""),
            "theme": c.theme.as_deref().unwrap_or("preferred-color-scheme"),
        }),
    })
}

/// The `site.newsletter` template value: provider, form `action` and the
/// email field's `name`, or `None` without `[newsletter]`.
pub fn newsletter_context(config: &SiteConfig) -> Option<serde_json::Value> {
    let n = config.newsletter.as_ref()?;
    Some(match n.provider {
        NewsletterProvider::Buttondown => serde_json::json!({
            "provider": "buttondown",
            "action": format!(
                "https://buttondown.com/api/emails/embed-subscribe/{}",
                n.username.as_deref().unwrap_or("")
            ),
            "email_field": "email",
        }),
        NewsletterProvider::Kit => serde_json::json!({
            "provider": "kit",
            "action": n.action.clone().unwrap_or_else(|| format!(
                "https://app.kit.com/forms/{}/subscriptions",
                n.form_id.as_deref().unwrap_or("")
            )),
            "email_field": "email_address",
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn giscus() -> CommentsSection {
        CommentsSection {
            provider: CommentsProvider::Giscus,
            repo: "owner/repo".into(),
            repo_id: Some("R_kgDOabc".into()),
            category: Some("Announcements".into()),
            category_id: Some("DIC_kwDOabc".into()),
            mapping: None,
            reactions: None,
            theme: None,
            lang: None,
            loading: None,
            issue_term: None,
            label: None,
        }
    }

    #[test]
    fn test_github_repo_format() {
        assert!(is_github_repo("owner/repo"));
        assert!(is_github_repo("my-org/my.site_2"));
        assert!(!is_github_repo("owner"));
        assert!(!is_github_repo("owner/repo/extra"));
        assert!(!is_github_repo("\"><script>/x"));
        assert!(!is_github_repo("../x"));
        assert!(!is_github_repo("owner/"));
    }

    #[test]
    fn test_validate_comments_accepts_valid_giscus() {
        assert!(validate_comments(&giscus()).is_ok());
        let mut c = giscus();
        c.theme = Some("https://example.com/giscus.css".into());
        c.lang = Some("pt-BR".into());
        c.mapping = Some("og:title".into());
        assert!(validate_comments(&c).is_ok());
    }

    #[test]
    fn test_validate_comments_rejects_hostile_values() {
        let hostile = "\"><script>alert(1)</script>";
        type Mutate = Box<dyn Fn(&mut CommentsSection)>;
        let cases: Vec<(&str, Mutate)> = vec![
            ("repo", Box::new(|c| c.repo = hostile.into())),
            ("repo_id", Box::new(|c| c.repo_id = Some(hostile.into()))),
            ("category", Box::new(|c| c.category = Some(hostile.into()))),
            (
                "category_id",
                Box::new(|c| c.category_id = Some(hostile.into())),
            ),
            ("mapping", Box::new(|c| c.mapping = Some(hostile.into()))),
            ("theme", Box::new(|c| c.theme = Some(hostile.into()))),
            (
                "theme",
                Box::new(|c| c.theme = Some("https://x.com/\"onload=\"x".into())),
            ),
            ("lang", Box::new(|c| c.lang = Some(hostile.into()))),
            ("loading", Box::new(|c| c.loading = Some("soon".into()))),
            ("label", Box::new(|c| c.label = Some("x".into()))),
        ];
        for (key, mutate) in cases {
            let mut c = giscus();
            mutate(&mut c);
            let (msg, locator) = validate_comments(&c).unwrap_err();
            assert_eq!(locator, format!("[comments]{key}"), "{msg}");
        }
    }

    #[test]
    fn test_validate_comments_giscus_requires_ids() {
        let mut c = giscus();
        c.repo_id = None;
        assert_eq!(validate_comments(&c).unwrap_err().1, "[comments]repo_id");
    }

    #[test]
    fn test_validate_utterances() {
        let mut c = CommentsSection {
            provider: CommentsProvider::Utterances,
            repo: "owner/repo".into(),
            repo_id: None,
            category: None,
            category_id: None,
            mapping: None,
            reactions: None,
            theme: Some("github-dark".into()),
            lang: None,
            loading: None,
            issue_term: Some("title".into()),
            label: Some("comments 💬".into()),
        };
        assert!(validate_comments(&c).is_ok());
        c.theme = Some("preferred_color_scheme".into());
        assert_eq!(validate_comments(&c).unwrap_err().1, "[comments]theme");
        c.theme = None;
        c.label = Some("<b>".into());
        assert_eq!(validate_comments(&c).unwrap_err().1, "[comments]label");
        c.label = None;
        c.repo_id = Some("x".into());
        assert_eq!(validate_comments(&c).unwrap_err().1, "[comments]repo_id");
    }

    #[test]
    fn test_validate_newsletter() {
        let mut n = NewsletterSection {
            provider: NewsletterProvider::Buttondown,
            username: Some("my-news".into()),
            form_id: None,
            action: None,
        };
        assert!(validate_newsletter(&n).is_ok());
        n.username = Some("a\"><script>".into());
        assert_eq!(
            validate_newsletter(&n).unwrap_err().1,
            "[newsletter]username"
        );
        n.username = None;
        assert_eq!(
            validate_newsletter(&n).unwrap_err().1,
            "[newsletter]username"
        );
        let mut k = NewsletterSection {
            provider: NewsletterProvider::Kit,
            username: None,
            form_id: Some("1234567".into()),
            action: None,
        };
        assert!(validate_newsletter(&k).is_ok());
        k.form_id = Some("12a".into());
        assert_eq!(
            validate_newsletter(&k).unwrap_err().1,
            "[newsletter]form_id"
        );
    }

    #[test]
    fn test_kit_action_override() {
        for ok in [
            "https://app.kit.com/forms/123/subscriptions",
            "https://app.convertkit.com/forms/123/subscriptions",
            "https://kit.com/x",
        ] {
            assert!(is_kit_action(ok), "{ok}");
        }
        for bad in [
            "http://app.kit.com/forms/1/subscriptions",
            "https://evilkit.com/forms/1",
            "https://kit.com.evil.com/x",
            "https://app.kit.com/forms/1?x=\"><script>",
            "https://user@app.kit.com/x",
            "https://app.kit.com/../x",
            "javascript:alert(1)",
        ] {
            assert!(!is_kit_action(bad), "{bad}");
        }
        let mut k = NewsletterSection {
            provider: NewsletterProvider::Kit,
            username: None,
            form_id: None,
            action: Some("https://app.kit.com/forms/9/subscriptions".into()),
        };
        assert!(validate_newsletter(&k).is_ok());
        k.action = Some("https://evil.example/x".into());
        assert_eq!(validate_newsletter(&k).unwrap_err().1, "[newsletter]action");
        let b = NewsletterSection {
            provider: NewsletterProvider::Buttondown,
            username: Some("n".into()),
            form_id: None,
            action: Some("https://app.kit.com/x".into()),
        };
        assert_eq!(validate_newsletter(&b).unwrap_err().1, "[newsletter]action");
    }

    #[test]
    fn test_is_lang() {
        assert!(is_lang("en"));
        assert!(is_lang("zh-Hans"));
        assert!(!is_lang("e"));
        assert!(!is_lang("en\""));
    }
}
