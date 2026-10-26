---
title: "Theme Gallery"
description: "seite's bundled themes: four starters (default, docs, landing, blank) and a gallery of seven example looks, all embedded in the binary. Or design your own with impeccable or your coding agent."
weight: 10
---

seite ships 11 themes inside the binary: four **starters** that `seite init` offers, and a **gallery** of seven example looks. They are starting points. For a site that looks like yours, start from `blank` and design it with your coding agent or impeccable: see [Design your own theme](/docs/custom-themes).

Every theme owns only its layout and CSS. The SEO and GEO head, search, language switcher and end-of-body scripts come from seite's [partials](/docs/templates#seite-partials), so they stay correct in every theme and improve when you update seite.

## Starters

`seite init` picks `docs` for a docs-only site and `default` otherwise; choose another with `seite init --theme <starter>`.

<div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 24px; margin: 32px 0;">

<div style="border: 1px solid #e2e2e2; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #ffffff; font-family: system-ui, -apple-system, sans-serif; min-height: 140px;">
<div style="font-size: 11px; color: #0057b7; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">DEFAULT</div>
<div style="font-size: 18px; font-weight: 700; color: #111; margin-bottom: 6px;">Clean Readable Baseline</div>
<div style="font-size: 13px; color: #555; line-height: 1.5; margin-bottom: 12px;">720px centered column, system-ui font stack, blue links. The sensible starting point for any site.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #ffffff; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #111111; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #0057b7; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f8f8f8; border-top: 1px solid #e2e2e2; font-size: 13px; font-family: monospace; color: #555;">seite theme apply default</div>
</div>

<div style="border: 1px solid #d0d7de; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #ffffff; font-family: system-ui, -apple-system, sans-serif; min-height: 140px;">
<div style="font-size: 11px; color: #0969da; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">DOCS</div>
<div style="font-size: 18px; font-weight: 700; color: #1f2328; margin-bottom: 6px;">Sidebar Documentation</div>
<div style="font-size: 13px; color: #656d76; line-height: 1.5; margin-bottom: 12px;">Fixed 260px sidebar with auto-scrolling nav. GitHub-style colors. Built for technical docs.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #ffffff; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #1f2328; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #0969da; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f6f8fa; border-top: 1px solid #d0d7de; font-size: 13px; font-family: monospace; color: #656d76;">seite theme apply docs</div>
</div>

<div style="border: 1px solid #e2e2e2; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #faf8f5; font-family: Georgia, serif; min-height: 140px;">
<div style="font-size: 11px; color: #4f46e5; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">LANDING</div>
<div style="font-size: 18px; font-weight: 700; color: #1b1b1f; margin-bottom: 6px;">Hero + Calls to Action</div>
<div style="font-size: 13px; color: #636370; line-height: 1.5; margin-bottom: 12px;">Marketing layout with hero sections, feature grids and CTAs. Fraunces display type on warm paper, indigo accent.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #faf8f5; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #1b1b1f; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #4f46e5; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f8f8f8; border-top: 1px solid #e2e2e2; font-size: 13px; font-family: monospace; color: #636370;">seite theme apply landing</div>
</div>

<div style="border: 1px solid #e2e2e2; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #ffffff; font-family: system-ui, -apple-system, sans-serif; min-height: 140px;">
<div style="font-size: 11px; color: #0b57d0; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">BLANK</div>
<div style="font-size: 18px; font-weight: 700; color: #1f2328; margin-bottom: 6px;">Design Your Own</div>
<div style="font-size: 13px; color: #59636e; line-height: 1.5; margin-bottom: 12px;">Bare semantic layout, neutral type, CSS in static/css/site.css. The canvas for impeccable or your agent.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #ffffff; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #1f2328; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #0b57d0; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f8f8f8; border-top: 1px solid #e2e2e2; font-size: 13px; font-family: monospace; color: #59636e;">seite theme new</div>
</div>

</div>

## Gallery

Example looks to try or learn from. Apply one with:

```bash
seite theme apply <name>
```

<div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 24px; margin: 32px 0;">

<div style="border: 1px solid #e2e2e2; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #faf9f6; font-family: Georgia, 'Times New Roman', serif; min-height: 140px;">
<div style="font-size: 11px; color: #888; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px; font-family: system-ui, sans-serif;">MINIMAL</div>
<div style="font-size: 18px; font-weight: 700; color: #1a1a1a; margin-bottom: 6px;">Typography First</div>
<div style="font-size: 13px; color: #555; line-height: 1.5; margin-bottom: 12px;">600px column, Georgia serif, understated search. Literary feel that lets content breathe.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #faf9f6; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #1a1a1a; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #888888; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f0eeeb; border-top: 1px solid #e2e2e2; font-size: 13px; font-family: monospace; color: #555;">seite theme apply minimal</div>
</div>

