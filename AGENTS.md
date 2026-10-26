# seite — Static Site Generator with LLM Integration

## What This Is

`seite` is a Rust CLI static site generator designed to be AI-native. Content and templates are structured for LLM generation and consumption. Sites ship with `llms.txt`, `llms-full.txt`, and markdown versions of every page alongside the HTML.

Generated sites are set up for coding agents (Claude Code, Codex, OpenCode, Cursor, Gemini CLI): a compact AGENTS.md, one routed `/seite` skill, check hooks, and version-matched docs in `.seite/docs/`. `seite agent` is a thin launcher for those agents — no API keys, no paid services.

## Quick Commands

```bash
cargo build          # Build the binary
cargo test           # Run all tests
cargo fmt --all      # Format — CI enforces this
cargo clippy         # Lint — must be zero warnings
cargo run -- init mysite --title "My Site" --collections posts,docs,pages
cargo run -- build   # Build site from seite.toml
cargo run -- check   # Validate config/content/templates/links (no dist/ writes)
cargo run -- context # Agent orientation: site summary + DIRECTIVES (also: docs, page, search, stats, templates, frontmatter, explain)
cargo run -- serve   # Dev server with REPL (live reload)
cargo run -- serve --open  # Dev server + open browser
cargo run -- serve --host 0.0.0.0  # Bind to all interfaces
cargo run -- new post "My Post" --tags rust,web
cargo run -- agent "create a blog post about Rust"
cargo run -- hooks status        # coding-agent check hooks (on|off|status)
cargo run -- check --file content/posts/x.md  # fast per-file check
cargo run -- theme new           # blank starter for a custom design
cargo run -- theme create "coral brutalist with lime accents"
cargo run -- deploy  # Commit, push, build, and deploy
cargo run -- skill install seomachine
cargo run -- skill install impeccable   # pinned npm installer, asks first
cargo run -- upgrade # Upgrade project config to current binary
cargo run -- self-update
cargo run -- completions bash  # Generate shell completions
```

## Module Map

