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

/// Generate an Atom 1.0 feed from site config and a pre-filtered slice of content items.
///
/// `feed_url` is the absolute URL of the Atom feed itself (used for the `rel="self"` link),
/// e.g. `"https://example.com/atom.xml"` or `"https://example.com/es/atom.xml"`.
pub fn generate_atom(
    config: &SiteConfig,
    items: &[&ContentItem],
    feed_url: &str,
) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');

    write(
        &mut writer,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)),
    )?;

    let mut feed = BytesStart::new("feed");
    feed.push_attribute(("xmlns", "http://www.w3.org/2005/Atom"));
    write(&mut writer, Event::Start(feed))?;

    write_text_element(&mut writer, "title", &config.site.title)?;
    if !config.site.description.is_empty() {
        write_text_element(&mut writer, "subtitle", &config.site.description)?;
    }

    // Alternate link to site
    let mut alt_link = BytesStart::new("link");
    alt_link.push_attribute(("href", base));
    alt_link.push_attribute(("rel", "alternate"));
    write(&mut writer, Event::Empty(alt_link))?;

    // Self link to this feed
    let mut self_link = BytesStart::new("link");
    self_link.push_attribute(("href", feed_url));
    self_link.push_attribute(("rel", "self"));
    self_link.push_attribute(("type", "application/atom+xml"));
    write(&mut writer, Event::Empty(self_link))?;

    // Feed ID — uses base URL; note this changes if the domain changes.
    write_text_element(&mut writer, "id", &format!("{base}/"))?;

    // Feed-level <author> is required by Atom spec (RFC 4287 §4.1.1)
    // unless every <entry> has its own <author>.
    if !config.site.author.is_empty() {
        write(&mut writer, Event::Start(BytesStart::new("author")))?;
        write_text_element(&mut writer, "name", &config.site.author)?;
        write(&mut writer, Event::End(BytesEnd::new("author")))?;
    }

    // <updated> is the most recent item date, or epoch if no items have dates
    let most_recent = items
        .iter()
        .filter_map(|item| item.frontmatter.updated.or(item.frontmatter.date))
        .max();
    let updated_str = date_to_rfc3339(most_recent);
    write_text_element(&mut writer, "updated", &updated_str)?;

    for item in items {
        write(&mut writer, Event::Start(BytesStart::new("entry")))?;
        write_text_element(&mut writer, "title", &item.frontmatter.title)?;

        let href = format!("{}{}", base, item.url);
        let mut entry_link = BytesStart::new("link");
        entry_link.push_attribute(("href", href.as_str()));
        entry_link.push_attribute(("rel", "alternate"));
        write(&mut writer, Event::Empty(entry_link))?;

        write_text_element(&mut writer, "id", &href)?;

        // <updated> is required per Atom spec; prefer updated, fall back to date
        let entry_updated = date_to_rfc3339(item.frontmatter.updated.or(item.frontmatter.date));
        write_text_element(&mut writer, "updated", &entry_updated)?;

        if let Some(date) = item.frontmatter.date {
            write_text_element(&mut writer, "published", &date_to_rfc3339(Some(date)))?;
        }

        let summary = item.frontmatter.description.as_deref().unwrap_or("");
        if !summary.is_empty() {
            write_text_element(&mut writer, "summary", summary)?;
        }

        write(&mut writer, Event::End(BytesEnd::new("entry")))?;
    }

    write(&mut writer, Event::End(BytesEnd::new("feed")))?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| PageError::Build(format!("Atom encoding error: {e}")))
}

/// Format a NaiveDate as RFC 3339 (required by Atom). Falls back to epoch if None.
fn date_to_rfc3339(date: Option<chrono::NaiveDate>) -> String {
    match date {
        Some(d) => {
            let datetime = d.and_hms_opt(12, 0, 0).expect("valid HMS for Atom date");
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(datetime, chrono::Utc)
                .to_rfc3339()
        }
        None => "1970-01-01T00:00:00+00:00".to_string(),
    }
}

fn write(writer: &mut Writer<Cursor<Vec<u8>>>, event: Event<'_>) -> Result<()> {
    writer
        .write_event(event)
        .map_err(|e| PageError::Build(format!("Feed write error: {e}")))
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
                description: desc.map(|d| d.into()),
                ..Default::default()
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

    // --- Atom feed tests ---

    #[test]
    fn test_generate_atom_basic() {
        let config = test_config();
        let item = test_item(
            "Hello World",
            Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()),
            Some("A first post"),
        );
        let items: Vec<&ContentItem> = vec![&item];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        assert!(atom.contains("xmlns=\"http://www.w3.org/2005/Atom\""));
        assert!(atom.contains("<title>Test Blog</title>"));
        assert!(atom.contains("<subtitle>A test blog</subtitle>"));
        assert!(atom.contains("<title>Hello World</title>"));
        assert!(atom.contains("https://example.com/posts/test-post"));
        assert!(atom.contains("<summary>A first post</summary>"));
        assert!(atom.contains("<published>"));
        assert!(atom.contains("rel=\"self\""));
        assert!(atom.contains("atom.xml"));
    }

    #[test]
    fn test_generate_atom_no_date() {
        let config = test_config();
        let item = test_item("No Date", None, Some("desc"));
        let items: Vec<&ContentItem> = vec![&item];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        assert!(atom.contains("<title>No Date</title>"));
        assert!(!atom.contains("<published>"));
        // <updated> is always present (required by Atom spec)
        assert!(atom.contains("<updated>"));
    }

    #[test]
    fn test_generate_atom_empty_items() {
        let config = test_config();
        let items: Vec<&ContentItem> = vec![];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        assert!(atom.contains("<feed"));
        assert!(atom.contains("</feed>"));
        assert!(!atom.contains("<entry>"));
        // Feed-level <updated> falls back to epoch
        assert!(atom.contains("1970-01-01T00:00:00+00:00"));
    }

    #[test]
    fn test_generate_atom_trailing_slash_removed() {
        let mut config = test_config();
        config.site.base_url = "https://example.com/".into();
        let item = test_item("Trailing", None, None);
        let items: Vec<&ContentItem> = vec![&item];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        assert!(atom.contains("https://example.com/posts/test-post"));
        assert!(!atom.contains("https://example.com//posts/test-post"));
    }

    #[test]
    fn test_generate_atom_empty_description_omitted() {
        let config = test_config();
        let item = test_item("No Desc", None, None);
        let items: Vec<&ContentItem> = vec![&item];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        // Empty summary should not be written
        assert!(!atom.contains("<summary>"));
    }

    #[test]
    fn test_generate_atom_uses_updated_date() {
        let config = test_config();
        let mut item = test_item(
            "Updated Post",
            Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            None,
        );
        item.frontmatter.updated = Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap());
        let items: Vec<&ContentItem> = vec![&item];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        // Entry <updated> should use the updated date, not published
        assert!(atom.contains("2025-06-15"));
    }

    #[test]
    fn test_generate_atom_empty_description_no_subtitle() {
        let mut config = test_config();
        config.site.description = String::new();
        let items: Vec<&ContentItem> = vec![];
        let atom = generate_atom(&config, &items, "https://example.com/atom.xml").unwrap();
        assert!(!atom.contains("<subtitle>"));
    }
}
