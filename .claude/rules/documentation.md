---
paths:
  - "seite-sh/content/docs/**"
  - "src/docs.rs"
  - "src/scaffold/**"
---
# Documentation

- Docs site in `seite-sh/`, built with seite itself
- Docs in `seite-sh/content/docs/` — compiled into binary via `include_str!` in `src/docs.rs`
- Single source of truth: update docs and the binary embeds them automatically

## Update docs when changing user-facing features:
- `cli-reference.md` — all CLI commands and flags
- `deployment.md` — deploy targets, pre-flight checks, setup
- `configuration.md` — `seite.toml` options
- `collections.md` — collection presets and config
- `templates.md` — template variables and blocks
- `i18n.md` — multi-language features
- `trust-center.md` — trust center setup and management
- `contact-forms.md` — contact form providers and shortcode

## Also update this repo's CLAUDE.md when adding new patterns or architecture.

## Scaffold files (`src/scaffold/`)
Static markdown sections embedded via `include_str!` into generated CLAUDE.md and `.claude/rules/` for user sites. Edit these when changing the AI agent context.
