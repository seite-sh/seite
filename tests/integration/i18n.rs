use super::common::*;

#[test]
fn test_i18n_translated_pages_urls() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "i18n Test", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Prueba i18n");

    // Create a page in default language and its Spanish translation
    let about_en = "---\ntitle: About\n---\n\nAbout us.\n";
    fs::write(site_dir.join("content/pages/about.md"), about_en).unwrap();

    let about_es = "---\ntitle: Acerca de\n---\n\nSobre nosotros.\n";
    fs::write(site_dir.join("content/pages/about.es.md"), about_es).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // English version at root
    assert!(site_dir.join("dist/about.html").exists());
    // Spanish version under /es/
    assert!(site_dir.join("dist/es/about.html").exists());

    // Verify content
    let en_html = fs::read_to_string(site_dir.join("dist/about.html")).unwrap();
    assert!(en_html.contains("About us."));

    let es_html = fs::read_to_string(site_dir.join("dist/es/about.html")).unwrap();
    assert!(es_html.contains("Sobre nosotros."));
}

#[test]
fn test_i18n_per_language_index() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Index i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Índice i18n");

    // Create a Spanish post
    let es_post = "---\ntitle: Hola Mundo\ndate: 2025-01-15\n---\n\nContenido en español.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-15-hola-mundo.es.md"),
        es_post,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default index should have Hello World (English) but NOT Hola Mundo
    let index_en = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(index_en.contains("Hello World"));
    assert!(!index_en.contains("Hola Mundo"));

    // Spanish index should have Hola Mundo but NOT Hello World
    let index_es = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(index_es.contains("Hola Mundo"));
    assert!(!index_es.contains("Hello World"));
    // Spanish index uses the Spanish site title
    assert!(index_es.contains("Índice i18n"));
}

#[test]
fn test_i18n_sitemap_alternates() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Sitemap i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Mapa del sitio");

    // Create a page with translation
    let about_en = "---\ntitle: About\n---\n\nAbout us.\n";
    fs::write(site_dir.join("content/pages/about.md"), about_en).unwrap();
    let about_es = "---\ntitle: Acerca de\n---\n\nSobre nosotros.\n";
    fs::write(site_dir.join("content/pages/about.es.md"), about_es).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let sitemap = fs::read_to_string(site_dir.join("dist/sitemap.xml")).unwrap();
    // Sitemap should contain xhtml namespace for multilingual
    assert!(sitemap.contains("xmlns:xhtml"));
    // Should have hreflang alternate links
    assert!(sitemap.contains("xhtml:link"));
    assert!(sitemap.contains("hreflang"));
    // Should contain x-default for index
    assert!(sitemap.contains("x-default"));
}

#[test]
fn test_i18n_per_language_rss() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "RSS i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "RSS i18n ES");

    // Create a Spanish post
    let es_post = "---\ntitle: Hola Mundo\ndate: 2025-01-15\n---\n\nContenido.\n";
    fs::write(
        site_dir.join("content/posts/2025-01-15-hola-mundo.es.md"),
        es_post,
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default feed should exist and contain Hello World
    let feed_en = fs::read_to_string(site_dir.join("dist/feed.xml")).unwrap();
    assert!(feed_en.contains("Hello World"));
    assert!(!feed_en.contains("Hola Mundo"));

    // Spanish feed should exist
    let feed_es = fs::read_to_string(site_dir.join("dist/es/feed.xml")).unwrap();
    assert!(feed_es.contains("Hola Mundo"));
    assert!(!feed_es.contains("Hello World"));
}

#[test]
fn test_i18n_discovery_files() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Discovery i18n", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Discovery ES");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default language discovery files at root
    assert!(site_dir.join("dist/llms.txt").exists());
    assert!(site_dir.join("dist/llms-full.txt").exists());

    // Per-language discovery files
    assert!(site_dir.join("dist/es/llms.txt").exists());
    assert!(site_dir.join("dist/es/llms-full.txt").exists());
}

