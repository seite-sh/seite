---
title: "Shortcodes for Rich Content"
description: "Embed YouTube videos, callouts, figures, GitHub Gists, contact forms, comments and newsletter forms in your seite site with built-in and custom shortcodes."
weight: 7
---

Shortcodes are reusable content components you can use inside markdown files to embed rich content in your static site: videos, callout boxes, figures, and more, without writing raw HTML.

## Syntax

There are two types of shortcodes:

### Inline shortcodes

Output raw HTML. Use `{{<` and `>}}` delimiters:

```
{{< youtube(id="dQw4w9WgXcQ") >}}
```

### Body shortcodes

Wrap markdown content that gets processed normally. Use `{{% ` and ` %}}` delimiters with a `{{% end %}}` closing tag:

```
{{% callout(type="warning") %}}
This is **bold** markdown inside the callout.
{{% end %}}
```

{{% callout(type="tip") %}}
Content inside body shortcodes is processed as regular markdown: bold, links, lists, and code blocks all work as expected.
{{% end %}}

### Arguments

All arguments are named (key=value). Supported value types:

- **Strings**: `id="dQw4w9WgXcQ"` (quoted)
- **Integers**: `width=800`
- **Floats**: `ratio=1.5`
- **Booleans**: `autoplay=true`

## Built-in shortcodes

### youtube

Embeds a responsive YouTube video player.

