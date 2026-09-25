//! Detect unknown keys in `seite.toml`.
//!
//! `SiteConfig` deliberately does not use `deny_unknown_fields` (old configs
//! and hand-edited extras must keep loading), so a typo such as
//! `[build] minfy = true` is silently ignored by serde. This module walks the
//! spanned TOML document against the config schema and reports each unknown
//! key as a `config-unknown-key` **warning** with its line and a did-you-mean
//! hint drawn from the sibling keys that *are* known.
//!
//! The known field names come straight from each struct's `Deserialize` impl
//! (see `fields_of`), so adding a field to a config struct never requires
//! touching this file; only a new *section* needs a line in `schema_for_root`.

use std::path::Path;

use serde::de::{self, DeserializeOwned, Visitor};
use toml::de::{DeTable, DeValue};

use super::{
    AccessSection, AnalyticsSection, BuildSection, CollectionConfig, ContactSection, DeploySection,
    ImageSection, LanguageConfig, SiteConfig, SiteSection, TrustSection,
};
use crate::diagnostics::{did_you_mean, line_col_at, Diagnostic};

/// Expected shape of a TOML value in the config schema.
enum Schema {
    /// A struct: its known keys, and the schema of each key's value.
    Struct(&'static [&'static str], fn(&str) -> Schema),
    /// An array whose elements all have the inner schema (`[[collections]]`).
    ArrayOf(fn() -> Schema),
    /// A table with free-form keys whose values have the inner schema
    /// (`[languages.<code>]`).
    MapOf(fn() -> Schema),
    /// Anything goes (scalars, free-form values).
    Any,
}

fn schema_for_root() -> Schema {
    Schema::Struct(fields_of::<SiteConfig>(), |key| match key {
        "site" => Schema::Struct(fields_of::<SiteSection>(), |_| Schema::Any),
        "build" => Schema::Struct(fields_of::<BuildSection>(), |_| Schema::Any),
        "deploy" => Schema::Struct(fields_of::<DeploySection>(), |_| Schema::Any),
        "images" => Schema::Struct(fields_of::<ImageSection>(), |_| Schema::Any),
        "analytics" => Schema::Struct(fields_of::<AnalyticsSection>(), |_| Schema::Any),
        "trust" => Schema::Struct(fields_of::<TrustSection>(), |_| Schema::Any),
        "contact" => Schema::Struct(fields_of::<ContactSection>(), |_| Schema::Any),
        "access" => Schema::Struct(fields_of::<AccessSection>(), |_| Schema::Any),
        "collections" => {
            Schema::ArrayOf(|| Schema::Struct(fields_of::<CollectionConfig>(), |_| Schema::Any))
        }
        "languages" => {
            Schema::MapOf(|| Schema::Struct(fields_of::<LanguageConfig>(), |_| Schema::Any))
        }
        _ => Schema::Any,
    })
}

/// Report every key in `source` (the text of `file`) that the config schema
/// does not know, as `config-unknown-key` warnings. Returns nothing when the
/// document does not parse — `SiteConfig::load` reports that error.
pub fn unknown_key_diagnostics(source: &str, file: &Path) -> Vec<Diagnostic> {
    let Ok(doc) = DeTable::parse(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_table(
        doc.get_ref(),
        &schema_for_root(),
        "",
        source,
        file,
        &mut out,
    );
    out
}

fn walk_table(
    table: &DeTable<'_>,
    schema: &Schema,
    section: &str,
    source: &str,
    file: &Path,
    out: &mut Vec<Diagnostic>,
) {
    match schema {
        Schema::Struct(known, child) => {
            for (key, value) in table {
                let name: &str = key.get_ref();
                if known.contains(&name) {
                    walk_value(
                        value.get_ref(),
                        &child(name),
                        &join(section, name),
                        source,
                        file,
                        out,
                    );
                    continue;
                }
                let (line, column) = line_col_at(source, key.span().start);
                let location = if section.is_empty() {
                    "at the top level".to_string()
                } else {
                    format!("in [{section}]")
                };
                let mut d = Diagnostic::warning(
                    "config-unknown-key",
                    format!("unknown key `{name}` {location} (it is ignored)"),
                )
                .with_file(file)
                .with_line(line)
                .with_column(column);
                let hint = did_you_mean(name, known.iter().copied())
                    .unwrap_or_else(|| format!("known keys here: {}", known.join(", ")));
                d = d.with_hint(hint);
                out.push(d);
            }
        }
        Schema::MapOf(inner) => {
            for (key, value) in table {
                let name: &str = key.get_ref();
                walk_value(
                    value.get_ref(),
                    &inner(),
                    &join(section, name),
                    source,
                    file,
                    out,
                );
            }
        }
        Schema::ArrayOf(_) | Schema::Any => {}
    }
}

fn walk_value(
    value: &DeValue<'_>,
    schema: &Schema,
    section: &str,
    source: &str,
    file: &Path,
    out: &mut Vec<Diagnostic>,
) {
    match (value, schema) {
        (DeValue::Table(table), Schema::Struct(..) | Schema::MapOf(_)) => {
            walk_table(table, schema, section, source, file, out);
        }
        (DeValue::Array(items), Schema::ArrayOf(inner)) => {
            let inner = inner();
            for item in items.iter() {
                walk_value(item.get_ref(), &inner, section, source, file, out);
            }
        }
        // Type mismatches are reported by the real deserializer.
        _ => {}
    }
}

fn join(section: &str, key: &str) -> String {
    if section.is_empty() {
        key.to_string()
    } else {
        format!("{section}.{key}")
    }
}

/// The field names a struct's `Deserialize` impl accepts, captured by feeding
/// it a deserializer that records the `fields` argument of
/// `deserialize_struct` and then bails out.
fn fields_of<T: DeserializeOwned>() -> &'static [&'static str] {
    let mut fields: &'static [&'static str] = &[];
    let _ = T::deserialize(FieldProbe(&mut fields));
    fields
}

