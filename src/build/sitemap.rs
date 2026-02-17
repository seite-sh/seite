use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::{PageError, Result};

pub fn generate_sitemap(config: &SiteConfig, items: &[&ContentItem]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');

    write(&mut writer, Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let mut urlset = BytesStart::new("urlset");
    urlset.push_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"));
    write(&mut writer, Event::Start(urlset))?;

    // Index page
    write_url(&mut writer, &format!("{base}/"), None)?;

    for item in items {
        let loc = format!("{}{}", base, item.url);
        write_url(
            &mut writer,
            &loc,
            item.frontmatter.date.map(|d| d.to_string()),
        )?;
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
