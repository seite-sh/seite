use std::collections::HashMap;
use std::path::Path;

use crate::config::CollectionConfig;
use crate::diagnostics::{did_you_mean, Diagnostic, Diagnostics};
use crate::error::Result;
use crate::themes;

/// Default base template comes from the "default" bundled theme.
pub fn default_base() -> &'static str {
    // This is a static leak but it's a single allocation for the program lifetime.
    // We use it because themes::default() returns an owned Theme and we need &'static str.
    themes::default().base_html
}

pub const DEFAULT_INDEX: &str = r#"{% extends "base.html" %}
{% block title %}{% set _home_url = lang_prefix ~ "/" %}{% if pagination %}{{ collections[0].label }} — Page {{ pagination.current_page }} — {{ site.title }}{% elif page and page.collection and page.title and page.url != _home_url %}{{ page.title }} — {{ site.title }}{% else %}{{ site.title }}{% endif %}{% endblock %}
{% block site_title %}<h1 class="site-title"><a href="{{ lang_prefix }}/">{{ site.title }}</a></h1>{% endblock %}
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
        {% if page.taxonomies and page.taxonomies.tags %}{% for tag in page.taxonomies.tags %}<a href="{{ tag.url }}">{{ tag.name }}</a> {% endfor %}{% else %}{% for tag in page.tags %}<a href="{{ lang_prefix }}/tags/{{ tag | slugify }}/">{{ tag }}</a> {% endfor %}{% endif %}
    </div>
    {% endif %}{% if page.taxonomies %}{% for tax_name, terms in page.taxonomies %}{% if tax_name != "tags" and terms | length > 0 %}
    <div class="tags terms-{{ tax_name }}">
        {% for term in terms %}<a href="{{ term.url }}">{{ term.name }}</a> {% endfor %}
    </div>{% endif %}{% endfor %}{% endif %}
    <div class="content">{{ page.content | safe }}</div>{% if page.series %}
    <nav class="article-nav series-nav" aria-label="{{ t.series }}">
        <span>{{ t.series_part | replace(from="{n}", to=page.series.index ~ "") | replace(from="{total}", to=page.series.total ~ "") }} &middot; <a href="{{ page.series.url }}">{{ page.series.name }}</a></span>
        <span>{% if page.series.prev %}<a href="{{ page.series.prev.url }}" rel="prev">&larr; {{ page.series.prev.title }}</a>{% endif %}{% if page.series.prev and page.series.next %} &middot; {% endif %}{% if page.series.next %}<a href="{{ page.series.next.url }}" rel="next">{{ page.series.next.title }} &rarr;</a>{% endif %}</span>
    </nav>{% endif %}
</article>
{% if prev_post or next_post %}
<nav class="article-nav">
    {% if next_post %}<a href="{{ next_post.url }}">&larr; {{ next_post.title }}</a>{% endif %}
    {% if prev_post %}<a href="{{ prev_post.url }}">{{ prev_post.title }} &rarr;</a>{% endif %}
</nav>
{% endif %}{% if page.related %}
<aside class="related" aria-labelledby="related-title">
    <h2 id="related-title">{{ t.related }}</h2>
    <ul>
        {% for r in page.related %}<li><a href="{{ r.url }}">{{ r.title }}</a></li>
        {% endfor %}</ul>
</aside>{% endif %}
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

pub const DEFAULT_DOCS_INDEX: &str = r##"{% extends "base.html" %}
{% block title %}{% if page.title %}{{ page.title }} — {% endif %}{{ site.title }}{% endblock %}
{% block content %}
{% if page.content %}
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
{% else %}
<h1>{{ page.title | default(value=t.documentation) }}</h1>
{% if nav %}
{% for section in nav %}
<section class="docs-section-overview">
    {% if section.label %}<h2>{{ section.label }}</h2>{% endif %}
    <ul class="docs-section-list">
    {% for item in section.items %}
        <li><a href="{{ item.url }}">{{ item.title }}</a></li>
    {% endfor %}
    </ul>
