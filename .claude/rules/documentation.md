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
Static markdown embedded via `include_str!` into the per-agent rules and the `seite` skill for user sites (`.claude/rules/*.md`, `.cursor/rules/*.mdc`, `.claude/skills/seite/`, `.agents/skills/seite/`). Rules = path-triggered hard contracts (`src/scaffold/*.md`, registered in `RULES`); the skill = workflows (`src/scaffold/skill-seite/SKILL.md` + `reference/*.md`, registered in `SKILLS`/`SEITE_REFERENCES`). Don't repeat a fact in both. AGENTS.md itself is generated in `generate_agents_md` (`src/cli/init.rs`) and stays compact (size budget tested). Bump `# seite-skill-version` when changing the skill or its references. `init`/`upgrade` also write every embedded doc to `.seite/docs/`.