<div style="border: 1px solid #333; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #0a0a0a; min-height: 140px;">
<div style="font-size: 11px; color: #8b5cf6; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px; font-family: system-ui, sans-serif;">DARK</div>
<div style="font-size: 18px; font-weight: 700; color: #e8e8e8; margin-bottom: 6px; font-family: system-ui, sans-serif;">True Black + Violet</div>
<div style="font-size: 13px; color: #999; line-height: 1.5; margin-bottom: 12px; font-family: system-ui, sans-serif;">True black background, violet accent, styled focus rings. Easy on the eyes, day or night.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #0a0a0a; border: 1px solid #333; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #e8e8e8; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #8b5cf6; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #111; border-top: 1px solid #333; font-size: 13px; font-family: monospace; color: #999;">seite theme apply dark</div>
</div>

<div style="border: 3px solid #000000; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #fffef0; font-family: system-ui, -apple-system, sans-serif; min-height: 140px;">
<div style="font-size: 11px; color: #000; font-weight: 900; letter-spacing: 0.5px; margin-bottom: 8px;">BRUTALIST</div>
<div style="font-size: 18px; font-weight: 900; color: #000; margin-bottom: 6px;">Hard Edges, Bold Type</div>
<div style="font-size: 13px; color: #333; line-height: 1.5; margin-bottom: 12px;">Cream background, 3px borders, hard shadows, yellow accent. Zero border-radius. Maximum impact.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; background: #fffef0; border: 2px solid #000; display: inline-block;"></span>
<span style="width: 20px; height: 20px; background: #000000; display: inline-block;"></span>
<span style="width: 20px; height: 20px; background: #ffe600; border: 2px solid #000; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #ffe600; border-top: 3px solid #000; font-size: 13px; font-family: monospace; color: #000; font-weight: 700;">seite theme apply brutalist</div>
</div>

<div style="border: 1px solid #e8e4df; border-radius: 20px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #f4f0eb; font-family: system-ui, -apple-system, sans-serif; min-height: 140px;">
<div style="font-size: 11px; color: #5046e5; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">BENTO</div>
<div style="font-size: 18px; font-weight: 700; color: #1a1a1a; margin-bottom: 6px;">Card Grid Layout</div>
<div style="font-size: 13px; color: #555; line-height: 1.5; margin-bottom: 12px;">CSS grid, mixed card sizes, rounded corners, soft shadows. Dark and indigo accent cards for rhythm.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #f4f0eb; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #1a1a1a; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #5046e5; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #e8e4df; border-top: 1px solid #ddd; font-size: 13px; font-family: monospace; color: #555; border-radius: 0 0 20px 20px;">seite theme apply bento</div>
</div>

<div style="border: 1px solid #1f3a1a; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #0a0a0a; font-family: ui-monospace, monospace; min-height: 140px;">
<div style="font-size: 11px; color: #20c20e; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">TERMINAL</div>
<div style="font-size: 18px; font-weight: 700; color: #33ff00; margin-bottom: 6px;">Phosphor Green</div>
<div style="font-size: 13px; color: #20c20e; line-height: 1.5; margin-bottom: 12px;">Monospace everything, green-on-black CRT look, prompt-style headings.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #0a0a0a; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #33ff00; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #20c20e; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #0e1209; border-top: 1px solid #1f3a1a; font-size: 13px; font-family: monospace; color: #20c20e;">seite theme apply terminal</div>
</div>

<div style="border: 1px solid #e2e2e2; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #ffffff; font-family: Georgia, serif; min-height: 140px;">
<div style="font-size: 11px; color: #e63946; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">MAGAZINE</div>
<div style="font-size: 18px; font-weight: 700; color: #111111; margin-bottom: 6px;">Editorial Columns</div>
<div style="font-size: 13px; color: #555555; line-height: 1.5; margin-bottom: 12px;">Masthead, Playfair Display headlines, multi-column article grid, red accent.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #ffffff; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #111111; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #e63946; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f8f8f8; border-top: 1px solid #e2e2e2; font-size: 13px; font-family: monospace; color: #555555;">seite theme apply magazine</div>
</div>

