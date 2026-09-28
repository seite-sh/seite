---
title: "Static Site Generator with Built-In Themes: 10 Compiled In"
date: 2026-03-13
updated: 2026-09-28
description: "seite is a static site generator with built-in themes: 10 of them compiled into one binary. No npm, no gems, no git submodules. How it works and what it costs."
tags:
 - rust
 - static-site-generator
 - themes
 - engineering
extra:
 primary_keyword: "static site generator with built-in themes"
---

You're on a flight with no wifi. You run `seite theme apply terminal` and your site has a new look. Nothing was fetched. The theme was already inside the `seite` binary on your laptop, compiled in when the release was built.

That's what we mean by a static site generator with built-in themes. Every seite release embeds 10 complete themes using Rust's `include_str!` macro: default, minimal, dark, docs, brutalist, bento, landing, terminal, magazine and academic. Install the binary and you have all of them. Applying one needs no package manager and no network.

When this post first went up in March 2026, seite shipped six themes. Four more have landed since, and the theme commands have grown: you can now install a theme from a URL, export your own and apply either kind by name. This update covers how the embedding works, what `seite theme apply` does to your files, the four newer themes and the tradeoffs we accepted. If you've ever lost a CI run to a missing git submodule or a Gemfile that wouldn't resolve, some of this will sound familiar.

## The Problem with Downloaded Themes

Most static site generators treat a theme as a dependency that lives somewhere else. How it reaches your machine depends on the ecosystem:

- **Jekyll** uses gem-based themes. You add something like `gem "minima"` to your `Gemfile`, run `bundle install` and now manage Ruby and Bundler versions alongside the site.
- **Hugo and Zola** sites usually pull a theme into `themes/` as a git clone or submodule. Hugo also offers Hugo Modules, which need Go installed.
- **Astro and Eleventy** themes are usually starter projects you copy or packages you install through npm, so a theme arrives with a `package.json` and a `node_modules` tree.

The submodule version of this is the one most people have been bitten by:

```bash
git clone https://github.com/you/site.git
cd site
# themes/paper is an empty directory until you also run:
git submodule update --init --recursive
```

Each approach puts a step between "clone the repo" and "build the site", and each step can fail. A registry is down. An upstream repo gets renamed or deleted. A theme release changes a template you depended on. A CI config forgets the submodule init.

None of these failures has anything to do with your content, and all of them show up at the worst moment: on a new machine, in CI or in a locked-down network.

We wanted theme setup to be something you never think about. The simplest way to get there was to stop distributing themes separately and build a static site generator with built-in themes.

## How a Static Site Generator with Built-In Themes Works

