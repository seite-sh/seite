use std::collections::HashMap;
use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::{PageError, Result};

/// A translation link used in the sitemap's xhtml:link alternates.
use super::TranslationLink;

pub(crate) fn generate_sitemap(
    config: &SiteConfig,
    items: &[&ContentItem],
    translation_map: &HashMap<(String, String), Vec<TranslationLink>>,
    extra_urls: &[String],
) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');
    let is_multilingual = config.is_multilingual();

    write(
        &mut writer,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)),
    )?;

    let mut urlset = BytesStart::new("urlset");
    urlset.push_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"));
    if is_multilingual {
        urlset.push_attribute(("xmlns:xhtml", "http://www.w3.org/1999/xhtml"));
    }
    write(&mut writer, Event::Start(urlset))?;

    // Index page(s)
    if is_multilingual {
        let all_langs = config.all_languages();
        // Default language index
        write_url_with_alternates(
            &mut writer,
            &format!("{base}/"),
            None,
            &all_langs,
            base,
            &config.site.language,
        )?;
        // Non-default language indices
        for lang in config.languages.keys() {
            write_url_with_alternates(
                &mut writer,
                &format!("{base}/{lang}/"),
                None,
                &all_langs,
                base,
                &config.site.language,
            )?;
        }
    } else {
        write_url(&mut writer, &format!("{base}/"), None)?;
    }

    for item in items {
        let loc = format!("{}{}", base, item.url);
        let lastmod = item.frontmatter.date.map(|d| d.to_string());

        if is_multilingual {
            let key = (item.collection.clone(), item.slug.clone());
            if let Some(translations) = translation_map.get(&key) {
                if translations.len() > 1 {
                    write_url_with_translations(&mut writer, &loc, lastmod, translations, base)?;
                } else {
                    write_url(&mut writer, &loc, lastmod)?;
                }
            } else {
                write_url(&mut writer, &loc, lastmod)?;
            }
        } else {
            write_url(&mut writer, &loc, lastmod)?;
        }
    }

    // Extra URLs (e.g. tag pages) — simple entries with no lastmod
    for url in extra_urls {
        let loc = format!("{base}{url}");
        write_url(&mut writer, &loc, None)?;
    }

    write(&mut writer, Event::End(BytesEnd::new("urlset")))?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| PageError::Build(format!("Sitemap encoding error: {e}")))
}

fn write(writer: &mut Writer<Cursor<Vec<u8>>>, event: Event<'_>) -> Result<()> {
    writer
        .write_event(event)
        .map_err(|e| PageError::Build(format!("Sitemap write error: {e}")))
}

fn write_url(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    loc: &str,
    lastmod: Option<String>,
) -> Result<()> {
    write(writer, Event::Start(BytesStart::new("url")))?;
    write(writer, Event::Start(BytesStart::new("loc")))?;
    write(writer, Event::Text(BytesText::new(loc)))?;
    write(writer, Event::End(BytesEnd::new("loc")))?;
    if let Some(date) = lastmod {
        write(writer, Event::Start(BytesStart::new("lastmod")))?;
        write(writer, Event::Text(BytesText::new(&date)))?;
        write(writer, Event::End(BytesEnd::new("lastmod")))?;
    }
    write(writer, Event::End(BytesEnd::new("url")))
}

fn write_url_with_translations(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    loc: &str,
    lastmod: Option<String>,
    translations: &[TranslationLink],
    base: &str,
) -> Result<()> {
    write(writer, Event::Start(BytesStart::new("url")))?;
    write(writer, Event::Start(BytesStart::new("loc")))?;
    write(writer, Event::Text(BytesText::new(loc)))?;
    write(writer, Event::End(BytesEnd::new("loc")))?;
    if let Some(date) = lastmod {
        write(writer, Event::Start(BytesStart::new("lastmod")))?;
        write(writer, Event::Text(BytesText::new(&date)))?;
        write(writer, Event::End(BytesEnd::new("lastmod")))?;
    }
    for t in translations {
        let mut link = BytesStart::new("xhtml:link");
        link.push_attribute(("rel", "alternate"));
        link.push_attribute(("hreflang", t.lang.as_str()));
        let href = format!("{base}{}", t.url);
        link.push_attribute(("href", href.as_str()));
        write(writer, Event::Empty(link))?;
    }
    write(writer, Event::End(BytesEnd::new("url")))
}