</section>
{% endfor %}
{% elif items %}
<ul>
{% for item in items %}
    <li><a href="{{ item.url }}">{{ item.title }}</a>{% if item.description %} — {{ item.description }}{% endif %}</li>
{% endfor %}
</ul>
{% endif %}
{% endif %}
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
{% block head %}{% if term and term.feed_url %}<link rel="alternate" type="application/rss+xml" title="{{ site.title }} — {{ tag_name }}" href="{{ term.feed_url }}">
    <link rel="alternate" type="application/atom+xml" title="{{ site.title }} — {{ tag_name }}" href="{{ term.url }}atom.xml">{% endif %}{% endblock %}
{% block content %}
<h1>{{ t.tagged }} "{{ tag_name }}"</h1>
<div class="tag-items">
    {% for item in items %}
    <article>
        <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
        {% if item.date %}<time>{{ item.date }}</time>{% endif %}
        {% if item.reading_time %}<span class="reading-time">{{ item.reading_time }} {{ t.min_read }}</span>{% endif %}
        {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
    </article>
    {% endfor %}
</div>{% if pagination %}
<nav class="pagination">
    {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">&larr; {{ t.newer }}</a>{% endif %}
    <span>{{ t.page_n_of_total | replace(from="{n}", to=pagination.current_page ~ "") | replace(from="{total}", to=pagination.total_pages ~ "") }}</span>
    {% if pagination.next_url %}<a href="{{ pagination.next_url }}">{{ t.older }} &rarr;</a>{% endif %}
</nav>{% endif %}
<p><a href="{{ tags_url }}">&larr; {{ t.all_tags }}</a></p>
{% endblock %}"##;

pub const DEFAULT_TAXONOMY: &str = r##"{% extends "base.html" %}
{% block title %}{{ taxonomy.label }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ taxonomy.label }}</h1>
<div class="tags-index">
    {% for term in terms %}
    <a href="{{ term.url }}" class="tag-link">{{ term.name }} <span class="tag-count">({{ term.count }})</span></a>
    {% endfor %}
</div>
{% endblock %}"##;

pub const DEFAULT_TERM: &str = r##"{% extends "base.html" %}
{% block title %}{{ taxonomy.label }}: {{ term.name }} — {{ site.title }}{% endblock %}
{% block head %}{% if term.feed_url %}<link rel="alternate" type="application/rss+xml" title="{{ site.title }} — {{ term.name }}" href="{{ term.feed_url }}">
    <link rel="alternate" type="application/atom+xml" title="{{ site.title }} — {{ term.name }}" href="{{ term.url }}atom.xml">{% endif %}{% endblock %}
{% block content %}
<h1>{{ taxonomy.label }}: {{ term.name }}</h1>
<div class="tag-items">
    {% for item in items %}
    <article>
        <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
        {% if item.date %}<time>{{ item.date }}</time>{% endif %}
        {% if item.reading_time %}<span class="reading-time">{{ item.reading_time }} {{ t.min_read }}</span>{% endif %}
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
<p><a href="{{ taxonomy.url }}">&larr; {{ taxonomy.label }}</a></p>
{% endblock %}"##;

pub const DEFAULT_SERIES: &str = r##"{% extends "base.html" %}
{% block title %}{{ series.name }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>{{ series.name }}</h1>
<ol class="series-parts">
    {% for item in items %}
    <li>
        <a href="{{ item.url }}">{{ item.title }}</a>
        {% if item.date %}<time>{{ item.date }}</time>{% endif %}
        {% if item.description %}<p>{{ item.description }}</p>{% endif %}
    </li>
    {% endfor %}
</ol>
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
                <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
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
                <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
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
                <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
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
                <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
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
            <h2><a href="{{ item.url }}">{{ item.title }}</a></h2>
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
        {% if cert.auditor %}<div class="cert-meta"><strong>{{ t.auditor }}:</strong> {{ cert.auditor | i18n(lang=lang) }}</div>{% endif %}
        {% if cert.scope %}<div class="cert-meta"><strong>{{ t.scope }}:</strong> {{ cert.scope | i18n(lang=lang) }}</div>{% endif %}
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
                <h3 class="cert-card__title">{{ cert.name | i18n(lang=lang) }}</h3>
                {% if cert.description %}<p class="cert-card__desc">{{ cert.description | i18n(lang=lang) }}</p>{% endif %}
                {% if cert.slug %}<a href="{{ lang_prefix }}/trust/certifications/{{ cert.slug }}" class="cert-card__link" aria-label="{{ t.learn_more | escape_attr }}: {{ cert.name | i18n(lang=lang) | escape_attr }}">{{ t.learn_more }} &rarr;</a>{% endif %}
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
                <td>{{ sp.name | i18n(lang=lang) }}</td>
                <td>{{ sp.purpose | default(value="") | i18n(lang=lang) }}</td>
                <td>{{ sp.location | default(value="") | i18n(lang=lang) }}</td>
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
            <summary>{{ item.question | i18n(lang=lang) }}</summary>
            <div class="faq-answer">{{ item.answer | i18n(lang=lang) }}</div>
        </details>
        {% endfor %}
    </section>
    {% endif %}
</div>
{% endblock %}"##;

/// seite-owned partials under the reserved `seite/` namespace. Every bundled
/// theme includes them (`{% include "seite/head.html" %}` etc.), so SEO/GEO,
/// search and i18n fixes ship with the binary instead of living in 10 copies
/// of `base.html`. A user file at `templates/seite/<name>` overrides one, at
/// the cost of no longer receiving those fixes.
///
/// Extension points for later features (documented in the partials):
/// - `seite/head.html`: generated OG cards hook in next to the `og:image` block.
/// - `seite/scripts.html`: new seite-owned scripts are added as one more include.
/// - `seite/search-script.html`: the search implementation (index format and ranking).
pub const SEITE_PARTIALS: &[(&str, &str)] = &[
    ("seite/head.html", include_str!("seite/head.html")),
    ("seite/search.html", include_str!("seite/search.html")),
    (
        "seite/search-script.html",
        include_str!("seite/search-script.html"),
    ),
    (
        "seite/lang-switcher.html",
        include_str!("seite/lang-switcher.html"),
    ),
    ("seite/scripts.html", include_str!("seite/scripts.html")),
    ("seite/comments.html", include_str!("seite/comments.html")),
    (
        "seite/newsletter.html",
        include_str!("seite/newsletter.html"),
    ),
];

/// Escape a string for an HTML attribute value: `"`, `<`, `>` and any `&`
/// that does not already start a character reference. Plain text and URLs
/// without those characters come out unchanged.
pub fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for (i, c) in value.char_indices() {
        match c {
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' if !starts_char_reference(&value[i + 1..]) => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }
    out
}

/// Whether `rest` (the text after an `&`) is a character reference such as
/// `amp;`, `#39;` or `#x2F;`.
fn starts_char_reference(rest: &str) -> bool {
    let Some(end) = rest.find(';') else {
        return false;
    };
    let name = &rest[..end];
    if let Some(num) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        !num.is_empty() && num.chars().all(|c| c.is_ascii_hexdigit())
    } else if let Some(num) = name.strip_prefix('#') {
        !num.is_empty() && num.chars().all(|c| c.is_ascii_digit())
    } else {
        !name.is_empty() && name.len() <= 32 && name.chars().all(|c| c.is_ascii_alphanumeric())
    }
}

/// JSON for a `<script>` element: like `json_encode`, with every `<`
/// written as `\u003c` so a value can never close the script
/// (`</script>`) or open a comment (`<!--`).
pub fn json_script(value: &serde_json::Value) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|_| "null".to_string())
        .replace('<', "\\u003c")
}

/// The function [`guard_macro_recursion`] calls first in every macro body.
const MACRO_GUARD_FN: &str = "__seite_macro_guard";

/// Stack that nested macro calls may use below the shallowest macro call
/// seen on the thread. The smallest stack seite renders on is Windows'
/// 1 MiB main thread (spawned threads get 2 MiB). One level of macro
/// recursion takes about 4 KiB in release builds (25 KiB in debug), so this
/// leaves room for legitimate recursion (menus, trees) about 100 levels
/// deep and stops a runaway one well before the stack overflows.
const MACRO_STACK_BUDGET: usize = 512 * 1024;

thread_local! {
    /// Highest stack address of a macro guard call on this thread (the
    /// stack grows down on every platform seite ships for). Never reset:
    /// renders start within a few KiB of each other on one thread, far less
    /// than the budget.
    static MACRO_STACK_TOP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Make a recursive macro that never stops fail the render with an error
/// instead of overflowing the stack, which aborts the whole process (`seite
/// check`, the hooks, the dev server, the MCP server). Tera has no
/// recursion limit, and whether a recursive macro terminates can't be
/// decided statically, so every macro body starts with a call to a function
/// that measures how deep the stack is. The call renders nothing, so output
/// stays byte-identical. Call after every template is added.
pub(crate) fn guard_macro_recursion(tera: &mut tera::Tera) {
    use tera::ast::{Expr, ExprVal, FunctionCall, Node, WS};
    tera.register_function(MACRO_GUARD_FN, macro_guard);
    for template in tera.templates.values_mut() {
        for (name, definition) in template.macros.iter_mut() {
            let guarded = matches!(
                definition.body.first(),
                Some(Node::VariableBlock(_, Expr { val: ExprVal::FunctionCall(call), .. }))
                    if call.name == MACRO_GUARD_FN
            );
            if guarded {
                continue;
            }
            let args = HashMap::from([(
                "name".to_string(),
                Expr::new(ExprVal::String(format!("{}::{name}", template.name))),
            )]);
            let call = Expr::new(ExprVal::FunctionCall(FunctionCall {
                name: MACRO_GUARD_FN.to_string(),
                args,
            }));
            definition
                .body
                .insert(0, Node::VariableBlock(WS::default(), call));
        }
    }
}

fn macro_guard(args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
    let marker = 0u8;
    let here = std::hint::black_box(&marker) as *const u8 as usize;
    let used = MACRO_STACK_TOP.with(|top| {
        if here > top.get() {
            top.set(here);
        }
        top.get() - here
    });
    if used > MACRO_STACK_BUDGET {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("a macro");
        return Err(tera::Error::msg(format!(
            "macro `{name}` nests too deeply: a recursive macro that never reaches its base case? \
             Make sure every recursive call works on a smaller value"
        )));
    }
    Ok(tera::Value::String(String::new()))
}

/// Register the filters seite's partials use: `escape_attr` (HTML attribute
/// values) and `json_script` (JSON inside `<script>`).
pub fn register_filters(tera: &mut tera::Tera) {
    tera.register_filter(
        "escape_attr",
        |value: &tera::Value, _: &std::collections::HashMap<String, tera::Value>| {
            Ok(tera::Value::String(match value {
                tera::Value::String(s) => escape_attr(s),
                tera::Value::Null => String::new(),
                other => escape_attr(&other.to_string()),
            }))
        },
    );
    tera.register_filter(
        "json_script",
        |value: &tera::Value, _: &std::collections::HashMap<String, tera::Value>| {
            Ok(tera::Value::String(json_script(value)))
        },
    );
}

/// The embedded seite partial with this name (`seite/head.html`, ...), if any.
pub fn seite_partial(name: &str) -> Option<&'static str> {
    SEITE_PARTIALS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, content)| *content)
}

