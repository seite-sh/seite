# /seite landing [page]

The homepage or a standalone landing page. It needs a `pages` collection (`seite collection add pages`).

| Page | Content file | Template |
|---|---|---|
| Homepage | `content/pages/index.md` | `templates/index.html` |
| Other page | `content/pages/<slug>.md` | `templates/<slug>-page.html`, with `template: <slug>-page.html` in the frontmatter |

1. Read the existing files, if any, and improve them.
2. Ask for what you cannot infer (audience, offer, main call to action), once, in one message. Write real copy, never placeholders.
3. Content file: set `title`, `description` (160 characters at most), and `image` (e.g. `/static/og.png`). Put section copy in `extra:` keys (`hero_headline`, `cta_primary_text`, `cta_primary_url`, `feature_1_title`, …). Leave the body empty unless there is prose.
4. Template: `{% extends "base.html" %}`, CSS in `{% block extra_css %}`, markup in `{% block content %}` reading `page.extra.*`. Use one `<h1>`, make it work without JavaScript, and respect `prefers-reduced-motion`. On the homepage, keep the `{% for collection in collections %}` listing unless the user says otherwise. Do not change `base.html` for a landing page.
5. Preview as in [preview.md](preview.md), then verify as in [check.md](check.md).