```
{{< youtube(id="dQw4w9WgXcQ") >}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `id` | yes | YouTube video ID |
| `start` | no | Start time in seconds |
| `title` | no | Accessible title (default: "YouTube video") |

### vimeo

Embeds a responsive Vimeo video player.

```
{{< vimeo(id="123456") >}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `id` | yes | Vimeo video ID |
| `title` | no | Accessible title (default: "Vimeo video") |

### gist

Embeds a GitHub Gist.

```
{{< gist(user="octocat", id="abc123") >}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `user` | yes | GitHub username |
| `id` | yes | Gist ID |

### figure

Renders a semantic `<figure>` element with optional caption.

```
{{< figure(src="/static/photo.jpg", caption="Sunset over the bay", alt="Orange sunset") >}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `src` | yes | Image path or URL |
| `alt` | no | Alt text for accessibility |
| `caption` | no | Caption displayed below the image |
| `width` | no | Image width attribute |
| `height` | no | Image height attribute |
| `class` | no | CSS class on the figure element |

### callout

Renders an admonition/callout box. This is a **body shortcode**. The content between the tags is processed as markdown.

```
{{% callout(type="warning") %}}
Be careful with this operation. It **cannot be undone**.
{{% end %}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `type` | no | Box style: `info` (default), `note`, `warning`, `danger`, `tip` (`note` is styled in grey with the default "Info" title; pass `title="…"` for your own heading) |

Every bundled theme except `blank` includes styled callout boxes with appropriate colors for each type.

### contact_form

A contact form for the `[contact]` provider. See [Contact Forms](/docs/contact-forms).

### comments

Embeds a free, GitHub-backed comment thread for the page: [giscus](https://giscus.app) (GitHub Discussions) or [utterances](https://utteranc.es) (GitHub Issues). Configure it once in `seite.toml`:

```toml
[comments]
provider = "giscus"              # or "utterances"
repo = "owner/repo"              # public repo with the giscus/utterances app installed
repo_id = "R_kgDOAbCdEf"              # giscus: copy the ids from https://giscus.app
category = "Announcements"       # giscus: discussion category
category_id = "DIC_kwDOAbCdEf4Cg1Hj"        # giscus
# mapping = "pathname"           # giscus: pathname (default) | url | title | og:title
# reactions = true               # giscus: reactions on the page's discussion
# theme = "preferred_color_scheme"  # giscus theme name or https:// stylesheet; utterances: github-light, github-dark, preferred-color-scheme (default), …
# lang = "en"                    # giscus UI language (default: the page's language)
# loading = "lazy"               # giscus: lazy (default) | eager
# issue_term = "pathname"        # utterances: pathname (default) | url | title | og:title
# label = "comments"             # utterances: label for created issues
```

```
{{< comments() >}}
```

It renders the provider's official `<script>` embed (`https://giscus.app/client.js` or `https://utteranc.es/client.js`). Every value is validated when `seite.toml` loads (a bad value is a [`config-invalid`](/docs/errors#config-invalid) error at its line) and attribute-escaped in the HTML. Without `[comments]` the shortcode renders nothing and the build warns ([`comments-not-configured`](/docs/errors#comments-not-configured)).

To show comments on every post without the shortcode, include the partial in your theme's `post.html` (it also renders nothing without `[comments]`):

```html
{% include "seite/comments.html" %}
```

Templates can read the settings as `site.comments` (`provider`, `repo`, …, with defaults filled in).

### newsletter

A subscribe form for a newsletter with a free tier: [Buttondown](https://buttondown.com) or [Kit](https://kit.com) (formerly ConvertKit).

```toml
[newsletter]
provider = "buttondown"   # or "kit"
username = "my-newsletter" # buttondown: your newsletter username
# form_id = "1234567"     # kit: the number in the form's embed code
# action = "https://app.kit.com/forms/1234567/subscriptions"  # kit: optional, the form's action URL from Kit's embed code
```

```
{{< newsletter() >}}
{{< newsletter(title="Get new posts by email", description="One email a month.", button="Subscribe") >}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `title` | no | Heading above the form |
| `description` | no | Text above the field |
| `button` | no | Button text (default: `t.newsletter_submit`, "Subscribe") |
| `id` | no | `id` of the email input (default `newsletter-email`; set it when a page has two forms) |

The form is plain HTML that posts to the provider (`https://buttondown.com/api/emails/embed-subscribe/<username>` with field `email`, or `https://app.kit.com/forms/<form_id>/subscriptions` with field `email_address` — the Kit URL is unverified (Kit publishes no spec): copy your form's action URL from Kit's HTML embed code into `action` if it differs); the provider shows its own confirmation page (and, for Buttondown, a CAPTCHA when it suspects a bot). No third-party JavaScript. The email field is labelled, `type="email"`, `required` and `autocomplete="email"`. The form uses the `contact-form` classes, so every bundled theme styles it like the contact form, plus `newsletter-form`, `newsletter-form-field` and `newsletter-form-submit` for your own CSS. Labels come from the `newsletter_email` and `newsletter_submit` [UI strings](/docs/i18n#ui-string-translations).

Without `[newsletter]` the shortcode renders nothing and the build warns ([`newsletter-not-configured`](/docs/errors#newsletter-not-configured)). Themes can place the form anywhere with `{% include "seite/newsletter.html" %}` (settings in `site.newsletter`).

## Custom shortcodes

Create Tera templates in the `templates/shortcodes/` directory. Each `.html` file becomes a shortcode named after the file.

### Example: alert shortcode

Create `templates/shortcodes/alert.html`:

```html
<div class="alert alert-{{ level | default(value='info') }}">
{{ body }}
</div>
```

Use in markdown:

```
{{% alert(level="error") %}}
Something went wrong. Please try again.
{{% end %}}
```

### Template variables

Shortcode templates have access to:

- All named arguments as top-level variables
- `{{ body }}`: the raw body content (for body shortcodes)
- `{{ page }}`: current page context (title, slug, tags, etc.)
- `{{ site }}`: site context (title, base_url, language)

### Example: button shortcode with conditional styling

Create `templates/shortcodes/button.html`:

```html
<a href="{{ url }}"
   class="btn{% if style %} btn-{{ style }}{% endif %}"
   {% if external %}target="_blank" rel="noopener"{% endif %}>
  {{ label | default(value="Click here") }}
</a>
```

Use in markdown:

```
{{< button(url="/docs/getting-started", label="Get Started", style="primary") >}}
{{< button(url="https://github.com/user/repo", label="GitHub", external=true) >}}
```

### Overriding built-in shortcodes

To customize a built-in shortcode, create a file with the same name in `templates/shortcodes/`. For example, `templates/shortcodes/youtube.html` will override the built-in YouTube embed.

## Escaping shortcodes

To show shortcode syntax literally (e.g., in documentation), put it inside a fenced code block:

````
```
{{< youtube(id="example") >}}
```
````

Shortcodes inside fenced code blocks and inline code spans are never expanded.

## Error handling

- **Unknown shortcode**: Build fails with the shortcode name and available alternatives
- **Unclosed body shortcode**: Build fails with the file path and line number
- **Missing arguments**: Build fails when the template references an undefined variable
- **Invalid syntax**: Build fails with a descriptive error and line number

All error messages include the source file path and line number for quick debugging.

## Next Steps

- [Templates & Themes](/docs/templates): template variables available inside shortcode templates
- [Theme Gallery](/docs/theme-gallery): all bundled themes include styling for built-in shortcodes
