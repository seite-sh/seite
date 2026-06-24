//! Per-language resolution of data values ("language maps").
//!
//! A *language map* is a JSON object whose keys are EXACTLY the site's
//! configured language codes, e.g. `{ "en": "Hello", "de": "Hallo" }`. When a
//! page is rendered for language `L`, such a value resolves to `value[L]`,
//! falling back to the default language, then the first present entry.
//!
//! Any other value — scalars, arrays, or maps that contain a non-language key
//! (e.g. `{ icon, label }`) — passes through untouched, so plain
//! single-language data stays byte-identical and fully backward compatible.
//!
//! Resolution is exposed as the Tera `i18n` filter (alias `localize`):
//! `{{ value | i18n(lang=lang) }}`.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

/// True if `obj` is a non-empty map whose keys are *all* known language codes.
///
/// This is the gate from requirement 3: a map is only treated as a language map
/// when every key is a configured language code. A map that merely contains a
/// language-like key alongside others (`{ icon, label }`) is left untouched.
pub fn is_language_map(obj: &Map<String, Value>, lang_codes: &HashSet<String>) -> bool {
    !obj.is_empty() && obj.keys().all(|k| lang_codes.contains(k))
}

/// Resolve `value` for `lang`.
///
/// If `value` is a language map, return `value[lang]`, falling back to
/// `value[default_lang]`, then the first present entry. Otherwise return a
/// clone of `value` unchanged.
pub fn resolve_value(
    value: &Value,
    lang: &str,
    default_lang: &str,
    lang_codes: &HashSet<String>,
) -> Value {
    if let Value::Object(obj) = value {
        if is_language_map(obj, lang_codes) {
            // Requested language, then the default, then any entry.
            // `is_language_map` guarantees a non-empty map, so this always yields.
            if let Some(v) = obj
                .get(lang)
                .or_else(|| obj.get(default_lang))
                .or_else(|| obj.values().next())
            {
                return v.clone();
            }
        }
    }
    value.clone()
}

/// Collect human-readable warnings for *partial* language maps in `data`:
/// objects whose keys are all language codes but which are missing one or more
/// configured languages. Only meaningful for multilingual sites.
pub fn partial_language_map_warnings(data: &Value, lang_codes: &HashSet<String>) -> Vec<String> {
    let mut warnings = Vec::new();
    // `data` is always a JSON object (the data dir loads into a root map).
    if lang_codes.len() > 1 {
        if let Value::Object(obj) = data {
            for (key, value) in obj {
                // `data.i18n` (data/i18n/{lang}.yaml) is the reserved UI-strings
                // convention: missing languages intentionally fall back to the
                // English defaults, so it is not a "partial" translation.
                if key == "i18n" {
                    continue;
                }
                collect_partial(value, &format!("data.{key}"), lang_codes, &mut warnings);
            }
        }
    }
    warnings
}

fn collect_partial(value: &Value, path: &str, lang_codes: &HashSet<String>, out: &mut Vec<String>) {
    match value {
        Value::Object(obj) => {
            if is_language_map(obj, lang_codes) {
                if obj.len() < lang_codes.len() {
                    let mut missing: Vec<&str> = lang_codes
                        .iter()
                        .filter(|c| !obj.contains_key(*c))
                        .map(|s| s.as_str())
                        .collect();
                    missing.sort_unstable();
                    out.push(format!(
                        "{path}: language map missing translation(s) for {}",
                        missing.join(", ")
                    ));
                }
                // The variants are leaf prose; don't recurse into them.
            } else {
                for (k, v) in obj {
                    collect_partial(v, &format!("{path}.{k}"), lang_codes, out);
                }
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                collect_partial(v, &format!("{path}[{i}]"), lang_codes, out);
            }
        }
        _ => {}
    }
}

/// A Tera filter that resolves a language map to the current language.
#[derive(Clone)]
struct LangFilter {
    default_lang: String,
    lang_codes: HashSet<String>,
}

impl tera::Filter for LangFilter {
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("");
        let default = args
            .get("default")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_lang);
        Ok(resolve_value(value, lang, default, &self.lang_codes))
    }
}