/// Names of every embedded default template (see [`embedded_default`]).
pub const EMBEDDED_TEMPLATE_NAMES: &[&str] = &[
    "seite/head.html",
    "seite/search.html",
    "seite/search-script.html",
    "seite/lang-switcher.html",
    "seite/scripts.html",
    "seite/comments.html",
    "seite/newsletter.html",
    "base.html",
    "index.html",
    "post.html",
    "doc.html",
    "docs-index.html",
    "page.html",
    "trust-item.html",
    "trust-index.html",
    "404.html",
    "tags.html",
    "tag.html",
    "taxonomy.html",
    "term.html",
    "series.html",
    "changelog-entry.html",
    "changelog-index.html",
    "roadmap-item.html",
    "roadmap-index.html",
    "roadmap-kanban.html",
    "roadmap-timeline.html",
];

/// The embedded default template with this name, if any. The build uses it
/// whenever the site's template directory does not provide the template.
pub fn embedded_default(name: &str) -> Option<&'static str> {
    get_default_template(name)
}

fn get_default_template(name: &str) -> Option<&'static str> {
    match name {
        "base.html" => Some(default_base()),
        "index.html" => Some(DEFAULT_INDEX),
        "post.html" => Some(DEFAULT_POST),
        "doc.html" => Some(DEFAULT_DOC),
        "docs-index.html" => Some(DEFAULT_DOCS_INDEX),
        "page.html" => Some(DEFAULT_PAGE),
        "trust-item.html" => Some(DEFAULT_TRUST_ITEM),
        "trust-index.html" => Some(DEFAULT_TRUST_INDEX),
        "404.html" => Some(DEFAULT_404),
        "tags.html" => Some(DEFAULT_TAGS_INDEX),
        "tag.html" => Some(DEFAULT_TAG),
        "taxonomy.html" => Some(DEFAULT_TAXONOMY),
        "term.html" => Some(DEFAULT_TERM),
        "series.html" => Some(DEFAULT_SERIES),
        "changelog-entry.html" => Some(DEFAULT_CHANGELOG_ENTRY),
        "changelog-index.html" => Some(DEFAULT_CHANGELOG_INDEX),
        "roadmap-item.html" => Some(DEFAULT_ROADMAP_ITEM),
        "roadmap-index.html" => Some(DEFAULT_ROADMAP_INDEX),
        "roadmap-kanban.html" => Some(DEFAULT_ROADMAP_KANBAN),
        "roadmap-timeline.html" => Some(DEFAULT_ROADMAP_TIMELINE),
        _ => seite_partial(name),
    }
}

/// Load Tera templates from the user's template directory, falling back to
/// embedded defaults for any template not provided.
pub fn load_templates(template_dir: &Path, collections: &[CollectionConfig]) -> Result<tera::Tera> {
    let (tera, warnings) = load_templates_with_warnings(template_dir, collections)?;
    for warning in &warnings {
        eprintln!("⚠ Warning: {warning}");
    }
    Ok(tera)
}

/// Like [`load_templates`], but returns non-fatal problems (e.g. a user
/// template that failed to parse, causing a fallback to the embedded defaults)
/// as warnings instead of printing them. Callers decide how to surface them.
pub fn load_templates_with_warnings(
    template_dir: &Path,
    collections: &[CollectionConfig],
) -> Result<(tera::Tera, Vec<String>)> {
    let mut warnings = Vec::new();
    #[allow(clippy::manual_unwrap_or_default)]
    let mut tera = if template_dir.exists() {
        let glob_pattern = format!("{}/**/*.html", template_dir.display());
        match tera::Tera::new(&glob_pattern) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(format!(
                    "failed to parse user templates in {}, using built-in defaults \
                     (your custom templates were ignored): {}",
                    template_dir.display(),
                    error_chain(&e)
                ));
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
    register_filters(&mut tera);

    // seite-owned partials: the embedded copy unless the site overrides it
    // with templates/seite/<name>.
    for (name, content) in SEITE_PARTIALS {
        if tera.get_template(name).is_err() {
            tera.add_raw_template(name, content)?;
        }
    }

    // Always ensure essential templates exist
    for name in [
        "base.html",
        "index.html",
        "404.html",
        "tags.html",
        "tag.html",
        "taxonomy.html",
        "term.html",
        "series.html",
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

    guard_macro_recursion(&mut tera);
    Ok((tera, warnings))
}

/// Parse every user template under `template_dir` (including
/// `shortcodes/*.html`) individually and report each syntax error as a
/// `template-parse` error diagnostic with the template file and, when Tera's
/// parser provides it, the line and column. Unlike loading the whole
/// directory with Tera (which stops at the first bad file), this finds every
/// broken template in one pass. Returns an empty list when all templates parse.
pub fn template_parse_diagnostics(template_dir: &Path) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    if !template_dir.is_dir() {
        return diagnostics;
    }
    let mut files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(template_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "html"))
        .collect();
    files.sort();
    for path in files {
        if let Some(d) = template_file_diagnostic(template_dir, &path) {
            diagnostics.push(d);
        }
    }
    diagnostics
}

/// Find user templates (every `*.html` under `template_dir`; the embedded
/// defaults are never scanned) that still build the markdown URL as
/// `{{ page.url }}.md` — broken (`/.md`, `/tags/.md`) on pages without a
/// markdown copy — and report each occurrence as a `template-markdown-link`
/// warning with file and line.
pub fn stale_markdown_link_diagnostics(template_dir: &Path) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    if !template_dir.is_dir() {
        return diagnostics;
    }
    let mut files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(template_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "html"))
        .collect();
    files.sort();
    for path in files {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue; // unreadable templates are reported as template-parse
        };
        for (idx, line) in content.lines().enumerate() {
            if let Some(column) = find_page_url_md(line) {
                diagnostics.push(
                    Diagnostic::warning(
                        "template-markdown-link",
                        "`{{ page.url }}.md` links to a markdown copy that does not exist on every page (e.g. /.md, /tags/.md)",
                    )
                    .with_file(&path)
                    .with_line(idx + 1)
                    .with_column(column)
                    .with_hint(
                        "use `{% if page.markdown_url %}...{{ page.markdown_url }}...{% endif %}` (set only when a .md copy is published)",
                    ),
                );
            }
        }
    }
    diagnostics
}

/// The snippet that adopts seite's partials in a custom `base.html`.
pub const SEITE_PARTIALS_SNIPPET: &str = r#"{% include "seite/head.html" %}      inside <head>, right after <title>
{% include "seite/search.html" %}    where the search box goes
{% include "seite/scripts.html" %}   right before </body>"#;