```
src/
  main.rs              CLI entrypoint (clap dispatch)
  lib.rs, error.rs     Module declarations, PageError enum (thiserror)
  safe_fs.rs           Project-confined writes/deletes (refuse symlinked components) for init/upgrade/hooks/skill/serve
  diagnostics.rs       Diagnostic/Diagnostics (stable codes, file:line:col, JSON `docs_url`) for build, check, MCP; every code needs a `## <code>` section in seite-sh/content/docs/errors.md
  themes.rs            11 bundled themes (starters default/docs/landing/blank + gallery) from src/themes/*.tera; layout + CSS only
  shortcodes/          ShortcodeRegistry, parser, builtins (youtube, vimeo, gist, callout, figure, contact_form)
  build/               build pipeline (mod.rs), analytics, base_path, code_copy, links, markdown, feed, sitemap, discovery, images, math, mermaid, taxonomy, related, series, content_model, external_links (`seite check --external`)
  docs.rs              Embedded docs (16 pages from seite-sh/content/docs/ + releases; `error_sections()` backs `seite explain`)
  i18n.rs              Language-map resolution + `i18n`/`localize` Tera filter (per-language data values)
  meta.rs              Project metadata (.seite/config.json)
  mcp/                 MCP server (JSON-RPC over stdio): mod.rs, resources.rs, tools.rs
  cli/                 subcommands: init, new, build, check, serve, deploy, agent, theme, hooks, mcp, workspace, upgrade, contact, collection, access, skill, self_update, completions, perf, telemetry; agent-facing context, docs, page, search, stats, templates, frontmatter, explain (thin adapters over MCP tool handlers via site_tools.rs)
  update_check.rs      Background update check (24h cache)
  scaffold/            Static markdown for generated AGENTS.md + .claude/rules/ (include_str!)
  config/              SiteConfig, CollectionConfig, defaults; unknown_keys.rs walks the raw TOML against the schema for `config-unknown-key` diagnostics
  data/                Data file loading (YAML/JSON/TOML)
  content/             Frontmatter parsing, ContentItem, slug generation, schema (typed frontmatter), schedule (future `date`/`expires`, site timezone)
  deploy/              GitHub Pages + Cloudflare + Netlify
  workspace/           Multi-site workspaces (config, build, server, deploy)
  output/              CommandOutput trait, human (colored), json
  server/              tiny_http dev server, file watcher, live reload
  templates/           Tera template loading with embedded defaults; seite/*.html partials (SEO head, search, lang switcher, scripts) every theme includes
tests/integration/     Integration tests, one module per command (assert_cmd + tempfile)
build.rs               Generates releases.md from changelog at compile time
```

## Config (seite.toml)

```toml
[site]
title = "My Site"
description = ""
base_url = "http://localhost:3000"
language = "en"
# timezone = "Europe/Berlin" # "today" for scheduled posts (IANA or "+02:00"; default: local date)

[[collections]]
name = "posts"
# strict_fields = true   # warn about frontmatter keys the schema does not declare

[collections.fields.description]   # typed frontmatter schema (src/content/schema.rs)
type = "string"          # string|text|number|integer|bool|date|datetime|url|image|enum|list|object
description = "Summary for search results"  # also: required, enum/values, items, min, max, pattern, default
# subdomain = "blog"     # optional: deploy separately
# private = true         # optional: exclude from discovery
# access_group = "members" # optional: password group when [access] is enabled

[access]                 # optional; Cloudflare Pages only
mode = "password"
session_hours = 168

[build]
output_dir = "dist"
minify = true
fingerprint = true
math = true              # KaTeX
mermaid = true           # Mermaid diagrams (```mermaid fences, client-side)
related = 3              # page.related size (0 disables; per-collection `related` overrides)
schedule = true          # scheduled publishing for dated collections (init writes it; absent = off)
# future = true          # build future-dated content (= `seite build --future`; serve always does)

[[taxonomies]]           # optional; `tags` is implicit (no feeds unless listed here)
name = "categories"      # frontmatter `categories: [..]` → /categories/, /categories/<slug>/ (+ feed.xml/atom.xml)
# paginate = 10

[deploy]
target = "github-pages"  # or "cloudflare" or "netlify"
auto_commit = true

[languages.es]
title = "Mi Sitio"

[images]
widths = [480, 800, 1200]
quality = 80
webp = true

[trust]
company = "Acme Corp"
frameworks = ["soc2", "iso27001"]

[analytics]
provider = "google"
id = "G-XXXXXXXXXX"

[contact]
provider = "formspree"
endpoint = "xpznqkdl"
```

## Patterns and Conventions

### Error Handling
- Library code: `crate::error::Result<T>` (PageError + thiserror). Never `unwrap()`.
- CLI commands: `anyhow::Result<()>`

### Output
- Human-readable output goes through `output::human::{success,info,warning,error,header}` — never raw `println!`.
- `--json` mode: set the command's payload with `output::json::set_data()`; the top-level `{"ok","command","data","warnings"}` / `{"ok":false,"error":{...}}` document (with `data` too, if the command set it before failing) is assembled and printed by `main.rs`. `CommandOutput` trait covers older per-command JSON.
- Child processes whose output isn't captured (`.status()`/`.spawn()` of git, gh, wrangler, netlify, npm, claude, …) must set `.stdout(output::child_stdout())` — stderr in `--json` mode, so stdout stays one JSON document on every platform (the Unix fd-1 redirect is only a backstop).
- Interactive prompts always go through `src/cli/prompt.rs`, never `dialoguer` directly — it degrades safely under `--yes`/`SEITE_YES=1` or a non-TTY (default if there is one, else an error naming the missing flag).

### Versioning
- Source of truth: `Cargo.toml` (semver). **Every code change must bump version.**
- PATCH: bug fixes, refactors. MINOR: new features/commands/config. MAJOR: breaking changes.
- **Always add a changelog entry** in `seite-sh/content/changelog/YYYY-MM-DD-vX-Y-Z.md` when bumping the version. Missing entries crash the deploy (`build.rs` embeds them into the binary).

### Testing
- `cargo fmt --all && cargo clippy && cargo test` before every commit
- **Check code coverage before committing**: `cargo tarpaulin --out Html` (or `cargo llvm-cov`). Coverage must not regress.
- Integration tests: `assert_cmd` + `tempfile`. Naming: `test_{command}_{behavior}`

### Adding a User-Facing Feature
1. Config model → `src/config/mod.rs`
2. Build pipeline → `src/build/mod.rs`
3. Init scaffolding → `src/cli/init.rs`
4. Docs → `seite-sh/content/docs/*.md` (compiled into binary)
5. MCP compliance → verify visibility via `seite://config` and `seite_lookup_docs`
6. AGENTS.md → update this file
7. Tests → unit + integration
8. Deploy fixtures → update `src/deploy/mod.rs` if SiteConfig changed
9. i18n → `{{ t.key }}` for UI text, `{{ lang_prefix }}` for links, `{{ value | i18n(lang=lang) }}` for per-language data prose (language maps; see `src/i18n.rs`)

### Generated Site Structure
- `seite init --agents claude,codex,opencode,cursor,gemini` (non-interactive default all; the interactive picker preselects agents detected on PATH/`$HOME`; stored in `.seite/config.json` `agents`) writes a compact AGENTS.md (always, ≤ ~3.5 KB; `generate_agents_md` in `src/cli/init.rs`), `.seite/docs/<slug>.md` (embedded docs, `harness::local_docs`), plus per-agent harness files, all rendered from one source in `src/cli/harness.rs`: `RULES`, `SKILLS` (+ `SEITE_REFERENCES`), the optional MCP entry, and the `PROVIDERS` table (one `AgentSpec` per agent — config display, MCP config path/format, rules dir/format, skills dir + frontmatter, permissions file, command wrappers, setup/MCP steps, detection; each quirk commented with its source):
  - claude: CLAUDE.md `@AGENTS.md` shim, `.claude/settings.json`, `.claude/launch.json` (desktop preview server), `.claude/rules/*.md` (`paths:` frontmatter), `.claude/skills/` (`seite` skill gets `argument-hint`)
  - cursor: `.cursor/cli.json`, `.cursor/rules/*.mdc` (`description`/`globs`/`alwaysApply: false`)
  - opencode: `opencode.json` (`permission`) + `.opencode/commands/seite.md` (`/seite` → skill wrapper)
  - gemini: GEMINI.md `@AGENTS.md` shim, `.gemini/settings.json` (hooks), `.gemini/commands/seite.toml` (`/seite` custom command)
  - codex/opencode/cursor/gemini: `.agents/skills/` (portable frontmatter: `name`/`description` only); sites without claude/cursor: `.agents/rules/` (the AGENTS.md rules index target)
  - MCP config only with `--mcp` (init and upgrade): `.mcp.json` + `enabledMcpjsonServers`/`mcp__seite`, `.cursor/mcp.json` + `Mcp(seite:*)`, `.codex/config.toml`, `opencode.json` `mcp`, `.gemini/settings.json` `mcpServers`; upgrade without `--mcp` never touches existing MCP config
- The `seite` skill (`src/scaffold/skill-seite/SKILL.md` + `reference/*.md`, version 4) is one routed skill: `/seite check|new|preview|build|deploy|theme|brand|landing|collection|upgrade|hooks` (`$seite` in Codex); references are written to `<skills dir>/seite/reference/` for every provider
- Harness output is snapshot-tested in `tests/snapshots/harness/*.snap` (`SEITE_UPDATE_SNAPSHOTS=1 cargo test --test integration harness_snapshot` to regenerate)
- Two-tier hooks (`src/cli/harness_hooks.rs`; only errors are reported, always exit 0, commands wrapped as `command -v seite … && seite check --hook <agent> [--post-edit] || true`): post-edit (`seite check --hook <agent> --post-edit` → `check::check_files` on the edited file) and turn-end (full check, one continuation per turn). claude `.claude/settings.json` `Stop` + `PostToolUse`, codex `.codex/hooks.json` `Stop` + `PostToolUse`, cursor `.cursor/hooks.json` `stop` + `postToolUse`, opencode `.opencode/plugins/seite-check.js` (`session.idle` + `tool.execute.after`), gemini `.gemini/settings.json` `AfterAgent` + `AfterTool`. `.seite/config.json` `hooks_installed` records them per tier (`<agent>` = turn-end, `<agent>:post-edit` = after-edit); `seite upgrade` adds only unrecorded tiers (0.20's bare records get post-edit only while the stop hook is present), so a user-deleted hook stays deleted, tier by tier; `seite hooks on|off|status [--tier stop|post-edit|all]` (`src/cli/hooks.rs`) manages them, removing only exact seite-generated commands
- AGENTS.md carries seite-owned `<!-- seite:agent-setup -->` (per-agent setup table; MCP column only when MCP is on) and `<!-- seite:context-rules -->` (rules index) blocks
- `seite upgrade [--agents …] [--mcp]` adds missing harness files for the selected agents (ungated, idempotent), merges into existing configs (JSON: add keys only; Codex TOML: append table, comments kept), refreshes the marker blocks and `.seite/docs/`, and never deletes files of deselected agents
- Generated permissions allow `seite theme apply|list` only (never `theme create`/`agent`; Cursor `Shell(seite:<sub>*)`); upgrade narrows the exact 0.20 rules. `seite agent`/`theme create` refuse to nest inside an agent session; `--yes` never adds auto-approve flags (`seite agent --auto-approve`, TTY only)
- All harness/upgrade/hooks/skill/dev-server writes and deletes go through `crate::safe_fs`; `.seite/docs/` (lazily written, version-stamped, pruned) and `.seite/dev.json` are gitignored. Upgrade prunes 0.20's legacy AGENTS.md sections (`src/scaffold/legacy/`) when unedited; one-time notices are recorded in `notices_shown`
- Rules files are created only when missing; skills (with their reference files) and command wrappers are replaced when the bundled `# seite-skill-version` is newer. Pre-0.21 `theme-builder`/`brand-identity`/`landing-page` skills and the `design-prompts` rule are removed only when unmodified (`LEGACY_BODY_HASHES` in `src/cli/upgrade.rs`: SHA-256 of every released body)

### Content Model Checks
- Frontmatter schemas (`src/content/schema.rs`, design note at the top): `CollectionConfig.fields` (`IndexMap<String, FieldSchema>`) + `strict_fields`. Built-in/top-level keys (`schema::top_level_keys`: `BUILTIN_KEYS` ∪ `Frontmatter`'s serde fields ∪ `[[taxonomies]]` names) validate at the top level; custom fields (`schema::custom_field_names`) at the top level or under `extra:` (validated where they appear; both = error), exposed to templates as `page.extra.<name>` (`schema::page_extra`) and kept in published `.md` copies. `Schema::for_site(config, collection)` validates `ParsedContent.frontmatter_text` with exact key lines → `frontmatter-schema` errors / `frontmatter-schema-unknown` warnings. Wired into build Step 3 (errors join `fatal`, non-drafts unless `--drafts`), `check --file`/post-edit hook (drafts too), `create_content_file` (defaults + placeholders; `reject_invalid` for MCP), and `seite_update_frontmatter` (refuses violations on touched keys; edits an existing top-level custom key in place, routes new custom keys into `extra`). Config mistakes → located `config-invalid` via `schema::validate_config`; unknown schema keys via `unknown_keys.rs`. Presets (`schema::preset_fields`) are never `required` and have no bounds; list-of-string items accept numbers/bools (like the parser); `seite upgrade` adds no schemas.
- Scheduled publishing (`src/content/schedule.rs`): opt-in `[build] schedule = true` (init writes it; absent = off; upgrade only shows the one-time `schedule-feature` notice), dated collections only. `Schedule::new(config).is_published(collection, fm)` in build Step 3 (future `date` hidden unless `[build] future`; past `expires` hidden). `seite build --future` / `seite check --future` / `seite serve` set `config.build.future`. "Today" = `schedule::today(config)`: `[site] timezone` (chrono-tz IANA or fixed offset), else `chrono::Local` — also used by every writer (new, init starter content, MCP create, serve REPL). `hidden_pages`/`hidden_summary` give the "N scheduled posts hidden; next goes live …" line (build, serve, check info note); `schedule::link_diagnostics` turns broken links to hidden pages into `link-to-scheduled` (check and `build --strict`). `seite check` builds like the real build, then renders scheduled pages in a second scratch build and keeps only their diagnostics. Generated workflows carry `deploy::SCHEDULE_TRIGGER` (daily cron); upgrade adds it (ungated, idempotent, only with `schedule = true`) to byte-identical generated workflows, else a one-time notice.
- `seite check --external` (`src/build/external_links.rs`): runs inside `check_site_full` on the scratch output; ureq, 8 workers / 2 per host, cache `.seite/cache/links.json` via `safe_fs` (gitignored), skip list `DEFAULT_SKIP` + `[check] external_skip`; `external-link-broken` / `external-link-unreachable`. Never in build or hooks; tests use a local tiny_http server.

## Context Rules

Detailed reference guides are in `.claude/rules/` and load automatically when working with matching files:
- `build-pipeline.md` — build pipeline steps, content model, data files, shortcodes
- `seo-guardrails.md` — SEO/GEO meta tags, JSON-LD, robots.txt, i18n in themes
- `templates-themes.md` — template variables, blocks, seite partials, starters + gallery, theme hash table
- `collections-content.md` — 6 presets, i18n, trust center, contact forms, changelog/roadmap
- `cli-commands.md` — subcommands, agent, dev server, workspace, deploy, skills
- `mcp-server.md` — resources, tools, architecture
- `release-deploy.md` — version flow, CI, installers, self-update, upgrade system
- `testing.md` — test patterns, CI gates
- `documentation.md` — docs site, scaffold files, what to update
