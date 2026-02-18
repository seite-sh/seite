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
    {% if pagination.prev_url %}<a href="{{ pagination.prev_url }}">&larr; Newer</a>{% endif %}
    <span>Page {{ pagination.current_page }} of {{ pagination.total_pages }}</span>
    {% if pagination.next_url %}<a href="{{ pagination.next_url }}">Older &rarr;</a>{% endif %}
</nav>
{% endif %}
{% if not page and collections | length == 0 %}
<p>No content yet. Create some with <code>page new post "My First Post"</code></p>
{% endif %}
{% endblock %}"#;

pub const DEFAULT_POST: &str = r#"{% extends "base.html" %}
{% block title %}{{ page.title }} - {{ site.title }}{% endblock %}
{% block content %}
<article>
    <h1>{{ page.title }}</h1>
    {% if page.date %}<time>{{ page.date }}</time>{% endif %}
    {% if page.reading_time %}<span class="reading-time">{{ page.reading_time }} min read</span>{% endif %}
    {% if page.tags | length > 0 %}
    <div class="tags">
        {% for tag in page.tags %}<a href="{% if lang and lang != site.language %}/{{ lang }}{% endif %}/tags/{{ tag | slugify }}/">{{ tag }}</a> {% endfor %}
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
        <h4>Contents</h4>
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
{% block title %}Page Not Found — {{ site.title }}{% endblock %}
{% block content %}
<article>
    <h1>404 — Page Not Found</h1>
    <p>The page you requested could not be found. <a href="/">Go to the homepage</a>.</p>
</article>
{% endblock %}"#;

pub const DEFAULT_TAGS_INDEX: &str = r#"{% extends "base.html" %}
{% block title %}Tags — {{ site.title }}{% endblock %}
{% block content %}
<h1>Tags</h1>
<div class="tags-index">
    {% for tag in tags %}
    <a href="{{ tag.url }}" class="tag-link">{{ tag.name }} <span class="tag-count">({{ tag.count }})</span></a>
    {% endfor %}
</div>
{% endblock %}"#;

pub const DEFAULT_TAG: &str = r##"{% extends "base.html" %}
{% block title %}Tag: {{ tag_name }} — {{ site.title }}{% endblock %}
{% block content %}
<h1>Tagged "{{ tag_name }}"</h1>
<div class="tag-items">
    {% for item in items %}
    <article>
        <h3><a href="{{ item.url }}">{{ item.title }}</a></h3>
        {% if item.date %}<time>{{ item.date }}</time>{% endif %}
        {% if item.reading_time %}<span class="reading-time">{{ item.reading_time }} min read</span>{% endif %}
        {% if item.description %}<p>{{ item.description }}</p>{% elif item.excerpt %}<div class="excerpt">{{ item.excerpt | safe }}</div>{% endif %}
    </article>
    {% endfor %}
</div>
<p><a href="{{ tags_url }}">&larr; All tags</a></p>
{% endblock %}"##;

fn get_default_template(name: &str) -> Option<&'static str> {
    match name {
        "base.html" => Some(default_base()),
        "index.html" => Some(DEFAULT_INDEX),
        "post.html" => Some(DEFAULT_POST),
        "doc.html" => Some(DEFAULT_DOC),
        "page.html" => Some(DEFAULT_PAGE),
        "404.html" => Some(DEFAULT_404),
        "tags.html" => Some(DEFAULT_TAGS_INDEX),
        "tag.html" => Some(DEFAULT_TAG),
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
            Err(_) => tera::Tera::default(),
        }
    } else {
        tera::Tera::default()
    };

    // Always ensure essential templates exist
    for name in ["base.html", "index.html", "404.html", "tags.html", "tag.html"] {
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
    }

    Ok(tera)
}