/// Warn (`template-missing-seite-head`) when the site's own `base.html`
/// does not include `seite/head.html`, the partial that carries canonical,
/// description, Open Graph/Twitter, JSON-LD, hreflang, feed and markdown
/// links. A `base.html` that still has its own copy of that head (an older
/// bundled theme or a copy of one) is reported as a legacy theme: it works,
/// but no longer gets SEO/GEO fixes. No `base.html` (the embedded default)
/// is fine.
pub fn seite_head_diagnostics(template_dir: &Path) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    let path = template_dir.join("base.html");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return diagnostics;
    };
    // Follow {% include %} / {% extends %} / {% import %} from base.html
    // (user templates first, then the embedded ones), so a head included
    // indirectly (e.g. via templates/partials/head.html) counts.
    let reached = reachable_templates(template_dir, "base.html", &content);
    if reached.iter().any(|(name, _)| name == "seite/head.html") {
        return diagnostics;
    }
    // Point at <title> (the include goes right after it), else at <head>.
    let find = |needle: &str| content.lines().position(|l| l.contains(needle));
    let line = find("</title>")
        .or_else(|| find("<head"))
        .map_or(1, |i| i + 1);
    let legacy = reached
        .iter()
        .any(|(_, src)| src.contains("og:title") || src.contains("application/ld+json"));
    let diagnostic = if legacy {
        Diagnostic::warning(
            "template-missing-seite-head",
            "legacy theme: base.html has its own copy of the SEO head instead of including `seite/head.html`, so it gets no automatic SEO/GEO updates",
        )
        .with_hint(
            "run `seite upgrade` (replaces an unedited bundled theme) or `seite theme apply <name>` to get automatic SEO updates; to keep your design, replace the SEO tags after <title> with `{% include \"seite/head.html\" %}`",
        )
    } else {
        Diagnostic::warning(
            "template-missing-seite-head",
            "missing SEO head: base.html does not include `seite/head.html`, so pages have no canonical URL, description, Open Graph/Twitter tags or JSON-LD",
        )
        .with_hint("add `{% include \"seite/head.html\" %}` inside <head>, right after <title>")
    };
    diagnostics.push(diagnostic.with_file(&path).with_line(line));
    diagnostics
}

/// Every template reachable from `name` (whose source is `source`) through
/// `include`, `extends` and `import` tags, as `(name, source)`, starting with
/// `name` itself. Targets resolve to the user's file in `template_dir`, else
/// the embedded template of that name; unknown targets are skipped.
fn reachable_templates(template_dir: &Path, name: &str, source: &str) -> Vec<(String, String)> {
    let mut reached = vec![(name.to_string(), source.to_string())];
    let mut next = 0;
    while next < reached.len() {
        let targets = template_references(&reached[next].1);
        next += 1;
        for target in targets {
            if reached.iter().any(|(n, _)| *n == target) {
                continue;
            }
            let user = template_dir.join(&target);
            let source = match std::fs::read_to_string(&user) {
                Ok(text) if user.starts_with(template_dir) => Some(text),
                _ => embedded_default(&target).map(str::to_string),
            };
            if let Some(source) = source {
                reached.push((target, source));
            }
        }
    }
    reached
}

/// Template names in the `include` / `extends` / `import` tags of `source`
/// (every quoted string in the tag, so `include ["a.html", "b.html"]` works).
fn template_references(source: &str) -> Vec<String> {
    template_references_located(source)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// [`template_references`] with the 1-based line of each tag. Tags inside
/// `{# comments #}` and `{% raw %}` blocks are not references.
fn template_references_located(source: &str) -> Vec<(String, usize)> {
    let mut names = Vec::new();
    let mut pos = 0;
    while pos < source.len() {
        let rest = &source[pos..];
        let next_tag = rest.find("{%");
        let next_comment = rest.find("{#");
        let start = match (next_tag, next_comment) {
            (Some(t), Some(c)) if c < t => {
                // Skip the comment (an unclosed one runs to the end).
                match rest[c + 2..].find("#}") {
                    Some(end) => {
                        pos += c + 2 + end + 2;
                        continue;
                    }
                    None => break,
                }
            }
            (Some(t), _) => t,
            (None, _) => break,
        };
        let after = &rest[start + 2..];
        let Some(end) = after.find("%}") else { break };
        let tag = after[..end].trim_start_matches('-').trim_start();
        let tag_end = pos + start + 2 + end + 2;
        if tag.starts_with("raw") {
            // Everything up to `{% endraw %}` is literal text.
            let mut scan = tag_end;
            loop {
                let Some(open) = source[scan..].find("{%") else {
                    return names;
                };
                let inner = &source[scan + open + 2..];
                let Some(close) = inner.find("%}") else {
                    return names;
                };
                scan += open + 2 + close + 2;
                let inner_tag =
                    inner[..close].trim_matches(|c: char| c == '-' || c.is_whitespace());
                if inner_tag == "endraw" {
                    break;
                }
            }
            pos = scan;
            continue;
        }
        if ["include", "extends", "import"]
            .iter()
            .any(|kw| tag.starts_with(kw))
        {
            let line = source[..pos + start].matches('\n').count() + 1;
            for (i, part) in tag.split(['"', '\'']).enumerate() {
                if i % 2 == 1 && !part.is_empty() && !part.contains("..") {
                    names.push((part.to_string(), line));
                }
            }
        }
        pos = tag_end;
    }
    names
}

/// Report every cycle of `include` / `extends` / `import` tags among the
/// templates the build renders: the user's `*.html` under `template_dir`
/// (including `shortcodes/`), and the embedded template of a name the user
/// does not provide (an embedded `post.html` extends the user's
/// `base.html`). Tera recurses into such a cycle until the stack overflows
/// and aborts the process (`seite check`, the dev server, the MCP server),
/// so the build reports a `template-cycle` error instead of rendering. Only
/// literal names are followed (`{% include name_var %}` cannot be resolved
/// statically). Symlinked templates are skipped (`symlinked-source`).
pub fn template_cycle_diagnostics(template_dir: &Path) -> Diagnostics {
    let mut user = std::collections::BTreeMap::new();
    if template_dir.is_dir() {
        for entry in walkdir::WalkDir::new(template_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
        {
            let Ok(rel) = entry.path().strip_prefix(template_dir) else {
                continue;
            };
            let name = rel.to_string_lossy().replace('\\', "/");
            // Unreadable templates are reported as template-parse.
            if let Ok(source) = std::fs::read_to_string(entry.path()) {
                user.insert(name, (entry.path().to_path_buf(), source));
            }
        }
    }
    cycle_diagnostics(&user, embedded_default)
}

/// Cycles among `user` templates (name → file, source), following
/// references to `fallback(name)` sources for names `user` lacks. One
/// `template-cycle` error per cycle, at the tag in a user template that
/// closes it (or, when an embedded template closes it, at the user
/// template's tag that leads into the embedded one).
pub(crate) fn cycle_diagnostics(
    user: &std::collections::BTreeMap<String, (std::path::PathBuf, String)>,
    fallback: impl Fn(&str) -> Option<&'static str>,
) -> Diagnostics {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Active,
        Done,
    }
    struct Walk<'a, F> {
        user: &'a std::collections::BTreeMap<String, (std::path::PathBuf, String)>,
        fallback: F,
        marks: HashMap<String, Mark>,
        // (template, line of the tag that leads to the next one)
        stack: Vec<(String, usize)>,
        found: Diagnostics,
    }
    impl<F: Fn(&str) -> Option<&'static str>> Walk<'_, F> {
        fn source(&self, name: &str) -> Option<String> {
            match self.user.get(name) {
                Some((_, source)) => Some(source.clone()),
                None => (self.fallback)(name).map(str::to_string),
            }
        }
        fn visit(&mut self, name: &str) {
            let Some(source) = self.source(name) else {
                return;
            };
            self.marks.insert(name.to_string(), Mark::Active);
            self.stack.push((name.to_string(), 0));
            for (target, line) in template_references_located(&source) {
                if let Some(top) = self.stack.last_mut() {
                    top.1 = line;
                }
                match self.marks.get(&target) {
                    Some(Mark::Done) => {}
                    Some(Mark::Active) => self.report(&target),
                    None => self.visit(&target),
                }
            }
            self.stack.pop();
            self.marks.insert(name.to_string(), Mark::Done);
        }
        fn report(&mut self, target: &str) {
            let Some(from) = self.stack.iter().position(|(n, _)| n == target) else {
                return;
            };
            let cycle = &self.stack[from..];
            let chain: Vec<&str> = cycle
                .iter()
                .map(|(n, _)| n.as_str())
                .chain([target])
                .collect();
            // The last user template on the cycle: its tag leads (back) on.
            let Some((file, line)) = cycle
                .iter()
                .rev()
                .find_map(|(n, line)| self.user.get(n).map(|(path, _)| (path.clone(), *line)))
            else {
                return;
            };
            self.found.push(
                Diagnostic::error(
                    "template-cycle",
                    format!(
                        "template cycle: {} (rendering it would never end)",
                        chain.join(" → ")
                    ),
                )
                .with_file(file)
                .with_line(line)
                .with_hint(
                    "remove the include, extends or import that leads back to a template already being rendered",
                ),
            );
        }
    }
    let mut walk = Walk {
        user,
        fallback,
        marks: HashMap::new(),
        stack: Vec::new(),
        found: Diagnostics::new(),
    };
    for name in user.keys() {
        if !walk.marks.contains_key(name) {
            walk.visit(name);
        }
    }
    walk.found
}

