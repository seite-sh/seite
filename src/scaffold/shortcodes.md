## Shortcodes

Shortcodes are reusable content components you can use inside markdown files.

### Inline shortcodes (raw HTML output)

```markdown
{{< youtube(id="dQw4w9WgXcQ") >}}
{{< vimeo(id="123456") >}}
{{< gist(user="octocat", id="abc123") >}}
{{< figure(src="/static/photo.jpg", caption="A great photo", alt="Description") >}}
```

### Body shortcodes (markdown-processed body)

```markdown
{{% callout(type="warning") %}}
This is **bold** markdown inside a callout box.
{{% end %}}
```

Callout types: `info`, `warning`, `danger`, `tip`

### Built-in shortcodes

| Shortcode | Type | Parameters |
|-----------|------|------------|
| `youtube` | inline | `id` (required), `start`, `title` |
| `vimeo` | inline | `id` (required), `title` |
| `gist` | inline | `user` (required), `id` (required) |
| `figure` | inline | `src` (required), `alt`, `caption`, `width`, `height`, `class` |
| `callout` | body | `type` (default: `info`) |

### Custom shortcodes

Create Tera templates in `templates/shortcodes/`. Example `templates/shortcodes/alert.html`:

```html
<div class="alert alert-{{ level }}">{{ body }}</div>
```

Use in markdown: `{{% alert(level="error") %}}Something went wrong{{% end %}}`

Shortcode templates have access to `{{ page }}` and `{{ site }}` context variables.