#[test]
fn test_i18n_backward_compat_single_language() {
    // A site without [languages] should work exactly like before
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Compat Test", "posts,pages");

    let site_dir = tmp.path().join("site");

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // No /es/ or other language directories
    let dist = site_dir.join("dist");
    assert!(dist.join("index.html").exists());
    assert!(dist.join("feed.xml").exists());
    assert!(dist.join("sitemap.xml").exists());

    // Sitemap should NOT have xhtml namespace
    let sitemap = fs::read_to_string(dist.join("sitemap.xml")).unwrap();
    assert!(!sitemap.contains("xmlns:xhtml"));
}

#[test]
fn test_i18n_data_language_maps_resolve_per_language() {
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "de", "Vertrauen");
    write_bilingual_trust_data(&site_dir);

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    // Default language (en) at /trust/
    let en = fs::read_to_string(site_dir.join("dist/trust/index.html")).unwrap();
    // Language maps must never leak through as raw objects.
    assert!(
        !en.contains("[object]"),
        "en trust index has unresolved map"
    );
    // Certification description, subprocessor purpose/location, FAQ Q&A → English
    assert!(en.contains("Information security management."));
    assert!(en.contains("Cloud infrastructure"));
    assert!(en.contains("United States"));
    assert!(en.contains("Where is data stored?"));
    assert!(en.contains("Data is stored in the EU."));
    // Plain-string field renders verbatim
    assert!(en.contains("ISO 27001"));
    assert!(en.contains("AWS"));
    // German variants must NOT appear on the English page
    assert!(!en.contains("Informationssicherheitsmanagement."));
    assert!(!en.contains("Cloud-Infrastruktur"));
    assert!(!en.contains("Daten werden in der EU gespeichert."));

    // Secondary language (de) at /de/trust/
    let de = fs::read_to_string(site_dir.join("dist/de/trust/index.html")).unwrap();
    assert!(
        !de.contains("[object]"),
        "de trust index has unresolved map"
    );
    assert!(de.contains("Informationssicherheitsmanagement."));
    assert!(de.contains("Cloud-Infrastruktur"));
    assert!(de.contains("Vereinigte Staaten"));
    assert!(de.contains("Wo werden Daten gespeichert?"));
    assert!(de.contains("Daten werden in der EU gespeichert."));
    // English variants must NOT appear on the German page
    assert!(!de.contains("Information security management."));
    assert!(!de.contains("Cloud infrastructure"));
    assert!(!de.contains("Data is stored in the EU."));
}

#[test]
fn test_i18n_data_plain_values_render_verbatim() {
    // A single-language site with plain-string data must render byte-for-byte
    // the same prose — the i18n filter is a no-op on scalars (no regression).
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");

    let site_dir = tmp.path().join("site");
    let trust_dir = site_dir.join("data/trust");
    fs::create_dir_all(&trust_dir).unwrap();
    fs::write(
        trust_dir.join("faq.yaml"),
        "- question: How is data encrypted?\n  answer: AES-256 at rest and TLS in transit.\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let html = fs::read_to_string(site_dir.join("dist/trust/index.html")).unwrap();
    assert!(!html.contains("[object]"));
    assert!(html.contains("How is data encrypted?"));
    assert!(html.contains("AES-256 at rest and TLS in transit."));
}

#[test]
fn test_i18n_data_partial_language_map_warns() {
    // A value present in some but not all configured languages should emit a
    // build warning flagging the missing translation.
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "de", "Vertrauen");

    let trust_dir = site_dir.join("data/trust");
    fs::create_dir_all(&trust_dir).unwrap();
    fs::write(
        trust_dir.join("faq.yaml"),
        "- question:\n    en: English only?\n    de: Nur Englisch?\n  answer:\n    en: This answer has no German translation.\n",
    )
    .unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success()
        .stderr(
            predicate::str::contains("missing translation").and(predicate::str::contains("de")),
        );
}

