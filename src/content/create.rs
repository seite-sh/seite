//! Creating new content files — shared by `seite new` and the MCP server's
//! `seite_create_content` tool so both produce identical files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use super::{generate_frontmatter, slug_from_title, Frontmatter};
use crate::config::{CollectionConfig, SiteConfig};
use crate::error::{PageError, Result};

/// Everything needed to create one content file.
#[derive(Debug, Default)]
pub struct NewContent<'a> {
    pub title: &'a str,
    /// Explicit filename slug. Defaults to a slug derived from the title.
    pub slug: Option<&'a str>,
    pub description: Option<&'a str>,
    pub tags: Vec<String>,
    pub draft: bool,
    pub weight: Option<i32>,
    /// Arbitrary frontmatter data exposed to templates as `page.extra`.
    pub extra: HashMap<String, serde_yaml_ng::Value>,
    /// Language code for a translation (`about.es.md`). The default language
    /// needs no suffix; other codes must be configured under `[languages]`.
    pub lang: Option<&'a str>,
    /// Sub-directory inside the collection (nested collections only),
    /// e.g. `guides/advanced`.
    pub subdir: Option<&'a str>,
    /// Markdown body. May be empty.
    pub body: &'a str,
    /// Replace an existing file instead of refusing.
    pub overwrite: bool,
}

/// The file that was written.
#[derive(Debug)]
pub struct CreatedContent {
    pub path: PathBuf,
    /// Slug used in the filename (without date prefix, language, or subdir).
    pub slug: String,
    /// True when an existing file was replaced (only possible with `overwrite`).
    pub overwritten: bool,
}

/// Validate an explicit slug: lowercase ASCII letters, digits, `-` and `_`,
/// starting with a letter or digit.
pub fn validate_slug(slug: &str) -> Result<()> {
    let mut chars = slug.chars();
    let valid_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    if valid_first
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(PageError::Other(format!(
            "invalid slug '{slug}': use lowercase letters, digits, '-' and '_' \
             (e.g. '{}')",
            fallback_example(slug)
        )))
    }
}

fn fallback_example(slug: &str) -> String {
    let s = slug_from_title(slug);
    if s.is_empty() {
        "my-page".to_string()
    } else {
        s
    }
}

/// Validate a collection sub-directory: relative, `/`-separated components of
/// letters, digits, `-` and `_` only — no `..`, `.`, absolute paths, or
/// backslashes — so the file always stays inside the collection directory.
pub fn validate_subdir(subdir: &str) -> Result<Vec<&str>> {
    let invalid = |why: &str| {
        Err(PageError::Other(format!(
            "invalid subdir '{subdir}': {why}. Use a relative path like 'guides' or 'guides/advanced'"
        )))
    };
    if subdir.is_empty() {
        return invalid("it is empty");
    }
    if subdir.starts_with('/') || subdir.contains('\\') || subdir.contains(':') {
        return invalid("absolute paths and backslashes are not allowed");
    }
    let parts: Vec<&str> = subdir.trim_end_matches('/').split('/').collect();
    for part in &parts {
        if part.is_empty() || *part == "." || *part == ".." {
            return invalid("empty, '.' and '..' path segments are not allowed");
        }
        if !part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return invalid("segments may only contain letters, digits, '-' and '_'");
        }
    }
    Ok(parts)
}

/// Resolve the language suffix for a new file: `None` for the default
/// language, `Some(code)` for a configured translation language.
pub fn resolve_lang_suffix<'a>(
    config: &SiteConfig,
    lang: Option<&'a str>,
) -> Result<Option<&'a str>> {
    match lang {
        None => Ok(None),
        Some(l) if l == config.site.language => Ok(None),
        Some(l) if config.languages.contains_key(l) => Ok(Some(l)),
        Some(l) => Err(PageError::Other(format!(
            "unknown language '{l}'. Configured languages: {}",
            config.all_languages().join(", ")
        ))),
    }
}

