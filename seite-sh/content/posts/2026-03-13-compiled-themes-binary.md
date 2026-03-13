---
title: "6 Themes, 0 Downloads: Why seite Compiles Themes into the Binary"
date: 2026-03-13
description: "seite ships 6 production themes compiled directly into an 8 MB binary. No downloads, no npm, no git submodules. Here's why we made that call."
tags:
 - rust
 - static-site-generator
 - themes
 - engineering
extra:
 primary_keyword: "seite themes binary"
 meta_title: "6 Themes, 0 Downloads: Why seite Compiles Themes into the Binary | seite"
 meta_description: "seite ships 6 production-ready themes compiled into the binary. No downloads, no npm, no git submodules. Here's why we made that call and what it costs."
 url_slug: "compiled-themes-binary"
---

You're on a flight, no wifi. You open your laptop, run `seite theme apply brutalist`, and the theme is there. No CDN request, no git clone, no npm install. The theme was already inside the binary on your machine, compiled in at build time.

That's the seite themes binary approach in one sentence. Every seite release ships with 6 production-ready themes embedded directly in the binary using Rust's `include_str!` macro. Install the binary and you have all the themes. No downloads, no package managers, no network requests ever.

This post explains why we made that call, how `include_str!` makes it work, and what the honest tradeoffs are. If you've ever watched a [Jekyll](https://jekyllrb.com/) build fail because a gem wasn't installed, or lost a CI run to a missing git submodule, this might resonate.

## The Standard Theme Distribution Problem

Static site generators have been solving theme distribution roughly the same way since Jekyll popularized the pattern. The shape of the problem looks like this:

1. Find a theme you like
2. Install it via npm, RubyGems or a git submodule
3. Hope the version is compatible with your SSG version
4. Set up CI to install the theme before every build
5. Hope the upstream repo exists next time you clone

Each step is a new failure mode. Let's be specific about what that looks like in practice.

### npm Themes (Astro, Eleventy)

```bash
npm install @astro-community/astro-theme-typography
```

This pulls in a `node_modules` directory for what is fundamentally a set of HTML templates. Theme updates require `npm update` followed by manual regression testing. Minor version bumps from theme authors can include breaking changes if they're not careful with semver. `npm audit` will flag vulnerabilities in theme dependencies you never asked for and can't patch.

### RubyGems Themes (Jekyll)

```bash
gem install jekyll-theme-minimal
bundle install
```

You're now managing Ruby, Bundler and gem versions in addition to the site generator itself. Apple Silicon caused platform-specific gem compilation failures for years. Theme development requires either a local gem server or a full RubyGems publish cycle. A fresh checkout on a new machine requires getting the gem environment right before you can build anything.

### Git Submodule Themes (Hugo, Zola)

```bash
git submodule add https://github.com/user/hugo-paper.git themes/paper
git submodule update --init --recursive
```

