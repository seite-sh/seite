use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::error::Result;

pub fn generate_sitemap(config: &SiteConfig, items: &[&ContentItem]) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let base = config.site.base_url.trim_end_matches('/');

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| crate::error::PageError::Build(format!("Sitemap write error: {e}")))?;

    let mut urlset = BytesStart::new("urlset");
    urlset.push_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"));
    writer.write_event(Event::Start(urlset)).unwrap();

    // Index page
    write_url(&mut writer, &format!("{base}/"), None);

    for item in items {
        let loc = format!("{}{}", base, item.url);
        write_url(
            &mut writer,
            &loc,
            item.frontmatter.date.map(|d| d.to_string()),
        );
    }

    writer
        .write_event(Event::End(BytesEnd::new("urlset")))
        .unwrap();

    let bytes = writer.into_inner().into_inner();
    Ok(String::from_utf8(bytes).unwrap())
}

fn write_url(writer: &mut Writer<Cursor<Vec<u8>>>, loc: &str, lastmod: Option<String>) {
    writer
        .write_event(Event::Start(BytesStart::new("url")))
        .unwrap();
    writer
        .write_event(Event::Start(BytesStart::new("loc")))
        .unwrap();
    writer
        .write_event(Event::Text(BytesText::new(loc)))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::new("loc")))
        .unwrap();
    if let Some(date) = lastmod {
        writer
            .write_event(Event::Start(BytesStart::new("lastmod")))
            .unwrap();
        writer
            .write_event(Event::Text(BytesText::new(&date)))
            .unwrap();
        writer
            .write_event(Event::End(BytesEnd::new("lastmod")))
            .unwrap();
    }
    writer
        .write_event(Event::End(BytesEnd::new("url")))
        .unwrap();
}