<div style="border: 1px solid #e2e2e2; border-radius: 12px; overflow: hidden;">
<div style="padding: 24px 20px 16px; background: #fffff8; font-family: Georgia, serif; min-height: 140px;">
<div style="font-size: 11px; color: #a00000; font-weight: 600; letter-spacing: 0.5px; margin-bottom: 8px;">ACADEMIC</div>
<div style="font-size: 18px; font-weight: 700; color: #111111; margin-bottom: 6px;">Scholarly Serif</div>
<div style="font-size: 13px; color: #555555; line-height: 1.5; margin-bottom: 12px;">EB Garamond, wide margins, numbered headings and footnotes for long-form research writing.</div>
<div style="display: flex; gap: 6px;">
<span style="width: 20px; height: 20px; border-radius: 50%; background: #fffff8; border: 1px solid #ddd; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #111111; display: inline-block;"></span>
<span style="width: 20px; height: 20px; border-radius: 50%; background: #a00000; display: inline-block;"></span>
</div>
</div>
<div style="padding: 10px 20px; background: #f8f8f8; border-top: 1px solid #e2e2e2; font-size: 13px; font-family: monospace; color: #555555;">seite theme apply academic</div>
</div>

</div>

### Why the gallery is embedded

The gallery lives in the `seite` binary instead of on a server: it works offline, every theme is versioned with the binary (a theme always matches the partials and template variables of the seite that applies it), and there is no download to verify. `seite theme install <url>` remains for community themes.

{{% callout(type="tip") %}}
In the dev server REPL, type `theme dark` to instantly preview any theme with live reload, no restart needed.
{{% end %}}

## Install Community Themes

Download and install themes shared by others:

```bash
# Install from a URL
seite theme install https://example.com/themes/aurora.tera

# Install with a custom name
seite theme install https://example.com/themes/aurora.tera --name my-aurora
```

Installed themes are saved to `templates/themes/<name>.tera` and appear in `seite theme list`. Apply them like any bundled theme:

```bash
seite theme apply aurora
```

## Export & Share Your Theme

Package your current theme for others to install:

```bash
# Export with a description
seite theme export my-theme --description "Dark theme with green accents"

# The theme is saved to templates/themes/my-theme.tera
```

Host the `.tera` file anywhere (GitHub, your own server, a Gist) and others can install it with `seite theme install <url>`.

## Design Your Own Theme

The bundled themes are examples. For your own design, start from the `blank` starter and let your coding agent (or impeccable) design it:

```bash
seite theme new                  # blank starter: templates/base.html + static/css/site.css
seite skill install impeccable   # optional: design tooling for your agent
```

Then, in your agent: `/impeccable init` and `/impeccable <describe your design>` with impeccable, or `/seite theme <description>` without it. `seite theme create "<description>"` does the same from the shell, starting from `blank` when the site is still on a bundled example. The full flow is in [Design your own theme](/docs/custom-themes).

After designing, export it to share:

```bash
seite theme export cyberpunk --description "Neon cyberpunk with scanlines"
# Share templates/themes/cyberpunk.tera with anyone
```

## Theme Anatomy

Every theme (bundled or installed) is a Tera template that serves as `base.html`. It owns layout and CSS, and includes seite's partials for everything that must stay correct:

- **HTML structure**: `<!DOCTYPE html>` through `</html>`, `<html lang="{{ lang }}">`
- **`<title>{% block title %}…{% endblock %}</title>`**, then `{% include "seite/head.html" %}` (canonical, Open Graph, Twitter Card, JSON-LD, hreflang, feeds, `llms.txt`, markdown alternate, robots, KaTeX)
- **Template blocks**: `title`, `head`, `extra_css`, `header`, `content`, `footer`, `extra_js`
- **Search**: `{% include "seite/search.html" %}` where the box goes, `{% include "seite/scripts.html" %}` before `</body>`
- **Language switcher**: `{% include "seite/lang-switcher.html" %}`
- **Accessibility**: skip-to-main link, focus styles, `prefers-reduced-motion`

The bundled examples keep their CSS inline in `<style>`; `blank` links `static/css/site.css`, which is easier for agents and design tools to edit.

## Managing Themes

```bash
seite theme list              # Starters, the gallery, and installed themes
seite theme apply <name>      # Apply a theme (writes templates/base.html)
seite theme new [name]        # Start your own design from the blank starter
seite theme install <url>     # Download a theme from a URL
seite theme export <name>     # Export current theme as shareable .tera file
seite theme create "<prompt>" # Design a custom theme with your coding agent
```

In the dev server REPL, type `theme` to list themes or `theme <name>` to apply and auto-rebuild.

## Next Steps

- [Design your own theme](/docs/custom-themes): the blank starter, impeccable, and the theme contract
- [Templates & Themes](/docs/templates): customize theme blocks, override templates, and use data files
- [AI Agent](/docs/agent): use Claude for content creation, site management, and more