/// Write a URL entry with hreflang alternates for index pages.
fn write_url_with_alternates(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    loc: &str,
    lastmod: Option<String>,
    all_langs: &[String],
    base: &str,
    default_lang: &str,
) -> Result<()> {
    write(writer, Event::Start(BytesStart::new("url")))?;
    write(writer, Event::Start(BytesStart::new("loc")))?;
    write(writer, Event::Text(BytesText::new(loc)))?;
    write(writer, Event::End(BytesEnd::new("loc")))?;
    if let Some(date) = lastmod {
        write(writer, Event::Start(BytesStart::new("lastmod")))?;
        write(writer, Event::Text(BytesText::new(&date)))?;
        write(writer, Event::End(BytesEnd::new("lastmod")))?;
    }
    for lang in all_langs {
        let mut link = BytesStart::new("xhtml:link");
        link.push_attribute(("rel", "alternate"));
        link.push_attribute(("hreflang", lang.as_str()));
        let href = if lang == default_lang {
            format!("{base}/")
        } else {
            format!("{base}/{lang}/")
        };
        link.push_attribute(("href", href.as_str()));
        write(writer, Event::Empty(link))?;
    }
    // x-default
    let mut xdefault = BytesStart::new("xhtml:link");
    xdefault.push_attribute(("rel", "alternate"));
    xdefault.push_attribute(("hreflang", "x-default"));
    let xdefault_href = format!("{base}/");
    xdefault.push_attribute(("href", xdefault_href.as_str()));
    write(writer, Event::Empty(xdefault))?;

    write(writer, Event::End(BytesEnd::new("url")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Frontmatter;

    fn test_config(base_url: &str) -> SiteConfig {
        SiteConfig {
            site: crate::config::SiteSection {
                title: "Test".into(),
                description: "".into(),
                base_url: base_url.into(),
                language: "en".into(),
                author: "".into(),
            },
            collections: vec![],
            build: Default::default(),
            deploy: Default::default(),
            languages: Default::default(),
            images: Default::default(),
            analytics: None,
            trust: None,
            contact: None,
        }
    }

    fn test_item(slug: &str, url: &str, collection: &str) -> ContentItem {
        ContentItem {
            frontmatter: Frontmatter {
                title: slug.into(),
                date: None,
                updated: None,
                description: None,
                image: None,
                slug: None,
                tags: vec![],
                draft: false,
                template: None,
                robots: None,
                weight: None,
                extra: Default::default(),
            },
            raw_body: String::new(),
            html_body: String::new(),
            source_path: std::path::PathBuf::from("test.md"),
            slug: slug.into(),
            collection: collection.into(),
            url: url.into(),
            lang: "en".into(),
            excerpt: String::new(),
            toc: vec![],
            word_count: 0,
            reading_time: 0,
            excerpt_html: String::new(),
        }
    }

    #[test]
    fn test_generate_sitemap_basic() {
        let config = test_config("https://example.com");
        let item = test_item("hello", "/posts/hello", "posts");
        let items: Vec<&ContentItem> = vec![&item];
        let translation_map = HashMap::new();
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("<urlset"));
        assert!(result.contains("https://example.com/"));
        assert!(result.contains("https://example.com/posts/hello"));
    }

    #[test]
    fn test_generate_sitemap_strips_trailing_slash() {
        let config = test_config("https://example.com/");
        let items: Vec<&ContentItem> = vec![];
        let translation_map = HashMap::new();
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("https://example.com/"));
        assert!(!result.contains("https://example.com//"));
    }

    #[test]
    fn test_generate_sitemap_with_lastmod() {
        let config = test_config("https://example.com");
        let mut item = test_item("hello", "/posts/hello", "posts");
        item.frontmatter.date = Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap());
        let items: Vec<&ContentItem> = vec![&item];
        let translation_map = HashMap::new();
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("<lastmod>2025-06-15</lastmod>"));
    }

    #[test]
    fn test_generate_sitemap_extra_urls() {
        let config = test_config("https://example.com");
        let items: Vec<&ContentItem> = vec![];
        let translation_map = HashMap::new();
        let extra = vec!["/tags/".to_string(), "/tags/rust/".to_string()];
        let result = generate_sitemap(&config, &items, &translation_map, &extra).unwrap();
        assert!(result.contains("https://example.com/tags/"));
        assert!(result.contains("https://example.com/tags/rust/"));
    }

    #[test]
    fn test_generate_sitemap_empty() {
        let config = test_config("https://example.com");
        let items: Vec<&ContentItem> = vec![];
        let translation_map = HashMap::new();
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("<urlset"));
        assert!(result.contains("</urlset>"));
        assert!(result.contains("https://example.com/"));
    }

    #[test]
    fn test_generate_sitemap_multilingual() {
        let mut config = test_config("https://example.com");
        config.languages.insert(
            "es".into(),
            crate::config::LanguageConfig {
                title: Some("Test ES".into()),
                description: None,
            },
        );
        let item = test_item("hello", "/posts/hello", "posts");
        let items: Vec<&ContentItem> = vec![&item];
        let translation_map = HashMap::new();
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("xmlns:xhtml"));
        assert!(result.contains("hreflang"));
        assert!(result.contains("https://example.com/es/"));
    }

    #[test]
    fn test_generate_sitemap_with_translations() {
        let mut config = test_config("https://example.com");
        config.languages.insert(
            "es".into(),
            crate::config::LanguageConfig {
                title: Some("Test ES".into()),
                description: None,
            },
        );
        let item_en = test_item("hello", "/posts/hello", "posts");
        let mut item_es = test_item("hello", "/es/posts/hello", "posts");
        item_es.lang = "es".into();
        let items: Vec<&ContentItem> = vec![&item_en, &item_es];
        let mut translation_map = HashMap::new();
        translation_map.insert(
            ("posts".to_string(), "hello".to_string()),
            vec![
                TranslationLink {
                    lang: "en".into(),
                    url: "/posts/hello".into(),
                },
                TranslationLink {
                    lang: "es".into(),
                    url: "/es/posts/hello".into(),
                },
            ],
        );
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("hreflang=\"en\""));
        assert!(result.contains("hreflang=\"es\""));
        assert!(result.contains("/posts/hello"));
        assert!(result.contains("/es/posts/hello"));
    }

    #[test]
    fn test_generate_sitemap_multiple_items() {
        let config = test_config("https://example.com");
        let item1 = test_item("first", "/posts/first", "posts");
        let item2 = test_item("second", "/posts/second", "posts");
        let item3 = test_item("about", "/about", "pages");
        let items: Vec<&ContentItem> = vec![&item1, &item2, &item3];
        let translation_map = HashMap::new();
        let result = generate_sitemap(&config, &items, &translation_map, &[]).unwrap();
        assert!(result.contains("/posts/first"));
        assert!(result.contains("/posts/second"));
        assert!(result.contains("/about"));
    }
}