struct FieldProbe<'a>(&'a mut &'static [&'static str]);

#[derive(Debug)]
struct ProbeDone;

impl std::fmt::Display for ProbeDone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("field probe")
    }
}

impl std::error::Error for ProbeDone {}

impl de::Error for ProbeDone {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        ProbeDone
    }
}

impl<'de> de::Deserializer<'de> for FieldProbe<'_> {
    type Error = ProbeDone;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, ProbeDone> {
        Err(ProbeDone)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, ProbeDone> {
        *self.0 = fields;
        Err(ProbeDone)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diags(src: &str) -> Vec<Diagnostic> {
        unknown_key_diagnostics(src, Path::new("seite.toml"))
    }

    #[test]
    fn test_fields_of_reads_struct_fields() {
        let build = fields_of::<BuildSection>();
        assert!(build.contains(&"minify"));
        assert!(build.contains(&"output_dir"));
        assert!(fields_of::<SiteConfig>().contains(&"collections"));
    }

    #[test]
    fn test_unknown_key_in_section_has_line_and_hint() {
        let src = "[site]\ntitle = \"x\"\n\n[build]\nminfy = true\n";
        let d = diags(src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code, "config-unknown-key");
        assert!(!d[0].is_error());
        assert_eq!(d[0].line, Some(5));
        assert_eq!(d[0].column, Some(1));
        assert!(d[0].message.contains("`minfy`"));
        assert!(d[0].message.contains("[build]"));
        assert_eq!(d[0].hint.as_deref(), Some("did you mean `minify`?"));
    }

    #[test]
    fn test_unknown_keys_in_arrays_maps_top_level_and_dotted() {
        let src = r#"
bulid = 1
[site]
title = "x"
[[collections]]
name = "posts"
lable = "Posts"
[[collections]]
name = "docs"
nestd = true
[languages.es]
title = "Hola"
titel = "typo"
[deploy]
target = "netlify"
auto_comit = false
[images]
webp = true
"#;
        let d = diags(src);
        let names: Vec<_> = d.iter().map(|d| d.message.clone()).collect();
        assert_eq!(d.len(), 5, "{names:?}");
        assert!(d[0].message.contains("`bulid` at the top level"));
        assert_eq!(d[0].hint.as_deref(), Some("did you mean `build`?"));
        assert!(d[1].message.contains("[collections]"));
        assert_eq!(d[1].hint.as_deref(), Some("did you mean `label`?"));
        assert_eq!(d[2].hint.as_deref(), Some("did you mean `nested`?"));
        assert!(d[3].message.contains("[languages.es]"));
        assert_eq!(d[4].hint.as_deref(), Some("did you mean `auto_commit`?"));
        assert_eq!(d[4].line, Some(16));
    }

    #[test]
    fn test_dotted_and_inline_tables() {
        let d = diags("site = { title = \"x\", tittle = \"y\" }\nbuild.minfy = true\n");
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d[1].message.contains("[build]"));
    }

    #[test]
    fn test_clean_and_unparseable_config() {
        assert!(diags("[site]\ntitle = \"x\"\n[build]\nminify = true\n").is_empty());
        assert!(diags("[site\n").is_empty());
    }

    #[test]
    fn test_no_close_match_lists_known_keys() {
        let d = diags("[trust]\nzzzzzzzz = 1\n");
        assert!(d[0].hint.as_ref().unwrap().starts_with("known keys here:"));
    }
}
