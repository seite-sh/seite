---
title: "Coding Agents"
description: "Run your seite site with Claude Code, Codex, OpenCode, Cursor, or Gemini CLI: one AGENTS.md, one /seite skill, check hooks that hand errors back, and no API keys or paid services."
weight: 9
---

seite is built so a coding agent can run a site end to end: write content, design the theme, check its own work, preview, and deploy. Any agent works, including free ones. The agent gets its context from the project itself, and the `seite` CLI does the work that has to be right every time.

## What `seite init` sets up

`seite init --agents claude,codex,opencode,cursor,gemini` (default: all; the interactive picker preselects the agents found on your machine) writes:

- **`AGENTS.md`**: a compact project guide every agent reads. It covers what the site is, the layout, the collections, the content format, the hard rules, and the workflow. Claude Code and Gemini CLI read it through one-line `CLAUDE.md` / `GEMINI.md` files that import it.
- **The `seite` skill**: one routed skill, `/seite <command>` (Codex: `$seite <command>`), with a short `SKILL.md` and one reference file per command in `reference/`. Claude Code reads it from `.claude/skills/seite/`; Codex, Cursor, OpenCode, and Gemini CLI read `.agents/skills/seite/`. OpenCode gets a `/seite` command wrapper in `.opencode/commands/`, Gemini CLI in `.gemini/commands/seite.toml`.
- **Context rules**: path-scoped guides (templates, SEO, i18n, shortcodes, data files, config) in `.claude/rules/` and `.cursor/rules/`, loaded automatically for matching files. Codex, OpenCode, and Gemini CLI find them through the index in `AGENTS.md` (`.agents/rules/` when no Claude or Cursor rules exist).
- **Local docs**: `.seite/docs/` holds the documentation for the installed seite version, so agents look things up without a network call. `seite docs <query>` searches it.
- **Permissions**: `.claude/settings.json`, `.cursor/cli.json`, and `opencode.json` let the agent read the site, edit `content/`, `templates/`, `static/`, `data/`, and `seite.toml`, and run the read-only and site-editing `seite` commands without asking. `seite deploy` still asks. `.env` files are never readable.
- **Check hooks**: see [Check hooks](#check-hooks).
- **Claude desktop preview**: `.claude/launch.json` declares the `seite` dev server, so the Claude desktop app opens the site in its preview pane with live reload.

The MCP server is not wired in by default: coding agents use the CLI. Pass `--mcp` to add it as well (see [MCP Server](/docs/mcp-server)).

Each agent has a one-time step before the project config takes effect. `AGENTS.md` lists them under "Agent setup":

| Agent | One-time step |
|---|---|
| Claude Code | Accept the workspace trust prompt |
| Codex CLI | Trust the project, then approve the hooks in `/hooks` |
| OpenCode | None |
| Cursor | Trust the workspace (`cursor-agent --trust` or the prompt) |
| Gemini CLI | Trust the folder (`gemini trust`) and approve the project hooks |

## The `/seite` skill

| Command | What the agent does |
|---|---|
| `check` | `seite check`, fixes every error at its file and line, checks once more, then stops |
| `new <type> "<title>"` | Creates the file with `seite new`, writes it, checks it |
| `preview` | Opens the running dev server, or starts one (built-in browser in Claude's desktop app and Cursor, else `seite serve --open`) |
| `build` | `seite build --json`, reports broken links, missing assets, diagnostics |
| `deploy` | Checks, runs `seite deploy --dry-run`, asks you before the real deploy |
| `theme [name \| description]` | Applies a bundled theme, or designs a custom one in `templates/base.html` |
| `brand` | Logo, palette (`data/brand.yaml`), favicon |
| `landing [page]` | Homepage or landing page: content in `content/pages/`, template in `templates/` |
| `collection [preset]` | Lists collections or adds a preset |
| `upgrade` | `seite upgrade --check`, then applies it after you agree |
| `hooks [on \| off \| status]` | Manages the check hooks |

With no command, the agent picks one from your request and asks once if two fit. Each session starts with `seite context`, which prints the config, collections, theme, dev server, and pending upgrades in one go.

### Design with impeccable

For design work, install the optional [impeccable](https://github.com/pbakaus/impeccable) skill: `seite skill install impeccable`. When it is present, `/seite theme` uses `/impeccable` for the design decisions and keeps seite's template contract (required blocks, SEO head, i18n, search). It builds directly in `templates/`, previews with `seite serve`, and finishes with `seite check` plus one `impeccable detect dist/`.

## Check hooks

Each agent gets two hooks that run `seite check` for it and hand errors back. Warnings never block.

- **After each edit**: `seite check --hook <agent> --post-edit` checks only the edited file (frontmatter, shortcodes, template syntax, data-file syntax) and returns its errors with the tool result.
- **At the end of a turn**: `seite check --hook <agent>` checks the whole site. With errors, the agent keeps working; it is sent back at most once per turn.

| Agent | Hook config | After edit | End of turn |
|---|---|---|---|
| Claude Code | `.claude/settings.json` | `PostToolUse` (`Write\|Edit\|MultiEdit`) | `Stop` |
| Codex CLI | `.codex/hooks.json` | `PostToolUse` (`apply_patch`) | `Stop` |
| Cursor | `.cursor/hooks.json` | `postToolUse` (`Write`) | `stop` |
| OpenCode | `.opencode/plugins/seite-check.js` | `tool.execute.after` | `session.idle` |
| Gemini CLI | `.gemini/settings.json` | `AfterTool` (`write_file\|replace`) | `AfterAgent` |

Every hook command is wrapped so a missing `seite` binary exits 0 silently (`command -v seite >/dev/null 2>&1 && seite check --hook … || true`); a collaborator without seite is never blocked. The wrapper is POSIX shell syntax: on Windows, hooks need Git Bash (Cursor runs hooks in PowerShell there, where the wrapper does not work). The hook exits 0 on every path and is silent outside a seite site.

```bash
seite hooks status                 # installed / missing, per agent
seite hooks off --agents cursor    # remove seite's hook entries only
seite hooks on                     # install them again
```

`.seite/config.json` records which agents seite installed hooks for (`hooks_installed`). `seite upgrade` adds hooks only for agents not recorded there, so hooks you turned off or deleted stay off. For recorded agents whose turn-end hook is still in place, upgrade adds the after-edit hook and the missing-binary wrapper.

## `seite agent`

`seite agent` opens a coding agent in the site. The prompt is passed through unchanged, and the agent runs with the project's own permissions:

```bash
seite agent                                     # interactive session
seite agent "write a post about our launch"     # interactive, starting with this prompt
seite agent --once "fix all check errors"       # run once and exit
seite agent --with gemini "add a docs page"     # another agent
```

| `--with` | Binary | One-shot | Install |
|---|---|---|---|
| `claude` (default) | `claude` | `claude -p "<prompt>"` | `npm install -g @anthropic-ai/claude-code` |
| `codex` | `codex` | `codex exec --sandbox workspace-write "<prompt>"` | `npm install -g @openai/codex` |
| `opencode` | `opencode` | `opencode run "<prompt>"` | `curl -fsSL https://opencode.ai/install \| bash` |
| `cursor` | `cursor-agent` | `cursor-agent -p "<prompt>" --output-format text` | `curl https://cursor.com/install -fsS \| bash` |
| `gemini` | `gemini` | `gemini -p "<prompt>"` | `npm install -g @google/gemini-cli` |

`SEITE_AGENT` sets the default harness. Auto-approval stays opt-in: the global `--yes` adds `opencode run --auto`, `cursor-agent --force --approve-mcps`, or `gemini --approval-mode=yolo`. If the binary isn't on `PATH`, `seite agent` fails right away with the install command.

## `seite theme create`

```bash
seite theme create "dark mode with neon green accents"
```

With impeccable installed, this prints how to run `/impeccable` with your description. Otherwise it runs your agent once with `/seite theme <description>` (`$seite theme …` in Codex), then suggests `seite skill install impeccable` for deeper design work.

## Keeping a site current

```bash
seite upgrade --check     # list pending changes (exit 1 when there are some)
seite upgrade             # apply them
seite upgrade --agents claude,gemini   # change the agent selection
seite upgrade --mcp       # also wire in the MCP server
```

Upgrade adds missing files for the selected agents and merges into existing configs without replacing your settings. It replaces the `seite` skill and its references when the bundled version is newer, and refreshes `.seite/docs/` after a seite update. It never deletes files of deselected agents, and it leaves existing MCP config alone unless you pass `--mcp`.

Sites from seite 0.20 and earlier also had the `theme-builder`, `brand-identity`, and `landing-page` skills and a `design-prompts` rule. `/seite theme`, `/seite brand`, and `/seite landing` replace them. Upgrade removes copies that are still exactly as seite wrote them (checked against the content hash of every released version) and keeps edited copies, with a notice.

## Next steps

- [MCP Server](/docs/mcp-server): for chat apps without a shell, such as Claude Desktop
- [CLI Reference](/docs/cli-reference): every flag of `seite agent`, `seite check`, and `seite hooks`
- [Getting Started](/docs/getting-started): create your first site