/// 1-based column of the first `{{ page.url }}.md` in `line` (whitespace
/// and Tera's `-` whitespace control inside the braces are optional), if any.
fn find_page_url_md(line: &str) -> Option<usize> {
    let mut search_from = 0;
    while let Some(rel) = line[search_from..].find("{{") {
        let start = search_from + rel;
        let inner = &line[start + 2..];
        let rest = inner.strip_prefix('-').unwrap_or(inner).trim_start();
        if let Some(rest) = rest.strip_prefix("page.url") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('-').unwrap_or(rest);
            if let Some(rest) = rest.strip_prefix("}}") {
                if rest.starts_with(".md") {
                    return Some(line[..start].chars().count() + 1);
                }
            }
        }
        search_from = start + 2;
    }
    None
}

/// Parse one template file (`path`, inside `template_dir`) and return its
/// `template-parse` error, if any. Used by `seite check --file`.
pub fn template_file_diagnostic(template_dir: &Path, path: &Path) -> Option<Diagnostic> {
    let name = path
        .strip_prefix(template_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return Some(
                Diagnostic::error(
                    "template-parse",
                    format!("cannot read template `{name}`: {e}"),
                )
                .with_file(path),
            );
        }
    };
    tera::Template::new(&name, None, &content)
        .err()
        .map(|e| parse_error_diagnostic(&e, &name, path))
}

/// Build a `template-parse` diagnostic from a Tera parse error, extracting the
/// `--> line:col` position and the `= expected ...` explanation from pest's
/// report.
fn parse_error_diagnostic(err: &tera::Error, name: &str, path: &Path) -> Diagnostic {
    let full = error_chain(err);
    let mut line_col = None;
    let mut explanation = Vec::new();
    for l in full.lines() {
        let t = l.trim();
        if let Some(pos) = t.strip_prefix("--> ") {
            if let Some((a, b)) = pos.split_once(':') {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                    line_col.get_or_insert((a, b));
                }
            }
        } else if let Some(rest) = t.strip_prefix("= ") {
            explanation.push(rest.to_string());
        }
    }
    let detail = if explanation.is_empty() {
        full.lines().next().unwrap_or_default().to_string()
    } else {
        explanation.join("; ")
    };
    let mut d = Diagnostic::error(
        "template-parse",
        format!("syntax error in template `{name}`: {detail}"),
    )
    .with_file(path);
    if let Some((line, col)) = line_col {
        d = d.with_line(line).with_column(col);
    }
    d
}

/// Build a `template-render` diagnostic for a failed render of `template`.
///
/// `source` is the content file being rendered (when there is one); the
/// diagnostic points at it and names the template (with its path under
/// `template_dir` when it is a user template). The message carries Tera's full
/// cause chain, e.g. ``Variable `page.titel` not found in context``, and a
/// did-you-mean hint is added for unknown variables using the keys of `ctx`.
pub fn render_error_diagnostic(
    err: &tera::Error,
    template: &str,
    source: Option<&Path>,
    template_dir: &Path,
    root: &Path,
    ctx: &tera::Context,
) -> Diagnostic {
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).to_path_buf();
    let user_template = template_dir.join(template);
    let is_user_template = user_template.is_file();
    let template_desc = if is_user_template {
        format!("template `{template}` ({})", rel(&user_template).display())
    } else {
        format!("built-in template `{template}`")
    };
    let chain = error_chain(err);
    let message = match source {
        Some(src) => format!(
            "failed to render {} with {template_desc}: {chain}",
            rel(src).display()
        ),
        None => format!("failed to render {template_desc}: {chain}"),
    };
    let mut d = Diagnostic::error("template-render", message);
    d = match source {
        Some(src) => d.with_file(rel(src)),
        None if is_user_template => d.with_file(rel(&user_template)),
        None => d,
    };
    if let Some(hint) = unknown_variable_hint(&chain, ctx) {
        d = d.with_hint(hint);
    }
    d
}

/// For ``Variable `a.b` not found`` errors, suggest the closest existing key:
/// among the fields of `a` when `a` exists (and is an object), otherwise among
/// the context's top-level keys.
fn unknown_variable_hint(chain: &str, ctx: &tera::Context) -> Option<String> {
    let start = chain.find("Variable `")? + "Variable `".len();
    let rest = &chain[start..];
    let var = &rest[..rest.find('`')?];
    let json = ctx.clone().into_json();
    let obj = json.as_object()?;
    let mut parts = var.split('.');
    let top = parts.next()?;
    let Some(mut current) = obj.get(top) else {
        return did_you_mean(top, obj.keys().map(String::as_str));
    };
    // Walk down existing object keys; suggest at the first missing segment.
    let mut path = top.to_string();
    for segment in parts {
        let map = current.as_object()?;
        match map.get(segment) {
            Some(next) => {
                current = next;
                path.push('.');
                path.push_str(segment);
            }
            None => {
                let best = did_you_mean(segment, map.keys().map(String::as_str))?;
                let suggestion = best
                    .trim_start_matches("did you mean `")
                    .trim_end_matches("`?");
                return Some(format!("did you mean `{path}.{suggestion}`?"));
            }
        }
    }
    None
}

