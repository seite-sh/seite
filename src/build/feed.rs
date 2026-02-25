use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::{PageError, Result};

pub fn generate_rss(config: &SiteConfig, items: &[&ContentItem]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');

    write(
        &mut writer,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)),
    )?;

    let mut rss = BytesStart::new("rss");
    rss.push_attribute(("version", "2.0"));
    write(&mut writer, Event::Start(rss))?;
    write(&mut writer, Event::Start(BytesStart::new("channel")))?;

    write_text_element(&mut writer, "title", &config.site.title)?;
    write_text_element(&mut writer, "link", &config.site.base_url)?;
    write_text_element(&mut writer, "description", &config.site.description)?;
    write_text_element(&mut writer, "language", &config.site.language)?;

    for item in items {
        write(&mut writer, Event::Start(BytesStart::new("item")))?;
        write_text_element(&mut writer, "title", &item.frontmatter.title)?;
        let link = format!("{}{}", base, item.url);
        write_text_element(&mut writer, "link", &link)?;
        write_text_element(&mut writer, "guid", &link)?;
        if let Some(date) = item.frontmatter.date {
            if let Some(datetime) = date.and_hms_opt(12, 0, 0) {
                let rfc2822 = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                    datetime,
                    chrono::Utc,
                )
                .to_rfc2822();
                write_text_element(&mut writer, "pubDate", &rfc2822)?;
            }
        }
        let desc = item.frontmatter.description.as_deref().unwrap_or("");
        write_text_element(&mut writer, "description", desc)?;
        write(&mut writer, Event::End(BytesEnd::new("item")))?;
    }

    write(&mut writer, Event::End(BytesEnd::new("channel")))?;
    write(&mut writer, Event::End(BytesEnd::new("rss")))?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| PageError::Build(format!("RSS encoding error: {e}")))
}

fn write(writer: &mut Writer<Cursor<Vec<u8>>>, event: Event<'_>) -> Result<()> {
    writer
        .write_event(event)
        .map_err(|e| PageError::Build(format!("RSS write error: {e}")))
}

fn write_text_element(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str, text: &str) -> Result<()> {
    write(writer, Event::Start(BytesStart::new(tag)))?;
    write(writer, Event::Text(BytesText::new(text)))?;
    write(writer, Event::End(BytesEnd::new(tag)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{ContentItem, Frontmatter};

    fn test_config() -> SiteConfig {
        SiteConfig {
            site: crate::config::SiteSection {
                title: "Test Blog".into(),
                description: "A test blog".into(),
                base_url: "https://example.com".into(),
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

    fn test_item(title: &str, date: Option<chrono::NaiveDate>, desc: Option<&str>) -> ContentItem {
        ContentItem {
            frontmatter: Frontmatter {
                title: title.into(),
                date,
                updated: None,
                description: desc.map(|d| d.into()),
                image: None,
                slug: None,
                tags: vec![],
                draft: false,
                template: None,
                robots: None,
                weight: None,
                extra: Default::default(),
            },
            raw_body: "test".into(),
            html_body: "<p>test</p>".into(),
            source_path: std::path::PathBuf::from("test.md"),
            slug: "test-post".into(),
            collection: "posts".into(),
            url: "/posts/test-post".into(),
            lang: "en".into(),
            word_count: 1,
            reading_time: 1,
            excerpt: String::new(),
            excerpt_html: String::new(),
            toc: vec![],
        }
    }

    #[test]
    fn test_generate_rss_basic() {
        let config = test_config();
        let item = test_item(
            "Hello World",
            Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()),
            Some("A first post"),
        );
        let items: Vec<&ContentItem> = vec![&item];
        let rss = generate_rss(&config, &items).unwrap();
        assert!(rss.contains("<title>Test Blog</title>"));
        assert!(rss.contains("<title>Hello World</title>"));
        assert!(rss.contains("https://example.com/posts/test-post"));
        assert!(rss.contains("<pubDate>"));
        assert!(rss.contains("<description>A first post</description>"));
    }

    #[test]
    fn test_generate_rss_no_date() {
        let config = test_config();
        let item = test_item("No Date", None, Some("desc"));
        let items: Vec<&ContentItem> = vec![&item];
        let rss = generate_rss(&config, &items).unwrap();
        assert!(rss.contains("<title>No Date</title>"));
        assert!(!rss.contains("<pubDate>"));
    }

    #[test]
    fn test_generate_rss_no_description() {
        let config = test_config();
        let item = test_item("No Desc", None, None);
        let items: Vec<&ContentItem> = vec![&item];
        let rss = generate_rss(&config, &items).unwrap();
        assert!(rss.contains("<description></description>"));
    }

    #[test]
    fn test_generate_rss_empty_items() {
        let config = test_config();
        let items: Vec<&ContentItem> = vec![];
        let rss = generate_rss(&config, &items).unwrap();
        assert!(rss.contains("<channel>"));
        assert!(rss.contains("</channel>"));
        assert!(!rss.contains("<item>"));
    }

    #[test]
    fn test_generate_rss_trailing_slash_removed() {
        let mut config = test_config();
        config.site.base_url = "https://example.com/".into();
        let item = test_item("Trailing", None, None);
        let items: Vec<&ContentItem> = vec![&item];
        let rss = generate_rss(&config, &items).unwrap();
        assert!(rss.contains("https://example.com/posts/test-post"));
        assert!(!rss.contains("https://example.com//posts/test-post"));
    }
}
