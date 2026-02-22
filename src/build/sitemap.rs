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