#[test]
fn test_i18n_data_trust_item_resolves_cert_scope_per_language() {
    // The cert-detail block in trust-item.html must resolve language-map prose
    // (scope) for the page's own language, on both the default and /de/ pages.
    let tmp = TempDir::new().unwrap();
    init_trust_site(&tmp, "site");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "de", "Vertrauen");
    write_bilingual_trust_data(&site_dir);

    // A certification detail page (default + German translation) that triggers
    // the cert-detail block via extra.type/extra.framework.
    let cert_dir = site_dir.join("content/trust/certifications");
    fs::create_dir_all(&cert_dir).unwrap();
    let fm = "---\ntitle: ISO 27001\nextra:\n  type: certification\n  framework: iso27001\n---\n\nDetails.\n";
    fs::write(cert_dir.join("iso27001.md"), fm).unwrap();
    fs::write(cert_dir.join("iso27001.de.md"), fm).unwrap();

    page_cmd()
        .arg("build")
        .current_dir(&site_dir)
        .assert()
        .success();

    let en = fs::read_to_string(site_dir.join("dist/trust/certifications/iso27001.html")).unwrap();
    assert!(en.contains("All production systems."));
    assert!(!en.contains("Alle Produktionssysteme."));

    let de =
        fs::read_to_string(site_dir.join("dist/de/trust/certifications/iso27001.html")).unwrap();
    assert!(de.contains("Alle Produktionssysteme."));
    assert!(!de.contains("All production systems."));
}

#[test]
fn test_new_with_lang_flag() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Lang New Test", "posts,pages");

    let site_dir = tmp.path().join("site");
    add_language(&site_dir, "es", "Prueba");

    page_cmd()
        .args(["new", "page", "About", "--lang", "es"])
        .current_dir(&site_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // File should have .es.md suffix
    assert!(site_dir.join("content/pages/about.es.md").exists());
}

#[test]
fn test_new_with_invalid_lang() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "site", "Invalid Lang", "posts,pages");

    let site_dir = tmp.path().join("site");
    // Don't add any languages

    page_cmd()
        .args(["new", "page", "About", "--lang", "fr"])
        .current_dir(&site_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown language"));
}

// --- i18n template context tests ---

#[test]
fn test_build_html_lang_uses_current_language() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "langsite", "Lang Site", "posts");
    let site_dir = tmp.path().join("langsite");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Sitio\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    // Spanish post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English page should have <html lang="en">
    let en_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        en_html.contains(r#"<html lang="en">"#),
        "English page should have <html lang=\"en\">"
    );

    // Spanish page should have <html lang="es">
    let es_html = fs::read_to_string(site_dir.join("dist/es/posts/hello.html")).unwrap();
    assert!(
        es_html.contains(r#"<html lang="es">"#),
        "Spanish page should have <html lang=\"es\">"
    );

    // English index should have <html lang="en">
    let en_index = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        en_index.contains(r#"<html lang="en">"#),
        "English index should have <html lang=\"en\">"
    );

    // Spanish index should have <html lang="es">
    let es_index = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(
        es_index.contains(r#"<html lang="es">"#),
        "Spanish index should have <html lang=\"es\">"
    );
}

#[test]
fn test_build_lang_prefix_in_context() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "lpsite", "LP Site", "posts");
    let site_dir = tmp.path().join("lpsite");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"LP ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\ntags:\n  - greetings\n---\nHello.",
    )
    .unwrap();
    // Spanish post with tag
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\ntags:\n  - saludos\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English post tag links should NOT have lang prefix
    let en_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        en_html.contains(r#"href="/tags/greetings/"#),
        "English tag link should not have lang prefix"
    );

    // Spanish post tag links SHOULD have /es prefix
    let es_html = fs::read_to_string(site_dir.join("dist/es/posts/hello.html")).unwrap();
    assert!(
        es_html.contains(r#"href="/es/tags/saludos/"#),
        "Spanish tag link should have /es lang prefix"
    );
}

#[test]
fn test_build_ui_strings_default() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "tsite", "T Site", "posts");
    let site_dir = tmp.path().join("tsite");

    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Index page should use t.search_placeholder (default: "Search…")
    let html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        html.contains("Search\u{2026}") || html.contains("Search..."),
        "Default search placeholder should be rendered"
    );
    // Skip to main content should be rendered
    assert!(
        html.contains("Skip to main content"),
        "Skip to content link should use default t.skip_to_content"
    );
}