/// Register the `i18n` filter (and its `localize` alias) on a Tera instance.
///
/// Usage in templates: `{{ value | i18n(lang=lang) }}`. The optional `default`
/// argument overrides the fallback language. Without a `lang` argument the
/// filter resolves to the default language.
pub fn register_filters(tera: &mut tera::Tera, default_lang: &str, lang_codes: &HashSet<String>) {
    let filter = LangFilter {
        default_lang: default_lang.to_string(),
        lang_codes: lang_codes.clone(),
    };
    tera.register_filter("i18n", filter.clone());
    tera.register_filter("localize", filter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn codes(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    // ---- is_language_map -------------------------------------------------

    #[test]
    fn language_map_when_all_keys_are_codes() {
        let v = json!({ "en": "Hello", "de": "Hallo" });
        assert!(is_language_map(
            v.as_object().unwrap(),
            &codes(&["en", "de"])
        ));
    }

    #[test]
    fn not_language_map_when_a_key_is_not_a_code() {
        let v = json!({ "icon": "x", "label": "y" });
        assert!(!is_language_map(
            v.as_object().unwrap(),
            &codes(&["en", "de"])
        ));
    }

    #[test]
    fn not_language_map_when_empty() {
        let v = json!({});
        assert!(!is_language_map(
            v.as_object().unwrap(),
            &codes(&["en", "de"])
        ));
    }

    #[test]
    fn not_language_map_when_one_key_is_foreign() {
        // `xx` is not configured, so the whole map is left untouched.
        let v = json!({ "en": "Hello", "xx": "??" });
        assert!(!is_language_map(
            v.as_object().unwrap(),
            &codes(&["en", "de"])
        ));
    }

    // ---- resolve_value ---------------------------------------------------

    #[test]
    fn resolves_to_current_language() {
        let v = json!({ "en": "Cloud infrastructure", "de": "Cloud-Infrastruktur" });
        let out = resolve_value(&v, "de", "en", &codes(&["en", "de"]));
        assert_eq!(out, json!("Cloud-Infrastruktur"));
    }

    #[test]
    fn falls_back_to_default_language_when_current_missing() {
        let v = json!({ "en": "Cloud infrastructure" });
        let out = resolve_value(&v, "de", "en", &codes(&["en", "de"]));
        assert_eq!(out, json!("Cloud infrastructure"));
    }

    #[test]
    fn falls_back_to_first_entry_when_default_also_missing() {
        // Only `fr` present; neither requested `de` nor default `en` exist.
        let v = json!({ "fr": "Bonjour" });
        let out = resolve_value(&v, "de", "en", &codes(&["en", "de", "fr"]));
        assert_eq!(out, json!("Bonjour"));
    }

    #[test]
    fn passes_scalar_through_unchanged() {
        let v = json!("plain string");
        let out = resolve_value(&v, "de", "en", &codes(&["en", "de"]));
        assert_eq!(out, json!("plain string"));
    }

    #[test]
    fn passes_non_language_map_through_unchanged() {
        let v = json!({ "icon": "shield", "label": "Security" });
        let out = resolve_value(&v, "de", "en", &codes(&["en", "de"]));
        assert_eq!(out, v);
    }

    #[test]
    fn passes_number_and_bool_through_unchanged() {
        assert_eq!(
            resolve_value(&json!(42), "de", "en", &codes(&["en", "de"])),
            json!(42)
        );
        assert_eq!(
            resolve_value(&json!(true), "de", "en", &codes(&["en", "de"])),
            json!(true)
        );
    }

    // ---- Tera filter -----------------------------------------------------

    fn render(tmpl: &str, ctx: &tera::Context, default_lang: &str, langs: &[&str]) -> String {
        let mut tera = tera::Tera::default();
        register_filters(&mut tera, default_lang, &codes(langs));
        tera.add_raw_template("t", tmpl).unwrap();
        tera.render("t", ctx).unwrap()
    }

    #[test]
    fn filter_resolves_with_explicit_lang() {
        let mut ctx = tera::Context::new();
        ctx.insert("v", &json!({ "en": "Hello", "de": "Hallo" }));
        ctx.insert("lang", "de");
        let out = render("{{ v | i18n(lang=lang) }}", &ctx, "en", &["en", "de"]);
        assert_eq!(out, "Hallo");
    }

    #[test]
    fn filter_alias_localize_works() {
        let mut ctx = tera::Context::new();
        ctx.insert("v", &json!({ "en": "Hello", "de": "Hallo" }));
        ctx.insert("lang", "de");
        let out = render("{{ v | localize(lang=lang) }}", &ctx, "en", &["en", "de"]);
        assert_eq!(out, "Hallo");
    }

    #[test]
    fn filter_without_lang_arg_uses_default_language() {
        let mut ctx = tera::Context::new();
        ctx.insert("v", &json!({ "en": "Hello", "de": "Hallo" }));
        let out = render("{{ v | i18n }}", &ctx, "en", &["en", "de"]);
        assert_eq!(out, "Hello");
    }

    #[test]
    fn filter_leaves_plain_string_byte_identical() {
        let mut ctx = tera::Context::new();
        ctx.insert("v", &json!("SOC 2 Type II report"));
        ctx.insert("lang", "de");
        let out = render("{{ v | i18n(lang=lang) }}", &ctx, "en", &["en", "de"]);
        assert_eq!(out, "SOC 2 Type II report");
    }

    // ---- partial language map warnings -----------------------------------

    #[test]
    fn warns_on_partial_language_map() {
        let data = json!({ "trust": { "faq": [ { "answer": { "en": "Yes" } } ] } });
        let warnings = partial_language_map_warnings(&data, &codes(&["en", "de"]));
        assert_eq!(warnings.len(), 1, "warnings: {warnings:?}");
        assert!(warnings[0].contains("de"), "warnings: {warnings:?}");
        assert!(
            warnings[0].contains("data.trust.faq[0].answer"),
            "warnings: {warnings:?}"
        );
    }

    #[test]
    fn no_warning_when_language_map_is_complete() {
        let data = json!({ "x": { "en": "Yes", "de": "Ja" } });
        let warnings = partial_language_map_warnings(&data, &codes(&["en", "de"]));
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
    }

    #[test]
    fn no_warning_for_non_language_map() {
        let data = json!({ "badge": { "icon": "shield", "label": "Security" } });
        let warnings = partial_language_map_warnings(&data, &codes(&["en", "de"]));
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
    }

    #[test]
    fn no_warning_for_monolingual_site() {
        let data = json!({ "x": { "en": "Yes" } });
        let warnings = partial_language_map_warnings(&data, &codes(&["en"]));
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
    }

    #[test]
    fn no_warning_for_reserved_i18n_ui_strings() {
        // `data/i18n/{lang}.yaml` deliberately falls back to English defaults for
        // any language without a file; it must not be flagged as a partial map.
        let data = json!({ "i18n": { "en": { "newer": "Newer" } } });
        let warnings = partial_language_map_warnings(&data, &codes(&["en", "de"]));
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
    }
}
