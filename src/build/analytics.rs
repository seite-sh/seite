use crate::config::{AnalyticsProvider, AnalyticsSection};

/// Build the Plausible script URL, incorporating extensions if present.
///
/// With no extensions: `https://plausible.io/js/script.js`
/// With extensions:    `https://plausible.io/js/script.tagged-events.outbound-links.js`
/// With custom script_url: uses that URL as-is (extensions are ignored).
fn plausible_script_url(config: &AnalyticsSection) -> String {
    if let Some(custom) = &config.script_url {
        return custom.clone();
    }
    if config.extensions.is_empty() {
        return "https://plausible.io/js/script.js".to_string();
    }
    format!(
        "https://plausible.io/js/script.{}.js",
        config.extensions.join(".")
    )
}

/// Generate the inline analytics `<script>` tags for the configured provider.
/// This is injected directly when cookie consent is disabled.
fn analytics_script(config: &AnalyticsSection) -> String {
    match config.provider {
        AnalyticsProvider::Google => {
            format!(
                r#"<script async src="https://www.googletagmanager.com/gtag/js?id={id}"></script>
<script>window.dataLayer=window.dataLayer||[];function gtag(){{dataLayer.push(arguments)}}gtag('js',new Date());gtag('config','{id}');</script>"#,
                id = config.id
            )
        }
        AnalyticsProvider::Gtm => {
            format!(
                r#"<script>(function(w,d,s,l,i){{w[l]=w[l]||[];w[l].push({{'gtm.start':new Date().getTime(),event:'gtm.js'}});var f=d.getElementsByTagName(s)[0],j=d.createElement(s),dl=l!='dataLayer'?'&l='+l:'';j.async=true;j.src='https://www.googletagmanager.com/gtm.js?id='+i+dl;f.parentNode.insertBefore(j,f)}})(window,document,'script','dataLayer','{id}');</script>"#,
                id = config.id
            )
        }
        AnalyticsProvider::Plausible => {
            let src = plausible_script_url(config);
            format!(
                r#"<script defer data-domain="{id}" src="{src}"></script>"#,
                id = config.id,
                src = src,
            )
        }
        AnalyticsProvider::Fathom => {
            let src = config
                .script_url
                .as_deref()
                .unwrap_or("https://cdn.usefathom.com/script.js");
            format!(
                r#"<script src="{src}" data-site="{id}" defer></script>"#,
                id = config.id,
                src = src,
            )
        }
        AnalyticsProvider::Umami => {
            let src = config
                .script_url
                .as_deref()
                .unwrap_or("https://cloud.umami.is/script.js");
            format!(
                r#"<script defer src="{src}" data-website-id="{id}"></script>"#,
                id = config.id,
                src = src,
            )
        }
    }
}

/// Generate the JS that dynamically loads the analytics script.
/// Used inside the consent banner flow when the user clicks "Accept".
fn analytics_loader_js(config: &AnalyticsSection) -> String {
    match config.provider {
        AnalyticsProvider::Google => {
            format!(
                r#"var s=document.createElement('script');s.async=true;s.src='https://www.googletagmanager.com/gtag/js?id={id}';document.head.appendChild(s);s.onload=function(){{window.dataLayer=window.dataLayer||[];function gtag(){{dataLayer.push(arguments)}}gtag('js',new Date());gtag('config','{id}');}}"#,
                id = config.id
            )
        }
        AnalyticsProvider::Gtm => {
            format!(
                r#"(function(w,d,s,l,i){{w[l]=w[l]||[];w[l].push({{'gtm.start':new Date().getTime(),event:'gtm.js'}});var f=d.getElementsByTagName(s)[0],j=d.createElement(s),dl=l!='dataLayer'?'&l='+l:'';j.async=true;j.src='https://www.googletagmanager.com/gtm.js?id='+i+dl;f.parentNode.insertBefore(j,f)}})(window,document,'script','dataLayer','{id}')"#,
                id = config.id
            )
        }
        AnalyticsProvider::Plausible => {
            let src = plausible_script_url(config);
            format!(
                r#"var s=document.createElement('script');s.defer=true;s.setAttribute('data-domain','{id}');s.src='{src}';document.head.appendChild(s)"#,
                id = config.id,
                src = src,
            )
        }
        AnalyticsProvider::Fathom => {
            let src = config
                .script_url
                .as_deref()
                .unwrap_or("https://cdn.usefathom.com/script.js");
            format!(
                r#"var s=document.createElement('script');s.defer=true;s.setAttribute('data-site','{id}');s.src='{src}';document.head.appendChild(s)"#,
                id = config.id,
                src = src,
            )
        }
        AnalyticsProvider::Umami => {
            let src = config
                .script_url
                .as_deref()
                .unwrap_or("https://cloud.umami.is/script.js");
            format!(
                r#"var s=document.createElement('script');s.defer=true;s.setAttribute('data-website-id','{id}');s.src='{src}';document.head.appendChild(s)"#,
                id = config.id,
                src = src,
            )
        }
    }
}