/// Render an error and all of its sources (Tera nests the useful detail —
/// file, line, and the parser message — in the source chain).
pub(crate) fn error_chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(s) = source {
        let msg = s.to_string();
        if !out.contains(&msg) {
            out.push_str(": ");
            out.push_str(&msg);
        }
        source = s.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_template_names_all_resolve() {
        for name in EMBEDDED_TEMPLATE_NAMES {
            assert!(embedded_default(name).is_some(), "{name}");
        }
        assert!(embedded_default("nope.html").is_none());
    }

    #[test]
    fn test_default_base_is_non_empty() {
        let base = default_base();
        assert!(!base.is_empty());
        assert!(base.contains("<!DOCTYPE html>") || base.contains("<!doctype html>"));
    }

    #[test]
    fn test_get_default_template_known_names() {
        assert!(get_default_template("base.html").is_some());
        assert!(get_default_template("index.html").is_some());
        assert!(get_default_template("post.html").is_some());
        assert!(get_default_template("doc.html").is_some());
        assert!(get_default_template("docs-index.html").is_some());
        assert!(get_default_template("page.html").is_some());
        assert!(get_default_template("404.html").is_some());
        assert!(get_default_template("tags.html").is_some());
        assert!(get_default_template("tag.html").is_some());
        assert!(get_default_template("changelog-entry.html").is_some());
        assert!(get_default_template("changelog-index.html").is_some());
        assert!(get_default_template("roadmap-item.html").is_some());
        assert!(get_default_template("roadmap-index.html").is_some());
        assert!(get_default_template("roadmap-kanban.html").is_some());
        assert!(get_default_template("roadmap-timeline.html").is_some());
        assert!(get_default_template("trust-item.html").is_some());
        assert!(get_default_template("trust-index.html").is_some());
    }

    #[test]
    fn test_get_default_template_unknown_returns_none() {
        assert!(get_default_template("unknown.html").is_none());
        assert!(get_default_template("").is_none());
    }

    #[test]
    fn test_load_templates_no_dir() {
        let path = std::path::Path::new("/nonexistent/templates");
        let tera = load_templates(path, &[]).unwrap();
        // Essential templates should be loaded from defaults
        assert!(tera.get_template("base.html").is_ok());
        assert!(tera.get_template("index.html").is_ok());
        assert!(tera.get_template("404.html").is_ok());
    }

    #[test]
    fn test_load_templates_with_collections() {
        let path = std::path::Path::new("/nonexistent/templates");
        let collections = vec![
            CollectionConfig::preset_posts(),
            CollectionConfig::preset_docs(),
        ];
        let tera = load_templates(path, &collections).unwrap();
        assert!(tera.get_template("post.html").is_ok());
        assert!(tera.get_template("doc.html").is_ok());
        assert!(tera.get_template("docs-index.html").is_ok());
    }

    #[test]
    fn test_load_templates_with_changelog_collection() {
        let path = std::path::Path::new("/nonexistent/templates");
        let collections = vec![CollectionConfig::preset_changelog()];
        let tera = load_templates(path, &collections).unwrap();
        assert!(tera.get_template("changelog-entry.html").is_ok());
        assert!(tera.get_template("changelog-index.html").is_ok());
    }

    #[test]
    fn test_load_templates_with_roadmap_collection() {
        let path = std::path::Path::new("/nonexistent/templates");
        let collections = vec![CollectionConfig::preset_roadmap()];
        let tera = load_templates(path, &collections).unwrap();
        assert!(tera.get_template("roadmap-item.html").is_ok());
        assert!(tera.get_template("roadmap-index.html").is_ok());
    }

    #[test]
    fn test_load_templates_with_trust_collection() {
        let path = std::path::Path::new("/nonexistent/templates");
        let collections = vec![CollectionConfig::preset_trust()];
        let tera = load_templates(path, &collections).unwrap();
        assert!(tera.get_template("trust-item.html").is_ok());
        assert!(tera.get_template("trust-index.html").is_ok());
    }

    #[test]
    fn test_load_templates_from_real_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl_dir = tmp.path().join("templates");
        std::fs::create_dir_all(&tpl_dir).unwrap();
        // Write a custom base.html
        std::fs::write(
            tpl_dir.join("base.html"),
            "<!DOCTYPE html><html><body>{% block content %}{% endblock %}</body></html>",
        )
        .unwrap();
        let tera = load_templates(&tpl_dir, &[]).unwrap();
        // Our custom template should be loaded
        assert!(tera.get_template("base.html").is_ok());
    }

    #[test]
    fn test_load_templates_with_warnings_reports_parse_fallback() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tpl_dir = tmp.path().join("templates");
        std::fs::create_dir_all(&tpl_dir).unwrap();
        std::fs::write(tpl_dir.join("base.html"), "<html>{% if %}</html>").unwrap();
        let (tera, warnings) = load_templates_with_warnings(&tpl_dir, &[]).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("failed to parse user templates"));
        assert!(warnings[0].contains("base.html"), "{}", warnings[0]);
        // Falls back to the embedded default.
        assert!(tera.get_template("base.html").is_ok());
    }

    #[test]
    fn test_template_parse_diagnostics_reports_each_file_with_location() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("templates");
        std::fs::create_dir_all(dir.join("shortcodes")).unwrap();
        std::fs::write(dir.join("ok.html"), "<p>{{ page.title }}</p>").unwrap();
        std::fs::write(dir.join("post.html"), "<html>\n<body>\n{% if %}\n</body>").unwrap();
        std::fs::write(dir.join("shortcodes/bad.html"), "{{ x ").unwrap();
        let diags = template_parse_diagnostics(&dir).into_vec();
        assert_eq!(diags.len(), 2, "{diags:?}");
        let post = diags
            .iter()
            .find(|d| d.file.as_ref().unwrap().ends_with("post.html"))
            .unwrap();
        assert_eq!(post.code, "template-parse");
        assert_eq!(post.line, Some(3), "{post}");
        assert!(post.column.is_some());
        assert!(post.message.contains("post.html"), "{}", post.message);
        assert!(diags
            .iter()
            .any(|d| d.file.as_ref().unwrap().ends_with("shortcodes/bad.html")));
        assert!(template_parse_diagnostics(&tmp.path().join("missing")).is_empty());
    }

    #[test]
    fn test_render_error_diagnostic_names_source_template_and_hint() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template("post.html", "{{ page.titel }}")
            .unwrap();
        let mut ctx = tera::Context::new();
        ctx.insert("page", &serde_json::json!({"title": "x", "slug": "y"}));
        let err = tera.render("post.html", &ctx).unwrap_err();
        let d = render_error_diagnostic(
            &err,
            "post.html",
            Some(Path::new("/site/content/posts/a.md")),
            Path::new("/nonexistent"),
            Path::new("/site"),
            &ctx,
        );
        assert_eq!(d.code, "template-render");
        assert_eq!(d.file.as_deref(), Some(Path::new("content/posts/a.md")));
        assert!(d.message.contains("post.html"), "{}", d.message);
        assert!(d.message.contains("page.titel"), "{}", d.message);
        assert_eq!(d.hint.as_deref(), Some("did you mean `page.title`?"));
    }

    #[test]
    fn test_find_page_url_md_variants() {
        assert_eq!(find_page_url_md(r#"href="{{ page.url }}.md""#), Some(7));
        assert_eq!(find_page_url_md("{{page.url}}.md"), Some(1));
        assert_eq!(find_page_url_md("x {{-  page.url  -}}.md"), Some(3));
        assert_eq!(
            find_page_url_md("{{ site.base_url }}{{ page.url }}.md"),
            Some(20)
        );
        assert_eq!(find_page_url_md("{{ page.url }}"), None);
        assert_eq!(find_page_url_md("{{ page.url }}.html"), None);
        assert_eq!(find_page_url_md("{{ page.url_md }}.md"), None);
        assert_eq!(find_page_url_md("{{ page.markdown_url }}"), None);
    }

    #[test]
    fn test_stale_markdown_link_diagnostics() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(stale_markdown_link_diagnostics(&tmp.path().join("missing")).is_empty());
        std::fs::create_dir_all(tmp.path().join("partials")).unwrap();
        std::fs::write(
            tmp.path().join("base.html"),
            "<head>\n<link rel=\"alternate\" href=\"{{ page.url }}.md\">\n</head>",
        )
        .unwrap();
        std::fs::write(tmp.path().join("partials/ok.html"), "{{ page.url }}").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), "{{ page.url }}.md").unwrap();
        let found: Vec<_> = stale_markdown_link_diagnostics(tmp.path())
            .into_iter()
            .collect();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "template-markdown-link");
        assert_eq!(found[0].severity, crate::diagnostics::Severity::Warning);
        assert_eq!(found[0].line, Some(2));
        assert!(found[0].file.as_ref().unwrap().ends_with("base.html"));
        assert!(found[0]
            .hint
            .as_ref()
            .unwrap()
            .contains("page.markdown_url"));
    }

    #[test]
    fn test_embedded_defaults_have_no_stale_markdown_links() {
        for theme in crate::themes::all() {
            for line in theme.base_html.lines() {
                assert!(find_page_url_md(line).is_none(), "{}: {line}", theme.name);
            }
        }
    }

    #[test]
    fn test_unknown_variable_hint_top_level() {
        let mut ctx = tera::Context::new();
        ctx.insert("site", &serde_json::json!({}));
        assert_eq!(
            unknown_variable_hint("Variable `sit.title` not found", &ctx).as_deref(),
            Some("did you mean `site`?")
        );
        assert!(unknown_variable_hint("some other error", &ctx).is_none());
    }

    #[test]
    fn test_load_templates_with_warnings_clean_dir_has_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (_, warnings) = load_templates_with_warnings(&tmp.path().join("missing"), &[]).unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_all_default_templates_extend_base() {
        // All page-level templates should extend base.html
        let templates_that_extend_base = [
            "index.html",
            "post.html",
            "doc.html",
            "docs-index.html",
            "page.html",
            "404.html",
            "tags.html",
            "tag.html",
            "taxonomy.html",
            "term.html",
            "series.html",
            "changelog-entry.html",
            "changelog-index.html",
            "roadmap-item.html",
            "roadmap-index.html",
            "roadmap-kanban.html",
            "roadmap-timeline.html",
            "trust-item.html",
            "trust-index.html",
        ];
        for name in templates_that_extend_base {
            let content = get_default_template(name).unwrap();
            assert!(
                content.contains(r#"extends "base.html""#),
                "Template '{}' should extend base.html",
                name
            );
        }
    }

    // --- seite partials ---

    #[test]
    fn test_seite_partials_resolve_and_start_with_marker() {
        for (name, content) in SEITE_PARTIALS {
            assert_eq!(seite_partial(name), Some(*content));
            assert_eq!(embedded_default(name), Some(*content));
            assert!(EMBEDDED_TEMPLATE_NAMES.contains(name), "{name}");
            let short = name
                .strip_prefix("seite/")
                .and_then(|n| n.strip_suffix(".html"))
                .unwrap();
            assert!(
                content.starts_with(&format!(
                    "{{# seite partial: {short} — managed by seite, updated on upgrade"
                )),
                "{name}"
            );
            for line in content.lines() {
                assert!(find_page_url_md(line).is_none(), "{name}: {line}");
            }
        }
        assert!(seite_partial("seite/nope.html").is_none());
    }

    #[test]
    fn test_every_bundled_theme_includes_the_partials() {
        for theme in crate::themes::all() {
            for partial in [
                "seite/head.html",
                "seite/search.html",
                "seite/scripts.html",
                "seite/lang-switcher.html",
            ] {
                let include = format!("{{% include \"{partial}\" %}}");
                assert_eq!(
                    theme.base_html.matches(&include).count(),
                    1,
                    "{}: {partial}",
                    theme.name
                );
            }
            // Themes own layout and CSS only: no copy of the SEO head.
            for tag in [
                "og:title",
                "application/ld+json",
                "rel=\"canonical\"",
                "search-index.json",
            ] {
                assert!(!theme.base_html.contains(tag), "{}: {tag}", theme.name);
            }
            assert!(
                theme.base_html.contains("{% block title %}"),
                "{}",
                theme.name
            );
        }
    }

    #[test]
    fn test_load_templates_registers_partials_and_honors_overrides() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tera, _) = load_templates_with_warnings(&tmp.path().join("missing"), &[]).unwrap();
        for (name, _) in SEITE_PARTIALS {
            assert!(tera.get_template(name).is_ok(), "{name}");
        }

        // A user file under templates/seite/ wins over the embedded partial,
        // and a user base.html can include partials it does not define.
        std::fs::create_dir_all(tmp.path().join("seite")).unwrap();
        std::fs::write(tmp.path().join("seite/head.html"), "<meta name=\"mine\">").unwrap();
        std::fs::write(
            tmp.path().join("base.html"),
            "<head>{% include \"seite/head.html\" %}</head>{% include \"seite/search.html\" %}",
        )
        .unwrap();
        let (tera, warnings) = load_templates_with_warnings(tmp.path(), &[]).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut ctx = tera::Context::new();
        ctx.insert(
            "t",
            &serde_json::json!({"search_label": "Search", "search_placeholder": "Find"}),
        );
        let html = tera.render("base.html", &ctx).unwrap();
        assert!(html.contains("<meta name=\"mine\">"), "{html}");
        assert!(html.contains("id=\"search-input\""), "{html}");
    }

    #[test]
    fn test_seite_head_diagnostics() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No base.html: the embedded default theme is used.
        assert!(seite_head_diagnostics(tmp.path()).is_empty());

        // A bundled (partial-based) theme: fine.
        std::fs::write(
            tmp.path().join("base.html"),
            crate::themes::dark().base_html,
        )
        .unwrap();
        assert!(seite_head_diagnostics(tmp.path()).is_empty());

        // A pre-partials theme: legacy message pointing at upgrade/apply.
        let legacy = include_str!("../../tests/fixtures/themes-pre-partials/dark.tera");
        std::fs::write(tmp.path().join("base.html"), legacy).unwrap();
        let found: Vec<_> = seite_head_diagnostics(tmp.path()).into_iter().collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "template-missing-seite-head");
        assert_eq!(found[0].severity, crate::diagnostics::Severity::Warning);
        assert!(
            found[0].message.starts_with("legacy theme"),
            "{}",
            found[0].message
        );
        let hint = found[0].hint.clone().unwrap();
        assert!(
            hint.contains("seite upgrade") && hint.contains("seite theme apply"),
            "{hint}"
        );
        assert_eq!(found[0].line, Some(7));

        // A head without any SEO tags: missing message.
        std::fs::write(
            tmp.path().join("base.html"),
            "<html>\n<head>\n<title>x</title>\n</head>\n<body></body></html>",
        )
        .unwrap();
        let found: Vec<_> = seite_head_diagnostics(tmp.path()).into_iter().collect();
        assert_eq!(found.len(), 1);
        assert!(
            found[0].message.starts_with("missing SEO head"),
            "{}",
            found[0].message
        );
        assert_eq!(found[0].line, Some(3));
    }

    #[test]
    fn test_seite_head_diagnostics_follows_indirect_includes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("partials")).unwrap();
        // base.html -> partials/head.html -> seite/head.html: fine.
        std::fs::write(
            dir.join("base.html"),
            "<html><head><title>x</title>{%- include 'partials/head.html' -%}</head></html>",
        )
        .unwrap();
        std::fs::write(
            dir.join("partials/head.html"),
            "{% include \"seite/head.html\" %}<meta name=\"x\">",
        )
        .unwrap();
        assert!(seite_head_diagnostics(dir).is_empty());

        // base.html extends a layout that includes the partial: fine.
        std::fs::write(dir.join("base.html"), "{% extends \"layout.html\" %}").unwrap();
        std::fs::write(
            dir.join("layout.html"),
            "<head>{% include [\"partials/head.html\", \"x.html\"] %}</head>",
        )
        .unwrap();
        assert!(seite_head_diagnostics(dir).is_empty());

        // A partial with a copied head (no seite/head.html): legacy, even
        // though base.html itself has no SEO tags.
        std::fs::write(
            dir.join("partials/head.html"),
            "<meta property=\"og:title\" content=\"{{ page.title }}\">",
        )
        .unwrap();
        let found: Vec<_> = seite_head_diagnostics(dir).into_iter().collect();
        assert_eq!(found.len(), 1);
        assert!(
            found[0].message.starts_with("legacy theme"),
            "{}",
            found[0].message
        );

        // Cycles and missing targets end the walk.
        std::fs::write(
            dir.join("partials/head.html"),
            "{% include \"partials/head.html\" %}{% include \"missing.html\" %}",
        )
        .unwrap();
        let found: Vec<_> = seite_head_diagnostics(dir).into_iter().collect();
        assert!(
            found[0].message.starts_with("missing SEO head"),
            "{}",
            found[0].message
        );
        assert_eq!(
            template_references("{% include \"a.html\" %}{%- extends 'b.html' -%}{% import \"../c.html\" as m %}{{ x }}{% if y %}"),
            ["a.html", "b.html"]
        );
    }

    #[test]
    fn test_escape_attr_and_json_script() {
        assert_eq!(escape_attr("plain /url?a=1"), "plain /url?a=1");
        assert_eq!(
            escape_attr(r#"Say "hi" <b> & go"#),
            "Say &quot;hi&quot; &lt;b&gt; &amp; go"
        );
        // Existing character references are left alone.
        assert_eq!(
            escape_attr("Q&amp;A &#39; &#x2F; &copy;"),
            "Q&amp;A &#39; &#x2F; &copy;"
        );
        assert_eq!(escape_attr("a & b;c"), "a &amp; b;c");
        assert_eq!(
            json_script(&serde_json::json!("x</script><!-- y")),
            "\"x\\u003c/script>\\u003c!-- y\""
        );
        assert_eq!(
            json_script(&serde_json::json!("No results")),
            "\"No results\""
        );
        let parsed: String =
            serde_json::from_str(&json_script(&serde_json::json!("a</b"))).unwrap();
        assert_eq!(parsed, "a</b");
    }

    #[test]
    fn test_head_partial_escapes_attributes_and_json_ld() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tera, _) = load_templates_with_warnings(&tmp.path().join("missing"), &[]).unwrap();
        let mut ctx = tera::Context::new();
        ctx.insert(
            "site",
            &serde_json::json!({"title": "Site", "description": "d", "base_url": "https://x.dev", "author": "A"}),
        );
        ctx.insert(
            "page",
            &serde_json::json!({
                "title": "A \"quoted\" title",
                "description": "Ends </script><script>alert(1)</script>",
                "url": "/posts/a/",
                "collection": "posts",
                "date": "2026-01-01",
            }),
        );
        ctx.insert("lang", "en");
        ctx.insert("lang_prefix", "");
        ctx.insert("translations", &Vec::<serde_json::Value>::new());
        ctx.insert("math_enabled", &false);
        let head = tera.render("seite/head.html", &ctx).unwrap();
        assert!(
            head.contains(r#"<meta property="og:title" content="A &quot;quoted&quot; title">"#),
            "{head}"
        );
        assert!(
            head.contains(r#"content="Ends &lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;""#),
            "{head}"
        );
        // Exactly the two JSON-LD scripts open and close: no breakout.
        assert_eq!(head.matches("<script").count(), 2, "{head}");
        assert_eq!(head.matches("</script>").count(), 2, "{head}");
        assert!(
            head.contains(
                "\"description\":\"Ends \\u003c/script>\\u003cscript>alert(1)\\u003c/script>\""
            ),
            "{head}"
        );
        // The JSON-LD stays valid JSON with the original text.
        let start = head.find("{\"@context\"").unwrap();
        let end = start + head[start..].find('\n').unwrap();
        let ld: serde_json::Value = serde_json::from_str(head[start..end].trim()).unwrap();
        assert_eq!(ld["headline"], "A \"quoted\" title");
        assert_eq!(ld["description"], "Ends </script><script>alert(1)</script>");
    }

    /// Tera with one template `t.html` importing `m.html`, optionally guarded.
    fn macro_tera(macros: &str, call: &str, guard: bool) -> tera::Tera {
        let mut tera = tera::Tera::default();
        tera.add_raw_templates(vec![
            ("m.html", macros.to_string()),
            ("t.html", format!("{{% import \"m.html\" as m %}}{call}")),
        ])
        .unwrap();
        if guard {
            guard_macro_recursion(&mut tera);
        }
        tera
    }

    /// Run `f` on a thread with Windows' 1 MiB main-thread stack, the
    /// smallest seite renders on.
    fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap()
    }

    #[test]
    fn test_runaway_macro_recursion_is_an_error() {
        let err = on_small_stack(|| {
            let tera = macro_tera(
                "{% macro f(n) %}{{ self::f(n=n+1) }}{% endmacro f %}",
                "{{ m::f(n=0) }}",
                true,
            );
            error_chain(&tera.render("t.html", &tera::Context::new()).unwrap_err())
        });
        assert!(err.contains("macro `m.html::f` nests too deeply"), "{err}");
        // Mutual recursion through two macros is caught the same way.
        let err = on_small_stack(|| {
            let tera = macro_tera(
                "{% macro a() %}{{ self::b() }}{% endmacro a %}{% macro b() %}{{ self::a() }}{% endmacro b %}",
                "{{ m::a() }}",
                true,
            );
            error_chain(&tera.render("t.html", &tera::Context::new()).unwrap_err())
        });
        assert!(err.contains("nests too deeply"), "{err}");
    }

    #[test]
    fn test_terminating_macro_recursion_renders_unchanged() {
        let macros =
            "{% macro t(n) %}{% if n > 0 %}<i>{{ self::t(n=n-1) }}</i>{% endif %}{% endmacro t %}";
        let (guarded, plain) = on_small_stack(move || {
            let render = |guard| {
                macro_tera(macros, " a {{ m::t(n=8) }} b ", guard)
                    .render("t.html", &tera::Context::new())
                    .unwrap()
            };
            (render(true), render(false))
        });
        assert_eq!(guarded, plain);
        assert_eq!(guarded.matches("<i>").count(), 8);
    }

    #[test]
    fn test_guard_macro_recursion_is_idempotent() {
        let mut tera = macro_tera("{% macro f() %}x{% endmacro f %}", "{{ m::f() }}", true);
        guard_macro_recursion(&mut tera);
        let body = &tera.templates["m.html"].macros["f"].body;
        assert_eq!(body.len(), 2, "{body:?}");
        assert_eq!(tera.render("t.html", &tera::Context::new()).unwrap(), "x");
    }

    #[test]
    fn test_load_templates_guards_user_macros() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("m.html"),
            "{% macro f(n) %}{{ self::f(n=n+1) }}{% endmacro f %}",
        )
        .unwrap();
        let (tera, _) = load_templates_with_warnings(tmp.path(), &[]).unwrap();
        let body = &tera.templates["m.html"].macros["f"].body;
        assert!(
            matches!(&body[0], tera::ast::Node::VariableBlock(_, e)
                if matches!(&e.val, tera::ast::ExprVal::FunctionCall(c) if c.name == MACRO_GUARD_FN)),
            "{body:?}"
        );
    }
}