Each seite theme is a single `base.html` [Tera](https://keats.github.io/tera/) template, stored as a `.tera` file in `src/themes/` in the seite repo. `src/themes.rs` wraps each one in a small constructor. [`include_str!`](https://doc.rust-lang.org/std/macro.include_str.html) reads the file at compile time and stores its contents in the binary as a `&'static str`:

```rust
// src/themes.rs (abridged)
pub fn terminal() -> Theme {
    Theme {
        name: "terminal",
        description: "Monospace hacker theme with green-on-black terminal aesthetic",
        base_html: include_str!("themes/terminal.tera"),
    }
}

pub fn all() -> Vec<Theme> {
    vec![
        default(), minimal(), dark(), docs(), brutalist(),
        bento(), landing(), terminal(), magazine(), academic(),
    ]
}
```

At runtime there is no theme directory to look up and nothing to fetch. `seite theme list` prints the name and description of each entry in `all()`. `seite theme apply` writes the chosen `base_html` string to `templates/base.html`. That's the whole mechanism.

The rest of the [template set](/docs/templates) works the same way. Page templates like `post.html`, `doc.html` and `404.html` extend `base.html`, and the build falls back to embedded defaults for any template your `templates/` directory doesn't provide. `seite init` writes the default theme to `templates/base.html`, so you have a real file to edit from the first commit.

Every bundled theme carries the same SEO and GEO head block: a canonical URL, Open Graph and Twitter Card tags, JSON-LD structured data, a link to your [llms.txt file](/blog/what-is-llms-txt) and a markdown alternate link for each page. Switching themes changes the design, not your metadata.

### What 10 Themes Cost in Bytes

An earlier version of this post put six themes at roughly 80 KB. With 10 themes that number no longer holds, so here are measured ones for v0.20.0:

- The 10 `.tera` files add up to 349 KB of template text (5,527 lines). `include_str!` stores that text as-is, so that's what the themes add to the binary.
- Compressed with gzip on their own, the same files come to about 42 KB.
- The v0.20.0 release downloads are compressed archives of 7.5 MB (macOS on Apple Silicon) to 8.6 MB (Windows x64).

So the themes are roughly half a percent of the smallest download. That's small, but it isn't zero, and every user pays it whether they use one theme or all 10.

## The 10 Built-In Themes

Here's what `seite theme list` prints. It works anywhere, even outside a seite project, because the list comes from the binary:

```bash
$ seite theme list
Bundled themes
  default    Clean, readable theme with system fonts
  minimal    Ultra-minimal, typography-first theme
  dark       Dark mode theme, easy on the eyes
  docs       Documentation-focused theme with sidebar layout
  brutalist  Neo-brutalist theme with thick borders and hard shadows
  bento      Card grid layout inspired by bento box design
  landing    Marketing and landing page theme with hero sections and CTAs
  terminal   Monospace hacker theme with green-on-black terminal aesthetic
  magazine   Multi-column editorial layout with featured articles
  academic   Scholarly serif theme for research and long-form writing

  Preview all: https://seite.sh/docs/theme-gallery
```

The first six are the ones this post originally covered. The four newer ones fill gaps those left:

- **landing** is for marketing pages: hero sections and calls to action, for a product homepage or a launch page.
- **terminal** is monospace, green on black. It suits developer blogs and CLI tool sites that want to look like a shell.
- **magazine** is a multi-column editorial layout with featured articles, for sites with a lot of posts to surface.
- **academic** uses serif typography built for research notes and long-form writing.

The [theme gallery](/docs/theme-gallery) has visual previews of the original six, plus the install and export workflow. The quickest way to judge the newer four is to apply them to your own site and run `seite serve`. Add `--json` to `seite theme list` and you get the same list, plus any installed themes, as structured data.

## What `seite theme apply` Does to Your Files

Applying a theme to an existing site is one command, then a build:

```bash
$ seite theme apply terminal
✓ Applied bundled theme 'terminal'
ℹ Run 'seite build' or the watcher will pick it up automatically.
```

If `seite serve` is running, the file watcher picks up the new `templates/base.html` and rebuilds. From then on it's an ordinary file in your repo. Edit it, commit it, diff it. Nothing replaces it except another `seite theme apply`.

v0.20.0 made that command safer in two ways.

**It backs up your edits.** Before writing, seite compares your current `templates/base.html` with every bundled and installed theme. If it matches none of them, you've customized it, so seite copies it to `base.html.bak` first (then `base.html.bak.1`, `base.html.bak.2` and so on if that name is taken):

```bash
$ seite theme apply academic
ℹ Backed up your customized base.html to /Users/you/mysite/templates/base.html.bak
✓ Applied bundled theme 'academic'
ℹ Run 'seite build' or the watcher will pick it up automatically.
```

An unmodified theme file gets replaced without a backup, because the original is still in the binary.

**It validates theme names.** Names end up in file paths (`templates/themes/<name>.tera`), so they must match `^[a-z0-9][a-z0-9-]*$`: lowercase letters, digits and hyphens. Anything else, like `../evil`, is rejected before seite touches the disk. An unknown but valid name gets a pointer to `seite theme list`.

The MCP server's `seite_apply_theme` tool goes through the same code, so a coding agent that switches your theme gets the same backup and the same name check.

## When You Need a Theme That Isn't Built In

The bundled set is curated, and it won't cover every design. Three commands handle the rest, and all of them store their result as plain files in your project.

**Install a theme from a URL.** A seite theme is a single `.tera` file, so sharing one means hosting one file:

```bash
seite theme install https://example.com/themes/coral.tera --name coral
seite theme apply coral
```

`install` downloads the file, checks that it looks like an HTML template and saves it to `templates/themes/coral.tera`. That's a network request, but only once. The file lives in your repo from then on, so builds and later applies stay offline.

The check confirms the file is HTML, not that it's well built, so read a theme before you apply it. Bundled themes take priority by name, so give an installed theme a name of its own. seite warns you if it would shadow a bundled one.

**Export your own.** Once you've tuned a theme, package it for someone else:

```bash
seite theme export coral-wide --description "Coral with a wider content column"
```

This copies `templates/base.html` to `templates/themes/coral-wide.tera` and adds a `{#- theme-description: ... -#}` comment at the top, which `seite theme list` shows next to the name. Put the file anywhere with a URL and others can `seite theme install` it.

**Generate one with Claude Code.** Describe the design and let an agent write it:

```bash
seite theme create "editorial serif, warm cream background, single column, 620px max width"
```

This runs Claude Code with the Read, Write and Edit tools and a prompt that spells out the required Tera blocks, the template variables and the patterns for search, pagination and language switching. Claude writes `templates/base.html` directly.

Two caveats. It needs Claude Code installed. And it doesn't go through the backup step that `apply` does, so commit your current template first. Compare the generated `<head>` with a bundled theme's before you ship: the prompt doesn't spell out the full SEO block the bundled themes carry.

To build or adjust a theme by hand, the [custom themes guide](/docs/custom-themes) walks through the required blocks, the SEO and JSON-LD tags to include, search, pagination, accessibility and starting from a copy of a bundled theme.

What doesn't exist yet is a registry. There's no `seite theme browse` and no `seite theme validate`. Both are listed under the [theme ecosystem item on the roadmap](/roadmap/theme-community-ecosystem), which is in progress, not shipped. Today, sharing themes means sharing URLs.

## The Tradeoffs of a Static Site Generator with Built-In Themes

Compiling themes in buys a lot. It also costs some things. Here's the accounting.

| | Built-in themes (seite) | Downloaded themes (npm, gems, submodules) |
|---|---|---|
| Works offline and in locked-down CI | Yes | Only after the first fetch |
| Theme version tied to tool version | Yes | Separate version to pin |
| Dependency surface to audit | None | Package, gem or repo |
| Size cost | ~42 KB compressed, for everyone | Only for themes you install |
| Choice | 10 bundled + install/export/generate | Whole marketplace |
| Theme fixes ship | With the next seite release | On the theme's own schedule |

### What You Gain

**Themes work wherever the binary runs.** Offline, in a container with no network, in CI with strict egress rules, on a new machine on day one. There's no gap between "installed seite" and "themes available."

**Version coherence.** The themes in seite v0.20.0 are the themes in seite v0.20.0. Pin the binary version in CI and you've pinned the themes.

**No theme dependency surface.** There's no theme `package.json` to audit, no gem to update and no submodule pointing at a repo that might disappear.

### What You Give Up

**Binary size.** Measured above: 349 KB of template text in the binary, about 42 KB compressed, paid by everyone.

**A curated set instead of a marketplace.** You get 10 themes plus whatever you install, export or generate. There's no catalog to browse yet.

**Theme updates follow seite releases.** A fix to a bundled theme ships with the next seite version, not on its own schedule.

**Your `base.html` is a copy.** Upgrading seite doesn't rewrite `templates/base.html`. To pick up improvements to a bundled theme, re-apply it. If you'd customized it, your version lands in `base.html.bak` and you merge your changes back by hand. That's the price of owning the file.

For us that trade is worth it. A theme you can't build without a network connection is a theme that will eventually fail a build for reasons unrelated to your site.

## Why Coding Agents Benefit

seite is built to be driven by coding agents. Claude Code, Codex, OpenCode and Cursor can all load its [MCP server](/docs/mcp-server). When an agent changes your theme, a few things need to be true:

- No network request, so the task doesn't depend on a third party being up
- The same command gives the same result every time
- The agent can find out what's available without guessing

Built-in themes cover all three. The agent calls `seite_apply_theme` or runs `seite theme apply docs`, and `seite theme list --json` tells it exactly which bundled and installed themes exist. If the site's `base.html` was hand-edited, the backup keeps a bad agent decision recoverable.

Compare an agent adding a Hugo theme as a submodule. It now depends on GitHub being reachable, the repo still existing under the same name and the theme matching the installed Hugo version. When one of those fails, the agent spends its turn on network errors instead of your site. For the full agent workflow, see [how to build a website with an AI coding agent](/blog/how-to-build-website-ai-coding-agent).

## Try the Built-In Themes

The short version:

- seite compiles 10 themes into its binary with `include_str!`. Applying one never touches the network.
- `seite theme apply` backs up a customized `base.html` and rejects unsafe names.
- Need something else? `install` from a URL, `export` your own or `create` one with Claude Code.
- The cost is 349 KB of template text in the binary, a curated set and theme updates tied to seite releases.

If you already have seite, run `seite theme list` and try a couple of themes against your own content. They're already on your machine. Browse the theme gallery linked above for previews, or read the [getting started guide](/docs/getting-started) if you're new to seite.