/// GTM requires a `<noscript>` iframe in `<body>` for fallback tracking.
fn gtm_noscript(config: &AnalyticsSection) -> Option<String> {
    if config.provider == AnalyticsProvider::Gtm && !config.cookie_consent {
        Some(format!(
            r#"<noscript><iframe src="https://www.googletagmanager.com/ns.html?id={id}" height="0" width="0" style="display:none;visibility:hidden"></iframe></noscript>"#,
            id = config.id
        ))
    } else {
        None
    }
}

const CONSENT_BANNER_CSS: &str = r#"#seite-cookie-banner{position:fixed;bottom:0;left:0;right:0;background:#1a1a1a;color:#f5f5f5;padding:1rem 1.5rem;display:flex;align-items:center;justify-content:space-between;gap:1rem;z-index:9999;font-family:system-ui,-apple-system,sans-serif;font-size:0.9rem;box-shadow:0 -2px 10px rgba(0,0,0,0.15)}#seite-cookie-banner p{margin:0;flex:1}#seite-cookie-banner .seite-cb-buttons{display:flex;gap:0.5rem;flex-shrink:0}#seite-cookie-banner button{padding:0.45rem 1rem;border:none;border-radius:4px;cursor:pointer;font-size:0.85rem;font-family:inherit}#seite-cookie-accept{background:#2563eb;color:#fff}#seite-cookie-accept:hover{background:#1d4ed8}#seite-cookie-accept:focus-visible{outline:3px solid #93c5fd;outline-offset:2px}#seite-cookie-decline{background:transparent;color:#d4d4d4;border:1px solid #555}#seite-cookie-decline:hover{background:#333}#seite-cookie-decline:focus-visible{outline:3px solid #93c5fd;outline-offset:2px}@media(max-width:600px){#seite-cookie-banner{flex-direction:column;text-align:center}}"#;

/// Build the full consent banner HTML + JS that gates analytics on user action.
fn consent_banner_html(config: &AnalyticsSection) -> String {
    let loader_js = analytics_loader_js(config);
    format!(
        r#"<style>{css}</style>
<div id="seite-cookie-banner" role="dialog" aria-label="Cookie consent">
<p>This site uses cookies and analytics to improve your experience.</p>
<div class="seite-cb-buttons">
<button id="seite-cookie-accept">Accept</button>
<button id="seite-cookie-decline">Decline</button>
</div>
</div>
<script>
(function(){{
var c=localStorage.getItem('seite_analytics_consent');
var b=document.getElementById('seite-cookie-banner');
function load(){{{loader_js}}}
if(c==='accepted'){{load();b.style.display='none'}}
else if(c==='declined'){{b.style.display='none'}}
document.getElementById('seite-cookie-accept').addEventListener('click',function(){{localStorage.setItem('seite_analytics_consent','accepted');b.style.display='none';load()}});
document.getElementById('seite-cookie-decline').addEventListener('click',function(){{localStorage.setItem('seite_analytics_consent','declined');b.style.display='none'}});
}})();
</script>"#,
        css = CONSENT_BANNER_CSS,
        loader_js = loader_js,
    )
}

