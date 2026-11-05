---
paths:
  - ".github/**"
  - "scripts/**"
  - "src/deploy/**"
  - "src/cli/self_update.rs"
  - "src/update_check.rs"
---
# Release & Distribution

## Version
Source of truth: `Cargo.toml` version (semver). Every code change must bump version.
- PATCH: bug fixes, refactors, new build steps
- MINOR: new features, commands, config options
- MAJOR: breaking changes

## Release Flow
1. `scripts/prepare-release.sh` → scaffold changelog entry
2. Fill changelog → bump `Cargo.toml` version → `cargo run -- skill export . --format repo` (the checked-in Claude plugin's `plugin.json` carries the version; `tests/integration/skill_export.rs` fails until it is regenerated) → commit → push
3. CI: auto-tag → matrix builds (macOS/Linux/Windows) → GitHub Release → crates.io → deploy seite.sh
4. `build.rs` generates `releases.md` from changelog at compile time

## CI Workflows
- `release-tag.yml`: detects version changes on main, creates `v{version}` tag. Blocks if changelog missing
- `release.yml`: on `v*` tag — build, release, SLSA provenance, publish crate, deploy site
- Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `CARGO_REGISTRY_TOKEN`

## Markdown negotiation by host
`[deploy] markdown_negotiation` (default false) only acts on Cloudflare (`src/build/access.rs`, build Step 13): every request then invokes the Worker (Workers Free: 100k requests/day), so it stays opt-in; `[access]` Workers always negotiate. Netlify: not generated (redirect conditions can't match `Accept`; Edge Functions aren't deployed by the generated `nwtgck/actions-netlify` workflow); docs carry a manual Edge Function example. GitHub Pages: impossible, `.md` URLs + `sitemap.md` only.

## Installers
- `install.sh`: `curl -fsSL https://seite.sh/install.sh | sh`
- `install.ps1`: `irm https://seite.sh/install.ps1 | iex`

## Self-update (`src/cli/self_update.rs`)
Downloads from GitHub Releases, detects platform via `cfg!()`, verifies SHA256, atomic binary replacement.

## Background Update Check (`src/update_check.rs`)
Runs after every CLI command (except self-update, mcp). Checks `seite.sh/version.txt` every 24h, 3s timeout.

## Project Metadata (`src/meta.rs`)
`.seite/config.json` tracks version. `PageMeta`, `load()`, `write()`, `needs_upgrade()`.

## Upgrade System (`src/cli/upgrade.rs`)
Version-gated steps with Create, MergeJson, Append actions. Non-destructive. `--check` exits 1 if outdated.
