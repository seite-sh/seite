use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::{PageError, Result};

pub fn generate_rss(config: &SiteConfig, items: &[&ContentItem]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');

    write(&mut writer, Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

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
                let rfc2822 =
                    chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(datetime, chrono::Utc)
                        .to_rfc2822();
                write_text_element(&mut writer, "pubDate", &rfc2822)?;
            }
        }
        let desc = item
            .frontmatter
            .description
            .as_deref()
            .unwrap_or("");
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
