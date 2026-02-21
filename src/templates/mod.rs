use std::path::Path;

use crate::config::CollectionConfig;
use crate::error::Result;
use crate::themes;

/// Default base template comes from the "default" bundled theme.
pub fn default_base() -> &'static str {
    // This is a static leak but it's a single allocation for the program lifetime.
    // We use it because themes::default() returns an owned Theme and we need &'static str.
    themes::default().base_html
}

pub const DEFAULT_INDEX: &str = r#"{% extends "base.html" %}
{% block title %}{% if pagination %}{{ collections[0].label }} — Page {{ pagination.current_page }} — {{ site.title }}{% else %}{{ site.title }}{% endif %}{% endblock %}
{% block content %}
{% if page.content %}
<div class="homepage-content">{{ page.content | safe }}</div>
{% endif %}
{% for collection in collections %}
<section>
    <h2>{{ collection.label }}</h2>
    {% for item in collection.items %}
    <article>
        <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
        {% if item.date %}<time>{{ item.date }}</time>{% endif %}
        {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
    </article>
    {% endfor %}
    {% if collection.items | length == 0 %}
    <p>No {{ collection.name }} yet.</p>
    {% endif %}
</section>
{% endfor %}
{% if pagination %}
<nav class="pagination">
    {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">&larr; {{ t.newer }}</a>{% endif %}
    <span>{{ t.page_n_of_total | replace(from="{n}", to=pagination.current_page ~ "") | replace(from="{total}", to=pagination.total_pages ~ "") }}</span>
    {% if pagination.next_url %}<a href="{{ pagination.next_url }}">{{ t.older }} &rarr;</a>{% endif %}
</nav>
{% endif %}
{% if not page and collections | length == 0 %}
<p>No content yet. Create some with <code>seite new post "My First Post"</code></p>
{% endif %}
{% endblock %}"#;

pub const DEFAULT_POST: &str = r#"{% extends "base.html" %}
{% block title %}{{ page.title }} - {{ site.title }}{% endblock %}
{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    {% if page.date %}<time>{{ page.date }}</time>{% endif %}
    {% if page.reading_time %}<span class="reading-time">{{ page.reading_time }} {{ t.min_read }}</span>{% endif %}
    {% if page.tags | length > 0 %}
    <div class="tags">
        {% for tag in page.tags %}<a href="{{ lang_prefix }}/tags/{{ tag | slugify }}/">{{ tag }}</a> {% endfor %}
    </div>
    {% endif %}
    <div class="content">{{ page.content | safe }}</div>
</article>
{% endblock %}"#;

pub const DEFAULT_DOC: &str = r##"{% extends "base.html" %}
{% block title %}{{ page.title }} - {{ site.title }}{% endblock %}
{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    {% if page.toc | length > 1 %}
    <nav class="toc">
        <h4>{{ t.contents }}</h4>
        <ul>
        {% for entry in page.toc %}<li class="toc-level-{{ entry.level }}"><a href="#{{ entry.id }}">{{ entry.text }}</a></li>
        {% endfor %}</ul>
    </nav>
    {% endif %}
    <div class="content">{{ page.content | safe }}</div>
</article>
{% endblock %}"##;

pub const DEFAULT_PAGE: &str = r#"{% extends "base.html" %}
{% block title %}{{ page.title }} - {{ site.title }}{% endblock %}
{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    <div class="content">{{ page.content | safe }}</div>
</article>
{% endblock %}"#;

pub const DEFAULT_404: &str = r#"{% extends "base.html" %}
{% block title %}{{ t.not_found_title }} — {{ site.title }}{% endblock %}
{% block content %}
<article>
    <h1>404 — {{ t.not_found_title }}</h1>
    <p>{{ t.not_found_message }} <a href="{{ lang_prefix }}/">{{ t.go_home }}</a>.</p>
</article>
{% endblock %}"#;

pub const DEFAULT_TAGS_INDEX: &str = r#"{% extends "base.html" %}
{% block title %}{{ t.tags }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ t.tags }}</h1>
<div class="tags-index">
    {% for tag in tags %}
    <a href="{{ tag.url }}" class="tag-link">{{ tag.name }} <span class="tag-count">({{ tag.count }})</span></a>
    {% endfor %}
</div>
{% endblock %}"#;

pub const DEFAULT_TAG: &str = r##"{% extends "base.html" %}
{% block title %}Tag: {{ tag_name }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ t.tagged }} "{{ tag_name }}"</h1>
<div class="tag-items">
    {% for item in items %}
    <article>
        <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
        {% if item.date %}<time>{{ item.date }}</time>{% endif %}
        {% if item.reading_time %}<span class="reading-time">{{ item.reading_time }} {{ t.min_read }}</span>{% endif %}
        {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
    </article>
    {% endfor %}
</div>
<p><a href="{{ tags_url }}">&larr; {{ t.all_tags }}</a></p>
{% endblock %}"##;

pub const DEFAULT_CHANGELOG_ENTRY: &str = r#"{% extends "base.html" %}
{% block title %}{{ page.title }} — {{ t.changelog }} — {{ site.title }}{% endblock %}
{% block content %}
<nav class="changelog-back"><a href="{{ lang_prefix }}/changelog/">&larr; {{ t.all_releases }}</a></nav>
<article class="changelog-entry">
    <header class="changelog-entry-header">
        <h1>{{ page.title }}</h1>
        <div class="changelog-meta">
            {% if page.date %}<time datetime="{{ page.date }}">{{ page.date }}</time>{% endif %}
            {% if page.tags | length > 0 %}
            <div class="changelog-tags">
                {% for tag in page.tags %}<span class="changelog-tag changelog-tag--{{ tag | slugify }}">{{ tag }}</span>{% endfor %}
            </div>
            {% endif %}
        </div>
    </header>
    {% if page.description %}<p class="changelog-summary">{{ page.description }}</p>{% endif %}
    <div class="content">{{ page.content | safe }}</div>
</article>
{% endblock %}"#;

pub const DEFAULT_CHANGELOG_INDEX: &str = r#"{% extends "base.html" %}
{% block title %}{% if pagination and pagination.current_page > 1 %}{{ t.changelog }} — {{ t.page_n_of_total | replace(from="{n}", to=pagination.current_page ~ "") | replace(from="{total}", to=pagination.total_pages ~ "") }} — {% endif %}{{ t.changelog }} — {{ site.title }}{% endblock %}
{% block content %}
<div class="changelog-header">
    <h1>{{ t.changelog }}</h1>
    <a href="{{ lang_prefix }}/feed.xml" class="changelog-rss" aria-label="RSS Feed">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 11a9 9 0 0 1 9 9"/><path d="M4 4a16 16 0 0 1 16 16"/><circle cx="5" cy="19" r="1"/></svg>
        RSS
    </a>
</div>
<div class="changelog-timeline">
    {% for item in items %}
    <article class="changelog-item">
        <div class="changelog-item-header">
            <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
            {% if item.date %}<time datetime="{{ item.date }}">{{ item.date }}</time>{% endif %}
        </div>
        {% if item.tags | length > 0 %}
        <div class="changelog-tags">
            {% for tag in item.tags %}<span class="changelog-tag changelog-tag--{{ tag | slugify }}">{{ tag }}</span>{% endfor %}
        </div>
        {% endif %}
        {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
    </article>
    {% endfor %}
</div>
{% if pagination %}
<nav class="pagination">
    {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">&larr; {{ t.newer }}</a>{% endif %}
    <span>{{ t.page_n_of_total | replace(from="{n}", to=pagination.current_page ~ "") | replace(from="{total}", to=pagination.total_pages ~ "") }}</span>
    {% if pagination.next_url %}<a href="{{ pagination.next_url }}">{{ t.older }} &rarr;</a>{% endif %}
</nav>
{% endif %}
{% endblock %}"#;

pub const DEFAULT_ROADMAP_ITEM: &str = r#"{% extends "base.html" %}
{% block title %}{{ page.title }} — {{ t.roadmap }} — {{ site.title }}{% endblock %}
{% block content %}
<nav class="roadmap-back"><a href="{{ lang_prefix }}/roadmap/">&larr; {{ t.roadmap }}</a></nav>
<article class="roadmap-entry">
    <header class="roadmap-entry-header">
        <h1>{{ page.title }}</h1>
        {% if page.tags | length > 0 %}
        <div class="roadmap-status-badges">
            {% for tag in page.tags %}<span class="roadmap-status roadmap-status--{{ tag | slugify }}">{{ tag }}</span>{% endfor %}
        </div>
        {% endif %}
    </header>
    {% if page.description %}<p class="roadmap-summary">{{ page.description }}</p>{% endif %}
    <div class="content">{{ page.content | safe }}</div>
</article>
{% endblock %}"#;

pub const DEFAULT_ROADMAP_INDEX: &str = r#"{% extends "base.html" %}
{% block title %}{{ t.roadmap }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ t.roadmap }}</h1>
<div class="roadmap-grouped">
    {% set in_progress = [] %}
    {% set planned = [] %}
    {% set done = [] %}
    {% set other = [] %}
    {% for item in items %}
        {% set status = item.tags | first | default(value="planned") %}
        {% if status == "in-progress" %}
            {% set_global in_progress = in_progress | concat(with=[item]) %}
        {% elif status == "planned" %}
            {% set_global planned = planned | concat(with=[item]) %}
        {% elif status == "done" %}
            {% set_global done = done | concat(with=[item]) %}
        {% else %}
            {% set_global other = other | concat(with=[item]) %}
        {% endif %}
    {% endfor %}
    {% if in_progress | length > 0 %}
    <section class="roadmap-section">
        <div class="roadmap-section-header"><span class="roadmap-status roadmap-status--in-progress">{{ t.in_progress }}</span> <span class="roadmap-count">{{ in_progress | length }}</span></div>
        <div class="roadmap-items">
            {% for item in in_progress %}
            <div class="roadmap-card">
                <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
                {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
            </div>
            {% endfor %}
        </div>
    </section>
    {% endif %}
    {% if planned | length > 0 %}
    <section class="roadmap-section">
        <div class="roadmap-section-header"><span class="roadmap-status roadmap-status--planned">{{ t.planned }}</span> <span class="roadmap-count">{{ planned | length }}</span></div>
        <div class="roadmap-items">
            {% for item in planned %}
            <div class="roadmap-card">
                <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
                {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
            </div>
            {% endfor %}
        </div>
    </section>
    {% endif %}
    {% if done | length > 0 %}
    <section class="roadmap-section">
        <div class="roadmap-section-header"><span class="roadmap-status roadmap-status--done">{{ t.done }}</span> <span class="roadmap-count">{{ done | length }}</span></div>
        <div class="roadmap-items">
            {% for item in done %}
            <div class="roadmap-card">
                <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
                {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
            </div>
            {% endfor %}
        </div>
    </section>
    {% endif %}
    {% if other | length > 0 %}
    <section class="roadmap-section">
        <div class="roadmap-section-header"><span class="roadmap-status">{{ t.other }}</span> <span class="roadmap-count">{{ other | length }}</span></div>
        <div class="roadmap-items">
            {% for item in other %}
            <div class="roadmap-card">
                <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
                {% if item.tags | length > 0 %}<div class="roadmap-status-badges">{% for tag in item.tags %}<span class="roadmap-status roadmap-status--{{ tag | slugify }}">{{ tag }}</span>{% endfor %}</div>{% endif %}
                {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
            </div>
            {% endfor %}
        </div>
    </section>
    {% endif %}
</div>
{% endblock %}"#;

pub const DEFAULT_ROADMAP_KANBAN: &str = r#"{% extends "base.html" %}
{% block title %}{{ t.roadmap }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ t.roadmap }}</h1>
<div class="roadmap-kanban">
    {% set in_progress = [] %}
    {% set planned = [] %}
    {% set done = [] %}
    {% for item in items %}
        {% set status = item.tags | first | default(value="planned") %}
        {% if status == "in-progress" %}
            {% set_global in_progress = in_progress | concat(with=[item]) %}
        {% elif status == "done" %}
            {% set_global done = done | concat(with=[item]) %}
        {% else %}
            {% set_global planned = planned | concat(with=[item]) %}
        {% endif %}
    {% endfor %}
    <div class="roadmap-column">
        <h2 class="roadmap-column-header"><span class="roadmap-status roadmap-status--planned">{{ t.planned }}</span> <span class="roadmap-count">{{ planned | length }}</span></h2>
        {% for item in planned %}
        <div class="roadmap-card">
            <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
            {% if item.description %}<p>{{ item.description }}</p>{% endif %}
        </div>
        {% endfor %}
    </div>
    <div class="roadmap-column">
        <h2 class="roadmap-column-header"><span class="roadmap-status roadmap-status--in-progress">{{ t.in_progress }}</span> <span class="roadmap-count">{{ in_progress | length }}</span></h2>
        {% for item in in_progress %}
        <div class="roadmap-card">
            <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
            {% if item.description %}<p>{{ item.description }}</p>{% endif %}
        </div>
        {% endfor %}
    </div>
    <div class="roadmap-column">
        <h2 class="roadmap-column-header"><span class="roadmap-status roadmap-status--done">{{ t.done }}</span> <span class="roadmap-count">{{ done | length }}</span></h2>
        {% for item in done %}
        <div class="roadmap-card">
            <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
            {% if item.description %}<p>{{ item.description }}</p>{% endif %}
        </div>
        {% endfor %}
    </div>
</div>
{% endblock %}"#;

pub const DEFAULT_ROADMAP_TIMELINE: &str = r#"{% extends "base.html" %}
{% block title %}{{ t.roadmap }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ t.roadmap }}</h1>
<div class="roadmap-timeline">
    {% for item in items %}
    <div class="roadmap-milestone {% if loop.index is odd %}roadmap-milestone--left{% else %}roadmap-milestone--right{% endif %}">
        <div class="roadmap-milestone-dot">
            {% set status = item.tags | first | default(value="planned") %}
            <span class="roadmap-status roadmap-status--{{ status | slugify }}"></span>
        </div>
        <div class="roadmap-milestone-content">
            <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
            {% if item.tags | length > 0 %}
            <div class="roadmap-status-badges">
                {% for tag in item.tags %}<span class="roadmap-status roadmap-status--{{ tag | slugify }}">{{ tag }}</span>{% endfor %}
            </div>
            {% endif %}
            {% if item.description %}<p>{{ item.description }}</p>{% endif %}
        </div>
    </div>
    {% endfor %}
</div>
{% endblock %}"#;

pub const DEFAULT_TRUST_ITEM: &str = r##"{% extends "base.html" %}
{% block title %}{{ page.title }} — {{ t.trust_center }} — {{ site.title }}{% endblock %}
{% block content %}
<article class="trust-page">
    <nav class="trust-breadcrumb"><a href="{{ lang_prefix }}/trust/">{{ t.trust_center }}</a> &rsaquo; {{ page.title }}</nav>
    <h1>{{ page.title }}</h1>
    {% if page.description %}<p class="trust-description">{{ page.description }}</p>{% endif %}
    {% if page.extra and page.extra.type is defined and page.extra.type == "certification" and page.extra.framework is defined %}
    {% if data.trust.certifications %}
    {% for cert in data.trust.certifications %}
    {% if cert.framework == page.extra.framework %}
    <div class="cert-detail">
        <span class="cert-status cert-status--{{ cert.status }}">
            {% if cert.status == "active" %}{{ t.active }}{% elif cert.status == "in_progress" %}{{ t.in_progress }}{% else %}{{ t.planned }}{% endif %}
        </span>
        {% if cert.auditor %}<div class="cert-meta"><strong>{{ t.auditor }}:</strong> {{ cert.auditor }}</div>{% endif %}
        {% if cert.scope %}<div class="cert-meta"><strong>{{ t.scope }}:</strong> {{ cert.scope }}</div>{% endif %}
        {% if cert.issued %}<div class="cert-meta"><strong>{{ t.issued }}:</strong> {{ cert.issued }}</div>{% endif %}
        {% if cert.expires %}<div class="cert-meta"><strong>{{ t.expires }}:</strong> {{ cert.expires }}</div>{% endif %}
    </div>
    {% endif %}
    {% endfor %}
    {% endif %}
    {% endif %}
    <div class="content">{{ page.content | safe }}</div>
</article>
{% endblock %}"##;

pub const DEFAULT_TRUST_INDEX: &str = r##"{% extends "base.html" %}
{% block title %}{{ t.trust_center }} — {{ site.title }}{% endblock %}
{% block content %}
<div class="trust-hub">
    <div class="trust-hero">
        <h1>{{ t.trust_center }}</h1>
        <p>{{ t.trust_hero_subtitle | replace(from="{site}", to=site.title) }}</p>
    </div>

    {% if data.trust.certifications %}
    <section class="trust-section">
        <h2>{{ t.certifications_compliance }}</h2>
        <div class="cert-grid">
            {% for cert in data.trust.certifications %}
            <div class="cert-card">
                <div class="cert-card__header">
                    <span class="cert-status cert-status--{{ cert.status }}">
                        {% if cert.status == "active" %}{{ t.active }}{% elif cert.status == "in_progress" %}{{ t.in_progress }}{% else %}{{ t.planned }}{% endif %}
                    </span>
                </div>
                <h3 class="cert-card__title">{{ cert.name }}</h3>
                {% if cert.description %}<p class="cert-card__desc">{{ cert.description }}</p>{% endif %}
                {% if cert.slug %}<a href="{{ lang_prefix }}/trust/certifications/{{ cert.slug }}" class="cert-card__link">{{ t.learn_more }} &rarr;</a>{% endif %}
            </div>
            {% endfor %}
        </div>
    </section>
    {% endif %}

    {% for collection in collections %}
    {% if collection.items | length > 0 %}
    <section class="trust-section">
        <h2>{{ collection.label }}</h2>
        {% for item in collection.items %}
        {% if item.slug is not starting_with("certifications/") %}
        <article class="trust-item">
            <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
            {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
        </article>
        {% endif %}
        {% endfor %}
    </section>
    {% endif %}
    {% endfor %}

    {% if data.trust.subprocessors %}
    <section class="trust-section">
        <h2>{{ t.subprocessors }}</h2>
        <div class="subprocessor-table-wrap">
        <table class="subprocessor-table">
            <thead><tr><th>{{ t.vendor }}</th><th>{{ t.purpose }}</th><th>{{ t.location }}</th><th>{{ t.dpa }}</th></tr></thead>
            <tbody>
            {% for sp in data.trust.subprocessors %}
            <tr>
                <td>{{ sp.name }}</td>
                <td>{{ sp.purpose | default(value="") }}</td>
                <td>{{ sp.location | default(value="") }}</td>
                <td>{% if sp.dpa %}{{ t.yes }}{% else %}{{ t.no }}{% endif %}</td>
            </tr>
            {% endfor %}
            </tbody>
        </table>
        </div>
    </section>
    {% endif %}

    {% if data.trust.faq %}
    <section class="trust-section faq-section">
        <h2>{{ t.faq }}</h2>
        {% for item in data.trust.faq %}
        <details class="faq-item">
            <summary>{{ item.question }}</summary>
            <div class="faq-answer">{{ item.answer }}</div>
        </details>
        {% endfor %}
    </section>
    {% endif %}
</div>
{% endblock %}"##;

fn get_default_template(name: &str) -> Option<&'static str> {
    match name {
        "base.html" => Some(default_base()),
        "index.html" => Some(DEFAULT_INDEX),
        "post.html" => Some(DEFAULT_POST),
        "doc.html" => Some(DEFAULT_DOC),
        "page.html" => Some(DEFAULT_PAGE),
        "trust-item.html" => Some(DEFAULT_TRUST_ITEM),
        "trust-index.html" => Some(DEFAULT_TRUST_INDEX),
        "404.html" => Some(DEFAULT_404),
        "tags.html" => Some(DEFAULT_TAGS_INDEX),
        "tag.html" => Some(DEFAULT_TAG),
        "changelog-entry.html" => Some(DEFAULT_CHANGELOG_ENTRY),
        "changelog-index.html" => Some(DEFAULT_CHANGELOG_INDEX),
        "roadmap-item.html" => Some(DEFAULT_ROADMAP_ITEM),
        "roadmap-index.html" => Some(DEFAULT_ROADMAP_INDEX),
        "roadmap-kanban.html" => Some(DEFAULT_ROADMAP_KANBAN),
        "roadmap-timeline.html" => Some(DEFAULT_ROADMAP_TIMELINE),
        _ => None,
    }
}

/// Load Tera templates from the user's template directory, falling back to
/// embedded defaults for any template not provided.
pub fn load_templates(template_dir: &Path, collections: &[CollectionConfig]) -> Result<tera::Tera> {
    #[allow(clippy::manual_unwrap_or_default)]
    let mut tera = if template_dir.exists() {
        let glob_pattern = format!("{}/**/*.html", template_dir.display());
        match tera::Tera::new(&glob_pattern) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("⚠ Warning: failed to parse user templates, using defaults: {e}");
                tera::Tera::default()
            }
        }
    } else {
        tera::Tera::default()
    };

    // Disable auto-escaping: our templates control all output, and user content
    // already uses `| safe`. Auto-escaping causes URLs to be mangled with &#x2F;
    // in href attributes (e.g. `href="&#x2F;posts"` instead of `href="/posts"`).
    tera.autoescape_on(vec![]);

    // Always ensure essential templates exist
    for name in [
        "base.html",
        "index.html",
        "404.html",
        "tags.html",
        "tag.html",
        "roadmap-kanban.html",
        "roadmap-timeline.html",
    ] {
        if tera.get_template(name).is_err() {
            if let Some(content) = get_default_template(name) {
                tera.add_raw_template(name, content)?;
            }
        }
    }

    // Register default template for each collection's default_template
    for collection in collections {
        let tmpl_name = &collection.default_template;
        if tera.get_template(tmpl_name).is_err() {
            if let Some(content) = get_default_template(tmpl_name) {
                tera.add_raw_template(tmpl_name, content)?;
            }
        }
        // Also register collection-specific index templates (e.g., trust-index.html)
        let index_tmpl = format!("{}-index.html", collection.name);
        if tera.get_template(&index_tmpl).is_err() {
            if let Some(content) = get_default_template(&index_tmpl) {
                tera.add_raw_template(&index_tmpl, content)?;
            }
        }
    }

    Ok(tera)
}