#[test]
fn test_build_ui_strings_override() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "toverride", "T Override", "posts");
    let site_dir = tmp.path().join("toverride");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Sitio\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // Create i18n override file
    let i18n_dir = site_dir.join("data/i18n");
    fs::create_dir_all(&i18n_dir).unwrap();
    fs::write(
        i18n_dir.join("es.yaml"),
        "search_placeholder: \"Buscar\\u2026\"\nskip_to_content: \"Ir al contenido\"",
    )
    .unwrap();

    // English post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    // Spanish post
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Spanish index should use translated search placeholder
    let es_html = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(
        es_html.contains("Buscar"),
        "Spanish page should use translated search placeholder from data/i18n/es.yaml"
    );
    assert!(
        es_html.contains("Ir al contenido"),
        "Spanish page should use translated skip-to-content from data/i18n/es.yaml"
    );

    // English page should still use defaults
    let en_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        en_html.contains("Skip to main content"),
        "English page should still use default English strings"
    );
}

#[test]
fn test_build_nav_links_with_lang_prefix() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "navlang", "Nav Lang", "posts,pages");
    let site_dir = tmp.path().join("navlang");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"Nav ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // Create nav data with internal and external links
    let data_dir = site_dir.join("data");
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("nav.yaml"),
        "- title: Blog\n  url: /posts\n- title: GitHub\n  url: https://github.com\n  external: true",
    )
    .unwrap();

    // Content
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // English page: nav should have /posts (no lang prefix)
    let en_html = fs::read_to_string(site_dir.join("dist/index.html")).unwrap();
    assert!(
        en_html.contains(r#"href="/posts""#),
        "English nav should link to /posts without prefix"
    );
    // External link should NOT have lang prefix
    assert!(
        en_html.contains(r#"href="https://github.com""#),
        "External link should stay unchanged"
    );

    // Spanish page: nav should have /es/posts
    let es_html = fs::read_to_string(site_dir.join("dist/es/index.html")).unwrap();
    assert!(
        es_html.contains(r#"href="/es/posts""#),
        "Spanish nav should link to /es/posts with lang prefix"
    );
    // External link should NOT have lang prefix
    assert!(
        es_html.contains(r#"href="https://github.com""#),
        "External link should stay unchanged on Spanish page"
    );
    // External link should have target="_blank"
    assert!(
        es_html.contains(r#"target="_blank""#),
        "External link should have target=_blank"
    );
}

#[test]
fn test_build_default_language_in_context() {
    let tmp = TempDir::new().unwrap();
    init_site(&tmp, "dlsite", "DL Site", "posts");
    let site_dir = tmp.path().join("dlsite");

    // Add Spanish language
    let config = fs::read_to_string(site_dir.join("seite.toml")).unwrap();
    let config = format!("{config}\n[languages.es]\ntitle = \"DL ES\"\n");
    fs::write(site_dir.join("seite.toml"), config).unwrap();

    // English + Spanish posts
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.md"),
        "---\ntitle: Hello\ndate: 2025-01-15\n---\nHello.",
    )
    .unwrap();
    fs::write(
        site_dir.join("content/posts/2025-01-15-hello.es.md"),
        "---\ntitle: Hola\ndate: 2025-01-15\n---\nHola.",
    )
    .unwrap();

    page_cmd()
        .args(["build"])
        .current_dir(&site_dir)
        .assert()
        .success();

    // Both pages should have og:locale matching their language
    let en_html = fs::read_to_string(site_dir.join("dist/posts/hello.html")).unwrap();
    assert!(
        en_html.contains(r#"og:locale" content="en"#),
        "English page og:locale should be 'en'"
    );

    let es_html = fs::read_to_string(site_dir.join("dist/es/posts/hello.html")).unwrap();
    assert!(
        es_html.contains(r#"og:locale" content="es"#),
        "Spanish page og:locale should be 'es'"
    );
}
