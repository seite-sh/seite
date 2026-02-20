---
title: Shortcodes
description: Reusable content components for embedding videos, callouts, figures, and custom elements in markdown
weight: 5
---

Shortcodes are reusable content components you can use inside markdown files. They let you embed rich content like videos, callout boxes, and figures without writing raw HTML.

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
Content inside body shortcodes is processed as regular markdown — bold, links, lists, and code blocks all work as expected.
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

Renders an admonition/callout box. This is a **body shortcode** — the content between the tags is processed as markdown.

```
{{% callout(type="warning") %}}
Be careful with this operation. It **cannot be undone**.
{{% end %}}
```

| Parameter | Required | Description |
|-----------|----------|-------------|
| `type` | no | Box style: `info` (default), `warning`, `danger`, `tip` |

All six bundled themes include styled callout boxes with appropriate colors for each type.

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
- `{{ body }}` — the raw body content (for body shortcodes)
- `{{ page }}` — current page context (title, slug, tags, etc.)
- `{{ site }}` — site context (title, base_url, language)

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

- [Templates & Themes](/docs/templates) — template variables available inside shortcode templates
- [Theme Gallery](/docs/theme-gallery) — all bundled themes include styling for built-in shortcodes