/// Create a content file in `collection` under `content_dir`.
///
/// Refuses to replace an existing file unless `spec.overwrite` is set, and
/// rejects titles that produce an empty slug (e.g. `"!!!"`) unless an
/// explicit slug is given.
pub fn create_content_file(
    config: &SiteConfig,
    content_dir: &Path,
    collection: &CollectionConfig,
    spec: &NewContent,
    today: NaiveDate,
) -> Result<CreatedContent> {
    if spec.title.trim().is_empty() {
        return Err(PageError::Other("title must not be empty".into()));
    }

    let slug = match spec.slug {
        Some(s) => {
            validate_slug(s)?;
            s.to_string()
        }
        None => {
            let s = slug_from_title(spec.title);
            if s.is_empty() {
                return Err(PageError::Other(format!(
                    "title '{}' does not produce a usable slug (it has no letters or digits); \
                     provide an explicit slug",
                    spec.title
                )));
            }
            s
        }
    };

    let lang_suffix = resolve_lang_suffix(config, spec.lang)?;

    let mut dir = content_dir.join(&collection.directory);
    if let Some(subdir) = spec.subdir {
        if !collection.nested {
            return Err(PageError::Other(format!(
                "collection '{}' is not nested, so 'subdir' is not supported \
                 (files in sub-directories would not get sub-directory URLs)",
                collection.name
            )));
        }
        for part in validate_subdir(subdir)? {
            dir = dir.join(part);
        }
    }

    let stem = if collection.has_date {
        format!("{}-{slug}", today.format("%Y-%m-%d"))
    } else {
        slug.clone()
    };
    let filename = match lang_suffix {
        Some(lang) => format!("{stem}.{lang}.md"),
        None => format!("{stem}.md"),
    };
    let path = dir.join(filename);

    let existed = path.exists();
    if existed && !spec.overwrite {
        return Err(PageError::Other(format!(
            "{} already exists; refusing to overwrite it. Edit the existing file \
             or choose a different title",
            path.display()
        )));
    }

    let fm = Frontmatter {
        title: spec.title.to_string(),
        date: if collection.has_date {
            Some(today)
        } else {
            None
        },
        description: spec.description.map(str::to_string),
        tags: spec.tags.clone(),
        draft: spec.draft,
        weight: spec.weight,
        extra: spec.extra.clone(),
        ..Default::default()
    };

    std::fs::create_dir_all(&dir)?;
    let frontmatter = generate_frontmatter(&fm);
    let file_content = if spec.body.trim().is_empty() {
        format!("{frontmatter}\n")
    } else {
        format!("{frontmatter}\n\n{}\n", spec.body.trim_end())
    };
    std::fs::write(&path, file_content)?;

    Ok(CreatedContent {
        path,
        slug,
        overwritten: existed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BuildSection, DeploySection, LanguageConfig, SiteSection};
    use std::collections::BTreeMap;

    fn config() -> SiteConfig {
        let mut languages = BTreeMap::new();
        languages.insert(
            "es".to_string(),
            LanguageConfig {
                title: None,
                description: None,
            },
        );
        SiteConfig {
            site: SiteSection {
                title: "T".into(),
                description: String::new(),
                base_url: "http://localhost:3000".into(),
                language: "en".into(),
                author: String::new(),
            },
            collections: vec![
                CollectionConfig::preset_posts(),
                CollectionConfig::preset_docs(),
            ],
            build: BuildSection::default(),
            deploy: DeploySection::default(),
            languages,
            images: None,
            analytics: None,
            trust: None,
            contact: None,
            access: None,
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 4).unwrap()
    }

    #[test]
    fn test_create_refuses_overwrite() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config();
        let docs = CollectionConfig::preset_docs();
        let spec = NewContent {
            title: "Intro",
            tags: vec!["keep".into()],
            body: "original",
            ..Default::default()
        };
        let created = create_content_file(&cfg, tmp.path(), &docs, &spec, today()).unwrap();
        assert!(!created.overwritten);
        let err = create_content_file(
            &cfg,
            tmp.path(),
            &docs,
            &NewContent {
                title: "Intro",
                body: "new",
                ..Default::default()
            },
            today(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
        let on_disk = std::fs::read_to_string(&created.path).unwrap();
        assert!(on_disk.contains("original") && on_disk.contains("keep"));

        let again = create_content_file(
            &cfg,
            tmp.path(),
            &docs,
            &NewContent {
                title: "Intro",
                body: "new",
                overwrite: true,
                ..Default::default()
            },
            today(),
        )
        .unwrap();
        assert!(again.overwritten);
        assert!(std::fs::read_to_string(&again.path)
            .unwrap()
            .contains("new"));
    }

    #[test]
    fn test_create_rejects_empty_slug() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = create_content_file(
            &config(),
            tmp.path(),
            &CollectionConfig::preset_docs(),
            &NewContent {
                title: "!!!",
                ..Default::default()
            },
            today(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("slug"));
        assert!(!tmp.path().join("docs/.md").exists());
    }

    #[test]
    fn test_create_with_all_fields() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut extra = HashMap::new();
        extra.insert(
            "hero".to_string(),
            serde_yaml_ng::Value::String("big".into()),
        );
        let created = create_content_file(
            &config(),
            tmp.path(),
            &CollectionConfig::preset_posts(),
            &NewContent {
                title: "Hello",
                slug: Some("custom-slug"),
                description: Some("Desc"),
                tags: vec!["a".into()],
                draft: true,
                weight: Some(3),
                extra,
                lang: Some("es"),
                body: "Body",
                ..Default::default()
            },
            today(),
        )
        .unwrap();
        assert_eq!(
            created.path,
            tmp.path().join("posts/2026-03-04-custom-slug.es.md")
        );
        let (fm, body) = crate::content::parse_content_file(&created.path).unwrap();
        assert_eq!(fm.title, "Hello");
        assert_eq!(fm.description.as_deref(), Some("Desc"));
        assert!(fm.draft);
        assert_eq!(fm.weight, Some(3));
        assert_eq!(fm.date, Some(today()));
        assert!(fm.extra.contains_key("hero"));
        assert_eq!(body.trim(), "Body");
    }

    #[test]
    fn test_create_subdir_nested_only_and_validated() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config();
        let docs = CollectionConfig::preset_docs();
        let created = create_content_file(
            &cfg,
            tmp.path(),
            &docs,
            &NewContent {
                title: "Intro",
                subdir: Some("guides/advanced"),
                ..Default::default()
            },
            today(),
        )
        .unwrap();
        assert_eq!(
            created.path,
            tmp.path().join("docs/guides/advanced/intro.md")
        );

        for bad in ["../escape", "/abs", "a/../b", "a\\b", "", "a//b", "c:x"] {
            let r = create_content_file(
                &cfg,
                tmp.path(),
                &docs,
                &NewContent {
                    title: "X",
                    subdir: Some(bad),
                    ..Default::default()
                },
                today(),
            );
            assert!(r.is_err(), "subdir {bad:?} should be rejected");
        }

        let r = create_content_file(
            &cfg,
            tmp.path(),
            &CollectionConfig::preset_posts(),
            &NewContent {
                title: "X",
                subdir: Some("sub"),
                ..Default::default()
            },
            today(),
        );
        assert!(r.unwrap_err().to_string().contains("not nested"));
    }

    #[test]
    fn test_create_rejects_bad_slug_and_lang() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config();
        let docs = CollectionConfig::preset_docs();
        for bad in ["", "Upper", "a/b", "../x", "a.b", "-x"] {
            let r = create_content_file(
                &cfg,
                tmp.path(),
                &docs,
                &NewContent {
                    title: "X",
                    slug: Some(bad),
                    ..Default::default()
                },
                today(),
            );
            assert!(r.is_err(), "slug {bad:?} should be rejected");
        }
        let r = create_content_file(
            &cfg,
            tmp.path(),
            &docs,
            &NewContent {
                title: "X",
                lang: Some("fr"),
                ..Default::default()
            },
            today(),
        );
        assert!(r.unwrap_err().to_string().contains("unknown language"));
    }

    #[test]
    fn test_create_empty_body_has_no_placeholder() {
        let tmp = tempfile::TempDir::new().unwrap();
        let created = create_content_file(
            &config(),
            tmp.path(),
            &CollectionConfig::preset_docs(),
            &NewContent {
                title: "Empty",
                ..Default::default()
            },
            today(),
        )
        .unwrap();
        let raw = std::fs::read_to_string(created.path).unwrap();
        assert!(raw.ends_with("---\n"), "{raw:?}");
    }
}