Git submodules are used by [Hugo](https://gohugo.io/) and [Zola](https://www.getzola.org/). They're the most honest of the three approaches: you're explicitly declaring the dependency. They're also the most fragile in practice. A fresh clone without `--recursive` silently produces an empty themes directory. The upstream repo can disappear, rename, or change its license. CI needs `git submodule update` before every build. New team members forget to initialize submodules. Every one of these fails quietly if you're not watching.

The common thread across all three: a network request and a package manager are required at install time. The theme lives outside the binary, outside your reproducibility guarantees, and outside your control.

## The seite Approach: Compile Time Embedding

When we designed seite, we made a deliberate choice. If we ship themes, they go inside the binary. The mechanism is Rust's `include_str!` macro, which resolves file contents at compile time and embeds them as `&'static str` constants.

Here's the pattern (simplified for illustration):

```rust
// src/themes.rs
const THEME_DEFAULT: &str = include_str!("themes/default.tera");
const THEME_MINIMAL: &str = include_str!("themes/minimal.tera");
const THEME_DARK: &str = include_str!("themes/dark.tera");
const THEME_DOCS: &str = include_str!("themes/docs.tera");
const THEME_BRUTALIST: &str = include_str!("themes/brutalist.tera");
const THEME_BENTO: &str = include_str!("themes/bento.tera");
```

At compile time, the Rust compiler reads each `.tera` file and embeds its contents as a string constant in the binary. There are no file system lookups at runtime and no network requests ever. When you run `seite theme apply minimal`, the code writes `THEME_MINIMAL` to `templates/base.html`. One file write. Done.

Each theme is a `base.html` [Tera](https://keats.github.io/tera/) template: HTML with Tera syntax for variables, blocks and conditional rendering. The full SEO and GEO head block (Open Graph, JSON-LD structured data, canonical URL, [llms.txt](/blog/what-is-llms-txt) discovery link, Twitter Card tags) is compiled into every theme. If a theme ships in a seite release, it has correct SEO output by definition. There's no way to ship a theme without it.

You can browse all six themes in the [theme gallery](/docs/theme-gallery) to see what each looks like before applying one.

## The Developer Experience This Creates

From `seite init` to a themed, built site, there are zero theme-related setup steps:

```bash
# Install seite (once, offline after this)
curl -fsSL https://seite.sh/install.sh | sh

# Create a new site
seite init mysite --title "My Blog"
cd mysite

# All themes available immediately, offline
seite theme list

# Apply a theme
seite theme apply brutalist

# Build with no dependencies to install
seite build
```

That's it. No `npm install`. No `bundle install`. No `git submodule update --init`. The binary you installed is the complete tool.

For an existing site, applying a theme is two commands:

```bash
seite theme apply docs
seite build
```

`seite theme apply` writes a new `templates/base.html`. `seite build` picks it up immediately. The file is yours to edit, version control and own from that point forward.

### Three Scenarios Where This Matters

**Scenario 1: Marcus is setting up a new developer machine.** He clones the site repo, runs `seite build` and gets a working build on the first try. He never touches a package manager. There's no README section about installing theme prerequisites because there aren't any.

**Scenario 2: Priya is setting up a CI pipeline for a client site.** The GitHub Actions workflow runs `seite build`. No node installation step, no gem install, no submodule initialization. The workflow is 12 lines. It never breaks due to a theme dependency going missing.

**Scenario 3: Daniele is deploying from an air-gapped corporate environment.** Strict egress restrictions mean most npm registries and GitHub are blocked. `seite theme apply` works anyway because there's nothing to download.

## The Tradeoffs, Honestly

The embedded approach gains you a lot. It also costs you some things. Here's the accurate accounting.

### What You Gain

**Zero-dependency themes.** Works offline, in Docker containers with no internet access, in CI with strict network policies, and on day one with no setup. There is no gap between "installed seite" and "themes available."

**Version coherence.** The theme bundled with seite v0.4.0 is always the theme bundled with seite v0.4.0. Pin the binary version and you pin the theme. No accidental upstream changes, no theme author introducing a breaking change between your last build and this one.

**No maintenance surface.** No theme `package.json` to audit, no RubyGem with a CVE, no submodule pointing at a deleted repo. Theme security is seite's responsibility, not yours.

**Deterministic builds.** The same seite binary with the same content produces the same output. Offline, online, today or six months from now.

### What You Trade Away

**Binary size.** Six Tera templates add roughly 80 KB to the binary. The full seite binary is around 8 MB. Theme content is under 1% of total binary size. That's an acceptable cost for most use cases, but it's real and worth naming.

**No community theme ecosystem.** You cannot run `seite theme install some-community-theme`. The only themes available via `seite theme apply` are the ones compiled into the specific seite version you have installed. If you want a theme that doesn't exist in the bundled set, you need `seite theme create` or you edit `templates/base.html` directly.

**Update cycle tied to seite releases.** A theme improvement ships with the next seite release, not independently. This is a feature if you value stability; it's a constraint if you want to track upstream theme changes actively without upgrading the tool.

To put the size tradeoff in context: `npm install` for a typical Astro theme pulls 40 to 80 MB of `node_modules` at install time. seite's 80 KB for all six themes is paid once when you download the binary.

## Why AI Agent Workflows Specifically Benefit

seite's [AI agent](/docs/agent) runs Claude Code with full site context. When an agent session applies a theme, a few things need to be true:

- No network request should be triggered (agent sessions shouldn't depend on external availability)
- The operation should be deterministic (same command, same result, every time)
- The agent should know exactly what template variables and blocks each theme supports, without runtime discovery

All three are trivially satisfied when themes are compiled into the binary. The agent calls `seite theme apply docs` with full confidence that it will work offline, produce a known template structure, and never fail due to an external dependency. The agent can iterate on themes inside automated sessions without any network-related failure modes.

Contrast this with an agent trying to apply a Hugo theme via git submodule:

```bash
git submodule add https://github.com/user/hugo-paper.git themes/paper
git submodule update --init
```

The agent now depends on GitHub being reachable, the repo not having been deleted or renamed, and the theme being compatible with the current Hugo version. Any of these can fail silently or with confusing error output. The agent has to handle network failure modes that have nothing to do with the task it was given.

This is part of a broader principle in seite's design: reduce the number of external dependencies the agent has to reason about. If you're interested in how this plays out for content creation, the post on [building a website with an AI coding agent](/blog/how-to-build-website-ai-coding-agent) goes deeper.

## When the Bundled Themes Aren't Enough

Six themes cover the most common use cases: default, minimal, dark, docs, brutalist and bento. When you need something they don't provide, `seite theme create` generates a custom `templates/base.html` using Claude Code:

```bash
seite theme create "editorial serif, warm cream background, single column layout, 620px max width"
```

This runs a Claude Code session with the full Tera template spec as context, including all required SEO blocks, template variables and accessibility requirements. The output goes to `templates/base.html`. From there it's a normal file under version control: edit it, commit it, own it.

The custom theme route is a different thing from a community theme ecosystem. It's better in some ways (you get exactly what you asked for, no dependency chain) and worse in others (no community maintenance, no sharing). We're not pretending otherwise.

For everything you can customize in themes, including block overrides and per-collection templates, see the [custom themes documentation](/docs/custom-themes).

## The Design Decision in Summary

The seite themes binary approach makes one tradeoff: binary size for reliability. Six themes add 80 KB to an 8 MB binary. In exchange, you get themes that work everywhere the binary runs, with no setup, no maintenance and no network dependency.

For most developers building sites with seite, theme setup is a solved problem the moment you install the tool. That's the point.

The community ecosystem tradeoff is real. If you need a theme that doesn't exist in the bundled set, `seite theme create` gets you there. It's not the same as a community marketplace, but it produces results without adding a dependency chain.

See all six themes in the [theme gallery](/docs/theme-gallery), dive into how to customize them in the [custom themes guide](/docs/custom-themes), or try the agent if you want to generate a custom theme from a description with `seite agent "create a theme that..."`.
