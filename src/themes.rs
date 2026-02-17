/// Bundled themes. Each is a self-contained CSS string + base.html template.
/// Themes ship with the binary — no downloads needed.

pub struct Theme {
    pub name: &'static str,
    pub description: &'static str,
    pub base_html: &'static str,
}

pub fn all() -> Vec<Theme> {
    vec![
        default(),
        minimal(),
        dark(),
        docs(),
    ]
}

pub fn by_name(name: &str) -> Option<Theme> {
    all().into_iter().find(|t| t.name == name)
}

pub fn default() -> Theme {
    Theme {
        name: "default",
        description: "Clean, readable theme with system fonts",
        base_html: r#"<!DOCTYPE html>
<html lang="{{ site.language }}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ site.title }}{% endblock %}</title>
    <meta name="description" content="{{ site.description }}">
    <link rel="alternate" type="application/rss+xml" title="{{ site.title }}" href="/feed.xml">
    {% if translations %}{% for t in translations %}<link rel="alternate" hreflang="{{ t.lang }}" href="{{ site.base_url }}{{ t.url }}">
    {% endfor %}<link rel="alternate" hreflang="x-default" href="{{ site.base_url }}{{ page.url | default(value='/') }}">
    {% endif %}
    <style>
        *, *::before, *::after { box-sizing: border-box; }
        body { max-width: 720px; margin: 2rem auto; padding: 0 1rem; font-family: system-ui, -apple-system, sans-serif; line-height: 1.6; color: #222; }
        a { color: #0057b7; }
        a:hover { color: #003d82; }
        header { margin-bottom: 2rem; }
        header h1 { margin-bottom: 0.25rem; }
        header h1 a { text-decoration: none; color: inherit; }
        header p { margin-top: 0; color: #666; }
        nav { margin-bottom: 2rem; }
        nav a { margin-right: 1rem; text-decoration: none; font-weight: 500; }
        nav a:hover { text-decoration: underline; }
        .lang-switcher { display: inline-flex; gap: 0.5rem; font-size: 0.85rem; float: right; margin-top: 0.5rem; }
        .lang-switcher a { text-decoration: none; padding: 0.1rem 0.4rem; border-radius: 3px; }
        .lang-switcher a:hover { background: #eee; }
        .lang-switcher strong { padding: 0.1rem 0.4rem; }
        article { margin-bottom: 2rem; }
        time { color: #666; font-size: 0.9rem; }
        .tags span { background: #eee; padding: 0.15rem 0.5rem; border-radius: 3px; margin-right: 0.3rem; font-size: 0.85rem; }
        pre { background: #f6f6f6; padding: 1rem; border-radius: 4px; overflow-x: auto; }
        code { font-size: 0.9em; }
        blockquote { border-left: 3px solid #ddd; margin-left: 0; padding-left: 1rem; color: #555; }
        img { max-width: 100%; height: auto; }
        footer { margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #eee; color: #666; font-size: 0.85rem; }
    </style>
</head>
<body>
    <header>
        <h1><a href="/">{{ site.title }}</a></h1>
        {% if site.description %}<p>{{ site.description }}</p>{% endif %}
        {% if translations | length > 1 %}
        <nav class="lang-switcher">
            {% for t in translations %}{% if t.lang == lang %}<strong>{{ t.lang | upper }}</strong>{% else %}<a href="{{ t.url }}">{{ t.lang | upper }}</a>{% endif %}{% endfor %}
        </nav>
        {% endif %}
    </header>
    <main>
        {% block content %}{% endblock %}
    </main>
    <footer>
        <p>&copy; {{ site.author }}</p>
    </footer>
</body>
</html>"#,
    }
}

pub fn minimal() -> Theme {
    Theme {
        name: "minimal",
        description: "Ultra-minimal, typography-first theme",
        base_html: r#"<!DOCTYPE html>
<html lang="{{ site.language }}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ site.title }}{% endblock %}</title>
    <meta name="description" content="{{ site.description }}">
    <link rel="alternate" type="application/rss+xml" title="{{ site.title }}" href="/feed.xml">
    {% if translations %}{% for t in translations %}<link rel="alternate" hreflang="{{ t.lang }}" href="{{ site.base_url }}{{ t.url }}">
    {% endfor %}<link rel="alternate" hreflang="x-default" href="{{ site.base_url }}{{ page.url | default(value='/') }}">
    {% endif %}
    <style>
        *, *::before, *::after { box-sizing: border-box; }
        body { max-width: 600px; margin: 4rem auto; padding: 0 1.5rem; font-family: Georgia, 'Times New Roman', serif; line-height: 1.7; color: #333; font-size: 1.05rem; }
        a { color: #333; }
        header { margin-bottom: 3rem; }
        header h1 { font-size: 1.2rem; font-weight: normal; letter-spacing: 0.05em; text-transform: uppercase; }
        header h1 a { text-decoration: none; color: inherit; }
        header p { font-style: italic; color: #888; margin-top: -0.5rem; }
        .lang-switcher { display: inline-flex; gap: 0.5rem; font-size: 0.8rem; float: right; margin-top: 0.3rem; font-family: system-ui, sans-serif; }
        .lang-switcher a { text-decoration: none; color: #888; }
        .lang-switcher a:hover { color: #333; }
        .lang-switcher strong { color: #333; }
        article { margin-bottom: 2.5rem; }
        article h1 { font-size: 1.8rem; }
        article h3 a { text-decoration: none; color: inherit; }
        article h3 a:hover { text-decoration: underline; }
        time { color: #999; font-size: 0.85rem; font-style: italic; }
        .tags span { font-size: 0.8rem; color: #888; margin-right: 0.5rem; font-style: italic; }
        pre { background: #fafafa; padding: 1rem; border-left: 2px solid #ddd; overflow-x: auto; }
        code { font-size: 0.9em; }
        blockquote { border-left: 2px solid #ccc; margin-left: 0; padding-left: 1.5rem; font-style: italic; color: #666; }
        img { max-width: 100%; height: auto; }
        hr { border: none; border-top: 1px solid #ddd; margin: 2rem 0; }
        footer { margin-top: 4rem; font-size: 0.85rem; color: #aaa; }
    </style>
</head>
<body>
    <header>
        <h1><a href="/">{{ site.title }}</a></h1>
        {% if site.description %}<p>{{ site.description }}</p>{% endif %}
        {% if translations | length > 1 %}
        <nav class="lang-switcher">
            {% for t in translations %}{% if t.lang == lang %}<strong>{{ t.lang | upper }}</strong>{% else %}<a href="{{ t.url }}">{{ t.lang | upper }}</a>{% endif %}{% endfor %}
        </nav>
        {% endif %}
    </header>
    <main>
        {% block content %}{% endblock %}
    </main>
    <footer>
        <p>&copy; {{ site.author }}</p>
    </footer>
</body>
</html>"#,
    }
}

pub fn dark() -> Theme {
    Theme {
        name: "dark",
        description: "Dark mode theme, easy on the eyes",
        base_html: r#"<!DOCTYPE html>
<html lang="{{ site.language }}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ site.title }}{% endblock %}</title>
    <meta name="description" content="{{ site.description }}">
    <link rel="alternate" type="application/rss+xml" title="{{ site.title }}" href="/feed.xml">
    {% if translations %}{% for t in translations %}<link rel="alternate" hreflang="{{ t.lang }}" href="{{ site.base_url }}{{ t.url }}">
    {% endfor %}<link rel="alternate" hreflang="x-default" href="{{ site.base_url }}{{ page.url | default(value='/') }}">
    {% endif %}
    <style>
        *, *::before, *::after { box-sizing: border-box; }
        body { max-width: 720px; margin: 2rem auto; padding: 0 1rem; font-family: system-ui, -apple-system, sans-serif; line-height: 1.6; color: #e0e0e0; background: #1a1a2e; }
        a { color: #7eb8da; }
        a:hover { color: #a8d4f0; }
        header { margin-bottom: 2rem; }
        header h1 { margin-bottom: 0.25rem; }
        header h1 a { text-decoration: none; color: #e0e0e0; }
        header p { margin-top: 0; color: #888; }
        .lang-switcher { display: inline-flex; gap: 0.5rem; font-size: 0.85rem; float: right; margin-top: 0.5rem; }
        .lang-switcher a { text-decoration: none; color: #888; padding: 0.1rem 0.4rem; border-radius: 3px; }
        .lang-switcher a:hover { background: #2a2a4a; color: #e0e0e0; }
        .lang-switcher strong { padding: 0.1rem 0.4rem; color: #7eb8da; }
        article { margin-bottom: 2rem; }
        time { color: #888; font-size: 0.9rem; }
        .tags span { background: #2a2a4a; padding: 0.15rem 0.5rem; border-radius: 3px; margin-right: 0.3rem; font-size: 0.85rem; color: #aaa; }
        pre { background: #16162a; padding: 1rem; border-radius: 4px; overflow-x: auto; border: 1px solid #2a2a4a; }
        code { font-size: 0.9em; color: #c8d6e5; }
        blockquote { border-left: 3px solid #3a3a5a; margin-left: 0; padding-left: 1rem; color: #999; }
        img { max-width: 100%; height: auto; }
        footer { margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #2a2a4a; color: #666; font-size: 0.85rem; }
    </style>
</head>
<body>
    <header>
        <h1><a href="/">{{ site.title }}</a></h1>
        {% if site.description %}<p>{{ site.description }}</p>{% endif %}
        {% if translations | length > 1 %}
        <nav class="lang-switcher">
            {% for t in translations %}{% if t.lang == lang %}<strong>{{ t.lang | upper }}</strong>{% else %}<a href="{{ t.url }}">{{ t.lang | upper }}</a>{% endif %}{% endfor %}
        </nav>
        {% endif %}
    </header>
    <main>
        {% block content %}{% endblock %}
    </main>
    <footer>
        <p>&copy; {{ site.author }}</p>
    </footer>
</body>
</html>"#,
    }
}

pub fn docs() -> Theme {
    Theme {
        name: "docs",
        description: "Documentation-focused theme with sidebar layout",
        base_html: r#"<!DOCTYPE html>
<html lang="{{ site.language }}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{ site.title }}{% endblock %}</title>
    <meta name="description" content="{{ site.description }}">
    <link rel="alternate" type="application/rss+xml" title="{{ site.title }}" href="/feed.xml">
    {% if translations %}{% for t in translations %}<link rel="alternate" hreflang="{{ t.lang }}" href="{{ site.base_url }}{{ t.url }}">
    {% endfor %}<link rel="alternate" hreflang="x-default" href="{{ site.base_url }}{{ page.url | default(value='/') }}">
    {% endif %}
    <style>
        *, *::before, *::after { box-sizing: border-box; }
        body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #24292e; display: flex; min-height: 100vh; }
        a { color: #0366d6; text-decoration: none; }
        a:hover { text-decoration: underline; }
        .sidebar { width: 260px; padding: 2rem 1.5rem; background: #f6f8fa; border-right: 1px solid #e1e4e8; position: fixed; top: 0; bottom: 0; overflow-y: auto; }
        .sidebar h1 { font-size: 1.1rem; margin-bottom: 1.5rem; }
        .sidebar h1 a { color: #24292e; }
        .sidebar p { font-size: 0.85rem; color: #666; }
        .sidebar .lang-switcher { display: flex; gap: 0.4rem; margin-bottom: 1rem; font-size: 0.8rem; }
        .sidebar .lang-switcher a { color: #666; padding: 0.1rem 0.4rem; border-radius: 3px; }
        .sidebar .lang-switcher a:hover { background: #e1e4e8; text-decoration: none; }
        .sidebar .lang-switcher strong { padding: 0.1rem 0.4rem; color: #0366d6; }
        .sidebar nav { margin-top: 1.5rem; }
        .sidebar nav h3 { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: #888; margin: 1rem 0 0.5rem; }
        .sidebar nav ul { list-style: none; padding: 0; margin: 0; }
        .sidebar nav li { margin-bottom: 0.15rem; }
        .sidebar nav a { color: #24292e; font-size: 0.9rem; display: block; padding: 0.2rem 0.5rem; border-radius: 4px; }
        .sidebar nav a:hover { background: #e1e4e8; text-decoration: none; }
        .sidebar nav .active a { font-weight: 600; color: #0366d6; background: #e8f0fe; }
        .content { margin-left: 260px; padding: 2rem 3rem; max-width: 800px; flex: 1; }
        article { margin-bottom: 2rem; }
        article h1 { font-size: 2rem; border-bottom: 1px solid #e1e4e8; padding-bottom: 0.3rem; }
        article h2 { font-size: 1.5rem; margin-top: 2rem; border-bottom: 1px solid #eaecef; padding-bottom: 0.3rem; }
        time { color: #666; font-size: 0.9rem; }
        .tags span { background: #e1e4e8; padding: 0.1rem 0.5rem; border-radius: 3px; margin-right: 0.3rem; font-size: 0.85rem; }
        pre { background: #f6f8fa; padding: 1rem; border-radius: 6px; overflow-x: auto; border: 1px solid #e1e4e8; }
        code { font-size: 0.9em; background: #f6f8fa; padding: 0.1rem 0.3rem; border-radius: 3px; }
        pre code { background: none; padding: 0; }
        blockquote { border-left: 4px solid #dfe2e5; margin-left: 0; padding-left: 1rem; color: #6a737d; }
        img { max-width: 100%; height: auto; }
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #e1e4e8; padding: 0.5rem 1rem; text-align: left; }
        th { background: #f6f8fa; }
        footer { margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #e1e4e8; color: #666; font-size: 0.85rem; }
        @media (max-width: 768px) {
            .sidebar { position: static; width: 100%; border-right: none; border-bottom: 1px solid #e1e4e8; }
            .content { margin-left: 0; padding: 1rem; }
            body { flex-direction: column; }
        }
    </style>
</head>
<body>
    <div class="sidebar">
        <h1><a href="/">{{ site.title }}</a></h1>
        {% if site.description %}<p>{{ site.description }}</p>{% endif %}
        {% if translations | length > 1 %}
        <div class="lang-switcher">
            {% for t in translations %}{% if t.lang == lang %}<strong>{{ t.lang | upper }}</strong>{% else %}<a href="{{ t.url }}">{{ t.lang | upper }}</a>{% endif %}{% endfor %}
        </div>
        {% endif %}
        {% if nav %}
        <nav>
            {% for section in nav %}
                {% if section.name %}<h3>{{ section.label }}</h3>{% endif %}
                <ul>
                {% for item in section.items %}
                    <li{% if item.active %} class="active"{% endif %}><a href="{{ item.url }}">{{ item.title }}</a></li>
                {% endfor %}
                </ul>
            {% endfor %}
        </nav>
        {% endif %}
    </div>
    <div class="content">
        <main>
            {% block content %}{% endblock %}
        </main>
        <footer>
            <p>&copy; {{ site.author }}</p>
        </footer>
    </div>
</body>
</html>"#,
    }
}
