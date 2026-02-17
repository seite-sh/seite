use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::Result;

pub fn generate_rss(config: &SiteConfig, items: &[&ContentItem]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| crate::error::PageError::Build(format!("RSS write error: {e}")))?;

    let mut rss = BytesStart::new("rss");
    rss.push_attribute(("version", "2.0"));
    writer.write_event(Event::Start(rss)).unwrap();

    writer
        .write_event(Event::Start(BytesStart::new("channel")))
        .unwrap();

    write_text_element(&mut writer, "title", &config.site.title);
    write_text_element(&mut writer, "link", &config.site.base_url);
    write_text_element(&mut writer, "description", &config.site.description);
    write_text_element(&mut writer, "language", &config.site.language);

    for item in items {
        writer
            .write_event(Event::Start(BytesStart::new("item")))
            .unwrap();
        write_text_element(&mut writer, "title", &item.frontmatter.title);
        let link = format!("{}{}", base, item.url);
        write_text_element(&mut writer, "link", &link);
        write_text_element(&mut writer, "guid", &link);
        if let Some(date) = item.frontmatter.date {
            let datetime = date.and_hms_opt(12, 0, 0).unwrap();
            let rfc2822 =
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(datetime, chrono::Utc)
                    .to_rfc2822();
            write_text_element(&mut writer, "pubDate", &rfc2822);
        }
        let desc = item
            .frontmatter
            .description
            .as_deref()
            .unwrap_or("");
        write_text_element(&mut writer, "description", desc);
        writer
            .write_event(Event::End(BytesEnd::new("item")))
            .unwrap();
    }

    writer
        .write_event(Event::End(BytesEnd::new("channel")))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("rss")))
        .unwrap();

    let bytes = writer.into_inner().into_inner();
    Ok(String::from_utf8(bytes).unwrap())
}

fn write_text_element(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str, text: &str) {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new(text)))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .unwrap();
}
