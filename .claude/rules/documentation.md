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

## Also update the repo's canonical AGENTS.md instructions when adding new patterns or architecture.

## Scaffold files (`src/scaffold/`)
Static markdown sections embedded via `include_str!` into generated AGENTS.md and the per-agent rules/skills for user sites (`.claude/rules/*.md`, `.cursor/rules/*.mdc`, `.claude/skills/`, `.agents/skills/`). Register new rules/skills once in `RULES`/`SKILLS` in `src/cli/harness.rs`. Edit these when changing the AI agent context.