/// Inject analytics tags into a single HTML string.
///
/// - Without consent: injects script before `</head>` and optional noscript after `<body>`
/// - With consent: injects consent banner + gated loader before `</body>`
pub fn inject_analytics(html: &str, config: &AnalyticsSection) -> String {
    if config.cookie_consent {
        // Consent mode: inject banner + gated script before </body>
        let banner = consent_banner_html(config);
        if let Some(pos) = html.rfind("</body>") {
            let mut out = String::with_capacity(html.len() + banner.len() + 1);
            out.push_str(&html[..pos]);
            out.push('\n');
            out.push_str(&banner);
            out.push('\n');
            out.push_str(&html[pos..]);
            out
        } else {
            html.to_string()
        }
    } else {
        // Direct mode: inject script before </head>
        let script = analytics_script(config);
        let mut out = html.to_string();

        if let Some(pos) = out.find("</head>") {
            out.insert_str(pos, &format!("\n{script}\n"));
        }

        // GTM noscript fallback after <body...>
        if let Some(noscript) = gtm_noscript(config) {
            // Find <body> or <body ...> tag end
            if let Some(body_start) = out.find("<body") {
                if let Some(body_end) = out[body_start..].find('>') {
                    let insert_pos = body_start + body_end + 1;
                    out.insert_str(insert_pos, &format!("\n{noscript}"));
                }
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnalyticsProvider, AnalyticsSection};

    fn ga4_config(consent: bool) -> AnalyticsSection {
        AnalyticsSection {
            provider: AnalyticsProvider::Google,
            id: "G-TEST123".to_string(),
            cookie_consent: consent,
            script_url: None,
            extensions: vec![],
        }
    }

    fn gtm_config(consent: bool) -> AnalyticsSection {
        AnalyticsSection {
            provider: AnalyticsProvider::Gtm,
            id: "GTM-TEST123".to_string(),
            cookie_consent: consent,
            script_url: None,
            extensions: vec![],
        }
    }

    fn plausible_config() -> AnalyticsSection {
        AnalyticsSection {
            provider: AnalyticsProvider::Plausible,
            id: "example.com".to_string(),
            cookie_consent: false,
            script_url: None,
            extensions: vec![],
        }
    }

    fn fathom_config() -> AnalyticsSection {
        AnalyticsSection {
            provider: AnalyticsProvider::Fathom,
            id: "FATHOM123".to_string(),
            cookie_consent: false,
            script_url: None,
            extensions: vec![],
        }
    }

    fn umami_config() -> AnalyticsSection {
        AnalyticsSection {
            provider: AnalyticsProvider::Umami,
            id: "abc-123".to_string(),
            cookie_consent: false,
            script_url: Some("https://analytics.example.com/script.js".to_string()),
            extensions: vec![],
        }
    }

    const SIMPLE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Test</title></head>
<body><p>Hello</p></body>
</html>"#;

    #[test]
    fn test_ga4_direct_injection() {
        let result = inject_analytics(SIMPLE_HTML, &ga4_config(false));
        assert!(result.contains("googletagmanager.com/gtag/js?id=G-TEST123"));
        assert!(result.contains("gtag('config','G-TEST123')"));
        // Script should be before </head>
        let head_end = result.find("</head>").unwrap();
        let script_pos = result.find("googletagmanager.com/gtag").unwrap();
        assert!(script_pos < head_end);
    }

    #[test]
    fn test_ga4_consent_banner() {
        let result = inject_analytics(SIMPLE_HTML, &ga4_config(true));
        // Should have the consent banner
        assert!(result.contains("seite-cookie-banner"));
        assert!(result.contains("seite-cookie-accept"));
        assert!(result.contains("seite-cookie-decline"));
        assert!(result.contains("seite_analytics_consent"));
        // Banner should be before </body>
        let body_end = result.find("</body>").unwrap();
        let banner_pos = result.find("seite-cookie-banner").unwrap();
        assert!(banner_pos < body_end);
        // Should NOT have direct script in <head>
        let head_section = &result[..result.find("</head>").unwrap()];
        assert!(!head_section.contains("googletagmanager.com/gtag/js"));
    }

    #[test]
    fn test_gtm_direct_injection_with_noscript() {
        let result = inject_analytics(SIMPLE_HTML, &gtm_config(false));
        assert!(result.contains("gtm.js?id='+i+dl"));
        assert!(result.contains("GTM-TEST123"));
        // Should have noscript fallback after <body>
        assert!(result.contains("<noscript><iframe"));
        assert!(result.contains("ns.html?id=GTM-TEST123"));
    }

    #[test]
    fn test_gtm_consent_no_noscript() {
        let result = inject_analytics(SIMPLE_HTML, &gtm_config(true));
        // With consent mode, no noscript tag (it would load without consent)
        assert!(!result.contains("<noscript>"));
        assert!(result.contains("seite-cookie-banner"));
    }

    #[test]
    fn test_plausible_injection() {
        let result = inject_analytics(SIMPLE_HTML, &plausible_config());
        assert!(result.contains("plausible.io/js/script.js"));
        assert!(result.contains("data-domain=\"example.com\""));
    }

    #[test]
    fn test_fathom_injection() {
        let result = inject_analytics(SIMPLE_HTML, &fathom_config());
        assert!(result.contains("cdn.usefathom.com/script.js"));
        assert!(result.contains("data-site=\"FATHOM123\""));
    }

    #[test]
    fn test_umami_custom_script_url() {
        let result = inject_analytics(SIMPLE_HTML, &umami_config());
        assert!(result.contains("analytics.example.com/script.js"));
        assert!(result.contains("data-website-id=\"abc-123\""));
    }

    #[test]
    fn test_no_head_tag_unchanged() {
        let no_head = "<html><body><p>Hi</p></body></html>";
        let config = ga4_config(false);
        let result = inject_analytics(no_head, &config);
        // No </head> to inject into, should be unchanged
        assert_eq!(result, no_head);
    }

    #[test]
    fn test_no_body_tag_consent_unchanged() {
        let no_body = "<html><head><title>T</title></head></html>";
        let config = ga4_config(true);
        let result = inject_analytics(no_body, &config);
        // No </body> for consent banner, should be unchanged
        assert_eq!(result, no_body);
    }

    #[test]
    fn test_consent_banner_accessibility() {
        let result = inject_analytics(SIMPLE_HTML, &ga4_config(true));
        assert!(result.contains(r#"role="dialog""#));
        assert!(result.contains(r#"aria-label="Cookie consent""#));
    }

    #[test]
    fn test_consent_banner_responsive_css() {
        let result = inject_analytics(SIMPLE_HTML, &ga4_config(true));
        assert!(result.contains("@media(max-width:600px)"));
    }

    #[test]
    fn test_plausible_with_extensions() {
        let config = AnalyticsSection {
            provider: AnalyticsProvider::Plausible,
            id: "example.com".to_string(),
            cookie_consent: false,
            script_url: None,
            extensions: vec!["tagged-events".into(), "outbound-links".into()],
        };
        let result = inject_analytics(SIMPLE_HTML, &config);
        assert!(result.contains("plausible.io/js/script.tagged-events.outbound-links.js"));
        assert!(result.contains("data-domain=\"example.com\""));
    }

    #[test]
    fn test_plausible_single_extension() {
        let config = AnalyticsSection {
            provider: AnalyticsProvider::Plausible,
            id: "example.com".to_string(),
            cookie_consent: false,
            script_url: None,
            extensions: vec!["outbound-links".into()],
        };
        let result = inject_analytics(SIMPLE_HTML, &config);
        assert!(result.contains("plausible.io/js/script.outbound-links.js"));
    }

    #[test]
    fn test_plausible_extensions_with_consent() {
        let config = AnalyticsSection {
            provider: AnalyticsProvider::Plausible,
            id: "example.com".to_string(),
            cookie_consent: true,
            script_url: None,
            extensions: vec!["tagged-events".into(), "file-downloads".into()],
        };
        let result = inject_analytics(SIMPLE_HTML, &config);
        assert!(result.contains("plausible.io/js/script.tagged-events.file-downloads.js"));
        assert!(result.contains("seite-cookie-banner"));
        let head_end = result.find("</head>").unwrap();
        let head_section = &result[..head_end];
        assert!(!head_section.contains("plausible.io/js/script"));
    }

    #[test]
    fn test_plausible_custom_url_ignores_extensions() {
        let config = AnalyticsSection {
            provider: AnalyticsProvider::Plausible,
            id: "example.com".to_string(),
            cookie_consent: false,
            script_url: Some("https://proxy.example.com/js/script.js".into()),
            extensions: vec!["tagged-events".into()],
        };
        let result = inject_analytics(SIMPLE_HTML, &config);
        assert!(result.contains("proxy.example.com/js/script.js"));
        assert!(!result.contains("tagged-events"));
    }

    #[test]
    fn test_non_plausible_extensions_ignored() {
        let config = AnalyticsSection {
            provider: AnalyticsProvider::Google,
            id: "G-TEST123".to_string(),
            cookie_consent: false,
            script_url: None,
            extensions: vec!["tagged-events".into()],
        };
        let result = inject_analytics(SIMPLE_HTML, &config);
        assert!(result.contains("googletagmanager.com/gtag/js?id=G-TEST123"));
        assert!(!result.contains("tagged-events"));
    }
}
